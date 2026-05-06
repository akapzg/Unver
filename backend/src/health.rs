use std::sync::Arc;
use crate::state::AppState;
use reqwest::Client;
use std::time::Duration;

pub async fn run_health_checker(state: Arc<AppState>) {
    tracing::info!("Health checker started");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true) // For internal services with self-signed certs
        .build()
        .unwrap_or_else(|_| Client::new());

    loop {
        if let Err(e) = check_all_upstreams(&state, &client).await {
            tracing::error!("Health check loop error: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn check_all_upstreams(state: &Arc<AppState>, client: &Client) -> anyhow::Result<()> {
    let rules = sqlx::query!("SELECT id, target_url FROM proxy_rules WHERE enabled = 1")
        .fetch_all(&state.db)
        .await?;

    for rule in rules {
        let status = match client.get(&rule.target_url).send().await {
            Ok(resp) if resp.status().is_success() => "online",
            Ok(_) => "error",
            Err(_) => "offline",
        };

        sqlx::query!(
            "UPDATE proxy_rules SET status = ?, last_checked_at = datetime('now') WHERE id = ?",
            status,
            rule.id
        )
        .execute(&state.db)
        .await?;
    }

    Ok(())
}
