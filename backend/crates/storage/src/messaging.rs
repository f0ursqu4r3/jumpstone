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

#[derive(Debug, Clone, FromRow)]
pub struct GuildMembership {
    pub guild_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GuildMembershipSummary {
    pub guild_id: Uuid,
    #[sqlx(rename = "guild_name")]
    pub guild_name: String,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChannelMembership {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChannelMembershipSummary {
    pub channel_id: Uuid,
    pub guild_id: Uuid,
    #[sqlx(rename = "channel_name")]
    pub channel_name: String,
    pub role: String,
    pub joined_at: DateTime<Utc>,
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

    pub async fn user_ids_for_channel(&self, channel_id: Uuid) -> Result<Vec<Uuid>> {
        let ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT user_id
            FROM channel_memberships
            WHERE channel_id = $1
            "#,
        )
        .bind(channel_id)
        .fetch_all(self.pool.pool())
        .await?;
        Ok(ids)
    }

    pub async fn user_ids_for_guild(&self, guild_id: Uuid) -> Result<Vec<Uuid>> {
        let ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT user_id
            FROM guild_memberships
            WHERE guild_id = $1
            "#,
        )
        .bind(guild_id)
        .fetch_all(self.pool.pool())
        .await?;
        Ok(ids)
    }

    pub async fn upsert_guild_membership(
        &self,
        guild_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<GuildMembership> {
        let membership = sqlx::query_as::<_, GuildMembership>(
            r#"
            INSERT INTO guild_memberships (guild_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (guild_id, user_id)
            DO UPDATE SET role = EXCLUDED.role
            RETURNING guild_id, user_id, role, joined_at
            "#,
        )
        .bind(guild_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(membership)
    }

    pub async fn guild_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<GuildMembershipSummary>> {
        let memberships = sqlx::query_as::<_, GuildMembershipSummary>(
            r#"
            SELECT gm.guild_id,
                   g.name AS guild_name,
                   gm.role,
                   gm.joined_at
            FROM guild_memberships gm
            JOIN guilds g ON g.guild_id = gm.guild_id
            WHERE gm.user_id = $1
            ORDER BY g.created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;
        Ok(memberships)
    }

    pub async fn upsert_channel_membership(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<ChannelMembership> {
        let membership = sqlx::query_as::<_, ChannelMembership>(
            r#"
            INSERT INTO channel_memberships (channel_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (channel_id, user_id)
            DO UPDATE SET role = EXCLUDED.role
            RETURNING channel_id, user_id, role, joined_at
            "#,
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(self.pool.pool())
        .await?;
        Ok(membership)
    }

    pub async fn channel_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ChannelMembershipSummary>> {
        let memberships = sqlx::query_as::<_, ChannelMembershipSummary>(
            r#"
            SELECT cm.channel_id,
                   c.guild_id,
                   c.name AS channel_name,
                   cm.role,
                   cm.joined_at
            FROM channel_memberships cm
            JOIN channels c ON c.channel_id = cm.channel_id
            WHERE cm.user_id = $1
            ORDER BY cm.joined_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;
        Ok(memberships)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, StoragePool};
    use anyhow::Context;
    use serde_json::json;
    use sqlx::migrate::Migrator;
    use std::env;

    static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

    fn test_database_url() -> Option<String> {
        env::var("OPENGUILD_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .ok()
    }

    async fn truncate_tables(pool: &StoragePool) -> anyhow::Result<()> {
        sqlx::query("TRUNCATE channel_events, channel_memberships, guild_memberships, channels, guilds RESTART IDENTITY CASCADE")
            .execute(pool.pool())
            .await
            .map(|_| ())
            .context("failed to truncate messaging tables")
    }

    #[tokio::test]
    async fn messaging_repository_persists_entities_and_events() -> anyhow::Result<()> {
        let Some(database_url) = test_database_url() else {
            eprintln!("skipping messaging repository test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL");
            return Ok(());
        };

        let pool = connect(&database_url).await?;
        MIGRATOR
            .run(pool.pool())
            .await
            .context("running migrations for messaging repository tests failed")?;

        let repo = MessagingRepository::new(pool.clone());

        let alpha = repo.create_guild("Alpha").await?;
        let beta = repo.create_guild("Beta").await?;

        let guilds = repo.list_guilds().await?;
        assert!(guilds.iter().any(|g| g.guild_id == alpha.guild_id));
        assert!(guilds.iter().any(|g| g.guild_id == beta.guild_id));

        assert!(repo.guild_exists(alpha.guild_id).await?);
        assert!(!repo.guild_exists(Uuid::new_v4()).await?);

        let general = repo.create_channel(alpha.guild_id, "general").await?;
        let support = repo.create_channel(alpha.guild_id, "support").await?;

        let channels = repo.list_channels_for_guild(alpha.guild_id).await?;
        let channel_ids: Vec<_> = channels.iter().map(|c| c.channel_id).collect();
        assert!(channel_ids.contains(&general.channel_id));
        assert!(channel_ids.contains(&support.channel_id));

        assert!(repo.channel_exists(general.channel_id).await?);
        assert!(!repo.channel_exists(Uuid::new_v4()).await?);

        let first_event_id = format!("evt-{}", Uuid::new_v4());
        let second_event_id = format!("evt-{}", Uuid::new_v4());
        let payload = json!({ "content": "hello world" });

        let first = repo
            .append_event(general.channel_id, &first_event_id, "message", &payload)
            .await?;
        repo.append_event(general.channel_id, &second_event_id, "message", &payload)
            .await?;

        let all_events = repo
            .recent_events(general.channel_id, None, 10)
            .await
            .context("fetching recent events without sequence should succeed")?;
        assert_eq!(all_events.len(), 2, "expected two events in channel");
        assert_eq!(all_events[0].event_id, first_event_id);
        assert_eq!(all_events[1].event_id, second_event_id);

        let limited = repo.recent_events(general.channel_id, None, 1).await?;
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].event_id, second_event_id);

        let from_sequence = repo
            .recent_events(general.channel_id, Some(first.sequence), 10)
            .await?;
        assert_eq!(from_sequence.len(), 1);
        assert_eq!(from_sequence[0].event_id, second_event_id);

        truncate_tables(&pool).await?;
        Ok(())
    }

    #[tokio::test]
    async fn messaging_repository_tracks_memberships() -> anyhow::Result<()> {
        let Some(database_url) = test_database_url() else {
            eprintln!("skipping messaging repository test: set OPENGUILD_TEST_DATABASE_URL or DATABASE_URL");
            return Ok(());
        };

        let pool = connect(&database_url).await?;
        MIGRATOR
            .run(pool.pool())
            .await
            .context("running migrations for membership repository tests failed")?;

        let repo = MessagingRepository::new(pool.clone());
        truncate_tables(&pool).await?;

        let guild = repo.create_guild("Members").await?;
        let channel = repo.create_channel(guild.guild_id, "general").await?;
        let user_id = Uuid::new_v4();

        let guild_membership = repo
            .upsert_guild_membership(guild.guild_id, user_id, "admin")
            .await?;
        assert_eq!(guild_membership.role, "admin");

        let guilds = repo.guild_memberships_for_user(user_id).await?;
        assert_eq!(guilds.len(), 1);
        assert_eq!(guilds[0].guild_name, "Members");
        assert_eq!(guilds[0].role, "admin");

        let channel_membership = repo
            .upsert_channel_membership(channel.channel_id, user_id, "moderator")
            .await?;
        assert_eq!(channel_membership.role, "moderator");

        let channels = repo.channel_memberships_for_user(user_id).await?;
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].channel_name, "general");
        assert_eq!(channels[0].role, "moderator");

        truncate_tables(&pool).await?;
        Ok(())
    }
}
