use std::time::Duration;

/// Provider failures shared by embedding and reranking protocols.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LlmError {
    #[error("invalid model capability configuration: {0}")]
    InvalidConfiguration(&'static str),
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
    #[error("provider is temporarily overloaded")]
    Overloaded,
    #[error("connection to the provider failed")]
    Connection,
    #[error("provider request timed out")]
    Timeout,
    #[error("provider response exceeded the configured size limit")]
    ResponseTooLarge,
    #[error("provider returned an invalid response")]
    InvalidResponse,
}

pub(crate) fn classify_http_error(
    status: u16,
    retry_after: Option<Duration>,
    body: &[u8],
) -> LlmError {
    let message = String::from_utf8_lossy(body).to_ascii_lowercase();
    if (500..=599).contains(&status)
        && [
            "overloaded_error",
            "overloaded",
            "high demand",
            "service unavailable",
        ]
        .iter()
        .any(|marker| message.contains(marker))
    {
        return LlmError::Overloaded;
    }
    match status {
        401 | 403 => LlmError::Authentication,
        402 => LlmError::Quota,
        429 => LlmError::RateLimited { retry_after },
        500..=599 => LlmError::Server { status },
        _ => LlmError::Provider { status },
    }
}
