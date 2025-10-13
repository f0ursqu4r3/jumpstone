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

impl UserRepository {
    /// Create a new user with a hashed password.
    pub async fn create_user(pool: &PgPool, username: &str, password: &str) -> Result<Uuid> {
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
        .with_context(|| format!("creating user '{username}'"))?;

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
