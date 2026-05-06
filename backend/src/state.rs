use std::sync::Arc;
use governor::{
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use sqlx::SqlitePool;
use std::num::NonZeroU32;
use std::collections::HashMap;
use std::sync::RwLock;
use anyhow::Result;
use rand::Rng;

use crate::config::Config;
use crate::errors::AppResult;
use crate::models::{BackgroundJob, LogLine};
use crate::network::{NetTracker, SharedNetTracker};

pub type SharedRateLimiter =
    Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>;

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    /// Rate limiter: max 10 login attempts per minute (global)
    pub login_limiter: SharedRateLimiter,
    /// TLS certificate cache: domain -> CertifiedKey, updated by SSL manager
    pub cert_cache: Arc<RwLock<HashMap<String, Arc<rustls::sign::CertifiedKey>>>>,
    /// Background job tracker: job_id -> BackgroundJob (for SSL issuance etc.)
    pub background_jobs: Arc<tokio::sync::Mutex<HashMap<String, BackgroundJob>>>,
    /// Real-time log buffers for background jobs (job_id -> log lines)
    pub background_job_logs: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Vec<LogLine>>>>>>,
    /// Network traffic tracker
    pub net_tracker: SharedNetTracker,
    /// Server start time for uptime display
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool, net_tracker: SharedNetTracker) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(10).unwrap());
        let login_limiter = Arc::new(RateLimiter::direct(quota));
        Self { config, db, login_limiter, cert_cache: Arc::new(RwLock::new(HashMap::new())), background_jobs: Arc::new(tokio::sync::Mutex::new(HashMap::new())), background_job_logs: Arc::new(tokio::sync::Mutex::new(HashMap::new())), net_tracker, start_time: std::time::Instant::now() }
    }
}

/// Ensures a JWT signing secret exists in the settings table.
/// Generates a 64-char random secret on first run.
pub async fn ensure_jwt_secret(state: &Arc<AppState>) -> Result<()> {
    let row = sqlx::query!("SELECT value FROM settings WHERE key = 'jwt_secret'")
        .fetch_optional(&state.db)
        .await?;

    let secret_empty = row.map(|r| r.value.is_empty()).unwrap_or(true);

    if secret_empty {
        let secret: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        sqlx::query!(
            "INSERT INTO settings (key, value) VALUES ('jwt_secret', ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            secret
        )
        .execute(&state.db)
        .await?;

        tracing::info!("Generated new JWT signing secret");
    }

    Ok(())
}

/// Helper: get a setting value
pub async fn get_setting(db: &SqlitePool, key: &str) -> AppResult<String> {
    let row = sqlx::query!("SELECT value FROM settings WHERE key = ?", key)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| r.value).unwrap_or_default())
}

/// Helper: set a setting value
pub async fn set_setting(db: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        key,
        value
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Helper: get a setting that may be encrypted with the local JWT secret.
pub async fn get_secret_setting(db: &SqlitePool, key: &str) -> AppResult<String> {
    let value = get_setting(db, key).await?;
    let Some(encrypted) = value.strip_prefix("enc:") else {
        return Ok(value);
    };

    let jwt_secret = get_setting(db, "jwt_secret").await?;
    crate::security::decrypt_data(encrypted, &jwt_secret)
}

/// Helper: encrypt and store a sensitive setting.
pub async fn set_secret_setting(db: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    let jwt_secret = get_setting(db, "jwt_secret").await?;
    let encrypted = crate::security::encrypt_data(value, &jwt_secret)?;
    set_setting(db, key, &format!("enc:{encrypted}")).await
}
