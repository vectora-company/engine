use std::{collections::HashSet, sync::Arc};

use async_stream::stream;
use futures_util::StreamExt;
use serde::Deserialize;

use super::{
    ChatCompletion, ChatError, ChatEvent, ChatEventKind, ChatRequest, NativeChatClient,
    NativeChatSettings, OpenAiCompatibleClient, OpenAiCompatibleSettings,
    openai_compatible::ChatStream,
};

/// Explicit ordered candidates. Consumers resolve settings and credentials;
/// the engine never reads product configuration or discovers API keys.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackChatSettings {
    pub candidates: Vec<FallbackCandidateSettings>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackCandidateSettings {
    pub model_id: String,
    pub client: FallbackClientSettings,
}

#[derive(Deserialize)]
#[serde(tag = "protocol", content = "settings", rename_all = "snake_case")]
pub enum FallbackClientSettings {
    OpenAiCompatible(OpenAiCompatibleSettings),
    Native(NativeChatSettings),
}

#[derive(Clone)]
struct Candidate {
    model_id: String,
    model: String,
    client: Arc<ProviderClient>,
}

enum ProviderClient {
    OpenAiCompatible(OpenAiCompatibleClient),
    Native(NativeChatClient),
}

impl ProviderClient {
    async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        match self {
            Self::OpenAiCompatible(client) => client.complete(request).await,
            Self::Native(client) => client.complete(request).await,
        }
    }

    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        match self {
            Self::OpenAiCompatible(client) => client.stream(request).await,
            Self::Native(client) => client.stream(request).await,
        }
    }
}

/// Fallback runner over caller-supplied, already eligible provider candidates.
pub struct FallbackChatClient {
    candidates: Vec<Candidate>,
}

impl FallbackChatClient {
    pub fn new(settings: FallbackChatSettings) -> Result<Self, ChatError> {
        let mut candidates = Vec::with_capacity(settings.candidates.len());
        let mut seen_ids = HashSet::new();
        let mut primary_provider: Option<String> = None;
        for input in settings.candidates {
            let model_id = input.model_id.trim().to_owned();
            let (provider, model) =
                model_id
                    .split_once(':')
                    .ok_or(ChatError::InvalidConfiguration(
                        "fallback model IDs must use provider:model form",
                    ))?;
            if provider.trim().is_empty() || model.trim().is_empty() {
                return Err(ChatError::InvalidConfiguration(
                    "fallback model IDs must include provider and model",
                ));
            }
            let provider = normalize_provider(provider);
            let model = model.to_owned();
            if !seen_ids.insert(model_id.clone()) {
                continue;
            }
            if primary_provider
                .as_ref()
                .is_some_and(|primary| primary == &provider)
            {
                continue;
            }
            let client = match input.client {
                FallbackClientSettings::OpenAiCompatible(settings) => {
                    ProviderClient::OpenAiCompatible(OpenAiCompatibleClient::new(
                        settings.into_config(),
                    )?)
                }
                FallbackClientSettings::Native(settings) => {
                    ProviderClient::Native(NativeChatClient::new(settings)?)
                }
            };
            if primary_provider.is_none() {
                primary_provider = Some(provider);
            }
            candidates.push(Candidate {
                model_id,
                model,
                client: Arc::new(client),
            });
        }
        if candidates.is_empty() {
            return Err(ChatError::InvalidConfiguration(
                "at least one eligible fallback candidate is required",
            ));
        }
        Ok(Self { candidates })
    }

    pub async fn complete(&self, mut request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        let mut last_error = None;
        for (index, candidate) in self.candidates.iter().enumerate() {
            request.model.clone_from(&candidate.model);
            match candidate.client.complete(request.clone()).await {
                Ok(mut completion) => {
                    completion.selected_model_id = Some(candidate.model_id.clone());
                    return Ok(completion);
                }
                Err(error) if can_fallback(&error, index) => last_error = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(exhausted_error(
            self.candidates
                .last()
                .expect("validated non-empty candidates"),
            last_error.expect("the last candidate failed"),
        ))
    }

    pub async fn stream(&self, request: ChatRequest) -> Result<ChatStream, ChatError> {
        let candidates = self.candidates.clone();
        let stream = stream! {
            let mut final_error = None;
            for (index, candidate) in candidates.iter().enumerate() {
                if index > 0 {
                    let previous = &candidates[index - 1];
                    yield Ok(ChatEvent::new(ChatEventKind::ModelSwitched {
                        from_model_id: previous.model_id.clone(),
                        to_model_id: candidate.model_id.clone(),
                    }));
                }
                let mut candidate_request = request.clone();
                candidate_request.model.clone_from(&candidate.model);
                let candidate_error = match candidate.client.stream(candidate_request).await {
                    Ok(mut response_stream) => {
                        let mut emitted = false;
                        let mut failure = None;
                        while let Some(item) = response_stream.next().await {
                            match item {
                                Ok(event) => {
                                    emitted = true;
                                    yield Ok(event);
                                }
                                Err(error) => {
                                    if emitted || !can_fallback(&error, index) {
                                        yield Err(error);
                                        return;
                                    }
                                    failure = Some(error);
                                    break;
                                }
                            }
                        }
                        failure
                    }
                    Err(error) => Some(error),
                };
                if let Some(error) = candidate_error {
                    if !can_fallback(&error, index) {
                        yield Err(error);
                        return;
                    }
                    if index + 1 == candidates.len() {
                        yield Err(exhausted_error(candidate, error));
                        return;
                    }
                    final_error = Some(error);
                } else {
                    return;
                }
            }
            if let (Some(candidate), Some(error)) = (candidates.last(), final_error) {
                yield Err(exhausted_error(candidate, error));
            }
        };
        Ok(Box::pin(stream))
    }
}

fn can_fallback(error: &ChatError, candidate_index: usize) -> bool {
    if matches!(error, ChatError::Cancelled) {
        return false;
    }
    candidate_index > 0
        || matches!(
            error,
            ChatError::Quota
                | ChatError::RateLimited { .. }
                | ChatError::Connection
                | ChatError::Timeout
                | ChatError::Overloaded
                | ChatError::ProviderIncompatible
                | ChatError::MultimodalIncompatible
                | ChatError::Server { status: 503 | 529 }
        )
}

fn normalize_provider(provider: &str) -> String {
    match provider.replace('-', "_").to_ascii_lowercase().as_str() {
        "google_genai" => "google".to_owned(),
        normalized => normalized.to_owned(),
    }
}

fn exhausted_error(candidate: &Candidate, last_error: ChatError) -> ChatError {
    if matches!(last_error, ChatError::MultimodalIncompatible) {
        last_error
    } else {
        ChatError::FallbackExhausted {
            model_id: candidate.model_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_provider;

    #[test]
    fn provider_aliases_are_normalized_before_same_provider_filtering() {
        assert_eq!(normalize_provider("google-genai"), "google");
        assert_eq!(normalize_provider("Google"), "google");
        assert_eq!(normalize_provider("nine-router"), "nine_router");
    }
}
