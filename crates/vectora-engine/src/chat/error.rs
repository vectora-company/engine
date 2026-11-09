use std::time::Duration;

/// Failures have stable categories and never include request headers or bodies.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ChatError {
    #[error("invalid chat configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("unsupported chat contract version: {0}")]
    UnsupportedContractVersion(u16),
    #[error("provider authentication failed (HTTP 401)")]
    Authentication,
    #[error("provider quota or credit limit reached (HTTP 402)")]
    Quota,
    #[error("provider rate limit reached (HTTP 429)")]
    RateLimited { retry_after: Option<Duration> },
    #[error("provider rejected the request (HTTP {status})")]
    Provider { status: u16 },
    #[error("provider failed (HTTP {status})")]
    Server { status: u16 },
    #[error("connection to the provider failed")]
    Connection,
    #[error("provider request timed out")]
    Timeout,
    #[error("provider response exceeded the configured size limit")]
    ResponseTooLarge,
    #[error("provider returned an invalid response")]
    InvalidResponse,
    #[error("provider response violated the chat protocol: {0}")]
    Protocol(&'static str),
    #[error("chat request was cancelled")]
    Cancelled,
}

pub(crate) fn classify_status(status: u16, retry_after: Option<Duration>) -> ChatError {
    match status {
        401 => ChatError::Authentication,
        402 => ChatError::Quota,
        429 => ChatError::RateLimited { retry_after },
        500..=599 => ChatError::Server { status },
        _ => ChatError::Provider { status },
    }
}
