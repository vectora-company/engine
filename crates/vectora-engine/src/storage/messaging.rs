//! KV, durable queue, and ephemeral pub/sub contracts with Redis and NATS adapters.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

pub const MESSAGING_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KvStoreConfig {
    pub backend: super::KvBackend,
    pub endpoint: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueConfig {
    pub backend: super::QueueBackend,
    pub endpoint: String,
    pub namespace: String,
    #[serde(default = "default_visibility_timeout_seconds")]
    pub visibility_timeout_seconds: u64,
    /// Maximum messages retained by each queue stream.
    #[serde(default = "default_queue_max_messages")]
    pub max_messages: u64,
    /// Maximum total deliveries before a job moves to the dead-letter stream.
    #[serde(default = "default_queue_max_attempts")]
    pub max_attempts: u32,
}

fn default_visibility_timeout_seconds() -> u64 {
    30
}

fn default_queue_max_messages() -> u64 {
    100_000
}

fn default_queue_max_attempts() -> u32 {
    5
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PubSubConfig {
    pub backend: super::PubSubBackend,
    pub endpoint: String,
    pub namespace: String,
}

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("invalid messaging request")]
    InvalidInput,
    #[error("messaging operation failed")]
    Storage,
    #[error("operation is not supported by this adapter")]
    Unsupported,
}

#[async_trait]
pub trait KvStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MessagingError>;
    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError>;
    async fn delete(&self, key: &str) -> Result<(), MessagingError>;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QueueJob {
    pub id: String,
    pub payload: Vec<u8>,
}

pub struct RedisStore {
    client: redis::Client,
    prefix: String,
}

impl RedisStore {
    pub fn connect(url: &str, prefix: &str) -> Result<Self, MessagingError> {
        if prefix.trim().is_empty()
            || !prefix
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(MessagingError::InvalidInput);
        }
        let client = redis::Client::open(url).map_err(|_| MessagingError::InvalidInput)?;
        Ok(Self {
            client,
            prefix: prefix.to_owned(),
        })
    }
    fn key(&self, key: &str) -> Result<String, MessagingError> {
        validate_key(key)?;
        Ok(format!("{}:{key}", self.prefix))
    }
    async fn connection(&self) -> Result<redis::aio::MultiplexedConnection, MessagingError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| MessagingError::Storage)
    }
}

fn validate_key(key: &str) -> Result<(), MessagingError> {
    if key.is_empty() || key.len() > 512 || key.bytes().any(|c| c.is_ascii_control()) {
        Err(MessagingError::InvalidInput)
    } else {
        Ok(())
    }
}

fn ttl_seconds_ceil(ttl: Duration) -> u64 {
    let seconds = ttl.as_secs();
    if ttl.subsec_nanos() == 0 {
        seconds.max(1)
    } else {
        seconds.saturating_add(1).max(1)
    }
}

fn validate_identifier(value: &str) -> Result<(), MessagingError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        Err(MessagingError::InvalidInput)
    } else {
        Ok(())
    }
}

#[async_trait]
impl KvStore for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
    }
    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        let mut command = redis::cmd("SET");
        command.arg(key).arg(value);
        if let Some(ttl) = ttl {
            let seconds = ttl_seconds_ceil(ttl);
            command.arg("EX").arg(seconds);
        }
        command
            .query_async::<()>(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
    }
    async fn delete(&self, key: &str) -> Result<(), MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        redis::cmd("DEL")
            .arg(key)
            .query_async::<u64>(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(())
    }
}

pub struct NatsStore {
    client: async_nats::Client,
    jetstream: async_nats::jetstream::Context,
    bucket: String,
}

pub struct NatsCorePubSub {
    client: async_nats::Client,
    prefix: String,
}

impl NatsCorePubSub {
    pub async fn connect(url: &str, prefix: &str) -> Result<Self, MessagingError> {
        validate_subject(prefix)?;
        let client = async_nats::connect(url)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(Self {
            client,
            prefix: prefix.to_owned(),
        })
    }

    fn subject(&self, topic: &str) -> Result<String, MessagingError> {
        validate_subject(topic)?;
        Ok(format!("{}.{}", self.prefix, topic))
    }

    pub async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), MessagingError> {
        self.client
            .publish(self.subject(topic)?, payload.into())
            .await
            .map_err(|_| MessagingError::Storage)
    }

    pub async fn subscribe(&self, topic: &str) -> Result<NatsSubscription, MessagingError> {
        let subscriber = self
            .client
            .subscribe(self.subject(topic)?)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(NatsSubscription { subscriber })
    }
}

impl NatsStore {
    pub async fn connect(url: &str, bucket: &str) -> Result<Self, MessagingError> {
        validate_identifier(bucket)?;
        let client = async_nats::connect(url)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let jetstream = async_nats::jetstream::new(client.clone());
        let store = Self {
            client,
            jetstream,
            bucket: bucket.to_owned(),
        };
        store.kv().await?;
        Ok(store)
    }
    async fn kv(&self) -> Result<async_nats::jetstream::kv::Store, MessagingError> {
        let store = match self.jetstream.get_key_value(&self.bucket).await {
            Ok(store) => store,
            Err(_) => {
                return self
                    .jetstream
                    .create_key_value(async_nats::jetstream::kv::Config {
                        bucket: self.bucket.clone(),
                        limit_markers: Some(Duration::from_secs(60)),
                        ..Default::default()
                    })
                    .await
                    .map_err(|_| MessagingError::Storage);
            }
        };
        let mut stream_config = store.stream.cached_info().config.clone();
        if !stream_config.allow_message_ttl || stream_config.subject_delete_marker_ttl.is_none() {
            stream_config.allow_message_ttl = true;
            stream_config.subject_delete_marker_ttl = Some(Duration::from_secs(60));
            self.jetstream
                .update_stream(stream_config)
                .await
                .map_err(|_| MessagingError::Storage)?;
            return self
                .jetstream
                .get_key_value(&self.bucket)
                .await
                .map_err(|_| MessagingError::Storage);
        }
        Ok(store)
    }
    pub async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<(), MessagingError> {
        validate_subject(subject)?;
        self.client
            .publish(subject.to_owned(), payload.into())
            .await
            .map_err(|_| MessagingError::Storage)
    }
    pub async fn subscribe(&self, subject: &str) -> Result<NatsSubscription, MessagingError> {
        validate_subject(subject)?;
        let subscriber = self
            .client
            .subscribe(subject.to_owned())
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(NatsSubscription { subscriber })
    }
}

#[async_trait]
impl KvStore for NatsStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MessagingError> {
        validate_key(key)?;
        self.kv()
            .await?
            .get(key)
            .await
            .map(|value| value.map(|bytes| bytes.to_vec()))
            .map_err(|_| MessagingError::Storage)
    }
    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError> {
        validate_key(key)?;
        let store = self.kv().await?;
        match ttl {
            Some(ttl) => {
                let seconds = ttl_seconds_ceil(ttl);
                let mut headers = async_nats::HeaderMap::new();
                headers.insert(
                    async_nats::header::NATS_MESSAGE_TTL,
                    async_nats::HeaderValue::from(seconds),
                );
                self.jetstream
                    .publish_with_headers(
                        format!("$KV.{}.{}", self.bucket, key),
                        headers,
                        value.into(),
                    )
                    .await
                    .map_err(|_| MessagingError::Storage)?
                    .await
                    .map(|_| ())
                    .map_err(|_| MessagingError::Storage)
            }
            None => store
                .put(key, value.into())
                .await
                .map(|_| ())
                .map_err(|_| MessagingError::Storage),
        }
    }
    async fn delete(&self, key: &str) -> Result<(), MessagingError> {
        validate_key(key)?;
        self.kv()
            .await?
            .delete(key)
            .await
            .map_err(|_| MessagingError::Storage)
    }
}

pub enum KvStoreAdapter {
    Redis(RedisStore),
    Nats(Box<NatsStore>),
}

impl KvStoreAdapter {
    pub async fn connect(config: KvStoreConfig) -> Result<Self, MessagingError> {
        match config.backend {
            super::KvBackend::Redis => Ok(Self::Redis(RedisStore::connect(
                &config.endpoint,
                &config.namespace,
            )?)),
            super::KvBackend::NatsJetstream => Ok(Self::Nats(Box::new(
                NatsStore::connect(&config.endpoint, &config.namespace).await?,
            ))),
        }
    }
}

#[async_trait]
impl KvStore for KvStoreAdapter {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MessagingError> {
        match self {
            Self::Redis(store) => store.get(key).await,
            Self::Nats(store) => store.get(key).await,
        }
    }
    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError> {
        match self {
            Self::Redis(store) => store.put(key, value, ttl).await,
            Self::Nats(store) => store.put(key, value, ttl).await,
        }
    }
    async fn delete(&self, key: &str) -> Result<(), MessagingError> {
        match self {
            Self::Redis(store) => store.delete(key).await,
            Self::Nats(store) => store.delete(key).await,
        }
    }
}

pub struct NatsSubscription {
    subscriber: async_nats::Subscriber,
}

impl NatsSubscription {
    pub async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError> {
        use futures_util::StreamExt;
        match tokio::time::timeout(timeout, self.subscriber.next()).await {
            Ok(Some(message)) => Ok(Some(message.payload.to_vec())),
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }
}

fn validate_subject(subject: &str) -> Result<(), MessagingError> {
    if subject.is_empty()
        || subject.len() > 512
        || subject
            .split('.')
            .any(|part| part.is_empty() || part.contains(['*', '#']))
    {
        Err(MessagingError::InvalidInput)
    } else {
        Ok(())
    }
}

pub struct RedisPubSub {
    store: RedisStore,
}

impl RedisPubSub {
    pub fn new(store: RedisStore) -> Self {
        Self { store }
    }
    pub async fn publish(&self, channel: &str, payload: Vec<u8>) -> Result<u64, MessagingError> {
        validate_key(channel)?;
        let mut conn = self.store.connection().await?;
        redis::cmd("PUBLISH")
            .arg(format!("{}:{channel}", self.store.prefix))
            .arg(payload)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
    }
    pub async fn subscribe(&self, channel: &str) -> Result<RedisSubscription, MessagingError> {
        validate_key(channel)?;
        let mut pubsub = self
            .store
            .client
            .get_async_pubsub()
            .await
            .map_err(|_| MessagingError::Storage)?;
        pubsub
            .subscribe(format!("{}:{channel}", self.store.prefix))
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(RedisSubscription { pubsub })
    }
}

pub struct RedisStreamQueue {
    store: RedisStore,
    max_messages: u64,
}

impl RedisStreamQueue {
    pub fn new(store: RedisStore, max_messages: u64) -> Self {
        Self {
            store,
            max_messages,
        }
    }
    fn stream_key(&self, queue: &str) -> Result<String, MessagingError> {
        validate_key(queue)?;
        Ok(format!("{}:queue:{queue}", self.store.prefix))
    }
    pub async fn enqueue(&self, queue: &str, payload: Vec<u8>) -> Result<String, MessagingError> {
        let key = self.stream_key(queue)?;
        let mut conn = self.store.connection().await?;
        redis::cmd("XADD")
            .arg(key)
            .arg("MAXLEN")
            .arg(self.max_messages.min(usize::MAX as u64) as usize)
            .arg("*")
            .arg("payload")
            .arg(payload)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
    }
    pub async fn receive(
        &self,
        queue: &str,
        group: &str,
        consumer: &str,
        block: Duration,
    ) -> Result<Option<RedisQueueDelivery>, MessagingError> {
        self.receive_with_idle(
            queue,
            group,
            consumer,
            block,
            Duration::from_secs(30),
            Some(default_queue_max_attempts()),
        )
        .await
    }

    async fn receive_with_idle(
        &self,
        queue: &str,
        group: &str,
        consumer: &str,
        block: Duration,
        visibility_timeout: Duration,
        max_attempts: Option<u32>,
    ) -> Result<Option<RedisQueueDelivery>, MessagingError> {
        use redis::streams::StreamReadReply;
        let key = self.stream_key(queue)?;
        validate_key(group)?;
        validate_key(consumer)?;
        let mut conn = self.store.connection().await?;
        let _: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&key)
            .arg(group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;
        let reclaimed: redis::streams::StreamAutoClaimReply = redis::cmd("XAUTOCLAIM")
            .arg(&key)
            .arg(group)
            .arg(consumer)
            .arg(visibility_timeout.as_millis().min(usize::MAX as u128) as usize)
            .arg("0-0")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        if let Some(entry) = reclaimed.claimed.into_iter().next() {
            let pending: Vec<(String, String, usize, usize)> = redis::cmd("XPENDING")
                .arg(&key)
                .arg(group)
                .arg(&entry.id)
                .arg(&entry.id)
                .arg(1)
                .query_async(&mut conn)
                .await
                .map_err(|_| MessagingError::Storage)?;
            let Some((_, owner, _, attempts)) = pending.first() else {
                return Ok(None);
            };
            if owner != consumer {
                return Ok(None);
            }
            if max_attempts.is_some_and(|maximum| *attempts as u64 > u64::from(maximum)) {
                let payload = redis::from_redis_value::<Vec<u8>>(
                    entry.map.get("payload").ok_or(MessagingError::Storage)?,
                )
                .map_err(|_| MessagingError::Storage)?;
                let dead_letter = format!("{key}:dead");
                const MOVE_TO_DLQ: &str = r"
                    local pending = redis.call('XPENDING', KEYS[1], ARGV[4], ARGV[2], ARGV[2], 1)
                    if #pending == 0 or pending[1][2] ~= ARGV[6]
                        or pending[1][4] ~= tonumber(ARGV[3]) then
                        return 0
                    end
                    redis.call('XADD', KEYS[2], 'MAXLEN', ARGV[5], '*',
                        'payload', ARGV[1], 'source_id', ARGV[2], 'attempts', ARGV[3])
                    return redis.call('XACK', KEYS[1], ARGV[4], ARGV[2])
                ";
                redis::cmd("EVAL")
                    .arg(MOVE_TO_DLQ)
                    .arg(2)
                    .arg(&key)
                    .arg(dead_letter)
                    .arg(payload)
                    .arg(&entry.id)
                    .arg(attempts)
                    .arg(group)
                    .arg(self.max_messages.min(usize::MAX as u64) as usize)
                    .arg(consumer)
                    .query_async::<u64>(&mut conn)
                    .await
                    .map_err(|_| MessagingError::Storage)?;
                return Ok(None);
            }
            let attempts = u32::try_from(*attempts).unwrap_or(u32::MAX);
            return redis_delivery(&self.store, key, group, entry, attempts).map(Some);
        }
        let block_ms = block.as_millis().clamp(1, 30_000) as usize;
        let reply: StreamReadReply = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(block_ms)
            .arg("STREAMS")
            .arg(&key)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let Some(entry) = reply.keys.into_iter().flat_map(|stream| stream.ids).next() else {
            return Ok(None);
        };
        redis_delivery(&self.store, key, group, entry, 1).map(Some)
    }
}

fn redis_delivery(
    store: &RedisStore,
    stream: String,
    group: &str,
    entry: redis::streams::StreamId,
    attempts: u32,
) -> Result<RedisQueueDelivery, MessagingError> {
    let payload = redis::from_redis_value::<Vec<u8>>(
        entry.map.get("payload").ok_or(MessagingError::Storage)?,
    )
    .map_err(|_| MessagingError::Storage)?;
    Ok(RedisQueueDelivery {
        store: store.client.clone(),
        stream,
        group: group.into(),
        id: entry.id,
        payload,
        attempts,
    })
}

pub struct RedisQueueDelivery {
    store: redis::Client,
    stream: String,
    group: String,
    pub id: String,
    pub payload: Vec<u8>,
    pub attempts: u32,
}

impl RedisQueueDelivery {
    pub async fn ack(self) -> Result<(), MessagingError> {
        let mut conn = self
            .store
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| MessagingError::Storage)?;
        redis::cmd("XACK")
            .arg(self.stream)
            .arg(self.group)
            .arg(self.id)
            .query_async::<u64>(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(())
    }
}

pub struct NatsStreamQueue {
    jetstream: async_nats::jetstream::Context,
    stream: String,
    subject: String,
    ack_wait: Duration,
    max_messages: u64,
    max_attempts: u32,
    dead_stream: String,
    dead_subject: String,
}

impl NatsStreamQueue {
    pub async fn connect(
        url: &str,
        name: &str,
        ack_wait: Duration,
        max_messages: u64,
        max_attempts: u32,
    ) -> Result<Self, MessagingError> {
        validate_identifier(name)?;
        let client = async_nats::connect(url)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let jetstream = async_nats::jetstream::new(client);
        let queue = Self {
            jetstream,
            stream: format!("VQ_{}", name.to_ascii_uppercase()),
            subject: format!("vectora.queue.{name}"),
            ack_wait,
            max_messages,
            max_attempts,
            dead_stream: format!("VQ_{}_DLQ", name.to_ascii_uppercase()),
            dead_subject: format!("vectora.queue.{name}.dead"),
        };
        Ok(queue)
    }
    async fn ensure(
        &self,
        durable: &str,
    ) -> Result<async_nats::jetstream::consumer::PullConsumer, MessagingError> {
        use async_nats::jetstream::consumer::{FromConsumer, PullConsumer, pull::Config};
        let stream = self.ensure_stream().await?;
        let consumer: PullConsumer = stream
            .get_or_create_consumer(
                durable,
                Config {
                    durable_name: Some(durable.to_owned()),
                    ack_wait: self.ack_wait,
                    max_deliver: -1,
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| MessagingError::Storage)?;
        let current = &consumer.cached_info().config;
        if current.ack_wait == self.ack_wait && current.max_deliver == -1 {
            return Ok(consumer);
        }
        let mut config = Config::try_from_consumer_config(current.clone())
            .map_err(|_| MessagingError::Storage)?;
        config.ack_wait = self.ack_wait;
        // Keep broker redelivery available when a DLQ transfer fails.
        config.max_deliver = -1;
        self.jetstream
            .update_consumer_on_stream(config, &self.stream)
            .await
            .map_err(|_| MessagingError::Storage)
    }

    async fn ensure_stream(&self) -> Result<async_nats::jetstream::stream::Stream, MessagingError> {
        self.ensure_queue_stream(&self.stream, &self.subject).await
    }

    async fn ensure_dead_stream(
        &self,
    ) -> Result<async_nats::jetstream::stream::Stream, MessagingError> {
        self.ensure_queue_stream(&self.dead_stream, &self.dead_subject)
            .await
    }

    async fn ensure_queue_stream(
        &self,
        name: &str,
        subject: &str,
    ) -> Result<async_nats::jetstream::stream::Stream, MessagingError> {
        use async_nats::jetstream::stream::Config;

        let max_messages = self.max_messages.min(i64::MAX as u64) as i64;
        let stream = self
            .jetstream
            .get_or_create_stream(Config {
                name: name.to_owned(),
                subjects: vec![subject.to_owned()],
                max_messages,
                ..Default::default()
            })
            .await
            .map_err(|_| MessagingError::Storage)?;
        let current = &stream.cached_info().config;
        // Queue and DLQ names can overlap. Never rewrite a stream that serves
        // another subject, even if its name matches the configured queue.
        if current.subjects.len() != 1 || current.subjects[0] != subject {
            return Err(MessagingError::Storage);
        }
        if current.max_messages == max_messages {
            return Ok(stream);
        }
        let mut config = current.clone();
        config.max_messages = max_messages;
        self.jetstream
            .update_stream(config)
            .await
            .map_err(|_| MessagingError::Storage)?;
        self.jetstream
            .get_stream(name)
            .await
            .map_err(|_| MessagingError::Storage)
    }

    pub async fn enqueue(&self, payload: Vec<u8>) -> Result<u64, MessagingError> {
        self.ensure_stream().await?;
        self.jetstream
            .publish(self.subject.clone(), payload.into())
            .await
            .map_err(|_| MessagingError::Storage)?
            .await
            .map(|ack| ack.sequence)
            .map_err(|_| MessagingError::Storage)
    }
    pub async fn receive(
        &self,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<NatsQueueDelivery>, MessagingError> {
        validate_key(group)?;
        let consumer = self.ensure(group).await?;
        match pull_nats_queue_message(&consumer, timeout).await? {
            Some(message) => {
                let info = message.info().map_err(|_| MessagingError::Storage)?;
                let attempts = info.delivered.max(1);
                if attempts > i64::from(self.max_attempts) {
                    let dead_stream = self.ensure_dead_stream().await?;
                    let mut headers = async_nats::HeaderMap::new();
                    headers.insert(
                        async_nats::header::NATS_MESSAGE_ID,
                        format!("{}-{}", info.stream, info.stream_sequence),
                    );
                    headers.insert("Vectora-Source-Stream", info.stream);
                    headers.insert("Vectora-Source-Sequence", info.stream_sequence.to_string());
                    headers.insert("Vectora-Delivery-Attempts", attempts.to_string());
                    self.jetstream
                        .publish_with_headers(
                            self.dead_subject.clone(),
                            headers,
                            message.payload.clone(),
                        )
                        .await
                        .map_err(|_| MessagingError::Storage)?
                        .await
                        .map_err(|_| MessagingError::Storage)?;
                    message
                        .double_ack()
                        .await
                        .map_err(|_| MessagingError::Storage)?;
                    drop(dead_stream);
                    Ok(None)
                } else {
                    Ok(Some(NatsQueueDelivery {
                        message,
                        attempts: u32::try_from(attempts).unwrap_or(u32::MAX),
                    }))
                }
            }
            None => Ok(None),
        }
    }

    pub async fn receive_dead_letter(
        &self,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<NatsQueueDelivery>, MessagingError> {
        use async_nats::jetstream::consumer::PullConsumer;
        validate_key(group)?;
        let stream = self.ensure_dead_stream().await?;
        let consumer: PullConsumer = stream
            .get_or_create_consumer(
                group,
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(group.to_owned()),
                    ack_wait: self.ack_wait,
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| MessagingError::Storage)?;
        match pull_nats_queue_message(&consumer, timeout).await? {
            Some(message) => {
                let attempts = message
                    .info()
                    .map_err(|_| MessagingError::Storage)?
                    .delivered
                    .max(1) as u32;
                Ok(Some(NatsQueueDelivery { message, attempts }))
            }
            None => Ok(None),
        }
    }
}

async fn pull_nats_queue_message(
    consumer: &async_nats::jetstream::consumer::PullConsumer,
    timeout: Duration,
) -> Result<Option<async_nats::jetstream::Message>, MessagingError> {
    use futures_util::StreamExt;

    // Each API call owns one delivery. Expire the pull on the broker so an
    // idle receive cannot leave a request waiting with the SDK default expiry.
    let mut batch = consumer
        .batch()
        .max_messages(1)
        .expires(timeout.max(Duration::from_millis(1)))
        .messages()
        .await
        .map_err(|_| MessagingError::Storage)?;
    batch
        .next()
        .await
        .transpose()
        .map_err(|_| MessagingError::Storage)
}

pub struct NatsQueueDelivery {
    message: async_nats::jetstream::Message,
    attempts: u32,
}

impl NatsQueueDelivery {
    pub fn payload(&self) -> &[u8] {
        &self.message.payload
    }
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
    pub async fn ack(self) -> Result<(), MessagingError> {
        self.message
            .ack()
            .await
            .map_err(|_| MessagingError::Storage)
    }
}

/// Provider-neutral delivery. A delivery is acknowledged explicitly; dropping
/// it leaves the provider's normal redelivery policy in effect.
pub enum QueueDelivery {
    Redis(RedisQueueDelivery),
    Nats(Box<NatsQueueDelivery>),
}

#[async_trait]
pub trait DurableQueue: Send + Sync {
    async fn enqueue(&self, payload: Vec<u8>) -> Result<String, MessagingError>;
    async fn receive(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError>;
    async fn receive_dead_letter(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError>;
}

impl QueueDelivery {
    pub fn payload(&self) -> &[u8] {
        match self {
            Self::Redis(delivery) => &delivery.payload,
            Self::Nats(delivery) => delivery.payload(),
        }
    }

    pub fn attempts(&self) -> u32 {
        match self {
            Self::Redis(delivery) => delivery.attempts,
            Self::Nats(delivery) => delivery.attempts(),
        }
    }

    pub async fn ack(self) -> Result<(), MessagingError> {
        match self {
            Self::Redis(delivery) => delivery.ack().await,
            Self::Nats(delivery) => delivery.ack().await,
        }
    }
}

pub enum QueueAdapter {
    Redis {
        queue: RedisStreamQueue,
        name: String,
        visibility_timeout: Duration,
        max_attempts: u32,
    },
    Nats {
        queue: NatsStreamQueue,
    },
}

impl QueueAdapter {
    pub async fn connect(config: QueueConfig) -> Result<Self, MessagingError> {
        if config.visibility_timeout_seconds == 0
            || config.max_messages == 0
            || config.max_attempts == 0
        {
            return Err(MessagingError::InvalidInput);
        }
        match config.backend {
            super::QueueBackend::RedisStreams => Ok(Self::Redis {
                queue: RedisStreamQueue::new(
                    RedisStore::connect(&config.endpoint, &config.namespace)?,
                    config.max_messages,
                ),
                name: config.namespace,
                visibility_timeout: Duration::from_secs(config.visibility_timeout_seconds),
                max_attempts: config.max_attempts,
            }),
            super::QueueBackend::NatsJetstream => Ok(Self::Nats {
                queue: NatsStreamQueue::connect(
                    &config.endpoint,
                    &config.namespace,
                    Duration::from_secs(config.visibility_timeout_seconds),
                    config.max_messages,
                    config.max_attempts,
                )
                .await?,
            }),
        }
    }

    pub async fn enqueue(&self, payload: Vec<u8>) -> Result<String, MessagingError> {
        match self {
            Self::Redis { queue, name, .. } => queue.enqueue(name, payload).await,
            Self::Nats { queue, .. } => queue
                .enqueue(payload)
                .await
                .map(|sequence| sequence.to_string()),
        }
    }

    pub async fn receive(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError> {
        match self {
            Self::Redis {
                queue,
                name,
                visibility_timeout,
                max_attempts,
            } => queue
                .receive_with_idle(
                    name,
                    group,
                    consumer,
                    timeout,
                    *visibility_timeout,
                    Some(*max_attempts),
                )
                .await
                .map(|delivery| delivery.map(QueueDelivery::Redis)),
            Self::Nats { queue, .. } => queue
                .receive(group, timeout)
                .await
                .map(|delivery| delivery.map(|delivery| QueueDelivery::Nats(Box::new(delivery)))),
        }
    }

    /// Receives a failed delivery from the queue's recoverable dead-letter stream.
    pub async fn receive_dead_letter(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError> {
        match self {
            Self::Redis {
                queue,
                name,
                visibility_timeout,
                ..
            } => {
                let dead_name = format!("{name}:dead");
                queue
                    .receive_with_idle(
                        &dead_name,
                        group,
                        consumer,
                        timeout,
                        *visibility_timeout,
                        None,
                    )
                    .await
                    .map(|delivery| delivery.map(QueueDelivery::Redis))
            }
            Self::Nats { queue, .. } => queue
                .receive_dead_letter(group, timeout)
                .await
                .map(|delivery| delivery.map(|delivery| QueueDelivery::Nats(Box::new(delivery)))),
        }
    }
}

#[async_trait]
impl DurableQueue for QueueAdapter {
    async fn enqueue(&self, payload: Vec<u8>) -> Result<String, MessagingError> {
        QueueAdapter::enqueue(self, payload).await
    }

    async fn receive(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError> {
        QueueAdapter::receive(self, consumer, group, timeout).await
    }

    async fn receive_dead_letter(
        &self,
        consumer: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<Option<QueueDelivery>, MessagingError> {
        QueueAdapter::receive_dead_letter(self, consumer, group, timeout).await
    }
}

pub enum PubSubAdapter {
    Redis(RedisPubSub),
    Nats(NatsCorePubSub),
}

#[async_trait]
pub trait EphemeralPubSub: Send + Sync {
    async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), MessagingError>;
    async fn subscribe(&self, topic: &str) -> Result<PubSubSubscription, MessagingError>;
}

#[async_trait]
pub trait PubSubReceiver: Send {
    async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError>;
}

impl PubSubAdapter {
    pub async fn connect(config: PubSubConfig) -> Result<Self, MessagingError> {
        match config.backend {
            super::PubSubBackend::Redis => Ok(Self::Redis(RedisPubSub::new(RedisStore::connect(
                &config.endpoint,
                &config.namespace,
            )?))),
            super::PubSubBackend::NatsCore => Ok(Self::Nats(
                NatsCorePubSub::connect(&config.endpoint, &config.namespace).await?,
            )),
        }
    }

    pub async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), MessagingError> {
        match self {
            Self::Redis(adapter) => adapter.publish(topic, payload).await.map(|_| ()),
            Self::Nats(adapter) => adapter.publish(topic, payload).await,
        }
    }

    pub async fn subscribe(&self, topic: &str) -> Result<PubSubSubscription, MessagingError> {
        match self {
            Self::Redis(adapter) => adapter
                .subscribe(topic)
                .await
                .map(PubSubSubscription::Redis),
            Self::Nats(adapter) => adapter.subscribe(topic).await.map(PubSubSubscription::Nats),
        }
    }
}

pub enum PubSubSubscription {
    Redis(RedisSubscription),
    Nats(NatsSubscription),
}

impl PubSubSubscription {
    pub async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError> {
        match self {
            Self::Redis(subscription) => subscription.receive(timeout).await,
            Self::Nats(subscription) => subscription.receive(timeout).await,
        }
    }
}

#[async_trait]
impl EphemeralPubSub for PubSubAdapter {
    async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), MessagingError> {
        PubSubAdapter::publish(self, topic, payload).await
    }

    async fn subscribe(&self, topic: &str) -> Result<PubSubSubscription, MessagingError> {
        PubSubAdapter::subscribe(self, topic).await
    }
}

#[async_trait]
impl PubSubReceiver for PubSubSubscription {
    async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError> {
        PubSubSubscription::receive(self, timeout).await
    }
}

pub struct RedisSubscription {
    pubsub: redis::aio::PubSub,
}

impl RedisSubscription {
    pub async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError> {
        use futures_util::StreamExt;
        match tokio::time::timeout(timeout, self.pubsub.on_message().next()).await {
            Ok(Some(message)) => message
                .get_payload()
                .map(Some)
                .map_err(|_| MessagingError::Storage),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_retention_has_a_bounded_backward_compatible_default() {
        let config: QueueConfig = serde_json::from_str(
            r#"{"backend":"redis_streams","endpoint":"redis://localhost","namespace":"jobs"}"#,
        )
        .unwrap();
        assert_eq!(config.visibility_timeout_seconds, 30);
        assert_eq!(config.max_messages, 100_000);
        assert_eq!(config.max_attempts, 5);
    }

    #[tokio::test]
    async fn redis_kv_ttl_and_pubsub_against_real_service() {
        let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let namespace = format!("test_{}", std::process::id());
        let store = KvStoreAdapter::connect(KvStoreConfig {
            backend: super::super::KvBackend::Redis,
            endpoint: url.clone(),
            namespace: namespace.clone(),
        })
        .await
        .unwrap();
        store
            .put("persisted", b"value".to_vec(), None)
            .await
            .unwrap();
        assert_eq!(
            store.get("persisted").await.unwrap(),
            Some(b"value".to_vec())
        );
        store
            .put("expires", b"short".to_vec(), Some(Duration::from_secs(1)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(store.get("expires").await.unwrap(), None);
        let pubsub = PubSubAdapter::connect(PubSubConfig {
            backend: super::super::PubSubBackend::Redis,
            endpoint: url,
            namespace,
        })
        .await
        .unwrap();
        let mut subscription = pubsub.subscribe("events").await.unwrap();
        pubsub.publish("events", b"hello".to_vec()).await.unwrap();
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"hello".to_vec())
        );
    }

    #[tokio::test]
    async fn nats_kv_ttl_limit_and_core_pubsub_against_real_service() {
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let store = KvStoreAdapter::connect(KvStoreConfig {
            backend: super::super::KvBackend::NatsJetstream,
            endpoint: url.clone(),
            namespace: format!("TEST_{}", std::process::id()),
        })
        .await
        .unwrap();
        store.put("answer", b"42".to_vec(), None).await.unwrap();
        assert_eq!(store.get("answer").await.unwrap(), Some(b"42".to_vec()));
        store
            .put("ttl", b"persistent".to_vec(), None)
            .await
            .unwrap();
        store
            .put("ttl", b"x".to_vec(), Some(Duration::from_secs(1)))
            .await
            .unwrap();
        assert_eq!(store.get("ttl").await.unwrap(), Some(b"x".to_vec()));
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert_eq!(store.get("ttl").await.unwrap(), None);
        let pubsub = PubSubAdapter::connect(PubSubConfig {
            backend: super::super::PubSubBackend::NatsCore,
            endpoint: url,
            namespace: "vectora.test".into(),
        })
        .await
        .unwrap();
        let mut subscription = pubsub.subscribe("events").await.unwrap();
        pubsub.publish("events", b"hello".to_vec()).await.unwrap();
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"hello".to_vec())
        );
    }

    #[tokio::test]
    async fn redis_stream_queue_requires_ack_and_nats_jetstream_redelivers_unacked() {
        if let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") {
            let queue = QueueAdapter::connect(QueueConfig {
                backend: super::super::QueueBackend::RedisStreams,
                endpoint: url,
                namespace: format!("queue_{}", std::process::id()),
                visibility_timeout_seconds: 1,
                max_messages: 100_000,
                max_attempts: 5,
            })
            .await
            .unwrap();
            queue.enqueue(b"redis-job".to_vec()).await.unwrap();
            let delivery = queue
                .receive("worker-1", "workers", Duration::from_secs(1))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(delivery.payload(), b"redis-job");
            let delivery_id = match &delivery {
                QueueDelivery::Redis(delivery) => delivery.id.clone(),
                QueueDelivery::Nats(_) => unreachable!(),
            };
            drop(delivery);
            tokio::time::sleep(Duration::from_millis(1100)).await;
            let redelivered = queue
                .receive("worker-2", "workers", Duration::from_secs(1))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(redelivered.payload(), b"redis-job");
            match redelivered {
                QueueDelivery::Redis(delivery) => {
                    assert_eq!(delivery.id, delivery_id);
                    delivery.ack().await.unwrap();
                }
                QueueDelivery::Nats(_) => unreachable!(),
            }
        }
        if let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") {
            let queue = QueueAdapter::connect(QueueConfig {
                backend: super::super::QueueBackend::NatsJetstream,
                endpoint: url,
                namespace: format!("jobs_{}", std::process::id()),
                visibility_timeout_seconds: 1,
                max_messages: 100_000,
                max_attempts: 5,
            })
            .await
            .unwrap();
            queue.enqueue(b"nats-job".to_vec()).await.unwrap();
            let delivery = queue
                .receive("worker-1", "test-workers", Duration::from_secs(3))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(delivery.payload(), b"nats-job");
            drop(delivery);
            tokio::time::sleep(Duration::from_millis(1100)).await;
            let redelivered = queue
                .receive("worker-2", "test-workers", Duration::from_secs(3))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(redelivered.payload(), b"nats-job");
            redelivered.ack().await.unwrap();
        }
    }

    fn unique_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!(
            "{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[tokio::test]
    async fn acknowledged_jobs_are_not_redelivered_and_empty_queues_time_out() {
        let mut targets = Vec::new();
        if let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") {
            targets.push((super::super::QueueBackend::RedisStreams, url));
        }
        if let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") {
            targets.push((super::super::QueueBackend::NatsJetstream, url));
        }
        for (backend, endpoint) in targets {
            let queue = QueueAdapter::connect(QueueConfig {
                backend,
                endpoint,
                namespace: format!("ackq_{}", unique_suffix()),
                visibility_timeout_seconds: 1,
                max_messages: 100_000,
                max_attempts: 5,
            })
            .await
            .unwrap();
            assert!(
                queue
                    .receive("worker-1", "workers", Duration::from_millis(300))
                    .await
                    .unwrap()
                    .is_none(),
                "{backend:?}: an empty queue must time out without a delivery"
            );
            queue.enqueue(b"job".to_vec()).await.unwrap();
            let delivery = queue
                .receive("worker-1", "workers", Duration::from_secs(3))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(delivery.payload(), b"job");
            delivery.ack().await.unwrap();
            tokio::time::sleep(Duration::from_millis(1500)).await;
            assert!(
                queue
                    .receive("worker-2", "workers", Duration::from_millis(500))
                    .await
                    .unwrap()
                    .is_none(),
                "{backend:?}: an acknowledged job must not be redelivered"
            );
        }
    }

    #[tokio::test]
    async fn pubsub_drops_messages_published_without_a_subscriber() {
        let mut targets = Vec::new();
        if let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") {
            targets.push((super::super::PubSubBackend::Redis, url));
        }
        if let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") {
            targets.push((super::super::PubSubBackend::NatsCore, url));
        }
        for (backend, endpoint) in targets {
            let pubsub = PubSubAdapter::connect(PubSubConfig {
                backend,
                endpoint,
                namespace: format!("drop_{}", unique_suffix()),
            })
            .await
            .unwrap();
            pubsub.publish("events", b"early".to_vec()).await.unwrap();
            let mut subscription = pubsub.subscribe("events").await.unwrap();
            assert_eq!(
                subscription
                    .receive(Duration::from_millis(500))
                    .await
                    .unwrap(),
                None,
                "{backend:?}: a message without a subscriber must be dropped"
            );
            pubsub.publish("events", b"late".to_vec()).await.unwrap();
            assert_eq!(
                subscription.receive(Duration::from_secs(2)).await.unwrap(),
                Some(b"late".to_vec())
            );
        }
    }

    #[tokio::test]
    async fn kv_namespaces_are_isolated_and_delete_removes_keys() {
        let mut targets = Vec::new();
        if let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") {
            targets.push((super::super::KvBackend::Redis, url, "ns"));
        }
        if let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") {
            targets.push((super::super::KvBackend::NatsJetstream, url, "NS"));
        }
        for (backend, endpoint, prefix) in targets {
            let suffix = unique_suffix();
            let connect = |name: String| {
                let endpoint = endpoint.clone();
                let suffix = suffix.clone();
                async move {
                    KvStoreAdapter::connect(KvStoreConfig {
                        backend,
                        endpoint,
                        namespace: format!("{prefix}_{name}_{suffix}"),
                    })
                    .await
                    .unwrap()
                }
            };
            let first = connect("a".into()).await;
            let second = connect("b".into()).await;
            first.put("key", b"one".to_vec(), None).await.unwrap();
            assert_eq!(
                second.get("key").await.unwrap(),
                None,
                "{backend:?}: namespaces must not share keys"
            );
            second.put("key", b"two".to_vec(), None).await.unwrap();
            first.delete("key").await.unwrap();
            assert_eq!(first.get("key").await.unwrap(), None);
            assert_eq!(second.get("key").await.unwrap(), Some(b"two".to_vec()));
        }
    }

    /// Restarts a local test container. Enabled only when the operator sets
    /// `VECTORA_TEST_PODMAN_RESTART`, because it interrupts the service.
    fn restart_test_container(name: &str) -> bool {
        if std::env::var("VECTORA_TEST_PODMAN_RESTART").is_err() {
            return false;
        }
        let status = std::process::Command::new("podman")
            .args(["restart", "-t", "5", name])
            .status()
            .expect("podman is required for service restart tests");
        assert!(status.success(), "podman restart {name} failed");
        true
    }

    /// Retries until the adapter works again after a restart, so the test
    /// measures recovery and not how quickly the service listens.
    async fn eventually<T, F, Fut>(what: &str, mut operation: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, MessagingError>>,
    {
        let mut last = None;
        for _ in 0..40 {
            match tokio::time::timeout(Duration::from_secs(5), operation()).await {
                Ok(Ok(value)) => return value,
                Ok(Err(error)) => last = Some(error.to_string()),
                Err(_) => last = Some("timed out".into()),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("{what} did not recover after the service restart: {last:?}");
    }

    #[tokio::test]
    async fn redis_kv_and_stream_recover_after_service_restart() {
        let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let namespace = format!("restart_{}", unique_suffix());
        let store = KvStoreAdapter::connect(KvStoreConfig {
            backend: super::super::KvBackend::Redis,
            endpoint: url.clone(),
            namespace: namespace.clone(),
        })
        .await
        .unwrap();
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: url,
            namespace,
            visibility_timeout_seconds: 1,
            max_messages: 100_000,
            max_attempts: 5,
        })
        .await
        .unwrap();
        store.put("kept", b"value".to_vec(), None).await.unwrap();
        queue.enqueue(b"job".to_vec()).await.unwrap();
        if !restart_test_container("vectora-engine-redis") {
            return;
        }
        let kept = eventually("redis kv get", || store.get("kept")).await;
        assert_eq!(kept, Some(b"value".to_vec()), "persisted key must survive");
        let delivery = eventually("redis queue receive", || async {
            queue
                .receive("worker", "workers", Duration::from_secs(1))
                .await?
                .ok_or(MessagingError::Storage)
        })
        .await;
        assert_eq!(delivery.payload(), b"job");
        delivery.ack().await.unwrap();
        store.put("after", b"ok".to_vec(), None).await.unwrap();
    }

    #[tokio::test]
    async fn nats_kv_and_jetstream_recover_after_service_restart() {
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let suffix = unique_suffix();
        let store = KvStoreAdapter::connect(KvStoreConfig {
            backend: super::super::KvBackend::NatsJetstream,
            endpoint: url.clone(),
            namespace: format!("RESTART_{suffix}"),
        })
        .await
        .unwrap();
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url,
            namespace: format!("restartq_{suffix}"),
            visibility_timeout_seconds: 1,
            max_messages: 100_000,
            max_attempts: 5,
        })
        .await
        .unwrap();
        store.put("kept", b"value".to_vec(), None).await.unwrap();
        queue.enqueue(b"job".to_vec()).await.unwrap();
        if !restart_test_container("vectora-engine-nats") {
            return;
        }
        let kept = eventually("nats kv get", || store.get("kept")).await;
        assert_eq!(kept, Some(b"value".to_vec()), "persisted key must survive");
        let delivery = eventually("nats queue receive", || async {
            queue
                .receive("worker", "restart-workers", Duration::from_secs(2))
                .await?
                .ok_or(MessagingError::Storage)
        })
        .await;
        assert_eq!(delivery.payload(), b"job");
        delivery.ack().await.unwrap();
        store.put("after", b"ok".to_vec(), None).await.unwrap();
    }

    #[test]
    fn key_identifier_and_subject_bounds_are_enforced() {
        assert_eq!(ttl_seconds_ceil(Duration::from_millis(1)), 1);
        assert_eq!(ttl_seconds_ceil(Duration::from_millis(1_500)), 2);
        assert_eq!(ttl_seconds_ceil(Duration::from_secs(2)), 2);
        assert!(validate_key(&"k".repeat(512)).is_ok());
        for key in [String::new(), "k".repeat(513), "line\nbreak".to_owned()] {
            assert!(
                matches!(validate_key(&key), Err(MessagingError::InvalidInput)),
                "key {key:?} must be rejected"
            );
        }
        assert!(validate_identifier(&"a".repeat(128)).is_ok());
        for id in ["", "has space", "dot.ted", &"a".repeat(129)] {
            assert!(matches!(
                validate_identifier(id),
                Err(MessagingError::InvalidInput)
            ));
        }
        assert!(validate_subject("events.created").is_ok());
        for subject in ["", "a..b", ".a", "a.", "a.*", "a.#", &"s".repeat(513)] {
            assert!(
                matches!(validate_subject(subject), Err(MessagingError::InvalidInput)),
                "subject {subject:?} must be rejected"
            );
        }
    }

    #[tokio::test]
    async fn redis_stream_trims_oldest_entries_to_configured_length() {
        let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let adapter = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: url,
            namespace: format!("retain_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 1,
            max_attempts: 5,
        })
        .await
        .unwrap();
        let QueueAdapter::Redis { queue, name, .. } = &adapter else {
            unreachable!()
        };
        for payload in [b"one".to_vec(), b"two".to_vec()] {
            adapter.enqueue(payload).await.unwrap();
        }
        let mut conn = queue.store.connection().await.unwrap();
        let key = queue.stream_key(name).unwrap();
        let length: usize = redis::cmd("XLEN")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(length, 1, "the configured stream limit is enforced");
        let delivery = adapter
            .receive("worker", "workers", Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(delivery.payload(), b"two");
    }

    #[tokio::test]
    async fn nats_stream_uses_configured_finite_retention() {
        use async_nats::jetstream::stream::RetentionPolicy;
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let adapter = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url,
            namespace: format!("retain_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 1,
            max_attempts: 5,
        })
        .await
        .unwrap();
        let QueueAdapter::Nats { queue } = &adapter else {
            unreachable!()
        };
        adapter.enqueue(b"job".to_vec()).await.unwrap();
        adapter
            .receive("worker", "workers", Duration::from_secs(3))
            .await
            .unwrap()
            .unwrap()
            .ack()
            .await
            .unwrap();
        let info = queue
            .ensure_stream()
            .await
            .unwrap()
            .get_info()
            .await
            .unwrap();
        assert_eq!(
            info.state.messages, 1,
            "acknowledged jobs stay in the stream"
        );
        assert!(matches!(info.config.retention, RetentionPolicy::Limits));
        assert_eq!(info.config.max_messages, 1);
        assert_eq!(info.config.max_age, Duration::ZERO);
    }

    #[tokio::test]
    async fn redis_moves_exhausted_jobs_to_a_consumable_dead_letter_stream() {
        let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let config = QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: url,
            namespace: format!("dlq_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 2,
        };
        let queue = QueueAdapter::connect(config).await.unwrap();
        queue.enqueue(b"poison".to_vec()).await.unwrap();
        let first = queue
            .receive("worker", "workers", Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.attempts(), 1);
        drop(first);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        let second = queue
            .receive("worker", "workers", Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.attempts(), 2);
        drop(second);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(
            queue
                .receive("worker", "workers", Duration::from_secs(1))
                .await
                .unwrap()
                .is_none()
        );
        let dead = queue
            .receive_dead_letter("operator", "recovery", Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(dead.payload(), b"poison");
        dead.ack().await.unwrap();
    }

    #[tokio::test]
    async fn nats_moves_exhausted_jobs_to_a_consumable_dead_letter_stream() {
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let config = QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url,
            namespace: format!("dlq_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 2,
        };
        let queue = QueueAdapter::connect(config).await.unwrap();
        queue.enqueue(b"poison".to_vec()).await.unwrap();
        let first = queue
            .receive("worker", "workers", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.attempts(), 1);
        drop(first);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        let second = queue
            .receive("worker", "workers", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.attempts(), 2);
        drop(second);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(
            queue
                .receive("worker", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .is_none()
        );
        let dead = queue
            .receive_dead_letter("operator", "recovery", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(dead.payload(), b"poison");
        dead.ack().await.unwrap();
    }
}
