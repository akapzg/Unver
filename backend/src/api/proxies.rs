use std::sync::Arc;
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde_json::json;

use crate::{
    errors::{AppError, AppResult},
    middleware::AuthUser,
    models::{CreateProxyRule, ProxyRule, UpdateProxyRule},
    state::AppState,
};

/// GET /api/proxies
pub async fn list(State(state): State<Arc<AppState>>) -> AppResult<Json<Vec<ProxyRule>>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, domain, target_url, rule_type, redirect_code, port_group_id,
           ssl_enabled, cert_id, force_https, enabled, status,
           last_checked_at, created_at, updated_at
           FROM proxy_rules ORDER BY created_at DESC"#
    )
    .fetch_all(&state.db)
    .await?;

    let proxies: Vec<ProxyRule> = rows.into_iter().map(|r| ProxyRule {
        id: r.id.unwrap_or_default(),
        name: r.name,
        domain: r.domain,
        target_url: r.target_url,
        rule_type: r.rule_type,
        redirect_code: r.redirect_code,
        port_group_id: r.port_group_id,
        cert_id: r.cert_id,
        ssl_enabled: r.ssl_enabled != 0,
        force_https: r.force_https != 0,
        enabled: r.enabled != 0,
        status: r.status,
        last_checked_at: r.last_checked_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect();
    Ok(Json(proxies))
}

/// GET /api/proxies/:id
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ProxyRule>> {
    let row = sqlx::query!(
        r#"SELECT id, name, domain, target_url, rule_type, redirect_code, port_group_id,
           ssl_enabled, cert_id, force_https, enabled, status,
           last_checked_at, created_at, updated_at
           FROM proxy_rules WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ProxyRule {
        id: row.id.unwrap_or_default(),
        name: row.name,
        domain: row.domain,
        target_url: row.target_url,
        rule_type: row.rule_type,
        redirect_code: row.redirect_code,
        port_group_id: row.port_group_id,
        cert_id: row.cert_id,
        ssl_enabled: row.ssl_enabled != 0,
        force_https: row.force_https != 0,
        enabled: row.enabled != 0,
        status: row.status,
        last_checked_at: row.last_checked_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

/// POST /api/proxies
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<CreateProxyRule>,
) -> AppResult<Json<ProxyRule>> {
    validate_rule(&body.rule_type, &body.domain, &body.target_url)?;

    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO proxy_rules (id, name, domain, target_url, rule_type, redirect_code, port_group_id, ssl_enabled, cert_id, force_https, enabled)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id, body.name, body.domain, body.target_url, body.rule_type, body.redirect_code, body.port_group_id,
        body.ssl_enabled, body.cert_id, body.force_https, body.enabled
    )
    .execute(&state.db)
    .await?;

    let row = sqlx::query!(
        r#"SELECT id, name, domain, target_url, rule_type, redirect_code, port_group_id,
           ssl_enabled, cert_id, force_https, enabled, status,
           last_checked_at, created_at, updated_at
           FROM proxy_rules WHERE id = ?"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    let rule = ProxyRule {
        id: row.id.unwrap_or_default(),
        name: row.name,
        domain: row.domain,
        target_url: row.target_url,
        rule_type: row.rule_type,
        redirect_code: row.redirect_code,
        port_group_id: row.port_group_id,
        cert_id: row.cert_id,
        ssl_enabled: row.ssl_enabled != 0,
        force_https: row.force_https != 0,
        enabled: row.enabled != 0,
        status: row.status,
        last_checked_at: row.last_checked_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    tracing::info!("Created proxy rule: {} -> {}", rule.domain, rule.target_url);
    Ok(Json(rule))
}

/// PATCH /api/proxies/:id
pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProxyRule>,
) -> AppResult<Json<ProxyRule>> {
    sqlx::query!("SELECT id FROM proxy_rules WHERE id = ?", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if let Some(ref url) = body.target_url {
        validate_target_url(url)?;
    }

    if let Some(name) = body.name {
        sqlx::query!("UPDATE proxy_rules SET name = ?, updated_at = datetime('now') WHERE id = ?", name, id)
            .execute(&state.db).await?;
    }
    if let Some(ref domain) = body.domain {
        validate_domain(domain)?;
        sqlx::query!("UPDATE proxy_rules SET domain = ?, updated_at = datetime('now') WHERE id = ?", domain, id)
            .execute(&state.db).await?;
    }
    if let Some(target_url) = body.target_url {
        sqlx::query!("UPDATE proxy_rules SET target_url = ?, updated_at = datetime('now') WHERE id = ?", target_url, id)
            .execute(&state.db).await?;
    }
    if let Some(ssl) = body.ssl_enabled {
        sqlx::query!("UPDATE proxy_rules SET ssl_enabled = ?, updated_at = datetime('now') WHERE id = ?", ssl, id)
            .execute(&state.db).await?;
        // Refresh cert cache — turning SSL on/off affects which domains get certs
        let _ = crate::ssl::load_certs_to_cache(&state).await;
    }
    if let Some(force) = body.force_https {
        sqlx::query!("UPDATE proxy_rules SET force_https = ?, updated_at = datetime('now') WHERE id = ?", force, id)
            .execute(&state.db).await?;
    }
    if let Some(enabled) = body.enabled {
        sqlx::query!("UPDATE proxy_rules SET enabled = ?, updated_at = datetime('now') WHERE id = ?", enabled, id)
            .execute(&state.db).await?;
    }
    if let Some(pg_id) = body.port_group_id {
        sqlx::query!("UPDATE proxy_rules SET port_group_id = ?, updated_at = datetime('now') WHERE id = ?", pg_id, id)
            .execute(&state.db).await?;
    }
    if let Some(ref rt) = body.rule_type {
        sqlx::query!("UPDATE proxy_rules SET rule_type = ?, updated_at = datetime('now') WHERE id = ?", rt, id)
            .execute(&state.db).await?;
    }
    if let Some(cert_id) = body.cert_id {
        sqlx::query!("UPDATE proxy_rules SET cert_id = ?, updated_at = datetime('now') WHERE id = ?", cert_id, id)
            .execute(&state.db).await?;
        // Refresh cert cache to apply cert_id → rule-domain mappings
        let _ = crate::ssl::load_certs_to_cache(&state).await;
    }
    if let Some(code) = body.redirect_code {
        sqlx::query!("UPDATE proxy_rules SET redirect_code = ?, updated_at = datetime('now') WHERE id = ?", code, id)
            .execute(&state.db).await?;
    }

    get(State(state), Path(id)).await
}

/// DELETE /api/proxies/:id
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let res = sqlx::query!("DELETE FROM proxy_rules WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    tracing::info!("Deleted proxy rule: {id}");
    Ok(Json(json!({ "message": "Deleted" })))
}

// ── Validation ───────────────────────────────────────────────────────────────

fn validate_rule(rule_type: &str, domain: &str, target_url: &str) -> AppResult<()> {
    validate_domain(domain)?;
    match rule_type {
        "proxy" | "redirect" => validate_target_url(target_url),
        "tcp" => validate_tcp_backend(target_url),
        _ => Err(AppError::BadRequest("Invalid rule_type".to_string())),
    }
}

fn validate_tcp_backend(addr: &str) -> AppResult<()> {
    if addr.is_empty() { return Err(AppError::BadRequest("TCP backend required".into())); }
    // Accept host:port or host:port format
    if !addr.contains(':') {
        return Err(AppError::BadRequest("TCP backend must be host:port".into()));
    }
    Ok(())
}

fn validate_domain(domain: &str) -> AppResult<()> {
    if domain.is_empty() || domain.len() > 253 {
        return Err(AppError::BadRequest("Invalid domain".to_string()));
    }
    if domain.contains("://") || domain.contains('/') || domain.contains('\\') || domain.contains(' ') {
        return Err(AppError::BadRequest("Domain must be a hostname without scheme or path".to_string()));
    }
    if !domain
        .split('.')
        .all(|part| !part.is_empty() && part.len() <= 63 && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return Err(AppError::BadRequest("Invalid domain label".to_string()));
    }
    Ok(())
}

fn validate_target_url(url: &str) -> AppResult<()> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::BadRequest("Target URL is not valid".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest("Target URL must start with http:// or https://".to_string()));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::BadRequest("Target URL must include a host".to_string()));
    }
    if url.len() > 2048 {
        return Err(AppError::BadRequest("Target URL too long".to_string()));
    }
    Ok(())
}
