//! Versioned provider-neutral chat contracts and first provider transport.

mod decision_model;
mod error;
mod fallback;
mod laya;
mod llama_server;
mod media;
mod native_providers;
mod openai_compatible;
mod realtime;
mod speech;
mod sse;
mod turn;
mod types;

pub use decision_model::{
    DecisionQuestion, DecisionQuestionKind, DecisionRequest, DecisionResponse, DecisionUsage,
    EphemeralDecisionRequest,
};
pub use error::ChatError;
pub use fallback::{
    FallbackCandidateSettings, FallbackChatClient, FallbackChatSettings, FallbackClientSettings,
};
pub use laya::{LayaClient, LayaHealth, LayaSettings};
pub use llama_server::{
    LlamaServerArchitecture, LlamaServerClient, LlamaServerHealth, LlamaServerModel,
    LlamaServerModelList, LlamaServerProperties, LlamaServerSettings, SpeechStream,
};
pub use media::{
    CapabilityResolver, CapabilityState, Classified, ContentSite, DEFAULT_MAX_MEDIA_BYTES,
    InlineMedia, MediaDecision, MediaError, MediaExecutionContext, MediaKind, MediaMode, MediaPart,
    MediaRef, MediaSizeLimits, MediaStore, Modality, ModalityCapabilities, PartialCapabilities,
    PreparedContent, PreparedRequest, ProviderMediaLimits, classify, prepare_content,
    prepare_content_with_limit, prepare_content_with_limits, sha256_hex,
};
pub use native_providers::{NativeChatClient, NativeChatSettings, NativeProvider};
pub use openai_compatible::{
    ChatStream, OpenAiCompatibleClient, OpenAiCompatibleConfig, OpenAiCompatibleProfile,
    OpenAiCompatibleSettings,
};
pub use realtime::{RealtimeEvent, RealtimeIntent, RealtimeSession, RealtimeSessionRequest};
pub use speech::{
    EphemeralNativeSpeechRequest, NativeSpeechRequest, SpeechFormat, SpeechRequest, SpeechResponse,
    SpeechStreamInfo,
};
pub use turn::{TurnAccumulator, TurnAccumulatorSettings, TurnError, assemble_turn};
pub use types::{
    CHAT_CONTRACT_VERSION, ChatCompletion, ChatEvent, ChatEventKind, ChatRequest, ContentPart,
    EPHEMERAL_CHAT_REQUEST_CONTRACT_VERSION, EphemeralChatRequest, FinishReason, GenerationOptions,
    ImageUrl, Message, MessageContent, MessageRole, PersistableChatRequest, ToolArguments,
    ToolCall, ToolCallDelta, ToolDefinition, Usage,
};
