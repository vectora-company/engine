//! HTTP profile for the llama.cpp `llama-server` API.

use std::{collections::BTreeMap, fmt, time::Duration};

use futures_util::StreamExt;
use reqwest::{Client, Response, header};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout_at};
use url::Url;

use super::{
    ChatCompletion, ChatError, ChatRequest, ChatStream, MediaExecutionContext,
    OpenAiCompatibleClient, OpenAiCompatibleConfig,
};

const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_READ_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOTAL_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

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

/// Result of a successful `GET /health` probe.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlamaServerHealth {
    pub status: String,
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
            chat,
        })
    }

    pub async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        self.chat.complete(request).await
    }

    pub async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        self.chat.stream(request).await
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

fn classify_status(status: u16) -> ChatError {
    match status {
        401 | 403 => ChatError::Authentication,
        402 => ChatError::Quota,
        429 => ChatError::RateLimited { retry_after: None },
        500..=599 => ChatError::Server { status },
        _ => ChatError::Provider { status },
    }
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
