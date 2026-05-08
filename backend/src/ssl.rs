use std::sync::Arc;
use std::collections::HashMap;
use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine};
use chrono::{Utc, Duration};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewAccount, NewOrder, Order, OrderStatus,
};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use crate::security::{encrypt_data, decrypt_data};
use crate::state::{AppState, get_setting, get_secret_setting, set_secret_setting};
use crate::errors::{AppError, AppResult};
use crate::models::LogLine;

const LETS_ENCRYPT_URL: &str = "https://acme-v02.api.letsencrypt.org/directory";
const LETS_ENCRYPT_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

pub async fn run_ssl_manager(state: Arc<AppState>) {
    tracing::info!("SSL manager started");
    if let Err(e) = load_certs_to_cache(&state).await {
        tracing::error!("Failed to load certs into cache: {e}");
    }
    // Cleanup old ACME challenge tokens on startup
    if let Err(e) = cleanup_old_acme_tokens(&state).await {
        tracing::error!("Failed to cleanup ACME tokens: {e}");
    }
    loop {
        if let Err(e) = check_and_renew_certs(&state).await {
            tracing::error!("SSL renewal check failed: {e}");
        }
        // Cleanup old ACME challenge tokens every 12 hours
        if let Err(e) = cleanup_old_acme_tokens(&state).await {
            tracing::error!("Failed to cleanup ACME tokens: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(12 * 3600)).await;
    }
}

pub async fn load_certs_to_cache(state: &Arc<AppState>) -> AppResult<()> {
    let rows = sqlx::query!(
        "SELECT id, domain, cert_pem, key_pem FROM certificates WHERE expires_at > datetime('now')"
    ).fetch_all(&state.db).await?;

    // Decrypt all keys first (outside the lock, since decryption is async)
    let mut decrypted: Vec<(String, String, String, String)> = Vec::new();
    for row in rows {
        match decrypt_key_from_db(&state.db, &row.key_pem).await {
            Ok(key_pem) => decrypted.push((row.id.unwrap_or_default(), row.domain, row.cert_pem, key_pem)),
            Err(e) => tracing::warn!("Failed to decrypt key for {}: {e}", row.domain),
        }
    }

    // Sort: wildcards first, exact certs last → exact overwrites wildcard on conflict
    decrypted.sort_by_key(|(_, domain, _, _)| !domain.starts_with("*."));

    // Only cache certs for rules with ssl_enabled=true (NPM-style: per-domain control)
    let ssl_rules = sqlx::query!(
        "SELECT domain, cert_id FROM proxy_rules WHERE ssl_enabled = 1 AND enabled = 1"
    ).fetch_all(&state.db).await?;

    let mut cache = state.cert_cache.write().map_err(|_| AppError::Internal(anyhow::anyhow!("poison")))?;
    for (id, cert_domain, cert_pem, key_pem) in &decrypted {
        if let Ok(ck) = load_cert_into_cache(cert_pem, key_pem) {
            let ck = Arc::new(ck);
            for rule in &ssl_rules {
                let covers = if rule.cert_id.as_deref() == Some(id.as_str()) {
                    // Explicit cert_id binding — always match
                    true
                } else if rule.cert_id.as_deref().map_or(true, |s| s.is_empty()) {
                    // Auto-match: no cert_id set (None or empty string)
                    cert_covers_domain(cert_domain, &rule.domain)
                } else {
                    false // rule binds to a different cert, skip
                };
                if covers {
                    cache.insert(rule.domain.to_lowercase(), ck.clone());
                }
            }
        }
    }
    Ok(())
}

/// Check if a certificate's domain covers a rule's domain.
/// Exact match or single-level wildcard: *.example.com covers sub.example.com
fn cert_covers_domain(cert_domain: &str, rule_domain: &str) -> bool {
    if cert_domain.eq_ignore_ascii_case(rule_domain) {
        return true;
    }
    if let Some(rest) = cert_domain.strip_prefix("*.") {
        if let Some(dot) = rule_domain.find('.') {
            return rest.eq_ignore_ascii_case(&rule_domain[dot + 1..]);
        }
    }
    false
}

pub fn load_cert_into_cache(cert_pem: &str, key_pem: &str) -> AppResult<rustls::sign::CertifiedKey> {
    use rustls::pki_types::CertificateDer;
    let mut cr = std::io::BufReader::new(cert_pem.as_bytes());
    let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cr)
        .collect::<Result<Vec<_>, _>>().map_err(|e| AppError::Internal(anyhow::anyhow!("cert: {e}")))?;
    let mut kr = std::io::BufReader::new(key_pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut kr)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("key: {e}")))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no key")))?;
    let sk = rustls::crypto::ring::sign::any_supported_type(&key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("key type: {e}")))?;
    Ok(rustls::sign::CertifiedKey::new(certs, sk))
}

/// Parse the NotAfter date from a PEM certificate. Returns RFC3339 string or None on failure.
pub fn parse_cert_expiry(cert_pem: &str) -> Option<String> {
    let cert = openssl::x509::X509::from_pem(cert_pem.as_bytes()).ok()?;
    let not_after = cert.not_after();
    // Parse ASN1_TIME string "MMM DD HH:MM:SS YYYY GMT" directly
    // (avoids Asn1Time::diff platform quirks on musl/aarch64)
    let s = not_after.to_string();
    let naive = chrono::NaiveDateTime::parse_from_str(
        s.trim_end_matches(" GMT"),
        "%b %e %H:%M:%S %Y"
    ).ok()?;
    let expiry = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
    Some(expiry.to_rfc3339())
}

pub async fn check_and_renew_certs(state: &Arc<AppState>) -> AppResult<()> {
    let provider_name = get_setting(&state.db, "ddns_provider").await.unwrap_or_default();
   if provider_name.is_empty() { return Ok(()); }
    let email = get_setting(&state.db, "acme_email").await?;
    if email.is_empty() { return Ok(()); }
    let proxies = sqlx::query!("SELECT domain FROM proxy_rules WHERE ssl_enabled = 1 AND enabled = 1")
        .fetch_all(&state.db).await?;
    for proxy in proxies {
        let cert = sqlx::query!("SELECT expires_at FROM certificates WHERE domain = ? AND source = 'acme'", proxy.domain)
            .fetch_optional(&state.db).await?;
        let should = match cert {
            Some(c) => chrono::DateTime::parse_from_rfc3339(&c.expires_at)
                .map(|e| e.with_timezone(&Utc) < Utc::now() + Duration::days(10)).unwrap_or(true),
            None => false,  // no cert exists — don't auto-issue, user must trigger manually
        };
        if should {
            match issue_certificate_multi(state, &email, &[proxy.domain.clone()], &provider_name, None).await {
                Ok(_) => crate::logger::info(&state.db, &format!("SSL issued for {}", proxy.domain)).await,
                Err(e) => crate::logger::error(&state.db, &format!("SSL failed for {}: {e}", proxy.domain)).await,
            }
        }
    }
    Ok(())
}

/// Finalize CSR + poll certificate with 5-min timeout.
async fn finalize_and_poll(mut order: Order, csr_der: &[u8]) -> AppResult<String> {
    order.finalize_csr(csr_der).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Finalize: {e}")))?;
    let cert = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        order.poll_certificate(&instant_acme::RetryPolicy::default()),
    ).await.map_err(|_| AppError::Internal(anyhow::anyhow!("Poll timed out")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Poll: {e}")))?;
    Ok(cert)
}

fn push_log(log: &Option<Arc<tokio::sync::Mutex<Vec<LogLine>>>>, level: &str, msg: &str) {
    if let Some(l) = log {
        if let Ok(mut lines) = l.try_lock() {
            lines.push(LogLine { timestamp: Utc::now().format("%H:%M:%S").to_string(), level: level.into(), message: msg.into() });
        }
    }
    match level { "success" => tracing::info!("✅ {msg}"), "error" => tracing::error!("❌ {msg}"), _ => tracing::info!("{msg}") }
}

pub async fn issue_certificate_multi(
   state: &Arc<AppState>, email: &str, domains: &[String],
   provider_name: &str,
   log: Option<Arc<tokio::sync::Mutex<Vec<LogLine>>>>,
) -> AppResult<()> {
    let use_staging = get_setting(&state.db, "acme_staging").await.unwrap_or_default() == "true";
    let acme_url = if use_staging { LETS_ENCRYPT_STAGING } else { LETS_ENCRYPT_URL };
    push_log(&log, "info", &format!("🔐 ACME (staging={use_staging})"));

    let account = get_or_create_account(state, email, acme_url).await?;
   push_log(&log, "success", "ACME 账户就绪");

   let ids: Vec<Identifier> = domains.iter().map(|d| Identifier::Dns(d.clone())).collect();
   let mut order = account.new_order(&NewOrder::new(&ids)).await
       .map_err(|e| AppError::Internal(anyhow::anyhow!("Order: {e}")))?;
   push_log(&log, "info", &format!("📋 订单: {}", domains.join(", ")));

   let provider = crate::ddns::providers::get_provider(provider_name)
       .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Unknown DDNS provider: {provider_name}")))?;
   push_log(&log, "info", "🔑 验证 (DNS-01)");

    let mut auths = order.authorizations();
    while let Some(a) = auths.next().await {
        let mut auth = a.map_err(|e| AppError::Internal(anyhow::anyhow!("Auth: {e}")))?;

        let auth_state = auth.refresh().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Refresh: {e}")))?;
        let challenge = auth_state.challenges.iter()
            .find(|c| c.r#type == ChallengeType::Dns01)
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("No Dns01 challenge")))?;
        let challenge_token = challenge.token.clone();
        let identifier = auth_state.identifier().to_string();
        let key_thumbprint = account.key_thumbprint();
        let key_auth = format!("{}.{}", challenge_token, key_thumbprint);

        // Compute DNS-01 TXT value: base64url(sha256(key_authorization))
        let mut hasher = Sha256::new();
        hasher.update(key_auth.as_bytes());
        let dns_value = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());

       push_log(&log, "info", &format!("  🌐 DNS TXT: _acme-challenge.{identifier}"));
       let rid = provider.create_acme_txt(state, &identifier, &dns_value).await
           .map_err(|e| AppError::Internal(anyhow::anyhow!("Create TXT: {e}")))?;
       push_log(&log, "success", "  ✅ TXT 已创建");

       // Poll for validation (LE auto-polls DNS, set_ready is optional)
       push_log(&log, "info", "  ⏳ 等待验证...");
       let mut ok = false;
       for i in 1..=40 {
           tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
           let st = auth.refresh().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Refresh: {e}")))?.status;
           match st {
               AuthorizationStatus::Valid => { ok = true; break; }
               AuthorizationStatus::Invalid => { let _ = provider.delete_acme_txt(state, &identifier, &rid).await; return Err(AppError::Internal(anyhow::anyhow!("Invalid"))); }
               _ if i == 30 => { let _ = provider.delete_acme_txt(state, &identifier, &rid).await; return Err(AppError::Internal(anyhow::anyhow!("Timeout"))); }
               _ if i % 3 == 0 => push_log(&log, "info", &format!("    验证中 ({i}/30)")),
               _ => {}
           }
       }
       if ok { push_log(&log, "success", "  ✅ DNS-01 通过"); }
       let _ = provider.delete_acme_txt(state, &identifier, &rid).await;
    }

    push_log(&log, "info", "⏳ 等待订单就绪...");
    loop {
        let s = order.refresh().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Refresh: {e}")))?.status;
        match s { OrderStatus::Ready => break, OrderStatus::Invalid => return Err(AppError::Internal(anyhow::anyhow!("Invalid order"))), _ => tokio::time::sleep(tokio::time::Duration::from_secs(3)).await }
    }

    push_log(&log, "info", "📝 生成密钥/CSR...");
    let key_pair = KeyPair::generate().map_err(|e| AppError::Internal(anyhow::anyhow!("Key: {e}")))?;
    let csr_der = CertificateParams::new(domains.to_vec())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Params: {e}")))?
        .serialize_request(&key_pair)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("CSR: {e}")))?;

    push_log(&log, "info", "📤 提交 CSR...");
    let cert_pem = finalize_and_poll(order, csr_der.der()).await?;
    let key_pem = key_pair.serialize_pem();

    let primary = &domains[0];
    let expires_at = (Utc::now() + Duration::days(90)).to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let encrypted_key = encrypt_key_for_db(&state.db, &key_pem).await?;
    sqlx::query!(
        "INSERT INTO certificates (id, domain, cert_pem, key_pem, expires_at, source) VALUES (?, ?, ?, ?, ?, 'acme') ON CONFLICT(domain) DO UPDATE SET cert_pem = excluded.cert_pem, key_pem = excluded.key_pem, expires_at = excluded.expires_at, source = 'acme', updated_at = datetime('now')",
        id, primary, cert_pem, encrypted_key, expires_at
    ).execute(&state.db).await?;

    // Refresh cert cache (only inserts for rules with ssl_enabled=true)
    let _ = load_certs_to_cache(state).await;
    push_log(&log, "success", &format!("🎉 签发完成: {} (90天)", domains.join(", ")));
    Ok(())
}

async fn get_or_create_account(state: &Arc<AppState>, email: &str, acme_url: &str) -> AppResult<Account> {
    let creds_json = get_secret_setting(&state.db, "acme_credentials").await?;
    if !creds_json.is_empty() {
        let creds: instant_acme::AccountCredentials = serde_json::from_str(&creds_json)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Parse: {e}")))?;
        Account::builder().map_err(|e| AppError::Internal(anyhow::anyhow!("Builder: {e}")))?
            .from_credentials(creds).await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Load: {e}")))
    } else {
        let (account, new_creds) = Account::builder().map_err(|e| AppError::Internal(anyhow::anyhow!("Builder: {e}")))?
            .create(&NewAccount {
                contact: &[&format!("mailto:{email}")],
                terms_of_service_agreed: true,
                only_return_existing: false,
            }, acme_url.to_string(), None).await.map_err(|e| AppError::Internal(anyhow::anyhow!("Create: {e}")))?;
        set_secret_setting(&state.db, "acme_credentials", &serde_json::to_string(&new_creds)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Serialize: {e}")))?).await?;
        Ok(account)
    }
}

// ── ACME account management
pub async fn cleanup_acme_txt(state: &Arc<AppState>, domain: &str) -> Result<usize, String> {
   let provider_name = get_setting(&state.db, "ddns_provider").await.unwrap_or_default();
   if provider_name.is_empty() { return Ok(0); }
   let provider = crate::ddns::providers::get_provider(&provider_name)
       .ok_or_else(|| format!("Unknown DDNS provider: {provider_name}"))?;
   provider.cleanup_acme_txts(state, domain).await
}
// === Worker-thread version (no Send required, can use set_ready) ===

/// Issue certificate from the SSL worker thread. Uses set_ready for fast validation.
/// Returns cert_pem, key_pem, expires_at on success.
pub async fn issue_certificate_sync(
   email: &str, domains: &[String], provider_name: &str,
   state: &Arc<AppState>,
   use_staging: bool,
    db: &SqlitePool,
    _cert_cache: &Arc<std::sync::RwLock<HashMap<String, Arc<rustls::sign::CertifiedKey>>>>,
    log_buf: Option<Arc<tokio::sync::Mutex<Vec<LogLine>>>>,
) -> Result<crate::ssl_worker::SslResult, String> {
    let acme_url = if use_staging { LETS_ENCRYPT_STAGING } else { LETS_ENCRYPT_URL };

    let push = |level: &str, msg: &str| {
        if let Some(ref buf) = log_buf {
            let line = LogLine {
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                level: level.to_string(),
                message: msg.to_string(),
            };
            // Spawn on current_thread runtime — avoids block_on panic
            let buf = Arc::clone(buf);
            let _ = tokio::runtime::Handle::try_current().map(|h| {
                h.spawn(async move {
                    let mut lines = buf.lock().await;
                    lines.push(line);
                })
            });
        }
        match level {
            "success" => tracing::info!("✅ {msg}"),
            "error" => tracing::error!("❌ {msg}"),
            _ => tracing::info!("{msg}"),
        }
    };

    push("info", &format!("🔐 ACME (staging={use_staging})"));

    let account = get_or_create_account_sync(db, email, acme_url).await
        .map_err(|e| format!("Account: {e}"))?;
    push("success", "ACME 账户就绪");

    let ids: Vec<Identifier> = domains.iter().map(|d| Identifier::Dns(d.clone())).collect();
    let mut order = account.new_order(&NewOrder::new(&ids)).await
        .map_err(|e| format!("Order: {e}"))?;
   push("info", &format!("📋 订单: {}", domains.join(", ")));

   let provider = crate::ddns::providers::get_provider(provider_name)
       .ok_or_else(|| format!("Unknown DDNS provider: {provider_name}"))?;
   push("info", "🔑 验证 (DNS-01)");

   let mut auths = order.authorizations();
   while let Some(a) = auths.next().await {
       let mut auth = a.map_err(|e| format!("Auth: {e}"))?;

       let auth_state = auth.refresh().await.map_err(|e| format!("Refresh: {e}"))?;
       let challenge = auth_state.challenges.iter()
           .find(|c| c.r#type == ChallengeType::Dns01)
           .ok_or_else(|| "No Dns01 challenge".to_string())?;
       let challenge_token = challenge.token.clone();
       let challenge_url = challenge.url.clone();
       let identifier = auth_state.identifier().to_string();
       let key_thumbprint = account.key_thumbprint();
       let key_auth = format!("{}.{}", challenge_token, key_thumbprint);

       let mut hasher = Sha256::new();
       hasher.update(key_auth.as_bytes());
       let dns_value = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());

       push("info", &format!("  🌐 DNS TXT: _acme-challenge.{identifier}"));
       let rid = provider.create_acme_txt(state, &identifier, &dns_value).await
           .map_err(|e| format!("Create TXT: {e}"))?;
       push("success", "  ✅ TXT 已创建");

       // Notify LE we're ready — sends JWS-signed POST with {} body
       tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
       account.set_challenge_ready(&challenge_url).await
           .map_err(|e| format!("set_challenge_ready: {e}"))?;

       // ── Poll (wait for LE to validate on its own schedule) ─────
       let mut ok = false;
       for i in 1..=120 {
           tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
           let st = auth.refresh().await.map_err(|e| format!("Refresh: {e}"))?.status;
           match st {
               AuthorizationStatus::Valid => { ok = true; break; }
               AuthorizationStatus::Invalid => {
                   let _ = provider.delete_acme_txt(state, &identifier, &rid).await;
                   return Err("Challenge invalid".to_string());
               }
               _ if i == 120 => {
                   let _ = provider.delete_acme_txt(state, &identifier, &rid).await;
                   return Err("Validation timeout (240s)".to_string());
               }
               _ => {}
           }
       }
       if !ok {
           let _ = provider.delete_acme_txt(state, &identifier, &rid).await;
           return Err("Validation failed".to_string());
       }
       push("success", "  ✅ DNS-01 通过");
       let _ = provider.delete_acme_txt(state, &identifier, &rid).await;
    }

    // Wait for order to be ready
    push("info", "⏳ 等待订单就绪...");
    loop {
        let s = order.refresh().await.map_err(|e| format!("Order refresh: {e}"))?.status;
        match s {
            OrderStatus::Ready => break,
            OrderStatus::Invalid => return Err("Order invalid".to_string()),
            _ => tokio::time::sleep(tokio::time::Duration::from_secs(2)).await,
        }
    }

    if domains.is_empty() {
        return Err("domains list is empty".to_string());
    }

    push("info", "📝 生成密钥/CSR...");
    let key_pair = KeyPair::generate().map_err(|e| format!("Key gen: {e}"))?;

    let mut params = CertificateParams::new(domains.to_vec())
        .map_err(|e| format!("Params: {e}"))?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, &domains[0]);
    params.distinguished_name = dn;

    let csr_der = params
        .serialize_request(&key_pair)
        .map_err(|e| format!("CSR: {e}"))?;

    push("info", "📤 提交 CSR...");
    order.finalize_csr(csr_der.der()).await.map_err(|e| format!("Finalize: {e}"))?;

    let cert_pem = order.poll_certificate(&instant_acme::RetryPolicy::default()).await
        .map_err(|e| format!("Poll cert: {e}"))?;
    let key_pem = key_pair.serialize_pem();

    let primary = domains[0].clone();
    let expires_at = (Utc::now() + Duration::days(90)).to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();

    let encrypted_key = encrypt_key_for_db(db, &key_pem).await
        .map_err(|e| format!("Encrypt key: {e}"))?;
    sqlx::query!(
        "INSERT INTO certificates (id, domain, cert_pem, key_pem, expires_at, source) VALUES (?, ?, ?, ?, ?, 'acme') ON CONFLICT(domain) DO UPDATE SET cert_pem = excluded.cert_pem, key_pem = excluded.key_pem, expires_at = excluded.expires_at, source = 'acme', updated_at = datetime('now')",
        id, primary, cert_pem, encrypted_key, expires_at
    ).execute(db).await.map_err(|e| format!("DB: {e}"))?;

    // Refresh cert cache (only inserts for rules with ssl_enabled=true)
    let _ = load_certs_to_cache(state).await;

    push("success", &format!("🎉 签发完成: {} (90天)", primary));
    Ok(crate::ssl_worker::SslResult {
        primary_domain: primary,
        cert_pem,
        key_pem,
        expires_at,
    })
}

async fn get_or_create_account_sync(db: &SqlitePool, email: &str, acme_url: &str) -> Result<Account, String> {
    let creds_json = get_secret_setting(db, "acme_credentials").await
        .map_err(|e| format!("Settings: {e}"))?;
    if !creds_json.is_empty() {
        let creds: instant_acme::AccountCredentials = serde_json::from_str(&creds_json)
            .map_err(|e| format!("Parse creds: {e}"))?;
        Account::builder().map_err(|e| format!("Builder: {e}"))?
            .from_credentials(creds).await
            .map_err(|e| format!("Load account: {e}"))
    } else {
        let (account, new_creds) = Account::builder().map_err(|e| format!("Builder: {e}"))?
            .create(&NewAccount {
                contact: &[&format!("mailto:{email}")],
                terms_of_service_agreed: true,
                only_return_existing: false,
            }, acme_url.to_string(), None).await.map_err(|e| format!("Create account: {e}"))?;
        set_secret_setting(db, "acme_credentials", &serde_json::to_string(&new_creds)
            .map_err(|e| format!("Serialize: {e}"))?).await
            .map_err(|e| format!("Save creds: {e}"))?;
        Ok(account)
    }
}

/// Clean up old ACME challenge tokens (older than 1 hour) from settings table.
/// Prevents database bloat from accumulated challenge tokens.
async fn cleanup_old_acme_tokens(state: &Arc<AppState>) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM settings WHERE key LIKE 'acme_challenge_%' AND updated_at < datetime('now', '-1 hour')"
    )
    .execute(&state.db)
    .await?;
    Ok(())
}

// ── Key PEM Encryption Helpers ─────────────────────────────────────────────

/// Encrypt a private key PEM string for storage in the database.
/// The JWT secret is used as the encryption master key.
pub async fn encrypt_key_for_db(db: &SqlitePool, key_pem: &str) -> AppResult<String> {
    let jwt_secret = get_setting(db, "jwt_secret").await?;
    if jwt_secret.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("JWT secret not initialized")));
    }
    encrypt_data(key_pem, &jwt_secret)
}

/// Decrypt a private key PEM string from the database.
/// Supports backward compatibility: if decryption fails, returns the raw value
/// (assumes it was stored as plaintext before encryption was introduced).
pub async fn decrypt_key_from_db(db: &SqlitePool, stored: &str) -> AppResult<String> {
    let jwt_secret = get_setting(db, "jwt_secret").await?;
    if jwt_secret.is_empty() {
        // No secret yet — return as-is (shouldn't happen in normal flow)
        return Ok(stored.to_string());
    }
    match decrypt_data(stored, &jwt_secret) {
        Ok(plain) => Ok(plain),
        Err(_) => {
            // Backward compat: old data stored as plaintext
            tracing::info!("Key PEM appears to be plaintext (pre-encryption), treating as-is");
            Ok(stored.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, KeyPair, DnType, DistinguishedName};

    fn make_test_cert(domain: &str) -> (String, String) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(vec![domain.to_string()]).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, domain);
        params.distinguished_name = dn;
        let cert = params.self_signed(&key).unwrap();
        (cert.pem(), key.serialize_pem())
    }

    #[test]
    fn test_load_valid_cert_into_cache() {
        let (cert_pem, key_pem) = make_test_cert("example.com");
        let result = load_cert_into_cache(&cert_pem, &key_pem);
        assert!(result.is_ok(), "valid PEM should parse: {:?}", result.err());
    }

    #[test]
    fn test_load_invalid_cert_rejected() {
        assert!(load_cert_into_cache("not a cert", "not a key").is_err());
        assert!(load_cert_into_cache(
            "-----BEGIN CERTIFICATE-----\nbad\n-----END CERTIFICATE-----",
            "-----BEGIN PRIVATE KEY-----\nbad\n-----END PRIVATE KEY-----"
        ).is_err());
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_key_pem_roundtrip() {
        // Set up an in-memory database with a JWT secret
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Create minimal schema
        sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO settings (key, value) VALUES ('jwt_secret', 'test-secret-32-bytes-xxxxxxx')")
            .execute(&db).await.unwrap();

        let original = "-----BEGIN PRIVATE KEY-----\ntest-key\n-----END PRIVATE KEY-----";
        let encrypted = encrypt_key_for_db(&db, original).await.unwrap();
        assert_ne!(encrypted, original);

        // Roundtrip
        let decrypted = decrypt_key_from_db(&db, &encrypted).await.unwrap();
        assert_eq!(decrypted, original);
    }

    #[tokio::test]
    async fn test_decrypt_plaintext_backward_compat() {
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO settings (key, value) VALUES ('jwt_secret', 'test-secret')")
            .execute(&db).await.unwrap();

        // Plaintext PEM (pre-encryption data)
        let plaintext = "-----BEGIN PRIVATE KEY-----\nold-key\n-----END PRIVATE KEY-----";
        let result = decrypt_key_from_db(&db, plaintext).await.unwrap();
        assert_eq!(result, plaintext, "should return plaintext as-is for backward compat");
    }

    #[tokio::test]
    async fn test_encrypt_key_no_jwt_secret() {
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&db).await.unwrap();

        let result = encrypt_key_for_db(&db, "key-data").await;
        assert!(result.is_err(), "should fail without JWT secret");
    }
}
