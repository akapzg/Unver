use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use anyhow::Result;

use crate::config::Config;

pub async fn setup(config: &Config) -> Result<SqlitePool> {
    std::fs::create_dir_all(&config.data_dir)?;

    let db_path = config.database_path();
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let options = SqliteConnectOptions::from_str(&db_url)?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Database ready at {}", db_path.display());
    Ok(pool)
}
