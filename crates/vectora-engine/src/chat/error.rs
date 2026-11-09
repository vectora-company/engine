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
    #[error("provider is temporarily overloaded")]
    Overloaded,
    #[error("provider rejected a request because its model is incompatible with the payload")]
    ProviderIncompatible,
    #[error("provider model does not support the requested image input")]
    MultimodalIncompatible,
    #[error("all configured chat fallback candidates failed (last: {model_id})")]
    FallbackExhausted { model_id: String },
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

pub(crate) fn classify_provider_error(
    status: u16,
    retry_after: Option<Duration>,
    body: &[u8],
) -> ChatError {
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
        return ChatError::Overloaded;
    }
    if status == 400
        && [
            "tool_plan",
            "tool plan` cannot be used",
            "tool plan cannot be used",
        ]
        .iter()
        .any(|marker| message.contains(marker))
    {
        return ChatError::ProviderIncompatible;
    }
    if status == 400
        && [
            "image content is not supported",
            "image input is not supported",
            "image_url is not supported",
            "unsupported content type: image",
            "does not support image",
            "model does not support vision",
        ]
        .iter()
        .any(|marker| message.contains(marker))
    {
        return ChatError::MultimodalIncompatible;
    }
    classify_status(status, retry_after)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_provider_fallback_reasons_without_exposing_body() {
        assert!(matches!(
            classify_provider_error(529, None, br#"{"type":"overloaded_error"}"#),
            ChatError::Overloaded
        ));
        assert!(matches!(
            classify_provider_error(400, None, b"tool_plan cannot be used with this model"),
            ChatError::ProviderIncompatible
        ));
        assert!(matches!(
            classify_provider_error(400, None, b"model does not support vision"),
            ChatError::MultimodalIncompatible
        ));
        assert!(matches!(
            classify_provider_error(500, None, b"internal error"),
            ChatError::Server { status: 500 }
        ));
    }
}
