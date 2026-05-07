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

    // ── ACME DNS-01 challenge (certificate issuance) ─────────────────────

    /// Create a TXT record for ACME DNS-01 challenge verification.
    /// Returns a provider-specific record identifier for later deletion.
    async fn create_acme_txt(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        value: &str,
    ) -> Result<String, String>;

    /// Delete a TXT record created by create_acme_txt.
    async fn delete_acme_txt(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        record_id: &str,
    ) -> Result<(), String>;

    /// Clean up all leftover _acme-challenge TXT records for a domain.
    /// Returns the number of records deleted.
    async fn cleanup_acme_txts(
        &self,
        state: &Arc<AppState>,
        domain: &str,
    ) -> Result<usize, String>;
}

/// Return the provider instance for a given provider name string
pub fn get_provider(name: &str) -> Option<Box<dyn DnsProvider>> {
    match name {
        "cloudflare" => Some(Box::new(cloudflare::CloudflareProvider)),
        _ => None,
    }
}

pub mod cloudflare;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Mock DNS provider for unit-testing DNS-01 protocol without real API calls.
    /// Stores records in memory; all methods are synchronous-safe via Mutex.
    pub struct MockProvider {
        /// domain → Vec<(record_id, value)>
        records: Mutex<HashMap<String, Vec<(String, String)>>>,
    }

    impl MockProvider {
        pub fn new() -> Self {
            Self { records: Mutex::new(HashMap::new()) }
        }

        /// Expose stored records for assertion (useful for tests)
        pub fn records(&self) -> HashMap<String, Vec<(String, String)>> {
            self.records.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl DnsProvider for MockProvider {
        async fn upsert_record(
            &self,
            _state: &Arc<AppState>,
            domain: &str,
            ip: &str,
            record_type: &str,
        ) -> AppResult<()> {
            // Store as record for test visibility
            self.records.lock().unwrap()
                .entry(domain.to_string())
                .or_default()
                .push((record_type.to_string(), ip.to_string()));
            Ok(())
        }

        async fn delete_domain_records(
            &self,
            _state: &Arc<AppState>,
            domain: &str,
        ) -> Result<usize, String> {
            let count = self.records.lock().unwrap().remove(domain).map(|v| v.len()).unwrap_or(0);
            Ok(count)
        }

        async fn list_zones(&self, _token: &str) -> Result<Vec<serde_json::Value>, String> {
            Ok(vec![])
        }

        async fn create_acme_txt(
            &self,
            _state: &Arc<AppState>,
            domain: &str,
            value: &str,
        ) -> Result<String, String> {
            let record_id = format!("mock-txt-{}", uuid::Uuid::new_v4());
            let name = format!("_acme-challenge.{domain}");
            self.records.lock().unwrap()
                .entry(name)
                .or_default()
                .push((record_id.clone(), value.to_string()));
            Ok(record_id)
        }

        async fn delete_acme_txt(
            &self,
            _state: &Arc<AppState>,
            domain: &str,
            record_id: &str,
        ) -> Result<(), String> {
            let name = format!("_acme-challenge.{domain}");
            let mut records = self.records.lock().unwrap();
            if let Some(entries) = records.get_mut(&name) {
                let len_before = entries.len();
                entries.retain(|(id, _)| id != record_id);
                if entries.len() < len_before {
                    return Ok(());
                }
            }
            Err(format!("Record {record_id} not found for {name}"))
        }

        async fn cleanup_acme_txts(
            &self,
            _state: &Arc<AppState>,
            domain: &str,
        ) -> Result<usize, String> {
            let name = format!("_acme-challenge.{domain}");
            let count = self.records.lock().unwrap().remove(&name).map(|v| v.len()).unwrap_or(0);
            Ok(count)
        }
    }

    // ── get_provider 解析测试 ─────────────────────────────────────────────

    #[test]
    fn test_get_provider_cloudflare() {
        assert!(get_provider("cloudflare").is_some());
    }

    #[test]
    fn test_get_provider_unknown_returns_none() {
        assert!(get_provider("aliyun").is_none());
        assert!(get_provider("").is_none());
        assert!(get_provider("nonexistent").is_none());
    }

    // ── MockProvider DNS-01 完整流程测试 ──────────────────────────────────

    #[tokio::test]
    async fn test_mock_create_and_delete_acme_txt() {
        let p = MockProvider::new();
        let state = Arc::new(AppState::dummy().await);

        // 创建 TXT 记录
        let id = p.create_acme_txt(&state, "example.com", "challenge-token-123")
            .await
            .expect("create_acme_txt should succeed");
        assert!(id.starts_with("mock-txt-"), "record_id should have mock prefix, got: {id}");

        // 验证存储
        let records = p.records();
        let key = "_acme-challenge.example.com";
        assert!(records.contains_key(key), "expected key {key} in MockProvider");
        let entries = &records[key];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, id);
        assert_eq!(entries[0].1, "challenge-token-123");

        // 删除 TXT 记录
        p.delete_acme_txt(&state, "example.com", &id)
            .await
            .expect("delete_acme_txt should succeed");

        // 验证已删除
        assert!(p.records().get(key).map_or(true, |v| v.is_empty()));
    }

    #[tokio::test]
    async fn test_mock_delete_nonexistent_record() {
        let p = MockProvider::new();
        let state = Arc::new(AppState::dummy().await);

        let result = p.delete_acme_txt(&state, "example.com", "nonexistent-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_cleanup_acme_txts() {
        let p = MockProvider::new();
        let state = Arc::new(AppState::dummy().await);

        // 创建 3 条 TXT 记录
        p.create_acme_txt(&state, "example.com", "token-1").await.unwrap();
        p.create_acme_txt(&state, "example.com", "token-2").await.unwrap();
        p.create_acme_txt(&state, "example.com", "token-3").await.unwrap();

        // 清理
        let count = p.cleanup_acme_txts(&state, "example.com")
            .await
            .expect("cleanup should succeed");
        assert_eq!(count, 3);

        // 验证已全部删除
        assert!(!p.records().contains_key("_acme-challenge.example.com"));

        // 再次清理同一域名（应该返回 0）
        let count2 = p.cleanup_acme_txts(&state, "example.com")
            .await
            .expect("cleanup should succeed");
        assert_eq!(count2, 0);
    }

    #[tokio::test]
    async fn test_mock_cleanup_isolated_by_domain() {
        let p = MockProvider::new();
        let state = Arc::new(AppState::dummy().await);

        p.create_acme_txt(&state, "a.com", "token-a").await.unwrap();
        p.create_acme_txt(&state, "b.com", "token-b").await.unwrap();

        // 清理 a.com 不影响 b.com
        p.cleanup_acme_txts(&state, "a.com").await.unwrap();
        let records = p.records();
        assert!(!records.contains_key("_acme-challenge.a.com"));
        assert!(records.contains_key("_acme-challenge.b.com"));
    }

    #[tokio::test]
    async fn test_mock_multiple_providers_independent() {
        // 两个独立 MockProvider 互不干扰
        let p1 = MockProvider::new();
        let p2 = MockProvider::new();
        let state = Arc::new(AppState::dummy().await);

        p1.create_acme_txt(&state, "example.com", "from-p1").await.unwrap();
        p2.create_acme_txt(&state, "example.com", "from-p2").await.unwrap();

        let r1 = p1.records();
        let r2 = p2.records();
        assert_eq!(r1["_acme-challenge.example.com"][0].1, "from-p1");
        assert_eq!(r2["_acme-challenge.example.com"][0].1, "from-p2");
    }
}
