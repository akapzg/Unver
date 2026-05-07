use std::sync::Arc;
use crate::state::{AppState, get_secret_setting, get_setting, set_setting};
use crate::errors::{AppError, AppResult};

pub mod providers;


// ── Manager Loop ──────────────────────────────────────────────────────────

pub async fn run_ddns_manager(state: Arc<AppState>) {
    tracing::info!("DDNS manager started");
    loop {
        let enabled = get_setting(&state.db, "ddns_enabled")
            .await
            .unwrap_or_default()
            == "true";
        if enabled {
            if let Err(e) = update_ddns(&state).await {
                tracing::error!("DDNS update failed: {e}");
            }
        }

        let interval = get_setting(&state.db, "ddns_interval")
            .await
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .unwrap_or(300);

        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }
}

/// Run DDNS update for all configured domains
async fn update_ddns(state: &Arc<AppState>) -> AppResult<()> {
    let provider_name = get_setting(&state.db, "ddns_provider").await?;
    let provider = providers::get_provider(&provider_name)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Unknown DDNS provider: {provider_name}")))?;

    // Check provider-specific credentials
    let has_creds = match provider_name.as_str() {
        "cloudflare" => !get_secret_setting(&state.db, "ddns_cf_token").await.unwrap_or_default().is_empty(),
        "aliyun" => !get_secret_setting(&state.db, "ddns_aliyun_access_key_id").await.unwrap_or_default().is_empty(),
        _ => false,
    };
    if !has_creds {
        return Ok(());
    }

    // Collect domains
    let legacy_domain = get_setting(&state.db, "ddns_domain")
        .await
        .unwrap_or_default();
    let multi_domains = get_setting(&state.db, "ddns_domains")
        .await
        .unwrap_or_default();

    let mut domains: Vec<String> = Vec::new();
    if !legacy_domain.is_empty() {
        domains.push(legacy_domain.clone());
    }
    if !multi_domains.is_empty() {
        for d in multi_domains.lines() {
            let d = d.trim();
            if !d.is_empty() && d != legacy_domain {
                domains.push(d.to_string());
            }
        }
    }
    if domains.is_empty() {
        return Ok(());
    }

    // Detect public IPs
    let ip_source = get_setting(&state.db, "ddns_ip_source")
        .await
        .unwrap_or_else(|_| "public".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let (ipv4, ipv6) = if ip_source.starts_with("interface:") {
        let iface = ip_source.strip_prefix("interface:").unwrap_or("eth0");
        get_interface_ips(iface).await
    } else {
        let ipv4 = fetch_ip(&client, IPV4_ENDPOINTS, false)
            .await
            .unwrap_or_default();
        let ipv6 = fetch_ip(&client, IPV6_ENDPOINTS, true).await;
        (ipv4, ipv6)
    };

    tracing::debug!("DDNS: IPv4={}, IPv6={:?}", ipv4, ipv6);

    // Update each domain via the provider
    for domain in &domains {
        if !ipv4.is_empty() {
            provider.upsert_record(state, domain, &ipv4, "A").await?;
            set_setting(&state.db, &format!("ddns_ipv4_{}", domain), &ipv4).await?;
        }
        if let Some(ref ip) = ipv6 {
            if !ip.is_empty() && ip.contains(':') {
                provider.upsert_record(state, domain, ip, "AAAA").await?;
                set_setting(&state.db, &format!("ddns_ipv6_{}", domain), ip).await?;
            }
        }
    }

    Ok(())
}

// ── Public API (used by HTTP handlers) ────────────────────────────────────

/// Delete all DDNS records for a domain from the configured provider
pub async fn delete_ddns_domain(state: &Arc<AppState>, domain: &str) -> Result<usize, String> {
    let provider_name = get_setting(&state.db, "ddns_provider")
        .await
        .unwrap_or_default();
    let provider = providers::get_provider(&provider_name)
        .ok_or_else(|| format!("Unknown DDNS provider: {provider_name}"))?;
    provider.delete_domain_records(state, domain).await
}

// ── IP Detection (provider-agnostic) ──────────────────────────────────────

pub const IPV4_ENDPOINTS: &[&str] = &[
    "https://api.ipify.org",
    "https://ipv4.icanhazip.com",
    "https://checkip.amazonaws.com",
    "https://ipinfo.io/ip",
    "https://myexternalip.com/raw",
    "https://ipecho.net/plain",
    "https://ident.me",
];

pub const IPV6_ENDPOINTS: &[&str] = &[
    "https://api6.ipify.org",
    "https://ipv6.icanhazip.com",
    "https://api6.ip.sb/ip",
];

/// Try each endpoint in order. If `require_colon`, only accept addresses containing ':'
pub async fn fetch_ip(
    client: &reqwest::Client,
    endpoints: &[&str],
    require_colon: bool,
) -> Option<String> {
    for url in endpoints {
        match client.get(*url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    let ip = body.trim().to_string();
                    if ip.is_empty() {
                        continue;
                    }
                    if !ip.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') {
                        continue;
                    }
                    if require_colon && !ip.contains(':') {
                        tracing::debug!("DDNS: {url} returned IPv4 for IPv6 endpoint, skipping");
                        continue;
                    }
                    tracing::debug!("DDNS: Got IP {ip} from {url}");
                    return Some(ip);
                }
                Err(_) => {}
            },
            Err(e) => {
                tracing::debug!("DDNS: IP endpoint {url} failed: {e}");
            }
        }
    }
    tracing::warn!(
        "DDNS: All {} endpoints exhausted",
        if require_colon { "IPv6" } else { "IPv4" }
    );
    None
}

#[allow(dead_code)]
pub async fn fetch_public_ip(
    client: &reqwest::Client,
    endpoints: &[&str],
) -> Option<String> {
    fetch_ip(client, endpoints, false).await
}

// ── Interface IP Detection ────────────────────────────────────────────────

async fn get_interface_ips(iface: &str) -> (String, Option<String>) {
    let ipv4 = get_iface_ip(iface, "-4").await.unwrap_or_default();
    let ipv6 = get_iface_ip(iface, "-6").await.ok();
    (ipv4, ipv6)
}

async fn get_iface_ip(iface: &str, family: &str) -> AppResult<String> {
    let output = tokio::process::Command::new("ip")
        .args([family, "addr", "show", iface])
        .output()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("ip command failed: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        let prefix = if family == "-4" { "inet " } else { "inet6 " };
        if let Some(rest) = line.strip_prefix(prefix) {
            let ip = rest.split('/').next().unwrap_or("").trim();
            let ip = ip.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() {
                if family == "-6" && ip.starts_with("fe80:") {
                    continue;
                }
                return Ok(ip.to_string());
            }
        }
    }
    if family == "-6" {
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("inet6 ") {
                let ip = rest.split('/').next().unwrap_or("").trim();
                let ip = ip.split_whitespace().next().unwrap_or("");
                if !ip.is_empty() && ip.starts_with("fe80:") {
                    return Ok(ip.to_string());
                }
            }
        }
    }
    Err(AppError::Internal(anyhow::anyhow!(
        "No {family} address on {iface}"
    )))
}
