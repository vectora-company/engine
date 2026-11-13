use std::{collections::HashSet, sync::Arc};

use async_stream::stream;
use futures_util::StreamExt;
use serde::Deserialize;

use super::media::{has_native_media, mark_text_retry};
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
            let (candidate_request, mut decisions) = match context {
                Some(context) => {
                    let PreparedRequest { request, decisions } = context
                        .prepare_request(&candidate.provider, request.clone())
                        .await;
                    (request, decisions)
                }
                None => (request.clone(), Vec::new()),
            };
            let result = candidate.client.complete(candidate_request).await;
            let result = match (result, context) {
                (Err(ChatError::MultimodalIncompatible), Some(context))
                    if has_native_media(&decisions) =>
                {
                    let fallback = context
                        .prepare_request_text_fallback(&candidate.provider, request.clone())
                        .await;
                    decisions = fallback.decisions;
                    mark_text_retry(&mut decisions);
                    candidate.client.complete(fallback.request).await
                }
                (result, _) => result,
            };
            match result {
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
                let mut decisions = if let Some(context) = &context {
                    let PreparedRequest { request: prepared, decisions } =
                        context
                            .prepare_request(&candidate.provider, candidate_request)
                            .await;
                    candidate_request = prepared;
                    decisions
                } else {
                    Vec::new()
                };
                let initial_stream = candidate.client.stream(candidate_request).await;
                let initial_stream = match (initial_stream, &context) {
                    (Err(ChatError::MultimodalIncompatible), Some(context))
                        if has_native_media(&decisions) =>
                    {
                        let mut text_request = request.clone();
                        text_request.model.clone_from(&candidate.model);
                        let fallback = context
                            .prepare_request_text_fallback(&candidate.provider, text_request)
                            .await;
                        decisions = fallback.decisions;
                        mark_text_retry(&mut decisions);
                        candidate.client.stream(fallback.request).await
                    }
                    (result, _) => result,
                };
                let candidate_error = match initial_stream {
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
    use std::{
        collections::HashMap,
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
    };

    use async_trait::async_trait;
    use serde_json::Value;

    use super::{FallbackChatClient, can_fallback, normalize_provider};
    use crate::chat::{
        CapabilityResolver, ChatError, ChatRequest, ContentPart, MediaError, MediaExecutionContext,
        MediaPart, MediaRef, MediaStore, Message, MessageContent, MessageRole, sha256_hex,
    };

    struct Store(HashMap<String, Vec<u8>>);

    #[async_trait]
    impl MediaStore for Store {
        async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError> {
            self.0
                .get(&media_ref.asset_id)
                .cloned()
                .ok_or(MediaError::Unavailable)
        }
    }

    fn pdf_request(bytes: &[u8]) -> ChatRequest {
        ChatRequest::new(
            "model-x",
            vec![Message {
                role: MessageRole::User,
                content: MessageContent::Parts(vec![ContentPart::Document(MediaPart {
                    media_ref: MediaRef {
                        asset_id: "pdf".into(),
                        sha256: sha256_hex(bytes),
                        mime_type: "application/pdf".into(),
                        size_bytes: bytes.len() as u64,
                        name: "report.pdf".into(),
                    },
                    text: Some("extracted text".into()),
                    previews: Vec::new(),
                })]),
                tool_calls: Vec::new(),
                tool_call_id: None,
                name: None,
                is_error: false,
            }],
        )
    }

    fn test_server() -> (String, thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for index in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut received = Vec::new();
                let mut chunk = [0; 4096];
                let (body_start, content_length) = loop {
                    let count = socket.read(&mut chunk).unwrap();
                    assert_ne!(count, 0);
                    received.extend_from_slice(&chunk[..count]);
                    if let Some(start) =
                        received.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&received[..start]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        if received.len() >= start + 4 + length {
                            break (start + 4, length);
                        }
                    }
                };
                requests.push(
                    serde_json::from_slice(&received[body_start..body_start + content_length])
                        .unwrap(),
                );
                let (status, body) = if index == 0 {
                    (
                        "400 Bad Request",
                        br#"{"error":{"message":"PDF input is not supported"}}"#.as_slice(),
                    )
                } else {
                    (
                        "200 OK",
                        br#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#
                            .as_slice(),
                    )
                };
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                socket.write_all(body).unwrap();
            }
            requests
        });
        (format!("http://{address}/v1"), server)
    }

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

    #[tokio::test]
    async fn fallback_retries_same_candidate_as_text_before_exhausting_candidates() {
        let (base_url, server) = test_server();
        let settings: super::FallbackChatSettings = serde_json::from_value(serde_json::json!({
            "candidates": [{
                "model_id": "openai:model-x",
                "client": {
                    "protocol": "open_ai_compatible",
                    "settings": { "base_url": base_url }
                }
            }]
        }))
        .unwrap();
        let client = FallbackChatClient::new(settings).unwrap();
        let bytes = b"%PDF-1.7\ncontents";
        let context = MediaExecutionContext::new(
            CapabilityResolver::new(),
            Arc::new(Store(HashMap::from([("pdf".into(), bytes.to_vec())]))),
        );
        let completion = client
            .complete_with_media(pdf_request(bytes), &context)
            .await
            .unwrap();
        assert_eq!(completion.content, "ok");
        assert!(
            completion.media_decisions[0]
                .reason
                .contains("única nova tentativa")
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["messages"][0]["content"][0]["type"], "file");
        assert_eq!(requests[1]["messages"][0]["content"][0]["type"], "text");
    }
}
