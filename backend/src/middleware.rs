use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};

use crate::{
    errors::AppError,
    security::{validate_access_token, verify_token_hash},
    state::{get_setting, AppState},
};

/// Authenticated user injected by middleware
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
}

/// JWT auth middleware: validates Bearer token from Authorization header.
/// Also accepts X-API-Key header when API key auth is enabled.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let headers = req.headers().clone();

    let user_id = authenticate(&state, &headers).await?;
    req.extensions_mut().insert(AuthUser { user_id });

    Ok(next.run(req).await)
}

/// Optional API key middleware: only active when api_auth_enabled = true.
/// If enabled, API key in X-API-Key header is the only accepted credential
/// (in addition to normal JWT session).
async fn authenticate(state: &Arc<AppState>, headers: &HeaderMap) -> Result<String, AppError> {
    let jwt_secret = get_setting(&state.db, "jwt_secret").await?;
    let api_auth_enabled = get_setting(&state.db, "api_auth_enabled")
        .await
        .unwrap_or_default()
        == "true";

    // 1. Try Bearer token (JWT first, fall through to API key)
    if let Some(auth_header) = headers.get("Authorization") {
        let header_str = auth_header.to_str().unwrap_or("");
        if let Some(token) = header_str.strip_prefix("Bearer ") {
            // Try JWT first
            if let Ok(user_id) = validate_access_token(token, &jwt_secret) {
                return Ok(user_id);
            }
            // JWT failed — try as API key
            if token.starts_with("unver_") && api_auth_enabled {
                return verify_api_key(state, token).await;
            }
        }
    }

    // 2. Try API key (X-API-Key header) — only if api_auth_enabled = true
    if api_auth_enabled {
        if let Some(api_key) = headers.get("X-API-Key") {
            let key_str = api_key.to_str().unwrap_or("");
            return verify_api_key(state, key_str).await;
        }
    }

    Err(AppError::Unauthorized)
}

async fn verify_api_key(state: &Arc<AppState>, key: &str) -> Result<String, AppError> {
    if key.len() < 6 {
        return Err(AppError::Unauthorized);
    }
    let prefix = &key[..10.min(key.len())];

    let row = sqlx::query!(
        "SELECT id, key_hash FROM api_keys WHERE key_prefix = ? AND enabled = 1",
        prefix
    )
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Database)?;

    let row = row.ok_or(AppError::Unauthorized)?;

    if !verify_token_hash(key, &row.key_hash) {
        return Err(AppError::Unauthorized);
    }

    // Update last_used_at
    let _ = sqlx::query!(
        "UPDATE api_keys SET last_used_at = datetime('now') WHERE id = ?",
        row.id
    )
    .execute(&state.db)
    .await;

    // API key auth doesn't have a real user_id, use a sentinel
    Ok(format!("apikey:{}", row.id.unwrap_or_default()))
}
