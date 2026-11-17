//! HTTP profile for the llama.cpp `llama-server` API.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    pin::Pin,
    time::Duration,
};

use base64::Engine as _;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::{Client, Response, header};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout_at};
use tokio_websockets::{ClientBuilder, Limits};
use url::Url;

use super::decision_model::answer_matches;
use super::{
    CapabilityState, ChatCompletion, ChatError, ChatRequest, ChatStream, ContentPart, ContentSite,
    DecisionRequest, DecisionResponse, EphemeralDecisionRequest, EphemeralNativeSpeechRequest,
    MediaExecutionContext, MediaKind, MediaMode, ModalityCapabilities, NativeSpeechRequest,
    OpenAiCompatibleClient, OpenAiCompatibleConfig, RealtimeIntent, RealtimeSession,
    RealtimeSessionRequest, SpeechFormat, SpeechRequest, SpeechResponse, SpeechStreamInfo,
    prepare_content_with_limits,
};
use crate::llm::embeddings::{EmbeddingRequest, EmbeddingResponse, LlmUsage};

const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_READ_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOTAL_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const SPEECH_STREAM_CHUNK_BYTES: usize = 64 * 1024;

/// Incremental binary TTS response. Dropping the value cancels the HTTP body.
pub struct SpeechStream {
    body: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    pending: VecDeque<u8>,
    info: SpeechStreamInfo,
    deadline: Instant,
    read_timeout: Duration,
    max_bytes: usize,
    total_bytes: usize,
    received_any: bool,
    finished: bool,
}

impl SpeechStream {
    pub fn info(&self) -> &SpeechStreamInfo {
        &self.info
    }

    /// Returns the next bounded audio chunk, or `None` after a non-empty stream.
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ChatError> {
        if !self.pending.is_empty() {
            return Ok(Some(take_audio_chunk(&mut self.pending)));
        }
        if self.finished {
            return Ok(None);
        }
        loop {
            let read_deadline = (Instant::now() + self.read_timeout).min(self.deadline);
            let next = timeout_at(read_deadline, self.body.next())
                .await
                .map_err(|_| ChatError::Timeout)?;
            match next {
                Some(Ok(bytes)) if bytes.is_empty() => continue,
                Some(Ok(bytes)) => {
                    self.total_bytes = self
                        .total_bytes
                        .checked_add(bytes.len())
                        .filter(|total| *total <= self.max_bytes)
                        .ok_or(ChatError::ResponseTooLarge)?;
                    self.received_any = true;
                    self.pending.extend(bytes);
                    return Ok(Some(take_audio_chunk(&mut self.pending)));
                }
                Some(Err(error)) => {
                    return Err(if error.is_timeout() {
                        ChatError::Timeout
                    } else {
                        ChatError::Connection
                    });
                }
                None => {
                    self.finished = true;
                    if !self.received_any {
                        return Err(ChatError::InvalidResponse);
                    }
                    return Ok(None);
                }
            }
        }
    }
}

fn take_audio_chunk(pending: &mut VecDeque<u8>) -> Vec<u8> {
    let count = pending.len().min(SPEECH_STREAM_CHUNK_BYTES);
    pending.drain(..count).collect()
}

/// Settings for a `llama-server` endpoint. `base_url` may end in `/v1`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlamaServerSettings {
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    pub total_timeout_ms: u64,
    #[serde(default = "default_retry_connect_attempts")]
    pub retry_connect_attempts: u8,
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: usize,
    #[serde(default = "default_max_media_bytes")]
    pub max_media_bytes: u64,
    #[serde(default = "default_max_event_bytes")]
    pub max_event_bytes: usize,
}

const fn default_connect_timeout_ms() -> u64 {
    DEFAULT_CONNECT_TIMEOUT_MS
}
const fn default_read_timeout_ms() -> u64 {
    DEFAULT_READ_TIMEOUT_MS
}
const fn default_total_timeout_ms() -> u64 {
    DEFAULT_TOTAL_TIMEOUT_MS
}
const fn default_max_response_bytes() -> usize {
    DEFAULT_MAX_RESPONSE_BYTES
}
const fn default_max_media_bytes() -> u64 {
    16 * 1024 * 1024
}
const fn default_max_event_bytes() -> usize {
    1024 * 1024
}
const fn default_retry_connect_attempts() -> u8 {
    2
}

impl fmt::Debug for LlamaServerSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlamaServerSettings")
            .field("base_url", &safe_url(&self.base_url))
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("read_timeout_ms", &self.read_timeout_ms)
            .field("total_timeout_ms", &self.total_timeout_ms)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

/// Modality labels reported by llama-server. Unknown labels are retained.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LlamaServerArchitecture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_modalities: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One model as reported by `GET /v1/models`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LlamaServerModel {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<LlamaServerArchitecture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Model list returned by the OpenAI-compatible `GET /v1/models` route.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LlamaServerModelList {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    pub data: Vec<LlamaServerModel>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One ordered embedding item. Assets are referenced, never inlined in a
/// persistable request.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LlamaServerEmbeddingInput {
    pub content: Vec<ContentPart>,
}

/// Transient multimodal embedding request, materialized only for one call.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EphemeralLlamaServerEmbeddingRequest {
    pub contract_version: u16,
    pub model: String,
    pub inputs: Vec<LlamaServerEmbeddingInput>,
}

/// Result of a successful `GET /health` probe.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlamaServerHealth {
    pub status: String,
}

/// Model and endpoint properties returned by `GET /props`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LlamaServerProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voices: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modalities: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// llama.cpp server profile: health and model discovery plus shared chat transport.
pub struct LlamaServerClient {
    root_url: Url,
    v1_url: Url,
    client: Client,
    api_key: Option<header::HeaderValue>,
    headers: Vec<(header::HeaderName, header::HeaderValue)>,
    total_timeout: Duration,
    read_timeout: Duration,
    max_response_bytes: usize,
    max_media_bytes: u64,
    max_event_bytes: usize,
    chat: OpenAiCompatibleClient,
}

impl fmt::Debug for LlamaServerClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlamaServerClient")
            .field("root_url", &self.root_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .finish_non_exhaustive()
    }
}

impl LlamaServerClient {
    pub fn new(settings: LlamaServerSettings) -> Result<Self, ChatError> {
        if settings.connect_timeout_ms == 0
            || settings.read_timeout_ms == 0
            || settings.total_timeout_ms == 0
            || settings.max_response_bytes == 0
            || settings.max_media_bytes == 0
            || settings.max_event_bytes == 0
            || settings.max_event_bytes > settings.max_response_bytes
            || settings.retry_connect_attempts > 5
        {
            return Err(ChatError::InvalidConfiguration(
                "timeouts and response limit must be positive",
            ));
        }
        let mut root_url = Url::parse(settings.base_url.trim()).map_err(|_| {
            ChatError::InvalidConfiguration("base URL must be an absolute HTTP URL")
        })?;
        if !matches!(root_url.scheme(), "http" | "https")
            || root_url.host_str().is_none()
            || !root_url.username().is_empty()
            || root_url.password().is_some()
            || root_url.query().is_some()
            || root_url.fragment().is_some()
        {
            return Err(ChatError::InvalidConfiguration(
                "base URL is not a safe HTTP endpoint",
            ));
        }
        let path = root_url.path().trim_end_matches('/');
        let path = path.strip_suffix("/v1").unwrap_or(path).to_owned();
        root_url.set_path(&path);
        let mut v1_url = root_url.clone();
        v1_url.set_path(&format!("{}/v1", root_url.path().trim_end_matches('/')));

        let api_key = match settings.api_key.as_deref() {
            Some(key) if key.trim().is_empty() => {
                return Err(ChatError::InvalidConfiguration("API key must not be empty"));
            }
            Some(key) => Some(
                header::HeaderValue::from_str(&format!("Bearer {}", key.trim())).map_err(|_| {
                    ChatError::InvalidConfiguration("API key is not a valid header value")
                })?,
            ),
            None => None,
        };
        let mut headers = Vec::with_capacity(settings.headers.len());
        for (name, value) in &settings.headers {
            let name = header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header name"))?;
            let value = header::HeaderValue::from_str(value)
                .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header value"))?;
            if matches!(name.as_str(), "authorization" | "content-type" | "host") {
                return Err(ChatError::InvalidConfiguration(
                    "reserved HTTP header cannot be overridden",
                ));
            }
            headers.push((name, value));
        }

        let connect_timeout = Duration::from_millis(settings.connect_timeout_ms);
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ChatError::InvalidConfiguration("HTTP client could not be created"))?;

        let mut chat_config = OpenAiCompatibleConfig::new(v1_url.as_str())
            .media_provider("llama_cpp")
            .timeouts(
                connect_timeout,
                Duration::from_millis(settings.read_timeout_ms),
                Duration::from_millis(settings.total_timeout_ms),
            )
            .retry_connect_attempts(settings.retry_connect_attempts)
            .response_limits(settings.max_response_bytes, settings.max_event_bytes);
        if let Some(key) = &settings.api_key {
            chat_config = chat_config.api_key(key);
        }
        for (name, value) in &settings.headers {
            chat_config = chat_config.header(name, value);
        }
        let chat = OpenAiCompatibleClient::new(chat_config)?;

        Ok(Self {
            root_url,
            v1_url,
            client,
            api_key,
            headers,
            total_timeout: Duration::from_millis(settings.total_timeout_ms),
            read_timeout: Duration::from_millis(settings.read_timeout_ms),
            max_response_bytes: settings.max_response_bytes,
            max_media_bytes: settings.max_media_bytes,
            max_event_bytes: settings.max_event_bytes,
            chat,
        })
    }

    pub async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        self.chat.complete(request).await
    }

    pub async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        self.chat.stream(request).await
    }

    /// Answers typed questions via llama-server v1.0.0 `/v1/systemone`.
    /// The request is deliberately non-streaming, matching the upstream API.
    pub async fn decide(&self, request: DecisionRequest) -> Result<DecisionResponse, ChatError> {
        self.decide_ephemeral(EphemeralDecisionRequest {
            request,
            files: Vec::new(),
        })
        .await
    }

    /// Sends a transient decision request with in-memory media data URLs.
    pub async fn decide_ephemeral(
        &self,
        request: EphemeralDecisionRequest,
    ) -> Result<DecisionResponse, ChatError> {
        request
            .validate(self.max_media_bytes)
            .map_err(ChatError::InvalidRequest)?;
        let models = self.models().await?;
        let selected = models
            .data
            .iter()
            .find(|model| model.id == request.request.model)
            .ok_or(ChatError::ModelCapabilityUnsupported)?;
        if !selected
            .architecture
            .as_ref()
            .and_then(|architecture| architecture.output_modalities.as_ref())
            .is_some_and(|modalities| modalities.iter().any(|value| value == "decisions"))
        {
            return Err(ChatError::ModelCapabilityUnsupported);
        }
        let input_modalities = selected
            .architecture
            .as_ref()
            .and_then(|architecture| architecture.input_modalities.as_ref());
        for file in &request.files {
            let modality = if file.starts_with("data:image/") {
                "image"
            } else {
                "audio"
            };
            if !input_modalities.is_some_and(|values| values.iter().any(|value| value == modality))
            {
                return Err(ChatError::ModelCapabilityUnsupported);
            }
        }
        let mut endpoint = self.v1_url.clone();
        endpoint.set_path(&format!(
            "{}/systemone",
            endpoint.path().trim_end_matches('/')
        ));
        let body = serde_json::to_vec(&serde_json::json!({
            "state": request.request.state,
            "files": request.files,
            "questions": request.request.questions,
        }))
        .map_err(|_| ChatError::InvalidRequest("decision request could not be encoded"))?;
        if body.len() > self.max_response_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        let deadline = Instant::now() + self.total_timeout;
        let mut http = self
            .client
            .post(endpoint)
            .timeout(self.read_timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(key) = &self.api_key {
            http = http.header(header::AUTHORIZATION, key.clone());
        }
        for (name, value) in &self.headers {
            http = http.header(name, value);
        }
        let response = timeout_at(deadline, http.send())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    ChatError::Timeout
                } else {
                    ChatError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(classify_status(response.status().as_u16()));
        }
        let bytes = read_limited(response, self.max_response_bytes, deadline).await?;
        let response: DecisionResponse =
            serde_json::from_slice(&bytes).map_err(|_| ChatError::InvalidResponse)?;
        if response.answers.len() != request.request.questions.len()
            || request.request.questions.iter().any(|(id, question)| {
                response
                    .answers
                    .get(id)
                    .is_none_or(|answer| !answer_matches(question, answer))
            })
        {
            return Err(ChatError::InvalidResponse);
        }
        Ok(response)
    }

    pub async fn complete_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatCompletion, ChatError> {
        self.chat.complete_with_media(request, context).await
    }

    pub async fn stream_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatStream, ChatError> {
        self.chat.stream_with_media(request, context).await
    }

    pub async fn health(&self) -> Result<LlamaServerHealth, ChatError> {
        let mut endpoint = self.root_url.clone();
        endpoint.set_path(&format!("{}/health", endpoint.path().trim_end_matches('/')));
        let response = self.get(endpoint).await?;
        let value: LlamaServerHealth =
            serde_json::from_slice(&response).map_err(|_| ChatError::InvalidResponse)?;
        Ok(value)
    }

    pub async fn models(&self) -> Result<LlamaServerModelList, ChatError> {
        let mut endpoint = self.v1_url.clone();
        endpoint.set_path(&format!("{}/models", endpoint.path().trim_end_matches('/')));
        let response = self.get(endpoint).await?;
        serde_json::from_slice(&response).map_err(|_| ChatError::InvalidResponse)
    }

    /// Creates text embeddings through llama-server v1.0.0 `/v1/embeddings`.
    /// Multimodal input uses [`Self::embed_ephemeral`] and a consumer-owned asset store.
    pub async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ChatError> {
        if request.contract_version != crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION
            || request.model.trim().is_empty()
            || request.texts.iter().any(|text| text.trim().is_empty())
        {
            return Err(ChatError::InvalidRequest("invalid embedding request"));
        }
        self.post_embeddings(
            request.model,
            serde_json::json!(request.texts),
            request.texts.len(),
        )
        .await
    }

    /// Creates multimodal embeddings from short-lived, consumer-owned assets.
    /// Only data already present in the supplied `MediaExecutionContext` is
    /// materialized; remote URLs, paths, and inline base64 are rejected.
    pub async fn embed_ephemeral(
        &self,
        request: EphemeralLlamaServerEmbeddingRequest,
        context: &MediaExecutionContext,
    ) -> Result<EmbeddingResponse, ChatError> {
        if request.contract_version != crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION
            || request.model.trim().is_empty()
        {
            return Err(ChatError::InvalidRequest(
                "invalid ephemeral embedding request",
            ));
        }
        if request.inputs.is_empty() {
            return self
                .post_embeddings(request.model, serde_json::json!([]), 0)
                .await;
        }
        let expected_count = request.inputs.len();
        if request.inputs.iter().any(|input| {
            input.content.is_empty()
                || input.content.iter().any(|part| {
                    matches!(
                        part,
                        ContentPart::ImageUrl { .. }
                            | ContentPart::Inline(_)
                            | ContentPart::Document(_)
                            | ContentPart::File(_)
                    )
                })
        }) {
            return Err(ChatError::InvalidRequest(
                "embedding inputs require text or consumer asset references",
            ));
        }
        let model = self
            .models()
            .await?
            .data
            .into_iter()
            .find(|candidate| candidate.id == request.model)
            .ok_or(ChatError::ModelCapabilityUnsupported)?;
        let modalities = model
            .architecture
            .as_ref()
            .and_then(|architecture| architecture.input_modalities.as_ref());
        let supports =
            |kind: &str| modalities.is_some_and(|items| items.iter().any(|item| item == kind));
        let capabilities = ModalityCapabilities {
            vision: if supports("image") {
                CapabilityState::Supported
            } else {
                CapabilityState::Unsupported
            },
            pdf: CapabilityState::Unsupported,
            audio_input: if supports("audio") {
                CapabilityState::Supported
            } else {
                CapabilityState::Unsupported
            },
            video_input: if supports("video") {
                CapabilityState::Supported
            } else {
                CapabilityState::Unsupported
            },
            tool_result_images: CapabilityState::Unsupported,
        };
        let mut inputs = Vec::with_capacity(request.inputs.len());
        for input in request.inputs {
            let prepared = prepare_content_with_limits(
                input.content,
                &capabilities,
                context.store.as_ref(),
                &request.model,
                ContentSite::UserMessage,
                context.max_media_bytes.min(self.max_media_bytes),
                context
                    .resolver
                    .media_limits_for("llama_cpp", &request.model),
            )
            .await;
            if prepared
                .decisions
                .iter()
                .any(|decision| decision.mode != MediaMode::Native)
            {
                return Err(ChatError::ModelCapabilityUnsupported);
            }
            let mut content = Vec::with_capacity(prepared.parts.len());
            for part in prepared.parts {
                match part {
                    ContentPart::Text { text } if !text.trim().is_empty() => content.push(serde_json::json!({"type":"text","text":text})),
                    ContentPart::ImageUrl { image_url } if image_url.url.starts_with("data:image/") => content.push(serde_json::json!({"type":"image_url","image_url":{"url":image_url.url}})),
                    ContentPart::Inline(media) if media.kind == MediaKind::Audio => content.push(serde_json::json!({"type":"input_audio","input_audio":{"data":media.data_base64}})),
                    ContentPart::Inline(media) if media.kind == MediaKind::Video => content.push(serde_json::json!({"type":"input_video","input_video":{"data":media.data_base64}})),
                    _ => return Err(ChatError::InvalidRequest("unsupported or empty embedding content part")),
                }
            }
            inputs.push(serde_json::json!({"content":content}));
        }
        self.post_embeddings(request.model, serde_json::json!(inputs), expected_count)
            .await
    }

    async fn post_embeddings(
        &self,
        model: String,
        input: serde_json::Value,
        expected_count: usize,
    ) -> Result<EmbeddingResponse, ChatError> {
        if expected_count == 0 {
            return Ok(EmbeddingResponse {
                contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
                embeddings: Vec::new(),
                dimensions: 0,
                usage: None,
                selected_model_id: None,
                model_switches: Vec::new(),
            });
        }
        let mut endpoint = self.v1_url.clone();
        endpoint.set_path(&format!(
            "{}/embeddings",
            endpoint.path().trim_end_matches('/')
        ));
        let body = serde_json::to_vec(
            &serde_json::json!({"model":model,"input":input,"encoding_format":"float"}),
        )
        .map_err(|_| ChatError::InvalidRequest("embedding request could not be encoded"))?;
        if body.len() > self.max_response_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        let deadline = Instant::now() + self.total_timeout;
        let mut http = self
            .client
            .post(endpoint)
            .timeout(self.read_timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(key) = &self.api_key {
            http = http.header(header::AUTHORIZATION, key.clone());
        }
        for (name, value) in &self.headers {
            http = http.header(name, value);
        }
        let response = timeout_at(deadline, http.send())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    ChatError::Timeout
                } else {
                    ChatError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(classify_status(response.status().as_u16()));
        }
        let bytes = read_limited(response, self.max_response_bytes, deadline).await?;
        #[derive(Deserialize)]
        struct Data {
            index: usize,
            embedding: Vec<f32>,
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: u64,
        }
        #[derive(Deserialize)]
        struct Raw {
            data: Vec<Data>,
            #[serde(default)]
            usage: Option<Usage>,
        }
        let raw: Raw = serde_json::from_slice(&bytes).map_err(|_| ChatError::InvalidResponse)?;
        if raw.data.len() != expected_count {
            return Err(ChatError::InvalidResponse);
        }
        let mut vectors = vec![None; expected_count];
        let mut dimensions = None;
        for item in raw.data {
            if item.index >= vectors.len()
                || vectors[item.index].is_some()
                || item.embedding.is_empty()
                || item.embedding.iter().any(|value| !value.is_finite())
                || dimensions.is_some_and(|size| size != item.embedding.len())
            {
                return Err(ChatError::InvalidResponse);
            }
            dimensions = Some(item.embedding.len());
            vectors[item.index] = Some(item.embedding);
        }
        let embeddings = vectors
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(ChatError::InvalidResponse)?;
        Ok(EmbeddingResponse {
            contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
            dimensions: dimensions.unwrap_or_default() as u32,
            embeddings,
            usage: raw.usage.map(|usage| LlmUsage {
                input_tokens: usage.prompt_tokens,
                cost: None,
            }),
            selected_model_id: Some(model),
            model_switches: Vec::new(),
        })
    }

    /// Returns server properties such as model-specific TTS voice names.
    pub async fn properties(&self) -> Result<LlamaServerProperties, ChatError> {
        let mut endpoint = self.root_url.clone();
        endpoint.set_path(&format!("{}/props", endpoint.path().trim_end_matches('/')));
        let response = self.get(endpoint).await?;
        serde_json::from_slice(&response).map_err(|_| ChatError::InvalidResponse)
    }

    /// Opens the `/v1/realtime` WebSocket for transcription or conversation.
    pub async fn connect_realtime(
        &self,
        request: RealtimeSessionRequest,
    ) -> Result<RealtimeSession, ChatError> {
        request.validate().map_err(ChatError::InvalidRequest)?;
        let models = self.models().await?;
        let selected = models
            .data
            .iter()
            .find(|model| model.id == request.model)
            .ok_or(ChatError::ModelCapabilityUnsupported)?;
        if !selected
            .architecture
            .as_ref()
            .and_then(|architecture| architecture.input_modalities.as_ref())
            .is_some_and(|modalities| modalities.iter().any(|value| value == "audio"))
        {
            return Err(ChatError::ModelCapabilityUnsupported);
        }

        let mut endpoint = self.v1_url.clone();
        let scheme = match endpoint.scheme() {
            "http" => "ws",
            "https" => "wss",
            _ => {
                return Err(ChatError::InvalidConfiguration(
                    "unsupported realtime URL scheme",
                ));
            }
        };
        endpoint
            .set_scheme(scheme)
            .map_err(|_| ChatError::InvalidConfiguration("invalid realtime URL scheme"))?;
        endpoint.set_path("/v1/realtime");
        {
            let mut query = endpoint.query_pairs_mut();
            query.append_pair("model", &request.model);
            if request.intent == RealtimeIntent::Transcription {
                query.append_pair("intent", "transcription");
            }
        }
        let mut builder = ClientBuilder::new()
            .uri(endpoint.as_str())
            .map_err(|_| ChatError::InvalidConfiguration("invalid realtime WebSocket URI"))?
            .limits(Limits::default().max_payload_len(Some(self.max_event_bytes)));
        if let Some(key) = &self.api_key {
            builder = builder
                .add_header(header::AUTHORIZATION, key.clone())
                .map_err(|_| {
                    ChatError::InvalidConfiguration("invalid realtime authorization header")
                })?;
        }
        for (name, value) in &self.headers {
            builder = builder
                .add_header(name.clone(), value.clone())
                .map_err(|_| ChatError::InvalidConfiguration("invalid realtime request header"))?;
        }
        let (socket, _handshake) = tokio::time::timeout(self.total_timeout, builder.connect())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|_| ChatError::Connection)?;
        let mut session = RealtimeSession::new(
            socket,
            self.read_timeout,
            self.max_event_bytes,
            usize::try_from(self.max_media_bytes).unwrap_or(usize::MAX),
        );
        if let Some(rate) = request.input_sample_rate_hz {
            session.set_input_sample_rate(rate).await?;
        }
        Ok(session)
    }

    /// OpenAI-compatible, buffered speech synthesis at `/v1/audio/speech`.
    pub async fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResponse, ChatError> {
        request.validate().map_err(ChatError::InvalidRequest)?;
        if request.format == SpeechFormat::PcmF32 {
            return Err(ChatError::InvalidRequest(
                "the OpenAI-compatible speech route only supports wav or pcm16",
            ));
        }
        self.ensure_audio_model(&request.model).await?;
        self.ensure_voice(request.voice.as_deref()).await?;
        let mut endpoint = self.v1_url.clone();
        endpoint.set_path(&format!(
            "{}/audio/speech",
            endpoint.path().trim_end_matches('/')
        ));
        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            "voice": request.voice,
            "response_format": match request.format { SpeechFormat::Wav => "wav", _ => "pcm" },
            "speed": request.speed,
        });
        self.post_speech(endpoint, omit_null_fields(body), request.format)
            .await
    }

    /// OpenAI-compatible TTS streamed as bounded binary chunks.
    pub async fn synthesize_stream(
        &self,
        request: SpeechRequest,
    ) -> Result<SpeechStream, ChatError> {
        request.validate().map_err(ChatError::InvalidRequest)?;
        if request.format == SpeechFormat::PcmF32 {
            return Err(ChatError::InvalidRequest(
                "the OpenAI-compatible speech route only supports wav or pcm16",
            ));
        }
        self.ensure_audio_model(&request.model).await?;
        self.ensure_voice(request.voice.as_deref()).await?;
        let mut endpoint = self.v1_url.clone();
        endpoint.set_path(&format!(
            "{}/audio/speech",
            endpoint.path().trim_end_matches('/')
        ));
        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            "voice": request.voice,
            "response_format": match request.format { SpeechFormat::Wav => "wav", _ => "pcm" },
            "speed": request.speed,
            "stream_format": "audio",
        });
        self.post_speech_stream(endpoint, omit_null_fields(body), request.format)
            .await
    }

    /// Native `/tts` exposes model-specific language, sampling and PCM options.
    pub async fn synthesize_native(
        &self,
        request: NativeSpeechRequest,
    ) -> Result<SpeechResponse, ChatError> {
        self.synthesize_native_ephemeral(EphemeralNativeSpeechRequest {
            request,
            speaker_ref_b64: None,
        })
        .await
    }

    /// Native `/tts` stream with text and model-specific controls.
    pub async fn synthesize_native_stream(
        &self,
        request: NativeSpeechRequest,
    ) -> Result<SpeechStream, ChatError> {
        self.synthesize_native_stream_ephemeral(EphemeralNativeSpeechRequest {
            request,
            speaker_ref_b64: None,
        })
        .await
    }

    /// Native `/tts` stream with request-only speaker reference audio.
    pub async fn synthesize_native_stream_ephemeral(
        &self,
        request: EphemeralNativeSpeechRequest,
    ) -> Result<SpeechStream, ChatError> {
        request
            .request
            .request
            .validate()
            .map_err(ChatError::InvalidRequest)?;
        if let Some(reference) = &request.speaker_ref_b64 {
            if request.request.request.voice.is_some() {
                return Err(ChatError::InvalidRequest(
                    "predefined voice and speaker reference cannot be combined",
                ));
            }
            if reference.len() as u64 > self.max_media_bytes.saturating_add(2) / 3 * 4 + 4 {
                return Err(ChatError::InvalidRequest(
                    "speaker reference exceeds the configured media limit",
                ));
            }
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(reference)
                .map_err(|_| ChatError::InvalidRequest("speaker reference must be valid base64"))?;
            if decoded.is_empty() || decoded.len() as u64 > self.max_media_bytes {
                return Err(ChatError::InvalidRequest(
                    "speaker reference is empty or exceeds the configured media limit",
                ));
            }
        }
        self.ensure_audio_model(&request.request.request.model)
            .await?;
        self.ensure_voice(request.request.request.voice.as_deref())
            .await?;
        let mut endpoint = self.root_url.clone();
        endpoint.set_path(&format!("{}/tts", endpoint.path().trim_end_matches('/')));
        let speech = &request.request.request;
        let body = serde_json::json!({
            "input": speech.input,
            "lang": request.request.lang,
            "voice": speech.voice,
            "speaker_ref_b64": request.speaker_ref_b64,
            "top_k": request.request.top_k,
            "top_p": request.request.top_p,
            "temperature": request.request.temperature,
            "repeat_penalty": request.request.repeat_penalty,
            "seed": request.request.seed,
            "n_predict": request.request.n_predict,
            "response_format": match speech.format {
                SpeechFormat::Wav => "wav",
                SpeechFormat::Pcm16 => "pcm16",
                SpeechFormat::PcmF32 => "pcm",
            },
            "stream": true,
        });
        self.post_speech_stream(endpoint, omit_null_fields(body), speech.format)
            .await
    }

    /// Native `/tts` with transient audio used for supported voice cloning.
    pub async fn synthesize_native_ephemeral(
        &self,
        request: EphemeralNativeSpeechRequest,
    ) -> Result<SpeechResponse, ChatError> {
        request
            .request
            .request
            .validate()
            .map_err(ChatError::InvalidRequest)?;
        if let Some(reference) = &request.speaker_ref_b64 {
            if request.request.request.voice.is_some() {
                return Err(ChatError::InvalidRequest(
                    "predefined voice and speaker reference cannot be combined",
                ));
            }
            if reference.len() as u64 > self.max_media_bytes.saturating_add(2) / 3 * 4 + 4 {
                return Err(ChatError::InvalidRequest(
                    "speaker reference exceeds the configured media limit",
                ));
            }
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(reference)
                .map_err(|_| ChatError::InvalidRequest("speaker reference must be valid base64"))?;
            if decoded.is_empty() || decoded.len() as u64 > self.max_media_bytes {
                return Err(ChatError::InvalidRequest(
                    "speaker reference is empty or exceeds the configured media limit",
                ));
            }
        }
        self.ensure_audio_model(&request.request.request.model)
            .await?;
        self.ensure_voice(request.request.request.voice.as_deref())
            .await?;
        let mut endpoint = self.root_url.clone();
        endpoint.set_path(&format!("{}/tts", endpoint.path().trim_end_matches('/')));
        let speech = &request.request.request;
        let body = serde_json::json!({
            "input": speech.input,
            "lang": request.request.lang,
            "voice": speech.voice,
            "speaker_ref_b64": request.speaker_ref_b64,
            "top_k": request.request.top_k,
            "top_p": request.request.top_p,
            "temperature": request.request.temperature,
            "repeat_penalty": request.request.repeat_penalty,
            "seed": request.request.seed,
            "n_predict": request.request.n_predict,
            "response_format": match speech.format {
                SpeechFormat::Wav => "wav",
                SpeechFormat::Pcm16 => "pcm16",
                SpeechFormat::PcmF32 => "pcm",
            },
            "stream": false,
        });
        self.post_speech(endpoint, omit_null_fields(body), speech.format)
            .await
    }

    async fn ensure_audio_model(&self, model_id: &str) -> Result<(), ChatError> {
        let models = self.models().await?;
        let selected = models
            .data
            .iter()
            .find(|model| model.id == model_id)
            .ok_or(ChatError::ModelCapabilityUnsupported)?;
        if selected
            .architecture
            .as_ref()
            .and_then(|architecture| architecture.output_modalities.as_ref())
            .is_some_and(|modalities| modalities.iter().any(|value| value == "audio"))
        {
            Ok(())
        } else {
            Err(ChatError::ModelCapabilityUnsupported)
        }
    }

    async fn ensure_voice(&self, voice: Option<&str>) -> Result<(), ChatError> {
        if let Some(voice) = voice {
            let properties = self.properties().await?;
            if !properties
                .voices
                .is_some_and(|voices| voices.iter().any(|available| available == voice))
            {
                return Err(ChatError::InvalidRequest(
                    "voice is not listed by the selected llama-server model",
                ));
            }
        }
        Ok(())
    }

    async fn post_speech(
        &self,
        endpoint: Url,
        body: serde_json::Value,
        format: SpeechFormat,
    ) -> Result<SpeechResponse, ChatError> {
        let body = serde_json::to_vec(&body)
            .map_err(|_| ChatError::InvalidRequest("speech request could not be encoded"))?;
        if body.len() > self.max_response_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        let deadline = Instant::now() + self.total_timeout;
        let mut http = self
            .client
            .post(endpoint)
            .timeout(self.read_timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(key) = &self.api_key {
            http = http.header(header::AUTHORIZATION, key.clone());
        }
        for (name, value) in &self.headers {
            http = http.header(name, value);
        }
        let response = timeout_at(deadline, http.send())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    ChatError::Timeout
                } else {
                    ChatError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(classify_status(response.status().as_u16()));
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or(ChatError::InvalidResponse)?
            .to_owned();
        if !content_type
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().to_ascii_lowercase().starts_with("audio/"))
        {
            return Err(ChatError::InvalidResponse);
        }
        let audio = read_limited(response, self.max_response_bytes, deadline).await?;
        if audio.is_empty() {
            return Err(ChatError::InvalidResponse);
        }
        let sample_rate_hz = sample_rate(&content_type).or_else(|| wav_sample_rate(&audio));
        Ok(SpeechResponse {
            audio,
            content_type,
            format,
            sample_rate_hz,
        })
    }

    async fn post_speech_stream(
        &self,
        endpoint: Url,
        body: serde_json::Value,
        format: SpeechFormat,
    ) -> Result<SpeechStream, ChatError> {
        let body = serde_json::to_vec(&body)
            .map_err(|_| ChatError::InvalidRequest("speech request could not be encoded"))?;
        if body.len() > self.max_response_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        let deadline = Instant::now() + self.total_timeout;
        let mut http = self
            .client
            .post(endpoint)
            .timeout(self.total_timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(key) = &self.api_key {
            http = http.header(header::AUTHORIZATION, key.clone());
        }
        for (name, value) in &self.headers {
            http = http.header(name, value);
        }
        let response = timeout_at(deadline, http.send())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    ChatError::Timeout
                } else {
                    ChatError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(classify_status(response.status().as_u16()));
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or(ChatError::InvalidResponse)?
            .to_owned();
        if !content_type
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().to_ascii_lowercase().starts_with("audio/"))
        {
            return Err(ChatError::InvalidResponse);
        }
        let sample_rate_hz = sample_rate(&content_type);
        Ok(SpeechStream {
            body: Box::pin(response.bytes_stream()),
            pending: VecDeque::new(),
            info: SpeechStreamInfo {
                content_type,
                format,
                sample_rate_hz,
            },
            deadline,
            read_timeout: self.read_timeout,
            max_bytes: self.max_response_bytes,
            total_bytes: 0,
            received_any: false,
            finished: false,
        })
    }

    async fn get(&self, endpoint: Url) -> Result<Vec<u8>, ChatError> {
        let deadline = Instant::now() + self.total_timeout;
        let mut request = self.client.get(endpoint).timeout(self.read_timeout);
        if let Some(key) = &self.api_key {
            request = request.header(header::AUTHORIZATION, key.clone());
        }
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let response = timeout_at(deadline, request.send())
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    ChatError::Timeout
                } else {
                    ChatError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(classify_status(response.status().as_u16()));
        }
        read_limited(response, self.max_response_bytes, deadline).await
    }
}

fn omit_null_fields(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    value
}

fn classify_status(status: u16) -> ChatError {
    match status {
        401 | 403 => ChatError::Authentication,
        402 => ChatError::Quota,
        429 => ChatError::RateLimited { retry_after: None },
        500..=599 => ChatError::Server { status },
        _ => ChatError::Provider { status },
    }
}

fn sample_rate(content_type: &str) -> Option<u32> {
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        if name.trim().eq_ignore_ascii_case("rate") {
            value
                .trim()
                .trim_matches('"')
                .parse::<u32>()
                .ok()
                .filter(|rate| *rate > 0)
        } else {
            None
        }
    })
}

fn wav_sample_rate(audio: &[u8]) -> Option<u32> {
    if audio.get(..4)? != b"RIFF" || audio.get(8..12)? != b"WAVE" {
        return None;
    }
    let rate = u32::from_le_bytes(audio.get(24..28)?.try_into().ok()?);
    (rate > 0).then_some(rate)
}

async fn read_limited(
    response: Response,
    max_bytes: usize,
    deadline: Instant,
) -> Result<Vec<u8>, ChatError> {
    if response
        .content_length()
        .is_some_and(|length| length as usize > max_bytes)
    {
        return Err(ChatError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = timeout_at(deadline, stream.next())
        .await
        .map_err(|_| ChatError::Timeout)?
    {
        let chunk = chunk.map_err(|error| {
            if error.is_timeout() {
                ChatError::Timeout
            } else {
                ChatError::Connection
            }
        })?;
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn safe_url(value: &str) -> String {
    Url::parse(value).map_or_else(
        |_| "[invalid URL]".to_owned(),
        |url| {
            format!(
                "{}://{}{}",
                url.scheme(),
                url.host_str().unwrap_or("[host]"),
                url.path()
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn parses_sample_rate_from_content_type_or_wave_header() {
        assert_eq!(sample_rate("audio/pcm; rate=24000"), Some(24_000));
        assert_eq!(sample_rate("audio/pcm; RATE=\"48000\""), Some(48_000));
        assert_eq!(sample_rate("audio/pcm; rate=0"), None);
        let mut wav = vec![0; 28];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[24..28].copy_from_slice(&24_000u32.to_le_bytes());
        assert_eq!(wav_sample_rate(&wav), Some(24_000));
        assert_eq!(wav_sample_rate(b"not wave"), None);
    }

    #[test]
    fn speech_stream_chunks_are_bounded() {
        let mut bytes = VecDeque::from(vec![7; SPEECH_STREAM_CHUNK_BYTES + 3]);
        assert_eq!(
            take_audio_chunk(&mut bytes).len(),
            SPEECH_STREAM_CHUNK_BYTES
        );
        assert_eq!(bytes.len(), 3);
        assert_eq!(take_audio_chunk(&mut bytes), vec![7; 3]);
    }

    fn settings(base_url: String) -> LlamaServerSettings {
        LlamaServerSettings {
            base_url,
            api_key: Some("test-key".into()),
            headers: BTreeMap::new(),
            connect_timeout_ms: 2_000,
            read_timeout_ms: 2_000,
            total_timeout_ms: 4_000,
            retry_connect_attempts: 1,
            max_response_bytes: 16 * 1024,
            max_media_bytes: 16 * 1024,
            max_event_bytes: 8 * 1024,
        }
    }

    #[test]
    fn normalizes_root_and_v1_urls_and_redacts_credentials() {
        let root = LlamaServerClient::new(settings("http://127.0.0.1:8080".into())).unwrap();
        assert_eq!(root.root_url.as_str(), "http://127.0.0.1:8080/");
        assert_eq!(root.v1_url.as_str(), "http://127.0.0.1:8080/v1");
        let v1 = LlamaServerClient::new(settings("http://127.0.0.1:8080/v1/".into())).unwrap();
        assert_eq!(v1.root_url.as_str(), "http://127.0.0.1:8080/");
        assert_eq!(v1.v1_url.as_str(), "http://127.0.0.1:8080/v1");
        assert!(!format!("{v1:?}").contains("test-key"));
    }

    #[test]
    fn rejects_unsafe_urls_and_reserved_headers() {
        assert!(LlamaServerClient::new(settings("http://user:pass@localhost".into())).is_err());
        let mut invalid = settings("http://localhost".into());
        invalid
            .headers
            .insert("authorization".into(), "unsafe".into());
        assert!(matches!(
            LlamaServerClient::new(invalid),
            Err(ChatError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn model_metadata_preserves_unknown_modalities_and_missing_architecture() {
        let models: LlamaServerModelList = serde_json::from_value(serde_json::json!({
            "object": "list",
            "future_server_field": "preserved",
            "data": [
                {"id": "vision", "architecture": {
            "input_modalities": ["text", "image", "future_input"],
            "output_modalities": ["text", "decisions"],
            "future_architecture_field": true
                }, "meta": null},
                {"id": "legacy"},
                {"id": "partial", "architecture": {"input_modalities": ["text"]}}
            ]
        }))
        .unwrap();
        assert_eq!(
            models.data[0]
                .architecture
                .as_ref()
                .unwrap()
                .input_modalities
                .as_ref()
                .unwrap()[2],
            "future_input"
        );
        assert_eq!(
            models.data[0].architecture.as_ref().unwrap().extra["future_architecture_field"],
            true
        );
        assert_eq!(models.data[1].architecture, None);
        assert_eq!(
            models.data[2]
                .architecture
                .as_ref()
                .unwrap()
                .output_modalities,
            None
        );
        assert_eq!(models.extra["future_server_field"], "preserved");
    }

    async fn serve_once(listener: TcpListener, health: &'static str, models: &'static str) {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let n = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(
                request.contains("authorization: Bearer test-key")
                    || request.contains("Authorization: Bearer test-key")
            );
            let body = if request.starts_with("GET /health ") {
                health
            } else {
                assert!(request.starts_with("GET /v1/models "));
                models
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    }

    async fn serve_chat_once(listener: TcpListener) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        let n = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..n]);
        assert!(request.starts_with("POST /v1/chat/completions "));
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn embeddings_use_authenticated_endpoint_and_restore_response_order() {
        use crate::llm::embeddings::{EMBEDDING_CONTRACT_VERSION, EmbeddingInputType};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 8192];
            let n = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(request.starts_with("POST /v1/embeddings "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-key")
            );
            assert!(request.contains("\"input\":[\"first\",\"second\"]"));
            let body = r#"{"data":[{"index":1,"embedding":[0.3,0.4]},{"index":0,"embedding":[0.1,0.2]}],"usage":{"prompt_tokens":7}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let response = client
            .embed(EmbeddingRequest {
                contract_version: EMBEDDING_CONTRACT_VERSION,
                model: "embed-model".into(),
                input_type: EmbeddingInputType::Document,
                texts: vec!["first".into(), "second".into()],
                dimensions: None,
            })
            .await
            .unwrap();
        assert_eq!(response.embeddings, vec![vec![0.1, 0.2], vec![0.3, 0.4]]);
        assert_eq!(response.dimensions, 2);
        assert_eq!(response.usage.unwrap().input_tokens, 7);
        server.await.unwrap();
    }

    struct EmbeddingAssetStore(std::collections::HashMap<String, Vec<u8>>);

    #[async_trait::async_trait]
    impl super::super::MediaStore for EmbeddingAssetStore {
        async fn read(
            &self,
            media_ref: &super::super::MediaRef,
        ) -> Result<Vec<u8>, super::super::MediaError> {
            self.0
                .get(&media_ref.asset_id)
                .cloned()
                .ok_or(super::super::MediaError::Unavailable)
        }
    }

    fn png_ref(bytes: &[u8]) -> super::super::MediaRef {
        super::super::MediaRef {
            asset_id: "image-1".into(),
            sha256: super::super::sha256_hex(bytes),
            mime_type: "image/png".into(),
            size_bytes: bytes.len() as u64,
            name: "fixture.png".into(),
        }
    }

    #[tokio::test]
    async fn multimodal_embeddings_resolve_and_materialize_only_consumer_assets() {
        use super::super::{CapabilityResolver, MediaExecutionContext, MediaPart};
        let bytes = b"\x89PNG\r\n\x1a\nfixture".to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 16 * 1024];
                let n = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer test-key")
                );
                let (status, body) = if request.starts_with("GET /v1/models ") {
                    (
                        "200 OK",
                        r#"{"data":[{"id":"vision-embed","architecture":{"input_modalities":["text","image"]}}]}"#,
                    )
                } else {
                    assert!(request.starts_with("POST /v1/embeddings "));
                    assert!(request.contains("data:image/png;base64,"));
                    assert!(!request.contains("file://"));
                    ("200 OK", r#"{"data":[{"index":0,"embedding":[0.1,0.9]}]}"#)
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let context = MediaExecutionContext::new(
            CapabilityResolver::default(),
            std::sync::Arc::new(EmbeddingAssetStore(std::collections::HashMap::from([(
                "image-1".to_owned(),
                bytes.clone(),
            )]))),
        );
        let response = client
            .embed_ephemeral(
                EphemeralLlamaServerEmbeddingRequest {
                    contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
                    model: "vision-embed".into(),
                    inputs: vec![LlamaServerEmbeddingInput {
                        content: vec![
                            ContentPart::Text {
                                text: "describe".into(),
                            },
                            ContentPart::Image(MediaPart {
                                media_ref: png_ref(&bytes),
                                text: None,
                                previews: vec![],
                            }),
                        ],
                    }],
                },
                &context,
            )
            .await
            .unwrap();
        assert_eq!(response.embeddings, vec![vec![0.1, 0.9]]);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn multimodal_embeddings_encode_audio_and_video_as_native_data_parts() {
        use super::super::{
            CapabilityResolver, MediaExecutionContext, MediaPart, MediaRef, sha256_hex,
        };
        let audio = b"RIFF\x04\x00\x00\x00WAVE".to_vec();
        let mut video = vec![0; 16];
        video[4..8].copy_from_slice(b"ftyp");
        video[8..12].copy_from_slice(b"isom");
        let reference = |id: &str, bytes: &[u8], mime: &str, name: &str| MediaRef {
            asset_id: id.into(),
            sha256: sha256_hex(bytes),
            mime_type: mime.into(),
            size_bytes: bytes.len() as u64,
            name: name.into(),
        };
        let store = EmbeddingAssetStore(std::collections::HashMap::from([
            ("audio-1".into(), audio.clone()),
            ("video-1".into(), video.clone()),
        ]));
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio);
        let video_base64 = base64::engine::general_purpose::STANDARD.encode(&video);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 16 * 1024];
                let n = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                let body = if request.starts_with("GET /v1/models ") {
                    r#"{"data":[{"id":"av-embed","architecture":{"input_modalities":["text","audio","video"]}}]}"#
                } else {
                    assert!(request.contains("\"type\":\"input_audio\""));
                    assert!(request.contains("\"type\":\"input_video\""));
                    assert!(request.contains(&audio_base64));
                    assert!(request.contains(&video_base64));
                    r#"{"data":[{"index":0,"embedding":[0.3]},{"index":1,"embedding":[0.4]}]}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let context =
            MediaExecutionContext::new(CapabilityResolver::default(), std::sync::Arc::new(store));
        let response = client
            .embed_ephemeral(
                EphemeralLlamaServerEmbeddingRequest {
                    contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
                    model: "av-embed".into(),
                    inputs: vec![
                        LlamaServerEmbeddingInput {
                            content: vec![ContentPart::Audio(MediaPart {
                                media_ref: reference("audio-1", &audio, "audio/wav", "sound.wav"),
                                text: None,
                                previews: vec![],
                            })],
                        },
                        LlamaServerEmbeddingInput {
                            content: vec![ContentPart::Video(MediaPart {
                                media_ref: reference("video-1", &video, "video/mp4", "clip.mp4"),
                                text: None,
                                previews: vec![],
                            })],
                        },
                    ],
                },
                &context,
            )
            .await
            .unwrap();
        assert_eq!(response.embeddings, vec![vec![0.3], vec![0.4]]);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn multimodal_embeddings_reject_remote_urls_and_undeclared_modalities() {
        use super::super::{CapabilityResolver, ImageUrl, MediaExecutionContext};
        let client = LlamaServerClient::new(settings("http://127.0.0.1:1".into())).unwrap();
        let context = MediaExecutionContext::new(
            CapabilityResolver::default(),
            std::sync::Arc::new(EmbeddingAssetStore(std::collections::HashMap::new())),
        );
        let request = EphemeralLlamaServerEmbeddingRequest {
            contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
            model: "embed-model".into(),
            inputs: vec![LlamaServerEmbeddingInput {
                content: vec![ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: "https://example.invalid/image.png".into(),
                        detail: None,
                    },
                }],
            }],
        };
        assert!(matches!(
            client.embed_ephemeral(request, &context).await,
            Err(ChatError::InvalidRequest(_))
        ));
    }

    #[tokio::test]
    async fn multimodal_embeddings_fail_closed_when_model_does_not_declare_image() {
        use super::super::{CapabilityResolver, MediaExecutionContext, MediaPart};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let bytes = b"\x89PNG\r\n\x1a\nfixture".to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let request_count = std::sync::Arc::new(AtomicUsize::new(0));
        let server_request_count = request_count.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let n = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(request.starts_with("GET /v1/models "));
            server_request_count.fetch_add(1, Ordering::SeqCst);
            let body =
                r#"{"data":[{"id":"text-only","architecture":{"input_modalities":["text"]}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let context = MediaExecutionContext::new(
            CapabilityResolver::default(),
            std::sync::Arc::new(EmbeddingAssetStore(std::collections::HashMap::from([(
                "image-1".to_owned(),
                bytes.clone(),
            )]))),
        );
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let result = client
            .embed_ephemeral(
                EphemeralLlamaServerEmbeddingRequest {
                    contract_version: crate::llm::embeddings::EMBEDDING_CONTRACT_VERSION,
                    model: "text-only".into(),
                    inputs: vec![LlamaServerEmbeddingInput {
                        content: vec![ContentPart::Image(MediaPart {
                            media_ref: png_ref(&bytes),
                            text: None,
                            previews: vec![],
                        })],
                    }],
                },
                &context,
            )
            .await;

        assert!(matches!(result, Err(ChatError::ModelCapabilityUnsupported)));
        server.await.unwrap();
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn embeddings_reject_duplicate_indices_and_non_finite_vectors() {
        use crate::llm::embeddings::{EMBEDDING_CONTRACT_VERSION, EmbeddingInputType};
        for body in [
            r#"{"data":[{"index":0,"embedding":[1.0]},{"index":0,"embedding":[2.0]}]}"#,
            r#"{"data":[{"index":0,"embedding":[1e999]}]}"#,
            r#"{"data":[{"index":1,"embedding":[1.0]}]}"#,
            r#"{"data":[{"index":0,"embedding":[1.0]},{"index":1,"embedding":[1.0,2.0]}]}"#,
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let _ = stream.read(&mut request).await.unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
            let result = client
                .embed(EmbeddingRequest {
                    contract_version: EMBEDDING_CONTRACT_VERSION,
                    model: "embed-model".into(),
                    input_type: EmbeddingInputType::Query,
                    texts: vec!["input".into()],
                    dimensions: None,
                })
                .await;
            assert!(matches!(result, Err(ChatError::InvalidResponse)));
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn empty_embedding_batch_returns_without_network() {
        use crate::llm::embeddings::{EMBEDDING_CONTRACT_VERSION, EmbeddingInputType};
        let client = LlamaServerClient::new(settings("http://127.0.0.1:1".into())).unwrap();
        let response = client
            .embed(EmbeddingRequest {
                contract_version: EMBEDDING_CONTRACT_VERSION,
                model: "embed-model".into(),
                input_type: EmbeddingInputType::Query,
                texts: Vec::new(),
                dimensions: None,
            })
            .await
            .unwrap();
        assert!(response.embeddings.is_empty());
        assert_eq!(response.dimensions, 0);
    }

    #[tokio::test]
    async fn decision_uses_systemone_and_preserves_probabilities() {
        use super::super::{DecisionQuestion, DecisionQuestionKind};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let n = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                let body = if request.starts_with("GET /v1/models ") {
                    r#"{"data":[{"id":"decision-model","architecture":{"output_modalities":["decisions"]}}]}"#
                } else {
                    assert!(request.starts_with("POST /v1/systemone "));
                    assert!(
                        request
                            .to_ascii_lowercase()
                            .contains("authorization: bearer test-key")
                    );
                    assert!(request.contains("\"questions\""));
                    assert!(!request.contains("\"model\""));
                    r#"{"answers":{"route":{"choice":"billing","probabilities":{"billing":0.8,"shipping":0.2},"confidence":0.6}},"usage":{"input_tokens":12,"output_tokens":0}}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let request = DecisionRequest {
            model: "decision-model".into(),
            state: serde_json::json!({"ticket":"charged twice"}),
            questions: BTreeMap::from([(
                "route".into(),
                DecisionQuestion {
                    kind: DecisionQuestionKind::Choice,
                    instructions: Some(serde_json::json!("Which team?")),
                    criteria: Some(serde_json::json!({"billing": null, "shipping": null})),
                },
            )]),
        };
        let response = client.decide(request).await.unwrap();
        assert_eq!(response.answers["route"]["choice"], "billing");
        assert_eq!(response.answers["route"]["probabilities"]["billing"], 0.8);
        assert_eq!(response.usage.unwrap().input_tokens, Some(12));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn speech_exposes_openai_and_native_routes_with_binary_metadata() {
        use super::super::{NativeSpeechRequest, SpeechFormat, SpeechRequest};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..5 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let n = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..n]);
                let (content_type, body): (&str, Vec<u8>) = if request
                    .starts_with("GET /v1/models ")
                {
                    ("application/json", br#"{"data":[{"id":"speech-model","architecture":{"output_modalities":["audio"]}}]}"#.to_vec())
                } else if request.starts_with("GET /props ") {
                    (
                        "application/json",
                        br#"{"voices":["fixture-voice"]}"#.to_vec(),
                    )
                } else if request.starts_with("POST /v1/audio/speech ") {
                    assert!(request.contains("\"response_format\":\"wav\""));
                    (
                        "audio/wav",
                        b"RIFF\0\0\0\0WAVEfmt \0\0\0\0\x80\xbb\0\0".to_vec(),
                    )
                } else {
                    assert!(request.starts_with("POST /tts "));
                    assert!(request.contains("\"speaker_ref_b64\":\"aGVsbG8=\""));
                    assert!(request.contains("\"response_format\":\"pcm\""));
                    ("audio/pcm; rate=24000", vec![1, 2, 3, 4])
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let openai = client
            .synthesize(SpeechRequest {
                model: "speech-model".into(),
                input: "hello".into(),
                voice: Some("fixture-voice".into()),
                format: SpeechFormat::Wav,
                instructions: None,
                speed: 1.0,
            })
            .await
            .unwrap();
        assert_eq!(openai.content_type, "audio/wav");
        assert_eq!(openai.audio, b"RIFF\0\0\0\0WAVEfmt \0\0\0\0\x80\xbb\0\0");

        let native = client
            .synthesize_native_ephemeral(EphemeralNativeSpeechRequest {
                request: NativeSpeechRequest {
                    request: SpeechRequest {
                        model: "speech-model".into(),
                        input: "hello".into(),
                        voice: None,
                        format: SpeechFormat::PcmF32,
                        instructions: None,
                        speed: 1.0,
                    },
                    lang: Some("en".into()),
                    seed: Some(5),
                    top_k: None,
                    top_p: None,
                    temperature: None,
                    repeat_penalty: None,
                    n_predict: None,
                },
                speaker_ref_b64: Some("aGVsbG8=".into()),
            })
            .await
            .unwrap();
        assert_eq!(native.format, SpeechFormat::PcmF32);
        assert_eq!(native.sample_rate_hz, Some(24_000));
        assert_eq!(native.audio, [1, 2, 3, 4]);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn speech_streams_are_incremental_bounded_and_use_both_wire_contracts() {
        use super::super::{NativeSpeechRequest, SpeechFormat, SpeechRequest};
        use tokio::sync::oneshot;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (first_sent_tx, first_sent_rx) = oneshot::channel();
        let (continue_tx, continue_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first_sent_tx = Some(first_sent_tx);
            let mut continue_rx = Some(continue_rx);
            for index in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let read = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                if request.starts_with("GET /v1/models ") {
                    let models = r#"{"data":[{"id":"speech-model","architecture":{"output_modalities":["audio"]}}]}"#;
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{models}",
                        models.len()
                    );
                    stream.write_all(header.as_bytes()).await.unwrap();
                    continue;
                }
                if request.starts_with("POST /v1/audio/speech ") {
                    assert!(request.contains(r#""stream_format":"audio""#));
                } else {
                    assert!(request.starts_with("POST /tts "));
                    assert!(request.contains(r#""stream":true"#));
                }
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/pcm; rate=24000\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n")
                    .await
                    .unwrap();
                if index == 1 {
                    if let Some(sender) = first_sent_tx.take() {
                        let _ = sender.send(());
                    }
                    if let Some(receiver) = continue_rx.take() {
                        let _ = receiver.await;
                    }
                }
                stream.write_all(b"3\r\ndef\r\n0\r\n\r\n").await.unwrap();
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let mut stream = client
            .synthesize_stream(SpeechRequest {
                model: "speech-model".into(),
                input: "hello".into(),
                voice: None,
                format: SpeechFormat::Pcm16,
                instructions: None,
                speed: 1.0,
            })
            .await
            .unwrap();
        assert_eq!(stream.info().sample_rate_hz, Some(24_000));
        first_sent_rx.await.unwrap();
        assert_eq!(stream.next_chunk().await.unwrap().unwrap(), b"abc");
        continue_tx.send(()).unwrap();
        assert_eq!(stream.next_chunk().await.unwrap().unwrap(), b"def");
        assert_eq!(stream.next_chunk().await.unwrap(), None);

        let mut native = client
            .synthesize_native_stream(NativeSpeechRequest {
                request: SpeechRequest {
                    model: "speech-model".into(),
                    input: "hello".into(),
                    voice: None,
                    format: SpeechFormat::Wav,
                    instructions: None,
                    speed: 1.0,
                },
                lang: Some("en".into()),
                seed: None,
                top_k: None,
                top_p: None,
                temperature: None,
                repeat_penalty: None,
                n_predict: None,
            })
            .await
            .unwrap();
        assert_eq!(native.next_chunk().await.unwrap().unwrap(), b"abc");
        assert_eq!(native.next_chunk().await.unwrap().unwrap(), b"def");
        assert_eq!(native.next_chunk().await.unwrap(), None);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn speech_stream_reports_connection_loss_after_partial_audio() {
        use super::super::{SpeechFormat, SpeechRequest};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 2048];
                let read = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                if request.starts_with("GET /v1/models ") {
                    let models = r#"{"data":[{"id":"speech-model","architecture":{"output_modalities":["audio"]}}]}"#;
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{models}",
                        models.len()
                    );
                    stream.write_all(header.as_bytes()).await.unwrap();
                } else {
                    assert!(request.starts_with("POST /v1/audio/speech "));
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/pcm; rate=24000\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc")
                        .await
                        .unwrap();
                }
            }
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let mut stream = client
            .synthesize_stream(SpeechRequest {
                model: "speech-model".into(),
                input: "hello".into(),
                voice: None,
                format: SpeechFormat::Pcm16,
                instructions: None,
                speed: 1.0,
            })
            .await
            .unwrap();
        assert_eq!(stream.next_chunk().await.unwrap().unwrap(), b"abc");
        assert!(matches!(
            stream.next_chunk().await,
            Err(ChatError::Connection)
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn realtime_transcription_streams_pcm16_and_server_events() {
        use super::super::{RealtimeIntent, RealtimeSessionRequest};
        use futures_util::{SinkExt, StreamExt};
        use tokio_websockets::{Message, ServerBuilder};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut http, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 2048];
            let n = http.read(&mut request).await.unwrap();
            assert!(String::from_utf8_lossy(&request[..n]).starts_with("GET /v1/models "));
            let models = r#"{"data":[{"id":"audio-model","architecture":{"input_modalities":["audio"],"output_modalities":["text"]}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{models}",
                models.len()
            );
            http.write_all(response.as_bytes()).await.unwrap();

            let (tcp, _) = listener.accept().await.unwrap();
            let (request, mut socket) = ServerBuilder::new().accept(tcp).await.unwrap();
            assert_eq!(request.uri().path(), "/v1/realtime");
            assert!(request.uri().query().unwrap().contains("model=audio-model"));
            assert!(
                request
                    .uri()
                    .query()
                    .unwrap()
                    .contains("intent=transcription")
            );
            assert_eq!(request.headers()["authorization"], "Bearer test-key");
            let rate = socket.next().await.unwrap().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(rate.as_text().unwrap()).unwrap()["type"],
                "session.update"
            );
            socket
                .send(Message::text(r#"{"type":"session.updated"}"#))
                .await
                .unwrap();
            let audio = socket.next().await.unwrap().unwrap();
            let audio: serde_json::Value = serde_json::from_str(audio.as_text().unwrap()).unwrap();
            assert_eq!(audio["type"], "input_audio_buffer.append");
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(audio["audio"].as_str().unwrap())
                    .unwrap(),
                [1, 2, 3, 4]
            );
            let commit = socket.next().await.unwrap().unwrap();
            let commit: serde_json::Value =
                serde_json::from_str(commit.as_text().unwrap()).unwrap();
            assert_eq!(commit["type"], "input_audio_buffer.commit");
            assert_eq!(commit["final"], true);
            socket.send(Message::text(r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"item-1","delta":"hello"}"#)).await.unwrap();
            socket.send(Message::text(r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"item-1","transcript":"hello"}"#)).await.unwrap();
        });
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let mut session = client
            .connect_realtime(RealtimeSessionRequest {
                model: "audio-model".into(),
                intent: RealtimeIntent::Transcription,
                input_sample_rate_hz: Some(16_000),
            })
            .await
            .unwrap();
        assert_eq!(
            session.receive().await.unwrap().unwrap().event_type,
            "session.updated"
        );
        session.send_pcm16(&[1, 2, 3, 4]).await.unwrap();
        session.commit_audio(true).await.unwrap();
        let delta = session.receive().await.unwrap().unwrap();
        assert_eq!(delta.fields["delta"], "hello");
        let completed = session.receive().await.unwrap().unwrap();
        assert_eq!(completed.fields["transcript"], "hello");
        session.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn health_and_model_discovery_use_v1_0_wire_shapes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve_once(
            listener,
            r#"{"status":"ok"}"#,
            r#"{"object":"list","data":[{"id":"test-model","architecture":{"input_modalities":["text","image"],"output_modalities":["text"]},"meta":{"n_ctx_train":4096}}]}"#,
        ));
        let client = LlamaServerClient::new(settings(format!("http://{address}/v1"))).unwrap();
        assert_eq!(client.health().await.unwrap().status, "ok");
        let models = client.models().await.unwrap();
        assert_eq!(models.data[0].id, "test-model");
        assert_eq!(models.data[0].meta.as_ref().unwrap()["n_ctx_train"], 4096);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn chat_uses_the_v1_chat_completions_route() {
        use super::super::{Message, MessageRole};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve_chat_once(listener));
        let client = LlamaServerClient::new(settings(format!("http://{address}"))).unwrap();
        let request = ChatRequest::new("test-model", vec![Message::text(MessageRole::User, "hi")]);
        let response = client.complete(request).await.unwrap();
        assert_eq!(response.content, "hello");
        server.await.unwrap();
    }
}
