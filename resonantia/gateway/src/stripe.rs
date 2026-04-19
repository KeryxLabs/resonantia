use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{error, info};

use crate::{
    auth::resolve_user_context, AppError, GatewayContext,
};

#[derive(Clone)]
pub struct StripeConfig {
    pub secret_key: String,
    pub webhook_secret: String,
    pub webhook_tolerance_seconds: i64,
    pub price_id_resonant: String,
    pub price_id_soulful: String,
    pub success_url: String,
    pub cancel_url: String,
    pub api_base_url: String,
}

#[derive(Deserialize)]
pub(crate) struct CheckoutRequest {
    tier: String,
}

#[derive(Serialize)]
pub(crate) struct CheckoutSessionResponse {
    url: String,
}

pub async fn checkout_handler(
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
    let user_id = user_ctx
        .user_id
        .ok_or_else(|| AppError::unauthorized("checkout requires clerk auth".to_string()))?;

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
    let checkout_url = format!(
        "{}/v1/checkout/sessions",
        stripe.api_base_url.trim_end_matches('/')
    );
    let response = http
        .post(checkout_url)
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

pub async fn customer_portal_handler(
    State(context): State<GatewayContext>,
    headers: HeaderMap,
) -> Result<Json<CheckoutSessionResponse>, AppError> {
    let stripe = context
        .stripe
        .as_ref()
        .ok_or_else(|| AppError::bad_request("stripe is not configured on this gateway".to_string()))?;

    let user_ctx = resolve_user_context(&context, &headers).await?;
    let user_id = user_ctx
        .user_id
        .ok_or_else(|| AppError::unauthorized("customer portal requires clerk auth".to_string()))?;

    let customer_id = context
        .accounts
        .get_stripe_customer_id(&user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            AppError::bad_request("no stripe customer found for this account — subscribe first".to_string())
        })?;

    let return_url = format!(
        "{}",
        stripe
            .success_url
            .split('?')
            .next()
            .unwrap_or("https://account.resonantia.me")
    );

    let params = [
        ("customer", customer_id.as_str()),
        ("return_url", return_url.as_str()),
    ];

    let http = reqwest::Client::new();
    let portal_url = format!(
        "{}/v1/billing_portal/sessions",
        stripe.api_base_url.trim_end_matches('/')
    );
    let response = http
        .post(portal_url)
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

fn parse_stripe_signature(sig_header: &str) -> Option<(i64, Vec<&str>)> {
    // Stripe-Signature: t=<timestamp>,v1=<hex_sig>[,v1=<hex_sig2>...]
    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<&str> = Vec::new();

    for part in sig_header.split(',') {
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = t.trim().parse::<i64>().ok();
        } else if let Some(sig) = part.strip_prefix("v1=") {
            signatures.push(sig);
        }
    }

    match (timestamp, signatures.is_empty()) {
        (Some(ts), false) => Some((ts, signatures)),
        _ => None,
    }
}

fn timestamp_within_tolerance(timestamp: i64, tolerance_seconds: i64) -> bool {
    let tolerance_seconds = tolerance_seconds.max(1);
    let now = Utc::now().timestamp();
    let lower_bound = now.saturating_sub(tolerance_seconds);
    let upper_bound = now.saturating_add(tolerance_seconds);
    (lower_bound..=upper_bound).contains(&timestamp)
}

fn verify_stripe_signature(
    body: &[u8],
    sig_header: &str,
    secret: &str,
    tolerance_seconds: i64,
) -> bool {
    let Some((timestamp, signatures)) = parse_stripe_signature(sig_header) else {
        return false;
    };

    if !timestamp_within_tolerance(timestamp, tolerance_seconds) {
        return false;
    }

    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(signed_payload.as_bytes());
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);

    // Constant-time comparison: check if any v1 signature matches.
    signatures.iter().any(|sig| {
        sig.len() == expected_hex.len()
            && sig
                .bytes()
                .zip(expected_hex.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    })
}

pub fn looks_like_stripe_price_id(value: &str) -> bool {
    value.trim().starts_with("price_")
}

fn webhook_user_id_from_event(object: &serde_json::Value) -> Option<&str> {
    object["metadata"]["clerk_user_id"]
        .as_str()
        .or_else(|| object["subscription_details"]["metadata"]["clerk_user_id"].as_str())
        .or_else(|| object["client_reference_id"].as_str())
        .or_else(|| object["lines"]["data"][0]["metadata"]["clerk_user_id"].as_str())
        .or_else(|| {
            object["lines"]["data"][0]["subscription_item_details"]["metadata"]["clerk_user_id"]
                .as_str()
        })
        .or_else(|| object["parent"]["subscription_details"]["metadata"]["clerk_user_id"].as_str())
}

async fn resolve_webhook_user_id(
    context: &GatewayContext,
    object: &serde_json::Value,
    customer_id: Option<&str>,
) -> Option<String> {
    if let Some(user_id) = webhook_user_id_from_event(object) {
        return Some(user_id.to_string());
    }

    let customer_id = customer_id?;
    match context
        .accounts
        .get_user_id_by_stripe_customer_id(customer_id)
        .await
    {
        Ok(Some(user_id)) => {
            info!(%customer_id, %user_id, "resolved stripe webhook user id from customer mapping");
            Some(user_id)
        }
        Ok(None) => None,
        Err(err) => {
            error!(%customer_id, %err, "failed resolving stripe webhook user id by customer id");
            None
        }
    }
}

fn unix_seconds_to_rfc3339(value: i64) -> Option<String> {
    Utc.timestamp_opt(value, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
}

fn invoice_paid_at(object: &serde_json::Value) -> String {
    object["status_transitions"]["paid_at"]
        .as_i64()
        .and_then(unix_seconds_to_rfc3339)
        .or_else(|| object["created"].as_i64().and_then(unix_seconds_to_rfc3339))
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

fn subscription_cancelled_at(object: &serde_json::Value) -> String {
    object["canceled_at"]
        .as_i64()
        .and_then(unix_seconds_to_rfc3339)
        .or_else(|| object["status_transitions"]["finalized_at"].as_i64().and_then(unix_seconds_to_rfc3339))
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

async fn apply_paid_webhook_update(
    context: &GatewayContext,
    user_id: &str,
    tier: &str,
    paid_at: &str,
) -> Result<(), String> {
    match context
        .accounts
        .record_subscription_payment(user_id, tier, paid_at)
        .await
    {
        Ok(Some(_)) => {
            info!(%user_id, %tier, %paid_at, "account payment recorded via stripe webhook");
            Ok(())
        }
        Ok(None) => {
            context.accounts.provision(user_id).await?;
            context
                .accounts
                .record_subscription_payment(user_id, tier, paid_at)
                .await?;
            info!(%user_id, %tier, %paid_at, "account provisioned and payment recorded via stripe webhook");
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn apply_cancellation_webhook_update(
    context: &GatewayContext,
    user_id: &str,
    cancelled_at: &str,
) -> Result<(), String> {
    match context
        .accounts
        .mark_subscription_cancelled(user_id, cancelled_at)
        .await
    {
        Ok(Some(_)) => {
            info!(%user_id, %cancelled_at, "account cancellation recorded via stripe webhook");
            Ok(())
        }
        Ok(None) => {
            context.accounts.provision(user_id).await?;
            context
                .accounts
                .mark_subscription_cancelled(user_id, cancelled_at)
                .await?;
            info!(%user_id, %cancelled_at, "account provisioned and cancellation recorded via stripe webhook");
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub async fn stripe_webhook_handler(
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
            return (StatusCode::BAD_REQUEST, "missing stripe-signature header").into_response();
        }
    };

    let Some((timestamp, _)) = parse_stripe_signature(&sig_header) else {
        return (StatusCode::UNAUTHORIZED, "invalid stripe signature").into_response();
    };

    if !timestamp_within_tolerance(timestamp, stripe.webhook_tolerance_seconds) {
        let now = Utc::now().timestamp();
        let skew_seconds = now.saturating_sub(timestamp);
        error!(
            timestamp,
            now,
            skew_seconds,
            webhook_tolerance_seconds = stripe.webhook_tolerance_seconds,
            "stripe webhook signature timestamp outside tolerance; verify system clock synchronization",
        );
        return (StatusCode::UNAUTHORIZED, "invalid stripe signature").into_response();
    }

    if !verify_stripe_signature(
        &body,
        &sig_header,
        &stripe.webhook_secret,
        stripe.webhook_tolerance_seconds,
    ) {
        return (StatusCode::UNAUTHORIZED, "invalid stripe signature").into_response();
    }

    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid json").into_response(),
    };

    let event_type = event["type"].as_str().unwrap_or("");
    info!(%event_type, "stripe webhook received");

    let object = &event["data"]["object"];

    let customer_id = object["customer"].as_str().filter(|value| !value.is_empty());
    let user_id = resolve_webhook_user_id(&context, object, customer_id).await;

    // Persist Stripe customer mapping from webhook events as canonical source of truth.
    if let (Some(user_id), Some(customer_id)) = (
        user_id.as_deref(),
        customer_id,
    ) {
        if let Err(err) = context
            .accounts
            .set_stripe_customer_id(user_id, customer_id)
            .await
        {
            error!(%user_id, %customer_id, %err, "failed to persist stripe customer id from webhook");
        }
    }

    enum WebhookAction {
        Ignore,
        RecordPayment { tier: String, paid_at: String },
        RecordCancellation { cancelled_at: String },
    }

    let action = match event_type {
        // Do not grant privileges on checkout completion. Checkout can complete before
        // a durable paid invoice is finalized for subscription flows.
        "checkout.session.completed" => WebhookAction::Ignore,
        "invoice.payment_succeeded" => {
            let paid = object["paid"].as_bool().unwrap_or(false);
            let amount_paid = object["amount_paid"].as_i64().unwrap_or(0);
            if paid && amount_paid > 0 {
                let tier = object["subscription_details"]["metadata"]["tier"]
                    .as_str()
                    .or_else(|| object["metadata"]["tier"].as_str())
                    .unwrap_or("resonant");
                WebhookAction::RecordPayment {
                    tier: tier.to_string(),
                    paid_at: invoice_paid_at(object),
                }
            } else {
                info!(%paid, amount_paid, "ignoring invoice.payment_succeeded without captured payment");
                WebhookAction::Ignore
            }
        }
        "customer.subscription.deleted" | "invoice.payment_failed" => WebhookAction::RecordCancellation {
            cancelled_at: subscription_cancelled_at(object),
        },
        _ => WebhookAction::Ignore,
    };

    let Some(user_id) = user_id.as_deref() else {
        error!(%event_type, "stripe webhook user id resolution failed");
        return (StatusCode::OK, "no user id resolved for event, skipped").into_response();
    };

    let result = match action {
        WebhookAction::Ignore => {
            return (StatusCode::OK, "event ignored").into_response();
        }
        WebhookAction::RecordPayment { tier, paid_at } => {
            apply_paid_webhook_update(&context, user_id, &tier, &paid_at).await
        }
        WebhookAction::RecordCancellation { cancelled_at } => {
            apply_cancellation_webhook_update(&context, user_id, &cancelled_at).await
        }
    };

    if let Err(err) = result {
        error!(%user_id, %err, "failed to apply stripe webhook account update");
        return (StatusCode::INTERNAL_SERVER_ERROR, "account update failed").into_response();
    }

    (StatusCode::OK, "ok").into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use chrono::Utc;
    use hmac::{Hmac, Mac};
    use serde_json::{json, Value};
    use sha2::Sha256;
    use tokio::sync::Mutex;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::accounts::{AccountRecord, AccountsRepo};
    use crate::auth::AuthResolver;
    use crate::tenant_pool::TenantPool;
    use crate::{GatewayContext, ObservabilityConfig};

    use super::{
        checkout_handler, customer_portal_handler, looks_like_stripe_price_id,
        stripe_webhook_handler, verify_stripe_signature, StripeConfig,
    };

    #[derive(Default, Clone)]
    struct MockAccountsState {
        stripe_customer_by_user: HashMap<String, String>,
        tier_by_user: HashMap<String, String>,
        last_paid_at_by_user: HashMap<String, String>,
        cancelled_at_by_user: HashMap<String, String>,
        set_customer_calls: Vec<(String, String)>,
        get_user_by_customer_calls: Vec<String>,
        update_tier_calls: Vec<(String, String)>,
        record_payment_calls: Vec<(String, String, String)>,
        mark_cancelled_calls: Vec<(String, String)>,
        provision_calls: Vec<String>,
        get_customer_calls: Vec<String>,
    }

    #[derive(Default)]
    struct MockAccounts {
        state: Mutex<MockAccountsState>,
    }

    impl MockAccounts {
        async fn seed_customer(&self, user_id: &str, customer_id: &str) {
            let mut state = self.state.lock().await;
            state
                .stripe_customer_by_user
                .insert(user_id.to_string(), customer_id.to_string());
        }

        async fn snapshot(&self) -> MockAccountsState {
            self.state.lock().await.clone()
        }
    }

    #[async_trait]
    impl AccountsRepo for MockAccounts {
        async fn set_stripe_customer_id(&self, user_id: &str, customer_id: &str) -> Result<(), String> {
            let mut state = self.state.lock().await;
            state
                .stripe_customer_by_user
                .insert(user_id.to_string(), customer_id.to_string());
            state
                .set_customer_calls
                .push((user_id.to_string(), customer_id.to_string()));
            Ok(())
        }

        async fn get_stripe_customer_id(&self, user_id: &str) -> Result<Option<String>, String> {
            let mut state = self.state.lock().await;
            state.get_customer_calls.push(user_id.to_string());
            Ok(state.stripe_customer_by_user.get(user_id).cloned())
        }

        async fn get_user_id_by_stripe_customer_id(&self, customer_id: &str) -> Result<Option<String>, String> {
            let mut state = self.state.lock().await;
            state
                .get_user_by_customer_calls
                .push(customer_id.to_string());
            Ok(state
                .stripe_customer_by_user
                .iter()
                .find_map(|(user_id, cid)| {
                    if cid == customer_id {
                        Some(user_id.clone())
                    } else {
                        None
                    }
                }))
        }

        async fn update_tier(&self, user_id: &str, tier: &str) -> Result<Option<AccountRecord>, String> {
            let mut state = self.state.lock().await;
            state.update_tier_calls.push((user_id.to_string(), tier.to_string()));
            state
                .tier_by_user
                .insert(user_id.to_string(), tier.to_string());
            Ok(Some(AccountRecord {
                user_id: user_id.to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                tier: tier.to_string(),
                last_paid_at: state.last_paid_at_by_user.get(user_id).cloned(),
                subscription_cancelled_at: state.cancelled_at_by_user.get(user_id).cloned(),
            }))
        }

        async fn record_subscription_payment(
            &self,
            user_id: &str,
            tier: &str,
            paid_at: &str,
        ) -> Result<Option<AccountRecord>, String> {
            let mut state = self.state.lock().await;
            state
                .record_payment_calls
                .push((user_id.to_string(), tier.to_string(), paid_at.to_string()));
            state
                .tier_by_user
                .insert(user_id.to_string(), tier.to_string());
            state
                .last_paid_at_by_user
                .insert(user_id.to_string(), paid_at.to_string());
            state.cancelled_at_by_user.remove(user_id);

            Ok(Some(AccountRecord {
                user_id: user_id.to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                tier: tier.to_string(),
                last_paid_at: Some(paid_at.to_string()),
                subscription_cancelled_at: None,
            }))
        }

        async fn mark_subscription_cancelled(
            &self,
            user_id: &str,
            cancelled_at: &str,
        ) -> Result<Option<AccountRecord>, String> {
            let mut state = self.state.lock().await;
            state
                .mark_cancelled_calls
                .push((user_id.to_string(), cancelled_at.to_string()));
            state
                .cancelled_at_by_user
                .insert(user_id.to_string(), cancelled_at.to_string());

            let tier = state
                .tier_by_user
                .get(user_id)
                .cloned()
                .unwrap_or_else(|| "free".to_string());

            Ok(Some(AccountRecord {
                user_id: user_id.to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                tier,
                last_paid_at: state.last_paid_at_by_user.get(user_id).cloned(),
                subscription_cancelled_at: Some(cancelled_at.to_string()),
            }))
        }

        async fn provision(&self, user_id: &str) -> Result<(), String> {
            let mut state = self.state.lock().await;
            state.provision_calls.push(user_id.to_string());
            state
                .tier_by_user
                .entry(user_id.to_string())
                .or_insert_with(|| "free".to_string());
            Ok(())
        }

        async fn get(&self, user_id: &str) -> Result<Option<AccountRecord>, String> {
            let state = self.state.lock().await;
            Ok(state.tier_by_user.get(user_id).map(|tier| AccountRecord {
                user_id: user_id.to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                tier: tier.clone(),
                last_paid_at: state.last_paid_at_by_user.get(user_id).cloned(),
                subscription_cancelled_at: state.cancelled_at_by_user.get(user_id).cloned(),
            }))
        }
    }

    fn build_test_context(accounts: Arc<dyn AccountsRepo>, stripe: Arc<StripeConfig>) -> GatewayContext {
        let tenant_pool = Arc::new(TenantPool::new(
            std::env::temp_dir().join(format!("resonantia-gateway-test-{}", Uuid::new_v4())),
            "public".to_string(),
            8,
            Duration::from_secs(60),
            None,
        ));

        GatewayContext {
            tenant_pool,
            auth: Arc::new(AuthResolver::off_for_tests()),
            accounts,
            admin_secret: None,
            stripe: Some(stripe),
            ai: None,
            observability: Arc::new(ObservabilityConfig {
                request_log_sample_rate: 1.0,
            }),
        }
    }

    fn build_test_stripe_config(api_base_url: String) -> Arc<StripeConfig> {
        Arc::new(StripeConfig {
            secret_key: "sk_test_123".to_string(),
            webhook_secret: "whsec_test_123".to_string(),
            webhook_tolerance_seconds: 300,
            price_id_resonant: "price_resonant_test".to_string(),
            price_id_soulful: "price_soulful_test".to_string(),
            success_url: "https://account.resonantia.me?payment=success".to_string(),
            cancel_url: "https://account.resonantia.me?payment=cancelled".to_string(),
            api_base_url,
        })
    }

    async fn spawn_mock_stripe_server() -> String {
        let app = Router::new()
            .route(
                "/v1/checkout/sessions",
                post(|| async {
                    Json(json!({
                        "url": "https://checkout.mock/session",
                        "customer": "cus_mock_123"
                    }))
                }),
            )
            .route(
                "/v1/billing_portal/sessions",
                post(|| async {
                    Json(json!({
                        "url": "https://portal.mock/session"
                    }))
                }),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock stripe listener should bind");
        let addr = listener
            .local_addr()
            .expect("mock stripe listener should have local addr");

        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                panic!("mock stripe server failed: {err}");
            }
        });

        format!("http://{addr}")
    }

    #[tokio::test]
    async fn checkout_route_returns_url_and_persists_customer_id() {
        let mock_server_base = spawn_mock_stripe_server().await;
        let stripe = build_test_stripe_config(mock_server_base);
        let mock_accounts = Arc::new(MockAccounts::default());
        let accounts: Arc<dyn AccountsRepo> = mock_accounts.clone();

        let app = Router::new()
            .route("/api/v1/checkout", post(checkout_handler))
            .with_state(build_test_context(accounts, stripe));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/checkout")
                    .header("content-type", "application/json")
                    .header("x-resonantia-test-user-id", "user_123")
                    .body(Body::from(r#"{"tier":"resonant"}"#))
                    .expect("checkout request should build"),
            )
            .await
            .expect("checkout request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("checkout response body should read");
        let payload: Value = serde_json::from_slice(&body).expect("checkout response should be json");
        assert_eq!(payload["url"], "https://checkout.mock/session");

        let state = mock_accounts.snapshot().await;
        assert_eq!(
            state.set_customer_calls,
            vec![("user_123".to_string(), "cus_mock_123".to_string())]
        );
        assert_eq!(state.provision_calls, vec!["user_123".to_string()]);
    }

    #[tokio::test]
    async fn customer_portal_route_returns_url_for_existing_customer() {
        let mock_server_base = spawn_mock_stripe_server().await;
        let stripe = build_test_stripe_config(mock_server_base);
        let mock_accounts = Arc::new(MockAccounts::default());
        mock_accounts.seed_customer("user_456", "cus_existing_456").await;
        let accounts: Arc<dyn AccountsRepo> = mock_accounts.clone();

        let app = Router::new()
            .route("/api/v1/customer-portal", post(customer_portal_handler))
            .with_state(build_test_context(accounts, stripe));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/customer-portal")
                    .header("x-resonantia-test-user-id", "user_456")
                    .body(Body::empty())
                    .expect("customer portal request should build"),
            )
            .await
            .expect("customer portal request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("customer portal response body should read");
        let payload: Value = serde_json::from_slice(&body)
            .expect("customer portal response should be json");
        assert_eq!(payload["url"], "https://portal.mock/session");

        let state = mock_accounts.snapshot().await;
        assert_eq!(state.get_customer_calls, vec!["user_456".to_string()]);
        assert_eq!(state.provision_calls, vec!["user_456".to_string()]);
    }

    #[tokio::test]
    async fn webhook_route_records_payment_and_persists_customer_for_paid_invoice() {
        let stripe = build_test_stripe_config("http://127.0.0.1:9".to_string());
        let mock_accounts = Arc::new(MockAccounts::default());
        let accounts: Arc<dyn AccountsRepo> = mock_accounts.clone();

        let app = Router::new()
            .route("/stripe/webhook", post(stripe_webhook_handler))
            .with_state(build_test_context(accounts, stripe.clone()));

        let body = json!({
            "type": "invoice.payment_succeeded",
            "data": {
                "object": {
                    "paid": true,
                    "amount_paid": 1500,
                    "customer": "cus_webhook_789",
                    "status_transitions": {
                        "paid_at": 1710000000
                    },
                    "subscription_details": {
                        "metadata": {
                            "clerk_user_id": "user_789",
                            "tier": "soulful"
                        }
                    },
                    "metadata": {
                        "clerk_user_id": "user_789",
                        "tier": "soulful"
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).expect("webhook body should serialize");

        let timestamp = Utc::now().timestamp().to_string();
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(&body_bytes));
        let mut mac = Hmac::<Sha256>::new_from_slice(stripe.webhook_secret.as_bytes())
            .expect("hmac init should succeed");
        mac.update(signed_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let sig_header = format!("t={},v1={}", timestamp, signature);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/stripe/webhook")
                    .header("stripe-signature", sig_header)
                    .header("content-type", "application/json")
                    .body(Body::from(body_bytes))
                    .expect("webhook request should build"),
            )
            .await
            .expect("webhook request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let state = mock_accounts.snapshot().await;
        assert_eq!(
            state.set_customer_calls,
            vec![("user_789".to_string(), "cus_webhook_789".to_string())]
        );
        assert_eq!(state.update_tier_calls, Vec::<(String, String)>::new());
        assert_eq!(state.record_payment_calls.len(), 1);
        assert_eq!(state.record_payment_calls[0].0, "user_789");
        assert_eq!(state.record_payment_calls[0].1, "soulful");
    }

    #[tokio::test]
    async fn webhook_route_resolves_user_from_customer_mapping_when_metadata_missing() {
        let stripe = build_test_stripe_config("http://127.0.0.1:9".to_string());
        let mock_accounts = Arc::new(MockAccounts::default());
        mock_accounts
            .seed_customer("user_from_customer", "cus_lookup_123")
            .await;
        let accounts: Arc<dyn AccountsRepo> = mock_accounts.clone();

        let app = Router::new()
            .route("/stripe/webhook", post(stripe_webhook_handler))
            .with_state(build_test_context(accounts, stripe.clone()));

        let body = json!({
            "type": "invoice.payment_succeeded",
            "data": {
                "object": {
                    "paid": true,
                    "amount_paid": 1900,
                    "customer": "cus_lookup_123",
                    "status_transitions": {
                        "paid_at": 1710000200
                    },
                    "metadata": {
                        "tier": "resonant"
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).expect("webhook body should serialize");

        let timestamp = Utc::now().timestamp().to_string();
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(&body_bytes));
        let mut mac = Hmac::<Sha256>::new_from_slice(stripe.webhook_secret.as_bytes())
            .expect("hmac init should succeed");
        mac.update(signed_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let sig_header = format!("t={},v1={}", timestamp, signature);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/stripe/webhook")
                    .header("stripe-signature", sig_header)
                    .header("content-type", "application/json")
                    .body(Body::from(body_bytes))
                    .expect("webhook request should build"),
            )
            .await
            .expect("webhook request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let state = mock_accounts.snapshot().await;
        assert_eq!(state.get_user_by_customer_calls, vec!["cus_lookup_123".to_string()]);
        assert_eq!(state.record_payment_calls.len(), 1);
        assert_eq!(state.record_payment_calls[0].0, "user_from_customer");
    }

    #[tokio::test]
    async fn webhook_route_records_cancellation_without_immediate_downgrade() {
        let stripe = build_test_stripe_config("http://127.0.0.1:9".to_string());
        let mock_accounts = Arc::new(MockAccounts::default());
        let accounts: Arc<dyn AccountsRepo> = mock_accounts.clone();

        let app = Router::new()
            .route("/stripe/webhook", post(stripe_webhook_handler))
            .with_state(build_test_context(accounts, stripe.clone()));

        let body = json!({
            "type": "customer.subscription.deleted",
            "data": {
                "object": {
                    "canceled_at": 1710000100,
                    "metadata": {
                        "clerk_user_id": "user_cancel_1"
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).expect("webhook body should serialize");

        let timestamp = Utc::now().timestamp().to_string();
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(&body_bytes));
        let mut mac = Hmac::<Sha256>::new_from_slice(stripe.webhook_secret.as_bytes())
            .expect("hmac init should succeed");
        mac.update(signed_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let sig_header = format!("t={},v1={}", timestamp, signature);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/stripe/webhook")
                    .header("stripe-signature", sig_header)
                    .header("content-type", "application/json")
                    .body(Body::from(body_bytes))
                    .expect("webhook request should build"),
            )
            .await
            .expect("webhook request should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let state = mock_accounts.snapshot().await;
        assert_eq!(state.update_tier_calls, Vec::<(String, String)>::new());
        assert_eq!(state.mark_cancelled_calls.len(), 1);
        assert_eq!(state.mark_cancelled_calls[0].0, "user_cancel_1");
    }

    #[test]
    fn looks_like_stripe_price_id_only_accepts_price_prefix() {
        assert!(looks_like_stripe_price_id("price_123"));
        assert!(looks_like_stripe_price_id("  price_abc  "));
        assert!(!looks_like_stripe_price_id("prod_123"));
        assert!(!looks_like_stripe_price_id(""));
    }

    #[test]
    fn verify_stripe_signature_accepts_valid_signature() {
        let body = br#"{"id":"evt_123","type":"invoice.payment_succeeded"}"#;
        let secret = "whsec_test_secret";
        let timestamp = Utc::now().timestamp().to_string();
        let payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("hmac init should succeed");
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let sig_header = format!("t={},v1={}", timestamp, signature);

        assert!(verify_stripe_signature(body, &sig_header, secret, 300));
    }

    #[test]
    fn verify_stripe_signature_rejects_invalid_signature() {
        let body = br#"{"id":"evt_123"}"#;
        let secret = "whsec_test_secret";
        let timestamp = Utc::now().timestamp().to_string();
        let sig_header = format!("t={},v1=deadbeef", timestamp);

        assert!(!verify_stripe_signature(body, &sig_header, secret, 300));
    }

    #[test]
    fn verify_stripe_signature_rejects_stale_timestamps() {
        let body = br#"{"id":"evt_123","type":"invoice.payment_succeeded"}"#;
        let secret = "whsec_test_secret";
        let timestamp = (Utc::now().timestamp() - 400).to_string();
        let payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("hmac init should succeed");
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let sig_header = format!("t={},v1={}", timestamp, signature);

        assert!(!verify_stripe_signature(body, &sig_header, secret, 300));
    }
}
