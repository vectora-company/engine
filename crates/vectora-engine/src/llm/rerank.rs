use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    LlmError,
    embeddings::{LlmUsage, ModelSwitch},
    http::{ProviderHttpClient, ProviderHttpSettings},
};

pub const RERANK_CONTRACT_VERSION: u16 = 1;

const COHERE_URL: &str = "https://api.cohere.com";
const VOYAGE_URL: &str = "https://api.voyageai.com/v1";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";

#[derive(Deserialize)]
#[serde(tag = "protocol", content = "settings", rename_all = "snake_case")]
pub enum RerankClientSettings {
    Cohere(ProviderHttpSettings),
    Voyage(ProviderHttpSettings),
    OpenRouter(ProviderHttpSettings),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RerankRequest {
    pub contract_version: u16,
    pub model: String,
    pub query: String,
    pub documents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_doc: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RerankResult {
    pub index: usize,
    pub relevance_score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RerankResponse {
    pub contract_version: u16,
    /// Provider order is preserved; the first result has the highest rank.
    pub results: Vec<RerankResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_switches: Vec<ModelSwitch>,
}

enum RerankProtocol {
    Cohere(ProviderHttpClient),
    Voyage(ProviderHttpClient),
    OpenRouter(ProviderHttpClient),
}

pub struct RerankClient {
    protocol: RerankProtocol,
}

impl RerankClient {
    pub fn new(settings: RerankClientSettings) -> Result<Self, LlmError> {
        let protocol = match settings {
            RerankClientSettings::Cohere(settings) => {
                RerankProtocol::Cohere(ProviderHttpClient::new(settings, COHERE_URL, true)?)
            }
            RerankClientSettings::Voyage(settings) => {
                RerankProtocol::Voyage(ProviderHttpClient::new(settings, VOYAGE_URL, true)?)
            }
            RerankClientSettings::OpenRouter(settings) => {
                RerankProtocol::OpenRouter(ProviderHttpClient::new(settings, OPENROUTER_URL, true)?)
            }
        };
        Ok(Self { protocol })
    }

    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, LlmError> {
        validate_request(&request)?;
        if request.documents.is_empty() {
            return Ok(RerankResponse {
                contract_version: RERANK_CONTRACT_VERSION,
                results: Vec::new(),
                usage: None,
                selected_model_id: None,
                model_switches: Vec::new(),
            });
        }

        match &self.protocol {
            RerankProtocol::Cohere(_) if request.truncation.is_some() => {
                return Err(LlmError::InvalidConfiguration(
                    "Cohere rerank uses max_tokens_per_doc instead of truncation",
                ));
            }
            RerankProtocol::Voyage(_) if request.max_tokens_per_doc.is_some() => {
                return Err(LlmError::InvalidConfiguration(
                    "Voyage rerank does not support max_tokens_per_doc",
                ));
            }
            RerankProtocol::OpenRouter(_)
                if request.max_tokens_per_doc.is_some() || request.truncation.is_some() =>
            {
                return Err(LlmError::InvalidConfiguration(
                    "OpenRouter rerank does not support truncation options",
                ));
            }
            _ => {}
        }

        let (client, path, payload, field) = match &self.protocol {
            RerankProtocol::Cohere(client) => {
                let mut payload = json!({
                    "model": request.model,
                    "query": request.query,
                    "documents": request.documents,
                });
                if let Some(top_n) = request.top_n {
                    payload["top_n"] = json!(top_n);
                }
                if let Some(max_tokens) = request.max_tokens_per_doc {
                    payload["max_tokens_per_doc"] = json!(max_tokens);
                }
                (client, "/v2/rerank", payload, "results")
            }
            RerankProtocol::Voyage(client) => {
                let mut payload = json!({
                    "model": request.model,
                    "query": request.query,
                    "documents": request.documents,
                });
                if let Some(top_n) = request.top_n {
                    payload["top_k"] = json!(top_n);
                }
                if let Some(truncation) = request.truncation {
                    payload["truncation"] = json!(truncation);
                }
                (client, "/rerank", payload, "data")
            }
            RerankProtocol::OpenRouter(client) => {
                let mut payload = json!({
                    "model": request.model,
                    "query": request.query,
                    "documents": request.documents,
                });
                if let Some(top_n) = request.top_n {
                    payload["top_n"] = json!(top_n);
                }
                (client, "/rerank", payload, "results")
            }
        };
        let response = client.post_json(path, payload).await?;
        let results = parse_results(&response, field, request.documents.len())?;
        if request
            .top_n
            .is_some_and(|top_n| results.len() > top_n as usize)
        {
            return Err(LlmError::InvalidResponse);
        }
        let usage = parse_rerank_usage(&response);
        Ok(RerankResponse {
            contract_version: RERANK_CONTRACT_VERSION,
            results,
            usage,
            selected_model_id: None,
            model_switches: Vec::new(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RerankCandidateSettings {
    pub model_id: String,
    pub client: RerankClientSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackRerankSettings {
    pub candidates: Vec<RerankCandidateSettings>,
}

struct Candidate {
    model_id: String,
    model: String,
    client: RerankClient,
}

/// Ordered fallback for provider quota/rate-limit errors only.
pub struct FallbackRerankClient {
    candidates: Vec<Candidate>,
}

impl FallbackRerankClient {
    pub fn new(settings: FallbackRerankSettings) -> Result<Self, LlmError> {
        let mut candidates = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut primary_provider: Option<String> = None;
        for candidate in settings.candidates {
            let model_id = candidate.model_id.trim().to_owned();
            let (provider, model) = parse_model_id(&model_id)?;
            let model = model.to_owned();
            let provider = provider.replace('-', "_").to_ascii_lowercase();
            if !seen_ids.insert(model_id.clone())
                || primary_provider
                    .as_ref()
                    .is_some_and(|first| first == &provider)
            {
                continue;
            }
            let client = RerankClient::new(candidate.client)?;
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
                "at least one eligible rerank candidate is required",
            ));
        }
        Ok(Self { candidates })
    }

    pub async fn rerank(&self, mut request: RerankRequest) -> Result<RerankResponse, LlmError> {
        let mut switches = Vec::new();
        for (index, candidate) in self.candidates.iter().enumerate() {
            request.model.clone_from(&candidate.model);
            match candidate.client.rerank(request.clone()).await {
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

fn validate_request(request: &RerankRequest) -> Result<(), LlmError> {
    if request.contract_version != RERANK_CONTRACT_VERSION {
        return Err(LlmError::InvalidConfiguration(
            "unsupported rerank contract version",
        ));
    }
    if request.model.trim().is_empty() || request.query.trim().is_empty() {
        return Err(LlmError::InvalidConfiguration(
            "model and query must not be empty",
        ));
    }
    if request.top_n == Some(0) || request.max_tokens_per_doc == Some(0) {
        return Err(LlmError::InvalidConfiguration(
            "top_n and max_tokens_per_doc must be positive",
        ));
    }
    if request
        .top_n
        .is_some_and(|count| count as usize > request.documents.len())
    {
        return Err(LlmError::InvalidConfiguration(
            "top_n cannot exceed the number of documents",
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

fn is_quota_error(error: &LlmError) -> bool {
    matches!(error, LlmError::Quota | LlmError::RateLimited { .. })
}

fn parse_results(
    response: &Value,
    field: &str,
    document_count: usize,
) -> Result<Vec<RerankResult>, LlmError> {
    let items = response
        .get(field)
        .and_then(Value::as_array)
        .ok_or(LlmError::InvalidResponse)?;
    if items.len() > document_count {
        return Err(LlmError::InvalidResponse);
    }
    let mut seen = HashSet::new();
    items
        .iter()
        .map(|item| {
            let index = item
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index < document_count)
                .ok_or(LlmError::InvalidResponse)?;
            if !seen.insert(index) {
                return Err(LlmError::InvalidResponse);
            }
            let score = item
                .get("relevance_score")
                .or_else(|| item.get("score"))
                .and_then(Value::as_f64)
                .filter(|score| score.is_finite())
                .ok_or(LlmError::InvalidResponse)?;
            Ok(RerankResult {
                index,
                relevance_score: score,
            })
        })
        .collect()
}

fn parse_rerank_usage(response: &Value) -> Option<LlmUsage> {
    let usage = response.get("usage");
    let tokens = usage
        .and_then(|value| value.get("input_tokens"))
        .or_else(|| usage.and_then(|value| value.get("prompt_tokens")))
        .or_else(|| usage.and_then(|value| value.get("total_tokens")))
        .and_then(Value::as_u64)?;
    Some(LlmUsage {
        input_tokens: tokens,
        cost: usage
            .and_then(|value| value.get("cost"))
            .and_then(Value::as_f64),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{RERANK_CONTRACT_VERSION, RerankRequest, parse_results, validate_request};

    #[test]
    fn preserves_provider_ranking_order_and_accepts_score_alias() {
        let response = json!({"results": [
            {"index": 2, "relevance_score": 0.98},
            {"index": 0, "score": 0.41}
        ]});
        let results = parse_results(&response, "results", 3).unwrap();
        assert_eq!(results[0].index, 2);
        assert_eq!(results[1].index, 0);
        assert_eq!(results[1].relevance_score, 0.41);
    }

    #[test]
    fn rejects_duplicate_or_out_of_range_indices() {
        assert!(
            parse_results(
                &json!({"results": [{"index": 0, "score": 1.0}, {"index": 0, "score": 0.5}]}),
                "results",
                2
            )
            .is_err()
        );
        assert!(
            parse_results(
                &json!({"results": [{"index": 2, "score": 1.0}]}),
                "results",
                2
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_limits() {
        let request = RerankRequest {
            contract_version: RERANK_CONTRACT_VERSION,
            model: "model".into(),
            query: "query".into(),
            documents: vec!["doc".into()],
            top_n: Some(2),
            max_tokens_per_doc: None,
            truncation: None,
        };
        assert!(validate_request(&request).is_err());
    }
}
