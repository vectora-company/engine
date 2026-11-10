use std::{collections::HashSet, fmt, time::Duration};

use async_stream::try_stream;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::{Client, Response, header};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::time::{Instant, timeout_at};
use url::Url;

use crate::chat::{
    ChatError, ContentPart,
    error::classify_provider_error,
    sse::SseDecoder,
    types::{
        CHAT_CONTRACT_VERSION, ChatCompletion, ChatEvent, ChatEventKind, ChatRequest, FinishReason,
        Message, MessageContent, MessageRole, ToolArguments, ToolCall, ToolCallDelta, Usage,
    },
};

const CHAT_PATH: &str = "chat/completions";
const MAX_ERROR_BODY_BYTES: usize = 8 * 1024;

/// Serde-friendly options accepted by the Python and Node bindings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAiCompatibleSettings {
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
    #[serde(default = "default_connect_timeout_ms")]
    connect_timeout_ms: u64,
    #[serde(default = "default_read_timeout_ms")]
    read_timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    total_timeout_ms: u64,
    #[serde(default = "default_retry_connect_attempts")]
    retry_connect_attempts: u8,
    #[serde(default = "default_max_response_bytes")]
    max_response_bytes: usize,
    #[serde(default = "default_max_event_bytes")]
    max_event_bytes: usize,
}

impl OpenAiCompatibleSettings {
    /// Convert binding settings to the validated client configuration.
    pub fn into_config(self) -> OpenAiCompatibleConfig {
        let mut config = OpenAiCompatibleConfig::new(self.base_url)
            .timeouts(
                Duration::from_millis(self.connect_timeout_ms),
                Duration::from_millis(self.read_timeout_ms),
                Duration::from_millis(self.total_timeout_ms),
            )
            .retry_connect_attempts(self.retry_connect_attempts)
            .response_limits(self.max_response_bytes, self.max_event_bytes);
        if let Some(api_key) = self.api_key {
            config = config.api_key(api_key);
        }
        for (name, value) in self.headers {
            config = config.header(name, value);
        }
        config
    }
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

const fn default_retry_connect_attempts() -> u8 {
    2
}

const fn default_max_response_bytes() -> usize {
    16 * 1024 * 1024
}

const fn default_max_event_bytes() -> usize {
    1024 * 1024
}

pub type ChatStream = std::pin::Pin<Box<dyn Stream<Item = Result<ChatEvent, ChatError>> + Send>>;

#[derive(Clone)]
pub struct OpenAiCompatibleConfig {
    base_url: String,
    api_key: Option<String>,
    headers: Vec<(String, String)>,
    connect_timeout: Duration,
    read_timeout: Duration,
    total_timeout: Duration,
    retry_connect_attempts: u8,
    max_response_bytes: usize,
    max_event_bytes: usize,
}

impl fmt::Debug for OpenAiCompatibleConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCompatibleConfig")
            .field("base_url", &safe_endpoint(&self.base_url))
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .field("connect_timeout", &self.connect_timeout)
            .field("read_timeout", &self.read_timeout)
            .field("total_timeout", &self.total_timeout)
            .field("retry_connect_attempts", &self.retry_connect_attempts)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("max_event_bytes", &self.max_event_bytes)
            .finish()
    }
}

impl OpenAiCompatibleConfig {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: None,
            headers: Vec::new(),
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(60),
            total_timeout: Duration::from_secs(300),
            retry_connect_attempts: 2,
            max_response_bytes: 16 * 1024 * 1024,
            max_event_bytes: 1024 * 1024,
        }
    }

    pub fn api_key(mut self, value: impl Into<String>) -> Self {
        self.api_key = Some(value.into());
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn timeouts(mut self, connect: Duration, read: Duration, total: Duration) -> Self {
        self.connect_timeout = connect;
        self.read_timeout = read;
        self.total_timeout = total;
        self
    }

    pub fn retry_connect_attempts(mut self, attempts: u8) -> Self {
        self.retry_connect_attempts = attempts;
        self
    }

    pub fn response_limits(mut self, max_response_bytes: usize, max_event_bytes: usize) -> Self {
        self.max_response_bytes = max_response_bytes;
        self.max_event_bytes = max_event_bytes;
        self
    }
}

/// Chat Completions transport for endpoints that explicitly implement that protocol.
pub struct OpenAiCompatibleClient {
    config: OpenAiCompatibleConfig,
    endpoint: Url,
    client: Client,
}

impl fmt::Debug for OpenAiCompatibleClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCompatibleClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl OpenAiCompatibleClient {
    pub fn new(config: OpenAiCompatibleConfig) -> Result<Self, ChatError> {
        let endpoint = validate_config(&config)?;
        let client = Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|_| ChatError::InvalidConfiguration("HTTP client could not be created"))?;
        Ok(Self {
            config,
            endpoint,
            client,
        })
    }

    /// Return one assistant response. Dropping this future cancels the HTTP request.
    pub async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        validate_request(&request)?;
        let payload = request_payload(&request, false);
        let deadline = Instant::now() + self.config.total_timeout;
        let response = self.send_with_connect_retry(payload, deadline).await?;
        if !response.status().is_success() {
            return Err(http_error(response, deadline, self.config.max_response_bytes).await);
        }
        let bytes = read_body(
            response,
            deadline,
            self.config.read_timeout,
            self.config.max_response_bytes,
        )
        .await?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| ChatError::InvalidResponse)?;
        parse_completion(value)
    }

    /// Open a backpressured SSE stream. Dropping it drops the response body and cancels I/O.
    pub async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        validate_request(&request)?;
        let payload = request_payload(&request, true);
        let deadline = Instant::now() + self.config.total_timeout;
        let response = self.send_with_connect_retry(payload, deadline).await?;
        if !response.status().is_success() {
            return Err(http_error(response, deadline, self.config.max_response_bytes).await);
        }

        let read_timeout = self.config.read_timeout;
        let max_response_bytes = self.config.max_response_bytes;
        let max_event_bytes = self.config.max_event_bytes;
        let stream = try_stream! {
            let mut body = response.bytes_stream();
            let mut decoder = SseDecoder::new(max_event_bytes);
            let mut received_bytes = 0usize;
            let mut finish_reason = None;
            let mut completed = false;
            let mut completion_emitted = false;

            loop {
                let Some(chunk) = next_body_chunk(&mut body, deadline, read_timeout).await? else {
                    break;
                };
                received_bytes = received_bytes.saturating_add(chunk.len());
                if received_bytes > max_response_bytes {
                    Err(ChatError::ResponseTooLarge)?;
                }
                for frame in decoder.push(&chunk)? {
                    if frame == "[DONE]" {
                        if !completion_emitted {
                            yield ChatEvent::new(ChatEventKind::Completed {
                                finish_reason: finish_reason.clone(),
                            });
                        }
                        completed = true;
                        break;
                    }
                    for item in parse_stream_event(&frame)? {
                        if let ChatEventKind::Completed { finish_reason: reason } = &item.event {
                            finish_reason = reason.clone();
                            completion_emitted = true;
                        }
                        yield item;
                    }
                }
                if completed {
                    break;
                }
            }

            if !completed {
                for frame in decoder.finish()? {
                    if frame == "[DONE]" {
                        if !completion_emitted {
                            yield ChatEvent::new(ChatEventKind::Completed {
                                finish_reason: finish_reason.clone(),
                            });
                        }
                        completed = true;
                        break;
                    }
                    for item in parse_stream_event(&frame)? {
                        if let ChatEventKind::Completed { finish_reason: reason } = &item.event {
                            finish_reason = reason.clone();
                            completion_emitted = true;
                        }
                        yield item;
                    }
                }
                if !completed && !completion_emitted {
                    yield ChatEvent::new(ChatEventKind::Completed { finish_reason });
                }
            }
        };
        Ok(Box::pin(stream))
    }

    async fn send_with_connect_retry(
        &self,
        payload: Value,
        deadline: Instant,
    ) -> Result<Response, ChatError> {
        let streaming = payload
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut attempts = 0u8;
        loop {
            let mut builder = self.client.post(self.endpoint.clone()).json(&payload);
            if let Some(api_key) = &self.config.api_key {
                builder = builder.bearer_auth(api_key);
            }
            for (name, value) in &self.config.headers {
                let name = header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header name"))?;
                let value = header::HeaderValue::from_str(value)
                    .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header value"))?;
                builder = builder.header(name, value);
            }
            builder = builder.header(
                header::ACCEPT,
                if streaming {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            );

            match timeout_at(deadline, builder.send()).await {
                Err(_) => return Err(ChatError::Timeout),
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(error))
                    if error.is_connect() && attempts < self.config.retry_connect_attempts =>
                {
                    attempts += 1;
                    let shift = u32::from((attempts - 1).min(5));
                    let delay = Duration::from_millis(100u64.saturating_mul(1u64 << shift));
                    if timeout_at(deadline, tokio::time::sleep(delay))
                        .await
                        .is_err()
                    {
                        return Err(ChatError::Timeout);
                    }
                }
                Ok(Err(error)) if error.is_timeout() => return Err(ChatError::Timeout),
                Ok(Err(error)) if error.is_connect() => return Err(ChatError::Connection),
                Ok(Err(_)) => return Err(ChatError::Connection),
            }
        }
    }
}

fn validate_config(config: &OpenAiCompatibleConfig) -> Result<Url, ChatError> {
    for (duration, label) in [
        (config.connect_timeout, "connect timeout"),
        (config.read_timeout, "read timeout"),
        (config.total_timeout, "total timeout"),
    ] {
        if duration.is_zero() {
            return Err(ChatError::InvalidConfiguration(label));
        }
    }
    if config.max_response_bytes == 0
        || config.max_event_bytes == 0
        || config.max_event_bytes > config.max_response_bytes
    {
        return Err(ChatError::InvalidConfiguration("response size limits"));
    }
    if config.retry_connect_attempts > 5 {
        return Err(ChatError::InvalidConfiguration("connect retry attempts"));
    }

    let base = Url::parse(config.base_url.trim())
        .map_err(|_| ChatError::InvalidConfiguration("base URL must be an absolute HTTP URL"))?;
    if !matches!(base.scheme(), "http" | "https")
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(ChatError::InvalidConfiguration(
            "base URL is not a safe HTTP endpoint",
        ));
    }
    if config
        .api_key
        .as_ref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(ChatError::InvalidConfiguration("API key must not be empty"));
    }

    let mut seen = HashSet::new();
    for (name, value) in &config.headers {
        let header_name = header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header name"))?;
        header::HeaderValue::from_str(value)
            .map_err(|_| ChatError::InvalidConfiguration("invalid HTTP header value"))?;
        let canonical = header_name.as_str();
        if matches!(canonical, "authorization" | "content-type" | "host")
            || !seen.insert(canonical.to_owned())
        {
            return Err(ChatError::InvalidConfiguration(
                "reserved or duplicate HTTP header",
            ));
        }
    }

    let mut endpoint = base;
    let path = endpoint.path().trim_end_matches('/');
    endpoint.set_path(&format!("{path}/{CHAT_PATH}"));
    Ok(endpoint)
}

pub(crate) fn validate_request(request: &ChatRequest) -> Result<(), ChatError> {
    if request.contract_version != CHAT_CONTRACT_VERSION {
        return Err(ChatError::UnsupportedContractVersion(
            request.contract_version,
        ));
    }
    if request.model.trim().is_empty() {
        return Err(ChatError::InvalidConfiguration("model must not be empty"));
    }
    if request
        .options
        .temperature
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(ChatError::InvalidConfiguration(
            "temperature must be finite and non-negative",
        ));
    }
    if request.options.max_tokens == Some(0) {
        return Err(ChatError::InvalidConfiguration(
            "max_tokens must be positive",
        ));
    }
    const RESERVED_EXTENSION_FIELDS: [&str; 6] = [
        "model",
        "messages",
        "tools",
        "stream",
        "temperature",
        "max_tokens",
    ];
    if request
        .extensions
        .iter()
        .any(|(key, _)| key.trim().is_empty() || RESERVED_EXTENSION_FIELDS.contains(&key.as_str()))
    {
        return Err(ChatError::InvalidConfiguration(
            "provider extension uses an empty or reserved field",
        ));
    }
    let mut names = HashSet::new();
    for tool in &request.tools {
        if tool.name.trim().is_empty() || !names.insert(tool.name.as_str()) {
            return Err(ChatError::InvalidConfiguration(
                "tool names must be non-empty and unique",
            ));
        }
        if !tool.parameters.is_object() {
            return Err(ChatError::InvalidConfiguration(
                "tool parameters must be a JSON object",
            ));
        }
    }
    for message in &request.messages {
        if message.role == MessageRole::Tool
            && message.tool_call_id.as_deref().unwrap_or("").is_empty()
        {
            return Err(ChatError::InvalidConfiguration(
                "tool messages require tool_call_id",
            ));
        }
        if message.role != MessageRole::Assistant && !message.tool_calls.is_empty() {
            return Err(ChatError::InvalidConfiguration(
                "only assistant messages may contain tool calls",
            ));
        }
        if message
            .tool_calls
            .iter()
            .any(|call| call.name.trim().is_empty())
        {
            return Err(ChatError::InvalidConfiguration(
                "tool call names must not be empty",
            ));
        }
    }
    Ok(())
}

fn request_payload(request: &ChatRequest, stream: bool) -> Value {
    let messages = request
        .messages
        .iter()
        .map(message_payload)
        .collect::<Vec<_>>();
    let mut payload = json!({ "model": request.model, "messages": messages });
    if let Some(temperature) = request.options.temperature {
        payload["temperature"] = json!(temperature);
    }
    if let Some(max_tokens) = request.options.max_tokens {
        payload["max_tokens"] = json!(max_tokens);
    }
    if !request.tools.is_empty() {
        payload["tools"] = Value::Array(
            request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    })
                })
                .collect(),
        );
    }
    for (key, value) in &request.extensions {
        payload[key] = value.clone();
    }
    if stream {
        payload["stream"] = Value::Bool(true);
    }
    payload
}

fn message_payload(message: &Message) -> Value {
    let mut result = Map::new();
    result.insert("role".into(), json!(message.role));
    result.insert("content".into(), content_payload(&message.content));
    if let Some(id) = &message.tool_call_id {
        result.insert("tool_call_id".into(), json!(id));
    }
    if let Some(name) = &message.name {
        result.insert("name".into(), json!(name));
    }
    if !message.tool_calls.is_empty() {
        result.insert(
            "tool_calls".into(),
            Value::Array(
                message
                    .tool_calls
                    .iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": call.arguments.wire_value(),
                            }
                        })
                    })
                    .collect(),
            ),
        );
        if matches!(&message.content, MessageContent::Text(text) if text.is_empty()) {
            result.insert("content".into(), Value::Null);
        }
    }
    Value::Object(result)
}

fn content_payload(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(text) => json!(text),
        MessageContent::Parts(parts) => Value::Array(
            parts
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => json!({ "type": "text", "text": text }),
                    ContentPart::ImageUrl { image_url } => json!({
                        "type": "image_url",
                        "image_url": {
                            "url": image_url.url,
                            "detail": image_url.detail,
                        }
                    }),
                })
                .collect(),
        ),
    }
}

async fn http_error(response: Response, deadline: Instant, max_bytes: usize) -> ChatError {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    // Inspect only bounded known markers for fallback classification. The body
    // itself is never returned, logged, or included in Debug output.
    let body = read_body(
        response,
        deadline,
        Duration::from_secs(2),
        MAX_ERROR_BODY_BYTES.min(max_bytes),
    )
    .await
    .unwrap_or_default();
    classify_provider_error(status.as_u16(), retry_after, &body)
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
    while let Some(chunk) = next_body_chunk(&mut body, deadline, read_timeout).await? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn next_body_chunk<S>(
    body: &mut S,
    deadline: Instant,
    read_timeout: Duration,
) -> Result<Option<bytes::Bytes>, ChatError>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    let read_deadline = (Instant::now() + read_timeout).min(deadline);
    match timeout_at(read_deadline, body.next()).await {
        Err(_) => Err(ChatError::Timeout),
        Ok(Some(Err(error))) if error.is_timeout() => Err(ChatError::Timeout),
        Ok(Some(Err(error))) if error.is_connect() => Err(ChatError::Connection),
        Ok(Some(Err(_))) => Err(ChatError::Connection),
        Ok(Some(Ok(chunk))) => Ok(Some(chunk)),
        Ok(None) => Ok(None),
    }
}

fn parse_completion(value: Value) -> Result<ChatCompletion, ChatError> {
    let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or(ChatError::Protocol("completion has no choices"))?;
    let message = choice
        .get("message")
        .ok_or(ChatError::Protocol("choice has no message"))?;
    let content = content_text(message.get("content"));
    let tool_calls = parse_tool_calls(message.get("tool_calls"))?;
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(FinishReason::from_provider);
    let usage = parse_usage(value.get("usage"))?;
    Ok(ChatCompletion {
        contract_version: CHAT_CONTRACT_VERSION,
        id,
        selected_model_id: None,
        content,
        reasoning: message
            .get("reasoning")
            .or_else(|| message.get("reasoning_content"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        tool_calls,
        finish_reason,
        usage,
    })
}

fn parse_stream_event(frame: &str) -> Result<Vec<ChatEvent>, ChatError> {
    let value: Value = serde_json::from_str(frame).map_err(|_| ChatError::InvalidResponse)?;
    if value.get("error").is_some() {
        return Err(ChatError::Protocol(
            "provider sent an error in a successful stream",
        ));
    }
    let mut events = Vec::new();
    if let Some(usage) = parse_usage(value.get("usage"))? {
        events.push(ChatEvent::new(ChatEventKind::Usage { usage }));
    }
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .ok_or(ChatError::Protocol("stream event has no choices array"))?;
    if let Some(choice) = choices.first() {
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or(ChatError::Protocol("stream choice has no delta object"))?;
        if let Some(text) = delta.get("content") {
            match text {
                Value::String(text) if !text.is_empty() => {
                    events.push(ChatEvent::new(ChatEventKind::TextDelta {
                        text: text.clone(),
                    }));
                }
                Value::Null => {}
                Value::Array(parts) => {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(Value::as_str)
                            && !text.is_empty()
                        {
                            events.push(ChatEvent::new(ChatEventKind::TextDelta {
                                text: text.to_owned(),
                            }));
                        }
                    }
                }
                _ => return Err(ChatError::Protocol("text delta has an unsupported shape")),
            }
        }
        if let Some(reasoning) = delta
            .get("reasoning")
            .or_else(|| delta.get("reasoning_content"))
        {
            match reasoning {
                Value::String(text) if !text.is_empty() => {
                    events.push(ChatEvent::new(ChatEventKind::ReasoningDelta {
                        text: text.clone(),
                    }));
                }
                Value::Null => {}
                _ => return Err(ChatError::Protocol("reasoning delta is not a string")),
            }
        }
        if let Some(calls) = delta.get("tool_calls").filter(|value| !value.is_null()) {
            let calls = calls
                .as_array()
                .ok_or(ChatError::Protocol("tool_calls delta is not an array"))?;
            for call in calls {
                let function = call.get("function");
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let index = u32::try_from(index)
                    .map_err(|_| ChatError::Protocol("tool call index is out of range"))?;
                let name = function
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let arguments_fragment = function
                    .and_then(|function| function.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let id = call.get("id").and_then(Value::as_str).map(str::to_owned);
                events.push(ChatEvent::new(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index,
                        id,
                        name,
                        arguments_fragment,
                    },
                }));
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            events.push(ChatEvent::new(ChatEventKind::Completed {
                finish_reason: Some(FinishReason::from_provider(reason)),
            }));
        }
    } else if events.is_empty() {
        return Err(ChatError::Protocol(
            "stream event contains neither choices nor usage",
        ));
    }
    Ok(events)
}

fn parse_tool_calls(value: Option<&Value>) -> Result<Vec<ToolCall>, ChatError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let calls = value
        .as_array()
        .ok_or(ChatError::Protocol("tool_calls is not an array"))?;
    calls
        .iter()
        .map(|call| {
            let function = call
                .get("function")
                .ok_or(ChatError::Protocol("tool call has no function"))?;
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            Ok(ToolCall {
                id,
                name,
                arguments: ToolArguments::from_json(arguments),
            })
        })
        .collect()
}

fn parse_usage(value: Option<&Value>) -> Result<Option<Usage>, ChatError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let input = value
        .get("prompt_tokens")
        .or_else(|| value.get("input_tokens"))
        .and_then(Value::as_u64)
        .ok_or(ChatError::Protocol("usage has invalid prompt token count"))?;
    let output = value
        .get("completion_tokens")
        .or_else(|| value.get("output_tokens"))
        .and_then(Value::as_u64)
        .ok_or(ChatError::Protocol(
            "usage has invalid completion token count",
        ))?;
    let total = value
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input.saturating_add(output));
    let cost = match value.get("cost") {
        Some(Value::Number(number)) => Some(
            number
                .as_f64()
                .ok_or(ChatError::Protocol("usage has invalid cost"))?,
        ),
        Some(Value::Null) | None => None,
        Some(_) => return Err(ChatError::Protocol("usage has invalid cost")),
    };
    Ok(Some(Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        cost,
    }))
}

fn content_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect(),
        _ => String::new(),
    }
}

fn safe_endpoint(raw: &str) -> String {
    Url::parse(raw)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?;
            let port = url
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default();
            Some(format!("{}://{host}{port}", url.scheme()))
        })
        .unwrap_or_else(|| "configured endpoint".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ContentPart, ImageUrl, ToolDefinition};

    #[test]
    fn null_optional_usage_and_tool_calls_are_absent_in_completion_and_stream() {
        let completion = parse_completion(json!({
            "choices": [{"message": {"content": "ok", "tool_calls": null}, "finish_reason": "stop"}],
            "usage": null
        })).unwrap();
        assert_eq!(completion.content, "ok");
        assert!(completion.tool_calls.is_empty());
        assert!(completion.usage.is_none());
        let events = parse_stream_event(
            r#"{"choices":[{"delta":{"content":"ok","tool_calls":null}}],"usage":null}"#,
        )
        .unwrap();
        assert!(matches!(events[0].event, ChatEventKind::TextDelta { .. }));
        assert!(parse_tool_calls(Some(&json!({}))).is_err());
        assert!(parse_usage(Some(&json!({}))).is_err());
    }

    #[test]
    fn validates_configuration_before_network_use_and_redacts_secrets() {
        let config =
            OpenAiCompatibleConfig::new("https://user:password@example.test/v1?token=hidden")
                .api_key("super-secret");
        let error = OpenAiCompatibleClient::new(config.clone()).unwrap_err();
        assert!(matches!(error, ChatError::InvalidConfiguration(_)));
        let debug = format!("{config:?}");
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("password"));
    }

    #[test]
    fn validates_request_and_serializes_images_tools_and_tool_history() {
        let mut request = ChatRequest::new(
            "model",
            vec![Message {
                role: MessageRole::User,
                content: MessageContent::Parts(vec![
                    ContentPart::Text {
                        text: "inspect".into(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: "https://example.test/image.png".into(),
                            detail: Some("high".into()),
                        },
                    },
                ]),
                tool_calls: Vec::new(),
                tool_call_id: None,
                name: None,
                is_error: false,
            }],
        );
        request.tools.push(ToolDefinition {
            name: "read_file".into(),
            description: None,
            parameters: json!({"type":"object","properties":{"path":{"type":"string"}}}),
        });
        let payload = request_payload(&request, true);
        assert_eq!(
            payload["messages"][0]["content"][1]["image_url"]["detail"],
            "high"
        );
        assert_eq!(payload["tools"][0]["function"]["name"], "read_file");
        assert_eq!(payload["stream"], true);

        request.tools[0].parameters = json!([]);
        assert!(matches!(
            validate_request(&request),
            Err(ChatError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn invalid_tool_json_is_preserved_as_explicit_state() {
        let arguments = ToolArguments::from_json("{not-json");
        assert_eq!(
            arguments,
            ToolArguments::InvalidJson {
                raw: "{not-json".into()
            }
        );
        assert_eq!(arguments.wire_value(), "{not-json");
    }

    #[test]
    fn provider_extensions_keep_canonical_fields_owned_by_the_engine() {
        let mut request = ChatRequest::new("model", Vec::new());
        request
            .extensions
            .insert("reasoning".into(), json!({"effort": "high"}));
        assert_eq!(
            request_payload(&request, false)["reasoning"]["effort"],
            "high"
        );

        request
            .extensions
            .insert("model".into(), json!("replacement-model"));
        assert!(matches!(
            validate_request(&request),
            Err(ChatError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn reasoning_and_cost_are_preserved_outside_response_text() {
        let completion = parse_completion(json!({
            "choices": [{
                "message": {"content": "visible", "reasoning": "private analysis"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 7,
                "completion_tokens": 3,
                "total_tokens": 10,
                "cost": 0.00042
            }
        }))
        .unwrap();
        assert_eq!(completion.content, "visible");
        assert_eq!(completion.reasoning.as_deref(), Some("private analysis"));
        assert_eq!(completion.usage.unwrap().cost, Some(0.00042));

        let events = parse_stream_event(
            r#"{"choices":[{"delta":{"content":"visible","reasoning_content":"analysis"}}]}"#,
        )
        .unwrap();
        assert!(matches!(events[0].event, ChatEventKind::TextDelta { .. }));
        assert!(matches!(
            events[1].event,
            ChatEventKind::ReasoningDelta { .. }
        ));
    }

    #[test]
    fn endpoint_prefix_is_preserved_for_local_runtime_profiles() {
        let endpoint = validate_config(&OpenAiCompatibleConfig::new(
            "http://127.0.0.1:12434/engines/v1",
        ))
        .unwrap();
        assert_eq!(endpoint.path(), "/engines/v1/chat/completions");
    }
}
