//! Reusable model capabilities beyond chat.

pub mod embeddings;
pub mod error;
mod http;
pub mod rerank;
pub mod transcription;

pub use error::LlmError;
