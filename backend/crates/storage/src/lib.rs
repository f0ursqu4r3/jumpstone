//! Storage helpers for Postgres access.

use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    Ok(pool)
}

pub fn validate_database_url(database_url: &str) -> Result<()> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(database_url)?;
    Ok(())
}
