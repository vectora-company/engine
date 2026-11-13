use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::ser::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::media::{InlineMedia, MediaDecision, MediaKind, MediaPart, classify};

pub const CHAT_CONTRACT_VERSION: u16 = 1;
pub const EPHEMERAL_CHAT_REQUEST_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: ImageUrl,
    },
    /// Mídia por referência: o histórico guarda só o `MediaRef`.
    Image(MediaPart),
    Audio(MediaPart),
    Video(MediaPart),
    Document(MediaPart),
    File(MediaPart),
    /// Bytes em base64, montados só na cópia do request enviada ao provider
    /// (ver `prepare_content`). Não deve ser persistido.
    Inline(InlineMedia),
}

impl ContentPart {
    /// Texto que representa uma parte de mídia nos adapters que ainda não a
    /// enviam como arquivo: a forma textual ou uma nota, nunca os bytes.
    /// Devolve `None` para texto e imagem por URL, que têm caminho próprio.
    pub fn fallback_text(&self) -> Option<String> {
        use super::media::MediaKind;
        match self {
            Self::Text { .. } | Self::ImageUrl { .. } => None,
            Self::Image(media) => Some(media.textual_form(MediaKind::Image)),
            Self::Audio(media) => Some(media.textual_form(MediaKind::Audio)),
            Self::Video(media) => Some(media.textual_form(MediaKind::Video)),
            Self::Document(media) => Some(media.textual_form(MediaKind::Document)),
            Self::File(media) => Some(media.textual_form(MediaKind::File)),
            Self::Inline(item) => Some(format!(
                "[{}: {} — {}]",
                item.kind.label(),
                item.name,
                item.mime_type
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: MessageContent,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Marks a tool result as an error for providers that preserve this bit.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_error: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Message {
    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: MessageContent::Text(text.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ToolArguments {
    Object { value: BTreeMap<String, Value> },
    InvalidJson { raw: String },
}

impl ToolArguments {
    pub fn object(value: BTreeMap<String, Value>) -> Self {
        Self::Object { value }
    }

    pub fn from_json(raw: &str) -> Self {
        match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(map)) => Self::Object {
                value: map.into_iter().collect(),
            },
            _ => Self::InvalidJson {
                raw: raw.to_owned(),
            },
        }
    }

    pub(crate) fn wire_value(&self) -> String {
        match self {
            Self::Object { value } => {
                serde_json::to_string(value).expect("JSON values are always serializable")
            }
            Self::InvalidJson { raw } => raw.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: ToolArguments,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct GenerationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatRequest {
    pub contract_version: u16,
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub options: GenerationOptions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    /// Provider-specific request fields. Reserved canonical fields are
    /// rejected so extensions cannot replace engine-owned values.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
}

/// JSON-safe request boundary: inline bytes and data URLs are not accepted or
/// serialized. Use `EphemeralChatRequest` for an explicitly transient send.
#[derive(Clone, Debug, PartialEq)]
pub struct PersistableChatRequest {
    request: ChatRequest,
}

impl PersistableChatRequest {
    pub fn new(request: ChatRequest) -> Result<Self, String> {
        validate_persistable(&request)?;
        Ok(Self { request })
    }

    pub fn into_inner(self) -> ChatRequest {
        self.request
    }
}

impl TryFrom<ChatRequest> for PersistableChatRequest {
    type Error = String;

    fn try_from(request: ChatRequest) -> Result<Self, Self::Error> {
        Self::new(request)
    }
}

impl Serialize for PersistableChatRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        validate_persistable(&self.request).map_err(S::Error::custom)?;
        self.request.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PersistableChatRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let request = ChatRequest::deserialize(deserializer)?;
        Self::new(request).map_err(serde::de::Error::custom)
    }
}

fn validate_persistable(request: &ChatRequest) -> Result<(), String> {
    for message in &request.messages {
        let MessageContent::Parts(parts) = &message.content else {
            continue;
        };
        for part in parts {
            match part {
                ContentPart::Inline(_) => {
                    return Err("inline media requires an ephemeral chat request".into());
                }
                ContentPart::ImageUrl { image_url } if image_url.url.starts_with("data:") => {
                    return Err("data URLs require an ephemeral chat request".into());
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Explicit envelope for a request that may contain media bytes and must only
/// be held in memory for a provider call.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EphemeralChatRequest {
    pub contract_version: u16,
    pub request: ChatRequest,
}

impl EphemeralChatRequest {
    pub fn new(request: ChatRequest) -> Self {
        Self {
            contract_version: EPHEMERAL_CHAT_REQUEST_CONTRACT_VERSION,
            request,
        }
    }

    /// Checks and canonicalizes all inline bytes before a provider send.
    pub fn validate(&mut self, max_media_bytes: u64) -> Result<(), String> {
        if self.contract_version != EPHEMERAL_CHAT_REQUEST_CONTRACT_VERSION {
            return Err("unsupported ephemeral chat request version".into());
        }
        for message in &mut self.request.messages {
            let MessageContent::Parts(parts) = &mut message.content else {
                continue;
            };
            for part in parts {
                match part {
                    ContentPart::Inline(item) => {
                        let encoded_limit = max_media_bytes.div_ceil(3).saturating_mul(4);
                        if item.data_base64.len() as u64 > encoded_limit {
                            return Err("inline media exceeds the configured byte limit".into());
                        }
                        let bytes = STANDARD
                            .decode(&item.data_base64)
                            .map_err(|_| "inline media has invalid base64".to_owned())?;
                        if bytes.len() as u64 > max_media_bytes {
                            return Err("inline media exceeds the configured byte limit".into());
                        }
                        let detected = classify(&bytes, Some(&item.mime_type));
                        if detected.kind != item.kind
                            || (item.kind == MediaKind::Document
                                && detected.mime_type != "application/pdf")
                        {
                            return Err("inline media type does not match its bytes".into());
                        }
                        item.mime_type = detected.mime_type;
                        item.data_base64 = STANDARD.encode(bytes);
                    }
                    ContentPart::ImageUrl { image_url } if image_url.url.starts_with("data:") => {
                        let (mime, encoded) = parse_data_url(&image_url.url)?;
                        let encoded_limit = max_media_bytes.div_ceil(3).saturating_mul(4);
                        if encoded.len() as u64 > encoded_limit {
                            return Err("inline media exceeds the configured byte limit".into());
                        }
                        let bytes = STANDARD
                            .decode(encoded)
                            .map_err(|_| "inline media has invalid base64".to_owned())?;
                        if bytes.len() as u64 > max_media_bytes {
                            return Err("inline media exceeds the configured byte limit".into());
                        }
                        let detected = classify(&bytes, Some(mime));
                        if detected.kind != MediaKind::Image {
                            return Err("data URL bytes are not a recognized image".into());
                        }
                        image_url.url = format!(
                            "data:{};base64,{}",
                            detected.mime_type,
                            STANDARD.encode(bytes)
                        );
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn into_inner(self) -> ChatRequest {
        self.request
    }
}

impl From<ChatRequest> for EphemeralChatRequest {
    fn from(request: ChatRequest) -> Self {
        Self::new(request)
    }
}

impl Deref for EphemeralChatRequest {
    type Target = ChatRequest;

    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

impl DerefMut for EphemeralChatRequest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.request
    }
}

fn parse_data_url(url: &str) -> Result<(&str, &str), String> {
    let value = url
        .strip_prefix("data:")
        .ok_or_else(|| "invalid data URL".to_owned())?;
    let (metadata, encoded) = value
        .split_once(',')
        .ok_or_else(|| "invalid data URL".to_owned())?;
    let mime = metadata
        .strip_suffix(";base64")
        .ok_or_else(|| "data URL must use base64 encoding".to_owned())?;
    if mime.is_empty() {
        return Err("data URL is missing a MIME type".into());
    }
    Ok((mime, encoded))
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            contract_version: CHAT_CONTRACT_VERSION,
            model: model.into(),
            messages,
            options: GenerationOptions::default(),
            tools: Vec::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod request_boundary_tests {
    use super::*;

    fn request_with(part: ContentPart) -> ChatRequest {
        let mut request = ChatRequest::new("model", vec![Message::text(MessageRole::User, "")]);
        request.messages[0].content = MessageContent::Parts(vec![part]);
        request
    }

    fn png_inline() -> ContentPart {
        ContentPart::Inline(InlineMedia {
            kind: MediaKind::Image,
            mime_type: "image/jpeg".into(),
            name: "pixel.jpg".into(),
            data_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8V8AAAAASUVORK5CYII=".into(),
        })
    }

    #[test]
    fn persistable_request_rejects_inline_bytes_and_data_urls() {
        assert!(PersistableChatRequest::new(request_with(png_inline())).is_err());
        assert!(
            PersistableChatRequest::new(request_with(ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "data:image/png;base64,iVBORw0KGgo=".into(),
                    detail: None
                },
            }))
            .is_err()
        );
    }

    #[test]
    fn ephemeral_request_validates_bytes_and_canonicalizes_mime() {
        let mut request = EphemeralChatRequest::new(request_with(png_inline()));
        request.validate(1024).unwrap();
        let MessageContent::Parts(parts) = &request.messages[0].content else {
            panic!()
        };
        let ContentPart::Inline(media) = &parts[0] else {
            panic!()
        };
        assert_eq!(media.mime_type, "image/png");
    }

    #[test]
    fn ephemeral_request_rejects_wrong_category_and_oversize_payload() {
        let mut wrong_kind =
            EphemeralChatRequest::new(request_with(ContentPart::Inline(InlineMedia {
                kind: MediaKind::Audio,
                mime_type: "audio/wav".into(),
                name: "x".into(),
                data_base64: "aGVsbG8=".into(),
            })));
        assert!(wrong_kind.validate(1024).is_err());
        let mut oversized = EphemeralChatRequest::new(request_with(png_inline()));
        assert!(oversized.validate(1).is_err());
    }

    #[test]
    fn ephemeral_data_urls_are_decoded_and_mime_checked() {
        let part = ContentPart::ImageUrl { image_url: ImageUrl {
            url: "data:image/jpeg;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8V8AAAAASUVORK5CYII=".into(), detail: None,
        }};
        let mut request = EphemeralChatRequest::new(request_with(part));
        request.validate(1024).unwrap();
        let MessageContent::Parts(parts) = &request.messages[0].content else {
            panic!()
        };
        let ContentPart::ImageUrl { image_url } = &parts[0] else {
            panic!()
        };
        assert!(image_url.url.starts_with("data:image/png;base64,"));
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}

impl FinishReason {
    pub(crate) fn from_provider(value: &str) -> Self {
        match value {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tool_calls" | "function_call" => Self::ToolCalls,
            "content_filter" => Self::ContentFilter,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatCompletion {
    pub contract_version: u16,
    pub id: Option<String>,
    /// Selected provider/model identifier when fallback chose a candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model_id: Option<String>,
    pub content: String,
    /// Reasoning-channel content, kept separate from user-facing text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<Usage>,
    /// Media routing evidence; never includes media bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_decisions: Vec<MediaDecision>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub arguments_fragment: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEventKind {
    MediaRouted {
        decisions: Vec<MediaDecision>,
    },
    ModelSwitched {
        from_model_id: String,
        to_model_id: String,
    },
    TextDelta {
        text: String,
    },
    ReasoningDelta {
        text: String,
    },
    ToolCallDelta {
        call: ToolCallDelta,
    },
    Usage {
        usage: Usage,
    },
    Completed {
        finish_reason: Option<FinishReason>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatEvent {
    pub contract_version: u16,
    #[serde(flatten)]
    pub event: ChatEventKind,
}

impl ChatEvent {
    pub(crate) fn new(event: ChatEventKind) -> Self {
        Self {
            contract_version: CHAT_CONTRACT_VERSION,
            event,
        }
    }
}
