use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Insertable MLS key package payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMlsKeyPackage {
    pub identity: String,
    pub ciphersuite: String,
    pub signing_key: String,
    pub signature_key: String,
    pub hpke_public_key: String,
}

/// Persisted MLS key package record.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct MlsKeyPackageRecord {
    pub id: Uuid,
    pub identity: String,
    pub ciphersuite: String,
    pub signing_key: String,
    pub signature_key: String,
    pub hpke_public_key: String,
    pub created_at: DateTime<Utc>,
}

/// Repository for reading/writing MLS key packages.
#[derive(Clone)]
pub struct MlsKeyPackageStore {
    pool: Arc<PgPool>,
}

impl MlsKeyPackageStore {
    /// Wrap a Postgres pool for MLS key package persistence.
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Persist a key package rotation and return the stored record.
    pub async fn record_package(&self, package: &NewMlsKeyPackage) -> Result<MlsKeyPackageRecord> {
        let id = Uuid::new_v4();
        let record = sqlx::query_as::<_, MlsKeyPackageRecord>(
            r#"
            INSERT INTO mls_key_packages (id, identity, ciphersuite, signing_key, signature_key, hpke_public_key)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, identity, ciphersuite, signing_key, signature_key, hpke_public_key, created_at
            "#,
        )
        .bind(id)
        .bind(&package.identity)
        .bind(&package.ciphersuite)
        .bind(&package.signing_key)
        .bind(&package.signature_key)
        .bind(&package.hpke_public_key)
        .fetch_one(self.pool())
        .await?;

        Ok(record)
    }

    /// Fetch the most recent key package for each identity.
    pub async fn latest_packages(&self) -> Result<Vec<MlsKeyPackageRecord>> {
        let mut records = sqlx::query_as::<_, MlsKeyPackageRecord>(
            r#"
            SELECT DISTINCT ON (identity)
                id,
                identity,
                ciphersuite,
                signing_key,
                signature_key,
                hpke_public_key,
                created_at
            FROM mls_key_packages
            ORDER BY identity, created_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        records.sort_by(|a, b| a.identity.cmp(&b.identity));
        Ok(records)
    }

    /// Fetch the most recent key package for a specific identity.
    pub async fn latest_for_identity(&self, identity: &str) -> Result<Option<MlsKeyPackageRecord>> {
        let record = sqlx::query_as::<_, MlsKeyPackageRecord>(
            r#"
            SELECT
                id,
                identity,
                ciphersuite,
                signing_key,
                signature_key,
                hpke_public_key,
                created_at
            FROM mls_key_packages
            WHERE identity = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(identity)
        .fetch_optional(self.pool())
        .await?;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect;
    use sqlx::migrate::Migrator;
    use std::env;

    static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

    async fn setup_store() -> anyhow::Result<Option<MlsKeyPackageStore>> {
        let database_url = match env::var("OPENGUILD_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
        {
            Ok(url) => url,
            Err(_) => {
                eprintln!(
                    "skipping MLS key package store test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
                );
                return Ok(None);
            }
        };

        let pool = connect(&database_url).await?;
        MIGRATOR
            .run(pool.pool())
            .await
            .expect("running migrations for MLS key package store");

        Ok(Some(MlsKeyPackageStore::new(pool.cloned())))
    }

    fn sample_new(identity: &str) -> NewMlsKeyPackage {
        NewMlsKeyPackage {
            identity: identity.into(),
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".into(),
            signing_key: "signing-key".into(),
            signature_key: "verifying-key".into(),
            hpke_public_key: "hpke".into(),
        }
    }

    #[tokio::test]
    async fn record_and_fetch_latest_packages() -> anyhow::Result<()> {
        let Some(store) = setup_store().await? else {
            return Ok(());
        };

        // Record two identities with multiple entries to ensure ordering.
        store
            .record_package(&sample_new("alice"))
            .await
            .expect("alice stored");
        store
            .record_package(&sample_new("bob"))
            .await
            .expect("bob stored");

        let mut newer = sample_new("alice");
        newer.signing_key = "rotated".into();
        store.record_package(&newer).await.expect("alice rotated");

        let records = store.latest_packages().await?;
        assert_eq!(records.len(), 2);
        let alice = records
            .iter()
            .find(|pkg| pkg.identity == "alice")
            .expect("alice present");
        assert_eq!(alice.signing_key, "rotated");

        let latest_alice = store
            .latest_for_identity("alice")
            .await?
            .expect("latest alice exists");
        assert_eq!(latest_alice.signing_key, "rotated");

        let missing = store.latest_for_identity("carol").await?;
        assert!(missing.is_none());

        Ok(())
    }
}
