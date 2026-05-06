use sqlx::SqlitePool;
use std::sync::Arc;
use crate::ddns::providers::DnsProvider;
use crate::errors::{AppError, AppResult};
use crate::state::{AppState, get_secret_setting, get_setting};
use serde_json::json;

pub struct CloudflareProvider;

impl CloudflareProvider {
    async fn get_credentials(state: &Arc<AppState>) -> Result<(String, String), String> {
        let token = get_secret_setting(&state.db, "ddns_cf_token")
            .await
            .map_err(|e| format!("Token: {e}"))?;
        if token.is_empty() {
            return Err("Cloudflare token not configured".into());
        }
        let zone_id = get_setting(&state.db, "ddns_cf_zone_id")
            .await
            .unwrap_or_default();
        Ok((token, zone_id))
    }

    async fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }

    /// Resolve zone_id from a list of domains (auto-detection)
    async fn resolve_zone_id(
        client: &reqwest::Client,
        token: &str,
        domains: &[String],
    ) -> AppResult<String> {
        for domain in domains {
            let parts: Vec<&str> = domain.split('.').collect();
            for i in 0..parts.len().saturating_sub(1) {
                let candidate = parts[i..].join(".");
                let url = format!(
                    "https://api.cloudflare.com/client/v4/zones?name={}&status=active&per_page=3",
                    candidate
                );
                let resp = client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("CF zone lookup: {e}")))?;

                if !resp.status().is_success() {
                    continue;
                }
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                if body["success"].as_bool() != Some(true) {
                    continue;
                }
                if let Some(zones) = body["result"].as_array() {
                    if let Some(zone) = zones.first() {
                        let zid = zone["id"].as_str().unwrap_or("");
                        let zn = zone["name"].as_str().unwrap_or("");
                        if !zid.is_empty() {
                            tracing::info!("DDNS: domain {domain} → zone {zn} ({zid})");
                            return Ok(zid.to_string());
                        }
                    }
                }
            }
        }
        Err(AppError::Internal(anyhow::anyhow!(
            "无法自动获取 Zone ID：域名 {} 未匹配到任何 Zone，请手动填写",
            domains.join(", ")
        )))
    }

    /// Validate that a configured Zone ID matches at least one domain
    async fn validate_zone_id(
        client: &reqwest::Client,
        token: &str,
        zone_id: &str,
        domains: &[String],
    ) -> AppResult<()> {
        let url = format!("https://api.cloudflare.com/client/v4/zones/{}", zone_id);
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("CF zone lookup: {e}")))?;

        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if body["success"].as_bool() != Some(true) {
            return Err(AppError::Internal(anyhow::anyhow!("Zone not found")));
        }

        let zone_name = body["result"]["name"].as_str().unwrap_or("");
        for domain in domains {
            if domain == zone_name || domain.ends_with(&format!(".{zone_name}")) {
                return Ok(());
            }
        }
        Err(AppError::Internal(anyhow::anyhow!(
            "Zone '{zone_name}' ({zone_id}) 不匹配任何域名 ({})",
            domains.join(", ")
        )))
    }

    /// Resolve or validate zone_id; auto-detect if not configured
    pub async fn resolve_or_validate_zone_id(
        client: &reqwest::Client,
        token: &str,
        zone_id: &str,
        domains: &[String],
    ) -> AppResult<String> {
        if zone_id.is_empty() {
            Self::resolve_zone_id(client, token, domains).await
        } else {
            if let Err(e) = Self::validate_zone_id(client, token, zone_id, domains).await {
                tracing::warn!("DDNS: Configured Zone ID may be wrong: {e}. Will try anyway.");
            }
            Ok(zone_id.to_string())
        }
    }

    async fn create_record(
        client: &reqwest::Client,
        token: &str,
        zone_id: &str,
        domain: &str,
        ip: &str,
        record_type: &str,
        db: &SqlitePool,
    ) -> AppResult<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            zone_id
        );
        let res = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "type": record_type,
                "name": domain,
                "content": ip,
                "ttl": 1,
                "proxied": false,
            }))
            .send()
            .await?;

        let body: serde_json::Value = res.json().await.unwrap_or_default();
        if body["success"].as_bool() == Some(true) {
            let msg = format!("DDNS: Created {} record {} -> {}", record_type, domain, ip);
            tracing::info!("{msg}");
            crate::logger::info(db, &msg).await;
            Ok(())
        } else {
            let msgs: Vec<String> = body["errors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e["message"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let msg = format!("DDNS: Create {} failed: {}", domain, msgs.join(", "));
            tracing::error!("{msg}");
            crate::logger::error(db, &msg).await;
            Err(AppError::Internal(anyhow::anyhow!("DDNS create failed for {domain}: {msg}")))
        }
    }

    async fn update_one(
        client: &reqwest::Client,
        token: &str,
        zone_id: &str,
        record_id: &str,
        domain: &str,
        ip: &str,
        record_type: &str,
        db: &SqlitePool,
    ) -> AppResult<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            zone_id, record_id
        );
        let res = client
            .put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "type": record_type,
                "name": domain,
                "content": ip,
                "ttl": 1,
                "proxied": false,
            }))
            .send()
            .await?;

        let body: serde_json::Value = res.json().await.unwrap_or_default();
        if body["success"].as_bool() == Some(true) {
            let msg = format!("DDNS: Updated {} {} -> {}", domain, record_type, ip);
            tracing::info!("{msg}");
            crate::logger::info(db, &msg).await;
            Ok(())
        } else {
            let msgs: Vec<String> = body["errors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e["message"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let msg = format!("DDNS: Update {} failed: {}", domain, msgs.join(", "));
            tracing::error!("{msg}");
            crate::logger::error(db, &msg).await;
            Err(AppError::Internal(anyhow::anyhow!("DDNS update failed for {domain}: {msg}")))
        }
    }
}

#[async_trait::async_trait]
impl DnsProvider for CloudflareProvider {
    async fn upsert_record(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        ip: &str,
        record_type: &str,
    ) -> AppResult<()> {
        let (token, mut zone_id) = Self::get_credentials(state)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;

        let client = Self::client().await;

        // Auto-detect zone if needed
        let domains = vec![domain.to_string()];
        zone_id = Self::resolve_or_validate_zone_id(&client, &token, &zone_id, &domains).await?;

        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records?type={}&name={}&per_page=50",
            zone_id, record_type, domain
        );

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = format!("DDNS: CF lookup HTTP {status} for {domain}: {body}");
            tracing::warn!("{msg}");
            crate::logger::warn(&state.db, &msg).await;
            return Ok(());
        }

        let body: serde_json::Value = resp.json().await?;
        if body["success"].as_bool() != Some(true) {
            let msgs: Vec<String> = body["errors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e["message"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let msg = format!("DDNS: CF lookup failed for {}: {}", domain, msgs.join(", "));
            tracing::error!("{msg}");
            crate::logger::error(&state.db, &msg).await;
            return Ok(());
        }

        let records = body["result"].as_array().cloned().unwrap_or_default();

        if records.is_empty() {
            // If we've managed this domain before (cached IP exists) but the
            // record is now gone from Cloudflare, the user deleted it intentionally.
            // Don't auto-recreate — that would be a bug.
            let type_key = match record_type {
                "A" => "ipv4",
                "AAAA" => "ipv6",
                _ => return Ok(()),
            };
            let key = format!("ddns_{}_{}", type_key, domain);
            let has_been_managed = get_setting(&state.db, &key)
                .await
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if has_been_managed {
                let msg = format!(
                    "DDNS: {} {} 之前被 Unver 管理，但在 Cloudflare 上已被删除（用户手动删除）。跳过自动重建。从 DDNS 域名列表移除该域名以消除此警告。",
                    record_type, domain
                );
                tracing::warn!("{msg}");
                crate::logger::warn(&state.db, &msg).await;
            } else {
                Self::create_record(&client, &token, &zone_id, domain, ip, record_type, &state.db).await?;
            }
        } else {
            for record in &records {
                let record_id = record["id"].as_str().unwrap_or("");
                let current_ip = record["content"].as_str().unwrap_or("");
                if record_id.is_empty() || current_ip.is_empty() {
                    continue;
                }
                if current_ip == ip {
                    tracing::debug!("DDNS: {} {} unchanged ({})", record_type, domain, ip);
                    continue;
                }
                Self::update_one(
                    &client,
                    &token,
                    &zone_id,
                    record_id,
                    domain,
                    ip,
                    record_type,
                    &state.db,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn delete_domain_records(
        &self,
        state: &Arc<AppState>,
        domain: &str,
    ) -> Result<usize, String> {
        let (token, zone_id) = Self::get_credentials(state).await?;
        if zone_id.is_empty() {
            return Err("Zone ID not configured".into());
        }

        let client = Self::client().await;
        let mut deleted = 0usize;

        for record_type in &["A", "AAAA"] {
            let url = format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records?type={}&name={}",
                zone_id, record_type, domain
            );
            let resp = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .map_err(|e| format!("CF API: {e}"))?;

            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let records = body["result"].as_array().cloned().unwrap_or_default();

            for record in &records {
                let rid = record["id"].as_str().unwrap_or("");
                if rid.is_empty() {
                    continue;
                }
                let del_url = format!(
                    "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
                    zone_id, rid
                );
                let _ = client
                    .delete(&del_url)
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await;
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    async fn list_zones(&self, token: &str) -> Result<Vec<serde_json::Value>, String> {
        let client = Self::client().await;
        let mut zones = Vec::new();
        let mut page = 1;

        loop {
            let resp = client
                .get(format!(
                    "https://api.cloudflare.com/client/v4/zones?per_page=50&page={}",
                    page
                ))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .map_err(|e| format!("CF API: {e}"))?;

            if !resp.status().is_success() {
                return Err(format!("Cloudflare API error: HTTP {}", resp.status()));
            }

            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if body["success"].as_bool() != Some(true) {
                let msgs: Vec<String> = body["errors"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| e["message"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                return Err(msgs.join(", "));
            }

            if let Some(arr) = body["result"].as_array() {
                zones.extend(arr.iter().cloned());
            } else {
                break;
            }

            let total = body["result_info"]["total_count"].as_i64().unwrap_or(0);
            if zones.len() as i64 >= total {
                break;
            }
            page += 1;
        }

        Ok(zones)
    }
}
