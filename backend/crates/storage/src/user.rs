use anyhow::{anyhow, Context, Result};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
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

impl UserRepository {
    /// Create a new user with a hashed password.
    pub async fn create_user(
        pool: &PgPool,
        username: &str,
        password: &str,
    ) -> Result<Uuid, CreateUserError> {
        let id = Uuid::new_v4();
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| anyhow!("hashing password failed: {err}"))?
            .to_string();

        sqlx::query(
            r#"
            INSERT INTO users (id, username, password_hash)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(id)
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

        Ok(id)
    }

    /// Verify credentials and return the user id when successful.
    pub async fn verify_credentials(pool: &PgPool, username: &str, password: &str) -> Result<Uuid> {
        let record = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT id, password_hash
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
}
