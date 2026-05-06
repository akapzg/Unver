use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    extract::{ConnectInfo, State},
    response::{IntoResponse, Response},
    Extension, Json,
};
use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use serde_json::json;

use crate::{
    errors::{AppError, AppResult},
    logger,
    middleware::AuthUser,
    models::{AuthResponse, ChangePasswordRequest, LoginRequest, SetupRequest},
    security::{
        generate_refresh_token, hash_password, hash_token, issue_access_token, validate_password,
        validate_username, verify_password,
    },
    state::{get_setting, set_setting, AppState},
};

/// GET /api/setup/status
pub async fn setup_status(State(state): State<Arc<AppState>>) -> AppResult<impl IntoResponse> {
    let complete = get_setting(&state.db, "setup_complete").await? == "true";
    Ok(Json(json!({ "setup_complete": complete })))
}

/// POST /api/setup  — first-run only: create admin account
pub async fn do_setup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> AppResult<impl IntoResponse> {
    // Guard: only if not already set up
    if get_setting(&state.db, "setup_complete").await? == "true" {
        return Err(AppError::Forbidden);
    }

    validate_username(&body.username)?;
    validate_password(&body.password)?;

    let id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password(&body.password)?;

    sqlx::query!(
        "INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)",
        id,
        body.username,
        password_hash
    )
    .execute(&state.db)
    .await?;

    set_setting(&state.db, "setup_complete", "true").await?;
    set_setting(&state.db, "username", &body.username).await?;

    tracing::info!("Initial setup complete. Admin user '{}' created.", body.username);
    Ok(Json(json!({ "message": "Setup complete" })))
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    // Resolve real client IP (trusted proxy aware)
    let trusted_str = get_setting(&state.db, "trusted_proxy").await.unwrap_or_default();
    let trusted: Vec<std::net::IpAddr> = trusted_str.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let real_ip = if trusted.contains(&peer.ip()) {
        headers.get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(peer.ip())
    } else {
        peer.ip()
    };

    // Rate limit
    state
        .login_limiter
        .check()
        .map_err(|_| AppError::RateLimited)?;

    // Guard: setup must be done
    if get_setting(&state.db, "setup_complete").await? != "true" {
        return Err(AppError::SetupRequired);
    }

    // Fetch user (use constant-time comparison to avoid timing attacks)
    let user = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE username = ?",
        body.username
    )
    .fetch_optional(&state.db)
    .await?;

    // Always run verify_password even if user not found (to prevent timing oracle)
    let dummy_hash = "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let (user_id, stored_hash) = match user {
        Some(u) => (u.id.unwrap_or_default(), u.password_hash),
        None => (String::new(), dummy_hash.to_string()),
    };

    let valid = verify_password(&body.password, &stored_hash)?;
    if !valid || user_id.is_empty() {
        logger::error(&state.db, &format!("AUTH: Login failed for '{}' from {real_ip}", body.username)).await;
        return Err(AppError::Unauthorized);
    }

    // Issue tokens
    let jwt_secret = get_setting(&state.db, "jwt_secret").await?;
    let access_token = issue_access_token(&user_id, &jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_token(&refresh_token);

    let expires_at = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    let token_id = uuid::Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)",
        token_id,
        user_id,
        refresh_hash,
        expires_at
    )
    .execute(&state.db)
    .await?;

    // Set refresh token as HttpOnly, SameSite=Strict cookie.
    // Not marked Secure — the management UI is plain HTTP on internal networks.
    let cookie = Cookie::build(("unver_refresh", refresh_token))
        .http_only(true)
        // NOTE: secure(false) by design — the management UI runs on plain HTTP
        // (port 19688) in internal networks. SameSite=Strict provides CSRF
        // protection. For public deployments, access the UI through the HTTPS
        // proxy port (443) instead.
        .same_site(SameSite::Strict)
        .path("/api/auth")
        .max_age(time::Duration::days(7))
        .build();

    let jar = jar.add(cookie);

    let resp = AuthResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
    };

logger::info(&state.db, &format!("AUTH: Login success for '{}' from {real_ip}", body.username)).await;
    Ok((jar, Json(resp)).into_response())
}

/// POST /api/auth/refresh
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let refresh_token = jar
        .get("unver_refresh")
        .map(|c| c.value().to_string())
        .ok_or(AppError::Unauthorized)?;

    let token_hash = hash_token(&refresh_token);

    // Find matching refresh token by hash (indexed lookup)
    let row = sqlx::query!(
        "SELECT id, user_id FROM refresh_tokens WHERE token_hash = ? AND expires_at > datetime('now')",
        token_hash
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;

    // Rotate both refresh and access tokens.
    sqlx::query!("DELETE FROM refresh_tokens WHERE id = ?", row.id)
        .execute(&state.db)
        .await?;

    let jwt_secret = get_setting(&state.db, "jwt_secret").await?;
    let access_token = issue_access_token(&row.user_id, &jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_token(&refresh_token);
    let expires_at = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    let token_id = uuid::Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)",
        token_id,
        row.user_id,
        refresh_hash,
        expires_at
    )
    .execute(&state.db)
    .await?;

    let cookie = Cookie::build(("unver_refresh", refresh_token))
        .http_only(true)
        // NOTE: secure(false) by design — the management UI runs on plain HTTP
        // (port 19688) in internal networks. SameSite=Strict provides CSRF
        // protection. For public deployments, access the UI through the HTTPS
        // proxy port (443) instead.
        .same_site(SameSite::Strict)
        .path("/api/auth")
        .max_age(time::Duration::days(7))
        .build();

    let jar = jar.add(cookie);

    Ok((jar, Json(AuthResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
    })))
}

/// POST /api/auth/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    if let Some(cookie) = jar.get("unver_refresh") {
        let token_hash = hash_token(cookie.value());
        // Delete by hash (indexed lookup) — no need to iterate
        let _ = sqlx::query!("DELETE FROM refresh_tokens WHERE token_hash = ?", token_hash)
            .execute(&state.db)
            .await;
    }

    let removal = Cookie::build(("unver_refresh", ""))
        .http_only(true)
        .path("/api/auth")
        .max_age(time::Duration::seconds(0))
        .build();

    let jar = jar.add(removal);
    Ok((jar, Json(json!({ "message": "Logged out" }))))
}

/// POST /api/auth/change-password
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Json(body): Json<ChangePasswordRequest>,
) -> AppResult<impl IntoResponse> {
    validate_password(&body.new_password)?;

    let row = sqlx::query!(
        "SELECT password_hash FROM users WHERE id = ?",
        user.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    if !verify_password(&body.current_password, &row.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let new_hash = hash_password(&body.new_password)?;
    sqlx::query!(
        "UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
        new_hash,
        user.user_id
    )
    .execute(&state.db)
    .await?;

    // Invalidate all refresh tokens for this user
    sqlx::query!("DELETE FROM refresh_tokens WHERE user_id = ?", user.user_id)
        .execute(&state.db)
        .await?;

    Ok(Json(json!({ "message": "Password changed. Please log in again." })))
}
