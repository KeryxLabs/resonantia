use std::sync::Arc;

use axum::http::HeaderMap;
use resonantia_core::AppState;

use crate::{
    auth::resolve_user_context,
    has_cloud_sync_tier,
    AppError,
    GatewayContext,
};

pub(crate) async fn resolve_tenant_state(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<Arc<AppState>, AppError> {
    let user_ctx = resolve_user_context(context, headers).await?;

    context
        .tenant_pool
        .state_for(&user_ctx.tenant_id)
        .await
        .map_err(AppError::internal)
}

pub(crate) async fn resolve_sync_tenant_state(
    context: &GatewayContext,
    headers: &HeaderMap,
) -> Result<Arc<AppState>, AppError> {
    let user_ctx = resolve_user_context(context, headers).await?;
    enforce_cloud_sync_entitlement(context, user_ctx.user_id.as_deref()).await?;

    context
        .tenant_pool
        .state_for(&user_ctx.tenant_id)
        .await
        .map_err(AppError::internal)
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
        .ok_or_else(|| {
            AppError::unauthorized("account record missing for authenticated user".to_string())
        })?;

    let effective_tier = account.effective_tier();

    if has_cloud_sync_tier(&effective_tier) {
        return Ok(());
    }

    Err(AppError::forbidden(
        "cloud sync requires resonant or soulful tier; configure your own gateway URL for BYO sync"
            .to_string(),
    ))
}
