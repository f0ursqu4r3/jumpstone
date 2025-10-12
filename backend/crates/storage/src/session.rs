use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::StoragePool;
use anyhow::Result;

#[derive(Clone)]
pub struct SessionPersistence {
    pool: StoragePool,
}

#[derive(Debug, Clone)]
pub struct PersistedSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SessionPersistence {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn store_session(&self, session: &PersistedSession) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (session_id, user_id, issued_at, expires_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (session_id) DO UPDATE
            SET user_id = EXCLUDED.user_id,
                issued_at = EXCLUDED.issued_at,
                expires_at = EXCLUDED.expires_at
            "#,
        )
        .bind(session.session_id)
        .bind(session.user_id)
        .bind(session.issued_at)
        .bind(session.expires_at)
        .execute(self.pool.pool())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect;
    use anyhow::Context;
    use sqlx::Row;
    use std::env;

    #[tokio::test]
    async fn stores_session_when_database_available() -> anyhow::Result<()> {
        let database_url = match env::var("OPENGUILD_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
        {
            Ok(url) => url,
            Err(_) => {
                eprintln!(
                    "skipping session persistence test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
                );
                return Ok(());
            }
        };

        let pool = connect(&database_url).await?;
        let persistence = SessionPersistence::new(pool.clone());

        let session = PersistedSession {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            issued_at: Utc::now(),
            expires_at: Utc::now(),
        };

        persistence.store_session(&session).await?;

        let stored = sqlx::query(
            r#"SELECT user_id, issued_at, expires_at FROM sessions WHERE session_id = $1"#,
        )
        .bind(session.session_id)
        .fetch_one(pool.pool())
        .await
        .with_context(|| "expected session row to exist")?;

        let user_id: Uuid = stored.try_get("user_id")?;
        let issued_at: DateTime<Utc> = stored.try_get("issued_at")?;
        let expires_at: DateTime<Utc> = stored.try_get("expires_at")?;

        assert_eq!(user_id, session.user_id);
        assert_eq!(issued_at, session.issued_at);
        assert_eq!(expires_at, session.expires_at);

        sqlx::query(r#"DELETE FROM sessions WHERE session_id = $1"#)
            .bind(session.session_id)
            .execute(pool.pool())
            .await?;

        Ok(())
    }
}
