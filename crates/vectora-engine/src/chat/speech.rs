//! Binary speech synthesis contracts shared by provider adapters.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Output encodings supported by the selected llama-server speech route.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeechFormat {
    #[default]
    Wav,
    Pcm16,
    /// Only supported by the native `/tts` route, not `/v1/audio/speech`.
    PcmF32,
}

/// Persistable text-to-speech input. It contains no audio bytes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpeechRequest {
    pub model: String,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default)]
    pub format: SpeechFormat,
    /// llama-server v1.0.0 documents this field as ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// This profile accepts only 1.0, as required by llama-server v1.0.0.
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_speed() -> f32 {
    1.0
}

/// Additional native `/tts` controls that have no OpenAI speech equivalent.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeSpeechRequest {
    pub request: SpeechRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_predict: Option<u32>,
}

/// Request-only speaker audio for voice cloning.
#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EphemeralNativeSpeechRequest {
    pub request: NativeSpeechRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_ref_b64: Option<String>,
}

impl fmt::Debug for EphemeralNativeSpeechRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EphemeralNativeSpeechRequest")
            .field("request", &self.request)
            .field(
                "speaker_ref_b64",
                &self.speaker_ref_b64.as_ref().map(|_| "[REDACTED AUDIO]"),
            )
            .finish_non_exhaustive()
    }
}

/// Binary audio returned by a TTS endpoint, with the actual server format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechResponse {
    pub audio: Vec<u8>,
    pub content_type: String,
    pub format: SpeechFormat,
    /// The model's native sample rate parsed from `Content-Type`, if present.
    pub sample_rate_hz: Option<u32>,
}

impl SpeechRequest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.model.trim().is_empty() || self.input.trim().is_empty() {
            return Err("model and non-empty speech input are required");
        }
        if !self.speed.is_finite() || self.speed != 1.0 {
            return Err("llama-server v1.0.0 only accepts speech speed 1.0");
        }
        if self
            .voice
            .as_ref()
            .is_some_and(|voice| voice.trim().is_empty())
        {
            return Err("voice must not be empty");
        }
        Ok(())
    }
}
