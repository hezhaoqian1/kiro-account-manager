//! Postgres-backed persistence for the headless deployment.
//!
//! The existing desktop code intentionally keeps its file-oriented stores. The
//! server mirrors those files into a single database row per logical document,
//! which keeps the gateway/account implementation shared while making Railway
//! redeploys independent from an attached filesystem volume.

use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};
use tokio::time::interval;

const STATE_FILES: &[&str] = &[
    "accounts.json",
    "groups-tags.json",
    "gateway-config.json",
    "app-settings.json",
    "usage-history.json",
];

#[derive(Clone)]
pub struct DatabaseStore {
    pool: PgPool,
    data_dir: PathBuf,
}

impl DatabaseStore {
    pub async fn connect(database_url: &str, data_dir: PathBuf) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(15))
            .connect(database_url)
            .await
            .context("connect DATABASE_URL")?;
        let store = Self { pool, data_dir };
        store.init().await?;
        Ok(store)
    }

    async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS kiro_state (
                state_key TEXT PRIMARY KEY,
                state_value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await
        .context("create kiro_state table")?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .context("ping database")?;
        Ok(())
    }

    fn path_for(&self, relative: &str) -> PathBuf {
        self.data_dir.join(relative)
    }

    fn is_safe_relative(relative: &str) -> bool {
        !Path::new(relative).is_absolute()
            && Path::new(relative)
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
    }

    /// Restore database state only when it exists. Fresh installs keep the
    /// local defaults, which are then uploaded by `sync_now`.
    pub async fn restore(&self) -> Result<()> {
        for relative in STATE_FILES {
            if let Some(value) = self.read_row(relative).await? {
                self.write_file(relative, value.as_bytes()).await?;
            }
        }

        if let Some(value) = self.read_row("logs/gateway-request-log.jsonl").await? {
            self.write_file("logs/gateway-request-log.jsonl", value.as_bytes()).await?;
        }

        let cache_rows = sqlx::query(
            "SELECT state_key, state_value FROM kiro_state WHERE state_key LIKE 'cache/%'",
        )
        .fetch_all(&self.pool)
        .await
        .context("read response cache rows")?;
        for row in cache_rows {
            let key: String = row.try_get("state_key")?;
            if !Self::is_safe_relative(&key) {
                log::warn!("ignore unsafe persisted cache path: {key}");
                continue;
            }
            let value: String = row.try_get("state_value")?;
            self.write_file(&key, value.as_bytes())
                .await?;
        }
        Ok(())
    }

    async fn read_row(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT state_value FROM kiro_state WHERE state_key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("read database state {key}"))?;
        row.map(|value| value.try_get("state_value")).transpose().map_err(Into::into)
    }

    async fn write_file(&self, relative: &str, bytes: &[u8]) -> Result<()> {
        let path = self.path_for(relative);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }

    async fn upsert(&self, key: &str, value: String) -> Result<()> {
        sqlx::query(
            "INSERT INTO kiro_state (state_key, state_value, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (state_key) DO UPDATE
             SET state_value = EXCLUDED.state_value, updated_at = NOW()",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .with_context(|| format!("write database state {key}"))?;
        Ok(())
    }

    async fn sync_file(&self, relative: &str) -> Result<()> {
        let path = self.path_for(relative);
        if let Ok(value) = tokio::fs::read_to_string(path).await {
            self.upsert(relative, value).await?;
        }
        Ok(())
    }

    async fn delete_row(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM kiro_state WHERE state_key = $1")
            .bind(key)
            .execute(&self.pool)
            .await
            .with_context(|| format!("delete database state {key}"))?;
        Ok(())
    }

    async fn sync_cache(&self) -> Result<()> {
        // Cache commands remove files on disk. Clear the corresponding rows
        // first so a later deploy cannot resurrect an explicitly cleared cache.
        sqlx::query("DELETE FROM kiro_state WHERE state_key LIKE 'cache/%'")
            .execute(&self.pool)
            .await
            .context("clear persisted response cache")?;
        let cache_dir = self.path_for("cache");
        let mut entries = match tokio::fs::read_dir(&cache_dir).await {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let relative = format!("cache/{}", entry.file_name().to_string_lossy());
            let value = tokio::fs::read_to_string(entry.path()).await?;
            self.upsert(&relative, value).await?;
        }
        Ok(())
    }

    pub async fn sync_now(&self) -> Result<()> {
        for relative in STATE_FILES {
            self.sync_file(relative).await?;
        }
        let log_path = self.path_for("logs/gateway-request-log.jsonl");
        if tokio::fs::try_exists(&log_path).await.unwrap_or(false) {
            self.sync_file("logs/gateway-request-log.jsonl").await?;
        } else {
            self.delete_row("logs/gateway-request-log.jsonl").await?;
        }
        self.sync_cache().await?;
        Ok(())
    }

    pub fn spawn_sync_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(10));
            loop {
                ticker.tick().await;
                if let Err(error) = self.sync_now().await {
                    log::error!("database state sync failed: {error:#}");
                }
            }
        });
    }
}

pub fn data_file_exists(data_dir: &Path, relative: &str) -> bool {
    data_dir.join(relative).exists()
}
