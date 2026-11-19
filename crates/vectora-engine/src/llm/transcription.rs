//! Batch speech-to-text over the OpenAI transcription wire protocol.

use std::collections::BTreeMap;

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chat::{MediaError, MediaExecutionContext, MediaKind, MediaRef};

use super::{LlmError, http::ProviderHttpClient, http::ProviderHttpSettings};

pub const TRANSCRIPTION_CONTRACT_VERSION: u16 = 1;
pub const DEFAULT_MAX_TRANSCRIPTION_BYTES: u64 = 25 * 1024 * 1024;
const OPENAI_URL: &str = "https://api.openai.com/v1";
const TRANSCRIPTIONS_PATH: &str = "audio/transcriptions";

/// An OpenAI-compatible endpoint that implements `POST /audio/transcriptions`.
#[derive(Deserialize)]
#[serde(tag = "protocol", content = "settings", rename_all = "snake_case")]
pub enum TranscriptionClientSettings {
    OpenAi(ProviderHttpSettings),
    OpenAiCompatible(ProviderHttpSettings),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionResponseFormat {
    #[default]
    Json,
    Text,
    Srt,
    VerboseJson,
    Vtt,
    DiarizedJson,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampGranularity {
    Segment,
    Word,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServerVadSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptionChunkingStrategy {
    Auto,
    ServerVad(ServerVadSettings),
}

/// A single bounded recording referenced in the consumer-owned asset map.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionRequest {
    pub contract_version: u16,
    pub model: String,
    pub media_ref: MediaRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub response_format: TranscriptionResponseFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<TimestampGranularity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<TranscriptionChunkingStrategy>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TranscriptionSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TranscriptionWord {
    pub start: f64,
    pub end: f64,
    pub word: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
}

/// Provider output. For `text`, `srt` and `vtt`, `text` contains the complete
/// formatted body; structured formats expose transcript metadata and segments.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TranscriptionResponse {
    pub contract_version: u16,
    pub response_format: TranscriptionResponseFormat,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<TranscriptionSegment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<TranscriptionWord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ProviderTranscription {
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    segments: Vec<TranscriptionSegment>,
    #[serde(default)]
    words: Vec<TranscriptionWord>,
    #[serde(default)]
    usage: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub struct TranscriptionClient {
    http: ProviderHttpClient,
    max_file_bytes: u64,
}

impl TranscriptionClient {
    pub fn new(settings: TranscriptionClientSettings) -> Result<Self, LlmError> {
        let (http_settings, requires_api_key) = match settings {
            TranscriptionClientSettings::OpenAi(settings) => (settings, true),
            TranscriptionClientSettings::OpenAiCompatible(settings) => (settings, false),
        };
        Ok(Self {
            http: ProviderHttpClient::new(http_settings, OPENAI_URL, requires_api_key)?,
            max_file_bytes: DEFAULT_MAX_TRANSCRIPTION_BYTES,
        })
    }

    /// Transcribes a consumer-owned asset without serializing its bytes into
    /// the request contract or persistence layer.
    pub async fn transcribe_ephemeral(
        &self,
        request: TranscriptionRequest,
        media: &MediaExecutionContext,
    ) -> Result<TranscriptionResponse, LlmError> {
        validate_request(&request)?;
        let materialized = media
            .materialize_transcription_asset(&request.media_ref)
            .await
            .map_err(map_media_error)?;
        if materialized.bytes.len() as u64 > self.max_file_bytes {
            return Err(LlmError::InvalidConfiguration(
                "transcription file exceeds the provider limit",
            ));
        }
        if !matches!(materialized.kind, MediaKind::Audio | MediaKind::Video)
            || !supported_mime(&materialized.mime_type)
        {
            return Err(LlmError::InvalidConfiguration(
                "transcription media type is not supported by this endpoint",
            ));
        }

        let filename = safe_filename(&materialized.name, &materialized.mime_type);
        let file = Part::bytes(materialized.bytes)
            .file_name(filename)
            .mime_str(&materialized.mime_type)
            .map_err(|_| LlmError::InvalidConfiguration("invalid media content type"))?;
        let mut form = Form::new()
            .part("file", file)
            .text("model", request.model)
            .text(
                "response_format",
                format_name(request.response_format).to_owned(),
            );
        if let Some(language) = request.language {
            form = form.text("language", language);
        }
        if let Some(prompt) = request.prompt {
            form = form.text("prompt", prompt);
        }
        if let Some(temperature) = request.temperature {
            form = form.text("temperature", temperature.to_string());
        }
        if let Some(strategy) = request.chunking_strategy {
            let value = serde_json::to_string(&strategy)
                .map_err(|_| LlmError::InvalidConfiguration("invalid chunking strategy"))?;
            let value = if matches!(strategy, TranscriptionChunkingStrategy::Auto) {
                "auto".to_owned()
            } else {
                value
            };
            form = form.text("chunking_strategy", value);
        }
        if let Some(granularities) = request.timestamp_granularities {
            for granularity in granularities {
                let name = match granularity {
                    TimestampGranularity::Segment => "segment",
                    TimestampGranularity::Word => "word",
                };
                form = form.text("timestamp_granularities[]", name.to_owned());
            }
        }

        let bytes = self.http.post_multipart(TRANSCRIPTIONS_PATH, form).await?;
        parse_response(request.response_format, &bytes)
    }
}

fn validate_request(request: &TranscriptionRequest) -> Result<(), LlmError> {
    if request.contract_version != TRANSCRIPTION_CONTRACT_VERSION {
        return Err(LlmError::InvalidConfiguration(
            "unsupported transcription contract version",
        ));
    }
    if request.model.trim().is_empty() {
        return Err(LlmError::InvalidConfiguration("model must not be empty"));
    }
    if request.media_ref.size_bytes == 0
        || request.media_ref.size_bytes > DEFAULT_MAX_TRANSCRIPTION_BYTES
    {
        return Err(LlmError::InvalidConfiguration(
            "transcription file exceeds the provider limit",
        ));
    }
    if request.language.as_ref().is_some_and(|language| {
        !matches!(language.len(), 2 | 3) || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    }) {
        return Err(LlmError::InvalidConfiguration("invalid language code"));
    }
    if request
        .prompt
        .as_ref()
        .is_some_and(|prompt| prompt.len() > 32 * 1024)
    {
        return Err(LlmError::InvalidConfiguration(
            "prompt exceeds the size limit",
        ));
    }
    if request
        .temperature
        .is_some_and(|temperature| !temperature.is_finite() || !(0.0..=1.0).contains(&temperature))
    {
        return Err(LlmError::InvalidConfiguration(
            "temperature must be between zero and one",
        ));
    }
    if request
        .timestamp_granularities
        .as_ref()
        .is_some_and(Vec::is_empty)
        || (request.timestamp_granularities.is_some()
            && request.response_format != TranscriptionResponseFormat::VerboseJson)
    {
        return Err(LlmError::InvalidConfiguration(
            "timestamp granularities require verbose_json",
        ));
    }
    if let Some(TranscriptionChunkingStrategy::ServerVad(settings)) = &request.chunking_strategy
        && settings
            .threshold
            .is_some_and(|threshold| !threshold.is_finite() || !(0.0..=1.0).contains(&threshold))
    {
        return Err(LlmError::InvalidConfiguration(
            "server VAD threshold must be between zero and one",
        ));
    }
    Ok(())
}

fn parse_response(
    response_format: TranscriptionResponseFormat,
    bytes: &[u8],
) -> Result<TranscriptionResponse, LlmError> {
    let response = match response_format {
        TranscriptionResponseFormat::Text
        | TranscriptionResponseFormat::Srt
        | TranscriptionResponseFormat::Vtt => {
            let text = std::str::from_utf8(bytes).map_err(|_| LlmError::InvalidResponse)?;
            TranscriptionResponse {
                contract_version: TRANSCRIPTION_CONTRACT_VERSION,
                response_format,
                text: text.to_owned(),
                language: None,
                duration_seconds: None,
                segments: Vec::new(),
                words: Vec::new(),
                usage: None,
                extra: BTreeMap::new(),
            }
        }
        TranscriptionResponseFormat::Json
        | TranscriptionResponseFormat::VerboseJson
        | TranscriptionResponseFormat::DiarizedJson => {
            let payload: ProviderTranscription =
                serde_json::from_slice(bytes).map_err(|_| LlmError::InvalidResponse)?;
            if payload
                .duration
                .is_some_and(|duration| !duration.is_finite() || duration < 0.0)
                || payload.segments.iter().any(|segment| {
                    !segment.start.is_finite()
                        || !segment.end.is_finite()
                        || segment.start < 0.0
                        || segment.end < segment.start
                })
                || payload.words.iter().any(|word| {
                    !word.start.is_finite()
                        || !word.end.is_finite()
                        || word.start < 0.0
                        || word.end < word.start
                })
            {
                return Err(LlmError::InvalidResponse);
            }
            TranscriptionResponse {
                contract_version: TRANSCRIPTION_CONTRACT_VERSION,
                response_format,
                text: payload.text,
                language: payload.language,
                duration_seconds: payload.duration,
                segments: payload.segments,
                words: payload.words,
                usage: payload.usage,
                extra: payload.extra,
            }
        }
    };
    if response.text.len() > 16 * 1024 * 1024 {
        return Err(LlmError::ResponseTooLarge);
    }
    Ok(response)
}

fn map_media_error(error: MediaError) -> LlmError {
    match error {
        MediaError::Unavailable => {
            LlmError::InvalidConfiguration("transcription asset unavailable")
        }
        MediaError::TooLarge => {
            LlmError::InvalidConfiguration("transcription file exceeds the provider limit")
        }
        MediaError::Integrity => {
            LlmError::InvalidConfiguration("transcription asset failed integrity validation")
        }
        MediaError::UnsupportedType => {
            LlmError::InvalidConfiguration("transcription media type is unsupported")
        }
    }
}

fn supported_mime(mime: &str) -> bool {
    matches!(
        mime,
        "audio/flac"
            | "audio/mpeg"
            | "audio/mp4"
            | "audio/mpga"
            | "audio/ogg"
            | "audio/wav"
            | "audio/webm"
            | "video/mp4"
            | "video/mpeg"
            | "video/webm"
    )
}

fn safe_filename(name: &str, mime: &str) -> String {
    let leaf = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = leaf
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '-' | '_')
        })
        .take(120)
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        extension_for_mime(mime).to_owned()
    } else {
        cleaned
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "audio/flac" => "audio.flac",
        "audio/mpeg" => "audio.mp3",
        "audio/mp4" => "audio.m4a",
        "audio/mpga" => "audio.mpga",
        "audio/ogg" => "audio.ogg",
        "audio/wav" => "audio.wav",
        "audio/webm" | "video/webm" => "audio.webm",
        "video/mp4" => "video.mp4",
        "video/mpeg" => "video.mpeg",
        _ => "audio.bin",
    }
}

fn format_name(format: TranscriptionResponseFormat) -> &'static str {
    match format {
        TranscriptionResponseFormat::Json => "json",
        TranscriptionResponseFormat::Text => "text",
        TranscriptionResponseFormat::Srt => "srt",
        TranscriptionResponseFormat::VerboseJson => "verbose_json",
        TranscriptionResponseFormat::Vtt => "vtt",
        TranscriptionResponseFormat::DiarizedJson => "diarized_json",
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::chat::{CapabilityResolver, MediaStore, sha256_hex};

    struct MemoryStore(HashMap<String, Vec<u8>>);

    #[async_trait]
    impl MediaStore for MemoryStore {
        async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError> {
            self.0
                .get(&media_ref.asset_id)
                .cloned()
                .ok_or(MediaError::Unavailable)
        }
    }

    fn wav() -> Vec<u8> {
        b"RIFF\x08\x00\x00\x00WAVEfmt ".to_vec()
    }

    fn media_ref(data: &[u8]) -> MediaRef {
        MediaRef {
            asset_id: "asset-1".into(),
            sha256: sha256_hex(data),
            mime_type: "audio/wav".into(),
            size_bytes: data.len() as u64,
            name: "sample.wav".into(),
        }
    }

    fn request(media_ref: MediaRef) -> TranscriptionRequest {
        TranscriptionRequest {
            contract_version: TRANSCRIPTION_CONTRACT_VERSION,
            model: "whisper-test".into(),
            media_ref,
            language: Some("pt".into()),
            prompt: Some("Nomes próprios: Vectora".into()),
            temperature: Some(0.0),
            response_format: TranscriptionResponseFormat::VerboseJson,
            timestamp_granularities: Some(vec![TimestampGranularity::Segment]),
            chunking_strategy: Some(TranscriptionChunkingStrategy::Auto),
        }
    }

    async fn read_http_request(
        listener: TcpListener,
        status: u16,
        extra_headers: &[(&str, &str)],
        response: &[u8],
    ) -> Vec<u8> {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 2048];
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0, "client closed before sending request");
            request.extend_from_slice(&chunk[..read]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
        let content_length: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .unwrap()
            .parse()
            .unwrap();
        while request.len() < header_end + content_length {
            let mut chunk = [0_u8; 4096];
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0, "client closed before completing multipart body");
            request.extend_from_slice(&chunk[..read]);
        }
        let reason = if status == 200 { "OK" } else { "Error" };
        let extra_headers = extra_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let headers = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n",
            response.len()
        );
        stream.write_all(headers.as_bytes()).await.unwrap();
        stream.write_all(response).await.unwrap();
        request
    }

    #[tokio::test]
    async fn sends_verified_asset_as_multipart_and_parses_verbose_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response_body = "{\"text\":\"olá\",\"language\":\"pt\",\"duration\":1.25,\"segments\":[{\"start\":0.0,\"end\":1.0,\"text\":\"olá\"}]}".as_bytes();
        let server = tokio::spawn(read_http_request(listener, 200, &[], response_body));
        let data = wav();
        let media_ref = media_ref(&data);
        let context = MediaExecutionContext::new(
            CapabilityResolver::new(),
            Arc::new(MemoryStore(HashMap::from([(
                "asset-1".into(),
                data.clone(),
            )]))),
        );
        let client = TranscriptionClient::new(TranscriptionClientSettings::OpenAiCompatible(
            ProviderHttpSettings {
                base_url: Some(format!("http://{address}/v1")),
                ..ProviderHttpSettings::default()
            },
        ))
        .unwrap();

        let response = client
            .transcribe_ephemeral(request(media_ref), &context)
            .await
            .unwrap();
        let wire = server.await.unwrap();
        let body = String::from_utf8_lossy(&wire);
        assert!(body.starts_with("POST /v1/audio/transcriptions HTTP/1.1\r\n"));
        assert!(body.contains("name=\"file\"; filename=\"sample.wav\""));
        assert!(
            body.to_ascii_lowercase()
                .contains("content-type: audio/wav")
        );
        assert!(body.contains("name=\"model\"\r\n\r\nwhisper-test"));
        assert!(body.contains("name=\"language\"\r\n\r\npt"));
        assert!(body.contains("name=\"timestamp_granularities[]\"\r\n\r\nsegment"));
        assert!(wire.windows(data.len()).any(|window| window == data));
        assert_eq!(response.text, "olá");
        assert_eq!(response.language.as_deref(), Some("pt"));
        assert_eq!(response.duration_seconds, Some(1.25));
        assert_eq!(response.segments.len(), 1);
    }

    #[tokio::test]
    async fn classifies_rate_limit_without_exposing_provider_error_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(read_http_request(
            listener,
            429,
            &[("Retry-After", "3")],
            br#"{"error":{"message":"private provider detail"}}"#,
        ));
        let data = wav();
        let context = MediaExecutionContext::new(
            CapabilityResolver::new(),
            Arc::new(MemoryStore(HashMap::from([(
                "asset-1".into(),
                data.clone(),
            )]))),
        );
        let client = TranscriptionClient::new(TranscriptionClientSettings::OpenAiCompatible(
            ProviderHttpSettings {
                base_url: Some(format!("http://{address}/v1")),
                ..ProviderHttpSettings::default()
            },
        ))
        .unwrap();

        let error = client
            .transcribe_ephemeral(request(media_ref(&data)), &context)
            .await
            .unwrap_err();
        server.await.unwrap();
        assert!(matches!(
            error,
            LlmError::RateLimited {
                retry_after: Some(duration)
            } if duration.as_secs() == 3
        ));
        assert!(!error.to_string().contains("private provider detail"));
    }

    #[tokio::test]
    async fn enforces_configured_response_limit_for_transcription() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = br#"{"text":"this response is too large"}"#;
        let server = tokio::spawn(read_http_request(listener, 200, &[], response));
        let data = wav();
        let context = MediaExecutionContext::new(
            CapabilityResolver::new(),
            Arc::new(MemoryStore(HashMap::from([(
                "asset-1".into(),
                data.clone(),
            )]))),
        );
        let client = TranscriptionClient::new(TranscriptionClientSettings::OpenAiCompatible(
            ProviderHttpSettings {
                base_url: Some(format!("http://{address}/v1")),
                max_response_bytes: 8,
                ..ProviderHttpSettings::default()
            },
        ))
        .unwrap();

        let error = client
            .transcribe_ephemeral(request(media_ref(&data)), &context)
            .await
            .unwrap_err();
        server.await.unwrap();
        assert!(matches!(error, LlmError::ResponseTooLarge));
    }

    #[test]
    fn validates_provider_options_and_file_names() {
        let mut request = request(media_ref(&wav()));
        request.response_format = TranscriptionResponseFormat::Json;
        assert!(validate_request(&request).is_err());

        request.timestamp_granularities = None;
        request.temperature = Some(f32::NAN);
        assert!(validate_request(&request).is_err());

        assert_eq!(
            safe_filename("../../private recording.wav", "audio/wav"),
            "private recording.wav"
        );
        assert_eq!(safe_filename("../..", "audio/mpeg"), "audio.mp3");
    }

    #[test]
    fn parses_text_and_subtitle_formats_and_rejects_malformed_json() {
        let text = parse_response(
            TranscriptionResponseFormat::Srt,
            b"1\n00:00:00,000 --> 00:00:01,000\nOi\n",
        )
        .unwrap();
        assert_eq!(text.text, "1\n00:00:00,000 --> 00:00:01,000\nOi\n");
        assert!(parse_response(TranscriptionResponseFormat::Json, b"not json").is_err());
        assert!(
            parse_response(
                TranscriptionResponseFormat::VerboseJson,
                br#"{"text":"x","segments":[{"start":2.0,"end":1.0,"text":"x"}]}"#
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn refuses_asset_integrity_mismatch_before_network_request() {
        let data = wav();
        let mut reference = media_ref(&data);
        reference.sha256 = "0".repeat(64);
        let context = MediaExecutionContext::new(
            CapabilityResolver::new(),
            Arc::new(MemoryStore(HashMap::from([("asset-1".into(), data)]))),
        );
        let client = TranscriptionClient::new(TranscriptionClientSettings::OpenAiCompatible(
            ProviderHttpSettings {
                base_url: Some("http://127.0.0.1:1/v1".into()),
                ..ProviderHttpSettings::default()
            },
        ))
        .unwrap();
        let error = client
            .transcribe_ephemeral(request(reference), &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("integrity"));
    }
}
