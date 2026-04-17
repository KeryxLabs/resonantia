use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use hmac::{Hmac, Mac};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::{trace as sdktrace, Resource};
use sha2::{Digest, Sha256};
use resonantia_core::{
    create_app_state, get_health, get_graph, initialize_app_state, initialize_app_state_remote_strict,
    list_nodes, rename_session, store_context,
    AppState, GraphResponse, HealthResponse, ListNodesResponse, StoreContextRequest,
    StoreContextResponse, RenameSessionRequest, RenameSessionResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::engine::any::{connect as surreal_connect, Any as SurrealAny};
use surrealdb::Surreal;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tokio::sync::RwLock;
use uuid::Uuid;

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
        Self::connect_inner(&endpoint, None, None).await
    }

    async fn open_remote(
        endpoint: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, String> {
        Self::connect_inner(endpoint, Some(username), Some(password)).await
    }

    async fn connect_inner(
        endpoint: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let db: Surreal<SurrealAny> = surreal_connect(endpoint)
            .await
            .map_err(|err| format!("failed to open accounts db: {err}"))?;

        if let (Some(user), Some(pass)) = (username, password) {
            use surrealdb::opt::auth::Root;
            db.signin(Root { username: user.to_string(), password: pass.to_string() })
                .await
                .map_err(|err| format!("failed to sign in to accounts db: {err}"))?;
        }

        db.use_ns("resonantia")
            .use_db("accounts")
            .await
            .map_err(|err| format!("failed to select accounts namespace: {err}"))?;

        db.query(
            "DEFINE TABLE IF NOT EXISTS account SCHEMAFULL;\
             DEFINE FIELD IF NOT EXISTS user_id ON TABLE account TYPE string;\
             DEFINE FIELD IF NOT EXISTS created_at ON TABLE account TYPE string;\
             DEFINE FIELD IF NOT EXISTS tier ON TABLE account TYPE string DEFAULT 'free';\
             DEFINE FIELD IF NOT EXISTS stripe_customer_id ON TABLE account TYPE option<string>;\
             DEFINE INDEX IF NOT EXISTS idx_account_user_id ON TABLE account FIELDS user_id UNIQUE;",
        )
        .await
        .map_err(|err| format!("failed to define account schema: {err}"))?;

        Ok(Self { db })
    }

    async fn set_stripe_customer_id(&self, user_id: &str, customer_id: &str) -> Result<(), String> {
        self.db
            .query("UPDATE account SET stripe_customer_id = $cid WHERE user_id = $user_id")
            .bind(("user_id", user_id.to_string()))
            .bind(("cid", customer_id.to_string()))
            .await
            .map_err(|err| format!("failed to set stripe customer id: {err}"))?;
        Ok(())
    }

    async fn get_stripe_customer_id(&self, user_id: &str) -> Result<Option<String>, String> {
        let mut result = self
            .db
            .query("SELECT stripe_customer_id FROM account WHERE user_id = $user_id LIMIT 1")
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|err| format!("failed to query stripe customer id: {err}"))?;

        let values: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|err| format!("failed to take stripe customer id result: {err}"))?;

        Ok(values
            .into_iter()
            .next()
            .and_then(|v| v["stripe_customer_id"].as_str().map(str::to_string)))
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
struct StripeConfig {
    secret_key: String,
    webhook_secret: String,
    price_id_resonant: String,
    price_id_soulful: String,
    success_url: String,
    cancel_url: String,
}

#[derive(Clone)]
struct GatewayContext {
    tenant_pool: Arc<TenantPool>,
    auth: Arc<AuthResolver>,
    accounts: Arc<AccountStore>,
    admin_secret: Option<String>,
    stripe: Option<Arc<StripeConfig>>,
    ai: Option<Arc<AiConfig>>,
    observability: Arc<ObservabilityConfig>,
}

#[derive(Clone)]
struct ObservabilityConfig {
    request_log_sample_rate: f64,
}

#[derive(Clone)]
struct AiConfig {
    openai_api_key: String,
    openai_model: String,
    openai_base_url: String,
    require_soulful_for_chat: bool,
}

#[derive(Clone)]
struct SurrealConfig {
    endpoint: String,
    namespace: String,
    username: String,
    password: String,
}

struct TenantPool {
    data_root: PathBuf,
    default_tenant: String,
    states: RwLock<HashMap<String, TenantStateEntry>>,
    max_cached_tenants: usize,
    tenant_idle_ttl: Duration,
    surreal: Option<Arc<SurrealConfig>>,
}

struct TenantStateEntry {
    state: Arc<AppState>,
    last_access: Instant,
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiChatRequest {
    messages: Vec<AiMessage>,
    purpose: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiChatResponse {
    content: String,
    provider: String,
    model: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<AiMessage>,
    stream: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenAiChoice {
    message: OpenAiAssistantMessage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenAiAssistantMessage {
    content: String,
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
    token_leeway_seconds: u64,
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
    let telemetry = init_tracing();

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
    let max_cached_tenants = env::var("RESONANTIA_GATEWAY_MAX_CACHED_TENANTS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(256)
        .max(1);
    let tenant_idle_ttl_seconds = env::var("RESONANTIA_GATEWAY_TENANT_IDLE_TTL_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(1800)
        .max(30);
    let tenant_cache_cleanup_seconds = env::var("RESONANTIA_GATEWAY_TENANT_CACHE_CLEANUP_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(300)
        .max(15);

    let surreal_config = {
        let endpoint = env::var("RESONANTIA_SURREALDB_ENDPOINT").ok().filter(|s| !s.trim().is_empty());
        let namespace = env::var("RESONANTIA_SURREALDB_NS").ok().filter(|s| !s.trim().is_empty());
        let username = env::var("RESONANTIA_SURREALDB_USER").ok().filter(|s| !s.trim().is_empty());
        let password = env::var("RESONANTIA_SURREALDB_PASS").ok().filter(|s| !s.trim().is_empty());
        match (endpoint, namespace, username, password) {
            (Some(endpoint), Some(namespace), Some(username), Some(password)) => {
                info!(%endpoint, %namespace, "remote surrealdb configured for node storage");
                Some(Arc::new(SurrealConfig { endpoint, namespace, username, password }))
            }
            _ => {
                info!("no RESONANTIA_SURREALDB_* vars set — falling back to embedded surrealkv per-tenant");
                None
            }
        }
    };

    let accounts = if let Some(ref surreal) = surreal_config {
        AccountStore::open_remote(&surreal.endpoint, &surreal.username, &surreal.password)
            .await
            .expect("failed to open remote account store")
    } else {
        AccountStore::open(&data_root)
            .await
            .expect("failed to open account store")
    };

    let tenant_pool = Arc::new(TenantPool {
        data_root: data_root.clone(),
        default_tenant: default_tenant.clone(),
        states: RwLock::new(HashMap::new()),
        max_cached_tenants,
        tenant_idle_ttl: Duration::from_secs(tenant_idle_ttl_seconds),
        surreal: surreal_config.clone(),
    });

    info!(
        max_cached_tenants,
        tenant_idle_ttl_seconds,
        tenant_cache_cleanup_seconds,
        "tenant cache configured"
    );

    let admin_secret = env::var("RESONANTIA_GATEWAY_ADMIN_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let stripe = {
        let key = env::var("RESONANTIA_STRIPE_SECRET_KEY").ok().filter(|s| !s.trim().is_empty());
        let webhook_secret = env::var("RESONANTIA_STRIPE_WEBHOOK_SECRET").ok().filter(|s| !s.trim().is_empty());
        let price_id_resonant = env::var("RESONANTIA_STRIPE_PRICE_ID_RESONANT").ok().filter(|s| !s.trim().is_empty());
        let price_id_soulful = env::var("RESONANTIA_STRIPE_PRICE_ID_SOULFUL").ok().filter(|s| !s.trim().is_empty());
        match (key, webhook_secret, price_id_resonant, price_id_soulful) {
            (Some(secret_key), Some(webhook_secret), Some(price_id_resonant), Some(price_id_soulful)) => {
                if !looks_like_stripe_price_id(&price_id_resonant) {
                    warn!(price_id_resonant, "stripe resonant tier id does not look like a Price ID (expected prefix 'price_')");
                }
                if !looks_like_stripe_price_id(&price_id_soulful) {
                    warn!(price_id_soulful, "stripe soulful tier id does not look like a Price ID (expected prefix 'price_')");
                }
                let success_url = env::var("RESONANTIA_STRIPE_SUCCESS_URL")
                    .unwrap_or_else(|_| "https://account.resonantia.me?payment=success".to_string());
                let cancel_url = env::var("RESONANTIA_STRIPE_CANCEL_URL")
                    .unwrap_or_else(|_| "https://account.resonantia.me?payment=cancelled".to_string());
                info!("stripe integration enabled");
                Some(Arc::new(StripeConfig { secret_key, webhook_secret, price_id_resonant, price_id_soulful, success_url, cancel_url }))
            }
            _ => {
                info!("stripe integration disabled (missing STRIPE_SECRET_KEY / WEBHOOK_SECRET / PRICE_ID_RESONANT / PRICE_ID_SOULFUL)");
                None
            }
        }
    };

    let observability = Arc::new(read_observability_config());

    let context = GatewayContext {
        tenant_pool: tenant_pool.clone(),
        auth: Arc::new(AuthResolver::from_env().expect("invalid gateway auth configuration")),
        accounts: Arc::new(accounts),
        admin_secret,
        stripe,
        ai: read_ai_config(),
        observability,
    };

    start_tenant_cache_cleanup(
        tenant_pool,
        Duration::from_secs(tenant_cache_cleanup_seconds),
    );

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/account", get(account_handler))
        .route("/api/v1/account/tier", axum::routing::patch(update_tier_handler))
        .route("/api/v1/checkout", post(checkout_handler))
        .route("/api/v1/customer-portal", post(customer_portal_handler))
        .route("/stripe/webhook", post(stripe_webhook_handler))
        .route("/api/v1/ai/chat", post(ai_chat_handler))
        .route("/api/ai/chat", post(ai_chat_handler))
        .route("/ai/chat", post(ai_chat_handler))
        .route("/api/v1/store", post(store_handler))
        .route("/api/store", post(store_handler))
        .route("/store", post(store_handler))
        .route("/api/v1/session/rename", post(rename_session_handler))
        .route("/api/session/rename", post(rename_session_handler))
        .route("/session/rename", post(rename_session_handler))
        .route("/api/v1/nodes", get(list_nodes_handler))
        .route("/api/nodes", get(list_nodes_handler))
        .route("/nodes", get(list_nodes_handler))
        .route("/api/v1/graph", get(graph_handler))
        .route("/api/graph", get(graph_handler))
        .route("/graph", get(graph_handler))
        .layer(axum::middleware::from_fn_with_state(
            context.clone(),
            observability_middleware,
        ))
        .layer(build_cors_layer())
        .with_state(context);

    info!(%bind_addr, "resonantia gateway listening");

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("failed to bind gateway listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("gateway server failed");

    if telemetry.otel_enabled {
        global::shutdown_tracer_provider();
    }
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

    if !matches!(request.tier.as_str(), "free" | "resonant" | "soulful") {
        return Err(AppError::bad_request(
            "tier must be 'free', 'resonant', or 'soulful'".to_string(),
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
    let user_ctx = resolve_user_context(&context, &headers).await?;
    let user_id = user_ctx.user_id.ok_or_else(|| {
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

// ── Stripe ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CheckoutRequest {
    tier: String,
}

#[derive(Serialize)]
struct CheckoutSessionResponse {
    url: String,
}

async fn checkout_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Json(request): Json<CheckoutRequest>,
) -> Result<Json<CheckoutSessionResponse>, AppError> {
    let stripe = context
        .stripe
        .as_ref()
        .ok_or_else(|| AppError::bad_request("stripe is not configured on this gateway".to_string()))?;

    if !matches!(request.tier.as_str(), "resonant" | "soulful") {
        return Err(AppError::bad_request("tier must be 'resonant' or 'soulful'".to_string()));
    }

    let user_ctx = resolve_user_context(&context, &headers).await?;
    let user_id = user_ctx.user_id.ok_or_else(|| {
        AppError::unauthorized("checkout requires clerk auth".to_string())
    })?;

    let price_id = if request.tier == "soulful" {
        stripe.price_id_soulful.as_str()
    } else {
        stripe.price_id_resonant.as_str()
    };
    if !looks_like_stripe_price_id(price_id) {
        return Err(AppError::internal(format!(
            "invalid stripe price id configured for tier '{}': '{}'. Use a Price ID (price_...), not a Product ID (prod_...).",
            request.tier, price_id
        )));
    }
    let tier_str = request.tier.as_str();
    let user_id_str = user_id.as_str();

    // Build form body for Stripe Checkout Session creation.
    let params = [
        ("mode", "subscription"),
        ("allow_promotion_codes", "true"),
        ("line_items[0][price]", price_id),
        ("line_items[0][quantity]", "1"),
        ("success_url", stripe.success_url.as_str()),
        ("cancel_url", stripe.cancel_url.as_str()),
        ("metadata[clerk_user_id]", user_id_str),
        ("metadata[tier]", tier_str),
        ("client_reference_id", user_id_str),
        ("subscription_data[metadata][clerk_user_id]", user_id_str),
        ("subscription_data[metadata][tier]", tier_str),
    ];

    let http = reqwest::Client::new();
    let response = http
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&stripe.secret_key, None::<&str>)
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::internal(format!("stripe request failed: {err}")))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("stripe checkout session failed: {body}")));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| AppError::internal(format!("stripe response parse failed: {err}")))?;

    // Persist the Stripe customer ID from the session so the portal handler can reuse it.
    if let Some(customer_id) = payload["customer"].as_str().filter(|s| !s.is_empty()) {
        if let Err(err) = context.accounts.set_stripe_customer_id(&user_id, customer_id).await {
            error!(%user_id, %err, "failed to persist stripe customer id");
        }
    }

    let url = payload["url"]
        .as_str()
        .ok_or_else(|| AppError::internal("stripe response missing url".to_string()))?
        .to_string();

    Ok(Json(CheckoutSessionResponse { url }))
}

async fn customer_portal_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
) -> Result<Json<CheckoutSessionResponse>, AppError> {
    let stripe = context
        .stripe
        .as_ref()
        .ok_or_else(|| AppError::bad_request("stripe is not configured on this gateway".to_string()))?;

    let user_ctx = resolve_user_context(&context, &headers).await?;
    let user_id = user_ctx.user_id.ok_or_else(|| {
        AppError::unauthorized("customer portal requires clerk auth".to_string())
    })?;

    let customer_id = context
        .accounts
        .get_stripe_customer_id(&user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::bad_request("no stripe customer found for this account — subscribe first".to_string()))?;

    let return_url = format!("{}", stripe.success_url
        .split('?').next().unwrap_or("https://account.resonantia.me"));

    let params = [
        ("customer", customer_id.as_str()),
        ("return_url", return_url.as_str()),
    ];

    let http = reqwest::Client::new();
    let response = http
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(&stripe.secret_key, None::<&str>)
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::internal(format!("stripe billing portal request failed: {err}")))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("stripe billing portal session failed: {body}")));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| AppError::internal(format!("stripe portal response parse failed: {err}")))?;

    let url = payload["url"]
        .as_str()
        .ok_or_else(|| AppError::internal("stripe portal response missing url".to_string()))?
        .to_string();

    Ok(Json(CheckoutSessionResponse { url }))
}

fn verify_stripe_signature(body: &[u8], sig_header: &str, secret: &str) -> bool {
    // Stripe-Signature: t=<timestamp>,v1=<hex_sig>[,v1=<hex_sig2>...]
    let mut timestamp: Option<&str> = None;
    let mut signatures: Vec<&str> = Vec::new();

    for part in sig_header.split(',') {
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t);
        } else if let Some(sig) = part.strip_prefix("v1=") {
            signatures.push(sig);
        }
    }

    let Some(t) = timestamp else { return false };
    if signatures.is_empty() { return false; }

    let signed_payload = format!("{}.{}", t, String::from_utf8_lossy(body));

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(signed_payload.as_bytes());
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);

    // Constant-time comparison: check if any v1 signature matches.
    signatures.iter().any(|sig| {
        sig.len() == expected_hex.len()
            && sig.bytes().zip(expected_hex.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    })
}

fn looks_like_stripe_price_id(value: &str) -> bool {
    value.trim().starts_with("price_")
}

async fn stripe_webhook_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let stripe = match context.stripe.as_ref() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "stripe not configured").into_response(),
    };

    let sig_header = match headers.get("stripe-signature").and_then(|v| v.to_str().ok()) {
        Some(s) => s.to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "missing stripe-signature header").into_response()
        }
    };

    if !verify_stripe_signature(&body, &sig_header, &stripe.webhook_secret) {
        return (StatusCode::UNAUTHORIZED, "invalid stripe signature").into_response();
    }

    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid json").into_response(),
    };

    let event_type = event["type"].as_str().unwrap_or("");
    info!(%event_type, "stripe webhook received");

    let new_tier: Option<String> = match event_type {
        "checkout.session.completed" => {
            let payment_status = event["data"]["object"]["payment_status"].as_str().unwrap_or("");
            if payment_status == "paid" {
                let tier = event["data"]["object"]["metadata"]["tier"]
                    .as_str()
                    .unwrap_or("resonant");
                Some(tier.to_string())
            } else {
                None
            }
        }
        "invoice.payment_succeeded" => {
            let tier = event["data"]["object"]["subscription_details"]["metadata"]["tier"]
                .as_str()
                .unwrap_or("resonant");
            Some(tier.to_string())
        }
        "customer.subscription.deleted" | "invoice.payment_failed" => Some("free".to_string()),
        _ => None,
    };

    let Some(tier) = new_tier else {
        return (StatusCode::OK, "event ignored").into_response();
    };

    // Extract clerk_user_id from metadata (set when creating the checkout session).
    let user_id = event["data"]["object"]["metadata"]["clerk_user_id"]
        .as_str()
        .or_else(|| {
            // For invoice events, it's on the subscription metadata.
            event["data"]["object"]["subscription_details"]["metadata"]["clerk_user_id"].as_str()
        });

    let Some(user_id) = user_id else {
        error!(%event_type, "stripe webhook missing clerk_user_id in metadata");
        return (StatusCode::OK, "no clerk_user_id in metadata, skipped").into_response();
    };

    match context.accounts.update_tier(user_id, &tier).await {
        Ok(Some(_)) => info!(%user_id, %tier, "account tier updated via stripe webhook"),
        Ok(None) => {
            // User hasn't signed in to the gateway yet — provision and set tier.
            if let Err(err) = context.accounts.provision(user_id).await {
                error!(%user_id, %err, "failed to provision account from stripe webhook");
            } else if let Err(err) = context.accounts.update_tier(user_id, &tier).await {
                error!(%user_id, %err, "failed to set tier after provisioning from stripe webhook");
            }
        }
        Err(err) => {
            error!(%user_id, %err, "failed to update tier from stripe webhook");
            return (StatusCode::INTERNAL_SERVER_ERROR, "tier update failed").into_response();
        }
    }

    (StatusCode::OK, "ok").into_response()
}

async fn list_nodes_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Query(query): Query<ListNodesQuery>,
) -> Result<Json<ListNodesResponse>, AppError> {
    let tenant = resolve_sync_tenant_context(&context, &headers).await?;
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
    let tenant = resolve_sync_tenant_context(&context, &headers).await?;
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
    let tenant = resolve_sync_tenant_context(&context, &headers).await?;
    let response = store_context(&tenant.state, request)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(response))
}

async fn rename_session_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Json(request): Json<RenameSessionRequest>,
) -> Result<Json<RenameSessionResponse>, AppError> {
    let tenant = resolve_sync_tenant_context(&context, &headers).await?;
    let response = rename_session(&tenant.state, request)
        .await
        .map_err(AppError::bad_request)?;
    Ok(Json(response))
}

fn has_cloud_sync_tier(tier: &str) -> bool {
    matches!(tier, "resonant" | "soulful")
}

fn has_soulful_tier(tier: &str) -> bool {
    matches!(tier, "soulful")
}

fn read_ai_config() -> Option<Arc<AiConfig>> {
    let openai_api_key = env::var("RESONANTIA_OPENAI_API_KEY")
        .ok()
        .or_else(|| env::var("OPENAI_API_KEY").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(openai_api_key) = openai_api_key else {
        info!("managed ai disabled (missing RESONANTIA_OPENAI_API_KEY/OPENAI_API_KEY)");
        return None;
    };

    let openai_model = env::var("RESONANTIA_OPENAI_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let openai_base_url = env::var("RESONANTIA_OPENAI_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let require_soulful_for_chat = env::var("RESONANTIA_AI_REQUIRE_SOULFUL_FOR_CHAT")
        .ok()
        .and_then(|value| value.trim().parse::<bool>().ok())
        .unwrap_or(true);

    info!(model = %openai_model, base = %openai_base_url, require_soulful_for_chat, "managed ai enabled");
    Some(Arc::new(AiConfig {
        openai_api_key,
        openai_model,
        openai_base_url,
        require_soulful_for_chat,
    }))
}

fn client_kind(headers: &HeaderMap) -> String {
    headers
        .get("x-resonantia-client")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn ai_purpose(input: Option<&str>) -> String {
    input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "chat".to_string())
}

async fn enforce_ai_entitlement(
    context: &GatewayContext,
    user_id: Option<&str>,
    purpose: &str,
    client: &str,
) -> Result<(), AppError> {
    let Some(user_id) = user_id else {
        // BYO/auth-off mode: allow.
        return Ok(());
    };

    let account = context
        .accounts
        .get(user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::unauthorized("account record missing for authenticated user".to_string()))?;

    let allowed = if purpose == "transmutation" {
        has_soulful_tier(&account.tier) || client == "tauri"
    } else if context
        .ai
        .as_ref()
        .map(|ai| ai.require_soulful_for_chat)
        .unwrap_or(true)
    {
        has_soulful_tier(&account.tier)
    } else {
        true
    };

    if allowed {
        return Ok(());
    }

    Err(AppError::forbidden(
        "managed ai requires soulful tier for this operation".to_string(),
    ))
}

async fn call_openai_chat(
    http: &reqwest::Client,
    ai: &AiConfig,
    messages: Vec<AiMessage>,
) -> Result<String, AppError> {
    let url = format!("{}/chat/completions", ai.openai_base_url.trim_end_matches('/'));
    let payload = OpenAiChatRequest {
        model: ai.openai_model.clone(),
        messages,
        stream: false,
    };

    let response = http
        .post(&url)
        .bearer_auth(&ai.openai_api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|err| AppError::internal(format!("openai request failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("openai response failed: {status} {body}")));
    }

    let parsed = response
        .json::<OpenAiChatResponse>()
        .await
        .map_err(|err| AppError::internal(format!("openai response parse failed: {err}")))?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::internal("openai returned empty content".to_string()))?;

    Ok(content)
}

async fn ai_chat_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
    Json(request): Json<AiChatRequest>,
) -> Result<Json<AiChatResponse>, AppError> {
    let ai = context
        .ai
        .as_ref()
        .ok_or_else(|| AppError::bad_request("managed ai is not configured on this gateway".to_string()))?;

    if request.messages.is_empty() {
        return Err(AppError::bad_request("messages are required".to_string()));
    }

    let user_ctx = resolve_user_context(&context, &headers).await?;
    let purpose = ai_purpose(request.purpose.as_deref());
    let client = client_kind(&headers);
    enforce_ai_entitlement(&context, user_ctx.user_id.as_deref(), &purpose, &client).await?;

    let http = reqwest::Client::new();
    let content = call_openai_chat(&http, ai, request.messages).await?;
    Ok(Json(AiChatResponse {
        content,
        provider: "openai".to_string(),
        model: ai.openai_model.clone(),
    }))
}

async fn enforce_cloud_sync_entitlement(
    context: &GatewayContext,
    user_id: Option<&str>,
) -> Result<(), AppError> {
    // In auth-off mode there is no stable user identity/tier mapping.
    let Some(user_id) = user_id else {
        return Ok(());
    };

    let account = context
        .accounts
        .get(user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::unauthorized("account record missing for authenticated user".to_string()))?;

    if has_cloud_sync_tier(&account.tier) {
        return Ok(());
    }

    Err(AppError::forbidden(
        "cloud sync requires resonant or soulful tier; configure your own gateway URL for BYO sync".to_string(),
    ))
}

async fn resolve_tenant_context(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<TenantRequestContext, AppError> {
    let user_ctx = resolve_user_context(context, headers).await?;

    let state = context
        .tenant_pool
        .state_for(&user_ctx.tenant_id)
        .await
        .map_err(AppError::internal)?;

    Ok(TenantRequestContext {
        state,
    })
}

async fn resolve_sync_tenant_context(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<TenantRequestContext, AppError> {
    let user_ctx = resolve_user_context(context, headers).await?;
    enforce_cloud_sync_entitlement(context, user_ctx.user_id.as_deref()).await?;

    let state = context
        .tenant_pool
        .state_for(&user_ctx.tenant_id)
        .await
        .map_err(AppError::internal)?;

    Ok(TenantRequestContext {
        state,
    })
}

async fn resolve_user_context(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<UserContext, AppError> {
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

    Ok(user_ctx)
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

        let token_leeway_seconds = env::var("RESONANTIA_GATEWAY_CLERK_TOKEN_LEEWAY_SECONDS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(60);

        info!(
            %issuer,
            %jwks_url,
            tenant_claim = %tenant_claim,
            token_leeway_seconds,
            "gateway auth mode: clerk"
        );

        Ok(Self {
            mode: GatewayAuthMode::Clerk(ClerkAuth {
                issuer,
                audience,
                tenant_claim,
                allow_tenant_header_fallback,
                token_leeway_seconds,
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
        validation.leeway = self.token_leeway_seconds;
        if let Some(audience) = &self.audience {
            validation.set_audience(&[audience.as_str()]);
        }

        let data = decode::<Value>(token, &decoding_key, &validation).map_err(|err| {
            if matches!(err.kind(), ErrorKind::ExpiredSignature) {
                // Re-parse claims with exp/nbf checks disabled so we can log token timing details.
                let mut relaxed = validation.clone();
                relaxed.validate_exp = false;
                relaxed.validate_nbf = false;

                if let Ok(relaxed_data) = decode::<Value>(token, &decoding_key, &relaxed) {
                    let now = Utc::now().timestamp();
                    let exp = relaxed_data.claims.get("exp").and_then(|v| v.as_i64());
                    let iat = relaxed_data.claims.get("iat").and_then(|v| v.as_i64());
                    let nbf = relaxed_data.claims.get("nbf").and_then(|v| v.as_i64());

                    warn!(
                        now_ts = now,
                        exp_ts = ?exp,
                        iat_ts = ?iat,
                        nbf_ts = ?nbf,
                        leeway_seconds = self.token_leeway_seconds,
                        "clerk token rejected as expired"
                    );

                    if let Some(exp_ts) = exp {
                        return AppError::unauthorized(format!(
                            "token verification failed: ExpiredSignature (now={now}, exp={exp_ts}, leeway={})",
                            self.token_leeway_seconds
                        ));
                    }
                }
            }

            AppError::unauthorized(format!("token verification failed: {err}"))
        })?;

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
            let mut guard = self.states.write().await;
            Self::evict_idle_locked(&mut guard, self.tenant_idle_ttl);
            if let Some(existing) = guard.get_mut(tenant_id) {
                existing.last_access = Instant::now();
                return Ok(existing.state.clone());
            }
            if guard.len() >= self.max_cached_tenants {
                Self::evict_one_lru_locked(&mut guard);
            }
        }

        let state = if let Some(surreal) = &self.surreal {
            // Remote SurrealDB — tenant_id is the database name, no local dir needed.
            let endpoint = surreal.endpoint.clone();
            let namespace = surreal.namespace.clone();
            let database = tenant_id.to_string();
            let username = surreal.username.clone();
            let password = surreal.password.clone();

            tokio::task::spawn_blocking(move || {
                let state = Arc::new(create_app_state());
                initialize_app_state_remote_strict(
                    &state,
                    &endpoint,
                    &namespace,
                    &database,
                    &username,
                    &password,
                )?;
                Ok::<Arc<AppState>, String>(state)
            })
            .await
            .map_err(|err| format!("failed to join remote app-state init task: {err}"))??
        } else {
            // Embedded surrealkv fallback — one file per tenant.
            let tenant_dir = self.data_root.join("tenants").join(tenant_id);

            tokio::task::spawn_blocking(move || {
                let state = Arc::new(create_app_state());
                initialize_app_state(&state, &tenant_dir)?;
                Ok::<Arc<AppState>, String>(state)
            })
            .await
            .map_err(|err| format!("failed to join local app-state init task: {err}"))??
        };

        let mut guard = self.states.write().await;
        Self::evict_idle_locked(&mut guard, self.tenant_idle_ttl);
        if let Some(existing) = guard.get_mut(tenant_id) {
            existing.last_access = Instant::now();
            return Ok(existing.state.clone());
        }
        if guard.len() >= self.max_cached_tenants {
            Self::evict_one_lru_locked(&mut guard);
        }

        guard.insert(
            tenant_id.to_string(),
            TenantStateEntry {
                state: state.clone(),
                last_access: Instant::now(),
            },
        );

        Ok(state)
    }

    async fn evict_idle_tenants(&self) -> usize {
        let mut guard = self.states.write().await;
        Self::evict_idle_locked(&mut guard, self.tenant_idle_ttl)
    }

    fn evict_idle_locked(
        states: &mut HashMap<String, TenantStateEntry>,
        tenant_idle_ttl: Duration,
    ) -> usize {
        let before = states.len();
        states.retain(|_, entry| entry.last_access.elapsed() <= tenant_idle_ttl);
        before.saturating_sub(states.len())
    }

    fn evict_one_lru_locked(states: &mut HashMap<String, TenantStateEntry>) -> Option<String> {
        let oldest_key = states
            .iter()
            .max_by_key(|(_, entry)| entry.last_access.elapsed())
            .map(|(tenant_id, _)| tenant_id.clone())?;
        states.remove(&oldest_key);
        Some(oldest_key)
    }
}

fn start_tenant_cache_cleanup(tenant_pool: Arc<TenantPool>, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            let evicted = tenant_pool.evict_idle_tenants().await;
            if evicted > 0 {
                info!(evicted, "evicted idle tenant states from cache");
            }
        }
    });
}

struct TelemetryRuntime {
    otel_enabled: bool,
}

fn parse_sample_rate_env(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(default)
}

fn read_observability_config() -> ObservabilityConfig {
    let request_log_sample_rate = parse_sample_rate_env(
        "RESONANTIA_GATEWAY_OBS_REQUEST_LOG_SAMPLE_RATE",
        0.2,
    );

    info!(request_log_sample_rate, "gateway request observability configured");
    ObservabilityConfig {
        request_log_sample_rate,
    }
}

fn extract_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

fn should_sample(seed: &str, sample_rate: f64) -> bool {
    if sample_rate <= 0.0 {
        return false;
    }
    if sample_rate >= 1.0 {
        return true;
    }

    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();

    let mut slice = [0u8; 8];
    slice.copy_from_slice(&digest[..8]);
    let unit = (u64::from_be_bytes(slice) as f64) / (u64::MAX as f64);
    unit < sample_rate
}

async fn observability_middleware(
    State(context): State<GatewayContext>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let client = client_kind(request.headers());
    let request_id = extract_request_id(request.headers()).unwrap_or_else(generate_request_id);

    let traceparent = request
        .headers()
        .get("traceparent")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let span = tracing::info_span!(
        "http.request",
        request_id = %request_id,
        method = %method,
        path = %path,
        client = %client,
        traceparent = tracing::field::Empty,
    );

    if let Some(traceparent_value) = traceparent.as_deref() {
        span.record("traceparent", tracing::field::display(traceparent_value));
    }

    global::get_text_map_propagator(|propagator| {
        let parent = propagator.extract(&HeaderExtractor(request.headers()));
        span.set_parent(parent);
    });

    let mut response = next.run(request).instrument(span).await;
    let status_code = response.status().as_u16();
    let duration_ms = started.elapsed().as_millis() as u64;

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }

    let should_log = status_code >= 500
        || should_sample(&request_id, context.observability.request_log_sample_rate);

    if should_log {
        if status_code >= 500 {
            error!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed with server error"
            );
        } else if status_code >= 400 {
            warn!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed with client error"
            );
        } else {
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed"
            );
        }
    }

    response
}

fn build_cors_layer() -> CorsLayer {
    let origins = allowed_origins();
    let mut layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::OPTIONS])
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

fn init_tracing() -> TelemetryRuntime {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("resonantia_gateway=info,tower_http=info"));

    let otel_service_name = env::var("RESONANTIA_OTEL_SERVICE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "resonantia-gateway".to_string());

    let otlp_endpoint = env::var("RESONANTIA_OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let otel_trace_sample_rate = parse_sample_rate_env("RESONANTIA_OTEL_TRACE_SAMPLE_RATE", 0.1);

    global::set_text_map_propagator(TraceContextPropagator::new());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    if let Some(endpoint) = otlp_endpoint {
        let resource = Resource::new(vec![
            KeyValue::new("service.name", otel_service_name.clone()),
            KeyValue::new("service.namespace", "resonantia"),
        ]);

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .build()
            .expect("failed to build OTLP span exporter");

        let tracer_provider = sdktrace::TracerProvider::builder()
            .with_sampler(sdktrace::Sampler::TraceIdRatioBased(otel_trace_sample_rate))
            .with_resource(resource)
            .with_batch_exporter(exporter, Tokio)
            .build();

        let tracer = tracer_provider.tracer(otel_service_name.clone());
        global::set_tracer_provider(tracer_provider);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .init();

        info!(
            %endpoint,
            service = %otel_service_name,
            otel_trace_sample_rate,
            "opentelemetry exporter enabled"
        );

        return TelemetryRuntime { otel_enabled: true };
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    info!("opentelemetry exporter disabled (set RESONANTIA_OTEL_EXPORTER_OTLP_ENDPOINT to enable)");
    TelemetryRuntime { otel_enabled: false }
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

    fn forbidden(message: String) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
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
