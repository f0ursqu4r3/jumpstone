use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::StoragePool;

#[derive(Debug, Clone)]
pub struct DeviceMetadata {
    pub device_id: String,
    pub device_name: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

impl DeviceMetadata {
    pub fn new(
        device_id: impl Into<String>,
        device_name: Option<impl Into<String>>,
        user_agent: Option<impl Into<String>>,
        ip_address: Option<impl Into<String>>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: device_name.map(Into::into),
            user_agent: user_agent.map(Into::into),
            ip_address: ip_address.map(Into::into),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewRefreshSession {
    pub refresh_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub metadata: DeviceMetadata,
}

#[derive(Debug, Clone, FromRow)]
pub struct RefreshSessionRecord {
    pub refresh_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub device_id: String,
    pub device_name: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct RefreshSessionStore {
    pool: StoragePool,
}

impl RefreshSessionStore {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, session: &NewRefreshSession) -> Result<RefreshSessionRecord> {
        let metadata = &session.metadata;
        let record = sqlx::query_as::<_, RefreshSessionRecord>(
            r#"
            INSERT INTO refresh_sessions (
                refresh_id,
                user_id,
                session_id,
                device_id,
                device_name,
                user_agent,
                ip_address,
                created_at,
                last_used_at,
                expires_at,
                revoked_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)
            ON CONFLICT (user_id, device_id) DO UPDATE
            SET refresh_id = EXCLUDED.refresh_id,
                session_id = EXCLUDED.session_id,
                device_name = EXCLUDED.device_name,
                user_agent = EXCLUDED.user_agent,
                ip_address = EXCLUDED.ip_address,
                last_used_at = EXCLUDED.last_used_at,
                expires_at = EXCLUDED.expires_at,
                revoked_at = NULL
             RETURNING refresh_id,
                       user_id,
                       session_id,
                       device_id,
                       device_name,
                       user_agent,
                       ip_address,
                       created_at,
                       last_used_at,
                       expires_at,
                       revoked_at
            "#,
        )
        .bind(session.refresh_id)
        .bind(session.user_id)
        .bind(session.session_id)
        .bind(&metadata.device_id)
        .bind(metadata.device_name.as_deref())
        .bind(metadata.user_agent.as_deref())
        .bind(metadata.ip_address.as_deref())
        .bind(session.issued_at)
        .bind(session.issued_at)
        .bind(session.expires_at)
        .fetch_one(self.pool.pool())
        .await?;

        Ok(record)
    }

    pub async fn record_use(&self, refresh_id: Uuid, used_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE refresh_sessions
            SET last_used_at = $2
            WHERE refresh_id = $1
            "#,
        )
        .bind(refresh_id)
        .bind(used_at)
        .execute(self.pool.pool())
        .await?;
        Ok(())
    }

    pub async fn revoke(&self, refresh_id: Uuid, revoked_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE refresh_sessions
            SET revoked_at = $2
            WHERE refresh_id = $1
            "#,
        )
        .bind(refresh_id)
        .bind(revoked_at)
        .execute(self.pool.pool())
        .await?;
        Ok(())
    }

    pub async fn find(&self, refresh_id: Uuid) -> Result<Option<RefreshSessionRecord>> {
        let record = sqlx::query_as::<_, RefreshSessionRecord>(
            r#"
            SELECT refresh_id,
                   user_id,
                   session_id,
                   device_id,
                   device_name,
                   user_agent,
                   ip_address,
                   created_at,
                   last_used_at,
                   revoked_at
            FROM refresh_sessions
            WHERE refresh_id = $1
            "#,
        )
        .bind(refresh_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(record)
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<RefreshSessionRecord>> {
        let records = sqlx::query_as::<_, RefreshSessionRecord>(
            r#"
            SELECT refresh_id,
                   user_id,
                   session_id,
                   device_id,
                   device_name,
                   user_agent,
                   ip_address,
                   created_at,
                   last_used_at,
                   revoked_at
            FROM refresh_sessions
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect;
    use anyhow::Context;
    use std::env;

    #[tokio::test]
    async fn upsert_and_revoke_refresh_session() -> anyhow::Result<()> {
        let database_url = match env::var("OPENGUILD_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
        {
            Ok(url) => url,
            Err(_) => {
                eprintln!(
                    "skipping refresh session persistence test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
                );
                return Ok(());
            }
        };

        let pool = connect(&database_url).await?;
        let store = RefreshSessionStore::new(pool.clone());
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let refresh_id = Uuid::new_v4();
        let issued_at = Utc::now();
        let expires_at = issued_at + chrono::Duration::days(30);

        // Ensure the user exists when foreign keys are enforced.
        sqlx::query(
            r#"
            INSERT INTO users (user_id, username, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $4)
            ON CONFLICT (user_id) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(format!("refresh_test_{user_id}"))
        .bind("argon2id$test")
        .bind(issued_at)
        .execute(pool.pool())
        .await
        .with_context(|| "failed to insert test user")?;

        let metadata = DeviceMetadata::new(
            "device-123",
            Some("Firefox on macOS"),
            Some("Mozilla/5.0"),
            Some("127.0.0.1"),
        );
        let new_session = NewRefreshSession {
            refresh_id,
            user_id,
            session_id,
            issued_at,
            expires_at,
            metadata,
        };

        let stored = store.upsert(&new_session).await?;
        assert_eq!(stored.refresh_id, refresh_id);
        assert_eq!(stored.user_id, user_id);
        assert_eq!(stored.session_id, session_id);
        assert_eq!(stored.device_id, "device-123");
        assert_eq!(stored.user_agent.as_deref(), Some("Mozilla/5.0"));
        assert_eq!(stored.expires_at, expires_at);
        assert!(stored.revoked_at.is_none());

        let fetched = store.find(refresh_id).await?.expect("record exists");
        assert_eq!(fetched.refresh_id, refresh_id);
        assert_eq!(fetched.expires_at, expires_at);

        let later = issued_at + chrono::Duration::minutes(5);
        store.record_use(refresh_id, later).await?;
        let updated = store.find(refresh_id).await?.expect("record exists");
        assert_eq!(updated.last_used_at, later);
        assert_eq!(updated.expires_at, expires_at);

        store.revoke(refresh_id, later).await?;
        let revoked = store.find(refresh_id).await?.expect("record exists");
        assert!(revoked.revoked_at.is_some());
        assert_eq!(revoked.expires_at, expires_at);

        let sessions = store.list_for_user(user_id).await?;
        assert!(!sessions.is_empty());

        sqlx::query("DELETE FROM refresh_sessions WHERE refresh_id = $1")
            .bind(refresh_id)
            .execute(pool.pool())
            .await?;
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(user_id)
            .execute(pool.pool())
            .await?;

        Ok(())
    }
}
