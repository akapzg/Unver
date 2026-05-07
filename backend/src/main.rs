mod api;
mod cli;
mod config;
mod db;
mod ddns;
mod errors;
mod health;
mod logger;
mod middleware;
mod models;
mod network;
mod proxy;
mod security;
mod ssl;
mod ssl_worker;
mod state;

use std::sync::Arc;
use anyhow::Result;
use mimalloc::MiMalloc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    // CLI commands (version, update, restart, status, interactive menu) exit early.
    // Returns false for "start" — fall through to start the full service.
    if cli::run().await? {
        return Ok(());
    }

    dotenvy::dotenv().ok();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring crypto provider");

    let config = config::Config::load()?;

    // ── Logging Configuration ────────────────────────────────────────────────
    let log_dir = config.data_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    
    let file_appender = tracing_appender::rolling::daily(&log_dir, "unver.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("unver={},tower_http=warn", config.log_level).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .init();

    tracing::info!(
        "Starting Unver v{} on port {}",
        env!("CARGO_PKG_VERSION"),
        config.web_port
    );

    // ── Shutdown signal (broadcast to all spawned tasks) ─────────────────────
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let pool = db::setup(&config).await?;
    let net_tracker = Arc::new(tokio::sync::RwLock::new(network::NetTracker::new()));
    let state = Arc::new(state::AppState::new(config.clone(), pool, Arc::clone(&net_tracker)));

    state::ensure_jwt_secret(&state).await?;

    // Check first-run status
    let setup_done = state::get_setting(&state.db, "setup_complete").await.unwrap_or_default() == "true";
    if !setup_done {
        tracing::warn!("⚠ First run detected — please complete setup at http://0.0.0.0:{}/setup", config.web_port);
    }

    // ── Spawn SSL worker thread (dedicated std::thread for ACME ops) ──────────
    let ssl_worker = ssl_worker::spawn();

    // ── Load SSL certificates into cache BEFORE proxy starts ────────────────
    if let Err(e) = ssl::load_certs_to_cache(&state).await {
        tracing::error!("Failed to load certs into cache: {e}");
    }

    // ── Spawn background tasks (each watches for shutdown) ───────────────────

    // Proxy engine
    let proxy_state = Arc::clone(&state);
    let mut proxy_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            _ = proxy::run_proxy_engine(proxy_state) => {},
            _ = proxy_rx.recv() => {
                tracing::info!("Proxy engine shutting down");
            }
        }
    });

    // SSL manager
    let ssl_state = Arc::clone(&state);
    let mut ssl_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            _ = ssl::run_ssl_manager(ssl_state) => {},
            _ = ssl_rx.recv() => {
                tracing::info!("SSL manager shutting down");
            }
        }
    });

    // DDNS manager
    let ddns_state = Arc::clone(&state);
    let mut ddns_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            _ = ddns::run_ddns_manager(ddns_state) => {},
            _ = ddns_rx.recv() => {
                tracing::info!("DDNS manager shutting down");
            }
        }
    });

    // Health checker
    let health_state = Arc::clone(&state);
    let mut health_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            _ = health::run_health_checker(health_state) => {},
            _ = health_rx.recv() => {
                tracing::info!("Health checker shutting down");
            }
        }
    });

    // Log pruner + WAL checkpoint
    let log_state = Arc::clone(&state);
    let mut log_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tracing::debug!("Log pruner started");
        loop {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(3600)) => {
                    logger::prune_logs(&log_state.db, 10_000).await;
                    logger::checkpoint(&log_state.db).await;
                }
                _ = log_rx.recv() => {
                    tracing::info!("Log pruner shutting down");
                    break;
                }
            }
        }
    });

    // Network monitor
    let net_mon = Arc::clone(&net_tracker);
    let mut net_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            _ = network::run_network_monitor(net_mon) => {},
            _ = net_rx.recv() => {
                tracing::info!("Network monitor shutting down");
            }
        }
    });

    // ── Graceful shutdown handler ────────────────────────────────────────────
    let shutdown_signal = async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        tracing::info!("Received SIGTERM / Ctrl+C, initiating graceful shutdown...");
        let _ = shutdown_tx.send(());
        // Give background tasks a moment to finish
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    };

    // Run web + API server (blocks until shutdown signal)
    api::serve(state, &config, shutdown_signal, ssl_worker).await?;

    tracing::info!("Unver stopped");
    Ok(())
}
