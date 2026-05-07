use std::sync::Arc;
use axum::{extract::State, Extension, Json};
use serde_json::json;

use crate::{
    ssl::{self, parse_cert_expiry},
    errors::{AppError, AppResult},
    middleware::AuthUser,
    models::{ApiKey, AppSettings, CreateApiKey, NewApiKey, UpdateSettings},
    security::{generate_api_key, hash_token},
    state::{get_secret_setting, get_setting, set_secret_setting, set_setting, AppState},
};

/// GET /api/settings
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<AppSettings>> {
    let api_auth_enabled = get_setting(&state.db, "api_auth_enabled").await? == "true";
    let setup_complete = get_setting(&state.db, "setup_complete").await? == "true";
    let acme_email = get_setting(&state.db, "acme_email").await?;
    let ddns_enabled = get_setting(&state.db, "ddns_enabled").await? == "true";
    let ddns_provider = get_setting(&state.db, "ddns_provider").await?;
    let ddns_cf_token = get_secret_setting(&state.db, "ddns_cf_token").await?;
    let ddns_cf_zone_id = get_setting(&state.db, "ddns_cf_zone_id").await?;
    let ddns_domain = get_setting(&state.db, "ddns_domain").await?;
    let ddns_interval = get_setting(&state.db, "ddns_interval").await?.parse().unwrap_or(300);
    let ddns_ip_source = get_setting(&state.db, "ddns_ip_source").await.unwrap_or_else(|_| "public".to_string());

    let ddns_aliyun_access_key_id = get_secret_setting(&state.db, "ddns_aliyun_access_key_id").await.unwrap_or_default();
    let ddns_aliyun_access_key_secret = get_secret_setting(&state.db, "ddns_aliyun_access_key_secret").await.unwrap_or_default();

    let username = get_setting(&state.db, "username").await.unwrap_or_default();
    let web_port_str = get_setting(&state.db, "web_port").await.unwrap_or_default();
    let web_port: u16 = web_port_str.parse().unwrap_or(19688);
    let web_interface = get_setting(&state.db, "web_interface").await.unwrap_or_else(|_| "0.0.0.0".to_string());
    let panel_lan_only = get_setting(&state.db, "panel_lan_only").await.unwrap_or_default() == "true";
    let ddns_domains = get_setting(&state.db, "ddns_domains").await.unwrap_or_default();

    // Mask token
    let masked_token = if ddns_cf_token.len() > 8 {
        format!("{}****{}", &ddns_cf_token[..4], &ddns_cf_token[ddns_cf_token.len()-4..])
    } else if !ddns_cf_token.is_empty() {
        "****".to_string()
    } else {
        "".to_string()
    };

    Ok(Json(AppSettings {
        api_auth_enabled,
        setup_complete,
        acme_email,
        ddns_enabled,
        ddns_provider,
        ddns_cf_token: masked_token,
        ddns_cf_zone_id,
        ddns_aliyun_access_key_id,
        ddns_aliyun_access_key_secret,
        ddns_domain,
        ddns_domains,
        ddns_interval,
        ddns_ip_source,
        username,
        web_port,
        web_interface,
        panel_lan_only: Some(panel_lan_only),
    }))
}

/// PATCH /api/settings
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<UpdateSettings>,
) -> AppResult<Json<AppSettings>> {
    if let Some(enabled) = body.api_auth_enabled {
        set_setting(&state.db, "api_auth_enabled", if enabled { "true" } else { "false" }).await?;
    }
    if let Some(email) = body.acme_email {
        set_setting(&state.db, "acme_email", &email).await?;
    }
    if let Some(enabled) = body.ddns_enabled {
        set_setting(&state.db, "ddns_enabled", if enabled { "true" } else { "false" }).await?;
    }
    if let Some(token) = body.ddns_cf_token {
        // Skip if token appears to be the masked version (unchanged by user).
        // A masked token has the pattern: first 4 chars + "****" + last 4 chars.
        // Also skip empty tokens to avoid clearing the stored value.
        let is_masked = token.len() > 8
            && token.contains("****")
            && {
                let current = get_secret_setting(&state.db, "ddns_cf_token").await.unwrap_or_default();
                let prefix_ok = current.len() >= 4 && token.starts_with(&current[..4]);
                let suffix_ok = current.len() >= 4 && token.ends_with(&current[current.len()-4..]);
                !current.is_empty() && prefix_ok && suffix_ok
            };
        if !is_masked && !token.is_empty() {
            set_secret_setting(&state.db, "ddns_cf_token", &token).await?;
        }
    }
    if let Some(zone) = body.ddns_cf_zone_id {
        set_setting(&state.db, "ddns_cf_zone_id", &zone).await?;
    }
    if let Some(key_id) = body.ddns_aliyun_access_key_id {
        if !key_id.is_empty() {
            set_secret_setting(&state.db, "ddns_aliyun_access_key_id", &key_id).await?;
        }
    }
    if let Some(secret) = body.ddns_aliyun_access_key_secret {
        if !secret.is_empty() {
            set_secret_setting(&state.db, "ddns_aliyun_access_key_secret", &secret).await?;
        }
    }
    if let Some(domain) = body.ddns_domain {
        set_setting(&state.db, "ddns_domain", &domain).await?;
    }
    if let Some(interval) = body.ddns_interval {
        set_setting(&state.db, "ddns_interval", &interval.to_string()).await?;
    }
    if let Some(source) = body.ddns_ip_source {
        set_setting(&state.db, "ddns_ip_source", &source).await?;
    }
    if let Some(provider) = body.ddns_provider {
        set_setting(&state.db, "ddns_provider", &provider).await?;
    }
    if let Some(domains) = body.ddns_domains {
        set_setting(&state.db, "ddns_domains", &domains).await?;
    }
    if let Some(port) = body.web_port {
        set_setting(&state.db, "web_port", &port.to_string()).await?;
    }
    if let Some(iface) = body.web_interface {
        set_setting(&state.db, "web_interface", &iface).await?;
    }
    if let Some(lan_only) = body.panel_lan_only {
        set_setting(&state.db, "panel_lan_only", if lan_only { "true" } else { "false" }).await?;
    }
    if let Some(proxy) = body.trusted_proxy {
        set_setting(&state.db, "trusted_proxy", &proxy).await?;
    }

    get_settings(State(state), Extension(_user)).await
}

/// GET /api/settings/api-keys
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<ApiKey>>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, key_prefix, enabled, created_at, last_used_at
           FROM api_keys ORDER BY created_at DESC"#
    )
    .fetch_all(&state.db)
    .await?;

    let keys: Vec<ApiKey> = rows.into_iter().map(|r| ApiKey {
        id: r.id.unwrap_or_default(),
        name: r.name,
        key_prefix: r.key_prefix,
        enabled: r.enabled != 0,
        created_at: r.created_at,
        last_used_at: r.last_used_at,
    }).collect();
    Ok(Json(keys))
}

/// POST /api/settings/api-keys
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<CreateApiKey>,
) -> AppResult<Json<NewApiKey>> {
    if body.name.is_empty() || body.name.len() > 64 {
        return Err(AppError::BadRequest("Key name must be 1-64 characters".to_string()));
    }

    let raw_key = generate_api_key();
    let key_hash = hash_token(&raw_key);
    let key_prefix = raw_key[..10.min(raw_key.len())].to_string();
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO api_keys (id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?)",
        id,
        body.name,
        key_hash,
        key_prefix
    )
    .execute(&state.db)
    .await?;

    tracing::info!("Created API key '{}' (prefix: {})", body.name, key_prefix);

    // Return the full key ONCE — it will not be shown again
    Ok(Json(NewApiKey {
        id,
        name: body.name,
        key: raw_key,
        key_prefix,
    }))
}

/// DELETE /api/settings/api-keys/:id
pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let res = sqlx::query!("DELETE FROM api_keys WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(json!({ "message": "API key deleted" })))
}

/// GET /api/system/stats
pub async fn system_stats(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let proxy_count = sqlx::query_scalar!("SELECT COUNT(*) FROM proxy_rules")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0) as i64;

    let active_proxies =
        sqlx::query_scalar!("SELECT COUNT(*) FROM proxy_rules WHERE enabled = 1")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0) as i64;

    let cert_count = sqlx::query_scalar!("SELECT COUNT(*) FROM certificates")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0) as i64;

    let auto_renew_certs = sqlx::query_scalar!("SELECT COUNT(*) FROM certificates WHERE auto_renew = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0) as i64;

    let sys_load = crate::network::get_system_load();
    let uptime_secs = crate::network::host_uptime_secs()
        .unwrap_or_else(|| state.start_time.elapsed().as_secs());
    let db_path = state.config.data_dir.join("unver.db");
    let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(Json(json!({
        "proxy_rules": proxy_count,
        "active_proxies": active_proxies,
        "certificates": cert_count,
        "auto_renew_certs": auto_renew_certs,
        "version": env!("CARGO_PKG_VERSION"),
        "cpu_percent": sys_load.cpu_percent,
        "mem_percent": sys_load.mem_percent,
        "mem_used": sys_load.mem_used,
        "mem_total": sys_load.mem_total,
        "disk_percent": sys_load.disk_percent,
        "uptime_seconds": uptime_secs,
        "db_size_bytes": db_size_bytes,
    })))
}

/// GET /api/system/network
pub async fn network_stats(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let info = {
        let tracker = state.net_tracker.read().await;
        json!({
            "rx_rate": tracker.rx_rate,
            "tx_rate": tracker.tx_rate,
            "total_rx": tracker.total_rx,
            "total_tx": tracker.total_tx,
            "rx_rate_str": crate::network::format_rate(tracker.rx_rate),
            "tx_rate_str": crate::network::format_rate(tracker.tx_rate),
            "total_rx_str": crate::network::format_bytes(tracker.total_rx),
            "total_tx_str": crate::network::format_bytes(tracker.total_tx),
            "container_mode": tracker.container_mode,
        })
    };
    Ok(Json(info))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

/// GET /api/system/logs
pub async fn list_logs(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<LogEntry>>> {
    let rows = sqlx::query_as!(
        LogEntry,
        "SELECT id, level, message, created_at FROM logs ORDER BY id DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

/// GET /api/system/backup
/// Returns a JSON containing proxy rules and non-sensitive settings.
pub async fn export_config(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let rules = sqlx::query!("SELECT * FROM proxy_rules").fetch_all(&state.db).await?;
    let settings = sqlx::query!("SELECT * FROM settings").fetch_all(&state.db).await?;
    let sensitive_settings = [
        "jwt_secret",
        "ddns_cf_token",
        "acme_account_key",
        "acme_challenge_",
    ];

    // Create a simple JSON backup
    let backup = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "proxy_rules": rules.iter().map(|r| json!({
            "name": &r.name, "domain": &r.domain, "target_url": &r.target_url,
            "ssl_enabled": r.ssl_enabled, "force_https": r.force_https, "enabled": r.enabled
        })).collect::<Vec<_>>(),
        "settings": settings.iter()
            .filter(|s| {
                let key = s.key.as_deref().unwrap_or("");
                !sensitive_settings.iter().any(|k| key == *k || key.starts_with(k))
            })
            .map(|s| json!({ "key": &s.key, "value": &s.value }))
            .collect::<Vec<_>>(),
    });

    Ok(Json(backup))
}

/// POST /api/system/restart
/// Detects whether running in a container. In container mode, spawns a new
/// process and exits gracefully so the container runtime's restart policy
/// can bring it back. On bare metal, spawn + exit as before.
pub async fn restart_service(
    State(_state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let in_container = std::path::Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker") || s.contains("containerd"))
            .unwrap_or(false);

    let exe = std::env::current_exe().unwrap_or_default();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if in_container {
            // In Docker: spawn replacement in background, then exit cleanly.
            // The container runtime's restart policy handles the restart.
            let _ = std::process::Command::new(&exe)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        std::process::exit(0);
    });
    Ok(Json(json!({"status": "restarting"})))
}

/// POST /api/system/restore
/// Imports proxy rules and non-sensitive settings.
/// Existing proxy rules are replaced; settings are upserted.
pub async fn import_config(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    // Validate structure
    let rules = body["proxy_rules"].as_array();
    let settings_arr = body["settings"].as_array();
    if rules.is_none() && settings_arr.is_none() {
        return Err(AppError::BadRequest(
            "Backup must contain at least 'proxy_rules' or 'settings' array".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;

    // Import proxy rules: clear existing first, then insert (prevents duplicates)
    if let Some(rules) = rules {
        sqlx::query!("DELETE FROM proxy_rules")
            .execute(&mut *tx)
            .await?;

        for r in rules {
            let name = r["name"]
                .as_str()
                .ok_or_else(|| AppError::BadRequest("Invalid proxy rule: missing name".to_string()))?;
            let domain = r["domain"]
                .as_str()
                .ok_or_else(|| AppError::BadRequest("Invalid proxy rule: missing domain".to_string()))?;
            let target_url = r["target_url"]
                .as_str()
                .ok_or_else(|| AppError::BadRequest("Invalid proxy rule: missing target_url".to_string()))?;

            // Validate fields before inserting
            if name.is_empty() || domain.is_empty() || target_url.is_empty() {
                return Err(AppError::BadRequest(
                    format!("Invalid proxy rule: name/domain/target_url must not be empty")
                ));
            }

            let id = uuid::Uuid::new_v4().to_string();
            let ssl = r["ssl_enabled"].as_bool().unwrap_or(false);
            let force = r["force_https"].as_bool().unwrap_or(false);
            let en = r["enabled"].as_bool().unwrap_or(true);
            sqlx::query!(
                "INSERT INTO proxy_rules (id, name, domain, target_url, ssl_enabled, force_https, enabled)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                id,
                name,
                domain,
                target_url,
                ssl,
                force,
                en,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    // Import settings (upsert, skip sensitive keys)
    if let Some(settings_arr) = settings_arr {
        for s in settings_arr {
            let key = s["key"].as_str().unwrap_or("");
            let val = s["value"].as_str().unwrap_or("");
            if !key.is_empty()
                && !matches!(key, "acme_account_key" | "setup_complete" | "jwt_secret" | "ddns_cf_token" | "ddns_aliyun_access_key_secret")
                && !key.starts_with("acme_challenge_")
            {
                sqlx::query!(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                    key,
                    val
                )
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;

    let imported_rules = rules.map(|r| r.len()).unwrap_or(0);
    let imported_settings = settings_arr.map(|s| s.len()).unwrap_or(0);
    tracing::info!("Config imported: {imported_rules} rules, {imported_settings} settings");

    Ok(Json(json!({
        "message": "Import successful",
        "proxy_rules_imported": imported_rules,
        "settings_imported": imported_settings,
    })))
}

/// POST /api/system/renew-ssl — manually trigger SSL certificate renewal
pub async fn renew_ssl(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    tracing::info!("Manual SSL renewal triggered by user");
    
    crate::ssl::check_and_renew_certs(&state).await
        .map_err(|e| {
            tracing::error!("Manual SSL renewal failed: {e}");
            AppError::Internal(anyhow::anyhow!("SSL renewal failed: {e}"))
        })?;

    Ok(Json(json!({ "message": "SSL renewal check completed" })))
}

// ── DDNS Status ──────────────────────────────────────────────────────────────

/// GET /api/ddns/status — return per-domain DDNS status
pub async fn ddns_status(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<crate::models::DdnsDomainStatus>>> {
    use crate::models::DdnsDomainStatus;
    let domains_str = get_setting(&state.db, "ddns_domains").await.unwrap_or_default();
    let disabled_str = get_setting(&state.db, "ddns_disabled").await.unwrap_or_default();
    let disabled: std::collections::HashSet<&str> = disabled_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    let mut statuses = Vec::new();
    for line in domains_str.lines() {
        let domain = line.trim();
        if domain.is_empty() { continue; }
        let cached_ip = get_setting(&state.db, &format!("ddns_ipv4_{}", domain)).await.unwrap_or_default();
        let cached_ipv6 = get_setting(&state.db, &format!("ddns_ipv6_{}", domain)).await.unwrap_or_default();
        statuses.push(DdnsDomainStatus {
            domain: domain.to_string(),
            ipv4: cached_ip,
            ipv6: if cached_ipv6.is_empty() { None } else { Some(cached_ipv6) },
            enabled: !disabled.contains(domain),
        });
    }
    Ok(Json(statuses))
}

/// PATCH /api/ddns/toggle/:domain — enable/disable a single DDNS domain
pub async fn ddns_toggle(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(domain): axum::extract::Path<String>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let enabled = body["enabled"].as_bool().unwrap_or(true);
    let disabled_str = get_setting(&state.db, "ddns_disabled").await.unwrap_or_default();
    let mut disabled: Vec<&str> = disabled_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if enabled {
        disabled.retain(|d| *d != domain);
    } else if !disabled.contains(&domain.as_str()) {
        disabled.push(&domain);
    }
    set_setting(&state.db, "ddns_disabled", &disabled.join(",")).await?;
    Ok(Json(json!({ "domain": domain, "enabled": enabled })))
}

// ── Certificate CRUD ────────────────────────────────────────────────────────

/// GET /api/certificates
pub async fn list_certificates(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<crate::models::Certificate>>> {
    use crate::models::Certificate;
    let rows = sqlx::query!(
        "SELECT id, domain, expires_at, auto_renew, source, created_at, updated_at FROM certificates ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;
    let certs: Vec<Certificate> = rows.into_iter().map(|r| Certificate {
        id: r.id.unwrap_or_default(),
        domain: r.domain,
        expires_at: r.expires_at,
        auto_renew: r.auto_renew != 0,
        source: r.source,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect();
    Ok(Json(certs))
}

/// POST /api/certificates — issue a new certificate
pub async fn issue_certificate(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Extension(worker): Extension<Arc<crate::ssl_worker::SslWorkerHandle>>,
    Json(body): Json<crate::models::IssueCertRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let primary = body.domain.trim().to_string();
    if primary.is_empty() {
        return Err(AppError::BadRequest("Domain is required".to_string()));
    }

    let mut domains = vec![primary.clone()];
    if let Some(ref sans) = body.sans {
        for s in sans.split(&[',', ';', ' '][..]) {
            let s = s.trim();
            if !s.is_empty() && s != primary && !domains.contains(&s.to_string()) {
                domains.push(s.to_string());
            }
        }
    }

    if let Some(ref email) = body.email {
        let email = email.trim();
        if !email.is_empty() && email.contains('@') {
            set_setting(&state.db, "acme_email", email).await?;
        }
    }

    let email = get_setting(&state.db, "acme_email").await.unwrap_or_default();
    if email.is_empty() {
        return Err(AppError::BadRequest("ACME email not configured".to_string()));
    }

    let provider_name = get_setting(&state.db, "ddns_provider").await.unwrap_or_default();
    if provider_name.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("DDNS provider not configured")));
    }

    let use_staging = get_setting(&state.db, "acme_staging").await.unwrap_or_default() == "true";

    // Register background job
    let job_id = uuid::Uuid::new_v4().to_string();
    let domain_label = domains.join(", ");
    let log_buf_for_worker = {
        let mut jobs = state.background_jobs.lock().await;
        let mut log_bufs = state.background_job_logs.lock().await;
        let logs = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        jobs.insert(job_id.clone(), crate::models::BackgroundJob {
            job_id: job_id.clone(),
            status: "running".to_string(),
            domain: domain_label.clone(),
            logs: vec![],
            result: None,
            error: None,
        });
        log_bufs.insert(job_id.clone(), Arc::clone(&logs));
        logs
    };

    // Send to SSL worker thread (dedicated std::thread, no Send constraint)
    let (tx, rx) = tokio::sync::oneshot::channel();
    let job_id2 = job_id.clone();
    let jobs2 = Arc::clone(&state.background_jobs);
    let db2 = state.db.clone();
    let domain_label2 = domain_label.clone();

    worker.issue(crate::ssl_worker::SslRequest {
        email,
        domains,
        provider_name: provider_name.clone(),
        state: Arc::clone(&state),
        use_staging,
        db: state.db.clone(),
        cert_cache: Arc::clone(&state.cert_cache),
        response: tx,
        log_buf: Some(log_buf_for_worker),
    });

    // Wait for result from worker
    tokio::spawn(async move {
        match rx.await {
            Ok(Ok(result)) => {
                let mut jobs = jobs2.lock().await;
                if let Some(job) = jobs.get_mut(&job_id2) {
                    job.status = "completed".to_string();
                    job.result = Some(format!("Certificate issued for {} (expires {})", domain_label2, result.expires_at));
                }
                crate::logger::info(&db2, &format!("Certificate issued for {}", domain_label2)).await;
            }
            Ok(Err(e)) => {
                let mut jobs = jobs2.lock().await;
                if let Some(job) = jobs.get_mut(&job_id2) {
                    job.status = "failed".to_string();
                    job.error = Some(e.clone());
                }
                crate::logger::error(&db2, &format!("SSL failed: {e}")).await;
            }
            Err(_) => {
                let mut jobs = jobs2.lock().await;
                if let Some(job) = jobs.get_mut(&job_id2) {
                    job.status = "failed".to_string();
                    job.error = Some("Worker communication lost".to_string());
                }
            }
        }
    });

    Ok(Json(json!({"job_id": job_id, "message": "Certificate issuance started"})))
}

/// GET /api/certificates/status/:job_id — poll background job progress
pub async fn certificate_status(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let mut jobs = state.background_jobs.lock().await;
    let log_bufs = state.background_job_logs.lock().await;
    match jobs.get_mut(&job_id) {
        Some(job) => {
            // Drain accumulated logs from the shared buffer
            let logs: Vec<crate::models::LogLine> = if let Some(buf) = log_bufs.get(&job_id) {
                let mut lines = buf.lock().await;
                std::mem::take(&mut *lines)
            } else {
                vec![]
            };
            Ok(Json(json!({
                "job_id": job.job_id,
                "status": job.status,
                "domain": job.domain,
                "logs": logs,
                "result": job.result,
                "error": job.error,
            })))
        }
        None => Err(AppError::NotFound),
    }
}

/// DELETE /api/certificates/:id
pub async fn delete_certificate(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Get domain before deleting
    let domain = sqlx::query_scalar!("SELECT domain FROM certificates WHERE id = ?", id)
        .fetch_optional(&state.db).await?
        .ok_or(AppError::NotFound)?;

    sqlx::query!("DELETE FROM certificates WHERE id = ?", id)
        .execute(&state.db).await?;

    // Disable SSL on all proxy rules using this certificate
    sqlx::query!("UPDATE proxy_rules SET ssl_enabled = 0, updated_at = datetime('now') WHERE domain = ?", domain)
        .execute(&state.db).await?;

    // Remove from TLS cache
    if let Ok(mut cache) = state.cert_cache.write() {
        cache.remove(&domain);
    }

    // Clean up _acme-challenge TXT records from Cloudflare (best-effort)
    let domain2 = domain.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::ssl::cleanup_acme_txt(&state, &domain2).await {
            tracing::warn!("ACME TXT cleanup for {} failed: {}", domain2, e);
        }
    });

    Ok(Json(json!({ "message": "Certificate deleted", "ssl_disabled_for": domain })))
}

/// POST /api/certificates/upload — upload existing certificate (PEM)
pub async fn upload_certificate(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let domain = body["domain"].as_str().unwrap_or("").trim().to_string();
    let cert_pem = body["cert_pem"].as_str().unwrap_or("").trim().to_string();
    let key_pem = body["key_pem"].as_str().unwrap_or("").trim().to_string();

    if domain.is_empty() || cert_pem.is_empty() || key_pem.is_empty() {
        return Err(AppError::BadRequest("domain, cert_pem, and key_pem are required".to_string()));
    }

    if !cert_pem.contains("-----BEGIN CERTIFICATE-----") {
        return Err(AppError::BadRequest("Invalid certificate PEM format".to_string()));
    }
    if !key_pem.contains("-----BEGIN") {
        return Err(AppError::BadRequest("Invalid key PEM format".to_string()));
    }

    let id = uuid::Uuid::new_v4().to_string();

    // Parse real expiration from the uploaded certificate
    let expires_at = match parse_cert_expiry(&cert_pem) {
        Some(exp) => exp,
        None => return Err(AppError::BadRequest("Unable to parse certificate expiration date".to_string())),
    };

    let encrypted_key = ssl::encrypt_key_for_db(&state.db, &key_pem).await?;
    sqlx::query!(
        "INSERT INTO certificates (id, domain, cert_pem, key_pem, expires_at, source, auto_renew)
         VALUES (?, ?, ?, ?, ?, 'manual', 0)
         ON CONFLICT(domain) DO UPDATE SET
           cert_pem = excluded.cert_pem, key_pem = excluded.key_pem,
           expires_at = excluded.expires_at, source = 'manual', auto_renew = 0, updated_at = datetime('now')",
        id, domain, cert_pem, encrypted_key, expires_at
    ).execute(&state.db).await?;

    // Push into live SNI cache immediately (no restart needed)
    if let Ok(ck) = ssl::load_cert_into_cache(&cert_pem, &key_pem) {
        if let Ok(mut cache) = state.cert_cache.write() {
            cache.insert(domain.clone(), Arc::new(ck));
        }
    }

    crate::logger::info(&state.db, &format!("Manual certificate uploaded for {}", domain)).await;
    Ok(Json(json!({ "message": "Certificate uploaded", "id": id, "domain": domain, "expires_at": expires_at })))
}

/// GET /api/certificates/:id/download — download certificate PEM
pub async fn download_certificate(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let row = sqlx::query!(
        "SELECT domain, cert_pem, key_pem FROM certificates WHERE id = ?",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let key_pem: String = ssl::decrypt_key_from_db(&state.db, &row.key_pem).await?;
    Ok(Json(json!({
        "domain": row.domain,
        "cert_pem": row.cert_pem,
        "key_pem": key_pem,
    })))
}

/// PATCH /api/certificates/:id — update auto_renew etc.
pub async fn update_certificate(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<crate::models::UpdateCertRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if let Some(auto_renew) = body.auto_renew {
        sqlx::query!(
            "UPDATE certificates SET auto_renew = ?, updated_at = datetime('now') WHERE id = ?",
            auto_renew, id
        ).execute(&state.db).await?;
    }
    Ok(Json(json!({ "message": "Updated" })))
}

// ── DDNS Test ──────────────────────────────────────────────────────────────

/// POST /api/ddns/test — comprehensive DDNS connectivity test
pub async fn ddns_test(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let provider = get_setting(&state.db, "ddns_provider").await.unwrap_or_default();
    let token = get_secret_setting(&state.db, "ddns_cf_token").await.unwrap_or_default();
    let zone_id = get_setting(&state.db, "ddns_cf_zone_id").await.unwrap_or_default();
    let ddns_enabled = get_setting(&state.db, "ddns_enabled").await.unwrap_or_default() == "true";
    let domains = get_setting(&state.db, "ddns_domains").await.unwrap_or_default();

    let mut results = json!({
        "ddns_enabled": ddns_enabled,
        "provider": provider,
        "config": {
            "token_set": !token.is_empty(),
            "zone_id_set": !zone_id.is_empty(),
            "domains_configured": !domains.is_empty(),
            "domain_count": domains.lines().filter(|l| !l.trim().is_empty()).count(),
        },
        "ip_detection": null,
        "cf_connectivity": null,
    });

    // Test IP detection
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let (ipv4, ipv4_source) = detect_ip_with_source(&client, crate::ddns::IPV4_ENDPOINTS, false).await;
    let (ipv6, ipv6_source) = detect_ip_with_source(&client, crate::ddns::IPV6_ENDPOINTS, true).await;

    results["ip_detection"] = json!({
        "ipv4": ipv4,
        "ipv4_source": ipv4_source,
        "ipv6": ipv6,
        "ipv6_source": ipv6_source,
        "success": ipv4.is_some() || ipv6.is_some(),
    });

    // Test Cloudflare connectivity
    if !token.is_empty() {
        let cf_result = test_cloudflare(&client, &token, &zone_id).await;
        results["cf_connectivity"] = cf_result;
    }

    Ok(Json(results))
}

/// GET /api/ddns/zones — list zones for the configured provider
pub async fn ddns_list_zones(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let token = get_secret_setting(&state.db, "ddns_cf_token").await.unwrap_or_default();
    if token.is_empty() {
        return Err(AppError::BadRequest("API token not configured".to_string()));
    }

    let provider_name = get_setting(&state.db, "ddns_provider").await.unwrap_or_default();
    let provider = crate::ddns::providers::get_provider(&provider_name)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Unknown DDNS provider: {provider_name}")))?;

    let zones = provider.list_zones(&token).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;

    Ok(Json(json!({ "zones": zones })))
}

/// DELETE /api/ddns/domain/:domain — remove DDNS records from the provider
pub async fn ddns_delete_domain(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(domain): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    match crate::ddns::delete_ddns_domain(&state, &domain).await {
        Ok(n) => {
            crate::logger::info(&state.db, &format!("DDNS: Deleted {} records for {}", n, domain)).await;
            Ok(Json(json!({ "message": format!("Deleted {} records for {}", n, domain), "deleted": n })))
        }
        Err(e) => {
            crate::logger::error(&state.db, &format!("DDNS: Delete {} failed: {}", domain, e)).await;
            Err(AppError::Internal(anyhow::anyhow!("{e}")))
        }
    }
}

/// GET /api/system/public-ip — fetch public IPv4/IPv6
pub async fn public_ip(
) -> AppResult<Json<serde_json::Value>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let (ipv4, _) = detect_ip_with_source(&client, crate::ddns::IPV4_ENDPOINTS, false).await;
    let (ipv6, _) = detect_ip_with_source(&client, crate::ddns::IPV6_ENDPOINTS, true).await;

    Ok(Json(json!({ "ipv4": ipv4, "ipv6": ipv6 })))
}

async fn detect_ip_with_source(client: &reqwest::Client, endpoints: &[&str], require_colon: bool) -> (Option<String>, Option<String>) {
    for url in endpoints {
        match client.get(*url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    let ip = body.trim().to_string();
                    if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') {
                        if require_colon && !ip.contains(':') { continue; }
                        return (Some(ip), Some(url.to_string()));
                    }
                }
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
    (None, None)
}

async fn test_cloudflare(client: &reqwest::Client, token: &str, zone_id: &str) -> serde_json::Value {
    let mut result = json!({
        "token_valid": false,
        "zone_accessible": false,
        "error": null,
    });

    // Verify token
    match client
        .get("https://api.cloudflare.com/client/v4/user/tokens/verify")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            result["token_valid"] = json!(body["success"].as_bool() == Some(true));
            if body["success"].as_bool() != Some(true) {
                result["error"] = body["errors"].as_array()
                    .and_then(|a| a.first())
                    .and_then(|e| e["message"].as_str())
                    .unwrap_or("Unknown error")
                    .into();
            }
        }
        Err(e) => {
            result["error"] = json!(format!("Connection failed: {e}"));
            return result;
        }
    }

    // Verify zone access
    if zone_id.is_empty() {
        return result;
    }
    match client
        .get(format!("https://api.cloudflare.com/client/v4/zones/{}", zone_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            result["zone_accessible"] = json!(body["success"].as_bool() == Some(true));
            if let Some(name) = body["result"]["name"].as_str() {
                result["zone_name"] = json!(name);
            }
        }
        Err(e) => {
            result["error"] = json!(format!("Zone lookup failed: {e}"));
        }
    }

    result
}

// ── SSL Test ───────────────────────────────────────────────────────────────

/// POST /api/certificates/test — test SSL/ACME setup
pub async fn test_certificate_setup(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let domain = body["domain"].as_str().unwrap_or("").to_string();
    let acme_email = get_setting(&state.db, "acme_email").await.unwrap_or_default();
    let http_port = state.config.proxy_http_port.to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut results = json!({
        "acme_configured": !acme_email.is_empty(),
        "acme_email": if acme_email.is_empty() { json!(null) } else {
            let at = acme_email.find('@').unwrap_or(0);
            let prefix = &acme_email[..at.min(3)];
            json!(format!("{prefix}***"))
        },
        "domain": domain,
        "letsencrypt_reachable": false,
        "dns_resolves": false,
        "domain_reachable": false,
        "warnings": [],
        "errors": [],
    });

    // Test Let's Encrypt directory
    match client.get("https://acme-v02.api.letsencrypt.org/directory").send().await {
        Ok(resp) => {
            results["letsencrypt_reachable"] = json!(resp.status().is_success());
            if !resp.status().is_success() {
                results["errors"].as_array_mut().unwrap().push(json!("Let's Encrypt directory unreachable"));
            }
        }
        Err(e) => {
            results["errors"].as_array_mut().unwrap().push(json!(format!("Let's Encrypt connection failed: {}", e)));
        }
    }

    // Test domain DNS resolution
    if !domain.is_empty() {
        match tokio::net::lookup_host(format!("{}:80", domain)).await {
            Ok(addrs) => {
                let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
                results["dns_resolves"] = json!(!ips.is_empty());
                results["resolved_ips"] = json!(ips);
            }
            Err(e) => {
                results["errors"].as_array_mut().unwrap().push(json!(format!("DNS resolution failed: {}", e)));
            }
        }

        // Test HTTP reachability on our proxy port
        if results["dns_resolves"].as_bool() == Some(true) {
            match client.get(format!("http://{}:{}/.well-known/acme-challenge/test", domain, http_port)).send().await {
                Ok(resp) => {
                    results["domain_reachable"] = json!(resp.status().is_success() || resp.status().as_u16() == 404);
                }
                Err(e) => {
                    results["warnings"].as_array_mut().unwrap().push(json!(format!("HTTP-01 may fail: {}:{} not reachable ({})", domain, http_port, e)));
                }
            }
        }
    }

    // Check overall readiness
    let ready = results["acme_configured"].as_bool() == Some(true)
        && results["letsencrypt_reachable"].as_bool() == Some(true)
        && (!domain.is_empty() || true);
    results["ready"] = json!(ready);

    Ok(Json(results))
}

// ── Categorized Logs ─────────────────────────────────────────────────────

/// GET /api/system/logs/:category
pub async fn list_logs_by_category(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<AuthUser>,
    axum::extract::Path(category): axum::extract::Path<String>,
) -> AppResult<Json<Vec<LogEntry>>> {
    let (pattern1, pattern2) = match category.as_str() {
        "ddns" => ("DDNS:%", None),
        "ssl" => ("SSL:%", Some("Certificate%")),
        "login" => ("AUTH:%", None),
        "proxy" => ("PROXY:%", None),
        _ => return Err(AppError::BadRequest(format!("Unknown log category: {category}"))),
    };

    let rows = if let Some(p2) = pattern2 {
        sqlx::query_as!(
            LogEntry,
            "SELECT id, level, message, created_at FROM logs WHERE message LIKE ? OR message LIKE ? ORDER BY id DESC LIMIT 100",
            pattern1, p2
        )
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as!(
            LogEntry,
            "SELECT id, level, message, created_at FROM logs WHERE message LIKE ? ORDER BY id DESC LIMIT 100",
            pattern1
        )
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(rows))
}
