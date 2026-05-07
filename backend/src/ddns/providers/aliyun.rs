use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::ddns::providers::DnsProvider;
use crate::errors::{AppError, AppResult};
use crate::state::{AppState, get_secret_setting};
use ring::hmac;

const ALIDNS_ENDPOINT: &str = "https://alidns.aliyuncs.com";
const API_VERSION: &str = "2015-01-09";

pub struct AliyunProvider;

impl AliyunProvider {
    async fn get_credentials(state: &Arc<AppState>) -> Result<(String, String), String> {
        let access_key_id = get_secret_setting(&state.db, "ddns_aliyun_access_key_id")
            .await
            .map_err(|e| format!("AccessKey ID: {e}"))?;
        if access_key_id.is_empty() {
            return Err("阿里云 AccessKey ID 未配置".into());
        }
        let access_key_secret = get_secret_setting(&state.db, "ddns_aliyun_access_key_secret")
            .await
            .map_err(|e| format!("AccessKey Secret: {e}"))?;
        if access_key_secret.is_empty() {
            return Err("阿里云 AccessKey Secret 未配置".into());
        }
        Ok((access_key_id, access_key_secret))
    }

    /// Aliyun API percent-encoding (RFC 3986, uppercase hex)
    fn percent_encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(b as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
        result
    }

    /// Generate Aliyun API Signature V1 (HMAC-SHA1 → base64)
    fn sign(method: &str, params: &[(&str, &str)], secret: &str) -> String {
        // Sort params by key (ascending)
        let mut sorted: Vec<_> = params.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));

        // Build canonicalized query string
        let canonical = sorted
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    Self::percent_encode(k),
                    Self::percent_encode(v)
                )
            })
            .collect::<Vec<_>>()
            .join("&");

        // StringToSign = HTTPMethod + "&" + percentEncode("/") + "&" + percentEncode(canonical)
        let string_to_sign = format!(
            "{}&{}&{}",
            method,
            Self::percent_encode("/"),
            Self::percent_encode(&canonical)
        );

        // HMAC-SHA1 with key = secret + "&"
        let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, format!("{secret}&").as_bytes());
        let tag = hmac::sign(&key, string_to_sign.as_bytes());

        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(tag.as_ref())
    }

    /// Build a signed GET request URL for Aliyun API
    fn build_request(
        action: &str,
        params: Vec<(&str, &str)>,
        access_key_id: &str,
        access_key_secret: &str,
    ) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let nonce = uuid::Uuid::new_v4().to_string();

        let mut common_params = vec![
            ("Action", action),
            ("Version", API_VERSION),
            ("Format", "JSON"),
            ("SignatureMethod", "HMAC-SHA1"),
            ("SignatureVersion", "1.0"),
            ("AccessKeyId", access_key_id),
            ("Timestamp", &timestamp),
            ("SignatureNonce", &nonce),
        ];

        common_params.extend(params);

        let signature = Self::sign("GET", &common_params, access_key_secret);

        // Build URL
        let mut query_pairs: Vec<String> = common_params
            .iter()
            .map(|(k, v)| {
                format!("{}={}", Self::percent_encode(k), Self::percent_encode(v))
            })
            .collect();
        query_pairs.push(format!("Signature={}", Self::percent_encode(&signature)));

        format!("{}?{}", ALIDNS_ENDPOINT, query_pairs.join("&"))
    }

    /// Call Aliyun API and parse response
    async fn call_api(
        action: &str,
        params: Vec<(&str, &str)>,
        access_key_id: &str,
        access_key_secret: &str,
    ) -> Result<serde_json::Value, String> {
        let url = Self::build_request(action, params, access_key_id, access_key_secret);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("阿里云 API 请求失败: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("阿里云 API HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON 解析失败: {e}"))?;

        // Check for API errors
        if let Some(code) = body["Code"].as_str() {
            let msg = body["Message"].as_str().unwrap_or(code);
            return Err(format!("阿里云 {action}: {msg}"));
        }

        Ok(body)
    }

    /// Resolve a full domain (e.g. "home.thepzg.site") to (DomainName, RR)
    /// DomainName = the registered domain in Aliyun DNS (e.g. "thepzg.site")
    /// RR = the subdomain part (e.g. "home" or "@" for apex)
    async fn resolve_domain_name(
        full_domain: &str,
        ak_id: &str,
        ak_secret: &str,
    ) -> Result<(String, String), String> {
        let parts: Vec<&str> = full_domain.split('.').collect();
        // Try from longest possible domain down to 2-part domain
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            let rr = if i == 0 {
                "@".to_string()
            } else {
                parts[..i].join(".")
            };

            // Test if this domain exists in Aliyun DNS
            match Self::call_api(
                "DescribeDomainRecords",
                vec![("DomainName", &candidate), ("PageSize", "1")],
                ak_id,
                ak_secret,
            )
            .await
            {
                Ok(_) => return Ok((candidate, rr)),
                Err(e) => {
                    if e.contains("DomainNotExist") || e.contains("InvalidDomainName") {
                        continue;
                    }
                    // Other errors (network, auth) → propagate
                    return Err(e);
                }
            }
        }
        Err(format!("域名 {full_domain} 未在阿里云 DNS 中找到"))
    }
}

#[async_trait::async_trait]
impl DnsProvider for AliyunProvider {
    async fn upsert_record(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        ip: &str,
        record_type: &str,
    ) -> AppResult<()> {
        let (ak_id, ak_secret) = Self::get_credentials(state)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;

        // Resolve domain name (e.g. home.example.com → DomainName=example.com, RR=home)
        let (domain_name, rr) = Self::resolve_domain_name(domain, &ak_id, &ak_secret)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DDNS 解析 {domain}: {e}")))?;

        // Find existing record
        let resp = Self::call_api(
            "DescribeDomainRecords",
            vec![
                ("DomainName", &domain_name),
                ("RRKeyWord", &rr),
                ("Type", record_type),
                ("PageSize", "50"),
            ],
            &ak_id,
            &ak_secret,
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("DDNS 查询 {domain}: {e}")))?;

        let records = resp["DomainRecords"]["Record"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        // Filter to exact RR match
        let exact: Vec<_> = records.into_iter()
            .filter(|r| r["RR"].as_str() == Some(&rr))
            .collect();

        if exact.is_empty() {
            // Create new record
            Self::call_api(
                "AddDomainRecord",
                vec![
                    ("DomainName", &domain_name),
                    ("RR", &rr),
                    ("Type", record_type),
                    ("Value", ip),
                    ("TTL", "120"),
                ],
                &ak_id,
                &ak_secret,
            )
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DDNS 创建 {domain}: {e}")))?;

            let msg = format!("DDNS: Created {} record {} -> {}", record_type, domain, ip);
            tracing::info!("{msg}");
            crate::logger::info(&state.db, &msg).await;
        } else {
            // Update existing records if IP differs
            for record in &exact {
                let record_id = record["RecordId"].as_str().unwrap_or("");
                let current_ip = record["Value"].as_str().unwrap_or("");

                if record_id.is_empty() {
                    continue;
                }
                if current_ip == ip {
                    tracing::debug!("DDNS: {} {} unchanged ({})", record_type, domain, ip);
                    continue;
                }

                Self::call_api(
                    "UpdateDomainRecord",
                    vec![
                        ("RecordId", record_id),
                        ("RR", &rr),
                        ("Type", record_type),
                        ("Value", ip),
                        ("TTL", "120"),
                    ],
                    &ak_id,
                    &ak_secret,
                )
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("DDNS 更新 {domain}: {e}"))
                })?;

                let msg = format!("DDNS: Updated {} {} -> {}", domain, record_type, ip);
                tracing::info!("{msg}");
                crate::logger::info(&state.db, &msg).await;
            }
        }

        Ok(())
    }

    async fn delete_domain_records(
        &self,
        state: &Arc<AppState>,
        domain: &str,
    ) -> Result<usize, String> {
        let (ak_id, ak_secret) = Self::get_credentials(state).await?;

        // Resolve domain name
        let (domain_name, rr) = match Self::resolve_domain_name(domain, &ak_id, &ak_secret).await {
            Ok(d) => d,
            Err(_) => return Ok(0), // domain not in Aliyun DNS, nothing to delete
        };

        let mut deleted = 0usize;

        for record_type in &["A", "AAAA"] {
            let resp = Self::call_api(
                "DescribeDomainRecords",
                vec![
                    ("DomainName", &domain_name),
                    ("RRKeyWord", &rr),
                    ("Type", record_type),
                    ("PageSize", "50"),
                ],
                &ak_id,
                &ak_secret,
            )
            .await?;

            let records = resp["DomainRecords"]["Record"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            // Filter to exact RR match
            for record in &records {
                if record["RR"].as_str() != Some(&rr) { continue; }
                let rid = record["RecordId"].as_str().unwrap_or("");
                if rid.is_empty() {
                    continue;
                }
                let _ = Self::call_api(
                    "DeleteDomainRecord",
                    vec![("RecordId", rid)],
                    &ak_id,
                    &ak_secret,
                )
                .await;
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    async fn list_zones(&self, _token: &str) -> Result<Vec<serde_json::Value>, String> {
        // Aliyun uses AccessKey pair, not single token.
        // The frontend passes AccessKey ID as the "token" for zone listing.
        // But we also need the secret. For now, zones are auto-detected by domain name.
        // This method is used by zone-fetch UI; return empty and let auto-detection work.
        Ok(vec![])
    }

    async fn create_acme_txt(
        &self,
        state: &Arc<AppState>,
        domain: &str,
        value: &str,
    ) -> Result<String, String> {
        let (ak_id, ak_secret) = Self::get_credentials(state).await?;

        // ACME domains are typically bare domains (thepzg.site), but resolve for safety
        let (domain_name, _zone_rr) = Self::resolve_domain_name(domain, &ak_id, &ak_secret).await?;
        let rr = "_acme-challenge";

        let resp = Self::call_api(
            "AddDomainRecord",
            vec![
                ("DomainName", &domain_name),
                ("RR", rr),
                ("Type", "TXT"),
                ("Value", value),
                ("TTL", "60"),
            ],
            &ak_id,
            &ak_secret,
        )
        .await?;

        resp["RecordId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!("No RecordId in response: {resp}"))
    }

    async fn delete_acme_txt(
        &self,
        state: &Arc<AppState>,
        _domain: &str,
        record_id: &str,
    ) -> Result<(), String> {
        let (ak_id, ak_secret) = Self::get_credentials(state).await?;

        Self::call_api(
            "DeleteDomainRecord",
            vec![("RecordId", record_id)],
            &ak_id,
            &ak_secret,
        )
        .await?;

        Ok(())
    }

    async fn cleanup_acme_txts(
        &self,
        state: &Arc<AppState>,
        domain: &str,
    ) -> Result<usize, String> {
        let (ak_id, ak_secret) = Self::get_credentials(state).await?;

        // Resolve domain name
        let (domain_name, _zone_rr) = match Self::resolve_domain_name(domain, &ak_id, &ak_secret).await {
            Ok(d) => d,
            Err(_) => return Ok(0),
        };

        // List all _acme-challenge TXT records
        let resp = Self::call_api(
            "DescribeDomainRecords",
            vec![
                ("DomainName", &domain_name),
                ("RRKeyWord", "_acme-challenge"),
                ("TypeKeyWord", "TXT"),
                ("PageSize", "50"),
            ],
            &ak_id,
            &ak_secret,
        )
        .await?;

        let records = resp["DomainRecords"]["Record"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let mut deleted = 0usize;
        for record in &records {
            let rid = record["RecordId"].as_str().unwrap_or("");
            if rid.is_empty() {
                continue;
            }
            if Self::call_api(
                "DeleteDomainRecord",
                vec![("RecordId", rid)],
                &ak_id,
                &ak_secret,
            )
            .await
            .is_ok()
            {
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}
