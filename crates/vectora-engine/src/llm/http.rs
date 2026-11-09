use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use reqwest::{Client, Response, header};
use serde::Deserialize;
use serde_json::Value;
use tokio::time::{Instant, timeout_at};
use url::Url;

use super::error::{LlmError, classify_http_error};

const MAX_ERROR_BODY_BYTES: usize = 8 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Common HTTP settings used by the non-streaming model capabilities.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderHttpSettings {
    #[serde(default)]
    pub base_url: Option<String>,
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
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: usize,
}

impl Default for ProviderHttpSettings {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            headers: BTreeMap::new(),
            connect_timeout_ms: default_connect_timeout_ms(),
            read_timeout_ms: default_read_timeout_ms(),
            total_timeout_ms: default_total_timeout_ms(),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

const fn default_connect_timeout_ms() -> u64 {
    10_000
}
const fn default_read_timeout_ms() -> u64 {
    120_000
}
const fn default_total_timeout_ms() -> u64 {
    300_000
}
const fn default_max_response_bytes() -> usize {
    DEFAULT_MAX_RESPONSE_BYTES
}

pub(crate) struct ProviderHttpClient {
    base_url: Url,
    api_key: Option<header::HeaderValue>,
    headers: Vec<(header::HeaderName, header::HeaderValue)>,
    client: Client,
    read_timeout: Duration,
    total_timeout: Duration,
    max_response_bytes: usize,
}

impl ProviderHttpClient {
    pub(crate) fn new(
        settings: ProviderHttpSettings,
        default_base_url: &str,
        requires_api_key: bool,
    ) -> Result<Self, LlmError> {
        let connect_timeout = Duration::from_millis(settings.connect_timeout_ms);
        let read_timeout = Duration::from_millis(settings.read_timeout_ms);
        let total_timeout = Duration::from_millis(settings.total_timeout_ms);
        if connect_timeout.is_zero() || read_timeout.is_zero() || total_timeout.is_zero() {
            return Err(LlmError::InvalidConfiguration("timeouts must be positive"));
        }
        if settings.max_response_bytes == 0 {
            return Err(LlmError::InvalidConfiguration(
                "max_response_bytes must be positive",
            ));
        }

        let url = settings.base_url.as_deref().unwrap_or(default_base_url);
        let base_url = Url::parse(url.trim())
            .map_err(|_| LlmError::InvalidConfiguration("base URL must be an absolute HTTP URL"))?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(LlmError::InvalidConfiguration(
                "base URL is not a safe HTTP endpoint",
            ));
        }

        let api_key = match settings.api_key {
            Some(key) if key.trim().is_empty() => {
                return Err(LlmError::InvalidConfiguration("API key must not be empty"));
            }
            Some(key) => {
                let value = format!("Bearer {}", key.trim());
                Some(header::HeaderValue::from_str(&value).map_err(|_| {
                    LlmError::InvalidConfiguration("API key is not a valid HTTP header value")
                })?)
            }
            None if requires_api_key => return Err(LlmError::Authentication),
            None => None,
        };

        let mut headers = Vec::with_capacity(settings.headers.len());
        for (name, value) in settings.headers {
            let name = header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| LlmError::InvalidConfiguration("invalid HTTP header name"))?;
            let value = header::HeaderValue::from_str(&value)
                .map_err(|_| LlmError::InvalidConfiguration("invalid HTTP header value"))?;
            if matches!(name.as_str(), "authorization" | "content-type" | "host") {
                return Err(LlmError::InvalidConfiguration(
                    "reserved HTTP header cannot be overridden",
                ));
            }
            headers.push((name, value));
        }

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|_| LlmError::InvalidConfiguration("HTTP client could not be created"))?;
        Ok(Self {
            base_url,
            api_key,
            headers,
            client,
            read_timeout,
            total_timeout,
            max_response_bytes: settings.max_response_bytes,
        })
    }

    pub(crate) async fn post_json(&self, path: &str, payload: Value) -> Result<Value, LlmError> {
        let endpoint = self.endpoint(path)?;
        let deadline = Instant::now() + self.total_timeout;
        let mut request = self.client.post(endpoint).json(&payload);
        if let Some(api_key) = &self.api_key {
            request = request.header(header::AUTHORIZATION, api_key.clone());
        }
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let response = timeout_at(deadline, request.send())
            .await
            .map_err(|_| LlmError::Timeout)?
            .map_err(|error| {
                if error.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::Connection
                }
            })?;
        if !response.status().is_success() {
            return Err(self.http_error(response, deadline).await);
        }
        let bytes = self
            .read_body(response, deadline, self.max_response_bytes)
            .await?;
        serde_json::from_slice(&bytes).map_err(|_| LlmError::InvalidResponse)
    }

    fn endpoint(&self, path: &str) -> Result<Url, LlmError> {
        let mut endpoint = self.base_url.clone();
        let prefix = endpoint.path().trim_end_matches('/');
        let suffix = path.trim_start_matches('/');
        endpoint.set_path(&format!("{prefix}/{suffix}"));
        Ok(endpoint)
    }

    async fn http_error(&self, response: Response, deadline: Instant) -> LlmError {
        let status = response.status();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        let body = self
            .read_body(response, deadline, MAX_ERROR_BODY_BYTES)
            .await
            .unwrap_or_default();
        classify_http_error(status.as_u16(), retry_after, &body)
    }

    async fn read_body(
        &self,
        response: Response,
        deadline: Instant,
        max_bytes: usize,
    ) -> Result<Vec<u8>, LlmError> {
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(LlmError::ResponseTooLarge);
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        loop {
            let read_deadline = deadline.min(Instant::now() + self.read_timeout);
            let next = timeout_at(read_deadline, stream.next())
                .await
                .map_err(|_| LlmError::Timeout)?;
            let Some(chunk) = next else { break };
            let chunk = chunk.map_err(|error| {
                if error.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::Connection
                }
            })?;
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(LlmError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
