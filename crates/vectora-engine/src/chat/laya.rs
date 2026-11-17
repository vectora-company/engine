//! HTTP client for the Laya `laya-serve` decision API.

use std::{collections::BTreeMap, fmt, time::Duration};

use reqwest::{Client, Response, header};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::time::{Instant, timeout_at};
use url::Url;

use super::error::classify_status;
use super::{ChatError, DecisionRequest, DecisionResponse, DecisionUsage};

const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_READ_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOTAL_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// Settings for the standalone Laya HTTP server (`laya-serve`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayaSettings {
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
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: usize,
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: usize,
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
const fn default_max_request_bytes() -> usize {
    DEFAULT_MAX_REQUEST_BYTES
}

impl fmt::Debug for LayaSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayaSettings")
            .field("base_url", &"[configured]")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("read_timeout_ms", &self.read_timeout_ms)
            .field("total_timeout_ms", &self.total_timeout_ms)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("max_request_bytes", &self.max_request_bytes)
            .finish()
    }
}

/// Client for Laya's `/v1/systemone` API. Laya accepts optional checkpoint
/// selection and does not expose OpenAI `/v1/models`.
pub struct LayaClient {
    root_url: Url,
    client: Client,
    api_key: Option<header::HeaderValue>,
    headers: Vec<(header::HeaderName, header::HeaderValue)>,
    read_timeout: Duration,
    total_timeout: Duration,
    max_response_bytes: usize,
    max_request_bytes: usize,
}

impl fmt::Debug for LayaClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayaClient")
            .field("root_url", &self.root_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field(
                "headers",
                &format!("{} redacted header(s)", self.headers.len()),
            )
            .finish_non_exhaustive()
    }
}

impl LayaClient {
    pub fn new(settings: LayaSettings) -> Result<Self, ChatError> {
        if settings.connect_timeout_ms == 0
            || settings.read_timeout_ms == 0
            || settings.total_timeout_ms == 0
            || settings.max_response_bytes == 0
            || settings.max_request_bytes == 0
        {
            return Err(ChatError::InvalidConfiguration(
                "timeouts and body limits must be positive",
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
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(settings.connect_timeout_ms))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ChatError::InvalidConfiguration("HTTP client could not be created"))?;
        Ok(Self {
            root_url,
            client,
            api_key,
            headers,
            read_timeout: Duration::from_millis(settings.read_timeout_ms),
            total_timeout: Duration::from_millis(settings.total_timeout_ms),
            max_response_bytes: settings.max_response_bytes,
            max_request_bytes: settings.max_request_bytes,
        })
    }

    pub async fn health(&self) -> Result<LayaHealth, ChatError> {
        let endpoint = self.endpoint("/health");
        let response = self.get(endpoint).await?;
        let health: LayaHealth =
            serde_json::from_slice(&response).map_err(|_| ChatError::InvalidResponse)?;
        if health.status != "ok" {
            return Err(ChatError::InvalidResponse);
        }
        Ok(health)
    }

    pub async fn decide(&self, request: DecisionRequest) -> Result<DecisionResponse, ChatError> {
        request.validate().map_err(ChatError::InvalidRequest)?;
        validate_laya_request(&request).map_err(ChatError::InvalidRequest)?;
        let mut payload = serde_json::json!({
            "state": request.state,
            "questions": request.questions,
        });
        if let Some(model) = request.model {
            payload["model"] = serde_json::json!(model);
        }
        let body = serde_json::to_vec(&payload)
            .map_err(|_| ChatError::InvalidRequest("decision request could not be encoded"))?;
        if body.len() > self.max_request_bytes {
            return Err(ChatError::InvalidRequest(
                "decision request exceeds the configured body limit",
            ));
        }
        let endpoint = self.endpoint("/v1/systemone");
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
            return Err(classify_status(
                response.status().as_u16(),
                retry_after(&response),
            ));
        }
        let bytes = self.read_response(response, deadline).await?;
        let wire: LayaWireResponse =
            serde_json::from_slice(&bytes).map_err(|_| ChatError::InvalidResponse)?;
        if wire.answers.len() != request.questions.len()
            || request.questions.iter().any(|(id, question)| {
                wire.answers
                    .get(id)
                    .is_none_or(|answer| !laya_answer_matches(question, answer))
            })
        {
            return Err(ChatError::InvalidResponse);
        }
        Ok(DecisionResponse {
            answers: wire.answers,
            usage: wire.usage.map(|usage| DecisionUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }),
        })
    }

    fn endpoint(&self, path: &str) -> Url {
        let mut endpoint = self.root_url.clone();
        endpoint.set_path(&format!(
            "{}{}",
            endpoint.path().trim_end_matches('/'),
            path
        ));
        endpoint
    }

    async fn get(&self, endpoint: Url) -> Result<Vec<u8>, ChatError> {
        let deadline = Instant::now() + self.total_timeout;
        let mut http = self.client.get(endpoint).timeout(self.read_timeout);
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
            return Err(classify_status(
                response.status().as_u16(),
                retry_after(&response),
            ));
        }
        self.read_response(response, deadline).await
    }

    async fn read_response(
        &self,
        response: Response,
        deadline: Instant,
    ) -> Result<Vec<u8>, ChatError> {
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(ChatError::ResponseTooLarge);
        }
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        loop {
            let read_deadline = (Instant::now() + self.read_timeout).min(deadline);
            let item = timeout_at(read_deadline, futures_util::StreamExt::next(&mut stream))
                .await
                .map_err(|_| ChatError::Timeout)?;
            match item {
                Some(Ok(chunk)) => {
                    if body.len().saturating_add(chunk.len()) > self.max_response_bytes {
                        return Err(ChatError::ResponseTooLarge);
                    }
                    body.extend_from_slice(&chunk);
                }
                Some(Err(error)) => {
                    return Err(if error.is_timeout() {
                        ChatError::Timeout
                    } else {
                        ChatError::Connection
                    });
                }
                None => return Ok(body),
            }
        }
    }
}

fn validate_laya_request(request: &DecisionRequest) -> Result<(), &'static str> {
    if request.state.is_null() {
        return Err("Laya state must not be null");
    }
    if serde_json::to_string(&request.state)
        .map_err(|_| "Laya state could not be encoded")?
        .chars()
        .count()
        > 50_000
    {
        return Err("Laya state exceeds 50,000 characters");
    }
    if request.questions.len() > 64 {
        return Err("Laya accepts at most 64 questions per request");
    }
    let mut option_count = 0;
    for question in request.questions.values() {
        if question.instructions.as_ref().is_none_or(Value::is_null) {
            return Err("Laya questions require instructions");
        }
        match question.kind {
            super::DecisionQuestionKind::Choice => {
                option_count += question
                    .criteria
                    .as_ref()
                    .and_then(Value::as_object)
                    .map_or(0, Map::len);
            }
            super::DecisionQuestionKind::Score => {
                if question
                    .criteria
                    .as_ref()
                    .and_then(Value::as_array)
                    .is_some_and(|levels| levels.len() > 32)
                {
                    return Err("Laya score questions accept at most 32 levels");
                }
                option_count += question
                    .criteria
                    .as_ref()
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
            }
            super::DecisionQuestionKind::Noul => option_count += 2,
        }
    }
    if option_count > 512 {
        return Err("Laya accepts at most 512 options across all questions");
    }
    Ok(())
}

fn retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LayaHealth {
    pub status: String,
}

#[derive(Deserialize)]
struct LayaWireResponse {
    answers: BTreeMap<String, Value>,
    #[serde(default)]
    usage: Option<LayaWireUsage>,
    #[serde(flatten)]
    _extra: Map<String, Value>,
}

#[derive(Deserialize)]
struct LayaWireUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(flatten)]
    _extra: Map<String, Value>,
}

fn probability(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
}

fn rounded_distribution(values: impl Iterator<Item = f64>, count: usize) -> bool {
    let values: Vec<_> = values.collect();
    let tolerance = (count as f64 * 0.0000501).max(0.0001);
    !values.is_empty() && (values.iter().sum::<f64>() - 1.0).abs() <= tolerance
}

fn laya_answer_matches(question: &super::DecisionQuestion, answer: &Value) -> bool {
    let Some(answer) = answer.as_object() else {
        return false;
    };
    match question.kind {
        super::DecisionQuestionKind::Choice => {
            let Some(criteria) = question.criteria.as_ref().and_then(Value::as_object) else {
                return false;
            };
            let Some(probabilities) = answer.get("probabilities").and_then(Value::as_object) else {
                return false;
            };
            answer
                .get("choice")
                .and_then(Value::as_str)
                .is_some_and(|choice| criteria.contains_key(choice))
                && probability(answer.get("confidence")).is_some()
                && probabilities.len() == criteria.len()
                && probabilities.keys().all(|key| criteria.contains_key(key))
                && rounded_distribution(
                    probabilities
                        .values()
                        .filter_map(|value| probability(Some(value))),
                    criteria.len(),
                )
                && probabilities
                    .values()
                    .all(|value| probability(Some(value)).is_some())
        }
        super::DecisionQuestionKind::Score => {
            let Some(levels) = question.criteria.as_ref().and_then(Value::as_array) else {
                return false;
            };
            let Some(probabilities) = answer.get("probabilities").and_then(Value::as_object) else {
                return false;
            };
            let Some(legend) = answer.get("legend").and_then(Value::as_object) else {
                return false;
            };
            let Some(score) = answer.get("score").and_then(Value::as_f64) else {
                return false;
            };
            let expected: BTreeMap<String, String> = levels
                .iter()
                .enumerate()
                .filter_map(|(index, level)| {
                    level
                        .as_str()
                        .map(|text| (index.to_string(), text.to_owned()))
                })
                .collect();
            !levels.is_empty()
                && expected.len() == levels.len()
                && score.is_finite()
                && (0.0..=(levels.len() - 1) as f64).contains(&score)
                && probability(answer.get("confidence")).is_some()
                && probabilities.len() == levels.len()
                && legend.len() == levels.len()
                && probabilities.keys().all(|key| {
                    expected.contains_key(key) && probability(probabilities.get(key)).is_some()
                })
                && expected
                    .iter()
                    .all(|(key, value)| legend.get(key).and_then(Value::as_str) == Some(value))
                && rounded_distribution(
                    probabilities
                        .values()
                        .filter_map(|value| probability(Some(value))),
                    levels.len(),
                )
        }
        super::DecisionQuestionKind::Noul => probability(answer.get("noul")).is_some(),
    }
}

#[cfg(test)]
mod tests;
