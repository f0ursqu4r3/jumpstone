//! Storage helpers for Postgres access.

use std::sync::Arc;

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;

pub mod messaging;
pub mod session;

pub use sqlx::PgPool;

pub use messaging::{Channel, ChannelEvent, Guild, MessagingRepository};
pub use session::{PersistedSession, SessionPersistence};

/// Thin wrapper around a shared `PgPool`.
#[derive(Clone)]
pub struct StoragePool {
    pool: Arc<PgPool>,
}

impl StoragePool {
    /// Wrap an existing pool in an `Arc` so it can be cloned safely.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    /// Borrow the underlying `PgPool`.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Clone the shared pool handle.
    pub fn cloned(&self) -> Arc<PgPool> {
        self.pool.clone()
    }
}

impl std::ops::Deref for StoragePool {
    type Target = PgPool;

    fn deref(&self) -> &Self::Target {
        self.pool()
    }
}

pub async fn connect(database_url: &str) -> Result<StoragePool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    Ok(StoragePool::new(pool))
}

pub fn validate_database_url(database_url: &str) -> Result<()> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(database_url)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use sqlx::migrate::Migrator;
    use std::env;

    static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

    #[test]
    fn discovers_migrations() {
        assert!(
            !MIGRATOR.migrations.is_empty(),
            "expected at least one migration"
        );
    }

    #[tokio::test]
    async fn migrations_apply_when_database_available() -> anyhow::Result<()> {
        let database_url =
            match env::var("OPENGUILD_TEST_DATABASE_URL").or_else(|_| env::var("DATABASE_URL")) {
                Ok(url) => url,
                Err(_) => {
                    eprintln!(
                    "skipping migration smoke test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
                );
                    return Ok(());
                }
            };

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .with_context(|| format!("failed to connect to '{database_url}'"))?;

        MIGRATOR
            .run(&pool)
            .await
            .with_context(|| "running SQLx migrations failed")?;
        Ok(())
    }
}
