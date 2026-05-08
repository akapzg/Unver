use serde::{Deserialize, Serialize};

// ── Proxy Rule ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ProxyRule {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub target_url: String,
    pub rule_type: String,
    pub redirect_code: Option<i64>,
    pub ssl_enabled: bool,
    pub cert_id: Option<String>,
    pub force_https: bool,
    pub enabled: bool,
    pub port_group_id: Option<String>,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Active connections (from in-memory counter, not DB)
    #[serde(default)]
    pub active_connections: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateProxyRule {
    pub name: String,
    pub domain: String,
    pub target_url: String,
    pub port_group_id: String,
    #[serde(default = "default_rule_type")]
    pub rule_type: String,
    #[serde(default)]
    pub redirect_code: Option<i64>,
    #[serde(default)]
    pub ssl_enabled: bool,
    #[serde(default)]
    pub cert_id: Option<String>,
    #[serde(default)]
    pub force_https: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProxyRule {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub target_url: Option<String>,
    pub port_group_id: Option<String>,
    pub rule_type: Option<String>,
    pub redirect_code: Option<i64>,
    pub ssl_enabled: Option<bool>,
    pub force_https: Option<bool>,
    pub enabled: Option<bool>,
    pub cert_id: Option<String>,
}

// ── Port Group ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct PortGroup {
    pub id: String,
    pub name: String,
    pub listen_port: i64,
    pub enabled: bool,
    pub skip_tls_verify: bool,
    pub force_https: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePortGroup {
    pub name: String,
    pub listen_port: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub skip_tls_verify: bool,
    #[serde(default)]
    pub force_https: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePortGroup {
    pub name: Option<String>,
    pub listen_port: Option<i64>,
    pub enabled: Option<bool>,
    pub skip_tls_verify: Option<bool>,
    pub force_https: Option<bool>,
}

// ── User ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

// ── Auth ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

// ── Certificate ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Certificate {
    pub id: String,
    pub domain: String,
    pub expires_at: String,
    pub auto_renew: bool,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

// ── API Key ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// Returned only once at creation time
#[derive(Debug, Serialize)]
pub struct NewApiKey {
    pub id: String,
    pub name: String,
    pub key: String, // full plaintext key, shown once only
    pub key_prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKey {
    pub name: String,
}

// ── Settings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_auth_enabled: bool,
    pub setup_complete: bool,
    pub acme_email: String,
    pub ddns_enabled: bool,
    pub ddns_provider: String,
    pub ddns_cf_token: String,
    pub ddns_cf_zone_id: String,
    pub ddns_aliyun_access_key_id: String,
    pub ddns_aliyun_access_key_secret: String,
    pub ddns_domain: String,
    pub ddns_domains: String,
    pub ddns_interval: u64,
    pub ddns_ip_source: String,
    pub username: String,
    pub web_port: u16,
    pub web_interface: String,
    pub panel_lan_only: Option<bool>,
    pub trusted_proxy: Option<String>,
    pub acme_provider: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettings {
    pub api_auth_enabled: Option<bool>,
    pub acme_email: Option<String>,
    pub ddns_enabled: Option<bool>,
    pub ddns_provider: Option<String>,
    pub ddns_cf_token: Option<String>,
    pub ddns_cf_zone_id: Option<String>,
    pub ddns_aliyun_access_key_id: Option<String>,
    pub ddns_aliyun_access_key_secret: Option<String>,
    pub ddns_domain: Option<String>,
    pub ddns_domains: Option<String>,
    pub ddns_interval: Option<u64>,
    pub ddns_ip_source: Option<String>,
    pub web_port: Option<u16>,
    pub web_interface: Option<String>,
    #[serde(default)]
    pub panel_lan_only: Option<bool>,
    #[serde(default)]
    pub trusted_proxy: Option<String>,
    #[serde(default)]
    pub acme_provider: Option<String>,
}

// ── DDNS Status ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DdnsDomainStatus {
    pub domain: String,
    pub ipv4: String,
    pub ipv6: Option<String>,
    pub enabled: bool,
}

// ── SSL Certificate Operations ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssueCertRequest {
    pub domain: String,              // primary domain (CN)
    pub sans: Option<String>,        // comma/semicolon separated extra domains (SAN)
    #[allow(dead_code)]
    pub method: String,              // "http01" | "dns01"
    #[allow(dead_code)]
    pub cf_token: Option<String>,
    pub email: Option<String>,       // ACME email (saves to settings if provided)
}

#[derive(Debug, Deserialize)]
pub struct UpdateCertRequest {
    pub auto_renew: Option<bool>,
}

// ── Background Job (for long-running async tasks like SSL issuance) ───────

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub timestamp: String,
    pub level: String,   // "info" | "success" | "error"
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundJob {
    pub job_id: String,
    pub status: String,   // "running" | "completed" | "failed"
    pub domain: String,
    pub logs: Vec<LogLine>,
    pub result: Option<String>,
    pub error: Option<String>,
}

// ── Password Change ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn default_true() -> bool { true }
fn default_rule_type() -> String { "proxy".to_string() }
