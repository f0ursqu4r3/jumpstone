use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        MatchedPath, Path, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    Extension, Json,
};
use openguild_core::{event::CanonicalEvent, messaging::MessagePayload};
use openguild_storage::{Channel, ChannelEvent, Guild, MessagingRepository, StoragePool};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    sync::{broadcast, Mutex, RwLock, Semaphore},
    time::timeout,
};
use uuid::Uuid;

#[cfg(feature = "metrics")]
use crate::metrics::MetricsContext;
use crate::{config::ServerConfig, session, AppState};
use tower_http::request_id::RequestId;
use tracing::Instrument;

const BROADCAST_CAPACITY: usize = 256;
const MAX_WS_CONNECTIONS: usize = 256;
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_GUILD_NAME_LENGTH: usize = 64;
const MAX_CHANNEL_NAME_LENGTH: usize = 64;
const MAX_MESSAGE_LENGTH: usize = 4000;
pub(crate) const MESSAGE_RATE_WINDOW: Duration = Duration::from_secs(60);
pub(crate) const DEFAULT_TIMELINE_LIMIT: i64 = 50;
pub(crate) const MAX_TIMELINE_LIMIT: i64 = 200;
#[cfg(test)]
const MAX_MESSAGES_PER_USER_PER_WINDOW: usize = 3;
#[cfg(not(test))]
const MAX_MESSAGES_PER_USER_PER_WINDOW: usize = 60;
#[cfg(test)]
const MAX_MESSAGES_PER_IP_PER_WINDOW: usize = 5;
#[cfg(not(test))]
const MAX_MESSAGES_PER_IP_PER_WINDOW: usize = 200;

#[cfg(test)]
pub(crate) const TEST_MAX_MESSAGES_PER_IP_PER_WINDOW: usize = MAX_MESSAGES_PER_IP_PER_WINDOW;

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("guild not found")]
    GuildNotFound,
    #[error("channel not found")]
    ChannelNotFound,
    #[error("invalid room id '{0}'")]
    InvalidRoomId(String),
    #[error("storage error: {0}")]
    Storage(#[from] anyhow::Error),
}

#[async_trait]
pub trait ChannelStore: Send + Sync {
    async fn create_guild(&self, name: &str) -> Result<Guild, MessagingError>;
    async fn list_guilds(&self) -> Result<Vec<Guild>, MessagingError>;
    async fn create_channel(&self, guild_id: Uuid, name: &str) -> Result<Channel, MessagingError>;
    async fn list_channels_for_guild(&self, guild_id: Uuid)
        -> Result<Vec<Channel>, MessagingError>;
    async fn append_event(
        &self,
        channel_id: Uuid,
        event_id: &str,
        event_type: &str,
        body: &serde_json::Value,
    ) -> Result<ChannelEvent, MessagingError>;
    async fn recent_events(
        &self,
        channel_id: Uuid,
        since_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChannelEvent>, MessagingError>;
    async fn channel_exists(&self, channel_id: Uuid) -> Result<bool, MessagingError>;
}

#[async_trait]
impl ChannelStore for MessagingRepository {
    async fn create_guild(&self, name: &str) -> Result<Guild, MessagingError> {
        MessagingRepository::create_guild(self, name)
            .await
            .map_err(MessagingError::from)
    }

    async fn list_guilds(&self) -> Result<Vec<Guild>, MessagingError> {
        MessagingRepository::list_guilds(self)
            .await
            .map_err(MessagingError::from)
    }

    async fn create_channel(&self, guild_id: Uuid, name: &str) -> Result<Channel, MessagingError> {
        if !self
            .guild_exists(guild_id)
            .await
            .map_err(MessagingError::from)?
        {
            return Err(MessagingError::GuildNotFound);
        }
        MessagingRepository::create_channel(self, guild_id, name)
            .await
            .map_err(MessagingError::from)
    }

    async fn list_channels_for_guild(
        &self,
        guild_id: Uuid,
    ) -> Result<Vec<Channel>, MessagingError> {
        MessagingRepository::list_channels_for_guild(self, guild_id)
            .await
            .map_err(MessagingError::from)
    }

    async fn append_event(
        &self,
        channel_id: Uuid,
        event_id: &str,
        event_type: &str,
        body: &serde_json::Value,
    ) -> Result<ChannelEvent, MessagingError> {
        if !self
            .channel_exists(channel_id)
            .await
            .map_err(MessagingError::from)?
        {
            return Err(MessagingError::ChannelNotFound);
        }
        MessagingRepository::append_event(self, channel_id, event_id, event_type, body)
            .await
            .map_err(MessagingError::from)
    }

    async fn recent_events(
        &self,
        channel_id: Uuid,
        since_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChannelEvent>, MessagingError> {
        MessagingRepository::recent_events(self, channel_id, since_sequence, limit)
            .await
            .map_err(MessagingError::from)
    }

    async fn channel_exists(&self, channel_id: Uuid) -> Result<bool, MessagingError> {
        MessagingRepository::channel_exists(self, channel_id)
            .await
            .map_err(MessagingError::from)
    }
}

#[derive(Default)]
struct InMemoryMessaging {
    guilds: RwLock<HashMap<Uuid, Guild>>,
    channels: RwLock<HashMap<Uuid, Channel>>,
    guild_index: RwLock<HashMap<Uuid, Vec<Uuid>>>,
    events: RwLock<HashMap<Uuid, Vec<ChannelEvent>>>,
    sequence: AtomicI64,
}

#[async_trait]
impl ChannelStore for InMemoryMessaging {
    async fn create_guild(&self, name: &str) -> Result<Guild, MessagingError> {
        let guild = Guild {
            guild_id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: chrono::Utc::now(),
        };
        self.guilds
            .write()
            .await
            .insert(guild.guild_id, guild.clone());
        Ok(guild)
    }

    async fn list_guilds(&self) -> Result<Vec<Guild>, MessagingError> {
        let mut guilds: Vec<_> = self.guilds.read().await.values().cloned().collect();
        guilds.sort_by_key(|g| g.created_at);
        Ok(guilds)
    }

    async fn create_channel(&self, guild_id: Uuid, name: &str) -> Result<Channel, MessagingError> {
        if !self.guilds.read().await.contains_key(&guild_id) {
            return Err(MessagingError::GuildNotFound);
        }
        let channel = Channel {
            channel_id: Uuid::new_v4(),
            guild_id,
            name: name.to_string(),
            created_at: chrono::Utc::now(),
        };
        self.channels
            .write()
            .await
            .insert(channel.channel_id, channel.clone());
        self.guild_index
            .write()
            .await
            .entry(guild_id)
            .or_default()
            .push(channel.channel_id);
        Ok(channel)
    }

    async fn list_channels_for_guild(
        &self,
        guild_id: Uuid,
    ) -> Result<Vec<Channel>, MessagingError> {
        let channels_map = self.channels.read().await;
        let index = self.guild_index.read().await;
        let ids = index.get(&guild_id).cloned().unwrap_or_default();
        let mut channels: Vec<_> = ids
            .into_iter()
            .filter_map(|id| channels_map.get(&id).cloned())
            .collect();
        channels.sort_by_key(|c| c.created_at);
        Ok(channels)
    }

    async fn append_event(
        &self,
        channel_id: Uuid,
        event_id: &str,
        event_type: &str,
        body: &serde_json::Value,
    ) -> Result<ChannelEvent, MessagingError> {
        if !self.channels.read().await.contains_key(&channel_id) {
            return Err(MessagingError::ChannelNotFound);
        }
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let event = ChannelEvent {
            sequence,
            channel_id,
            event_id: event_id.to_string(),
            event_type: event_type.to_string(),
            body: body.clone(),
            created_at: chrono::Utc::now(),
        };
        self.events
            .write()
            .await
            .entry(channel_id)
            .or_default()
            .push(event.clone());
        Ok(event)
    }

    async fn recent_events(
        &self,
        channel_id: Uuid,
        since_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChannelEvent>, MessagingError> {
        let events_map = self.events.read().await;
        let mut events = events_map.get(&channel_id).cloned().unwrap_or_default();
        events.sort_by_key(|e| e.sequence);
        if let Some(seq) = since_sequence {
            events.retain(|e| e.sequence > seq);
        }
        if events.len() as i64 > limit {
            events = events[(events.len() - limit as usize)..].to_vec();
        }
        Ok(events)
    }

    async fn channel_exists(&self, channel_id: Uuid) -> Result<bool, MessagingError> {
        Ok(self.channels.read().await.contains_key(&channel_id))
    }
}

#[derive(Clone)]
pub struct MessagingService {
    store: Arc<dyn ChannelStore>,
    broadcasters: Arc<RwLock<HashMap<Uuid, broadcast::Sender<Arc<OutboundEvent>>>>>,
    semaphore: Arc<Semaphore>,
    message_rate_limits: Arc<RateLimiter>,
    ip_rate_limits: Arc<RateLimiter>,
    origin_server: String,
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<MetricsContext>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutboundEvent {
    pub sequence: i64,
    pub channel_id: Uuid,
    pub event: serde_json::Value,
}

impl MessagingService {
    pub fn new_with_pool(pool: StoragePool, origin_server: String) -> Self {
        let repo = MessagingRepository::new(pool);
        Self::new_internal(repo, origin_server)
    }

    pub fn new_in_memory(origin_server: String) -> Self {
        Self::new_internal(Arc::new(InMemoryMessaging::default()), origin_server)
    }

    fn new_internal(store: Arc<dyn ChannelStore>, origin_server: String) -> Self {
        Self {
            store,
            broadcasters: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(MAX_WS_CONNECTIONS)),
            message_rate_limits: Arc::new(RateLimiter::new(
                MAX_MESSAGES_PER_USER_PER_WINDOW,
                MESSAGE_RATE_WINDOW,
            )),
            ip_rate_limits: Arc::new(RateLimiter::new(
                MAX_MESSAGES_PER_IP_PER_WINDOW,
                MESSAGE_RATE_WINDOW,
            )),
            origin_server,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    #[cfg(feature = "metrics")]
    pub fn with_metrics(mut self, metrics: Option<Arc<MetricsContext>>) -> Self {
        self.metrics = metrics;
        self
    }

    #[cfg(feature = "metrics")]
    fn metrics(&self) -> Option<&Arc<MetricsContext>> {
        self.metrics.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn set_max_websocket_connections(&mut self, max: usize) {
        self.semaphore = Arc::new(Semaphore::new(max));
    }

    pub async fn check_message_rate(&self, key: &str) -> bool {
        self.message_rate_limits.check_and_increment(key).await
    }

    pub async fn check_ip_rate(&self, key: &str) -> bool {
        self.ip_rate_limits.check_and_increment(key).await
    }

    pub async fn create_guild(&self, name: &str) -> Result<Guild, MessagingError> {
        self.store.create_guild(name).await
    }

    pub async fn list_guilds(&self) -> Result<Vec<Guild>, MessagingError> {
        self.store.list_guilds().await
    }

    pub async fn create_channel(
        &self,
        guild_id: Uuid,
        name: &str,
    ) -> Result<Channel, MessagingError> {
        self.store.create_channel(guild_id, name).await
    }

    pub async fn list_channels(&self, guild_id: Uuid) -> Result<Vec<Channel>, MessagingError> {
        self.store.list_channels_for_guild(guild_id).await
    }

    pub async fn append_message(
        &self,
        channel_id: Uuid,
        sender: &str,
        content: &str,
    ) -> Result<ChannelEvent, MessagingError> {
        let payload = MessagePayload {
            content: content.to_owned(),
        };
        let event = payload.to_event(
            &self.origin_server,
            &channel_id.to_string(),
            sender,
            Vec::new(),
        );
        let body =
            serde_json::to_value(&event).map_err(|err| MessagingError::Storage(err.into()))?;
        let stored = self
            .store
            .append_event(channel_id, &event.event_id, &event.event_type, &body)
            .await?;

        let broadcast_event = Arc::new(OutboundEvent {
            sequence: stored.sequence,
            channel_id,
            event: body,
        });
        let send_result = self.broadcast(channel_id, broadcast_event).await;

        #[cfg(feature = "metrics")]
        if let Some(metrics) = self.metrics() {
            let outcome = match &send_result {
                Ok(delivered) if *delivered == 0 => "no_subscribers",
                Ok(_) => "delivered",
                Err(_) => "dropped",
            };
            metrics.increment_messaging_events(outcome);
        }

        if let Err(err) = send_result {
            tracing::warn!(?err, channel_id = %channel_id, "failed to broadcast event");
        }

        Ok(stored)
    }

    pub async fn ingest_event(
        &self,
        event: &CanonicalEvent,
    ) -> Result<ChannelEvent, MessagingError> {
        let channel_id = Uuid::parse_str(&event.room_id)
            .map_err(|_| MessagingError::InvalidRoomId(event.room_id.clone()))?;
        let body =
            serde_json::to_value(event).map_err(|err| MessagingError::Storage(err.into()))?;

        let stored = self
            .store
            .append_event(channel_id, &event.event_id, &event.event_type, &body)
            .await?;

        let broadcast_event = Arc::new(OutboundEvent {
            sequence: stored.sequence,
            channel_id,
            event: body,
        });
        let send_result = self.broadcast(channel_id, broadcast_event).await;

        #[cfg(feature = "metrics")]
        if let Some(metrics) = self.metrics() {
            let outcome = match &send_result {
                Ok(delivered) if *delivered == 0 => "no_subscribers",
                Ok(_) => "delivered",
                Err(_) => "dropped",
            };
            metrics.increment_messaging_events(outcome);
        }

        if let Err(err) = send_result {
            tracing::warn!(?err, channel_id = %channel_id, "failed to broadcast federated event");
        }

        Ok(stored)
    }

    pub async fn recent_events(
        &self,
        channel_id: Uuid,
        since_sequence: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChannelEvent>, MessagingError> {
        self.store
            .recent_events(channel_id, since_sequence, limit)
            .await
    }

    pub async fn channel_exists(&self, channel_id: Uuid) -> Result<bool, MessagingError> {
        self.store.channel_exists(channel_id).await
    }

    async fn broadcast(
        &self,
        channel_id: Uuid,
        event: Arc<OutboundEvent>,
    ) -> Result<usize, broadcast::error::SendError<Arc<OutboundEvent>>> {
        let sender = {
            let read = self.broadcasters.read().await;
            read.get(&channel_id).cloned()
        };

        let sender = match sender {
            Some(sender) => sender,
            None => {
                let mut write = self.broadcasters.write().await;
                write
                    .entry(channel_id)
                    .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
                    .clone()
            }
        };

        let result = sender.send(event);

        #[cfg(feature = "metrics")]
        if let Some(metrics) = self.metrics() {
            metrics.set_websocket_queue_depth(&channel_id.to_string(), sender.len());
        }

        result
    }

    async fn subscribe(&self, channel_id: Uuid) -> broadcast::Receiver<Arc<OutboundEvent>> {
        let mut write = self.broadcasters.write().await;
        write
            .entry(channel_id)
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
            .subscribe()
    }

    pub async fn open_websocket(
        self: Arc<Self>,
        channel_id: Uuid,
        request_id: Option<String>,
        ws: WebSocketUpgrade,
    ) -> Response {
        let request_id_label = request_id.unwrap_or_else(|| "unknown".to_string());

        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                let svc = self.clone();
                let request_id_for_span = request_id_label.clone();
                ws.on_upgrade(move |socket| {
                    let span = tracing::info_span!(
                        "websocket.session",
                        channel_id = %channel_id,
                        request_id = %request_id_for_span
                    );
                    async move {
                        svc.run_socket(channel_id, socket, permit).await;
                    }
                    .instrument(span)
                })
            }
            Err(_) => {
                #[cfg(feature = "metrics")]
                if let Some(metrics) = self.metrics() {
                    metrics.increment_messaging_rejection("websocket_limit");
                }
                tracing::warn!(
                    channel_id = %channel_id,
                    request_id = %request_id_label,
                    "websocket connection limit reached"
                );
                Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body(axum::body::Body::from("connection limit reached"))
                    .unwrap()
            }
        }
    }

    async fn run_socket(
        self: Arc<Self>,
        channel_id: Uuid,
        mut socket: WebSocket,
        _permit: tokio::sync::OwnedSemaphorePermit,
    ) {
        if let Ok(events) = self.store.recent_events(channel_id, None, 50).await {
            for event in events {
                let payload = Arc::new(OutboundEvent {
                    sequence: event.sequence,
                    channel_id,
                    event: event.body,
                });
                if let Err(err) = timeout(
                    SEND_TIMEOUT,
                    socket.send(WsMessage::Text(
                        serde_json::to_string(&*payload).unwrap_or_default().into(),
                    )),
                )
                .await
                {
                    tracing::warn!(?err, "failed to send historical event");
                    return;
                }
            }
        }

        let mut rx = self.subscribe(channel_id).await;

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            if timeout(
                                SEND_TIMEOUT,
                                socket.send(WsMessage::Text(serde_json::to_string(&*event).unwrap_or_default().into()))
                            ).await.is_err() {
                                tracing::warn!("websocket send timeout");
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            let message = format!("lagged by {skipped} messages");
                            let _ = socket.send(WsMessage::Close(Some(axum::extract::ws::CloseFrame {
                                code: axum::extract::ws::close_code::POLICY,
                                reason: message.into(),
                            }))).await;
                            break;
                        }
                        Err(_) => break,
                    }
                }
                message = socket.recv() => {
                    match message {
                        Some(Ok(WsMessage::Close(_))) | None => break,
                        Some(Ok(WsMessage::Ping(payload))) => {
                            if socket.send(WsMessage::Pong(payload)).await.is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateGuildRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateGuildResponse {
    pub guild_id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateChannelResponse {
    pub channel_id: Uuid,
    pub guild_id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PostMessageRequest {
    pub sender: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PostMessageResponse {
    pub sequence: i64,
    pub event_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_guild(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateGuildRequest>,
) -> Result<Json<CreateGuildResponse>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    let name = body.name.trim();
    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let name_length = name.chars().count();

    if name.is_empty() {
        state.record_messaging_rejection("guild_name_empty");
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    if name_length > MAX_GUILD_NAME_LENGTH {
        state.record_messaging_rejection("guild_name_length");
        tracing::warn!(
            length = name_length,
            max = MAX_GUILD_NAME_LENGTH,
            "guild name exceeds maximum length"
        );
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    match messaging.create_guild(name).await {
        Ok(guild) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(CreateGuildResponse {
                guild_id: guild.guild_id,
                name: guild.name,
                created_at: guild.created_at,
            }))
        }
        Err(err) => {
            tracing::error!(?err, "failed to create guild");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

pub async fn list_guilds(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CreateGuildResponse>>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    match messaging.list_guilds().await {
        Ok(guilds) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(
                guilds
                    .into_iter()
                    .map(|g| CreateGuildResponse {
                        guild_id: g.guild_id,
                        name: g.name,
                        created_at: g.created_at,
                    })
                    .collect(),
            ))
        }
        Err(err) => {
            tracing::error!(?err, "failed to list guilds");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

pub async fn create_channel(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(guild_id): Path<Uuid>,
    Json(body): Json<CreateChannelRequest>,
) -> Result<Json<CreateChannelResponse>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let name = body.name.trim();
    let name_length = name.chars().count();
    if name.is_empty() {
        state.record_messaging_rejection("channel_name_empty");
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }
    if name_length > MAX_CHANNEL_NAME_LENGTH {
        state.record_messaging_rejection("channel_name_length");
        tracing::warn!(
            length = name_length,
            max = MAX_CHANNEL_NAME_LENGTH,
            guild_id = %guild_id,
            "channel name exceeds maximum length"
        );
        state.record_messaging_rejection("channel_name_length");
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    match messaging.create_channel(guild_id, name).await {
        Ok(channel) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(CreateChannelResponse {
                channel_id: channel.channel_id,
                guild_id: channel.guild_id,
                name: channel.name,
                created_at: channel.created_at,
            }))
        }
        Err(MessagingError::GuildNotFound) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
        Err(err) => {
            tracing::error!(?err, "failed to create channel");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

pub async fn list_channels(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(guild_id): Path<Uuid>,
) -> Result<Json<Vec<CreateChannelResponse>>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    match messaging.list_channels(guild_id).await {
        Ok(channels) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(
                channels
                    .into_iter()
                    .map(|c| CreateChannelResponse {
                        channel_id: c.channel_id,
                        guild_id: c.guild_id,
                        name: c.name,
                        created_at: c.created_at,
                    })
                    .collect(),
            ))
        }
        Err(MessagingError::GuildNotFound) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
        Err(err) => {
            tracing::error!(?err, "failed to list channels");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

pub async fn post_message(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
    Json(body): Json<PostMessageRequest>,
) -> Result<Json<PostMessageResponse>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    let claims = match session::authenticate_bearer(&state, &headers) {
        Ok(claims) => claims,
        Err(status) => {
            if status == StatusCode::UNAUTHORIZED {
                state.record_messaging_rejection("unauthorized");
            }
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
    };

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let sender_claim = claims.user_id.to_string();
    let requested_sender = body.sender.trim();
    let content = body.content.trim();
    let content_length = content.chars().count();

    if content.is_empty() {
        state.record_messaging_rejection("message_empty");
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    if content_length > MAX_MESSAGE_LENGTH {
        state.record_messaging_rejection("message_length");
        tracing::warn!(
            length = content_length,
            max = MAX_MESSAGE_LENGTH,
            channel_id = %channel_id,
            "message content exceeds maximum length"
        );
        let status = StatusCode::BAD_REQUEST;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    let client_ip = client_ip_from_headers(&headers);

    let sender = if requested_sender.is_empty() {
        sender_claim.clone()
    } else {
        if requested_sender != sender_claim {
            state.record_messaging_rejection("sender_mismatch");
            let status = StatusCode::FORBIDDEN;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
        sender_claim.clone()
    };

    if !messaging.check_ip_rate(&client_ip).await {
        state.record_messaging_rejection("ip_rate_limit");
        let status = StatusCode::TOO_MANY_REQUESTS;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    if !messaging.check_message_rate(&sender).await {
        state.record_messaging_rejection("message_rate_limit");
        let status = StatusCode::TOO_MANY_REQUESTS;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    match messaging
        .append_message(channel_id, sender.as_str(), content)
        .await
    {
        Ok(event) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(PostMessageResponse {
                sequence: event.sequence,
                event_id: event.event_id,
                created_at: event.created_at,
            }))
        }
        Err(MessagingError::ChannelNotFound) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
        Err(err) => {
            tracing::error!(?err, "failed to append channel event");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence: i64,
    pub channel_id: Uuid,
    pub event: serde_json::Value,
}

impl From<ChannelEvent> for TimelineEvent {
    fn from(event: ChannelEvent) -> Self {
        Self {
            sequence: event.sequence,
            channel_id: event.channel_id,
            event: event.body,
        }
    }
}

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn list_events(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<Vec<TimelineEvent>>, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    match messaging.channel_exists(channel_id).await {
        Ok(true) => {}
        Ok(false) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
        Err(err) => {
            tracing::error!(?err, channel_id = %channel_id, "failed to verify channel");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let limit = query
        .limit
        .unwrap_or(DEFAULT_TIMELINE_LIMIT)
        .clamp(1, MAX_TIMELINE_LIMIT);

    match messaging
        .recent_events(channel_id, query.since, limit)
        .await
    {
        Ok(events) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(events.into_iter().map(TimelineEvent::from).collect()))
        }
        Err(MessagingError::ChannelNotFound) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
        Err(err) => {
            tracing::error!(?err, channel_id = %channel_id, "failed to list events");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            Err(status)
        }
    }
}

pub async fn channel_socket(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
    Extension(request_id): Extension<RequestId>,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let Some(messaging) = state.messaging() else {
        let status = StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        if status == StatusCode::UNAUTHORIZED {
            state.record_messaging_rejection("unauthorized");
        }
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    match messaging.channel_exists(channel_id).await {
        Ok(true) => {}
        Ok(false) => {
            let status = StatusCode::NOT_FOUND;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
        Err(err) => {
            tracing::error!(?err, "failed to determine channel existence");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return Err(status);
        }
    }

    let request_id_value = request_id
        .header_value()
        .to_str()
        .ok()
        .map(|value| value.to_string());

    Ok(messaging
        .open_websocket(channel_id, request_id_value, ws)
        .await)
}

struct RateLimiter {
    limits: Mutex<HashMap<String, RateWindow>>,
    limit: usize,
    window: Duration,
}

impl RateLimiter {
    fn new(limit: usize, window: Duration) -> Self {
        Self {
            limits: Mutex::new(HashMap::new()),
            limit,
            window,
        }
    }

    async fn check_and_increment(&self, key: &str) -> bool {
        let mut limits = self.limits.lock().await;
        let now = Instant::now();
        let entry = limits
            .entry(key.to_string())
            .or_insert_with(|| RateWindow::new(now, self.window));

        if now >= entry.reset_at {
            entry.reset(now, self.window);
        }

        if entry.count >= self.limit {
            return false;
        }

        entry.count += 1;
        true
    }
}

struct RateWindow {
    count: usize,
    reset_at: Instant,
}

impl RateWindow {
    fn new(now: Instant, window: Duration) -> Self {
        Self {
            count: 0,
            reset_at: now + window,
        }
    }

    fn reset(&mut self, now: Instant, window: Duration) {
        self.count = 0;
        self.reset_at = now + window;
    }
}

fn client_ip_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(feature = "metrics")]
pub fn init_messaging_service(
    config: &ServerConfig,
    pool: Option<StoragePool>,
    metrics: Option<Arc<MetricsContext>>,
) -> MessagingService {
    let origin = config.server_name().to_string();
    let service = if let Some(pool) = pool {
        MessagingService::new_with_pool(pool, origin)
    } else {
        MessagingService::new_in_memory(origin)
    };
    service.with_metrics(metrics)
}

#[cfg(not(feature = "metrics"))]
pub fn init_messaging_service(
    config: &ServerConfig,
    pool: Option<StoragePool>,
) -> MessagingService {
    let origin = config.server_name().to_string();
    if let Some(pool) = pool {
        MessagingService::new_with_pool(pool, origin)
    } else {
        MessagingService::new_in_memory(origin)
    }
}
