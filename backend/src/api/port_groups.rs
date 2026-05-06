use std::sync::Arc;
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde_json::json;

use crate::{
    errors::{AppError, AppResult},
    middleware::AuthUser,
    models::{PortGroup, CreatePortGroup, UpdatePortGroup, ProxyRule},
    state::AppState,
};

/// GET /api/port-groups
pub async fn list(State(state): State<Arc<AppState>>) -> AppResult<Json<Vec<PortGroup>>> {
    let rows = sqlx::query!(
        "SELECT id, name, listen_port, enabled, skip_tls_verify, force_https, created_at, updated_at FROM port_groups ORDER BY listen_port ASC"
    )
    .fetch_all(&state.db)
    .await?;

    let groups: Vec<PortGroup> = rows.into_iter().map(|r| PortGroup {
        id: r.id.unwrap_or_default(),
        name: r.name,
        listen_port: r.listen_port,
        enabled: r.enabled != 0,
        skip_tls_verify: r.skip_tls_verify != 0,
        force_https: r.force_https != 0,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect();
    Ok(Json(groups))
}

/// GET /api/port-groups/:id
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<PortGroup>> {
    let row = sqlx::query!(
        "SELECT id, name, listen_port, enabled, skip_tls_verify, force_https, created_at, updated_at FROM port_groups WHERE id = ?",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(PortGroup {
        id: row.id.unwrap_or_default(),
        name: row.name,
        listen_port: row.listen_port,
        enabled: row.enabled != 0,
        skip_tls_verify: row.skip_tls_verify != 0,
        force_https: row.force_https != 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

/// GET /api/port-groups/:id/rules
pub async fn list_rules(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<ProxyRule>>> {
    // Verify group exists
    sqlx::query!("SELECT id FROM port_groups WHERE id = ?", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let rows = sqlx::query!(
        r#"SELECT id, name, domain, target_url, rule_type, redirect_code, port_group_id,
           ssl_enabled, force_https, enabled, status,
           last_checked_at, created_at, updated_at
           FROM proxy_rules WHERE port_group_id = ?
           ORDER BY created_at ASC"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    let rules: Vec<ProxyRule> = rows.into_iter().map(|r| ProxyRule {
        id: r.id.unwrap_or_default(),
        name: r.name,
        domain: r.domain,
        target_url: r.target_url,
        rule_type: r.rule_type,
        redirect_code: r.redirect_code,
        port_group_id: r.port_group_id,
        ssl_enabled: r.ssl_enabled != 0,
        force_https: r.force_https != 0,
        enabled: r.enabled != 0,
        status: r.status,
        last_checked_at: r.last_checked_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect();
    Ok(Json(rules))
}

/// POST /api/port-groups
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<CreatePortGroup>,
) -> AppResult<Json<PortGroup>> {
    if body.listen_port < 1 || body.listen_port > 65535 {
        return Err(AppError::BadRequest("Port must be 1-65535".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();

    // Check if port already in use
    let existing = sqlx::query!("SELECT id FROM port_groups WHERE listen_port = ?", body.listen_port)
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest(format!("Port {} is already in use", body.listen_port)));
    }

    sqlx::query!(
        "INSERT INTO port_groups (id, name, listen_port, enabled, skip_tls_verify, force_https) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        id, body.name, body.listen_port, body.enabled, body.skip_tls_verify, body.force_https
    )
    .execute(&state.db)
    .await?;

    get(State(state), Path(id)).await
}

/// PATCH /api/port-groups/:id
pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePortGroup>,
) -> AppResult<Json<PortGroup>> {
    sqlx::query!("SELECT id FROM port_groups WHERE id = ?", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if let Some(ref name) = body.name {
        sqlx::query!("UPDATE port_groups SET name = ?, updated_at = datetime('now') WHERE id = ?", name, id)
            .execute(&state.db).await?;
    }
    if let Some(port) = body.listen_port {
        if port < 1 || port > 65535 {
            return Err(AppError::BadRequest("Port must be 1-65535".into()));
        }
        sqlx::query!("UPDATE port_groups SET listen_port = ?, updated_at = datetime('now') WHERE id = ?", port, id)
            .execute(&state.db).await?;
    }
    if let Some(enabled) = body.enabled {
        sqlx::query!("UPDATE port_groups SET enabled = ?, updated_at = datetime('now') WHERE id = ?", enabled, id)
            .execute(&state.db).await?;
    }
    if let Some(skip_tls) = body.skip_tls_verify {
        sqlx::query!("UPDATE port_groups SET skip_tls_verify = ?, updated_at = datetime('now') WHERE id = ?", skip_tls, id)
            .execute(&state.db).await?;
    }
    if let Some(force) = body.force_https {
        sqlx::query!("UPDATE port_groups SET force_https = ?, updated_at = datetime('now') WHERE id = ?", force, id)
            .execute(&state.db).await?;
    }

    get(State(state), Path(id)).await
}

/// DELETE /api/port-groups/:id
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let res = sqlx::query!("DELETE FROM port_groups WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    // Also remove associated proxy rules
    sqlx::query!("DELETE FROM proxy_rules WHERE port_group_id = ?", id)
        .execute(&state.db)
        .await?;

    tracing::info!("Deleted port group: {id}");
    Ok(Json(json!({ "message": "Deleted" })))
}
