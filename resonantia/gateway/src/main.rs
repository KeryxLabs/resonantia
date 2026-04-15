use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use resonantia_core::{
    create_app_state, get_health, get_graph, initialize_app_state, list_nodes, store_context,
    AppState, GraphResponse, HealthResponse, ListNodesResponse, StoreContextRequest,
    StoreContextResponse,
};
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tokio::sync::RwLock;

#[derive(Clone)]
struct GatewayContext {
    tenant_pool: Arc<TenantPool>,
}

struct TenantPool {
    data_root: PathBuf,
    default_tenant: String,
    states: RwLock<HashMap<String, Arc<AppState>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListNodesQuery {
    limit: Option<i32>,
    session_id: Option<String>,
}

#[derive(Clone)]
struct TenantRequestContext {
    state: Arc<AppState>,
}

#[tokio::main]
async fn main() {
    init_tracing();

    let bind_addr = env::var("RESONANTIA_GATEWAY_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8090".to_string())
        .parse::<SocketAddr>()
        .expect("RESONANTIA_GATEWAY_BIND must be a valid socket address");

    let data_root = PathBuf::from(
        env::var("RESONANTIA_GATEWAY_DATA_DIR")
            .unwrap_or_else(|_| "./gateway-data".to_string()),
    );
    let default_tenant = env::var("RESONANTIA_GATEWAY_DEFAULT_TENANT")
        .unwrap_or_else(|_| "public".to_string());

    let context = GatewayContext {
        tenant_pool: Arc::new(TenantPool {
            data_root,
            default_tenant,
            states: RwLock::new(HashMap::new()),
        }),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/store", post(store_handler))
        .route("/api/store", post(store_handler))
        .route("/store", post(store_handler))
        .route("/api/v1/nodes", get(list_nodes_handler))
        .route("/api/nodes", get(list_nodes_handler))
        .route("/nodes", get(list_nodes_handler))
        .route("/api/v1/graph", get(graph_handler))
        .route("/api/graph", get(graph_handler))
        .route("/graph", get(graph_handler))
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(context);

    info!(%bind_addr, "resonantia gateway listening");

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("failed to bind gateway listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("gateway server failed");
}

async fn health_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
) -> Result<Json<HealthResponse>, AppError> {
    let tenant = resolve_tenant_context(&context, &headers).await?;
    let response = get_health(&tenant.state).await.map_err(AppError::internal)?;
    Ok(Json(response))
}

async fn list_nodes_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Query(query): Query<ListNodesQuery>,
) -> Result<Json<ListNodesResponse>, AppError> {
    let tenant = resolve_tenant_context(&context, &headers).await?;
    let response = list_nodes(
        &tenant.state,
        query.limit.unwrap_or(200),
        query.session_id,
    )
    .await
    .map_err(AppError::internal)?;
    Ok(Json(response))
}

async fn graph_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Query(query): Query<ListNodesQuery>,
) -> Result<Json<GraphResponse>, AppError> {
    let tenant = resolve_tenant_context(&context, &headers).await?;
    let response = get_graph(
        &tenant.state,
        query.limit.unwrap_or(200),
        query.session_id,
    )
    .await
    .map_err(AppError::internal)?;
    Ok(Json(response))
}

async fn store_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Json(request): Json<StoreContextRequest>,
) -> Result<Json<StoreContextResponse>, AppError> {
    let tenant = resolve_tenant_context(&context, &headers).await?;
    let response = store_context(&tenant.state, request)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(response))
}

async fn resolve_tenant_context(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<TenantRequestContext, AppError> {
    let tenant_id = tenant_id_from_headers(headers)
        .unwrap_or_else(|| context.tenant_pool.default_tenant.clone());
    let state = context
        .tenant_pool
        .state_for(&tenant_id)
        .await
        .map_err(AppError::internal)?;

    Ok(TenantRequestContext { state })
}

fn tenant_id_from_headers(headers: &HeaderMap) -> Option<String> {
    let candidates = [
        "x-resonantia-tenant",
        "x-tenant-id",
        "x-tenant",
    ];

    for key in candidates {
        let value = headers.get(key)?.to_str().ok()?.trim();
        if !value.is_empty() {
            return Some(sanitize_tenant_id(value));
        }
    }

    None
}

fn sanitize_tenant_id(input: &str) -> String {
    let mut sanitized = String::with_capacity(input.len());

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            sanitized.push(ch);
        }
    }

    let trimmed = sanitized.trim_matches(['-', '_']);
    if trimmed.is_empty() {
        "public".to_string()
    } else {
        trimmed.to_string()
    }
}

impl TenantPool {
    async fn state_for(&self, tenant_id: &str) -> Result<Arc<AppState>, String> {
        {
            let guard = self.states.read().await;
            if let Some(existing) = guard.get(tenant_id) {
                return Ok(existing.clone());
            }
        }

        let tenant_dir = self.data_root.join("tenants").join(tenant_id);
        let state = Arc::new(create_app_state());
        initialize_app_state(&state, &tenant_dir)?;

        let mut guard = self.states.write().await;
        let entry = guard
            .entry(tenant_id.to_string())
            .or_insert_with(|| state.clone());
        Ok(entry.clone())
    }
}

fn build_cors_layer() -> CorsLayer {
    let origins = allowed_origins();
    let mut layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    if origins.is_empty() {
        layer = layer.allow_origin(Any);
    } else {
        let parsed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|origin| HeaderValue::from_str(origin).ok())
            .collect();
        layer = layer.allow_origin(parsed);
    }

    layer
}

fn allowed_origins() -> Vec<String> {
    let raw = env::var("RESONANTIA_GATEWAY_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "https://app.resonantia.me".to_string());

    raw.split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("resonantia_gateway=info,tower_http=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn internal(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        error!(status = %self.status, message = %self.message, "gateway request failed");
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}
