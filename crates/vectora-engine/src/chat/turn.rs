//! Provider-neutral assembly of streaming chat events into one assistant turn.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::types::{
    CHAT_CONTRACT_VERSION, ChatCompletion, ChatEvent, ChatEventKind, FinishReason, ToolArguments,
    ToolCall, Usage,
};

const fn default_max_text_bytes() -> usize {
    4 * 1024 * 1024
}

const fn default_max_tool_calls() -> usize {
    128
}

const fn default_max_tool_argument_bytes() -> usize {
    1024 * 1024
}

/// Limits and correlation identity used to assemble one streamed turn.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TurnAccumulatorSettings {
    pub turn_id: String,
    #[serde(default = "default_max_text_bytes")]
    pub max_text_bytes: usize,
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: usize,
    #[serde(default = "default_max_tool_argument_bytes")]
    pub max_tool_argument_bytes: usize,
}

impl TurnAccumulatorSettings {
    /// Create settings using the conservative default accumulation limits.
    pub fn new(turn_id: impl Into<String>) -> Self {
        Self {
            turn_id: turn_id.into(),
            max_text_bytes: default_max_text_bytes(),
            max_tool_calls: default_max_tool_calls(),
            max_tool_argument_bytes: default_max_tool_argument_bytes(),
        }
    }

    fn validate(&self) -> Result<(), TurnError> {
        if self.turn_id.trim().is_empty() {
            return Err(TurnError::EmptyTurnId);
        }
        if self.max_tool_calls == 0 {
            return Err(TurnError::InvalidLimit("max_tool_calls"));
        }
        if self.max_tool_argument_bytes == 0 {
            return Err(TurnError::InvalidLimit("max_tool_argument_bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct ToolCallFragments {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

/// Stateful assembler for a single provider stream. It does not execute tools.
#[derive(Clone, Debug)]
pub struct TurnAccumulator {
    settings: TurnAccumulatorSettings,
    content: String,
    content_bytes: usize,
    tool_calls: BTreeMap<u32, ToolCallFragments>,
    usage: Option<Usage>,
    selected_model_id: Option<String>,
    finish_reason: Option<FinishReason>,
    completed: bool,
    received_provider_event: bool,
}

impl TurnAccumulator {
    /// Start assembling a turn with explicit correlation identity and limits.
    pub fn new(settings: TurnAccumulatorSettings) -> Result<Self, TurnError> {
        settings.validate()?;
        Ok(Self {
            settings,
            content: String::new(),
            content_bytes: 0,
            tool_calls: BTreeMap::new(),
            usage: None,
            selected_model_id: None,
            finish_reason: None,
            completed: false,
            received_provider_event: false,
        })
    }

    /// Consume one versioned provider event while enforcing ordering and limits.
    pub fn push(&mut self, event: ChatEvent) -> Result<(), TurnError> {
        if event.contract_version != CHAT_CONTRACT_VERSION {
            return Err(TurnError::UnsupportedContractVersion {
                received: event.contract_version,
            });
        }
        if self.completed {
            return Err(TurnError::EventAfterCompletion);
        }
        match event.event {
            ChatEventKind::TextDelta { text } => {
                self.received_provider_event = true;
                let next_size = self.content_bytes.saturating_add(text.len());
                if next_size > self.settings.max_text_bytes {
                    return Err(TurnError::LimitExceeded("max_text_bytes"));
                }
                self.content_bytes = next_size;
                self.content.push_str(&text);
            }
            ChatEventKind::ToolCallDelta { call } => {
                self.received_provider_event = true;
                self.push_tool_call(call)?;
            }
            ChatEventKind::Usage { usage } => {
                self.received_provider_event = true;
                self.usage = Some(usage);
            }
            ChatEventKind::ModelSwitched { to_model_id, .. } => {
                if self.received_provider_event {
                    return Err(TurnError::ModelSwitchedAfterOutput);
                }
                self.selected_model_id = Some(to_model_id);
            }
            // Reasoning is deliberately not retained in the public turn result.
            ChatEventKind::ReasoningDelta { .. } => self.received_provider_event = true,
            ChatEventKind::Completed { finish_reason } => {
                self.received_provider_event = true;
                self.completed = true;
                self.finish_reason = finish_reason;
            }
        }
        Ok(())
    }

    /// Finish the stream and produce the canonical assistant response.
    pub fn finish(&self) -> Result<ChatCompletion, TurnError> {
        if !self.completed {
            return Err(TurnError::MissingCompletion);
        }
        let mut ids = HashSet::with_capacity(self.tool_calls.len());
        let mut calls = Vec::with_capacity(self.tool_calls.len());
        for (index, fragments) in &self.tool_calls {
            let name = fragments
                .name
                .as_ref()
                .filter(|name| !name.is_empty())
                .ok_or(TurnError::MissingToolName(*index))?;
            let id = fragments
                .id
                .as_ref()
                .filter(|id| !id.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("{}:tool:{index}", self.settings.turn_id));
            if !ids.insert(id.clone()) {
                return Err(TurnError::DuplicateToolCallId);
            }
            let arguments = if fragments.arguments.is_empty() {
                "{}"
            } else {
                fragments.arguments.as_str()
            };
            calls.push(ToolCall {
                id,
                name: name.clone(),
                arguments: ToolArguments::from_json(arguments),
            });
        }
        Ok(ChatCompletion {
            contract_version: CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: self.selected_model_id.clone(),
            content: self.content.clone(),
            reasoning: None,
            tool_calls: calls,
            finish_reason: self.finish_reason.clone(),
            usage: self.usage.clone(),
        })
    }

    fn push_tool_call(&mut self, call: super::types::ToolCallDelta) -> Result<(), TurnError> {
        if !self.tool_calls.contains_key(&call.index)
            && self.tool_calls.len() >= self.settings.max_tool_calls
        {
            return Err(TurnError::LimitExceeded("max_tool_calls"));
        }
        let fragments = self.tool_calls.entry(call.index).or_default();
        if let Some(id) = call.id.filter(|id| !id.is_empty()) {
            if fragments
                .id
                .as_ref()
                .is_some_and(|previous| previous != &id)
            {
                return Err(TurnError::ConflictingToolCallField("id"));
            }
            fragments.id = Some(id);
        }
        if let Some(name) = call.name.filter(|name| !name.is_empty()) {
            if fragments
                .name
                .as_ref()
                .is_some_and(|previous| previous != &name)
            {
                return Err(TurnError::ConflictingToolCallField("name"));
            }
            fragments.name = Some(name);
        }
        let next_size = fragments
            .arguments
            .len()
            .saturating_add(call.arguments_fragment.len());
        if next_size > self.settings.max_tool_argument_bytes {
            return Err(TurnError::LimitExceeded("max_tool_argument_bytes"));
        }
        fragments.arguments.push_str(&call.arguments_fragment);
        Ok(())
    }
}

/// Assemble a complete stream using the same Rust logic exposed by both bindings.
pub fn assemble_turn(
    settings: TurnAccumulatorSettings,
    events: impl IntoIterator<Item = ChatEvent>,
) -> Result<ChatCompletion, TurnError> {
    let mut accumulator = TurnAccumulator::new(settings)?;
    for event in events {
        accumulator.push(event)?;
    }
    accumulator.finish()
}

/// Stable, redacted failures from turn assembly.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum TurnError {
    #[error("turn identifier is required")]
    EmptyTurnId,
    #[error("turn accumulator limit {0} must be greater than zero")]
    InvalidLimit(&'static str),
    #[error("unsupported chat contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("stream emitted an event after completion")]
    EventAfterCompletion,
    #[error("provider switched model after emitting turn output")]
    ModelSwitchedAfterOutput,
    #[error("stream closed without a completion event")]
    MissingCompletion,
    #[error("tool call at index {0} has no function name")]
    MissingToolName(u32),
    #[error("tool call IDs must be unique within a turn")]
    DuplicateToolCallId,
    #[error("provider sent conflicting tool call {0} fragments")]
    ConflictingToolCallField(&'static str),
    #[error("turn accumulator exceeded {0}")]
    LimitExceeded(&'static str),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::chat::{ChatEventKind, ToolCallDelta};

    fn event(event: ChatEventKind) -> ChatEvent {
        ChatEvent {
            contract_version: CHAT_CONTRACT_VERSION,
            event,
        }
    }

    fn settings() -> TurnAccumulatorSettings {
        TurnAccumulatorSettings::new("turn-42")
    }

    #[test]
    fn assembles_interleaved_text_and_fragmented_calls_in_index_order() {
        let completion = assemble_turn(
            settings(),
            [
                event(ChatEventKind::TextDelta {
                    text: "hello ".into(),
                }),
                event(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index: 1,
                        id: None,
                        name: Some("lookup".into()),
                        arguments_fragment: "{\"q\":".into(),
                    },
                }),
                event(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index: 0,
                        id: Some("call-0".into()),
                        name: Some("read".into()),
                        arguments_fragment: "{\"path\":\"a\"}".into(),
                    },
                }),
                event(ChatEventKind::TextDelta {
                    text: "world".into(),
                }),
                event(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index: 1,
                        id: None,
                        name: None,
                        arguments_fragment: "\"b\"}".into(),
                    },
                }),
                event(ChatEventKind::Usage {
                    usage: Usage {
                        input_tokens: 2,
                        output_tokens: 3,
                        total_tokens: 5,
                        cost: None,
                    },
                }),
                event(ChatEventKind::Completed {
                    finish_reason: Some(FinishReason::ToolCalls),
                }),
            ],
        )
        .unwrap();

        assert_eq!(completion.content, "hello world");
        assert_eq!(completion.tool_calls.len(), 2);
        assert_eq!(completion.tool_calls[0].id, "call-0");
        assert_eq!(completion.tool_calls[1].id, "turn-42:tool:1");
        assert_eq!(completion.tool_calls[1].name, "lookup");
        assert_eq!(
            completion.tool_calls[1].arguments,
            ToolArguments::from_json("{\"q\":\"b\"}")
        );
        assert_eq!(completion.usage.unwrap().total_tokens, 5);
    }

    #[test]
    fn preserves_malformed_or_non_object_arguments_without_silently_dropping_call() {
        let completion = assemble_turn(
            settings(),
            [
                event(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index: 0,
                        id: None,
                        name: Some("inspect".into()),
                        arguments_fragment: "[1,2]".into(),
                    },
                }),
                event(ChatEventKind::Completed {
                    finish_reason: None,
                }),
            ],
        )
        .unwrap();
        assert_eq!(
            completion.tool_calls[0].arguments,
            ToolArguments::InvalidJson {
                raw: "[1,2]".into()
            }
        );
    }

    #[test]
    fn reports_terminal_order_and_output_limits() {
        let events = [
            event(ChatEventKind::Completed {
                finish_reason: None,
            }),
            event(ChatEventKind::TextDelta {
                text: "late".into(),
            }),
        ];
        assert_eq!(
            assemble_turn(settings(), events),
            Err(TurnError::EventAfterCompletion)
        );

        let limited = TurnAccumulatorSettings {
            turn_id: "turn-42".into(),
            max_text_bytes: 2,
            ..settings()
        };
        assert!(matches!(
            assemble_turn(
                limited,
                [event(ChatEventKind::TextDelta { text: "abc".into() })]
            ),
            Err(TurnError::LimitExceeded("max_text_bytes"))
        ));
    }

    #[test]
    fn reports_missing_completion_and_tool_call_limits() {
        assert_eq!(
            assemble_turn(settings(), []),
            Err(TurnError::MissingCompletion)
        );
        let limited = TurnAccumulatorSettings {
            turn_id: "turn-42".into(),
            max_tool_calls: 1,
            ..settings()
        };
        assert!(matches!(
            assemble_turn(
                limited,
                [
                    event(ChatEventKind::ToolCallDelta {
                        call: ToolCallDelta {
                            index: 0,
                            id: None,
                            name: Some("one".into()),
                            arguments_fragment: "{}".into(),
                        },
                    }),
                    event(ChatEventKind::ToolCallDelta {
                        call: ToolCallDelta {
                            index: 1,
                            id: None,
                            name: Some("two".into()),
                            arguments_fragment: "{}".into(),
                        },
                    }),
                ]
            ),
            Err(TurnError::LimitExceeded("max_tool_calls"))
        ));
    }

    #[test]
    fn model_switch_must_precede_output_and_reasoning_is_not_retained() {
        let completion = assemble_turn(
            settings(),
            [
                event(ChatEventKind::ModelSwitched {
                    from_model_id: "primary".into(),
                    to_model_id: "fallback".into(),
                }),
                event(ChatEventKind::ReasoningDelta {
                    text: "private reasoning".into(),
                }),
                event(ChatEventKind::Completed {
                    finish_reason: Some(FinishReason::Stop),
                }),
            ],
        )
        .unwrap();
        assert_eq!(completion.selected_model_id.as_deref(), Some("fallback"));
        assert_eq!(completion.reasoning, None);

        let err = assemble_turn(
            settings(),
            [
                event(ChatEventKind::TextDelta {
                    text: "visible".into(),
                }),
                event(ChatEventKind::ModelSwitched {
                    from_model_id: "primary".into(),
                    to_model_id: "fallback".into(),
                }),
            ],
        )
        .unwrap_err();
        assert_eq!(err, TurnError::ModelSwitchedAfterOutput);

        let err = assemble_turn(
            settings(),
            [
                event(ChatEventKind::ReasoningDelta {
                    text: "hidden but delivered".into(),
                }),
                event(ChatEventKind::ModelSwitched {
                    from_model_id: "primary".into(),
                    to_model_id: "fallback".into(),
                }),
            ],
        )
        .unwrap_err();
        assert_eq!(err, TurnError::ModelSwitchedAfterOutput);
    }

    #[test]
    fn invalid_json_arguments_remain_serializable_and_versioned() {
        let completion = assemble_turn(
            settings(),
            [
                event(ChatEventKind::ToolCallDelta {
                    call: ToolCallDelta {
                        index: 0,
                        id: None,
                        name: Some("inspect".into()),
                        arguments_fragment: "{".into(),
                    },
                }),
                event(ChatEventKind::Completed {
                    finish_reason: None,
                }),
            ],
        )
        .unwrap();
        let encoded = serde_json::to_value(completion).unwrap();
        assert_eq!(encoded["contract_version"], json!(CHAT_CONTRACT_VERSION));
        assert_eq!(
            encoded["tool_calls"][0]["arguments"]["state"],
            "invalid_json"
        );
    }
}
