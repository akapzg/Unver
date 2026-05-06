use sqlx::SqlitePool;

pub async fn log_event(db: &SqlitePool, level: &str, message: &str) {
    let _ = sqlx::query!(
        "INSERT INTO logs (level, message, created_at) VALUES (?, ?, datetime('now','localtime'))",
        level,
        message
    )
    .execute(db)
    .await;
}

// Helper methods
pub async fn info(db: &SqlitePool, message: &str) {
    log_event(db, "INFO", message).await;
}

pub async fn warn(db: &SqlitePool, message: &str) {
    log_event(db, "WARN", message).await;
}

pub async fn error(db: &SqlitePool, message: &str) {
    log_event(db, "ERROR", message).await;
}

/// Prune old log entries, keeping the most recent `keep` rows
pub async fn prune_logs(db: &SqlitePool, keep: i64) {
    let deleted = sqlx::query!(
        "DELETE FROM logs WHERE id NOT IN (SELECT id FROM logs ORDER BY id DESC LIMIT ?)",
        keep
    )
    .execute(db)
    .await;

    match deleted {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::debug!("Pruned {} old log entries", r.rows_affected());
        }
        Err(e) => {
            tracing::error!("Log pruning failed: {e}");
        }
        _ => {}
    }
}

/// Run SQLite WAL checkpoint to prevent unlimited WAL file growth
pub async fn checkpoint(db: &SqlitePool) {
    // TRUNCATE mode: reset WAL file to zero after checkpoint
    match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(db)
        .await
    {
        Ok(_) => tracing::debug!("WAL checkpoint completed"),
        Err(e) => tracing::error!("WAL checkpoint failed: {e}"),
    }
}
