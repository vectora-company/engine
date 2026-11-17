//! Reusable model capabilities beyond chat.

pub mod embeddings;
pub mod error;
mod http;
pub mod image_generation;
pub mod rerank;
pub mod transcription;

pub use error::LlmError;
