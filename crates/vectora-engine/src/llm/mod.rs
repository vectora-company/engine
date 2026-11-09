//! Reusable model capabilities beyond chat.

pub mod embeddings;
pub mod error;
mod http;
pub mod rerank;

pub use error::LlmError;
