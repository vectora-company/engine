//! Versioned, transport-independent events for conversation execution.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{RunStatus, RunTransition};

/// Version of the public conversation event envelope.
pub const CONVERSATION_EVENT_CONTRACT_VERSION: u16 = 1;
const MAX_JSON_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A terminal outcome reported by the runtime, never inferred from model text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTerminalStatus {
    Completed,
    Degraded,
    Failed,
    Cancelled,
    RecoveryRequired,
}

/// Typed, user-visible or operational event data. Model reasoning is not a
/// supported event payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConversationEventKind {
    TurnQueued {
        turn_id: String,
    },
    TurnAccepted {
        turn_id: String,
    },
    RunStarted {
        run_id: String,
    },
    StepStarted {
        step_id: String,
        capability: String,
    },
    Progress {
        code: String,
    },
    ToolCallStarted {
        step_id: String,
        tool_call_id: String,
        tool_name: String,
    },
    ToolCallFinished {
        step_id: String,
        tool_call_id: String,
        succeeded: bool,
    },
    ApprovalRequested {
        approval_id: String,
    },
    ApprovalResolved {
        approval_id: String,
        approved: bool,
    },
    MessageDelta {
        text: String,
    },
    UsageUpdated {
        input_tokens: u64,
        output_tokens: u64,
        cost_micros: Option<u64>,
    },
    Terminal {
        status: ConversationTerminalStatus,
    },
}

/// The event that reports a run lifecycle transition to conversation readers.
///
/// Leaving `Planned` for `Running` is `RunStarted`. Reaching `Completed`,
/// `Degraded`, `Failed` or `Cancelled` is the single `Terminal` event of the
/// run. `RecoveryRequired` is progress, not terminal, because a run in that
/// state may resume; every other transition is progress with a stable code.
pub fn conversation_event_for_run_transition(transition: &RunTransition) -> ConversationEventKind {
    let terminal = match transition.to {
        RunStatus::Completed => Some(ConversationTerminalStatus::Completed),
        RunStatus::Degraded => Some(ConversationTerminalStatus::Degraded),
        RunStatus::Failed => Some(ConversationTerminalStatus::Failed),
        RunStatus::Cancelled => Some(ConversationTerminalStatus::Cancelled),
        _ => None,
    };
    if let Some(status) = terminal {
        return ConversationEventKind::Terminal { status };
    }
    if transition.from == RunStatus::Planned && transition.to == RunStatus::Running {
        return ConversationEventKind::RunStarted {
            run_id: transition.run_id.clone(),
        };
    }
    let status = serde_json::to_value(transition.to)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    ConversationEventKind::Progress {
        code: format!("run.{status}"),
    }
}

/// A single ordered event in a consumer-owned conversation.
///
/// IDs and sequence numbers are assigned by the trusted event producer. The
/// envelope is a value contract only; it does not persist, authorize, redact,
/// or deliver events.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationEvent {
    pub contract_version: u16,
    pub event_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    pub sequence: u64,
    /// Producer timestamp in Unix epoch milliseconds.
    pub timestamp_ms: u64,
    /// Stable name of the component that produced the event.
    pub source: String,
    pub kind: ConversationEventKind,
}

impl ConversationEvent {
    /// Validate envelope invariants and correlation required by the event kind.
    pub fn validate(&self) -> Result<(), ConversationEventError> {
        if self.contract_version != CONVERSATION_EVENT_CONTRACT_VERSION {
            return Err(ConversationEventError::UnsupportedContractVersion {
                received: self.contract_version,
            });
        }
        for (field, value) in [
            ("event_id", self.event_id.as_str()),
            ("conversation_id", self.conversation_id.as_str()),
            ("source", self.source.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ConversationEventError::EmptyField(field));
            }
        }
        for (field, value) in [
            ("turn_id", self.turn_id.as_deref()),
            ("run_id", self.run_id.as_deref()),
            ("step_id", self.step_id.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(ConversationEventError::EmptyField(field));
            }
        }
        if self.sequence == 0 {
            return Err(ConversationEventError::InvalidSequence);
        }
        for (field, value) in [
            ("sequence", self.sequence),
            ("timestamp_ms", self.timestamp_ms),
        ] {
            if value > MAX_JSON_SAFE_INTEGER {
                return Err(ConversationEventError::IntegerOutOfRange(field));
            }
        }

        let required = match &self.kind {
            ConversationEventKind::TurnQueued { turn_id }
            | ConversationEventKind::TurnAccepted { turn_id } => {
                require_nonempty("turn_id", turn_id)?;
                require_correlation("turn_id", self.turn_id.as_deref(), turn_id)?;
                RequiredCorrelation::Turn
            }
            ConversationEventKind::RunStarted { run_id } => {
                require_nonempty("run_id", run_id)?;
                require_correlation("run_id", self.run_id.as_deref(), run_id)?;
                RequiredCorrelation::Run
            }
            ConversationEventKind::StepStarted {
                step_id,
                capability,
            } => {
                require_nonempty("step_id", step_id)?;
                require_nonempty("capability", capability)?;
                require_correlation("step_id", self.step_id.as_deref(), step_id)?;
                RequiredCorrelation::Step
            }
            ConversationEventKind::Progress { code } => {
                require_nonempty("code", code)?;
                RequiredCorrelation::Run
            }
            ConversationEventKind::ToolCallStarted {
                step_id,
                tool_call_id,
                tool_name,
            } => {
                require_nonempty("step_id", step_id)?;
                require_nonempty("tool_call_id", tool_call_id)?;
                require_nonempty("tool_name", tool_name)?;
                require_correlation("step_id", self.step_id.as_deref(), step_id)?;
                RequiredCorrelation::Step
            }
            ConversationEventKind::ToolCallFinished {
                step_id,
                tool_call_id,
                ..
            } => {
                require_nonempty("step_id", step_id)?;
                require_nonempty("tool_call_id", tool_call_id)?;
                require_correlation("step_id", self.step_id.as_deref(), step_id)?;
                RequiredCorrelation::Step
            }
            ConversationEventKind::ApprovalRequested { approval_id }
            | ConversationEventKind::ApprovalResolved { approval_id, .. } => {
                require_nonempty("approval_id", approval_id)?;
                if self.run_id.as_deref().is_none_or(str::is_empty) {
                    return Err(ConversationEventError::MissingCorrelation("run_id"));
                }
                RequiredCorrelation::Run
            }
            ConversationEventKind::MessageDelta { .. } | ConversationEventKind::Terminal { .. } => {
                RequiredCorrelation::Run
            }
            ConversationEventKind::UsageUpdated {
                input_tokens,
                output_tokens,
                cost_micros,
            } => {
                for (field, value) in [
                    ("input_tokens", *input_tokens),
                    ("output_tokens", *output_tokens),
                    ("cost_micros", cost_micros.unwrap_or(0)),
                ] {
                    if value > MAX_JSON_SAFE_INTEGER {
                        return Err(ConversationEventError::IntegerOutOfRange(field));
                    }
                }
                RequiredCorrelation::Run
            }
        };

        match required {
            RequiredCorrelation::Turn if self.turn_id.as_deref().is_none_or(str::is_empty) => {
                return Err(ConversationEventError::MissingCorrelation("turn_id"));
            }
            RequiredCorrelation::Run if self.run_id.as_deref().is_none_or(str::is_empty) => {
                return Err(ConversationEventError::MissingCorrelation("run_id"));
            }
            RequiredCorrelation::Step if self.step_id.as_deref().is_none_or(str::is_empty) => {
                return Err(ConversationEventError::MissingCorrelation("step_id"));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum RequiredCorrelation {
    Turn,
    Run,
    Step,
}

fn require_nonempty(field: &'static str, value: &str) -> Result<(), ConversationEventError> {
    if value.trim().is_empty() {
        Err(ConversationEventError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn require_correlation(
    field: &'static str,
    envelope_value: Option<&str>,
    payload_value: &str,
) -> Result<(), ConversationEventError> {
    match envelope_value {
        None | Some("") => Err(ConversationEventError::MissingCorrelation(field)),
        Some(value) if value != payload_value => {
            Err(ConversationEventError::MismatchedCorrelation(field))
        }
        Some(_) => Ok(()),
    }
}

/// Validation errors for the versioned conversation event contract.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ConversationEventError {
    #[error("unsupported conversation event contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("conversation event field {0} must not be empty")]
    EmptyField(&'static str),
    #[error("conversation event sequence must start at one")]
    InvalidSequence,
    #[error("conversation event is missing required correlation field {0}")]
    MissingCorrelation(&'static str),
    #[error("conversation event correlation field {0} does not match its payload")]
    MismatchedCorrelation(&'static str),
    #[error("conversation event integer field {0} exceeds cross-language safe range")]
    IntegerOutOfRange(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: ConversationEventKind) -> ConversationEvent {
        ConversationEvent {
            contract_version: CONVERSATION_EVENT_CONTRACT_VERSION,
            event_id: "evt-1".into(),
            conversation_id: "conversation-1".into(),
            turn_id: Some("turn-1".into()),
            run_id: Some("run-1".into()),
            step_id: Some("step-1".into()),
            sequence: 1,
            timestamp_ms: 1_700_000_000_000,
            source: "engine.harness".into(),
            kind,
        }
    }

    #[test]
    fn validates_and_roundtrips_versioned_event_envelope() {
        let event = event(ConversationEventKind::MessageDelta {
            text: "visible answer".into(),
        });
        event.validate().unwrap();
        let encoded = serde_json::to_string(&event).unwrap();
        assert_eq!(
            serde_json::from_str::<ConversationEvent>(&encoded).unwrap(),
            event
        );
    }

    #[test]
    fn requires_correlation_for_turn_run_and_step_events() {
        let mut queued = event(ConversationEventKind::TurnQueued {
            turn_id: "turn-1".into(),
        });
        queued.turn_id = None;
        assert_eq!(
            queued.validate(),
            Err(ConversationEventError::MissingCorrelation("turn_id"))
        );

        let mut terminal = event(ConversationEventKind::Terminal {
            status: ConversationTerminalStatus::Failed,
        });
        terminal.run_id = None;
        assert_eq!(
            terminal.validate(),
            Err(ConversationEventError::MissingCorrelation("run_id"))
        );

        let mut tool_call = event(ConversationEventKind::ToolCallStarted {
            step_id: "step-1".into(),
            tool_call_id: "call-1".into(),
            tool_name: "filesystem.read".into(),
        });
        tool_call.step_id = None;
        assert_eq!(
            tool_call.validate(),
            Err(ConversationEventError::MissingCorrelation("step_id"))
        );
    }

    #[test]
    fn rejects_unknown_versions_empty_ids_and_zero_sequence() {
        let mut event = event(ConversationEventKind::Terminal {
            status: ConversationTerminalStatus::Completed,
        });
        event.contract_version += 1;
        assert!(matches!(
            event.validate(),
            Err(ConversationEventError::UnsupportedContractVersion { .. })
        ));
        event.contract_version = CONVERSATION_EVENT_CONTRACT_VERSION;
        event.sequence = 0;
        assert_eq!(
            event.validate(),
            Err(ConversationEventError::InvalidSequence)
        );
    }

    #[test]
    fn rejects_payload_correlation_that_disagrees_with_envelope() {
        let mut event = event(ConversationEventKind::TurnAccepted {
            turn_id: "turn-other".into(),
        });
        assert_eq!(
            event.validate(),
            Err(ConversationEventError::MismatchedCorrelation("turn_id"))
        );
        event.turn_id = None;
        assert_eq!(
            event.validate(),
            Err(ConversationEventError::MissingCorrelation("turn_id"))
        );
    }

    #[test]
    fn rejects_whitespace_only_optional_correlation() {
        let mut event = event(ConversationEventKind::Progress {
            code: "provider.retry".into(),
        });
        event.step_id = Some("  ".into());
        assert_eq!(
            event.validate(),
            Err(ConversationEventError::EmptyField("step_id"))
        );
    }

    #[test]
    fn rejects_integers_that_cannot_roundtrip_through_javascript_numbers() {
        let mut event = event(ConversationEventKind::Progress {
            code: "provider.retry".into(),
        });
        event.sequence = 9_007_199_254_740_992;
        assert_eq!(
            event.validate(),
            Err(ConversationEventError::IntegerOutOfRange("sequence"))
        );
    }

    fn transition(from: RunStatus, to: RunStatus) -> RunTransition {
        RunTransition {
            contract_version: crate::harness::RUN_CONTRACT_VERSION,
            run_id: "run-1".into(),
            sequence: 1,
            from,
            to,
        }
    }

    #[test]
    fn run_transitions_map_to_one_terminal_and_stable_progress_codes() {
        assert_eq!(
            conversation_event_for_run_transition(&transition(
                RunStatus::Planned,
                RunStatus::Running
            )),
            ConversationEventKind::RunStarted {
                run_id: "run-1".into()
            }
        );
        for (to, status) in [
            (RunStatus::Completed, ConversationTerminalStatus::Completed),
            (RunStatus::Degraded, ConversationTerminalStatus::Degraded),
            (RunStatus::Failed, ConversationTerminalStatus::Failed),
            (RunStatus::Cancelled, ConversationTerminalStatus::Cancelled),
        ] {
            assert_eq!(
                conversation_event_for_run_transition(&transition(RunStatus::Running, to)),
                ConversationEventKind::Terminal { status }
            );
        }
        for (to, code) in [
            (RunStatus::WaitingApproval, "run.waiting_approval"),
            (RunStatus::ToolRunning, "run.tool_running"),
            (RunStatus::RecoveryRequired, "run.recovery_required"),
        ] {
            assert_eq!(
                conversation_event_for_run_transition(&transition(RunStatus::Running, to)),
                ConversationEventKind::Progress { code: code.into() }
            );
        }
        assert_eq!(
            conversation_event_for_run_transition(&transition(
                RunStatus::RecoveryRequired,
                RunStatus::Running
            )),
            ConversationEventKind::Progress {
                code: "run.running".into()
            }
        );
    }
}
