use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::StoragePool;

#[derive(Clone)]
pub struct MessagingRepository {
    pool: StoragePool,
}

#[derive(Debug, Clone, FromRow)]
pub struct Guild {
    pub guild_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Channel {
    pub channel_id: Uuid,
    pub guild_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChannelEvent {
    pub sequence: i64,
    pub channel_id: Uuid,
    pub event_id: String,
    pub event_type: String,
    pub body: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl MessagingRepository {
    pub fn new(pool: StoragePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    pub async fn create_guild(&self, name: &str) -> Result<Guild> {
        let guild = sqlx::query_as::<_, Guild>(
            r#"
            INSERT INTO guilds (name)
            VALUES ($1)
            RETURNING guild_id, name, created_at
            "#,
        )
        .bind(name)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(guild)
    }

    pub async fn list_guilds(&self) -> Result<Vec<Guild>> {
        let guilds = sqlx::query_as::<_, Guild>(
            r#"
            SELECT guild_id, name, created_at
            FROM guilds
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;
        Ok(guilds)
    }

    pub async fn create_channel(&self, guild_id: Uuid, name: &str) -> Result<Channel> {
        let channel = sqlx::query_as::<_, Channel>(
            r#"
            INSERT INTO channels (guild_id, name)
            VALUES ($1, $2)
            RETURNING channel_id, guild_id, name, created_at
            "#,
        )
        .bind(guild_id)
        .bind(name)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(channel)
    }

    pub async fn list_channels_for_guild(&self, guild_id: Uuid) -> Result<Vec<Channel>> {
        let channels = sqlx::query_as::<_, Channel>(
            r#"
            SELECT channel_id, guild_id, name, created_at
            FROM channels
            WHERE guild_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(guild_id)
        .fetch_all(self.pool.pool())
        .await?;
        Ok(channels)
    }

    pub async fn guild_exists(&self, guild_id: Uuid) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM guilds WHERE guild_id = $1
            )
            "#,
        )
        .bind(guild_id)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(exists)
    }

    pub async fn channel_exists(&self, channel_id: Uuid) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM channels WHERE channel_id = $1
            )
            "#,
        )
        .bind(channel_id)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(exists)
    }

    pub async fn append_event(
        &self,
        channel_id: Uuid,
        event_id: &str,
        event_type: &str,
        body: &serde_json::Value,
    ) -> Result<ChannelEvent> {
        let event = sqlx::query_as::<_, ChannelEvent>(
            r#"
            INSERT INTO channel_events (channel_id, event_id, event_type, body)
            VALUES ($1, $2, $3, $4)
            RETURNING sequence, channel_id, event_id, event_type, body, created_at
            "#,
        )
        .bind(channel_id)
        .bind(event_id)
        .bind(event_type)
        .bind(body.clone())
        .fetch_one(self.pool.pool())
        .await?;
        Ok(event)
    }

    pub async fn recent_events(
        &self,
        channel_id: Uuid,
        since_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChannelEvent>> {
        let events = if let Some(seq) = since_sequence {
            sqlx::query_as::<_, ChannelEvent>(
                r#"
                SELECT sequence, channel_id, event_id, event_type, body, created_at
                FROM channel_events
                WHERE channel_id = $1 AND sequence > $2
                ORDER BY sequence ASC
                LIMIT $3
                "#,
            )
            .bind(channel_id)
            .bind(seq)
            .bind(limit)
            .fetch_all(self.pool.pool())
            .await?
        } else {
            sqlx::query_as::<_, ChannelEvent>(
                r#"
                SELECT sequence, channel_id, event_id, event_type, body, created_at
                FROM channel_events
                WHERE channel_id = $1
                ORDER BY sequence DESC
                LIMIT $2
                "#,
            )
            .bind(channel_id)
            .bind(limit)
            .fetch_all(self.pool.pool())
            .await?
            .into_iter()
            .rev()
            .collect()
        };

        Ok(events)
    }
}
