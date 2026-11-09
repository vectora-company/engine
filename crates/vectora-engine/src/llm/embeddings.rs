use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    LlmError,
    http::{ProviderHttpClient, ProviderHttpSettings},
};

pub const EMBEDDING_CONTRACT_VERSION: u16 = 1;
const COHERE_MAX_BATCH: usize = 96;

const OPENAI_COMPATIBLE_URL: &str = "https://api.openai.com/v1";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
const COHERE_URL: &str = "https://api.cohere.com";
const VOYAGE_URL: &str = "https://api.voyageai.com/v1";
const OLLAMA_URL: &str = "http://127.0.0.1:11434";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInputType {
    Document,
    Query,
}

/// Protocol and credentials for one embedding provider endpoint.
#[derive(Deserialize)]
#[serde(tag = "protocol", content = "settings", rename_all = "snake_case")]
pub enum EmbeddingClientSettings {
    OpenAiCompatible(ProviderHttpSettings),
    OpenRouter(ProviderHttpSettings),
    Cohere(ProviderHttpSettings),
    Voyage(ProviderHttpSettings),
    Ollama(ProviderHttpSettings),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingRequest {
    pub contract_version: u16,
    pub model: String,
    pub input_type: EmbeddingInputType,
    pub texts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelSwitch {
    pub from_model_id: String,
    pub to_model_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct LlmUsage {
    pub input_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct EmbeddingResponse {
    pub contract_version: u16,
    pub embeddings: Vec<Vec<f32>>,
    pub dimensions: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_switches: Vec<ModelSwitch>,
}

enum EmbeddingProtocol {
    OpenAiCompatible(ProviderHttpClient),
    OpenRouter(ProviderHttpClient),
    Cohere(ProviderHttpClient),
    Voyage(ProviderHttpClient),
    Ollama(ProviderHttpClient),
}

/// Embedding APIs for OpenAI-compatible, OpenRouter, Cohere, Voyage and Ollama.
pub struct EmbeddingClient {
    protocol: EmbeddingProtocol,
}

impl EmbeddingClient {
    pub fn new(settings: EmbeddingClientSettings) -> Result<Self, LlmError> {
        let protocol = match settings {
            EmbeddingClientSettings::OpenAiCompatible(settings) => {
                EmbeddingProtocol::OpenAiCompatible(ProviderHttpClient::new(
                    settings,
                    OPENAI_COMPATIBLE_URL,
                    false,
                )?)
            }
            EmbeddingClientSettings::OpenRouter(settings) => EmbeddingProtocol::OpenRouter(
                ProviderHttpClient::new(settings, OPENROUTER_URL, true)?,
            ),
            EmbeddingClientSettings::Cohere(settings) => {
                EmbeddingProtocol::Cohere(ProviderHttpClient::new(settings, COHERE_URL, true)?)
            }
            EmbeddingClientSettings::Voyage(settings) => {
                EmbeddingProtocol::Voyage(ProviderHttpClient::new(settings, VOYAGE_URL, true)?)
            }
            EmbeddingClientSettings::Ollama(settings) => {
                EmbeddingProtocol::Ollama(ProviderHttpClient::new(settings, OLLAMA_URL, false)?)
            }
        };
        Ok(Self { protocol })
    }

    pub async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, LlmError> {
        validate_request(&request)?;
        if request.texts.is_empty() {
            return Ok(EmbeddingResponse {
                contract_version: EMBEDDING_CONTRACT_VERSION,
                embeddings: Vec::new(),
                dimensions: request.dimensions.unwrap_or_default(),
                usage: None,
                selected_model_id: None,
                model_switches: Vec::new(),
            });
        }

        match &self.protocol {
            EmbeddingProtocol::OpenAiCompatible(client) => {
                embed_openai_compatible(client, &request, false).await
            }
            EmbeddingProtocol::OpenRouter(client) => {
                embed_openai_compatible(client, &request, true).await
            }
            EmbeddingProtocol::Cohere(client) => embed_cohere(client, &request).await,
            EmbeddingProtocol::Voyage(client) => embed_voyage(client, &request).await,
            EmbeddingProtocol::Ollama(client) => embed_ollama(client, &request).await,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingCandidateSettings {
    pub model_id: String,
    pub client: EmbeddingClientSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackEmbeddingSettings {
    pub candidates: Vec<EmbeddingCandidateSettings>,
}

struct Candidate {
    model_id: String,
    model: String,
    client: EmbeddingClient,
}

/// Ordered quota fallback over candidates selected by the consumer.
pub struct FallbackEmbeddingClient {
    candidates: Vec<Candidate>,
}

impl FallbackEmbeddingClient {
    pub fn new(settings: FallbackEmbeddingSettings) -> Result<Self, LlmError> {
        let mut candidates = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut primary_provider: Option<String> = None;
        for candidate in settings.candidates {
            let model_id = candidate.model_id.trim().to_owned();
            let (provider, model) = parse_model_id(&model_id)?;
            let model = model.to_owned();
            let provider = normalize_provider(provider);
            if !seen_ids.insert(model_id.clone())
                || primary_provider
                    .as_ref()
                    .is_some_and(|primary| primary == &provider)
            {
                continue;
            }
            let client = EmbeddingClient::new(candidate.client)?;
            if primary_provider.is_none() {
                primary_provider = Some(provider.clone());
            }
            candidates.push(Candidate {
                model_id,
                model,
                client,
            });
        }
        if candidates.is_empty() {
            return Err(LlmError::InvalidConfiguration(
                "at least one eligible embedding candidate is required",
            ));
        }
        Ok(Self { candidates })
    }

    pub async fn embed(
        &self,
        mut request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, LlmError> {
        let mut switches = Vec::new();
        for (index, candidate) in self.candidates.iter().enumerate() {
            request.model.clone_from(&candidate.model);
            match candidate.client.embed(request.clone()).await {
                Ok(mut response) => {
                    response.selected_model_id = Some(candidate.model_id.clone());
                    response.model_switches = switches;
                    return Ok(response);
                }
                Err(error) if is_quota_error(&error) && index + 1 < self.candidates.len() => {
                    let next = &self.candidates[index + 1];
                    switches.push(ModelSwitch {
                        from_model_id: candidate.model_id.clone(),
                        to_model_id: next.model_id.clone(),
                    });
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("a non-empty candidate list returns or fails in its loop")
    }
}

fn validate_request(request: &EmbeddingRequest) -> Result<(), LlmError> {
    if request.contract_version != EMBEDDING_CONTRACT_VERSION {
        return Err(LlmError::InvalidConfiguration(
            "unsupported embedding contract version",
        ));
    }
    if request.model.trim().is_empty() {
        return Err(LlmError::InvalidConfiguration("model must not be empty"));
    }
    if request.texts.iter().any(|text| text.trim().is_empty()) {
        return Err(LlmError::InvalidConfiguration(
            "embedding texts must not be empty",
        ));
    }
    if request.dimensions == Some(0) {
        return Err(LlmError::InvalidConfiguration(
            "dimensions must be positive",
        ));
    }
    Ok(())
}

fn parse_model_id(model_id: &str) -> Result<(&str, &str), LlmError> {
    let (provider, model) = model_id
        .split_once(':')
        .ok_or(LlmError::InvalidConfiguration(
            "fallback model IDs must use provider:model form",
        ))?;
    if provider.trim().is_empty() || model.trim().is_empty() {
        return Err(LlmError::InvalidConfiguration(
            "fallback model IDs must include provider and model",
        ));
    }
    Ok((provider, model.trim()))
}

fn normalize_provider(provider: &str) -> String {
    provider.replace('-', "_").to_ascii_lowercase()
}

fn is_quota_error(error: &LlmError) -> bool {
    matches!(error, LlmError::Quota | LlmError::RateLimited { .. })
}

async fn embed_openai_compatible(
    client: &ProviderHttpClient,
    request: &EmbeddingRequest,
    openrouter: bool,
) -> Result<EmbeddingResponse, LlmError> {
    let mut payload = json!({ "model": request.model, "input": request.texts });
    if let Some(dimensions) = request.dimensions {
        payload["dimensions"] = json!(dimensions);
    }
    if openrouter {
        payload["input_type"] = json!(input_type_name(request.input_type));
    }
    let response = client.post_json("/embeddings", payload).await?;
    let embeddings = parse_indexed_embeddings(&response, "data", request.texts.len())?;
    build_response(embeddings, request.dimensions, parse_usage(&response))
}

async fn embed_voyage(
    client: &ProviderHttpClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, LlmError> {
    let mut payload = json!({
        "model": request.model,
        "input": request.texts,
        "input_type": input_type_name(request.input_type),
        "output_dtype": "float",
        "truncation": true,
    });
    if let Some(dimensions) = request.dimensions {
        payload["output_dimension"] = json!(dimensions);
    }
    let response = client.post_json("/embeddings", payload).await?;
    let embeddings = parse_indexed_embeddings(&response, "data", request.texts.len())?;
    build_response(embeddings, request.dimensions, parse_usage(&response))
}

async fn embed_cohere(
    client: &ProviderHttpClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, LlmError> {
    let mut embeddings = Vec::with_capacity(request.texts.len());
    let mut usage = LlmUsage::default();
    let mut has_usage = false;
    for batch in request.texts.chunks(COHERE_MAX_BATCH) {
        let mut payload = json!({
            "model": request.model,
            "input_type": match request.input_type {
                EmbeddingInputType::Document => "search_document",
                EmbeddingInputType::Query => "search_query",
            },
            "texts": batch,
            "embedding_types": ["float"],
            "truncate": "END",
        });
        if let Some(dimensions) = request.dimensions {
            payload["output_dimension"] = json!(dimensions);
        }
        let response = client.post_json("/v2/embed", payload).await?;
        let vectors = response
            .pointer("/embeddings/float")
            .and_then(Value::as_array)
            .ok_or(LlmError::InvalidResponse)?;
        if vectors.len() != batch.len() {
            return Err(LlmError::InvalidResponse);
        }
        for vector in vectors {
            embeddings.push(parse_vector(vector)?);
        }
        if let Some(tokens) = response
            .pointer("/meta/billed_units/input_tokens")
            .and_then(Value::as_u64)
        {
            usage.input_tokens = usage.input_tokens.saturating_add(tokens);
            has_usage = true;
        }
    }
    build_response(embeddings, request.dimensions, has_usage.then_some(usage))
}

async fn embed_ollama(
    client: &ProviderHttpClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, LlmError> {
    if request.dimensions.is_some() {
        return Err(LlmError::InvalidConfiguration(
            "Ollama embedding protocol does not support output dimensions",
        ));
    }
    let response = client
        .post_json(
            "/api/embed",
            json!({ "model": request.model, "input": request.texts }),
        )
        .await?;
    let vectors = response
        .get("embeddings")
        .and_then(Value::as_array)
        .ok_or(LlmError::InvalidResponse)?;
    if vectors.len() != request.texts.len() {
        return Err(LlmError::InvalidResponse);
    }
    let embeddings = vectors
        .iter()
        .map(parse_vector)
        .collect::<Result<Vec<_>, _>>()?;
    build_response(embeddings, None, None)
}

fn input_type_name(input_type: EmbeddingInputType) -> &'static str {
    match input_type {
        EmbeddingInputType::Document => "document",
        EmbeddingInputType::Query => "query",
    }
}

fn parse_indexed_embeddings(
    response: &Value,
    field: &str,
    expected_count: usize,
) -> Result<Vec<Vec<f32>>, LlmError> {
    let items = response
        .get(field)
        .and_then(Value::as_array)
        .ok_or(LlmError::InvalidResponse)?;
    if items.len() != expected_count {
        return Err(LlmError::InvalidResponse);
    }
    let mut ordered = vec![None; expected_count];
    for item in items {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < expected_count)
            .ok_or(LlmError::InvalidResponse)?;
        if ordered[index].is_some() {
            return Err(LlmError::InvalidResponse);
        }
        ordered[index] = Some(parse_vector(
            item.get("embedding").ok_or(LlmError::InvalidResponse)?,
        )?);
    }
    ordered
        .into_iter()
        .map(|vector| vector.ok_or(LlmError::InvalidResponse))
        .collect()
}

fn parse_vector(value: &Value) -> Result<Vec<f32>, LlmError> {
    let values = value.as_array().ok_or(LlmError::InvalidResponse)?;
    if values.is_empty() {
        return Err(LlmError::InvalidResponse);
    }
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or(LlmError::InvalidResponse)? as f32;
            if number.is_finite() {
                Ok(number)
            } else {
                Err(LlmError::InvalidResponse)
            }
        })
        .collect()
}

fn build_response(
    embeddings: Vec<Vec<f32>>,
    requested_dimensions: Option<u32>,
    usage: Option<LlmUsage>,
) -> Result<EmbeddingResponse, LlmError> {
    let dimensions = embeddings.first().map_or(0, Vec::len);
    if embeddings.iter().any(|vector| vector.len() != dimensions)
        || requested_dimensions.is_some_and(|requested| requested as usize != dimensions)
    {
        return Err(LlmError::InvalidResponse);
    }
    let dimensions = u32::try_from(dimensions).map_err(|_| LlmError::InvalidResponse)?;
    Ok(EmbeddingResponse {
        contract_version: EMBEDDING_CONTRACT_VERSION,
        embeddings,
        dimensions,
        usage,
        selected_model_id: None,
        model_switches: Vec::new(),
    })
}

fn parse_usage(response: &Value) -> Option<LlmUsage> {
    let usage = response.get("usage")?;
    let input_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| {
            response
                .pointer("/meta/billed_units/input_tokens")
                .and_then(Value::as_u64)
        })?;
    let cost = usage.get("cost").and_then(Value::as_f64);
    Some(LlmUsage { input_tokens, cost })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{build_response, parse_indexed_embeddings};

    #[test]
    fn restores_explicit_provider_indices_and_rejects_bad_vectors() {
        let response = json!({"data": [
            {"index": 1, "embedding": [0.3, 0.4]},
            {"index": 0, "embedding": [0.1, 0.2]}
        ]});
        assert_eq!(
            parse_indexed_embeddings(&response, "data", 2).unwrap(),
            vec![vec![0.1, 0.2], vec![0.3, 0.4]]
        );

        let duplicate = json!({"data": [
            {"index": 0, "embedding": [0.1, 0.2]},
            {"index": 0, "embedding": [0.3, 0.4]}
        ]});
        assert!(parse_indexed_embeddings(&duplicate, "data", 2).is_err());

        let invalid = json!({"data": [
            {"index": 0, "embedding": [0.1]},
            {"index": 1, "embedding": [0.3, 0.4]}
        ]});
        let vectors = parse_indexed_embeddings(&invalid, "data", 2).unwrap();
        assert!(build_response(vectors, Some(2), None).is_err());
    }
}
