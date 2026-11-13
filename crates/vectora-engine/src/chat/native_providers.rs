use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    time::Duration,
};

use async_stream::try_stream;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::{Client, Response, header};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::time::{Instant, timeout_at};
use url::Url;

use super::media::{has_native_media, mark_text_retry};
use super::{
    ChatError, ChatEvent, ChatEventKind, ChatRequest, FinishReason, MediaExecutionContext, Message,
    MessageContent, MessageRole, ToolArguments, ToolCall, ToolCallDelta, Usage,
    error::classify_provider_error,
    openai_compatible::{ChatStream, validate_request},
    sse::SseDecoder,
    types::{CHAT_CONTRACT_VERSION, ChatCompletion},
};

const DEFAULT_ANTHROPIC_URL: &str = "https://api.anthropic.com/v1";
const DEFAULT_GOOGLE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_COHERE_URL: &str = "https://api.cohere.com/v2";
const MAX_ERROR_BODY_BYTES: usize = 8 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeProvider {
    Anthropic,
    Google,
    Cohere,
}

/// Settings for a native protocol provider. Credentials are supplied by the
/// consumer for each client instance and are never included in Debug output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeChatSettings {
    pub provider: NativeProvider,
    #[serde(default)]
    pub base_url: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    pub total_timeout_ms: u64,
}

const fn default_connect_timeout_ms() -> u64 {
    10_000
}
const fn default_read_timeout_ms() -> u64 {
    60_000
}
const fn default_total_timeout_ms() -> u64 {
    300_000
}

impl fmt::Debug for NativeChatSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeChatSettings")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url.as_deref().map(safe_endpoint))
            .field("api_key", &"[REDACTED]")
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("read_timeout_ms", &self.read_timeout_ms)
            .field("total_timeout_ms", &self.total_timeout_ms)
            .finish()
    }
}

enum NativeProtocolClient {
    Anthropic(ProtocolClient),
    Google(ProtocolClient),
    Cohere(ProtocolClient),
}

/// Unified native-protocol facade used by both language bindings.
pub struct NativeChatClient {
    inner: NativeProtocolClient,
    media_context: Option<MediaExecutionContext>,
}

impl fmt::Debug for NativeChatClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeChatClient")
            .field(
                "provider",
                &match self.inner {
                    NativeProtocolClient::Anthropic(_) => NativeProvider::Anthropic,
                    NativeProtocolClient::Google(_) => NativeProvider::Google,
                    NativeProtocolClient::Cohere(_) => NativeProvider::Cohere,
                },
            )
            .finish_non_exhaustive()
    }
}

struct ProtocolClient {
    provider: NativeProvider,
    api_key: String,
    headers: Vec<(header::HeaderName, header::HeaderValue)>,
    endpoint: Url,
    client: Client,
    read_timeout: Duration,
    total_timeout: Duration,
}

impl NativeChatClient {
    pub fn new(settings: NativeChatSettings) -> Result<Self, ChatError> {
        let base = match settings.provider {
            NativeProvider::Anthropic => DEFAULT_ANTHROPIC_URL,
            NativeProvider::Google => DEFAULT_GOOGLE_URL,
            NativeProvider::Cohere => DEFAULT_COHERE_URL,
        };
        let base_url = settings.base_url.as_deref().unwrap_or(base);
        if settings.api_key.trim().is_empty() {
            return Err(ChatError::InvalidConfiguration("API key must not be empty"));
        }
        let connect_timeout = Duration::from_millis(settings.connect_timeout_ms);
        let read_timeout = Duration::from_millis(settings.read_timeout_ms);
        let total_timeout = Duration::from_millis(settings.total_timeout_ms);
        if connect_timeout.is_zero() || read_timeout.is_zero() || total_timeout.is_zero() {
            return Err(ChatError::InvalidConfiguration("timeouts must be positive"));
        }
        let mut endpoint = Url::parse(base_url.trim()).map_err(|_| {
            ChatError::InvalidConfiguration("base URL must be an absolute HTTP URL")
        })?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(ChatError::InvalidConfiguration(
                "base URL is not a safe HTTP endpoint",
            ));
        }
        let path = endpoint.path().trim_end_matches('/').to_owned();
        if settings.provider == NativeProvider::Anthropic {
            endpoint.set_path(&format!("{path}/messages"));
        } else {
            endpoint.set_path(&path);
        }

        let mut headers = Vec::with_capacity(settings.headers.len());
        let mut seen = HashSet::new();
        for (name, value) in settings.headers {
            let name = header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header name"))?;
            let value = header::HeaderValue::from_str(&value)
                .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header value"))?;
            let canonical = name.as_str();
            let reserved = match settings.provider {
                NativeProvider::Anthropic => matches!(
                    canonical,
                    "x-api-key" | "anthropic-version" | "content-type" | "host"
                ),
                NativeProvider::Google => matches!(canonical, "content-type" | "host"),
                NativeProvider::Cohere => {
                    matches!(canonical, "authorization" | "content-type" | "host")
                }
            };
            if reserved || !seen.insert(canonical.to_owned()) {
                return Err(ChatError::InvalidConfiguration(
                    "reserved or duplicate HTTP header",
                ));
            }
            headers.push((name, value));
        }
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|_| ChatError::InvalidConfiguration("HTTP client could not be created"))?;
        let protocol = ProtocolClient {
            provider: settings.provider,
            api_key: settings.api_key,
            headers,
            endpoint,
            client,
            read_timeout,
            total_timeout,
        };
        Ok(Self {
            inner: match settings.provider {
                NativeProvider::Anthropic => NativeProtocolClient::Anthropic(protocol),
                NativeProvider::Google => NativeProtocolClient::Google(protocol),
                NativeProvider::Cohere => NativeProtocolClient::Cohere(protocol),
            },
            media_context: None,
        })
    }

    pub async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        if let Some(context) = &self.media_context {
            return self.complete_with_media(request, context).await;
        }
        self.complete_prepared(request).await
    }

    /// Configure the default context used by `complete` and `stream`.
    pub fn with_media_context(mut self, context: MediaExecutionContext) -> Self {
        self.media_context = Some(context);
        self
    }

    /// Complete with an ephemeral context overriding the client default.
    pub async fn complete_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatCompletion, ChatError> {
        let provider = self.provider_name();
        let prepared = context.prepare_request(provider, request.clone()).await;
        let mut decisions = prepared.decisions;
        let mut completion = match self.complete_prepared(prepared.request).await {
            Ok(completion) => completion,
            Err(ChatError::MultimodalIncompatible) if has_native_media(&decisions) => {
                let fallback = context
                    .prepare_request_text_fallback(provider, request)
                    .await;
                let mut fallback_decisions = fallback.decisions;
                mark_text_retry(&mut fallback_decisions);
                decisions = fallback_decisions;
                self.complete_prepared(fallback.request).await?
            }
            Err(error) => return Err(error),
        };
        completion.media_decisions = decisions;
        Ok(completion)
    }

    async fn complete_prepared(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        match &self.inner {
            NativeProtocolClient::Anthropic(client) => client.complete(request).await,
            NativeProtocolClient::Google(client) => client.complete(request).await,
            NativeProtocolClient::Cohere(client) => client.complete(request).await,
        }
    }

    pub async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        if let Some(context) = &self.media_context {
            return self.stream_with_media(request, context).await;
        }
        self.stream_prepared(request).await
    }

    /// Stream with an ephemeral context overriding the client default.
    pub async fn stream_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatStream, ChatError> {
        let provider = self.provider_name();
        let prepared = context.prepare_request(provider, request.clone()).await;
        let mut decisions = prepared.decisions;
        let stream = match self.stream_prepared(prepared.request).await {
            Ok(stream) => stream,
            Err(ChatError::MultimodalIncompatible) if has_native_media(&decisions) => {
                let fallback = context
                    .prepare_request_text_fallback(provider, request)
                    .await;
                let mut fallback_decisions = fallback.decisions;
                mark_text_retry(&mut fallback_decisions);
                decisions = fallback_decisions;
                self.stream_prepared(fallback.request).await?
            }
            Err(error) => return Err(error),
        };
        Ok(super::openai_compatible::with_media_evidence(
            stream, decisions,
        ))
    }

    fn provider_name(&self) -> &'static str {
        match self.inner {
            NativeProtocolClient::Anthropic(_) => "anthropic",
            NativeProtocolClient::Google(_) => "google",
            NativeProtocolClient::Cohere(_) => "cohere",
        }
    }

    async fn stream_prepared(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        match &self.inner {
            NativeProtocolClient::Anthropic(client) => client.stream(request).await,
            NativeProtocolClient::Google(client) => client.stream(request).await,
            NativeProtocolClient::Cohere(client) => client.stream(request).await,
        }
    }
}

impl ProtocolClient {
    fn endpoint_for(&self, model: &str, streaming: bool) -> Result<Url, ChatError> {
        if self.provider == NativeProvider::Anthropic {
            return Ok(self.endpoint.clone());
        }
        if self.provider == NativeProvider::Cohere {
            let mut url = self.endpoint.clone();
            let path = url.path().trim_end_matches('/');
            url.set_path(&format!("{path}/chat"));
            return Ok(url);
        }
        if model.contains('/') || model.trim().is_empty() {
            return Err(ChatError::InvalidConfiguration(
                "Google model name is invalid",
            ));
        }
        let mut url = self.endpoint.clone();
        let prefix = url.path().trim_end_matches('/');
        url.set_path(&format!(
            "{prefix}/models/{model}:{}",
            if streaming {
                "streamGenerateContent"
            } else {
                "generateContent"
            }
        ));
        url.query_pairs_mut().append_pair("key", &self.api_key);
        if streaming {
            url.query_pairs_mut().append_pair("alt", "sse");
        }
        Ok(url)
    }

    fn request_builder(&self, url: Url, streaming: bool, body: &Value) -> reqwest::RequestBuilder {
        let mut builder = self.client.post(url).json(body);
        match self.provider {
            NativeProvider::Anthropic => {
                builder = builder
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01");
            }
            NativeProvider::Google => {}
            NativeProvider::Cohere => {
                builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", self.api_key));
            }
        }
        for (name, value) in &self.headers {
            builder = builder.header(name, value);
        }
        if streaming {
            builder = builder.header(header::ACCEPT, "text/event-stream");
        }
        builder
    }

    async fn send(
        &self,
        url: Url,
        streaming: bool,
        body: &Value,
        deadline: Instant,
    ) -> Result<Response, ChatError> {
        match timeout_at(deadline, self.request_builder(url, streaming, body).send()).await {
            Err(_) => Err(ChatError::Timeout),
            Ok(Err(error)) if error.is_timeout() => Err(ChatError::Timeout),
            Ok(Err(error)) if error.is_connect() => Err(ChatError::Connection),
            Ok(Err(_)) => Err(ChatError::Connection),
            Ok(Ok(response)) => Ok(response),
        }
    }

    async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        validate_request(&request)?;
        let url = self.endpoint_for(&request.model, false)?;
        let body = native_payload(self.provider, &request, false)?;
        let deadline = Instant::now() + self.total_timeout;
        let response = self.send(url, false, &body, deadline).await?;
        if !response.status().is_success() {
            return Err(http_error(response, deadline, self.provider).await);
        }
        let bytes = read_body(response, deadline, self.read_timeout, MAX_RESPONSE_BYTES).await?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| ChatError::InvalidResponse)?;
        match self.provider {
            NativeProvider::Anthropic => parse_anthropic_completion(value),
            NativeProvider::Google => parse_google_completion(value),
            NativeProvider::Cohere => parse_cohere_completion(value),
        }
    }

    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        validate_request(&request)?;
        let url = self.endpoint_for(&request.model, true)?;
        let body = native_payload(self.provider, &request, true)?;
        let deadline = Instant::now() + self.total_timeout;
        let response = self.send(url, true, &body, deadline).await?;
        if !response.status().is_success() {
            return Err(http_error(response, deadline, self.provider).await);
        }
        let provider = self.provider;
        let read_timeout = self.read_timeout;
        let stream = try_stream! {
            let mut body = response.bytes_stream();
            let mut decoder = SseDecoder::new(MAX_EVENT_BYTES);
            let mut received = 0usize;
            let mut anthropic_blocks: HashMap<u32, (String, String)> = HashMap::new();
            let mut emitted_tool_names = HashSet::new();
            let mut usage = None;
            let mut finish_reason = None;
            let mut next_google_tool_index = 0u32;
            let mut cohere_tool_indexes = HashMap::new();
            let mut next_cohere_tool_index = 0u32;
            let mut completed = false;
            loop {
                let Some(chunk) = next_chunk(&mut body, deadline, read_timeout).await? else { break; };
                received = received.saturating_add(chunk.len());
                if received > MAX_RESPONSE_BYTES { Err(ChatError::ResponseTooLarge)?; }
                for frame in decoder.push(&chunk)? {
                    if frame == "[DONE]" { completed = true; break; }
                    let value: Value = serde_json::from_str(&frame).map_err(|_| ChatError::Protocol("invalid native provider SSE JSON"))?;
                    let events = match provider {
                        NativeProvider::Anthropic => parse_anthropic_event(&value, &mut anthropic_blocks, &mut emitted_tool_names, &mut usage, &mut finish_reason)?,
                        NativeProvider::Google => parse_google_event(&value, &mut usage, &mut finish_reason, &mut next_google_tool_index)?,
                        NativeProvider::Cohere => parse_cohere_event(&value, &mut usage, &mut finish_reason, &mut cohere_tool_indexes, &mut next_cohere_tool_index)?,
                    };
                    for event in events {
                        if matches!(event.event, ChatEventKind::Completed { .. }) { completed = true; }
                        yield event;
                    }
                    if completed { break; }
                }
                if completed { break; }
            }
            if !completed {
                for frame in decoder.finish()? {
                    if frame == "[DONE]" { completed = true; break; }
                    let value: Value = serde_json::from_str(&frame).map_err(|_| ChatError::Protocol("invalid native provider SSE JSON"))?;
                    let events = match provider {
                        NativeProvider::Anthropic => parse_anthropic_event(&value, &mut anthropic_blocks, &mut emitted_tool_names, &mut usage, &mut finish_reason)?,
                        NativeProvider::Google => parse_google_event(&value, &mut usage, &mut finish_reason, &mut next_google_tool_index)?,
                        NativeProvider::Cohere => parse_cohere_event(&value, &mut usage, &mut finish_reason, &mut cohere_tool_indexes, &mut next_cohere_tool_index)?,
                    };
                    for event in events {
                        if matches!(event.event, ChatEventKind::Completed { .. }) { completed = true; }
                        yield event;
                    }
                }
            }
            if provider == NativeProvider::Google && let Some(usage) = usage {
                yield ChatEvent::new(ChatEventKind::Usage { usage });
            }
            if !completed {
                yield ChatEvent::new(ChatEventKind::Completed { finish_reason });
            }
        };
        Ok(Box::pin(stream))
    }
}

fn native_payload(
    provider: NativeProvider,
    request: &ChatRequest,
    streaming: bool,
) -> Result<Value, ChatError> {
    match provider {
        NativeProvider::Anthropic => anthropic_payload(request, streaming),
        NativeProvider::Google => google_payload(request),
        NativeProvider::Cohere => cohere_payload(request, streaming),
    }
}

fn anthropic_payload(request: &ChatRequest, streaming: bool) -> Result<Value, ChatError> {
    let mut system = Vec::new();
    let mut messages = Vec::new();
    for message in &request.messages {
        match message.role {
            MessageRole::System => {
                let text = message_text(message);
                if !text.is_empty() {
                    system.push(text);
                }
            }
            MessageRole::User | MessageRole::Assistant => {
                let role = if message.role == MessageRole::User {
                    "user"
                } else {
                    "assistant"
                };
                let content = if role == "assistant" && !message.tool_calls.is_empty() {
                    let mut parts = Vec::new();
                    let text = message_text(message);
                    if !text.is_empty() {
                        parts.push(json!({"type":"text","text":text}));
                    }
                    for call in &message.tool_calls {
                        parts.push(json!({"type":"tool_use","id":call.id,"name":call.name,"input":arguments_value(&call.arguments)}));
                    }
                    Value::Array(parts)
                } else {
                    anthropic_content(&message.content)
                };
                messages.push(json!({"role":role,"content":content}));
            }
            MessageRole::Tool => {
                let id = message
                    .tool_call_id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .ok_or(ChatError::InvalidConfiguration(
                        "Anthropic tool result requires tool_call_id",
                    ))?;
                let mut result =
                    json!({"type":"tool_result","tool_use_id":id,"content":message_text(message)});
                if message.is_error {
                    result["is_error"] = Value::Bool(true);
                }
                messages.push(json!({"role":"user","content":[result]}));
            }
        }
    }
    let mut body = json!({"model":request.model,"messages":messages,"max_tokens":request.options.max_tokens.unwrap_or(4096)});
    if !system.is_empty() {
        body["system"] = json!(system.join("\n\n"));
    }
    if let Some(value) = request.options.temperature {
        body["temperature"] = json!(value);
    }
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(request.tools.iter().map(|tool| json!({"name":tool.name,"description":tool.description,"input_schema":tool.parameters})).collect());
    }
    if streaming {
        body["stream"] = Value::Bool(true);
    }
    merge_extensions(
        &mut body,
        &request.extensions,
        &[
            "model",
            "messages",
            "max_tokens",
            "system",
            "temperature",
            "tools",
            "stream",
        ],
    )?;
    Ok(body)
}

fn google_payload(request: &ChatRequest) -> Result<Value, ChatError> {
    let mut system = Vec::new();
    let mut contents = Vec::new();
    for message in &request.messages {
        match message.role {
            MessageRole::System => {
                let text = message_text(message);
                if !text.is_empty() {
                    system.push(text);
                }
            }
            MessageRole::User => {
                contents.push(json!({"role":"user","parts":google_parts(&message.content)}))
            }
            MessageRole::Assistant => {
                let mut parts = Vec::new();
                let text = message_text(message);
                if !text.is_empty() {
                    parts.push(json!({"text":text}));
                }
                for call in &message.tool_calls {
                    parts.push(json!({"functionCall":{"name":call.name,"args":arguments_value(&call.arguments)}}));
                }
                contents.push(json!({"role":"model","parts":parts}));
            }
            MessageRole::Tool => {
                let name = message
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .ok_or(ChatError::InvalidConfiguration(
                        "Google tool result requires tool name",
                    ))?;
                contents.push(json!({"role":"user","parts":[{"functionResponse":{"name":name,"response":{"result":message_text(message)}}}]}));
            }
        }
    }
    let mut body = json!({"contents":contents});
    if !system.is_empty() {
        body["systemInstruction"] = json!({"parts":[{"text":system.join("\n\n")}]});
    }
    let mut generation = Map::new();
    if let Some(value) = request.options.temperature {
        generation.insert("temperature".into(), json!(value));
    }
    if let Some(value) = request.options.max_tokens {
        generation.insert("maxOutputTokens".into(), json!(value));
    }
    if !generation.is_empty() {
        body["generationConfig"] = Value::Object(generation);
    }
    if !request.tools.is_empty() {
        body["tools"] = json!([{"functionDeclarations":request.tools.iter().map(google_tool).collect::<Result<Vec<_>,_>>()?}]);
    }
    merge_extensions(
        &mut body,
        &request.extensions,
        &["contents", "systemInstruction", "generationConfig", "tools"],
    )?;
    Ok(body)
}

fn cohere_payload(request: &ChatRequest, streaming: bool) -> Result<Value, ChatError> {
    let mut messages = Vec::with_capacity(request.messages.len());
    for message in &request.messages {
        match message.role {
            MessageRole::System | MessageRole::User | MessageRole::Assistant => {
                let role = match message.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    _ => "assistant",
                };
                let content = cohere_content(&message.content);
                if role == "assistant" && !message.tool_calls.is_empty() {
                    let mut translated = json!({"role":"assistant","tool_calls":message.tool_calls.iter().map(|call| json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments.wire_value()}})).collect::<Vec<_>>()});
                    if let Some(text) = content.as_str().filter(|text| !text.is_empty()) {
                        translated["tool_plan"] = json!(text);
                    }
                    messages.push(translated);
                } else {
                    messages.push(json!({"role":role,"content":content}));
                }
            }
            MessageRole::Tool => {
                let id = message
                    .tool_call_id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .ok_or(ChatError::InvalidConfiguration(
                        "Cohere tool result requires tool_call_id",
                    ))?;
                messages.push(json!({"role":"tool","tool_call_id":id,"content":[{"type":"document","document":{"data":{"result":message_text(message)}}}]}));
            }
        }
    }
    let mut body = json!({"model":request.model,"messages":messages,"stream":streaming});
    if let Some(value) = request.options.temperature {
        body["temperature"] = json!(value);
    }
    if let Some(value) = request.options.max_tokens {
        body["max_tokens"] = json!(value);
    }
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(request.tools.iter().map(|tool| json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.parameters}})).collect());
    }
    merge_extensions(
        &mut body,
        &request.extensions,
        &[
            "model",
            "messages",
            "stream",
            "temperature",
            "max_tokens",
            "tools",
        ],
    )?;
    Ok(body)
}

fn cohere_content(content: &MessageContent) -> Value {
    let parts = match content {
        MessageContent::Text(text) => return json!(text),
        MessageContent::Parts(parts) => parts,
    };
    if !parts.iter().any(|part| {
        matches!(
            part,
            super::ContentPart::ImageUrl { .. }
                | super::ContentPart::Inline(super::media::InlineMedia {
                    kind: super::media::MediaKind::Document,
                    ..
                })
        )
    }) {
        return json!(
            parts
                .iter()
                .filter_map(|part| match part {
                    super::ContentPart::Text { text } => Some(text.clone()),
                    other => other.fallback_text(),
                })
                .collect::<String>()
        );
    }
    Value::Array(
        parts
            .iter()
            .map(|part| match part {
                super::ContentPart::Text { text } => json!({"type":"text","text":text}),
                super::ContentPart::ImageUrl { image_url } => {
                    json!({"type":"image_url","image_url":{"url":image_url.url}})
                }
                other => json!({
                    "type":"text",
                    "text": other.fallback_text().unwrap_or_default(),
                }),
            })
            .collect(),
    )
}

fn parse_cohere_completion(value: Value) -> Result<ChatCompletion, ChatError> {
    let message = value
        .get("message")
        .and_then(Value::as_object)
        .ok_or(ChatError::InvalidResponse)?;
    let content = message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<String>();
    let mut calls = Vec::new();
    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let function = call.get("function").unwrap_or(&Value::Null);
        let raw = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}");
        calls.push(ToolCall {
            id: call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            name: function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            arguments: ToolArguments::from_json(raw),
        });
    }
    let usage = value.pointer("/usage/tokens").and_then(|tokens| {
        let input = tokens.get("input_tokens")?.as_u64().unwrap_or(0);
        let output = tokens.get("output_tokens")?.as_u64().unwrap_or(0);
        Some(Usage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input.saturating_add(output),
            cost: None,
        })
    });
    let finish_reason = if !calls.is_empty() {
        Some(FinishReason::ToolCalls)
    } else {
        value
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(|reason| match reason {
                "COMPLETE" => FinishReason::Stop,
                "MAX_TOKENS" => FinishReason::Length,
                other => FinishReason::Other(other.to_owned()),
            })
    };
    Ok(ChatCompletion {
        contract_version: CHAT_CONTRACT_VERSION,
        id: value.get("id").and_then(Value::as_str).map(str::to_owned),
        selected_model_id: None,
        content,
        reasoning: None,
        tool_calls: calls,
        finish_reason,
        usage,
        media_decisions: Vec::new(),
    })
}

fn parse_cohere_event(
    value: &Value,
    usage: &mut Option<Usage>,
    finish: &mut Option<FinishReason>,
    indexes: &mut HashMap<String, u32>,
    next_index: &mut u32,
) -> Result<Vec<ChatEvent>, ChatError> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let delta = value.get("delta").unwrap_or(&Value::Null);
    let message = delta.get("message").unwrap_or(&Value::Null);
    let mut events = Vec::new();
    match kind {
        "content-delta" => {
            if let Some(text) = message
                .pointer("/content/text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                events.push(ChatEvent::new(ChatEventKind::TextDelta {
                    text: text.to_owned(),
                }));
            }
        }
        "tool-call-start" | "tool-call-delta" => {
            let id = message
                .pointer("/tool_calls/id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let index = if let Some(index) = indexes.get(&id) {
                *index
            } else {
                let explicit = value.get("index").and_then(Value::as_u64).map(|n| n as u32);
                let index = explicit.unwrap_or(*next_index);
                *next_index = (*next_index).max(index.saturating_add(1));
                indexes.insert(id.clone(), index);
                index
            };
            let function = message
                .pointer("/tool_calls/function")
                .unwrap_or(&Value::Null);
            events.push(ChatEvent::new(ChatEventKind::ToolCallDelta {
                call: ToolCallDelta {
                    index,
                    id: (!id.is_empty()).then_some(id),
                    name: function
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    arguments_fragment: function
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                },
            }));
        }
        "message-end" => {
            if let Some(reason) = delta.get("finish_reason").and_then(Value::as_str) {
                *finish = Some(match reason {
                    "COMPLETE" => FinishReason::Stop,
                    "MAX_TOKENS" => FinishReason::Length,
                    "TOOL_CALL" => FinishReason::ToolCalls,
                    other => FinishReason::Other(other.to_owned()),
                });
            }
            if let Some(tokens) = delta.pointer("/usage/tokens") {
                let input = tokens
                    .get("input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let output = tokens
                    .get("output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let latest = Usage {
                    input_tokens: input,
                    output_tokens: output,
                    total_tokens: input.saturating_add(output),
                    cost: None,
                };
                *usage = Some(latest.clone());
                events.push(ChatEvent::new(ChatEventKind::Usage { usage: latest }));
            }
            events.push(ChatEvent::new(ChatEventKind::Completed {
                finish_reason: finish.clone(),
            }));
        }
        "error" => {
            return Err(ChatError::Protocol(
                "Cohere signaled an error during streaming",
            ));
        }
        _ => {}
    }
    Ok(events)
}

fn merge_extensions(
    body: &mut Value,
    extensions: &BTreeMap<String, Value>,
    reserved: &[&str],
) -> Result<(), ChatError> {
    for (key, value) in extensions {
        if key.trim().is_empty() || reserved.contains(&key.as_str()) {
            return Err(ChatError::InvalidConfiguration(
                "provider extension uses an empty or reserved field",
            ));
        }
        body[key] = value.clone();
    }
    Ok(())
}

fn message_text(message: &Message) -> String {
    match &message.content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                super::ContentPart::Text { text } => Some(text.clone()),
                other => other.fallback_text(),
            })
            .collect(),
    }
}

fn anthropic_content(content: &MessageContent) -> Value {
    let MessageContent::Parts(parts) = content else {
        return serde_json::to_value(content).unwrap_or(Value::Null);
    };
    if !parts.iter().any(|part| {
        matches!(part, super::ContentPart::ImageUrl { .. })
            || matches!(
                part,
                super::ContentPart::Inline(item)
                    if item.kind == super::media::MediaKind::Document
                        && item.mime_type == "application/pdf"
            )
    }) {
        return json!(
            parts
                .iter()
                .filter_map(|part| match part {
                    super::ContentPart::Text { text } => Some(text.clone()),
                    other => other.fallback_text(),
                })
                .collect::<String>()
        );
    }
    Value::Array(
        parts
            .iter()
            .map(|part| match part {
                super::ContentPart::Text { text } => json!({"type":"text","text":text}),
                super::ContentPart::ImageUrl { image_url } => anthropic_image(&image_url.url),
                super::ContentPart::Inline(item)
                    if item.kind == super::media::MediaKind::Document
                        && item.mime_type == "application/pdf" =>
                {
                    json!({"type":"document","source":{"type":"base64","media_type":item.mime_type,"data":item.data_base64}})
                }
                other => json!({
                    "type":"text",
                    "text": other.fallback_text().unwrap_or_default(),
                }),
            })
            .collect(),
    )
}

fn anthropic_image(url: &str) -> Value {
    if let Some((media, data)) = url
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(";base64,"))
    {
        json!({"type":"image","source":{"type":"base64","media_type":media,"data":data}})
    } else {
        json!({"type":"image","source":{"type":"url","url":url}})
    }
}

fn google_parts(content: &MessageContent) -> Vec<Value> {
    let mut parts = Vec::new();
    match content {
        MessageContent::Text(text) => {
            if !text.is_empty() {
                parts.push(json!({"text":text}));
            }
        }
        MessageContent::Parts(content_parts) => {
            for part in content_parts {
                match part {
                    super::ContentPart::Text { text } if !text.is_empty() => {
                        parts.push(json!({"text":text}))
                    }
                    super::ContentPart::ImageUrl { image_url } => {
                        let url = &image_url.url;
                        if let Some((media, data)) = url
                            .strip_prefix("data:")
                            .and_then(|rest| rest.split_once(";base64,"))
                        {
                            parts.push(json!({"inlineData":{"mimeType":media,"data":data}}));
                        } else {
                            parts.push(json!({"fileData":{"fileUri":url}}));
                        }
                    }
                    super::ContentPart::Inline(item) => {
                        parts.push(json!({"inlineData":{"mimeType":item.mime_type,"data":item.data_base64}}));
                    }
                    other => {
                        if let Some(text) = other.fallback_text() {
                            parts.push(json!({"text":text}));
                        }
                    }
                }
            }
        }
    }
    if parts.is_empty() {
        parts.push(json!({"text":""}));
    }
    parts
}

fn google_tool(tool: &super::ToolDefinition) -> Result<Value, ChatError> {
    let defs = tool
        .parameters
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let parameters = inline_refs(&tool.parameters, &defs, &mut HashSet::new());
    Ok(
        json!({"name":tool.name,"description":tool.description,"parameters":strip_additional_properties(parameters)}),
    )
}

fn inline_refs(value: &Value, defs: &Map<String, Value>, seen: &mut HashSet<String>) -> Value {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/$defs/"))
            {
                if !seen.insert(reference.to_owned()) {
                    return json!({"type":"object"});
                }
                let resolved = defs
                    .get(reference)
                    .map(|item| inline_refs(item, defs, seen))
                    .unwrap_or(json!({"type":"object"}));
                seen.remove(reference);
                return resolved;
            }
            Value::Object(
                map.iter()
                    .filter(|(key, _)| key.as_str() != "$defs")
                    .map(|(key, item)| (key.clone(), inline_refs(item, defs, seen)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| inline_refs(item, defs, seen))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn strip_additional_properties(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| key != "additionalProperties")
                .map(|(key, value)| (key, strip_additional_properties(value)))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(strip_additional_properties).collect())
        }
        other => other,
    }
}

fn arguments_value(arguments: &ToolArguments) -> Value {
    match arguments {
        ToolArguments::Object { value } => Value::Object(value.clone().into_iter().collect()),
        ToolArguments::InvalidJson { raw } => {
            serde_json::from_str(raw).unwrap_or(Value::String(raw.clone()))
        }
    }
}

fn parse_anthropic_completion(value: Value) -> Result<ChatCompletion, ChatError> {
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or(ChatError::Protocol(
            "Anthropic completion has no content blocks",
        ))?;
    let mut content = String::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                content.push_str(block.get("text").and_then(Value::as_str).unwrap_or(""))
            }
            Some("tool_use") => calls.push(ToolCall {
                id: block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                name: block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                arguments: arguments_from_value(block.get("input").cloned().unwrap_or(json!({}))),
            }),
            _ => {}
        }
    }
    let usage = parse_anthropic_usage(value.get("usage"));
    let finish_reason = if calls.is_empty() {
        value
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(anthropic_finish)
    } else {
        Some(FinishReason::ToolCalls)
    };
    Ok(ChatCompletion {
        contract_version: CHAT_CONTRACT_VERSION,
        id: value.get("id").and_then(Value::as_str).map(str::to_owned),
        selected_model_id: None,
        content,
        reasoning: None,
        tool_calls: calls,
        finish_reason,
        usage,
        media_decisions: Vec::new(),
    })
}

fn parse_google_completion(value: Value) -> Result<ChatCompletion, ChatError> {
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or(ChatError::Protocol(
            "Google returned no candidates (possibly blocked by safety settings)",
        ))?;
    let candidate = &candidates[0];
    let parts = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (content, calls) = parse_google_parts(&parts);
    let finish_reason = if calls.is_empty() {
        candidate
            .get("finishReason")
            .and_then(Value::as_str)
            .map(google_finish)
    } else {
        Some(FinishReason::ToolCalls)
    };
    Ok(ChatCompletion {
        contract_version: CHAT_CONTRACT_VERSION,
        id: None,
        selected_model_id: None,
        content,
        reasoning: None,
        tool_calls: calls,
        finish_reason,
        usage: parse_google_usage(value.get("usageMetadata")),
        media_decisions: Vec::new(),
    })
}

fn parse_google_parts(parts: &[Value]) -> (String, Vec<ToolCall>) {
    let mut content = String::new();
    let mut calls = Vec::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            content.push_str(text);
        }
        if let Some(call) = part.get("functionCall") {
            let name = call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            calls.push(ToolCall {
                id: format!("{name}_call"),
                name,
                arguments: arguments_from_value(call.get("args").cloned().unwrap_or(json!({}))),
            });
        }
    }
    (content, calls)
}

fn arguments_from_value(value: Value) -> ToolArguments {
    match value {
        Value::Object(map) => ToolArguments::Object {
            value: map.into_iter().collect(),
        },
        other => ToolArguments::InvalidJson {
            raw: other.to_string(),
        },
    }
}

fn parse_anthropic_usage(value: Option<&Value>) -> Option<Usage> {
    let value = value?;
    let input = value
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = value
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    Some(Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input.saturating_add(output),
        cost: None,
    })
}

fn parse_google_usage(value: Option<&Value>) -> Option<Usage> {
    let value = value?;
    let input = value
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = value
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    Some(Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: value
            .get("totalTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(input.saturating_add(output)),
        cost: None,
    })
}

fn anthropic_finish(reason: &str) -> FinishReason {
    match reason {
        "end_turn" | "stop_sequence" => FinishReason::Stop,
        "max_tokens" => FinishReason::Length,
        "tool_use" => FinishReason::ToolCalls,
        other => FinishReason::Other(other.to_owned()),
    }
}

fn google_finish(reason: &str) -> FinishReason {
    match reason {
        "STOP" => FinishReason::Stop,
        "MAX_TOKENS" => FinishReason::Length,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" => FinishReason::ContentFilter,
        other => FinishReason::Other(other.to_owned()),
    }
}

fn parse_anthropic_event(
    value: &Value,
    blocks: &mut HashMap<u32, (String, String)>,
    emitted_names: &mut HashSet<u32>,
    usage: &mut Option<Usage>,
    finish: &mut Option<FinishReason>,
) -> Result<Vec<ChatEvent>, ChatError> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let mut events = Vec::new();
    match kind {
        "error" => {
            return Err(ChatError::Protocol(
                "Anthropic signaled an error during streaming",
            ));
        }
        "content_block_start" => {
            let index = value
                .get("index")
                .and_then(Value::as_u64)
                .ok_or(ChatError::Protocol("Anthropic content block has no index"))?
                as u32;
            let block = value.get("content_block").unwrap_or(&Value::Null);
            if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                blocks.insert(
                    index,
                    (
                        block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                        block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                    ),
                );
            }
        }
        "content_block_delta" => {
            let index = value
                .get("index")
                .and_then(Value::as_u64)
                .ok_or(ChatError::Protocol("Anthropic content delta has no index"))?
                as u32;
            let delta = value.get("delta").unwrap_or(&Value::Null);
            match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => {
                    if let Some(text) = delta
                        .get("text")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                    {
                        events.push(ChatEvent::new(ChatEventKind::TextDelta {
                            text: text.to_owned(),
                        }));
                    }
                }
                Some("input_json_delta") => {
                    let (id, name) = blocks.get(&index).cloned().unwrap_or_default();
                    let name = if emitted_names.insert(index) {
                        Some(name)
                    } else {
                        None
                    };
                    events.push(ChatEvent::new(ChatEventKind::ToolCallDelta {
                        call: ToolCallDelta {
                            index,
                            id: Some(id),
                            name,
                            arguments_fragment: delta
                                .get("partial_json")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned(),
                        },
                    }));
                }
                _ => {}
            }
        }
        "message_start" => {
            *usage = parse_anthropic_usage(value.pointer("/message/usage"));
        }
        "message_delta" => {
            if let Some(reason) = value.pointer("/delta/stop_reason").and_then(Value::as_str) {
                *finish = Some(anthropic_finish(reason));
            }
            if let Some(latest) = parse_anthropic_usage(value.get("usage")) {
                let input = usage.as_ref().map_or(0, |current| current.input_tokens);
                *usage = Some(Usage {
                    input_tokens: input,
                    output_tokens: latest.output_tokens,
                    total_tokens: input.saturating_add(latest.output_tokens),
                    cost: None,
                });
            }
        }
        "message_stop" => {
            if let Some(usage) = usage.clone() {
                events.push(ChatEvent::new(ChatEventKind::Usage { usage }));
            }
            events.push(ChatEvent::new(ChatEventKind::Completed {
                finish_reason: finish.clone(),
            }));
        }
        _ => {}
    }
    Ok(events)
}

fn parse_google_event(
    value: &Value,
    usage: &mut Option<Usage>,
    finish: &mut Option<FinishReason>,
    next_tool_index: &mut u32,
) -> Result<Vec<ChatEvent>, ChatError> {
    let candidates = value.get("candidates").and_then(Value::as_array);
    let Some(candidate) = candidates.and_then(|items| items.first()) else {
        if value.get("promptFeedback").is_some() {
            return Err(ChatError::Protocol(
                "Google blocked the response before producing candidates",
            ));
        }
        if let Some(latest) = parse_google_usage(value.get("usageMetadata")) {
            *usage = Some(latest);
        }
        return Ok(Vec::new());
    };
    let mut events = Vec::new();
    let parts = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut saw_tool_call = false;
    for part in parts {
        if let Some(text) = part
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            events.push(ChatEvent::new(ChatEventKind::TextDelta {
                text: text.to_owned(),
            }));
        }
        if let Some(call) = part.get("functionCall") {
            saw_tool_call = true;
            let name = call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let args = call.get("args").cloned().unwrap_or(json!({}));
            let index = *next_tool_index;
            *next_tool_index = next_tool_index.saturating_add(1);
            events.push(ChatEvent::new(ChatEventKind::ToolCallDelta {
                call: ToolCallDelta {
                    index,
                    id: Some(format!("{name}_call")),
                    name: Some(name),
                    arguments_fragment: args.to_string(),
                },
            }));
        }
    }
    if saw_tool_call {
        *finish = Some(FinishReason::ToolCalls);
    } else if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str)
        && !(reason == "STOP" && matches!(finish, Some(FinishReason::ToolCalls)))
    {
        *finish = Some(google_finish(reason));
    }
    if let Some(latest) = parse_google_usage(value.get("usageMetadata")) {
        *usage = Some(latest);
    }
    Ok(events)
}

async fn http_error(response: Response, deadline: Instant, provider: NativeProvider) -> ChatError {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    let body = read_body(
        response,
        deadline,
        Duration::from_secs(2),
        MAX_ERROR_BODY_BYTES,
    )
    .await
    .unwrap_or_default();
    let code = status.as_u16();
    if code == 529 {
        ChatError::RateLimited { retry_after }
    } else if code == 403 && provider == NativeProvider::Google {
        ChatError::Authentication
    } else {
        classify_provider_error(code, retry_after, &body)
    }
}

async fn read_body(
    response: Response,
    deadline: Instant,
    read_timeout: Duration,
    max_bytes: usize,
) -> Result<Vec<u8>, ChatError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(ChatError::ResponseTooLarge);
    }
    let mut body = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = next_chunk(&mut body, deadline, read_timeout).await? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn next_chunk<S>(
    body: &mut S,
    deadline: Instant,
    read_timeout: Duration,
) -> Result<Option<bytes::Bytes>, ChatError>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    match timeout_at((Instant::now() + read_timeout).min(deadline), body.next()).await {
        Err(_) => Err(ChatError::Timeout),
        Ok(Some(Err(error))) if error.is_timeout() => Err(ChatError::Timeout),
        Ok(Some(Err(error))) if error.is_connect() => Err(ChatError::Connection),
        Ok(Some(Err(_))) => Err(ChatError::Connection),
        Ok(Some(Ok(chunk))) => Ok(Some(chunk)),
        Ok(None) => Ok(None),
    }
}

fn safe_endpoint(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(url) => format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or("configured endpoint"),
            url.port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default()
        ),
        Err(_) => "[invalid endpoint]".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> ChatRequest {
        ChatRequest::new("model-x", vec![Message::text(MessageRole::User, "hello")])
    }

    #[test]
    fn anthropic_maps_system_images_and_tool_results() {
        let mut request = ChatRequest::new(
            "claude",
            vec![
                Message::text(MessageRole::System, "one"),
                Message::text(MessageRole::System, "two"),
                Message {
                    role: MessageRole::User,
                    content: MessageContent::Parts(vec![
                        super::super::ContentPart::Text { text: "see".into() },
                        super::super::ContentPart::ImageUrl {
                            image_url: super::super::ImageUrl {
                                url: "data:image/png;base64,abc".into(),
                                detail: None,
                            },
                        },
                    ]),
                    tool_calls: vec![],
                    tool_call_id: None,
                    name: None,
                    is_error: false,
                },
                Message {
                    role: MessageRole::Tool,
                    content: MessageContent::Text("failed".into()),
                    tool_calls: vec![],
                    tool_call_id: Some("call-1".into()),
                    name: Some("read".into()),
                    is_error: true,
                },
            ],
        );
        request.options.max_tokens = Some(23);
        let payload = anthropic_payload(&request, true).unwrap();
        assert_eq!(payload["system"], "one\n\ntwo");
        assert_eq!(payload["max_tokens"], 23);
        assert_eq!(
            payload["messages"][0]["content"][1]["source"]["data"],
            "abc"
        );
        assert_eq!(payload["messages"][1]["content"][0]["is_error"], true);
        assert_eq!(payload["stream"], true);
    }

    #[test]
    fn native_media_uses_provider_wire_formats() {
        let mut request = req();
        request.messages[0].content = MessageContent::Parts(vec![
            super::super::ContentPart::Text {
                text: "inspect this".into(),
            },
            super::super::ContentPart::Inline(super::super::media::InlineMedia {
                kind: super::super::media::MediaKind::Document,
                mime_type: "application/pdf".into(),
                name: "report.pdf".into(),
                data_base64: "JVBERi0=".into(),
            }),
            super::super::ContentPart::Inline(super::super::media::InlineMedia {
                kind: super::super::media::MediaKind::Audio,
                mime_type: "audio/wav".into(),
                name: "voice.wav".into(),
                data_base64: "UklGRg==".into(),
            }),
            super::super::ContentPart::Inline(super::super::media::InlineMedia {
                kind: super::super::media::MediaKind::Video,
                mime_type: "video/mp4".into(),
                name: "clip.mp4".into(),
                data_base64: "AAAA".into(),
            }),
        ]);

        let anthropic = anthropic_payload(&request, false).unwrap();
        assert_eq!(anthropic["messages"][0]["content"][0]["type"], "text");
        assert_eq!(anthropic["messages"][0]["content"][1]["type"], "document");
        assert_eq!(
            anthropic["messages"][0]["content"][1]["source"],
            json!({
                "type": "base64",
                "media_type": "application/pdf",
                "data": "JVBERi0="
            })
        );

        let google = google_payload(&request).unwrap();
        assert_eq!(google["contents"][0]["parts"][0]["text"], "inspect this");
        assert_eq!(
            google["contents"][0]["parts"][1]["inlineData"],
            json!({"mimeType":"application/pdf","data":"JVBERi0="})
        );
        assert_eq!(
            google["contents"][0]["parts"][2]["inlineData"],
            json!({"mimeType":"audio/wav","data":"UklGRg=="})
        );
        assert_eq!(
            google["contents"][0]["parts"][3]["inlineData"],
            json!({"mimeType":"video/mp4","data":"AAAA"})
        );
    }

    #[test]
    fn google_inlines_defs_and_maps_extensions_without_product_defaults() {
        let mut request = req();
        request.tools.push(super::super::ToolDefinition { name: "lookup".into(), description: None, parameters: json!({"type":"object","properties":{"item":{"$ref":"#/$defs/Item"}},"$defs":{"Item":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string"}}}}}) });
        request
            .extensions
            .insert("safetySettings".into(), json!([]));
        let payload = google_payload(&request).unwrap();
        assert_eq!(
            payload["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["item"]["properties"]
                ["name"]["type"],
            "string"
        );
        assert!(
            payload["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["item"]
                .get("additionalProperties")
                .is_none()
        );
        assert_eq!(payload["safetySettings"], json!([]));
    }

    #[test]
    fn parse_responses_and_token_usage_for_both_protocols() {
        let anthropic = parse_anthropic_completion(json!({"id":"m1","stop_reason":"tool_use","content":[{"type":"tool_use","id":"c1","name":"add","input":{"x":1}}],"usage":{"input_tokens":7,"output_tokens":3}})).unwrap();
        assert_eq!(anthropic.finish_reason, Some(FinishReason::ToolCalls));
        assert_eq!(anthropic.usage.unwrap().total_tokens, 10);
        let google = parse_google_completion(json!({"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"ok"}]}}],"usageMetadata":{"promptTokenCount":4,"candidatesTokenCount":5,"totalTokenCount":9}})).unwrap();
        assert_eq!(google.finish_reason, Some(FinishReason::Length));
        assert_eq!(google.usage.unwrap().total_tokens, 9);
        let cohere = parse_cohere_completion(json!({"id":"c1","finish_reason":"TOOL_CALL","message":{"content":[{"type":"text","text":"plan"}],"tool_calls":[{"id":"call-1","function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]},"usage":{"tokens":{"input_tokens":3,"output_tokens":2}}})).unwrap();
        assert_eq!(cohere.content, "plan");
        assert_eq!(cohere.finish_reason, Some(FinishReason::ToolCalls));
        assert_eq!(cohere.usage.unwrap().total_tokens, 5);
        assert_eq!(cohere.tool_calls[0].name, "lookup");
    }

    #[test]
    fn cohere_request_and_stream_events_preserve_tools_usage_and_extensions() {
        let mut request = ChatRequest::new(
            "command-a",
            vec![
                Message::text(MessageRole::System, "rules"),
                Message::text(MessageRole::User, "find it"),
                Message {
                    role: MessageRole::Assistant,
                    content: MessageContent::Text("plan".into()),
                    tool_calls: vec![ToolCall {
                        id: "call-1".into(),
                        name: "lookup".into(),
                        arguments: ToolArguments::from_json("{\"q\":\"x\"}"),
                    }],
                    tool_call_id: None,
                    name: None,
                    is_error: false,
                },
                Message {
                    role: MessageRole::Tool,
                    content: MessageContent::Text("found".into()),
                    tool_calls: vec![],
                    tool_call_id: Some("call-1".into()),
                    name: Some("lookup".into()),
                    is_error: false,
                },
            ],
        );
        request.tools.push(super::super::ToolDefinition {
            name: "lookup".into(),
            description: Some("search".into()),
            parameters: json!({"type":"object"}),
        });
        request
            .extensions
            .insert("thinking".into(), json!({"type":"enabled"}));
        let body = cohere_payload(&request, true).unwrap();
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][2]["tool_plan"], "plan");
        assert_eq!(
            body["messages"][2]["tool_calls"][0]["function"]["arguments"],
            "{\"q\":\"x\"}"
        );
        assert_eq!(body["messages"][3]["content"][0]["type"], "document");
        assert_eq!(body["tools"][0]["function"]["name"], "lookup");
        assert_eq!(body["thinking"]["type"], "enabled");

        let mut usage = None;
        let mut finish = None;
        let mut indexes = HashMap::new();
        let mut next = 0;
        let events = parse_cohere_event(&json!({"type":"tool-call-start","index":4,"delta":{"message":{"tool_calls":{"id":"call-1","function":{"name":"lookup","arguments":""}}}}}), &mut usage, &mut finish, &mut indexes, &mut next).unwrap();
        let ChatEventKind::ToolCallDelta { call } = &events[0].event else {
            panic!("expected tool call")
        };
        assert_eq!(call.index, 4);
        assert_eq!(call.name.as_deref(), Some("lookup"));
        let events = parse_cohere_event(&json!({"type":"message-end","delta":{"finish_reason":"TOOL_CALL","usage":{"tokens":{"input_tokens":3,"output_tokens":2}}}}), &mut usage, &mut finish, &mut indexes, &mut next).unwrap();
        assert!(matches!(events[0].event, ChatEventKind::Usage { .. }));
        assert!(matches!(events[1].event, ChatEventKind::Completed { .. }));
        assert_eq!(finish, Some(FinishReason::ToolCalls));
        assert_eq!(usage.unwrap().total_tokens, 5);
    }

    #[test]
    fn native_settings_validate_endpoints_and_redact_credentials() {
        let settings = NativeChatSettings {
            provider: NativeProvider::Google,
            base_url: Some("https://example.test/v1beta?token=not-allowed".into()),
            api_key: "sensitive-key".into(),
            headers: BTreeMap::new(),
            connect_timeout_ms: 10_000,
            read_timeout_ms: 60_000,
            total_timeout_ms: 300_000,
        };
        let error = NativeChatClient::new(settings).unwrap_err();
        assert!(matches!(error, ChatError::InvalidConfiguration(_)));

        let settings = NativeChatSettings {
            provider: NativeProvider::Anthropic,
            base_url: None,
            api_key: "sensitive-key".into(),
            headers: BTreeMap::new(),
            connect_timeout_ms: 10_000,
            read_timeout_ms: 60_000,
            total_timeout_ms: 300_000,
        };
        let debug = format!("{settings:?}");
        assert!(!debug.contains("sensitive-key"));
    }

    #[test]
    fn anthropic_events_keep_tool_argument_fragments_and_google_usage_is_cumulative() {
        let mut blocks = HashMap::new();
        let mut names = HashSet::new();
        let mut usage = None;
        let mut finish = None;
        let start = parse_anthropic_event(&json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"c2","name":"edit"}}), &mut blocks, &mut names, &mut usage, &mut finish).unwrap();
        assert!(start.is_empty());
        let fragment = parse_anthropic_event(&json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\\\"a\\\":"}}), &mut blocks, &mut names, &mut usage, &mut finish).unwrap();
        let ChatEventKind::ToolCallDelta { call } = &fragment[0].event else {
            panic!("expected tool call")
        };
        assert_eq!(call.name.as_deref(), Some("edit"));
        assert_eq!(call.arguments_fragment, "{\\\"a\\\":");
        let mut cumulative = None;
        let mut next_tool_index = 0;
        let _ = parse_google_event(&json!({"candidates":[{"content":{"parts":[{"text":"hi"}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}}), &mut cumulative, &mut finish, &mut next_tool_index).unwrap();
        let _ = parse_google_event(&json!({"candidates":[{"content":{"parts":[{"text":"!"}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":4,"totalTokenCount":6}}), &mut cumulative, &mut finish, &mut next_tool_index).unwrap();
        assert_eq!(cumulative.unwrap().total_tokens, 6);

        let first = parse_google_event(&json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"one","args":{"x":1} }},{"functionCall":{"name":"two","args":{} }}]}}]}), &mut None, &mut finish, &mut next_tool_index).unwrap();
        let ChatEventKind::ToolCallDelta { call: first_call } = &first[0].event else {
            panic!("expected first tool call")
        };
        let ChatEventKind::ToolCallDelta { call: second_call } = &first[1].event else {
            panic!("expected second tool call")
        };
        assert_eq!((first_call.index, second_call.index), (0, 1));
        assert_eq!(finish, Some(FinishReason::ToolCalls));
        let _ = parse_google_event(
            &json!({"candidates":[{"finishReason":"STOP","content":{"parts":[]}}]}),
            &mut None,
            &mut finish,
            &mut next_tool_index,
        )
        .unwrap();
        assert_eq!(finish, Some(FinishReason::ToolCalls));
    }
}
