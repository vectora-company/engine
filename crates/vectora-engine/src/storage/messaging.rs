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
}

fn default_visibility_timeout_seconds() -> u64 {
    30
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
            let seconds = ttl.as_secs().max(1);
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
        if let Ok(store) = self.jetstream.get_key_value(&self.bucket).await {
            return Ok(store);
        }
        match self
            .jetstream
            .create_key_value(async_nats::jetstream::kv::Config {
                bucket: self.bucket.clone(),
                ..Default::default()
            })
            .await
        {
            Ok(store) => Ok(store),
            Err(_) => self
                .jetstream
                .get_key_value(&self.bucket)
                .await
                .map_err(|_| MessagingError::Storage),
        }
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
        if ttl.is_some() {
            return Err(MessagingError::Unsupported);
        }
        self.kv()
            .await?
            .put(key, value.into())
            .await
            .map(|_| ())
            .map_err(|_| MessagingError::Storage)
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
    Nats(NatsStore),
}

impl KvStoreAdapter {
    pub async fn connect(config: KvStoreConfig) -> Result<Self, MessagingError> {
        match config.backend {
            super::KvBackend::Redis => Ok(Self::Redis(RedisStore::connect(
                &config.endpoint,
                &config.namespace,
            )?)),
            super::KvBackend::NatsJetstream => Ok(Self::Nats(
                NatsStore::connect(&config.endpoint, &config.namespace).await?,
            )),
            super::KvBackend::Memory => Err(MessagingError::Unsupported),
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
}

impl RedisStreamQueue {
    pub fn new(store: RedisStore) -> Self {
        Self { store }
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
        self.receive_with_idle(queue, group, consumer, block, Duration::from_secs(30))
            .await
    }

    async fn receive_with_idle(
        &self,
        queue: &str,
        group: &str,
        consumer: &str,
        block: Duration,
        visibility_timeout: Duration,
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
            return redis_delivery(&self.store, key, group, entry).map(Some);
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
        redis_delivery(&self.store, key, group, entry).map(Some)
    }
}

fn redis_delivery(
    store: &RedisStore,
    stream: String,
    group: &str,
    entry: redis::streams::StreamId,
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
    })
}

pub struct RedisQueueDelivery {
    store: redis::Client,
    stream: String,
    group: String,
    pub id: String,
    pub payload: Vec<u8>,
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
}

impl NatsStreamQueue {
    pub async fn connect(
        url: &str,
        name: &str,
        ack_wait: Duration,
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
        };
        Ok(queue)
    }
    async fn ensure(
        &self,
        durable: &str,
    ) -> Result<async_nats::jetstream::consumer::PullConsumer, MessagingError> {
        use async_nats::jetstream::consumer::PullConsumer;
        let stream = self.ensure_stream().await?;
        let consumer: PullConsumer = stream
            .get_or_create_consumer(
                durable,
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(durable.to_owned()),
                    ack_wait: self.ack_wait,
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(consumer)
    }

    async fn ensure_stream(&self) -> Result<async_nats::jetstream::stream::Stream, MessagingError> {
        self.jetstream
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: self.stream.clone(),
                subjects: vec![self.subject.clone()],
                ..Default::default()
            })
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
        use futures_util::StreamExt;
        validate_key(group)?;
        let consumer = self.ensure(group).await?;
        let mut messages = consumer
            .messages()
            .await
            .map_err(|_| MessagingError::Storage)?;
        match tokio::time::timeout(timeout, messages.next()).await {
            Ok(Some(Ok(message))) => Ok(Some(NatsQueueDelivery { message })),
            Ok(Some(Err(_))) => Err(MessagingError::Storage),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

pub struct NatsQueueDelivery {
    message: async_nats::jetstream::Message,
}

impl NatsQueueDelivery {
    pub fn payload(&self) -> &[u8] {
        &self.message.payload
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
    Nats(NatsQueueDelivery),
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
}

impl QueueDelivery {
    pub fn payload(&self) -> &[u8] {
        match self {
            Self::Redis(delivery) => &delivery.payload,
            Self::Nats(delivery) => delivery.payload(),
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
    },
    Nats {
        queue: NatsStreamQueue,
    },
}

impl QueueAdapter {
    pub async fn connect(config: QueueConfig) -> Result<Self, MessagingError> {
        match config.backend {
            super::QueueBackend::RedisStreams => Ok(Self::Redis {
                queue: RedisStreamQueue::new(RedisStore::connect(
                    &config.endpoint,
                    &config.namespace,
                )?),
                name: config.namespace,
                visibility_timeout: Duration::from_secs(config.visibility_timeout_seconds),
            }),
            super::QueueBackend::NatsJetstream => Ok(Self::Nats {
                queue: NatsStreamQueue::connect(
                    &config.endpoint,
                    &config.namespace,
                    Duration::from_secs(config.visibility_timeout_seconds),
                )
                .await?,
            }),
            super::QueueBackend::Memory => Err(MessagingError::Unsupported),
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
            } => queue
                .receive_with_idle(name, group, consumer, timeout, *visibility_timeout)
                .await
                .map(|delivery| delivery.map(QueueDelivery::Redis)),
            Self::Nats { queue, .. } => queue
                .receive(group, timeout)
                .await
                .map(|delivery| delivery.map(QueueDelivery::Nats)),
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
            super::PubSubBackend::Memory => Err(MessagingError::Unsupported),
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
        assert!(matches!(
            store
                .put("ttl", b"x".to_vec(), Some(Duration::from_secs(1)))
                .await,
            Err(MessagingError::Unsupported)
        ));
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
}
