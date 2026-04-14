use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// Database bootstrap configuration for the Tauri layer.
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

/// Initialize SQLite pool in the application/bootstrap layer.
pub async fn init_db(cfg: &DbConfig) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(cfg.max_connections)
        .connect(&cfg.url)
        .await
}
