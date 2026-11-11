//! Core crate for reusable Vectora agent capabilities.
//!
//! Public runtime contracts are introduced only after their consumers and
//! conformance requirements are documented.

#![forbid(unsafe_code)]

/// Version of this crate's public package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod chat;
pub mod harness;
pub mod llm;
pub mod storage;
