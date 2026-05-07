// ssl_worker.rs — Dedicated std::thread for certificate issuance
//
// Runs ACME operations (including set_ready) on a single-threaded tokio runtime
// to avoid the Send constraint that breaks with instant-acme's non-Sync types.

use std::sync::Arc;
use std::collections::HashMap;
use std::thread;
use tokio::sync::{mpsc, oneshot};
use sqlx::SqlitePool;

/// Request from the API handler to the worker thread
pub struct SslRequest {
    pub email: String,
    pub domains: Vec<String>,
    pub provider_name: String,
    pub state: Arc<crate::state::AppState>,
    pub use_staging: bool,
    pub db: SqlitePool,
    pub cert_cache: Arc<std::sync::RwLock<HashMap<String, Arc<rustls::sign::CertifiedKey>>>>,
    pub response: oneshot::Sender<Result<SslResult, String>>,
    pub log_buf: Option<Arc<tokio::sync::Mutex<Vec<crate::models::LogLine>>>>,
}

#[derive(Debug, Clone)]
pub struct SslResult {
    #[allow(dead_code)]
    pub primary_domain: String,
    #[allow(dead_code)]
    pub cert_pem: String,
    #[allow(dead_code)]
    pub key_pem: String,
    pub expires_at: String,
}

/// Public handle for submitting SSL requests from async code
pub struct SslWorkerHandle {
    tx: mpsc::UnboundedSender<SslRequest>,
}

impl SslWorkerHandle {
    pub fn issue(&self, req: SslRequest) {
        let _ = self.tx.send(req);
    }

    /// Create a dummy handle for testing (no worker thread needed).
    #[cfg(test)]
    pub fn dummy() -> Self {
        let (tx, _rx) = mpsc::unbounded_channel::<SslRequest>();
        // Drop rx immediately — issue() calls will silently fail
        // This is fine for tests that don't test SSL issuance
        Self { tx }
    }
}

/// Spawn the SSL worker thread. Returns a sharable handle.
pub fn spawn() -> Arc<SslWorkerHandle> {
    let (tx, mut rx) = mpsc::unbounded_channel::<SslRequest>();

    thread::Builder::new()
        .name("ssl-worker".into())
        .spawn(move || {
            // Single-threaded runtime — no Send required for futures
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create ssl-worker runtime");

            while let Some(req) = rx.blocking_recv() {
                let log_buf = req.log_buf.clone();
                let result = rt.block_on(async {
                    crate::ssl::issue_certificate_sync(
                        &req.email,
                        &req.domains,
                        &req.provider_name,
                        &req.state,
                        req.use_staging,
                        &req.db,
                        &req.cert_cache,
                        log_buf,
                    ).await
                });
                let _ = req.response.send(result);
            }
        })
        .expect("Failed to spawn ssl-worker thread");

    Arc::new(SslWorkerHandle { tx })
}
