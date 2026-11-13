//! Versioned provider-neutral chat contracts and first provider transport.

mod error;
mod fallback;
mod media;
mod native_providers;
mod openai_compatible;
mod sse;
mod turn;
mod types;

pub use error::ChatError;
pub use fallback::{
    FallbackCandidateSettings, FallbackChatClient, FallbackChatSettings, FallbackClientSettings,
};
pub use media::{
    CapabilityResolver, CapabilityState, Classified, ContentSite, InlineMedia, MediaDecision,
    MediaError, MediaExecutionContext, MediaKind, MediaMode, MediaPart, MediaRef, MediaSizeLimits,
    MediaStore, Modality, ModalityCapabilities, PartialCapabilities, PreparedContent,
    PreparedRequest, ProviderMediaLimits, classify, prepare_content, prepare_content_with_limit,
    prepare_content_with_limits, sha256_hex,
};
pub use native_providers::{NativeChatClient, NativeChatSettings, NativeProvider};
pub use openai_compatible::{
    ChatStream, OpenAiCompatibleClient, OpenAiCompatibleConfig, OpenAiCompatibleSettings,
};
pub use turn::{TurnAccumulator, TurnAccumulatorSettings, TurnError, assemble_turn};
pub use types::{
    CHAT_CONTRACT_VERSION, ChatCompletion, ChatEvent, ChatEventKind, ChatRequest, ContentPart,
    FinishReason, GenerationOptions, ImageUrl, Message, MessageContent, MessageRole, ToolArguments,
    ToolCall, ToolCallDelta, ToolDefinition, Usage,
};
