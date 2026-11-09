use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CHAT_CONTRACT_VERSION: u16 = 1;

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
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
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
}

impl Message {
    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: MessageContent::Text(text.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
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
    /// Provider-specific OpenAI-compatible request fields. Reserved canonical
    /// fields are rejected so extensions cannot replace engine-owned values.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
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
    pub content: String,
    /// Reasoning-channel content, kept separate from user-facing text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<Usage>,
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
    TextDelta { text: String },
    ReasoningDelta { text: String },
    ToolCallDelta { call: ToolCallDelta },
    Usage { usage: Usage },
    Completed { finish_reason: Option<FinishReason> },
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
