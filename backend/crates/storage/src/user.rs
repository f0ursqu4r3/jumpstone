use anyhow::{anyhow, Context, Result};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use pwhash::rand_core::OsRng;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

/// Repository utilities for user persistence.
pub struct UserRepository;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("user not found")]
    UserNotFound,
    #[error("invalid credentials")]
    InvalidCredentials,
}

#[derive(Debug, Error)]
pub enum CreateUserError {
    #[error("username already exists")]
    UsernameTaken,
    #[error("failed to create user: {0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub user_id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserRepository {
    /// Create a new user with a hashed password.
    pub async fn create_user(
        pool: &PgPool,
        username: &str,
        password: &str,
    ) -> Result<Uuid, CreateUserError> {
        let user_id = Uuid::new_v4();
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| anyhow!("hashing password failed: {err}"))?
            .to_string();

        sqlx::query(
            r#"
            INSERT INTO users (user_id, username, password_hash)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(username)
        .bind(password_hash)
        .execute(pool)
        .await
        .map_err(|err| match err {
            sqlx::Error::Database(db_err) if matches!(db_err.code(), Some(code) if code.as_ref() == "23505") => {
                CreateUserError::UsernameTaken
            }
            other => CreateUserError::Other(
                anyhow!(other).context(format!("creating user '{username}'")),
            ),
        })?;

        Ok(user_id)
    }

    /// Verify credentials and return the user id when successful.
    pub async fn verify_credentials(pool: &PgPool, username: &str, password: &str) -> Result<Uuid> {
        let record = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT user_id, password_hash
            FROM users
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("querying user '{username}'"))?;

        let Some((user_id, password_hash)) = record else {
            return Err(CredentialError::UserNotFound.into());
        };

        let parsed_hash = PasswordHash::new(&password_hash)
            .map_err(|err| anyhow!("invalid password hash for '{username}': {err}"))?;

        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| CredentialError::InvalidCredentials)?;

        Ok(user_id)
    }

    pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<UserRecord>> {
        let record = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>, DateTime<Utc>)>(
            r#"
            SELECT user_id, username, created_at, updated_at
            FROM users
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("fetching user '{user_id}'"))?;

        Ok(record.map(|row| UserRecord {
            user_id: row.0,
            username: row.1,
            created_at: row.2,
            updated_at: row.3,
        }))
    }

    pub async fn upsert_role(pool: &PgPool, user_id: Uuid, role: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role)
            VALUES ($1, $2)
            ON CONFLICT (user_id, role) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .with_context(|| format!("granting role '{role}' to user '{user_id}'"))?;
        Ok(())
    }

    pub async fn revoke_role(pool: &PgPool, user_id: Uuid, role: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_roles
            WHERE user_id = $1 AND role = $2
            "#,
        )
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .with_context(|| format!("revoking role '{role}' from user '{user_id}'"))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_roles(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>> {
        let roles = sqlx::query_scalar::<_, String>(
            r#"
            SELECT role
            FROM user_roles
            WHERE user_id = $1
            ORDER BY granted_at ASC, role ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .with_context(|| format!("listing roles for user '{user_id}'"))?;
        Ok(roles)
    }

    pub async fn find_user_id_by_username(pool: &PgPool, username: &str) -> Result<Option<Uuid>> {
        let user_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT user_id
            FROM users
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("locating user '{username}'"))?;
        Ok(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect;
    use sqlx::migrate::Migrator;
    use std::env;

    static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

    fn test_database_url() -> Option<String> {
        env::var("OPENGUILD_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .ok()
    }

    async fn setup_pool() -> anyhow::Result<Option<crate::StoragePool>> {
        let Some(database_url) = test_database_url() else {
            return Ok(None);
        };

        let pool = connect(&database_url).await?;
        MIGRATOR
            .run(pool.pool())
            .await
            .map(|_| ())
            .map_err(|err| anyhow!(err).context("running user repository migrations failed"))?;
        Ok(Some(pool))
    }

    #[tokio::test]
    async fn create_user_persists_and_verifies_credentials() -> anyhow::Result<()> {
        let Some(pool) = setup_pool().await? else {
            eprintln!(
                "skipping user repository test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
            );
            return Ok(());
        };

        let username = format!("user_{}", Uuid::new_v4());
        let password = "hunter2!";
        let wrong_password = "password123";

        let user_id = UserRepository::create_user(pool.pool(), &username, password).await?;
        let verified_id =
            UserRepository::verify_credentials(pool.pool(), &username, password).await?;
        assert_eq!(verified_id, user_id);

        let err = UserRepository::verify_credentials(pool.pool(), &username, wrong_password)
            .await
            .expect_err("verification with wrong password should fail");
        let cred_err = err
            .downcast::<CredentialError>()
            .expect("error converts to CredentialError");
        assert!(matches!(cred_err, CredentialError::InvalidCredentials));

        let err = UserRepository::verify_credentials(pool.pool(), "missing_user", password)
            .await
            .expect_err("missing user should not authenticate");
        let cred_err = err
            .downcast::<CredentialError>()
            .expect("error converts to CredentialError");
        assert!(matches!(cred_err, CredentialError::UserNotFound));

        sqlx::query("DELETE FROM users WHERE username = $1")
            .bind(username)
            .execute(pool.pool())
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_usernames() -> anyhow::Result<()> {
        let Some(pool) = setup_pool().await? else {
            eprintln!(
                "skipping user repository test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
            );
            return Ok(());
        };

        let username = format!("dupe_{}", Uuid::new_v4());
        let password = "correct horse battery staple";

        UserRepository::create_user(pool.pool(), &username, password).await?;

        let err = UserRepository::create_user(pool.pool(), &username, password)
            .await
            .expect_err("creating user with duplicate username should fail");
        assert!(matches!(err, CreateUserError::UsernameTaken));

        sqlx::query("DELETE FROM users WHERE username = $1")
            .bind(username)
            .execute(pool.pool())
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn user_roles_round_trip() -> anyhow::Result<()> {
        let Some(pool) = setup_pool().await? else {
            eprintln!(
                "skipping user repository test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL"
            );
            return Ok(());
        };

        let username = format!("role_{}", Uuid::new_v4());
        let password = "role-test-secret";
        let user_id = UserRepository::create_user(pool.pool(), &username, password).await?;

        UserRepository::upsert_role(pool.pool(), user_id, "admin").await?;
        UserRepository::upsert_role(pool.pool(), user_id, "maintainer").await?;
        // duplicate grant should be ignored
        UserRepository::upsert_role(pool.pool(), user_id, "admin").await?;

        let mut roles = UserRepository::list_roles(pool.pool(), user_id).await?;
        roles.sort();
        assert_eq!(roles, vec!["admin".to_string(), "maintainer".to_string()]);

        let removed = UserRepository::revoke_role(pool.pool(), user_id, "admin").await?;
        assert!(removed, "expected admin role to be removed");
        let roles = UserRepository::list_roles(pool.pool(), user_id).await?;
        assert_eq!(roles, vec!["maintainer".to_string()]);

        sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
            .bind(user_id)
            .execute(pool.pool())
            .await?;
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(user_id)
            .execute(pool.pool())
            .await?;

        Ok(())
    }
}
