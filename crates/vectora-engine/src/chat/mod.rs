//! Versioned provider-neutral chat contracts and first provider transport.

mod error;
mod openai_compatible;
mod sse;
mod types;

pub use error::ChatError;
pub use openai_compatible::{
    ChatStream, OpenAiCompatibleClient, OpenAiCompatibleConfig, OpenAiCompatibleSettings,
};
pub use types::{
    CHAT_CONTRACT_VERSION, ChatCompletion, ChatEvent, ChatEventKind, ChatRequest, ContentPart,
    FinishReason, GenerationOptions, ImageUrl, Message, MessageContent, MessageRole, ToolArguments,
    ToolCall, ToolCallDelta, ToolDefinition, Usage,
};
