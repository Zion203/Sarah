use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::error::AppError;

pub mod migrations;
pub mod models;

#[derive(Clone)]
pub struct Database {
    write_pool: SqlitePool,
    read_pool: SqlitePool,
    pub db_path: PathBuf,
}

#[derive(Clone)]
pub struct DatabaseState(pub Arc<Database>);

impl Database {
    pub async fn new(app_handle: &AppHandle, max_read_connections: u32) -> Result<Self, AppError> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Config(format!("Failed to resolve app data dir: {e}")))?;

        tokio::fs::create_dir_all(&app_data_dir).await?;

        let db_path = app_data_dir.join("app.db");
        let base_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .pragma("cache_size", "-64000")
            .pragma("temp_store", "MEMORY")
            .pragma("mmap_size", "536870912");

        // Create write and read pools concurrently for faster startup
        let write_opts = base_options.clone();
        let read_opts = base_options;

        let (write_result, read_result) = tokio::join!(
            SqlitePoolOptions::new()
                .max_connections(1)
                .min_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(write_opts),
            SqlitePoolOptions::new()
                .max_connections(max_read_connections)
                .min_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(read_opts),
        );

        let write_pool = write_result?;
        let read_pool = read_result?;

        migrations::run_migrations(&write_pool).await?;

        Ok(Self {
            write_pool,
            read_pool,
            db_path,
        })
    }

    pub fn write_pool(&self) -> &SqlitePool {
        &self.write_pool
    }

    pub fn read_pool(&self) -> &SqlitePool {
        &self.read_pool
    }

    /// Run PRAGMA optimize before closing. Call this on app shutdown.
    pub async fn optimize(&self) {
        let _ = sqlx::query("PRAGMA optimize")
            .execute(&self.write_pool)
            .await;
        tracing::info!("Database PRAGMA optimize executed");
    }
}
