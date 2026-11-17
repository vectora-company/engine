//! KV, durable queue, and ephemeral pub/sub contracts with Redis and NATS adapters.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

pub const MESSAGING_CONTRACT_VERSION: u16 = 1;
const DEFAULT_QUEUE_DEDUPLICATION_WINDOW_SECONDS: u64 = 120;
const MAX_QUEUE_REPLAY_PAGE_SIZE: u32 = 1_000;

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
    /// Idempotency-key retention shared by Redis and NATS (default 120 seconds).
    #[serde(default = "default_queue_deduplication_window_seconds")]
    pub deduplication_window_seconds: u64,
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

fn default_queue_deduplication_window_seconds() -> u64 {
    DEFAULT_QUEUE_DEDUPLICATION_WINDOW_SECONDS
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
    #[error("concurrent KV updates exhausted the retry limit")]
    Contention,
    #[error("queue replay cursor expired because retention removed its position")]
    ReplayCursorExpired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KvEntry {
    pub value: Vec<u8>,
    pub revision: String,
}

#[async_trait]
pub trait KvStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MessagingError>;
    async fn get_with_revision(&self, key: &str) -> Result<Option<KvEntry>, MessagingError>;
    /// Replaces a live value only when its revision still matches.
    async fn compare_and_set(
        &self,
        key: &str,
        revision: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError>;

    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError>;
    async fn delete(&self, key: &str) -> Result<(), MessagingError>;
    /// Returns up to `limit` distinct keys in ascending byte order.
    /// Listing is a scan, so concurrent writes may change what is observed.
    async fn list_keys(&self, prefix: &str, limit: u32) -> Result<Vec<String>, MessagingError>;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QueueJob {
    pub id: String,
    pub payload: Vec<u8>,
}

/// Opaque checkpoint for read-only replay of retained queue history.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueueCursor {
    pub contract_version: u16,
    pub backend: super::QueueBackend,
    pub namespace: String,
    /// Redis stream ID (`milliseconds-sequence`) or JetStream sequence.
    pub position: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueueReplayRecord {
    pub cursor: QueueCursor,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueueReplayPage {
    pub records: Vec<QueueReplayRecord>,
    /// Cursor of the last scanned stream position, or the input cursor if none.
    pub next_cursor: Option<QueueCursor>,
}

enum RedisWriteCondition<'a> {
    Always,
    Absent,
    Revision(&'a str),
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
    async fn write_value(
        &self,
        key: &str,
        condition: RedisWriteCondition<'_>,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        const WRITE: &str = r"
            if ARGV[1] == 'absent' and redis.call('EXISTS', KEYS[1]) ~= 0 then
                return 0
            end
            if ARGV[1] == 'revision' and (redis.call('EXISTS', KEYS[1]) == 0
                or redis.call('GET', KEYS[2]) ~= ARGV[4]) then
                return 0
            end
            redis.call('DEL', KEYS[2])
            if ARGV[3] == '0' then
                redis.call('SET', KEYS[1], ARGV[2])
            else
                redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
            end
            return 1
        ";
        let (mode, revision) = match condition {
            RedisWriteCondition::Always => ("always", ""),
            RedisWriteCondition::Absent => ("absent", ""),
            RedisWriteCondition::Revision(revision) => ("revision", revision),
        };
        redis::cmd("EVAL")
            .arg(WRITE)
            .arg(2)
            .arg(&key)
            .arg(redis_revision_key(&key))
            .arg(mode)
            .arg(value)
            .arg(ttl.map_or(0, ttl_seconds_ceil))
            .arg(revision)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
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

fn validate_nats_kv_key(key: &str) -> Result<(), MessagingError> {
    validate_key(key)?;
    // JetStream KV uses the key as a subject suffix.
    if key.split('.').any(|part| part.is_empty())
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-/_=.".contains(&byte))
    {
        return Err(MessagingError::InvalidInput);
    }
    Ok(())
}

fn validate_key_listing(prefix: &str, limit: u32) -> Result<(), MessagingError> {
    if !(1..=1000).contains(&limit) {
        return Err(MessagingError::InvalidInput);
    }
    if !prefix.is_empty() {
        validate_key(prefix)?;
    }
    Ok(())
}

fn validate_replay_limit(limit: u32) -> Result<(), MessagingError> {
    if limit == 0 || limit > MAX_QUEUE_REPLAY_PAGE_SIZE {
        Err(MessagingError::InvalidInput)
    } else {
        Ok(())
    }
}

fn validate_redis_stream_id(id: &str) -> Result<(u64, u64), MessagingError> {
    let (milliseconds, sequence) = id.split_once('-').ok_or(MessagingError::InvalidInput)?;
    if milliseconds.is_empty() || sequence.is_empty() {
        return Err(MessagingError::InvalidInput);
    }
    Ok((
        milliseconds
            .parse()
            .map_err(|_| MessagingError::InvalidInput)?,
        sequence.parse().map_err(|_| MessagingError::InvalidInput)?,
    ))
}

fn compare_redis_stream_ids(left: &str, right: &str) -> Result<std::cmp::Ordering, MessagingError> {
    Ok(validate_redis_stream_id(left)?.cmp(&validate_redis_stream_id(right)?))
}

fn replay_payload(
    id: String,
    fields: std::collections::HashMap<String, redis::Value>,
) -> Result<(String, Vec<u8>), MessagingError> {
    let value = fields.get("payload").ok_or(MessagingError::Storage)?;
    let payload = redis::from_redis_value::<Vec<u8>>(value).map_err(|_| MessagingError::Storage)?;
    Ok((id, payload))
}

fn replay_cursor(backend: super::QueueBackend, namespace: &str, position: String) -> QueueCursor {
    QueueCursor {
        contract_version: MESSAGING_CONTRACT_VERSION,
        backend,
        namespace: namespace.to_owned(),
        position,
    }
}

fn validate_queue_cursor(
    cursor: Option<&QueueCursor>,
    backend: super::QueueBackend,
    namespace: &str,
) -> Result<(), MessagingError> {
    let Some(cursor) = cursor else {
        return Ok(());
    };
    if cursor.contract_version != MESSAGING_CONTRACT_VERSION
        || cursor.backend != backend
        || cursor.namespace != namespace
    {
        return Err(MessagingError::InvalidInput);
    }
    match backend {
        super::QueueBackend::RedisStreams => {
            validate_redis_stream_id(&cursor.position)?;
        }
        super::QueueBackend::NatsJetstream => {
            if cursor
                .position
                .parse::<u64>()
                .map_or(true, |sequence| sequence == 0)
            {
                return Err(MessagingError::InvalidInput);
            }
        }
    }
    Ok(())
}

fn keep_listed_key(keys: &mut std::collections::BTreeSet<String>, key: String, limit: u32) {
    keys.insert(key);
    if keys.len() > limit as usize {
        keys.pop_last();
    }
}

fn redis_revision_key(key: &str) -> String {
    format!(":vectora-kv-revision:{key}")
}

fn redis_revision_token() -> Result<String, MessagingError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| MessagingError::Storage)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
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
    async fn get_with_revision(&self, key: &str) -> Result<Option<KvEntry>, MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        const READ: &str = r"
            local value = redis.call('GET', KEYS[1])
            if not value then return nil end
            local revision = redis.call('GET', KEYS[2])
            if not revision then
                revision = ARGV[1]
                local ttl = redis.call('PTTL', KEYS[1])
                if ttl >= 0 then
                    redis.call('SET', KEYS[2], revision, 'PX', math.max(ttl, 1))
                else
                    redis.call('SET', KEYS[2], revision)
                end
            end
            return {value, revision}
        ";
        let entry: Option<(Vec<u8>, String)> = redis::cmd("EVAL")
            .arg(READ)
            .arg(2)
            .arg(&key)
            .arg(redis_revision_key(&key))
            .arg(redis_revision_token()?)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(entry.map(|(value, revision)| KvEntry { value, revision }))
    }

    async fn compare_and_set(
        &self,
        key: &str,
        revision: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        if revision.len() != 32 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(MessagingError::InvalidInput);
        }
        self.write_value(key, RedisWriteCondition::Revision(revision), value, ttl)
            .await
    }

    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError> {
        self.write_value(key, RedisWriteCondition::Always, value, ttl)
            .await
            .map(|_| ())
    }

    async fn delete(&self, key: &str) -> Result<(), MessagingError> {
        let key = self.key(key)?;
        let mut conn = self.connection().await?;
        redis::cmd("DEL")
            .arg(&key)
            .arg(redis_revision_key(&key))
            .query_async::<u64>(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(())
    }
    async fn list_keys(&self, prefix: &str, limit: u32) -> Result<Vec<String>, MessagingError> {
        validate_key_listing(prefix, limit)?;
        let namespace = format!("{}:", self.prefix);
        let mut pattern = namespace.clone();
        for ch in prefix.chars() {
            if matches!(ch, '*' | '?' | '[' | ']' | '\\') {
                pattern.push('\\');
            }
            pattern.push(ch);
        }
        pattern.push('*');
        let mut conn = self.connection().await?;
        let mut keys = std::collections::BTreeSet::new();
        let mut cursor = 0_u64;
        loop {
            let (next, batch): (u64, Vec<Vec<u8>>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("TYPE")
                .arg("string")
                .query_async(&mut conn)
                .await
                .map_err(|_| MessagingError::Storage)?;
            for key in batch {
                if let Ok(key) = std::str::from_utf8(&key)
                    && let Some(key) = key.strip_prefix(&namespace)
                    && key.starts_with(prefix)
                    && validate_key(key).is_ok()
                {
                    keep_listed_key(&mut keys, key.to_owned(), limit);
                }
            }
            cursor = next;
            if cursor == 0 {
                return Ok(keys.into_iter().collect());
            }
        }
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
            .map_err(|_| MessagingError::Storage)?;
        // Bindings may drop this short-lived adapter as soon as the call
        // returns. Flush so the queued publish reaches the server first.
        self.client
            .flush()
            .await
            .map_err(|_| MessagingError::Storage)
    }

    pub async fn subscribe(&self, topic: &str) -> Result<NatsSubscription, MessagingError> {
        let subscriber = self
            .client
            .subscribe(self.subject(topic)?)
            .await
            .map_err(|_| MessagingError::Storage)?;
        self.client
            .flush()
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(NatsSubscription { subscriber })
    }

    pub async fn subscribe_pattern(
        &self,
        pattern: &str,
    ) -> Result<NatsSubscription, MessagingError> {
        validate_subject_pattern(pattern)?;
        let subscriber = self
            .client
            .subscribe(format!("{}.{}", self.prefix, pattern))
            .await
            .map_err(|_| MessagingError::Storage)?;
        self.client
            .flush()
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
    async fn create_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        use async_nats::jetstream::kv::Operation;
        validate_nats_kv_key(key)?;
        let current = self
            .kv()
            .await?
            .entry(key)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let revision = match current {
            Some(entry) if entry.operation == Operation::Put => return Ok(false),
            Some(marker) => marker.revision,
            None => 0,
        };
        // Use the same TTL headers when recreating a key after a delete marker.
        self.publish_if_revision(key, revision, value, ttl).await
    }

    async fn publish_if_revision(
        &self,
        key: &str,
        revision: u64,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        use async_nats::jetstream::context::PublishErrorKind;
        let mut headers = async_nats::HeaderMap::new();
        headers.insert(
            async_nats::header::NATS_EXPECTED_STREAM,
            format!("KV_{}", self.bucket),
        );
        headers.insert(
            async_nats::header::NATS_EXPECTED_LAST_SUBJECT_SEQUENCE,
            async_nats::HeaderValue::from(revision),
        );
        if let Some(ttl) = ttl {
            headers.insert(
                async_nats::header::NATS_MESSAGE_TTL,
                async_nats::HeaderValue::from(ttl_seconds_ceil(ttl)),
            );
        }
        match self
            .jetstream
            .publish_with_headers(
                format!("$KV.{}.{}", self.bucket, key),
                headers,
                value.into(),
            )
            .await
            .map_err(|_| MessagingError::Storage)?
            .await
        {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == PublishErrorKind::WrongLastSequence => Ok(false),
            Err(_) => Err(MessagingError::Storage),
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
        validate_nats_kv_key(key)?;
        self.kv()
            .await?
            .get(key)
            .await
            .map(|value| value.map(|bytes| bytes.to_vec()))
            .map_err(|_| MessagingError::Storage)
    }
    async fn get_with_revision(&self, key: &str) -> Result<Option<KvEntry>, MessagingError> {
        use async_nats::jetstream::kv::Operation;
        validate_nats_kv_key(key)?;
        let entry = self
            .kv()
            .await?
            .entry(key)
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(entry
            .filter(|entry| entry.operation == Operation::Put)
            .map(|entry| KvEntry {
                value: entry.value.to_vec(),
                revision: entry.revision.to_string(),
            }))
    }

    async fn compare_and_set(
        &self,
        key: &str,
        revision: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        use async_nats::jetstream::kv::Operation;
        validate_nats_kv_key(key)?;
        let revision: u64 = revision
            .parse()
            .ok()
            .filter(|value| *value != 0)
            .ok_or(MessagingError::InvalidInput)?;
        let store = self.kv().await?;
        if !store
            .entry(key)
            .await
            .map_err(|_| MessagingError::Storage)?
            .is_some_and(|entry| entry.operation == Operation::Put && entry.revision == revision)
        {
            return Ok(false);
        }
        self.publish_if_revision(key, revision, value, ttl).await
    }

    async fn put(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), MessagingError> {
        validate_nats_kv_key(key)?;
        let store = self.kv().await?;
        match ttl {
            Some(ttl) => {
                let seconds = ttl_seconds_ceil(ttl);
                let mut headers = async_nats::HeaderMap::new();
                headers.insert(
                    async_nats::header::NATS_EXPECTED_STREAM,
                    format!("KV_{}", self.bucket),
                );
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
        validate_nats_kv_key(key)?;
        self.kv()
            .await?
            .delete(key)
            .await
            .map_err(|_| MessagingError::Storage)
    }
    async fn list_keys(&self, prefix: &str, limit: u32) -> Result<Vec<String>, MessagingError> {
        use futures_util::TryStreamExt;

        validate_key_listing(prefix, limit)?;
        let mut stream = self
            .kv()
            .await?
            .keys()
            .await
            .map_err(|_| MessagingError::Storage)?;
        let mut keys = std::collections::BTreeSet::new();
        while let Some(key) = stream
            .try_next()
            .await
            .map_err(|_| MessagingError::Storage)?
        {
            if key.starts_with(prefix) && validate_nats_kv_key(&key).is_ok() {
                keep_listed_key(&mut keys, key, limit);
            }
        }
        Ok(keys.into_iter().collect())
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

    /// Watches live changes to keys matching a literal prefix in this namespace.
    pub async fn watch(&self, prefix: &str) -> Result<KvWatch, MessagingError> {
        if !prefix.is_empty() {
            validate_key(prefix)?;
        }
        let source = match self {
            Self::Redis(store) => {
                let mut conn = store.connection().await?;
                let settings: std::collections::HashMap<String, String> = redis::cmd("CONFIG")
                    .arg("GET")
                    .arg("notify-keyspace-events")
                    .query_async(&mut conn)
                    .await
                    .map_err(|_| MessagingError::Storage)?;
                let flags = settings
                    .get("notify-keyspace-events")
                    .map(String::as_str)
                    .unwrap_or_default();
                if !flags.contains('K')
                    || !(flags.contains('A')
                        || ['g', '$', 'x', 'e']
                            .into_iter()
                            .all(|flag| flags.contains(flag)))
                {
                    return Err(MessagingError::Unsupported);
                }
                let db = store.client.get_connection_info().redis.db;
                let channel_prefix = format!("__keyspace@{db}__:{}:", store.prefix);
                let mut pubsub = store
                    .client
                    .get_async_pubsub()
                    .await
                    .map_err(|_| MessagingError::Storage)?;
                pubsub
                    .psubscribe(format!("{channel_prefix}*"))
                    .await
                    .map_err(|_| MessagingError::Storage)?;
                KvWatchSource::Redis {
                    channel_prefix,
                    pubsub,
                }
            }
            Self::Nats(store) => {
                let watch = store
                    .kv()
                    .await?
                    .watch_all()
                    .await
                    .map_err(|_| MessagingError::Storage)?;
                KvWatchSource::Nats(Box::new(watch))
            }
        };
        Ok(KvWatch::new(source, prefix))
    }

    /// Adds an integer delta to a JSON integer, using zero for an absent key.
    /// TTL replaces the prior per-key TTL. Omit it for a persistent value.
    pub async fn increment(
        &self,
        key: &str,
        delta: i64,
        ttl: Option<Duration>,
    ) -> Result<i64, MessagingError> {
        const SAFE_INTEGER: std::ops::RangeInclusive<i64> =
            -9_007_199_254_740_991..=9_007_199_254_740_991;
        if !SAFE_INTEGER.contains(&delta) {
            return Err(MessagingError::InvalidInput);
        }
        for _ in 0..32 {
            let entry = self.get_with_revision(key).await?;
            let current = match &entry {
                Some(entry) => serde_json::from_slice::<i64>(&entry.value)
                    .ok()
                    .filter(|value| SAFE_INTEGER.contains(value))
                    .ok_or(MessagingError::InvalidInput)?,
                None => 0,
            };
            let next = current
                .checked_add(delta)
                .filter(|value| SAFE_INTEGER.contains(value))
                .ok_or(MessagingError::InvalidInput)?;
            let value = next.to_string().into_bytes();
            let applied = match entry {
                Some(entry) => {
                    self.compare_and_set(key, &entry.revision, value, ttl)
                        .await?
                }
                None => match self {
                    Self::Redis(store) => {
                        store
                            .write_value(key, RedisWriteCondition::Absent, value, ttl)
                            .await?
                    }
                    Self::Nats(store) => store.create_if_absent(key, value, ttl).await?,
                },
            };
            if applied {
                return Ok(next);
            }
        }
        Err(MessagingError::Contention)
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
    async fn get_with_revision(&self, key: &str) -> Result<Option<KvEntry>, MessagingError> {
        match self {
            Self::Redis(store) => store.get_with_revision(key).await,
            Self::Nats(store) => store.get_with_revision(key).await,
        }
    }

    async fn compare_and_set(
        &self,
        key: &str,
        revision: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, MessagingError> {
        match self {
            Self::Redis(store) => store.compare_and_set(key, revision, value, ttl).await,
            Self::Nats(store) => store.compare_and_set(key, revision, value, ttl).await,
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
    async fn list_keys(&self, prefix: &str, limit: u32) -> Result<Vec<String>, MessagingError> {
        match self {
            Self::Redis(store) => store.list_keys(prefix, limit).await,
            Self::Nats(store) => store.list_keys(prefix, limit).await,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KvChangeKind {
    Put,
    Remove,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KvChange {
    pub key: String,
    pub kind: KvChangeKind,
}

enum KvWatchSource {
    Redis {
        channel_prefix: String,
        pubsub: redis::aio::PubSub,
    },
    Nats(Box<async_nats::jetstream::kv::Watch>),
}

/// A live subscription to key invalidations, with no portable replay guarantee.
#[derive(Clone)]
pub struct KvWatch {
    source: std::sync::Arc<tokio::sync::Mutex<Option<KvWatchSource>>>,
    closed: tokio::sync::watch::Sender<bool>,
    prefix: std::sync::Arc<str>,
}

impl KvWatch {
    fn new(source: KvWatchSource, prefix: &str) -> Self {
        Self {
            source: std::sync::Arc::new(tokio::sync::Mutex::new(Some(source))),
            closed: tokio::sync::watch::channel(false).0,
            prefix: prefix.into(),
        }
    }

    pub async fn receive(&self, timeout: Duration) -> Result<Option<KvChange>, MessagingError> {
        let mut closed = self.closed.subscribe();
        tokio::select! {
            biased;
            _ = closed.wait_for(|closed| *closed) => Ok(None),
            result = tokio::time::timeout(timeout, self.next_matching_change()) => {
                result.unwrap_or(Ok(None))
            }
        }
    }

    pub async fn close(&self) {
        self.closed.send_replace(true);
        self.source.lock().await.take();
    }

    async fn next_matching_change(&self) -> Result<Option<KvChange>, MessagingError> {
        use async_nats::jetstream::kv::Operation;
        use futures_util::StreamExt;

        let mut source = self.source.lock().await;
        let Some(source) = source.as_mut() else {
            return Ok(None);
        };
        loop {
            let change = match source {
                KvWatchSource::Redis {
                    channel_prefix,
                    pubsub,
                } => {
                    let msg = pubsub
                        .on_message()
                        .next()
                        .await
                        .ok_or(MessagingError::Storage)?;
                    let channel: Vec<u8> =
                        msg.get_channel().map_err(|_| MessagingError::Storage)?;
                    let Some(key) = std::str::from_utf8(&channel)
                        .ok()
                        .and_then(|name| name.strip_prefix(channel_prefix.as_str()))
                    else {
                        continue;
                    };
                    let event: String = msg.get_payload().map_err(|_| MessagingError::Storage)?;
                    let kind = match event.as_str() {
                        "set" => KvChangeKind::Put,
                        "del" | "expired" | "evicted" => KvChangeKind::Remove,
                        _ => continue,
                    };
                    KvChange {
                        key: key.to_owned(),
                        kind,
                    }
                }
                KvWatchSource::Nats(watch) => {
                    let entry = watch
                        .next()
                        .await
                        .ok_or(MessagingError::Storage)?
                        .map_err(|_| MessagingError::Storage)?;
                    let kind = match entry.operation {
                        Operation::Put => KvChangeKind::Put,
                        Operation::Delete | Operation::Purge => KvChangeKind::Remove,
                    };
                    KvChange {
                        key: entry.key,
                        kind,
                    }
                }
            };
            if change.key.starts_with(self.prefix.as_ref()) && validate_key(&change.key).is_ok() {
                return Ok(Some(change));
            }
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

fn validate_subject_pattern(pattern: &str) -> Result<(), MessagingError> {
    if pattern.is_empty() || pattern.len() > 512 {
        return Err(MessagingError::InvalidInput);
    }
    let parts: Vec<_> = pattern.split('.').collect();
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty()
            || part.contains('#')
            || (part.contains(['*', '>'])
                && *part != "*"
                && !(*part == ">" && index + 1 == parts.len()))
            || (*part == ">" && index + 1 != parts.len())
        {
            return Err(MessagingError::InvalidInput);
        }
    }
    Ok(())
}

fn subject_matches_pattern(pattern: &str, subject: &str) -> bool {
    let pattern: Vec<_> = pattern.split('.').collect();
    let subject: Vec<_> = subject.split('.').collect();
    for (index, part) in pattern.iter().enumerate() {
        match *part {
            ">" => return subject.len() > index,
            "*" if subject.get(index).is_some_and(|part| !part.is_empty()) => {}
            literal if subject.get(index) == Some(&literal) => {}
            _ => return false,
        }
    }
    pattern.len() == subject.len()
}

fn redis_subject_glob(prefix: &str, pattern: &str) -> String {
    let mut glob = format!("{prefix}:");
    for (index, part) in pattern.split('.').enumerate() {
        if index > 0 {
            glob.push('.');
        }
        if part == "*" || part == ">" {
            glob.push('*');
        } else {
            for character in part.chars() {
                if matches!(character, '*' | '?' | '[' | ']' | '\\') {
                    glob.push('\\');
                }
                glob.push(character);
            }
        }
    }
    glob
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
        Ok(RedisSubscription {
            pubsub,
            filter: None,
        })
    }

    pub async fn subscribe_pattern(
        &self,
        pattern: &str,
    ) -> Result<RedisSubscription, MessagingError> {
        validate_subject_pattern(pattern)?;
        let mut pubsub = self
            .store
            .client
            .get_async_pubsub()
            .await
            .map_err(|_| MessagingError::Storage)?;
        pubsub
            .psubscribe(redis_subject_glob(&self.store.prefix, pattern))
            .await
            .map_err(|_| MessagingError::Storage)?;
        Ok(RedisSubscription {
            pubsub,
            filter: Some((format!("{}:", self.store.prefix), pattern.to_owned())),
        })
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

    pub async fn enqueue_idempotent(
        &self,
        queue: &str,
        payload: Vec<u8>,
        idempotency_key: &str,
        deduplication_window: Duration,
    ) -> Result<String, MessagingError> {
        validate_identifier(idempotency_key)?;
        let stream = self.stream_key(queue)?;
        let dedupe = format!("{stream}:dedupe:{idempotency_key}");
        let mut conn = self.store.connection().await?;
        const ENQUEUE_ONCE: &str = r"
            local existing = redis.call('GET', KEYS[2])
            if existing then return existing end
            local id = redis.call('XADD', KEYS[1], 'MAXLEN', ARGV[1], '*', 'payload', ARGV[2])
            redis.call('SET', KEYS[2], id, 'PX', ARGV[3])
            return id
        ";
        redis::cmd("EVAL")
            .arg(ENQUEUE_ONCE)
            .arg(2)
            .arg(stream)
            .arg(dedupe)
            .arg(self.max_messages.min(usize::MAX as u64) as usize)
            .arg(payload)
            .arg(deduplication_window.as_millis() as u64)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)
    }

    pub async fn replay_after(
        &self,
        queue: &str,
        after: Option<&str>,
        limit: u32,
    ) -> Result<(Vec<(String, Vec<u8>)>, Option<String>), MessagingError> {
        use redis::streams::StreamRangeReply;

        validate_replay_limit(limit)?;
        let key = self.stream_key(queue)?;
        let mut conn = self.store.connection().await?;
        let bounds: StreamRangeReply = redis::cmd("XRANGE")
            .arg(&key)
            .arg("-")
            .arg("+")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let first = bounds.ids.first().map(|entry| entry.id.as_str());
        if let Some(cursor) = after {
            validate_redis_stream_id(cursor)?;
            let info: redis::streams::StreamInfoStreamReply = redis::cmd("XINFO")
                .arg("STREAM")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(|_| MessagingError::Storage)?;
            if compare_redis_stream_ids(cursor, &info.last_generated_id)?
                == std::cmp::Ordering::Greater
            {
                return Err(MessagingError::InvalidInput);
            }
            if let Some(first) = first {
                if compare_redis_stream_ids(cursor, first)? == std::cmp::Ordering::Less {
                    return Err(MessagingError::ReplayCursorExpired);
                }
            } else {
                return Err(MessagingError::ReplayCursorExpired);
            }
        }
        let Some(after) = after else {
            let rows: StreamRangeReply = redis::cmd("XRANGE")
                .arg(&key)
                .arg("-")
                .arg("+")
                .arg("COUNT")
                .arg(limit)
                .query_async(&mut conn)
                .await
                .map_err(|_| MessagingError::Storage)?;
            let last = rows.ids.last().map(|entry| entry.id.clone());
            let records = rows
                .ids
                .into_iter()
                .map(|entry| replay_payload(entry.id, entry.map))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((records, last));
        };
        validate_redis_stream_id(after)?;
        let exclusive = format!("({after}");
        let rows: StreamRangeReply = redis::cmd("XRANGE")
            .arg(&key)
            .arg(exclusive)
            .arg("+")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        let last = rows.ids.last().map(|entry| entry.id.clone());
        let records = rows
            .ids
            .into_iter()
            .map(|entry| replay_payload(entry.id, entry.map))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((records, last))
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
            return redis_delivery(&self.store, key, group, entry, attempts, visibility_timeout)
                .map(Some);
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
        redis_delivery(&self.store, key, group, entry, 1, visibility_timeout).map(Some)
    }
}

fn redis_delivery(
    store: &RedisStore,
    stream: String,
    group: &str,
    entry: redis::streams::StreamId,
    attempts: u32,
    visibility_timeout: Duration,
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
        visibility_timeout,
    })
}

pub struct RedisQueueDelivery {
    store: redis::Client,
    stream: String,
    group: String,
    pub id: String,
    pub payload: Vec<u8>,
    pub attempts: u32,
    visibility_timeout: Duration,
}

impl RedisQueueDelivery {
    pub async fn nack(self, delay: Duration) -> Result<(), MessagingError> {
        if delay > self.visibility_timeout {
            return Err(MessagingError::InvalidInput);
        }
        let idle_ms = self
            .visibility_timeout
            .saturating_sub(delay)
            .as_millis()
            .min(usize::MAX as u128) as usize;
        let mut conn = self
            .store
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| MessagingError::Storage)?;
        let claimed: Vec<String> = redis::cmd("XCLAIM")
            .arg(&self.stream)
            .arg(&self.group)
            .arg("vectora-retry")
            .arg(0)
            .arg(&self.id)
            .arg("IDLE")
            .arg(idle_ms)
            .arg("JUSTID")
            .query_async(&mut conn)
            .await
            .map_err(|_| MessagingError::Storage)?;
        if claimed.iter().any(|id| id == &self.id) {
            Ok(())
        } else {
            Err(MessagingError::Storage)
        }
    }

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
    deduplication_window: Duration,
    dead_stream: String,
    dead_subject: String,
}

impl NatsStreamQueue {
    fn namespace(&self) -> String {
        self.subject
            .strip_prefix("vectora.queue.")
            .unwrap_or(&self.subject)
            .to_owned()
    }
}

impl NatsStreamQueue {
    pub async fn connect(
        url: &str,
        name: &str,
        ack_wait: Duration,
        max_messages: u64,
        max_attempts: u32,
        deduplication_window: Duration,
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
            deduplication_window,
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
                duplicate_window: self.deduplication_window,
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
        if current.max_messages == max_messages
            && current.duplicate_window == self.deduplication_window
        {
            return Ok(stream);
        }
        let mut config = current.clone();
        config.max_messages = max_messages;
        config.duplicate_window = self.deduplication_window;
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

    pub async fn enqueue_idempotent(
        &self,
        payload: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<u64, MessagingError> {
        validate_identifier(idempotency_key)?;
        self.ensure_stream().await?;
        let mut headers = async_nats::HeaderMap::new();
        headers.insert(async_nats::header::NATS_MESSAGE_ID, idempotency_key);
        self.jetstream
            .publish_with_headers(self.subject.clone(), headers, payload.into())
            .await
            .map_err(|_| MessagingError::Storage)?
            .await
            .map(|ack| ack.sequence)
            .map_err(|_| MessagingError::Storage)
    }

    pub async fn replay_after(
        &self,
        after: Option<u64>,
        limit: u32,
    ) -> Result<(Vec<(String, Vec<u8>)>, Option<u64>), MessagingError> {
        validate_replay_limit(limit)?;
        let mut stream = self.ensure_stream().await?;
        let state = stream
            .info()
            .await
            .map_err(|_| MessagingError::Storage)?
            .state
            .clone();
        if let Some(cursor) = after {
            if cursor > state.last_sequence {
                return Err(MessagingError::InvalidInput);
            }
            if cursor < state.first_sequence {
                return Err(MessagingError::ReplayCursorExpired);
            }
        }
        if state.messages == 0 {
            return Ok((Vec::new(), None));
        }
        let mut records = Vec::new();
        let mut sequence = after
            .map(|cursor| cursor.saturating_add(1))
            .unwrap_or(state.first_sequence);
        let mut scanned = 0;
        while sequence <= state.last_sequence && scanned < limit {
            match stream.get_raw_message(sequence).await {
                Ok(message) if message.subject.as_str() == self.subject => {
                    records.push((sequence.to_string(), message.payload.to_vec()));
                }
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        async_nats::jetstream::stream::RawMessageErrorKind::NoMessageFound
                    ) => {}
                Err(_) => return Err(MessagingError::Storage),
            }
            sequence = sequence.saturating_add(1);
            scanned += 1;
        }
        let scanned_position = if scanned == 0 {
            None
        } else {
            Some(sequence.saturating_sub(1).min(state.last_sequence))
        };
        Ok((records, scanned_position))
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
                        ack_wait: self.ack_wait,
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
                Ok(Some(NatsQueueDelivery {
                    message,
                    attempts,
                    ack_wait: self.ack_wait,
                }))
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
    ack_wait: Duration,
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

    pub async fn nack(self, delay: Duration) -> Result<(), MessagingError> {
        if delay > self.ack_wait {
            return Err(MessagingError::InvalidInput);
        }
        self.message
            .ack_with(async_nats::jetstream::AckKind::Nak(Some(delay)))
            .await
            .map_err(|_| MessagingError::Storage)
    }
}

/// Provider-neutral delivery. A delivery is acknowledged explicitly; dropping
/// it leaves the provider's normal redelivery policy in effect.
pub enum QueueDelivery {
    Redis(Box<RedisQueueDelivery>),
    Nats(Box<NatsQueueDelivery>),
}

#[async_trait]
pub trait DurableQueue: Send + Sync {
    async fn enqueue(&self, payload: Vec<u8>) -> Result<String, MessagingError>;
    async fn enqueue_idempotent(
        &self,
        payload: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<String, MessagingError>;
    async fn replay_after(
        &self,
        cursor: Option<&QueueCursor>,
        limit: u32,
    ) -> Result<QueueReplayPage, MessagingError>;
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

    /// Reject this delivery and make it eligible after `delay`.
    /// The common limit is the configured visibility timeout.
    pub async fn nack(self, delay: Duration) -> Result<(), MessagingError> {
        match self {
            Self::Redis(delivery) => delivery.nack(delay).await,
            Self::Nats(delivery) => delivery.nack(delay).await,
        }
    }
}

pub enum QueueAdapter {
    Redis {
        queue: RedisStreamQueue,
        name: String,
        visibility_timeout: Duration,
        max_attempts: u32,
        deduplication_window: Duration,
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
            || config.deduplication_window_seconds == 0
            || config.deduplication_window_seconds > u64::MAX / 1_000
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
                deduplication_window: Duration::from_secs(config.deduplication_window_seconds),
            }),
            super::QueueBackend::NatsJetstream => Ok(Self::Nats {
                queue: NatsStreamQueue::connect(
                    &config.endpoint,
                    &config.namespace,
                    Duration::from_secs(config.visibility_timeout_seconds),
                    config.max_messages,
                    config.max_attempts,
                    Duration::from_secs(config.deduplication_window_seconds),
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

    /// Enqueues at most one message per key during the configured common window.
    pub async fn enqueue_idempotent(
        &self,
        payload: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<String, MessagingError> {
        validate_identifier(idempotency_key)?;
        match self {
            Self::Redis {
                queue,
                name,
                deduplication_window,
                ..
            } => {
                queue
                    .enqueue_idempotent(name, payload, idempotency_key, *deduplication_window)
                    .await
            }
            Self::Nats { queue, .. } => queue
                .enqueue_idempotent(payload, idempotency_key)
                .await
                .map(|sequence| sequence.to_string()),
        }
    }

    /// Reads retained queue history without creating or advancing a consumer.
    pub async fn replay_after(
        &self,
        cursor: Option<&QueueCursor>,
        limit: u32,
    ) -> Result<QueueReplayPage, MessagingError> {
        validate_replay_limit(limit)?;
        match self {
            Self::Redis { queue, name, .. } => {
                validate_queue_cursor(cursor, super::QueueBackend::RedisStreams, name)?;
                let (entries, scanned_position) = queue
                    .replay_after(name, cursor.map(|cursor| cursor.position.as_str()), limit)
                    .await?;
                let records: Vec<_> = entries
                    .into_iter()
                    .map(|(position, payload)| QueueReplayRecord {
                        cursor: replay_cursor(super::QueueBackend::RedisStreams, name, position),
                        payload,
                    })
                    .collect();
                let next_cursor = scanned_position
                    .map(|position| {
                        replay_cursor(super::QueueBackend::RedisStreams, name, position)
                    })
                    .or_else(|| cursor.cloned());
                Ok(QueueReplayPage {
                    records,
                    next_cursor,
                })
            }
            Self::Nats { queue } => {
                validate_queue_cursor(
                    cursor,
                    super::QueueBackend::NatsJetstream,
                    &queue.namespace(),
                )?;
                let (entries, scanned_position) = queue
                    .replay_after(
                        cursor
                            .map(|cursor| cursor.position.parse::<u64>())
                            .transpose()
                            .map_err(|_| MessagingError::InvalidInput)?,
                        limit,
                    )
                    .await?;
                let records: Vec<_> = entries
                    .into_iter()
                    .map(|(position, payload)| QueueReplayRecord {
                        cursor: replay_cursor(
                            super::QueueBackend::NatsJetstream,
                            &queue.namespace(),
                            position,
                        ),
                        payload,
                    })
                    .collect();
                let next_cursor = scanned_position
                    .map(|position| {
                        replay_cursor(
                            super::QueueBackend::NatsJetstream,
                            &queue.namespace(),
                            position.to_string(),
                        )
                    })
                    .or_else(|| cursor.cloned());
                Ok(QueueReplayPage {
                    records,
                    next_cursor,
                })
            }
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
                ..
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
                .map(|delivery| delivery.map(|delivery| QueueDelivery::Redis(Box::new(delivery)))),
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
                    .map(|delivery| {
                        delivery.map(|delivery| QueueDelivery::Redis(Box::new(delivery)))
                    })
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

    async fn enqueue_idempotent(
        &self,
        payload: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<String, MessagingError> {
        QueueAdapter::enqueue_idempotent(self, payload, idempotency_key).await
    }

    async fn replay_after(
        &self,
        cursor: Option<&QueueCursor>,
        limit: u32,
    ) -> Result<QueueReplayPage, MessagingError> {
        QueueAdapter::replay_after(self, cursor, limit).await
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
    async fn subscribe_pattern(&self, pattern: &str) -> Result<PubSubSubscription, MessagingError>;
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

    pub async fn subscribe_pattern(
        &self,
        pattern: &str,
    ) -> Result<PubSubSubscription, MessagingError> {
        match self {
            Self::Redis(adapter) => adapter
                .subscribe_pattern(pattern)
                .await
                .map(PubSubSubscription::Redis),
            Self::Nats(adapter) => adapter
                .subscribe_pattern(pattern)
                .await
                .map(PubSubSubscription::Nats),
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

    async fn subscribe_pattern(&self, pattern: &str) -> Result<PubSubSubscription, MessagingError> {
        PubSubAdapter::subscribe_pattern(self, pattern).await
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
    filter: Option<(String, String)>,
}

impl RedisSubscription {
    pub async fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, MessagingError> {
        use futures_util::StreamExt;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match tokio::time::timeout(remaining, self.pubsub.on_message().next()).await {
                Ok(Some(message)) => {
                    if let Some((namespace, pattern)) = &self.filter {
                        let channel = message.get_channel_name();
                        let Some(topic) = channel.strip_prefix(namespace) else {
                            continue;
                        };
                        if !subject_matches_pattern(pattern, topic) {
                            continue;
                        }
                    }
                    return message
                        .get_payload()
                        .map(Some)
                        .map_err(|_| MessagingError::Storage);
                }
                Ok(None) | Err(_) => return Ok(None),
            }
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
        assert_eq!(config.deduplication_window_seconds, 120);
    }

    #[test]
    fn pubsub_patterns_use_shared_nats_segment_semantics() {
        for pattern in ["jobs.*", "jobs.>", "*", "jobs.*.created"] {
            assert!(validate_subject_pattern(pattern).is_ok());
        }
        for pattern in [
            "",
            "jobs.",
            "jobs.**",
            "jobs.>.created",
            "jobs.foo*",
            "jobs.#",
        ] {
            assert!(matches!(
                validate_subject_pattern(pattern),
                Err(MessagingError::InvalidInput)
            ));
        }
        assert!(subject_matches_pattern("jobs.*", "jobs.created"));
        assert!(!subject_matches_pattern("jobs.*", "jobs.deep.created"));
        assert!(subject_matches_pattern("jobs.>", "jobs.created"));
        assert!(subject_matches_pattern("jobs.>", "jobs.deep.created"));
        assert!(!subject_matches_pattern("jobs.>", "jobs"));
    }

    #[tokio::test]
    async fn redis_pubsub_pattern_matches_one_segment_against_real_service() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let config = PubSubConfig {
            backend: super::super::PubSubBackend::Redis,
            endpoint,
            namespace: format!("pattern_{}", unique_suffix()),
        };
        let pubsub = PubSubAdapter::connect(config).await.unwrap();
        let mut subscription = pubsub.subscribe_pattern("jobs.*").await.unwrap();
        pubsub.publish("jobs.one", b"first".to_vec()).await.unwrap();
        pubsub
            .publish("jobs.deep.two", b"ignored".to_vec())
            .await
            .unwrap();
        pubsub
            .publish("jobs.three", b"last".to_vec())
            .await
            .unwrap();
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"first".to_vec())
        );
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"last".to_vec())
        );
        assert!(
            subscription
                .receive(Duration::from_millis(100))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn nats_pubsub_pattern_matches_one_segment_against_real_service() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let config = PubSubConfig {
            backend: super::super::PubSubBackend::NatsCore,
            endpoint,
            namespace: format!("pattern_{}", unique_suffix()),
        };
        let pubsub = PubSubAdapter::connect(config).await.unwrap();
        let mut subscription = pubsub.subscribe_pattern("jobs.*").await.unwrap();
        pubsub.publish("jobs.one", b"first".to_vec()).await.unwrap();
        pubsub
            .publish("jobs.deep.two", b"ignored".to_vec())
            .await
            .unwrap();
        pubsub
            .publish("jobs.three", b"last".to_vec())
            .await
            .unwrap();
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"first".to_vec())
        );
        assert_eq!(
            subscription.receive(Duration::from_secs(2)).await.unwrap(),
            Some(b"last".to_vec())
        );
        assert!(
            subscription
                .receive(Duration::from_millis(100))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn queue_rejects_a_zero_deduplication_window_before_connecting() {
        let result = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: "redis://127.0.0.1:1".into(),
            namespace: "jobs".into(),
            visibility_timeout_seconds: 30,
            max_messages: 100_000,
            max_attempts: 5,
            deduplication_window_seconds: 0,
        })
        .await;
        assert!(matches!(result, Err(MessagingError::InvalidInput)));
    }

    #[test]
    fn queue_replay_rejects_invalid_limits_and_cross_queue_cursors() {
        assert!(matches!(
            validate_replay_limit(0),
            Err(MessagingError::InvalidInput)
        ));
        assert!(matches!(
            validate_replay_limit(1_001),
            Err(MessagingError::InvalidInput)
        ));
        let cursor = QueueCursor {
            contract_version: MESSAGING_CONTRACT_VERSION,
            backend: super::super::QueueBackend::RedisStreams,
            namespace: "queue_a".into(),
            position: "1-0".into(),
        };
        assert!(matches!(
            validate_queue_cursor(
                Some(&cursor),
                super::super::QueueBackend::RedisStreams,
                "queue_b"
            ),
            Err(MessagingError::InvalidInput)
        ));
    }

    #[tokio::test]
    async fn redis_queue_replay_pages_retained_history_without_consuming() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let namespace = format!("replay_{}", unique_suffix());
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint,
            namespace,
            visibility_timeout_seconds: 2,
            max_messages: 10,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        for payload in [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()] {
            queue.enqueue(payload).await.unwrap();
        }
        let first = queue.replay_after(None, 2).await.unwrap();
        assert_eq!(
            first
                .records
                .iter()
                .map(|r| r.payload.as_slice())
                .collect::<Vec<_>>(),
            [b"one", b"two"]
        );
        let second = queue
            .replay_after(first.next_cursor.as_ref(), 2)
            .await
            .unwrap();
        assert_eq!(second.records.len(), 1);
        assert_eq!(second.records[0].payload, b"three");
        let replay = queue.replay_after(None, 10).await.unwrap();
        assert_eq!(replay.records.len(), 3);
        for expected in [b"one".as_slice(), b"two", b"three"] {
            let delivery = queue
                .receive("worker", "workers", Duration::from_secs(1))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(delivery.payload(), expected);
            delivery.ack().await.unwrap();
        }
        assert!(
            queue
                .receive("worker", "workers", Duration::from_millis(100))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn nats_queue_replay_pages_retained_history_without_consuming() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let namespace = format!("replay_{}", unique_suffix());
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint,
            namespace,
            visibility_timeout_seconds: 2,
            max_messages: 10,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        for payload in [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()] {
            queue.enqueue(payload).await.unwrap();
        }
        let first = queue.replay_after(None, 2).await.unwrap();
        assert_eq!(
            first
                .records
                .iter()
                .map(|r| r.payload.as_slice())
                .collect::<Vec<_>>(),
            [b"one", b"two"]
        );
        let second = queue
            .replay_after(first.next_cursor.as_ref(), 2)
            .await
            .unwrap();
        assert_eq!(second.records.len(), 1);
        assert_eq!(second.records[0].payload, b"three");
        let replay = queue.replay_after(None, 10).await.unwrap();
        assert_eq!(replay.records.len(), 3);
        for expected in [b"one".as_slice(), b"two", b"three"] {
            let delivery = queue
                .receive("worker", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(delivery.payload(), expected);
            delivery.ack().await.unwrap();
        }
        assert!(
            queue
                .receive("worker", "workers", Duration::from_millis(100))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn redis_queue_replay_rejects_cursor_removed_by_retention() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: endpoint.clone(),
            namespace: format!("replay_trim_{}", unique_suffix()),
            visibility_timeout_seconds: 2,
            max_messages: 2,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        queue.enqueue(b"one".to_vec()).await.unwrap();
        queue.enqueue(b"two".to_vec()).await.unwrap();
        let cursor = queue
            .replay_after(None, 1)
            .await
            .unwrap()
            .next_cursor
            .unwrap();
        queue.enqueue(b"three".to_vec()).await.unwrap();
        assert!(matches!(
            queue.replay_after(Some(&cursor), 10).await,
            Err(MessagingError::ReplayCursorExpired)
        ));

        let empty_queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint,
            namespace: format!("replay_empty_{}", unique_suffix()),
            visibility_timeout_seconds: 2,
            max_messages: 2,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        empty_queue.enqueue(b"removed".to_vec()).await.unwrap();
        let empty_cursor = empty_queue
            .replay_after(None, 1)
            .await
            .unwrap()
            .next_cursor
            .unwrap();
        if let QueueAdapter::Redis { queue, name, .. } = &empty_queue {
            let key = queue.stream_key(name).unwrap();
            let mut connection = queue.store.connection().await.unwrap();
            redis::cmd("XDEL")
                .arg(key)
                .arg(&empty_cursor.position)
                .query_async::<u64>(&mut connection)
                .await
                .unwrap();
        }
        assert!(matches!(
            empty_queue.replay_after(Some(&empty_cursor), 10).await,
            Err(MessagingError::ReplayCursorExpired)
        ));
    }

    #[tokio::test]
    async fn nats_queue_replay_rejects_cursor_removed_by_retention() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint,
            namespace: format!("replay_trim_{}", unique_suffix()),
            visibility_timeout_seconds: 2,
            max_messages: 2,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        queue.enqueue(b"one".to_vec()).await.unwrap();
        queue.enqueue(b"two".to_vec()).await.unwrap();
        let cursor = queue
            .replay_after(None, 1)
            .await
            .unwrap()
            .next_cursor
            .unwrap();
        queue.enqueue(b"three".to_vec()).await.unwrap();
        assert!(matches!(
            queue.replay_after(Some(&cursor), 10).await,
            Err(MessagingError::ReplayCursorExpired)
        ));
    }

    #[tokio::test]
    async fn redis_queue_replay_is_ordered_and_complete_after_concurrent_producers() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint,
            namespace: format!("replay_concurrent_{}", unique_suffix()),
            visibility_timeout_seconds: 2,
            max_messages: 100,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        let writes = (0..32).map(|n| {
            let queue = &queue;
            async move {
                queue
                    .enqueue(format!("job-{n}").into_bytes())
                    .await
                    .unwrap()
            }
        });
        futures_util::future::join_all(writes).await;

        let mut cursor = None;
        let mut positions = Vec::new();
        let mut payloads = std::collections::BTreeSet::new();
        loop {
            let page = queue.replay_after(cursor.as_ref(), 7).await.unwrap();
            let empty = page.records.is_empty();
            let next_cursor = page.next_cursor;
            for record in page.records {
                positions.push(record.cursor.position);
                payloads.insert(record.payload);
            }
            if empty || next_cursor == cursor {
                break;
            }
            cursor = next_cursor;
        }
        assert_eq!(positions.len(), 32);
        assert!(positions.windows(2).all(|ids| {
            compare_redis_stream_ids(&ids[0], &ids[1]).unwrap() == std::cmp::Ordering::Less
        }));
        assert_eq!(payloads.len(), 32);
    }

    #[tokio::test]
    async fn nats_queue_replay_is_ordered_and_complete_after_concurrent_producers() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint,
            namespace: format!("replay_concurrent_{}", unique_suffix()),
            visibility_timeout_seconds: 2,
            max_messages: 100,
            max_attempts: 5,
            deduplication_window_seconds: 7,
        })
        .await
        .unwrap();
        let writes = (0..32).map(|n| {
            let queue = &queue;
            async move {
                queue
                    .enqueue(format!("job-{n}").into_bytes())
                    .await
                    .unwrap()
            }
        });
        futures_util::future::join_all(writes).await;

        let mut cursor = None;
        let mut positions = Vec::new();
        let mut payloads = std::collections::BTreeSet::new();
        loop {
            let page = queue.replay_after(cursor.as_ref(), 7).await.unwrap();
            let empty = page.records.is_empty();
            let next_cursor = page.next_cursor;
            for record in page.records {
                positions.push(record.cursor.position.parse::<u64>().unwrap());
                payloads.insert(record.payload);
            }
            if empty || next_cursor == cursor {
                break;
            }
            cursor = next_cursor;
        }
        assert_eq!(positions.len(), 32);
        assert!(positions.windows(2).all(|ids| ids[0] < ids[1]));
        assert_eq!(payloads.len(), 32);
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
                deduplication_window_seconds: 120,
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
                deduplication_window_seconds: 120,
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
                deduplication_window_seconds: 120,
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
    async fn nack_schedules_redelivery_with_attempt_count_on_each_broker() {
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
                namespace: format!("nackq_{}", unique_suffix()),
                visibility_timeout_seconds: 2,
                max_messages: 100_000,
                max_attempts: 5,
                deduplication_window_seconds: 120,
            })
            .await
            .unwrap();
            queue.enqueue(b"retry-job".to_vec()).await.unwrap();
            let first = queue
                .receive("worker-a", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(first.attempts(), 1, "{backend:?}");
            first.nack(Duration::from_millis(300)).await.unwrap();

            assert!(
                queue
                    .receive("worker-b", "workers", Duration::from_millis(100))
                    .await
                    .unwrap()
                    .is_none(),
                "{backend:?}: a delayed NACK must not be immediately visible"
            );
            tokio::time::sleep(Duration::from_millis(350)).await;
            let retry = queue
                .receive("worker-b", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(retry.payload(), b"retry-job", "{backend:?}");
            assert_eq!(retry.attempts(), 2, "{backend:?}");
            retry.ack().await.unwrap();
        }
    }

    #[tokio::test]
    async fn idempotent_enqueue_returns_original_id_and_never_duplicates() {
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
                namespace: format!("dedupeq_{}", unique_suffix()),
                visibility_timeout_seconds: 2,
                max_messages: 100_000,
                max_attempts: 5,
                deduplication_window_seconds: 7,
            })
            .await
            .unwrap();
            assert!(matches!(
                queue
                    .enqueue_idempotent(b"invalid".to_vec(), "bad key")
                    .await,
                Err(MessagingError::InvalidInput)
            ));
            let original = queue
                .enqueue_idempotent(b"original".to_vec(), "job-42")
                .await
                .unwrap();
            let duplicate = queue
                .enqueue_idempotent(b"duplicate".to_vec(), "job-42")
                .await
                .unwrap();
            assert_eq!(duplicate, original, "{backend:?}");
            match &queue {
                QueueAdapter::Redis {
                    queue: redis_queue,
                    name,
                    ..
                } => {
                    let mut conn = redis_queue.store.connection().await.unwrap();
                    let ttl_ms: i64 = redis::cmd("PTTL")
                        .arg(format!(
                            "{}:dedupe:job-42",
                            redis_queue.stream_key(name).unwrap()
                        ))
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    assert!((0..=7_000).contains(&ttl_ms), "Redis TTL: {ttl_ms}ms");
                }
                QueueAdapter::Nats { queue: nats_queue } => {
                    let info = nats_queue
                        .ensure_stream()
                        .await
                        .unwrap()
                        .get_info()
                        .await
                        .unwrap();
                    assert_eq!(info.config.duplicate_window, Duration::from_secs(7));
                }
            }
            queue
                .enqueue_idempotent(b"independent".to_vec(), "job-43")
                .await
                .unwrap();

            let first = queue
                .receive("worker", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(first.payload(), b"original", "{backend:?}");
            first.ack().await.unwrap();
            let second = queue
                .receive("worker", "workers", Duration::from_secs(2))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(second.payload(), b"independent", "{backend:?}");
            second.ack().await.unwrap();
            assert!(
                queue
                    .receive("worker", "workers", Duration::from_millis(100))
                    .await
                    .unwrap()
                    .is_none(),
                "{backend:?}: duplicate enqueue must not create another delivery"
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

    #[tokio::test]
    async fn kv_cas_increment_prefix_watch_and_delete_match_across_backends() {
        let mut targets = Vec::new();
        if let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") {
            targets.push((super::super::KvBackend::Redis, url, "redis"));
        }
        if let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") {
            targets.push((super::super::KvBackend::NatsJetstream, url, "nats"));
        }
        for (backend, endpoint, label) in targets {
            let store = KvStoreAdapter::connect(KvStoreConfig {
                backend,
                endpoint,
                namespace: format!("CONFORMANCE_{}_{}", label, unique_suffix()),
            })
            .await
            .unwrap();
            store.put("job.b", b"1".to_vec(), None).await.unwrap();
            let first = store.get_with_revision("job.b").await.unwrap().unwrap();
            let (left, right) = tokio::join!(
                store.compare_and_set("job.b", &first.revision, b"2".to_vec(), None),
                store.compare_and_set("job.b", &first.revision, b"3".to_vec(), None),
            );
            let (left, right) = (left.unwrap(), right.unwrap());
            assert_eq!(usize::from(left) + usize::from(right), 1);
            assert_eq!(
                store.get("job.b").await.unwrap(),
                Some(if left { b"2".to_vec() } else { b"3".to_vec() })
            );
            assert!(
                !store
                    .compare_and_set("job.b", &first.revision, b"stale".to_vec(), None)
                    .await
                    .unwrap()
            );
            store
                .put(
                    "job.expiring",
                    b"short".to_vec(),
                    Some(Duration::from_secs(1)),
                )
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(1_200)).await;
            assert_eq!(store.get("job.expiring").await.unwrap(), None);

            let increments = futures_util::future::join_all(
                (0..8).map(|_| async { store.increment("job.counter", 1, None).await.unwrap() }),
            )
            .await;
            let mut increments = increments;
            increments.sort_unstable();
            assert_eq!(increments, (1..=8).collect::<Vec<_>>(), "{backend:?}");
            for value in &increments {
                store
                    .put(&format!("job.result.{value}"), b"done".to_vec(), None)
                    .await
                    .unwrap();
            }
            assert_eq!(store.increment("job.counter", -3, None).await.unwrap(), 5);
            assert_eq!(
                store.list_keys("job.result.", 10).await.unwrap(),
                (1..=8)
                    .map(|n| format!("job.result.{n}"))
                    .collect::<Vec<_>>()
            );
            assert_eq!(store.list_keys("job.", 1).await.unwrap(), vec!["job.b"]);

            let watch = store.watch("watch.").await.unwrap();
            store
                .put("watch.key", b"present".to_vec(), None)
                .await
                .unwrap();
            assert_eq!(
                watch.receive(Duration::from_secs(2)).await.unwrap(),
                Some(KvChange {
                    key: "watch.key".into(),
                    kind: KvChangeKind::Put,
                })
            );
            store.delete("watch.key").await.unwrap();
            assert_eq!(
                watch.receive(Duration::from_secs(2)).await.unwrap(),
                Some(KvChange {
                    key: "watch.key".into(),
                    kind: KvChangeKind::Remove,
                })
            );
            store
                .put(
                    "watch.expiring",
                    b"short".to_vec(),
                    Some(Duration::from_secs(1)),
                )
                .await
                .unwrap();
            assert_eq!(
                watch.receive(Duration::from_secs(2)).await.unwrap(),
                Some(KvChange {
                    key: "watch.expiring".into(),
                    kind: KvChangeKind::Put,
                })
            );
            assert_eq!(
                watch.receive(Duration::from_secs(3)).await.unwrap(),
                Some(KvChange {
                    key: "watch.expiring".into(),
                    kind: KvChangeKind::Remove,
                })
            );
            watch.close().await;
            store.delete("job.b").await.unwrap();
            store.delete("job.counter").await.unwrap();
            for n in 1..=8 {
                store.delete(&format!("job.result.{n}")).await.unwrap();
            }
            store.delete("job.expiring").await.unwrap();
            store.delete("watch.expiring").await.unwrap();
            assert_eq!(
                store.list_keys("job.", 100).await.unwrap(),
                Vec::<String>::new()
            );
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
            deduplication_window_seconds: 120,
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
            deduplication_window_seconds: 120,
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
            deduplication_window_seconds: 120,
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
            deduplication_window_seconds: 120,
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
    async fn nats_queue_preserves_operator_options_and_rejects_wrong_subjects() {
        use async_nats::jetstream::stream::{Config, RetentionPolicy, StorageType};
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let name = format!("preserve_{}", unique_suffix());
        let adapter = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url.clone(),
            namespace: name.clone(),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 5,
            deduplication_window_seconds: 120,
        })
        .await
        .unwrap();
        let QueueAdapter::Nats { queue } = &adapter else {
            unreachable!()
        };
        let client = async_nats::connect(&url).await.unwrap();
        let jetstream = async_nats::jetstream::new(client);
        jetstream
            .get_or_create_stream(Config {
                name: queue.stream.clone(),
                subjects: vec![queue.subject.clone()],
                max_messages: 50,
                max_bytes: 1_048_576,
                max_age: Duration::from_secs(300),
                retention: RetentionPolicy::Limits,
                storage: StorageType::Memory,
                ..Default::default()
            })
            .await
            .unwrap();
        adapter.enqueue(b"preserved".to_vec()).await.unwrap();
        let config = queue
            .ensure_stream()
            .await
            .unwrap()
            .get_info()
            .await
            .unwrap()
            .config;
        assert_eq!(config.max_messages, 10);
        assert_eq!(config.max_bytes, 1_048_576);
        assert_eq!(config.max_age, Duration::from_secs(300));
        assert_eq!(config.duplicate_window, Duration::from_secs(120));
        assert_eq!(config.retention, RetentionPolicy::Limits);
        assert_eq!(config.storage, StorageType::Memory);
        assert_eq!(config.subjects.len(), 1);
        assert_eq!(config.subjects[0], queue.subject);

        let incompatible_name = format!("wrong_{}", unique_suffix());
        let incompatible = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url,
            namespace: incompatible_name.clone(),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 5,
            deduplication_window_seconds: 120,
        })
        .await
        .unwrap();
        let QueueAdapter::Nats {
            queue: incompatible_queue,
        } = &incompatible
        else {
            unreachable!()
        };
        let original_subjects = vec![format!("operator.owned.{incompatible_name}")];
        jetstream
            .get_or_create_stream(Config {
                name: incompatible_queue.stream.clone(),
                subjects: original_subjects.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(matches!(
            incompatible.enqueue(b"must-not-publish".to_vec()).await,
            Err(MessagingError::Storage)
        ));
        let after = jetstream
            .get_stream(&incompatible_queue.stream)
            .await
            .unwrap()
            .get_info()
            .await
            .unwrap();
        assert_eq!(after.config.subjects, original_subjects);
    }

    #[tokio::test]
    async fn nats_receive_reserves_one_message_and_idle_pull_expires() {
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url,
            namespace: format!("one_pull_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 5,
            deduplication_window_seconds: 120,
        })
        .await
        .unwrap();
        for payload in [b"first".to_vec(), b"second".to_vec()] {
            queue.enqueue(payload).await.unwrap();
        }
        let first = queue
            .receive("worker", "workers", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.payload(), b"first");
        first.ack().await.unwrap();
        let second = queue
            .receive("worker", "workers", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.payload(), b"second");
        second.ack().await.unwrap();
        let started = tokio::time::Instant::now();
        assert!(
            queue
                .receive("worker", "workers", Duration::from_millis(200))
                .await
                .unwrap()
                .is_none()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn nats_dlq_transfer_retries_after_target_stream_is_repaired() {
        use async_nats::jetstream::stream::Config;
        let Ok(url) = std::env::var("VECTORA_TEST_NATS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::NatsJetstream,
            endpoint: url.clone(),
            namespace: format!("dlq_retry_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 1,
            deduplication_window_seconds: 120,
        })
        .await
        .unwrap();
        let QueueAdapter::Nats { queue: nats_queue } = &queue else {
            unreachable!()
        };
        let client = async_nats::connect(&url).await.unwrap();
        let jetstream = async_nats::jetstream::new(client);
        jetstream
            .get_or_create_stream(Config {
                name: nats_queue.dead_stream.clone(),
                subjects: vec!["operator.incompatible".into()],
                ..Default::default()
            })
            .await
            .unwrap();
        queue.enqueue(b"retry-me".to_vec()).await.unwrap();
        let first = queue
            .receive("worker", "workers", Duration::from_secs(2))
            .await
            .unwrap()
            .unwrap();
        drop(first);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(matches!(
            queue
                .receive("worker", "workers", Duration::from_secs(2))
                .await,
            Err(MessagingError::Storage)
        ));

        jetstream
            .delete_stream(&nats_queue.dead_stream)
            .await
            .unwrap();
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
        assert_eq!(dead.payload(), b"retry-me");
        dead.ack().await.unwrap();
        assert!(
            queue
                .receive_dead_letter("operator", "recovery", Duration::from_millis(200))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn redis_ack_racing_dead_letter_transfer_has_one_terminal_outcome() {
        let Ok(url) = std::env::var("VECTORA_TEST_REDIS_URL") else {
            return;
        };
        let queue = QueueAdapter::connect(QueueConfig {
            backend: super::super::QueueBackend::RedisStreams,
            endpoint: url,
            namespace: format!("dlq_race_{}", unique_suffix()),
            visibility_timeout_seconds: 1,
            max_messages: 10,
            max_attempts: 1,
            deduplication_window_seconds: 120,
        })
        .await
        .unwrap();
        let QueueAdapter::Redis {
            queue: redis_queue,
            name,
            ..
        } = &queue
        else {
            unreachable!()
        };
        queue.enqueue(b"race".to_vec()).await.unwrap();
        let first = queue
            .receive("worker", "workers", Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        let QueueDelivery::Redis(first) = first else {
            unreachable!()
        };
        let message_id = first.id.clone();
        drop(first);
        let key = redis_queue.stream_key(name).unwrap();
        let group = "workers";
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        let ack = async {
            let mut connection = redis_queue.store.connection().await.unwrap();
            redis::cmd("XACK")
                .arg(&key)
                .arg(group)
                .arg(&message_id)
                .query_async::<u64>(&mut connection)
                .await
                .unwrap()
        };
        let transfer = queue.receive("worker", group, Duration::from_millis(200));
        let (acknowledged, transfer_result) = tokio::join!(ack, transfer);
        assert!(transfer_result.is_ok());
        let dead = queue
            .receive_dead_letter("operator", "recovery", Duration::from_millis(300))
            .await
            .unwrap();
        match (acknowledged, dead) {
            (1, None) => {}
            (0, Some(dead)) => {
                assert_eq!(dead.payload(), b"race");
                dead.ack().await.unwrap();
                assert!(
                    queue
                        .receive_dead_letter("operator", "recovery", Duration::from_millis(100))
                        .await
                        .unwrap()
                        .is_none()
                );
            }
            _ => panic!("ACK/DLQ race must have exactly one terminal outcome"),
        }
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
            deduplication_window_seconds: 120,
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
            deduplication_window_seconds: 120,
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
