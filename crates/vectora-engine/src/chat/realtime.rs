//! Realtime PCM audio sessions for the llama-server WebSocket API.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_websockets::{CloseCode, Message, WebSocketStream, tls::MaybeTlsStream};

use super::ChatError;

/// Selects transcription-only or interactive audio conversation semantics.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeIntent {
    /// Stream audio and receive transcription events only.
    #[default]
    Transcription,
    /// Realtime audio conversation; event flow is controlled by the host.
    Conversation,
}

/// Settings for one llama-server realtime WebSocket session.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealtimeSessionRequest {
    pub model: String,
    #[serde(default)]
    pub intent: RealtimeIntent,
    /// Optional PCM16 rate override; omitted uses the model-native rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_sample_rate_hz: Option<u32>,
}

impl RealtimeSessionRequest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.model.trim().is_empty() {
            return Err("model is required");
        }
        if self
            .input_sample_rate_hz
            .is_some_and(|rate| !(8_000..=192_000).contains(&rate))
        {
            return Err("input sample rate must be between 8000 and 192000 Hz");
        }
        Ok(())
    }
}

/// A server event with the original JSON preserved for forward compatibility.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RealtimeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub fields: serde_json::Map<String, Value>,
}

/// A connected realtime session. Dropping it closes the socket and cancels work.
pub struct RealtimeSession {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    read_timeout: Duration,
    max_event_bytes: usize,
    max_audio_bytes: usize,
}

impl RealtimeSession {
    pub(crate) fn new(
        socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
        read_timeout: Duration,
        max_event_bytes: usize,
        max_audio_bytes: usize,
    ) -> Self {
        Self {
            socket,
            read_timeout,
            max_event_bytes,
            max_audio_bytes,
        }
    }

    /// Sends one mono PCM16 chunk. The chunk must contain complete 16-bit samples.
    pub async fn send_pcm16(&mut self, audio: &[u8]) -> Result<(), ChatError> {
        if audio.is_empty() || !audio.len().is_multiple_of(2) || audio.len() > self.max_audio_bytes
        {
            return Err(ChatError::InvalidRequest(
                "audio chunk must be non-empty PCM16 and within the configured limit",
            ));
        }
        let event = serde_json::json!({
            "type": "input_audio_buffer.append",
            "audio": STANDARD.encode(audio),
        });
        self.send_event_value(event).await
    }

    /// Commits buffered audio; `final_chunk` ends the input session after draining.
    pub async fn commit_audio(&mut self, final_chunk: bool) -> Result<(), ChatError> {
        let event = if final_chunk {
            serde_json::json!({ "type": "input_audio_buffer.commit", "final": true })
        } else {
            serde_json::json!({ "type": "input_audio_buffer.commit" })
        };
        self.send_event_value(event).await
    }

    /// Sends a host-defined event for realtime conversation controls.
    pub async fn send_event(&mut self, event: Value) -> Result<(), ChatError> {
        if !event.is_object() || event.get("type").and_then(Value::as_str).is_none() {
            return Err(ChatError::InvalidRequest(
                "realtime events must be objects with a type",
            ));
        }
        self.send_event_value(event).await
    }

    /// Receives the next JSON event; unknown event types and fields are preserved.
    pub async fn receive(&mut self) -> Result<Option<RealtimeEvent>, ChatError> {
        let message = timeout(self.read_timeout, self.socket.next())
            .await
            .map_err(|_| ChatError::Timeout)?
            .ok_or(ChatError::Connection)?
            .map_err(|_| ChatError::Connection)?;
        if message.is_close() {
            return Ok(None);
        }
        let Some(text) = message.as_text() else {
            return Err(ChatError::InvalidResponse);
        };
        if text.len() > self.max_event_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        let event: RealtimeEvent =
            serde_json::from_str(text).map_err(|_| ChatError::InvalidResponse)?;
        Ok(Some(event))
    }

    /// Closes the session gracefully. Dropping the session also cancels it.
    pub async fn close(&mut self) -> Result<(), ChatError> {
        self.socket
            .send(Message::close(
                Some(CloseCode::NORMAL_CLOSURE),
                "client closed",
            ))
            .await
            .map_err(|_| ChatError::Connection)
    }

    pub async fn set_input_sample_rate(&mut self, sample_rate_hz: u32) -> Result<(), ChatError> {
        if !(8_000..=192_000).contains(&sample_rate_hz) {
            return Err(ChatError::InvalidRequest(
                "input sample rate must be between 8000 and 192000 Hz",
            ));
        }
        self.send_event(serde_json::json!({
            "type": "session.update",
            "audio": { "input": { "format": { "type": "audio/pcm", "rate": sample_rate_hz } } }
        }))
        .await
    }

    async fn send_event_value(&mut self, event: Value) -> Result<(), ChatError> {
        let encoded = serde_json::to_string(&event)
            .map_err(|_| ChatError::InvalidRequest("event could not be encoded"))?;
        if encoded.len() > self.max_event_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        self.socket
            .send(Message::text(encoded))
            .await
            .map_err(|_| ChatError::Connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_rates_and_preserves_unknown_and_error_event_fields() {
        let request = RealtimeSessionRequest {
            model: "audio-model".into(),
            intent: RealtimeIntent::Transcription,
            input_sample_rate_hz: Some(8_000),
        };
        assert!(request.validate().is_ok());
        assert!(
            RealtimeSessionRequest {
                input_sample_rate_hz: Some(7_999),
                ..request.clone()
            }
            .validate()
            .is_err()
        );
        let event: RealtimeEvent = serde_json::from_value(serde_json::json!({
            "type": "error", "error": {"message": "model is not realtime"}, "future_field": 1
        }))
        .unwrap();
        assert_eq!(event.event_type, "error");
        assert_eq!(event.fields["error"]["message"], "model is not realtime");
        assert_eq!(event.fields["future_field"], 1);
    }
}
