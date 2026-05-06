use crate::errors::AppResult;
use std::sync::Arc;
use crate::state::AppState;

/// DNS provider trait — add a new file under providers/ and implement this
#[async_trait::async_trait]
pub trait DnsProvider: Send + Sync {
    /// Ensure a DNS record exists for the domain with the given IP.
    /// Creates if missing, updates if IP differs, skips if unchanged.
    async fn upsert_record(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        ip: &str,
        record_type: &str,
    ) -> AppResult<()>;

    /// Delete all DNS records (A + AAAA) for a domain
    async fn delete_domain_records(
        &self,
        state: &Arc<AppState>,
        domain: &str,
    ) -> Result<usize, String>;

    /// List available zones for the configured token
    async fn list_zones(&self, token: &str) -> Result<Vec<serde_json::Value>, String>;
}

/// Return the provider instance for a given provider name string
pub fn get_provider(name: &str) -> Option<Box<dyn DnsProvider>> {
    match name {
        "cloudflare" => Some(Box::new(cloudflare::CloudflareProvider)),
        _ => None,
    }
}

pub mod cloudflare;
