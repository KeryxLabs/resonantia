use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use resonantia_core::{
    create_app_state, get_health, get_graph, initialize_app_state, list_nodes, store_context,
    AppState, GraphResponse, HealthResponse, ListNodesResponse, StoreContextRequest,
    StoreContextResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::engine::any::{connect as surreal_connect, Any as SurrealAny};
use surrealdb::Surreal;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tokio::sync::RwLock;

// ── Account store ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountRecord {
    user_id: String,
    /// ISO 8601 string — stored as plain string to avoid SurrealDB datetime round-trip issues.
    created_at: String,
    tier: String,
}

#[derive(Debug, Serialize)]
struct AccountResponse {
    #[serde(rename = "userId")]
    user_id: String,
    tier: String,
    #[serde(rename = "memberSince")]
    member_since: String,
}

struct AccountStore {
    db: Surreal<SurrealAny>,
}

impl AccountStore {
    async fn open(data_root: &PathBuf) -> Result<Self, String> {
        let accounts_dir = data_root.join("accounts");
        std::fs::create_dir_all(&accounts_dir)
            .map_err(|err| format!("failed to create accounts dir: {err}"))?;

        let endpoint = format!("surrealkv://{}", accounts_dir.display());
        let db: Surreal<SurrealAny> = surreal_connect(&endpoint)
            .await
            .map_err(|err| format!("failed to open accounts db: {err}"))?;

        db.use_ns("resonantia")
            .use_db("accounts")
            .await
            .map_err(|err| format!("failed to select accounts namespace: {err}"))?;

        db.query(
            "DEFINE TABLE IF NOT EXISTS account SCHEMAFULL;\
             DEFINE FIELD IF NOT EXISTS user_id ON TABLE account TYPE string;\
             DEFINE FIELD IF NOT EXISTS created_at ON TABLE account TYPE string;\
             DEFINE FIELD IF NOT EXISTS tier ON TABLE account TYPE string DEFAULT 'free';\
             DEFINE INDEX IF NOT EXISTS idx_account_user_id ON TABLE account FIELDS user_id UNIQUE;",
        )
        .await
        .map_err(|err| format!("failed to define account schema: {err}"))?;

        Ok(Self { db })
    }

    async fn update_tier(&self, user_id: &str, tier: &str) -> Result<Option<AccountRecord>, String> {
        let user_id = user_id.to_string();
        let tier = tier.to_string();
        self.db
            .query("UPDATE account SET tier = $tier WHERE user_id = $user_id")
            .bind(("user_id", user_id.clone()))
            .bind(("tier", tier))
            .await
            .map_err(|err| format!("failed to update account tier: {err}"))?;

        self.get(&user_id).await
    }

    /// Insert the user account if it does not already exist. No-op on duplicate.
    async fn provision(&self, user_id: &str) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        self.db
            .query(
                "INSERT INTO account { user_id: $user_id, created_at: $now, tier: 'free' } \
                 ON DUPLICATE KEY UPDATE user_id = user_id",
            )
            .bind(("user_id", user_id.to_string()))
            .bind(("now", now))
            .await
            .map_err(|err| format!("failed to provision account: {err}"))?;
        Ok(())
    }

    async fn get(&self, user_id: &str) -> Result<Option<AccountRecord>, String> {
        let user_id = user_id.to_string();
        let mut result = self
            .db
            .query("SELECT user_id, created_at, tier FROM account WHERE user_id = $user_id LIMIT 1")
            .bind(("user_id", user_id))
            .await
            .map_err(|err| format!("failed to query account: {err}"))?;

        // SurrealDB v3 — take as serde_json::Value then deserialize.
        let values: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|err| format!("failed to take account results: {err}"))?;

        values
            .into_iter()
            .next()
            .map(|v| {
                serde_json::from_value::<AccountRecord>(v)
                    .map_err(|err| format!("failed to deserialize account: {err}"))
            })
            .transpose()
    }
}

// ── Gateway context ───────────────────────────────────────────────────────────

#[derive(Clone)]
struct GatewayContext {
    tenant_pool: Arc<TenantPool>,
    auth: Arc<AuthResolver>,
    accounts: Arc<AccountStore>,
    admin_secret: Option<String>,
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

struct UserContext {
    tenant_id: String,
    /// Present only when auth mode is Clerk and JWT was verified.
    user_id: Option<String>,
}

#[derive(Clone)]
struct TenantRequestContext {
    state: Arc<AppState>,
    user_id: Option<String>,
}

enum GatewayAuthMode {
    Off,
    Clerk(ClerkAuth),
}

struct AuthResolver {
    mode: GatewayAuthMode,
}

struct ClerkAuth {
    issuer: String,
    audience: Option<String>,
    tenant_claim: String,
    allow_tenant_header_fallback: bool,
    jwks_url: String,
    jwks_cache_ttl: Duration,
    http: reqwest::Client,
    jwks_cache: RwLock<Option<CachedJwks>>,
}

struct CachedJwks {
    fetched_at: Instant,
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Clone, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    n: String,
    e: String,
    alg: Option<String>,
    #[serde(rename = "use")]
    use_field: Option<String>,
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

    let accounts = AccountStore::open(&data_root)
        .await
        .expect("failed to open account store");

    let admin_secret = env::var("RESONANTIA_GATEWAY_ADMIN_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let context = GatewayContext {
        tenant_pool: Arc::new(TenantPool {
            data_root,
            default_tenant,
            states: RwLock::new(HashMap::new()),
        }),
        auth: Arc::new(AuthResolver::from_env().expect("invalid gateway auth configuration")),
        accounts: Arc::new(accounts),
        admin_secret,
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/account", get(account_handler))
        .route("/api/v1/account/tier", axum::routing::patch(update_tier_handler))
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

#[derive(Deserialize)]
struct UpdateTierRequest {
    user_id: String,
    tier: String,
}

fn check_admin_secret(headers: &HeaderMap, secret: &str) -> bool {
    headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == secret)
        .unwrap_or(false)
}

async fn update_tier_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Json(request): Json<UpdateTierRequest>,
) -> Result<Json<AccountResponse>, AppError> {
    let secret = context.admin_secret.as_deref().unwrap_or("");
    if secret.is_empty() || !check_admin_secret(&headers, secret) {
        return Err(AppError::unauthorized("valid x-admin-secret header required".to_string()));
    }

    if !matches!(request.tier.as_str(), "free" | "subscriber") {
        return Err(AppError::bad_request(
            "tier must be 'free' or 'subscriber'".to_string(),
        ));
    }

    let record = context
        .accounts
        .update_tier(&request.user_id, &request.tier)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("account not found".to_string()))?;

    Ok(Json(AccountResponse {
        user_id: record.user_id,
        tier: record.tier,
        member_since: record.created_at,
    }))
}

async fn account_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, AppError> {
    let tenant = resolve_tenant_context(&context, &headers).await?;
    let user_id = tenant.user_id.ok_or_else(|| {
        AppError::unauthorized("account endpoint requires clerk auth".to_string())
    })?;

    let record = context
        .accounts
        .get(&user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::internal("account record not found after provisioning".to_string()))?;

    Ok(Json(AccountResponse {
        user_id: record.user_id,
        tier: record.tier,
        member_since: record.created_at,
    }))
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
    let user_ctx = context
        .auth
        .resolve_user_context(headers, &context.tenant_pool.default_tenant)
        .await?;

    // Auto-provision the account record when we have a real user identity.
    if let Some(ref user_id) = user_ctx.user_id {
        if let Err(err) = context.accounts.provision(user_id).await {
            // Non-fatal: log but don't block the request.
            error!(%err, "account provisioning failed");
        }
    }

    let state = context
        .tenant_pool
        .state_for(&user_ctx.tenant_id)
        .await
        .map_err(AppError::internal)?;

    Ok(TenantRequestContext {
        state,
        user_id: user_ctx.user_id,
    })
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

fn env_flag(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

impl AuthResolver {
    fn from_env() -> Result<Self, String> {
        let mode = env::var("RESONANTIA_GATEWAY_AUTH_MODE")
            .unwrap_or_else(|_| "off".to_string())
            .trim()
            .to_ascii_lowercase();

        if mode.is_empty() || mode == "off" {
            info!("gateway auth mode: off (header/default tenant mode)");
            return Ok(Self {
                mode: GatewayAuthMode::Off,
            });
        }

        if mode != "clerk" {
            return Err(format!(
                "unsupported RESONANTIA_GATEWAY_AUTH_MODE: {mode} (expected off|clerk)"
            ));
        }

        let issuer = env::var("RESONANTIA_GATEWAY_CLERK_ISSUER")
            .map_err(|_| "RESONANTIA_GATEWAY_CLERK_ISSUER is required when auth mode is clerk".to_string())?
            .trim()
            .trim_end_matches('/')
            .to_string();
        if issuer.is_empty() {
            return Err("RESONANTIA_GATEWAY_CLERK_ISSUER cannot be empty".to_string());
        }

        let audience = env::var("RESONANTIA_GATEWAY_CLERK_AUDIENCE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let jwks_url = env::var("RESONANTIA_GATEWAY_CLERK_JWKS_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{issuer}/.well-known/jwks.json"));

        let tenant_claim = env::var("RESONANTIA_GATEWAY_CLERK_TENANT_CLAIM")
            .unwrap_or_else(|_| "org_id".to_string())
            .trim()
            .to_string();
        if tenant_claim.is_empty() {
            return Err("RESONANTIA_GATEWAY_CLERK_TENANT_CLAIM cannot be empty".to_string());
        }

        let jwks_ttl_seconds = env::var("RESONANTIA_GATEWAY_CLERK_JWKS_CACHE_SECONDS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(300);

        let allow_tenant_header_fallback =
            env_flag("RESONANTIA_GATEWAY_ALLOW_TENANT_HEADER_FALLBACK", false);

        info!(
            %issuer,
            %jwks_url,
            tenant_claim = %tenant_claim,
            "gateway auth mode: clerk"
        );

        Ok(Self {
            mode: GatewayAuthMode::Clerk(ClerkAuth {
                issuer,
                audience,
                tenant_claim,
                allow_tenant_header_fallback,
                jwks_url,
                jwks_cache_ttl: Duration::from_secs(jwks_ttl_seconds.max(15)),
                http: reqwest::Client::new(),
                jwks_cache: RwLock::new(None),
            }),
        })
    }

    async fn resolve_user_context(
        &self,
        headers: &HeaderMap,
        default_tenant: &str,
    ) -> Result<UserContext, AppError> {
        match &self.mode {
            GatewayAuthMode::Off => Ok(UserContext {
                tenant_id: tenant_id_from_headers(headers)
                    .unwrap_or_else(|| sanitize_tenant_id(default_tenant)),
                user_id: None,
            }),
            GatewayAuthMode::Clerk(clerk) => clerk.resolve_user_context(headers).await,
        }
    }
}

impl ClerkAuth {
    async fn resolve_user_context(&self, headers: &HeaderMap) -> Result<UserContext, AppError> {
        let token = bearer_token_from_headers(headers)
            .ok_or_else(|| AppError::unauthorized("missing bearer token".to_string()))?;

        let claims = self.verify_token(token).await?;

        // `sub` is always the stable Clerk user ID — use as account identity.
        let user_id = claim_string(&claims, "sub").ok_or_else(|| {
            AppError::unauthorized("token missing sub claim".to_string())
        })?;
        let sanitized_user_id = sanitize_tenant_id(&user_id);

        // Tenant can be org_id (or the configured claim) for multi-org setups,
        // falling back to the user's own sub so solo users get their own space.
        let tenant_id = if let Some(tenant) = claim_string(&claims, &self.tenant_claim) {
            let sanitized = sanitize_tenant_id(&tenant);
            if sanitized != "public" {
                sanitized
            } else {
                sanitized_user_id.clone()
            }
        } else {
            sanitized_user_id.clone()
        };

        if tenant_id == "public" {
            if self.allow_tenant_header_fallback {
                if let Some(header_tenant) = tenant_id_from_headers(headers) {
                    return Ok(UserContext {
                        tenant_id: header_tenant,
                        user_id: Some(sanitized_user_id),
                    });
                }
            }
            return Err(AppError::unauthorized(
                "token did not include a usable tenant claim".to_string(),
            ));
        }

        Ok(UserContext {
            tenant_id,
            user_id: Some(sanitized_user_id),
        })
    }

    async fn verify_token(&self, token: &str) -> Result<Value, AppError> {
        let header = decode_header(token)
            .map_err(|err| AppError::unauthorized(format!("invalid token header: {err}")))?;

        if header.alg != Algorithm::RS256 {
            return Err(AppError::unauthorized(
                "unsupported token algorithm; expected RS256".to_string(),
            ));
        }

        let kid = header
            .kid
            .ok_or_else(|| AppError::unauthorized("token missing kid".to_string()))?;
        let jwk = self
            .jwk_for_kid(&kid)
            .await
            .map_err(AppError::unauthorized)?;

        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|err| AppError::unauthorized(format!("invalid jwk key material: {err}")))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        if let Some(audience) = &self.audience {
            validation.set_audience(&[audience.as_str()]);
        }

        let data = decode::<Value>(token, &decoding_key, &validation)
            .map_err(|err| AppError::unauthorized(format!("token verification failed: {err}")))?;

        Ok(data.claims)
    }

    async fn jwk_for_kid(&self, kid: &str) -> Result<Jwk, String> {
        if let Some(key) = self.find_cached_jwk(kid).await {
            return Ok(key);
        }

        let fetched = self.fetch_jwks().await?;
        fetched
            .into_iter()
            .find(|key| {
                key.kid.as_deref() == Some(kid)
                    && key.kty.eq_ignore_ascii_case("rsa")
                    && key.alg
                        .as_deref()
                        .map(|value| value.eq_ignore_ascii_case("rs256"))
                        .unwrap_or(true)
                    && key
                        .use_field
                        .as_deref()
                        .map(|value| value.eq_ignore_ascii_case("sig"))
                        .unwrap_or(true)
            })
            .ok_or_else(|| format!("no JWK found for kid={kid}"))
    }

    async fn find_cached_jwk(&self, kid: &str) -> Option<Jwk> {
        let guard = self.jwks_cache.read().await;
        let cache = guard.as_ref()?;
        if cache.fetched_at.elapsed() > self.jwks_cache_ttl {
            return None;
        }

        cache
            .keys
            .iter()
            .find(|key| key.kid.as_deref() == Some(kid))
            .cloned()
    }

    async fn fetch_jwks(&self) -> Result<Vec<Jwk>, String> {
        let response = self
            .http
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|err| format!("failed to fetch JWKS: {err}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("failed to fetch JWKS: {status} {body}"));
        }

        let payload = response
            .json::<JwksResponse>()
            .await
            .map_err(|err| format!("failed to parse JWKS payload: {err}"))?;

        {
            let mut guard = self.jwks_cache.write().await;
            *guard = Some(CachedJwks {
                fetched_at: Instant::now(),
                keys: payload.keys.clone(),
            });
        }

        Ok(payload.keys)
    }
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get("authorization")?.to_str().ok()?.trim();
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }

    let trimmed = token.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn claim_string(claims: &Value, key: &str) -> Option<String> {
    claims
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

    fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }

    fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
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
