use std::{collections::HashSet, sync::Arc};

use async_stream::stream;
use futures_util::StreamExt;
use serde::Deserialize;

use super::{
    ChatCompletion, ChatError, ChatEvent, ChatEventKind, ChatRequest, MediaExecutionContext,
    NativeChatClient, NativeChatSettings, OpenAiCompatibleClient, OpenAiCompatibleSettings,
    PreparedRequest, openai_compatible::ChatStream,
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
    provider: String,
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
    media_context: Option<MediaExecutionContext>,
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
                primary_provider = Some(provider.clone());
            }
            candidates.push(Candidate {
                model_id,
                provider,
                model,
                client: Arc::new(client),
            });
        }
        if candidates.is_empty() {
            return Err(ChatError::InvalidConfiguration(
                "at least one eligible fallback candidate is required",
            ));
        }
        Ok(Self {
            candidates,
            media_context: None,
        })
    }

    /// Configure the default media context used by chat calls.
    pub fn with_media_context(mut self, context: MediaExecutionContext) -> Self {
        self.media_context = Some(context);
        self
    }

    pub async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, ChatError> {
        if let Some(context) = &self.media_context {
            return self.complete_with_media(request, context).await;
        }
        self.complete_candidates(request, None).await
    }

    /// Complete using an ephemeral context that overrides the client default.
    pub async fn complete_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatCompletion, ChatError> {
        self.complete_candidates(request, Some(context)).await
    }

    async fn complete_candidates(
        &self,
        mut request: ChatRequest,
        context: Option<&MediaExecutionContext>,
    ) -> Result<ChatCompletion, ChatError> {
        let mut last_error = None;
        for candidate in &self.candidates {
            request.model.clone_from(&candidate.model);
            let (candidate_request, decisions) = match context {
                Some(context) => {
                    let PreparedRequest { request, decisions } = context
                        .prepare_request(&candidate.provider, request.clone())
                        .await;
                    (request, decisions)
                }
                None => (request.clone(), Vec::new()),
            };
            match candidate.client.complete(candidate_request).await {
                Ok(mut completion) => {
                    completion.selected_model_id = Some(candidate.model_id.clone());
                    completion.media_decisions = decisions;
                    return Ok(completion);
                }
                Err(error) if can_fallback(&error) => last_error = Some(error),
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
        self.stream_candidates(request, self.media_context.clone())
            .await
    }

    /// Stream using an ephemeral context that overrides the client default.
    pub async fn stream_with_media(
        &self,
        request: ChatRequest,
        context: &MediaExecutionContext,
    ) -> Result<ChatStream, ChatError> {
        self.stream_candidates(request, Some(context.clone())).await
    }

    async fn stream_candidates(
        &self,
        request: ChatRequest,
        context: Option<MediaExecutionContext>,
    ) -> Result<ChatStream, ChatError> {
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
                let decisions = if let Some(context) = &context {
                    let PreparedRequest { request: prepared, decisions } =
                        context
                            .prepare_request(&candidate.provider, candidate_request)
                            .await;
                    candidate_request = prepared;
                    decisions
                } else {
                    Vec::new()
                };
                let candidate_error = match candidate.client.stream(candidate_request).await {
                    Ok(mut response_stream) => {
                        let mut emitted = false;
                        let mut media_reported = false;
                        let mut failure = None;
                        while let Some(item) = response_stream.next().await {
                            match item {
                                Ok(event) => {
                                    if !media_reported && !decisions.is_empty() {
                                        yield Ok(ChatEvent::new(ChatEventKind::MediaRouted {
                                            decisions: decisions.clone(),
                                        }));
                                        media_reported = true;
                                    }
                                    emitted = true;
                                    yield Ok(event);
                                }
                                Err(error) => {
                                    if emitted || !can_fallback(&error) {
                                        yield Err(error);
                                        return;
                                    }
                                    failure = Some(error);
                                    break;
                                }
                            }
                        }
                        if failure.is_none() && !media_reported && !decisions.is_empty() {
                            yield Ok(ChatEvent::new(ChatEventKind::MediaRouted { decisions }));
                        }
                        failure
                    }
                    Err(error) => Some(error),
                };
                if let Some(error) = candidate_error {
                    if !can_fallback(&error) {
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

fn can_fallback(error: &ChatError) -> bool {
    matches!(
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
    use super::{can_fallback, normalize_provider};
    use crate::chat::ChatError;

    #[test]
    fn provider_aliases_are_normalized_before_same_provider_filtering() {
        assert_eq!(normalize_provider("google-genai"), "google");
        assert_eq!(normalize_provider("Google"), "google");
        assert_eq!(normalize_provider("nine-router"), "nine_router");
    }

    #[test]
    fn fallback_eligibility_does_not_depend_on_candidate_position() {
        let eligible = [
            ChatError::Quota,
            ChatError::RateLimited { retry_after: None },
            ChatError::Connection,
            ChatError::Timeout,
            ChatError::Overloaded,
            ChatError::ProviderIncompatible,
            ChatError::MultimodalIncompatible,
            ChatError::Server { status: 503 },
            ChatError::Server { status: 529 },
        ];
        assert!(eligible.iter().all(can_fallback));
    }

    #[test]
    fn fallback_does_not_hide_permanent_errors_from_later_candidates() {
        let permanent = [
            ChatError::Authentication,
            ChatError::Provider { status: 400 },
            ChatError::Server { status: 500 },
            ChatError::InvalidConfiguration("invalid request"),
            ChatError::InvalidResponse,
            ChatError::Protocol("invalid stream"),
            ChatError::Cancelled,
        ];
        assert!(permanent.iter().all(|error| !can_fallback(error)));
    }
}
