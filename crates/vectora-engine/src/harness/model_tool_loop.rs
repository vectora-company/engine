//! In-memory model/tool round state machine shared by engine consumers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::chat::{
    CHAT_CONTRACT_VERSION, ChatCompletion, ContentPart, MediaPart, MediaRef, Message,
    MessageContent, MessageRole,
};

use super::{
    TOOL_DISPATCH_CONTRACT_VERSION, ToolDecision, ToolDenialCode, ToolDispatch, ToolEffect,
    ToolExecutionMode,
};

/// Version of the model/tool round state machine.
pub const MODEL_TOOL_LOOP_CONTRACT_VERSION: u16 = 3;
pub const TOOL_RESULT_CONTRACT_VERSION: u16 = 2;
const DEFAULT_MAX_MODEL_TURNS: u32 = 50;
const DEFAULT_MAX_TOOL_CALLS: u32 = 128;
const DEFAULT_MAX_RESULT_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_RESULTS_BYTES_PER_TURN: usize = 4 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;

/// Budgets for one non-durable model/tool loop.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelToolLoopSettings {
    #[serde(default = "default_max_model_turns")]
    pub max_model_turns: u32,
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: u32,
    #[serde(default = "default_max_result_bytes")]
    pub max_tool_result_bytes: usize,
    #[serde(default = "default_max_results_bytes_per_turn")]
    pub max_tool_results_bytes_per_turn: usize,
}

const fn default_max_model_turns() -> u32 {
    DEFAULT_MAX_MODEL_TURNS
}

const fn default_max_tool_calls() -> u32 {
    DEFAULT_MAX_TOOL_CALLS
}

const fn default_max_result_bytes() -> usize {
    DEFAULT_MAX_RESULT_BYTES
}

const fn default_max_results_bytes_per_turn() -> usize {
    DEFAULT_MAX_RESULTS_BYTES_PER_TURN
}

impl Default for ModelToolLoopSettings {
    fn default() -> Self {
        Self {
            max_model_turns: default_max_model_turns(),
            max_tool_calls: default_max_tool_calls(),
            max_tool_result_bytes: default_max_result_bytes(),
            max_tool_results_bytes_per_turn: default_max_results_bytes_per_turn(),
        }
    }
}

impl ModelToolLoopSettings {
    fn validate(&self) -> Result<(), ModelToolLoopError> {
        if self.max_model_turns == 0 {
            return Err(ModelToolLoopError::InvalidLimit("max_model_turns"));
        }
        if self.max_tool_calls == 0 {
            return Err(ModelToolLoopError::InvalidLimit("max_tool_calls"));
        }
        if self.max_tool_result_bytes == 0 || self.max_tool_result_bytes > MAX_RESULT_BYTES {
            return Err(ModelToolLoopError::InvalidLimit("max_tool_result_bytes"));
        }
        if self.max_tool_results_bytes_per_turn == 0
            || self.max_tool_results_bytes_per_turn > MAX_RESULT_BYTES
        {
            return Err(ModelToolLoopError::InvalidLimit(
                "max_tool_results_bytes_per_turn",
            ));
        }
        Ok(())
    }
}

/// Current in-memory phase of one model/tool loop.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelToolLoopPhase {
    AwaitingModel,
    AwaitingToolResults,
    Cancelling,
    Completed,
    Stopped,
    Cancelled,
    RecoveryRequired,
}

/// Typed status reported by a tool adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatus {
    Succeeded,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
}

/// What the adapter knows about an external effect when it returns.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffectState {
    NotApplicable,
    NotApplied,
    Applied,
    Unknown,
}

/// Correlated, size-bounded result returned by a consumer tool adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolResult {
    pub contract_version: u16,
    pub run_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub status: ToolResultStatus,
    pub effect_state: ToolEffectState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Ordered, consumer-redacted content supplied to the next model turn.
    pub parts: Vec<ToolResultPart>,
}

/// One typed part of a tool result. Images remain references to consumer-owned
/// assets; their bytes are loaded only by the media execution context.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultPart {
    Text { text: String },
    Image { media_ref: MediaRef },
}

impl ToolResult {
    /// Converts this result into the correlated chat message used for the next
    /// model turn. Provider adapters decide how to encode its typed parts.
    pub fn to_chat_message(&self) -> Message {
        let content = self
            .parts
            .iter()
            .map(|part| match part {
                ToolResultPart::Text { text } => ContentPart::Text { text: text.clone() },
                ToolResultPart::Image { media_ref } => ContentPart::Image(MediaPart {
                    media_ref: media_ref.clone(),
                    text: None,
                    previews: Vec::new(),
                }),
            })
            .collect();
        Message {
            role: MessageRole::Tool,
            content: MessageContent::Parts(content),
            tool_calls: Vec::new(),
            tool_call_id: Some(self.call_id.clone()),
            name: Some(self.tool_name.clone()),
            is_error: self.status != ToolResultStatus::Succeeded,
        }
    }
}

fn result_size_bytes(result: &ToolResult) -> Option<usize> {
    serde_json::to_vec(&result.parts)
        .ok()
        .map(|encoded| encoded.len())
}

/// Expected invocation and optional result kept while adapters are running.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingToolResult {
    pub call_id: String,
    pub tool_name: String,
    pub effect: ToolEffect,
    pub result: Option<ToolResult>,
}

/// Serializable state snapshot. It is not durable or safe to trust after reload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ModelToolLoopState {
    pub contract_version: u16,
    pub run_id: String,
    pub turn_id: String,
    pub settings: ModelToolLoopSettings,
    pub phase: ModelToolLoopPhase,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancellation_requested: bool,
    pub model_turns: u32,
    pub tool_calls: u32,
    pub pending_completion: Option<ChatCompletion>,
    pub pending_results: Vec<PendingToolResult>,
}

/// New state and host action produced by one loop input.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ModelToolLoopTransition {
    pub state: ModelToolLoopState,
    pub action: ModelToolLoopAction,
}

impl ModelToolLoopState {
    /// Begin a loop associated with the host's existing run and turn IDs.
    pub fn new(
        run_id: impl Into<String>,
        turn_id: impl Into<String>,
        settings: ModelToolLoopSettings,
    ) -> Result<Self, ModelToolLoopError> {
        settings.validate()?;
        let run_id = run_id.into();
        let turn_id = turn_id.into();
        if run_id.trim().is_empty() || turn_id.trim().is_empty() {
            return Err(ModelToolLoopError::EmptyIdentity);
        }
        Ok(Self {
            contract_version: MODEL_TOOL_LOOP_CONTRACT_VERSION,
            run_id,
            turn_id,
            settings,
            phase: ModelToolLoopPhase::AwaitingModel,
            cancellation_requested: false,
            model_turns: 0,
            tool_calls: 0,
            pending_completion: None,
            pending_results: Vec::new(),
        })
    }
}

/// Action for the consumer after advancing the shared loop.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ModelToolLoopAction {
    ExecuteTools {
        assistant_completion: ChatCompletion,
        execution_mode: ToolExecutionMode,
        invocations: Vec<ToolDecision>,
        immediate_results: Vec<ToolResult>,
    },
    AwaitingToolResults {
        remaining: usize,
    },
    ContinueWithToolResults {
        assistant_completion: ChatCompletion,
        results: Vec<ToolResult>,
        next_model_turn: u32,
    },
    Complete {
        assistant_completion: ChatCompletion,
        model_turns: u32,
        tool_calls: u32,
    },
    Stopped {
        reason: ModelToolLoopStopReason,
        model_turns: u32,
        tool_calls: u32,
    },
    RecoveryRequired {
        call_id: String,
        tool_name: String,
    },
    CancelPendingTools {
        calls: Vec<PendingToolCancellation>,
    },
    Cancelled {
        model_turns: u32,
        tool_calls: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingToolCancellation {
    pub call_id: String,
    pub tool_name: String,
}

/// Stable terminal reason when a loop budget is reached.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelToolLoopStopReason {
    ModelTurnLimit,
    ToolCallLimit,
}

/// Validation errors that leave the supplied loop snapshot unchanged.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelToolLoopError {
    #[error("run and turn identifiers are required")]
    EmptyIdentity,
    #[error("invalid loop limit {0}")]
    InvalidLimit(&'static str),
    #[error("unsupported model/tool loop contract version {received}")]
    UnsupportedLoopContractVersion { received: u16 },
    #[error("model/tool loop is not awaiting a model turn")]
    NotAwaitingModel,
    #[error("model/tool loop is not awaiting tool results")]
    NotAwaitingToolResults,
    #[error("unsupported chat contract version {received}")]
    UnsupportedChatContractVersion { received: u16 },
    #[error("unsupported tool dispatch contract version {received}")]
    UnsupportedDispatchContractVersion { received: u16 },
    #[error("dispatch identity does not match the active run and turn")]
    DispatchIdentityMismatch,
    #[error("tool dispatch does not exactly match the model tool calls")]
    DispatchMismatch,
    #[error("tool result contract version {received} is unsupported")]
    UnsupportedToolResultContractVersion { received: u16 },
    #[error("tool result identity does not match the active run and turn")]
    ResultIdentityMismatch,
    #[error("tool result call ID is unknown or already completed")]
    UnexpectedToolResult,
    #[error("tool result name does not match its invocation")]
    ResultToolMismatch,
    #[error("tool result output exceeds configured byte limit")]
    ToolResultTooLarge,
    #[error("tool result has inconsistent status, code, or effect state")]
    InvalidToolResult,
    #[error("tool results exceed the configured turn byte limit")]
    ToolResultsTooLarge,
    #[error("loop snapshot contains inconsistent pending state")]
    InvalidState,
    #[error("model/tool loop has no pending tool invocation to recover")]
    NotRecoverable,
    #[error("model/tool loop is already terminal")]
    AlreadyTerminal,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// Consume a model completion and an already-authorized dispatch decision.
pub fn accept_model_turn(
    state: &ModelToolLoopState,
    completion: ChatCompletion,
    dispatch: ToolDispatch,
) -> Result<(ModelToolLoopState, ModelToolLoopAction), ModelToolLoopError> {
    validate_state(state)?;
    if state.phase != ModelToolLoopPhase::AwaitingModel {
        return Err(ModelToolLoopError::NotAwaitingModel);
    }
    if completion.contract_version != CHAT_CONTRACT_VERSION {
        return Err(ModelToolLoopError::UnsupportedChatContractVersion {
            received: completion.contract_version,
        });
    }
    if dispatch.contract_version != TOOL_DISPATCH_CONTRACT_VERSION {
        return Err(ModelToolLoopError::UnsupportedDispatchContractVersion {
            received: dispatch.contract_version,
        });
    }
    if dispatch.run_id != state.run_id
        || dispatch.turn_id != state.turn_id
        || dispatch.decisions.len() != completion.tool_calls.len()
    {
        return Err(ModelToolLoopError::DispatchIdentityMismatch);
    }
    for (call, decision) in completion.tool_calls.iter().zip(&dispatch.decisions) {
        let matches = match decision {
            ToolDecision::Invoke {
                call: authorized, ..
            } => authorized == call,
            ToolDecision::Deny {
                call_id, tool_name, ..
            } => call_id == &call.id && tool_name == &call.name,
        };
        if !matches {
            return Err(ModelToolLoopError::DispatchMismatch);
        }
    }
    let expected_mode = if dispatch.decisions.iter().any(|decision| {
        matches!(
            decision,
            ToolDecision::Invoke {
                effect: ToolEffect::ExternalEffect | ToolEffect::Destructive,
                ..
            }
        )
    }) {
        ToolExecutionMode::Sequential
    } else {
        ToolExecutionMode::ConcurrentReadOnly
    };
    if !dispatch.decisions.is_empty() && dispatch.execution_mode != expected_mode {
        return Err(ModelToolLoopError::DispatchMismatch);
    }

    let mut next = state.clone();
    next.model_turns = next
        .model_turns
        .checked_add(1)
        .ok_or(ModelToolLoopError::InvalidState)?;
    let mut assistant_completion = completion;
    // Reasoning is internal provider output and never enters loop state/events.
    assistant_completion.reasoning = None;

    if assistant_completion.tool_calls.is_empty() {
        next.phase = ModelToolLoopPhase::Completed;
        return Ok((
            next.clone(),
            ModelToolLoopAction::Complete {
                assistant_completion,
                model_turns: next.model_turns,
                tool_calls: next.tool_calls,
            },
        ));
    }

    if next.model_turns >= next.settings.max_model_turns {
        next.phase = ModelToolLoopPhase::Stopped;
        return Ok((
            next.clone(),
            ModelToolLoopAction::Stopped {
                reason: ModelToolLoopStopReason::ModelTurnLimit,
                model_turns: next.model_turns,
                tool_calls: next.tool_calls,
            },
        ));
    }
    let call_count = u32::try_from(assistant_completion.tool_calls.len())
        .map_err(|_| ModelToolLoopError::InvalidState)?;
    let total_calls = next
        .tool_calls
        .checked_add(call_count)
        .ok_or(ModelToolLoopError::InvalidState)?;
    if total_calls > next.settings.max_tool_calls {
        next.phase = ModelToolLoopPhase::Stopped;
        return Ok((
            next.clone(),
            ModelToolLoopAction::Stopped {
                reason: ModelToolLoopStopReason::ToolCallLimit,
                model_turns: next.model_turns,
                tool_calls: next.tool_calls,
            },
        ));
    }
    next.tool_calls = total_calls;

    let mut invocations = Vec::new();
    let mut immediate_results = Vec::new();
    next.pending_results.clear();
    for decision in dispatch.decisions {
        match decision {
            ToolDecision::Invoke {
                call,
                schema_version,
                required_capability,
                timeout_ms,
                effect,
                idempotency,
                idempotency_key,
            } => {
                next.pending_results.push(PendingToolResult {
                    call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                    effect,
                    result: None,
                });
                invocations.push(ToolDecision::Invoke {
                    call,
                    schema_version,
                    required_capability,
                    timeout_ms,
                    effect,
                    idempotency,
                    idempotency_key,
                });
            }
            ToolDecision::Deny {
                call_id,
                tool_name,
                code,
            } => {
                let result = denied_result(&next.run_id, &next.turn_id, call_id, tool_name, code);
                next.pending_results.push(PendingToolResult {
                    call_id: result.call_id.clone(),
                    tool_name: result.tool_name.clone(),
                    effect: ToolEffect::ReadOnly,
                    result: Some(result.clone()),
                });
                immediate_results.push(result);
            }
        }
    }

    if invocations.is_empty() {
        let results = ordered_results(&next.pending_results)?;
        next.pending_results.clear();
        next.phase = ModelToolLoopPhase::AwaitingModel;
        return Ok((
            next.clone(),
            ModelToolLoopAction::ContinueWithToolResults {
                assistant_completion,
                results,
                next_model_turn: next.model_turns + 1,
            },
        ));
    }

    next.pending_completion = Some(assistant_completion.clone());
    next.phase = ModelToolLoopPhase::AwaitingToolResults;
    Ok((
        next,
        ModelToolLoopAction::ExecuteTools {
            assistant_completion,
            execution_mode: dispatch.execution_mode,
            invocations,
            immediate_results,
        },
    ))
}

/// Accept one adapter result; emit ordered results when the batch is complete.
pub fn submit_tool_result(
    state: &ModelToolLoopState,
    result: ToolResult,
) -> Result<(ModelToolLoopState, ModelToolLoopAction), ModelToolLoopError> {
    validate_state(state)?;
    if !matches!(
        state.phase,
        ModelToolLoopPhase::AwaitingToolResults | ModelToolLoopPhase::Cancelling
    ) {
        return Err(ModelToolLoopError::NotAwaitingToolResults);
    }
    if result.contract_version != TOOL_RESULT_CONTRACT_VERSION {
        return Err(ModelToolLoopError::UnsupportedToolResultContractVersion {
            received: result.contract_version,
        });
    }
    if result.run_id != state.run_id || result.turn_id != state.turn_id {
        return Err(ModelToolLoopError::ResultIdentityMismatch);
    }
    let result_size = result_size_bytes(&result).ok_or(ModelToolLoopError::ToolResultTooLarge)?;
    if result_size > state.settings.max_tool_result_bytes {
        return Err(ModelToolLoopError::ToolResultTooLarge);
    }
    validate_result_shape(&result)?;

    let mut next = state.clone();
    let pending = next
        .pending_results
        .iter_mut()
        .find(|pending| pending.call_id == result.call_id)
        .ok_or(ModelToolLoopError::UnexpectedToolResult)?;
    if pending.result.is_some() {
        return Err(ModelToolLoopError::UnexpectedToolResult);
    }
    if pending.tool_name != result.tool_name {
        return Err(ModelToolLoopError::ResultToolMismatch);
    }
    validate_effect_state(pending.effect, result.status, result.effect_state)?;
    pending.result = Some(result.clone());

    if result.effect_state == ToolEffectState::Unknown {
        next.phase = ModelToolLoopPhase::RecoveryRequired;
        return Ok((
            next,
            ModelToolLoopAction::RecoveryRequired {
                call_id: result.call_id,
                tool_name: result.tool_name,
            },
        ));
    }
    let all_results_present = next
        .pending_results
        .iter()
        .all(|item| item.result.is_some());
    if !all_results_present {
        let remaining = next
            .pending_results
            .iter()
            .filter(|item| item.result.is_none())
            .count();
        return Ok((next, ModelToolLoopAction::AwaitingToolResults { remaining }));
    }

    let result_bytes = next
        .pending_results
        .iter()
        .filter_map(|item| item.result.as_ref())
        .map(result_size_bytes)
        .collect::<Option<Vec<_>>>()
        .ok_or(ModelToolLoopError::ToolResultsTooLarge)?
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(ModelToolLoopError::ToolResultsTooLarge)?;
    if result_bytes > next.settings.max_tool_results_bytes_per_turn {
        return Err(ModelToolLoopError::ToolResultsTooLarge);
    }
    if next.cancellation_requested {
        let transition = cancelled_transition(next);
        return Ok((transition.state, transition.action));
    }
    let results = ordered_results(&next.pending_results)?;
    let assistant_completion = next
        .pending_completion
        .take()
        .ok_or(ModelToolLoopError::InvalidState)?;
    next.pending_results.clear();
    next.phase = ModelToolLoopPhase::AwaitingModel;
    Ok((
        next.clone(),
        ModelToolLoopAction::ContinueWithToolResults {
            assistant_completion,
            results,
            next_model_turn: next.model_turns + 1,
        },
    ))
}

/// Convenience wrapper that packages the new state with its next action.
pub fn advance_model_turn(
    state: &ModelToolLoopState,
    completion: ChatCompletion,
    dispatch: ToolDispatch,
) -> Result<ModelToolLoopTransition, ModelToolLoopError> {
    let (state, action) = accept_model_turn(state, completion, dispatch)?;
    Ok(ModelToolLoopTransition { state, action })
}

/// Convenience wrapper that packages the new state with its next action.
pub fn advance_tool_result(
    state: &ModelToolLoopState,
    result: ToolResult,
) -> Result<ModelToolLoopTransition, ModelToolLoopError> {
    let (state, action) = submit_tool_result(state, result)?;
    Ok(ModelToolLoopTransition { state, action })
}

/// Require explicit reconciliation for every unresolved invocation after a restart.
/// This function never returns an action that re-executes a tool.
pub fn recover_model_tool_loop(
    state: &ModelToolLoopState,
) -> Result<ModelToolLoopTransition, ModelToolLoopError> {
    validate_state(state)?;
    let mut next = state.clone();
    match next.phase {
        ModelToolLoopPhase::AwaitingToolResults | ModelToolLoopPhase::Cancelling => {
            next.phase = ModelToolLoopPhase::RecoveryRequired;
        }
        ModelToolLoopPhase::RecoveryRequired => {}
        _ => return Err(ModelToolLoopError::NotRecoverable),
    }
    let action = recovery_action(&next)?;
    Ok(ModelToolLoopTransition {
        state: next,
        action,
    })
}

/// Request cancellation of the current model/tool loop.
///
/// If adapters may be running, the host receives their identities to cancel.
/// The loop remains pending until their outcomes are known. Unknown effects
/// require reconciliation before the terminal cancelled state is recorded.
pub fn cancel_model_tool_loop(
    state: &ModelToolLoopState,
) -> Result<ModelToolLoopTransition, ModelToolLoopError> {
    validate_state(state)?;
    let mut next = state.clone();
    match next.phase {
        ModelToolLoopPhase::AwaitingModel => {
            next.contract_version = MODEL_TOOL_LOOP_CONTRACT_VERSION;
            next.cancellation_requested = true;
            next.phase = ModelToolLoopPhase::Cancelled;
            Ok(cancelled_transition(next))
        }
        ModelToolLoopPhase::AwaitingToolResults => {
            next.contract_version = MODEL_TOOL_LOOP_CONTRACT_VERSION;
            next.cancellation_requested = true;
            next.phase = ModelToolLoopPhase::Cancelling;
            Ok(cancel_pending_tools_transition(next))
        }
        ModelToolLoopPhase::Cancelling => Ok(cancel_pending_tools_transition(next)),
        ModelToolLoopPhase::RecoveryRequired => {
            next.contract_version = MODEL_TOOL_LOOP_CONTRACT_VERSION;
            next.cancellation_requested = true;
            let action = recovery_action(&next)?;
            Ok(ModelToolLoopTransition {
                state: next,
                action,
            })
        }
        ModelToolLoopPhase::Cancelled => Ok(cancelled_transition(next)),
        ModelToolLoopPhase::Completed | ModelToolLoopPhase::Stopped => {
            Err(ModelToolLoopError::AlreadyTerminal)
        }
    }
}

fn cancel_pending_tools_transition(state: ModelToolLoopState) -> ModelToolLoopTransition {
    let calls = state
        .pending_results
        .iter()
        .filter(|pending| pending.result.is_none())
        .map(|pending| PendingToolCancellation {
            call_id: pending.call_id.clone(),
            tool_name: pending.tool_name.clone(),
        })
        .collect();
    ModelToolLoopTransition {
        state,
        action: ModelToolLoopAction::CancelPendingTools { calls },
    }
}

fn cancelled_transition(mut state: ModelToolLoopState) -> ModelToolLoopTransition {
    state.phase = ModelToolLoopPhase::Cancelled;
    state.cancellation_requested = true;
    state.pending_completion = None;
    state.pending_results.clear();
    ModelToolLoopTransition {
        action: ModelToolLoopAction::Cancelled {
            model_turns: state.model_turns,
            tool_calls: state.tool_calls,
        },
        state,
    }
}

/// Record a host's explicit reconciliation of an invocation after interruption.
/// The host is responsible for establishing whether the external effect occurred.
pub fn reconcile_tool_result(
    state: &ModelToolLoopState,
    result: ToolResult,
) -> Result<ModelToolLoopTransition, ModelToolLoopError> {
    validate_state(state)?;
    if state.phase != ModelToolLoopPhase::RecoveryRequired {
        return Err(ModelToolLoopError::NotRecoverable);
    }
    if result.contract_version != TOOL_RESULT_CONTRACT_VERSION {
        return Err(ModelToolLoopError::UnsupportedToolResultContractVersion {
            received: result.contract_version,
        });
    }
    if result.run_id != state.run_id || result.turn_id != state.turn_id {
        return Err(ModelToolLoopError::ResultIdentityMismatch);
    }
    let result_size = result_size_bytes(&result).ok_or(ModelToolLoopError::ToolResultTooLarge)?;
    if result_size > state.settings.max_tool_result_bytes {
        return Err(ModelToolLoopError::ToolResultTooLarge);
    }
    validate_result_shape(&result)?;
    if result.effect_state == ToolEffectState::Unknown {
        return Err(ModelToolLoopError::InvalidToolResult);
    }

    let mut next = state.clone();
    let pending = next
        .pending_results
        .iter_mut()
        .find(|pending| pending.call_id == result.call_id)
        .ok_or(ModelToolLoopError::UnexpectedToolResult)?;
    if pending.tool_name != result.tool_name {
        return Err(ModelToolLoopError::ResultToolMismatch);
    }
    if pending
        .result
        .as_ref()
        .is_some_and(|prior| prior.effect_state != ToolEffectState::Unknown)
    {
        return Err(ModelToolLoopError::UnexpectedToolResult);
    }
    validate_effect_state(pending.effect, result.status, result.effect_state)?;
    pending.result = Some(result);

    if has_unresolved_invocations(&next) {
        let action = recovery_action(&next)?;
        return Ok(ModelToolLoopTransition {
            state: next,
            action,
        });
    }

    let result_bytes = next
        .pending_results
        .iter()
        .filter_map(|item| item.result.as_ref())
        .map(result_size_bytes)
        .collect::<Option<Vec<_>>>()
        .ok_or(ModelToolLoopError::ToolResultsTooLarge)?
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(ModelToolLoopError::ToolResultsTooLarge)?;
    if result_bytes > next.settings.max_tool_results_bytes_per_turn {
        return Err(ModelToolLoopError::ToolResultsTooLarge);
    }
    if next.cancellation_requested {
        return Ok(cancelled_transition(next));
    }
    let results = ordered_results(&next.pending_results)?;
    let assistant_completion = next
        .pending_completion
        .take()
        .ok_or(ModelToolLoopError::InvalidState)?;
    next.pending_results.clear();
    next.phase = ModelToolLoopPhase::AwaitingModel;
    let action = ModelToolLoopAction::ContinueWithToolResults {
        assistant_completion,
        results,
        next_model_turn: next.model_turns + 1,
    };
    Ok(ModelToolLoopTransition {
        state: next,
        action,
    })
}

fn has_unresolved_invocations(state: &ModelToolLoopState) -> bool {
    state.pending_results.iter().any(|pending| {
        pending
            .result
            .as_ref()
            .is_none_or(|result| result.effect_state == ToolEffectState::Unknown)
    })
}

fn recovery_action(state: &ModelToolLoopState) -> Result<ModelToolLoopAction, ModelToolLoopError> {
    let pending = state
        .pending_results
        .iter()
        .find(|pending| {
            pending
                .result
                .as_ref()
                .is_none_or(|result| result.effect_state == ToolEffectState::Unknown)
        })
        .ok_or(ModelToolLoopError::InvalidState)?;
    Ok(ModelToolLoopAction::RecoveryRequired {
        call_id: pending.call_id.clone(),
        tool_name: pending.tool_name.clone(),
    })
}

fn validate_state(state: &ModelToolLoopState) -> Result<(), ModelToolLoopError> {
    if state.contract_version != MODEL_TOOL_LOOP_CONTRACT_VERSION {
        return Err(ModelToolLoopError::UnsupportedLoopContractVersion {
            received: state.contract_version,
        });
    }
    state.settings.validate()?;
    if state.run_id.trim().is_empty() || state.turn_id.trim().is_empty() {
        return Err(ModelToolLoopError::EmptyIdentity);
    }
    let cancellation_phase = matches!(
        state.phase,
        ModelToolLoopPhase::Cancelling | ModelToolLoopPhase::Cancelled
    );
    if (cancellation_phase && !state.cancellation_requested)
        || (state.cancellation_requested
            && !cancellation_phase
            && state.phase != ModelToolLoopPhase::RecoveryRequired)
    {
        return Err(ModelToolLoopError::InvalidState);
    }
    if state.model_turns > state.settings.max_model_turns
        || state.tool_calls > state.settings.max_tool_calls
    {
        return Err(ModelToolLoopError::InvalidState);
    }
    match state.phase {
        ModelToolLoopPhase::AwaitingModel | ModelToolLoopPhase::Completed => {
            if state.cancellation_requested
                || state.pending_completion.is_some()
                || !state.pending_results.is_empty()
            {
                return Err(ModelToolLoopError::InvalidState);
            }
        }
        ModelToolLoopPhase::AwaitingToolResults => {
            if state.cancellation_requested
                || state.pending_completion.is_none()
                || state.pending_results.is_empty()
                || state
                    .pending_results
                    .iter()
                    .all(|pending| pending.result.is_some())
            {
                return Err(ModelToolLoopError::InvalidState);
            }
            validate_pending(state)?;
        }
        ModelToolLoopPhase::Cancelling => {
            if !state.cancellation_requested
                || state.pending_completion.is_none()
                || state.pending_results.is_empty()
                || state
                    .pending_results
                    .iter()
                    .all(|pending| pending.result.is_some())
            {
                return Err(ModelToolLoopError::InvalidState);
            }
            validate_pending(state)?;
        }
        ModelToolLoopPhase::RecoveryRequired => validate_pending(state)?,
        ModelToolLoopPhase::Stopped => {
            if state.cancellation_requested
                || state.pending_completion.is_some()
                || !state.pending_results.is_empty()
            {
                return Err(ModelToolLoopError::InvalidState);
            }
        }
        ModelToolLoopPhase::Cancelled => {
            if !state.cancellation_requested
                || state.pending_completion.is_some()
                || !state.pending_results.is_empty()
            {
                return Err(ModelToolLoopError::InvalidState);
            }
        }
    }
    Ok(())
}

/// Validate a deserialized loop snapshot before a host restores or persists it.
pub fn validate_model_tool_loop_state(
    state: &ModelToolLoopState,
) -> Result<(), ModelToolLoopError> {
    validate_state(state)
}

fn validate_pending(state: &ModelToolLoopState) -> Result<(), ModelToolLoopError> {
    let completion = state
        .pending_completion
        .as_ref()
        .ok_or(ModelToolLoopError::InvalidState)?;
    if completion.contract_version != CHAT_CONTRACT_VERSION
        || completion.tool_calls.len() != state.pending_results.len()
    {
        return Err(ModelToolLoopError::InvalidState);
    }
    let mut seen = std::collections::HashSet::with_capacity(state.pending_results.len());
    let mut result_bytes = 0usize;
    let mut has_unknown_effect = false;
    for (call, pending) in completion.tool_calls.iter().zip(&state.pending_results) {
        if call.id != pending.call_id
            || call.name != pending.tool_name
            || !seen.insert(pending.call_id.as_str())
        {
            return Err(ModelToolLoopError::InvalidState);
        }
        if let Some(result) = &pending.result
            && (result.run_id != state.run_id
                || result.turn_id != state.turn_id
                || result.call_id != pending.call_id
                || result.tool_name != pending.tool_name
                || result.contract_version != TOOL_RESULT_CONTRACT_VERSION
                || result_size_bytes(result)
                    .is_none_or(|size| size > state.settings.max_tool_result_bytes))
        {
            return Err(ModelToolLoopError::InvalidState);
        }
        if let Some(result) = &pending.result {
            validate_result_shape(result).map_err(|_| ModelToolLoopError::InvalidState)?;
            validate_effect_state(pending.effect, result.status, result.effect_state)
                .map_err(|_| ModelToolLoopError::InvalidState)?;
            result_bytes = result_bytes
                .checked_add(result_size_bytes(result).ok_or(ModelToolLoopError::InvalidState)?)
                .ok_or(ModelToolLoopError::InvalidState)?;
            has_unknown_effect |= result.effect_state == ToolEffectState::Unknown;
        }
    }
    if result_bytes > state.settings.max_tool_results_bytes_per_turn
        || (matches!(
            state.phase,
            ModelToolLoopPhase::AwaitingToolResults | ModelToolLoopPhase::Cancelling
        ) && has_unknown_effect)
        || (state.phase == ModelToolLoopPhase::RecoveryRequired
            && !has_unresolved_invocations(state))
    {
        return Err(ModelToolLoopError::InvalidState);
    }
    Ok(())
}

fn validate_result_shape(result: &ToolResult) -> Result<(), ModelToolLoopError> {
    if result.call_id.trim().is_empty() || result.tool_name.trim().is_empty() {
        return Err(ModelToolLoopError::InvalidToolResult);
    }
    match result.status {
        ToolResultStatus::Denied if result.effect_state != ToolEffectState::NotApplied => {
            Err(ModelToolLoopError::InvalidToolResult)
        }
        ToolResultStatus::Denied if result.code.as_deref().is_none_or(|code| !valid_code(code)) => {
            Err(ModelToolLoopError::InvalidToolResult)
        }
        _ if result.code.as_deref().is_some_and(|code| !valid_code(code)) => {
            Err(ModelToolLoopError::InvalidToolResult)
        }
        _ if result.parts.iter().any(|part| match part {
            ToolResultPart::Text { .. } => false,
            ToolResultPart::Image { media_ref } => {
                media_ref.asset_id.trim().is_empty()
                    || media_ref.mime_type.trim().is_empty()
                    || media_ref.name.trim().is_empty()
                    || media_ref.size_bytes == 0
                    || media_ref.sha256.len() != 64
                    || !media_ref
                        .sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
            }
        }) =>
        {
            Err(ModelToolLoopError::InvalidToolResult)
        }
        _ => Ok(()),
    }
}

fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 128
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn validate_effect_state(
    effect: ToolEffect,
    status: ToolResultStatus,
    effect_state: ToolEffectState,
) -> Result<(), ModelToolLoopError> {
    match (effect, status, effect_state) {
        (_, ToolResultStatus::Denied, ToolEffectState::NotApplied) => Ok(()),
        (ToolEffect::ReadOnly, _, ToolEffectState::NotApplicable) => Ok(()),
        (ToolEffect::ReadOnly, _, ToolEffectState::NotApplied) => Ok(()),
        (
            ToolEffect::ExternalEffect | ToolEffect::Destructive,
            ToolResultStatus::Succeeded,
            ToolEffectState::Applied,
        ) => Ok(()),
        (ToolEffect::ExternalEffect | ToolEffect::Destructive, ToolResultStatus::Succeeded, _) => {
            Err(ModelToolLoopError::InvalidToolResult)
        }
        (
            ToolEffect::ExternalEffect | ToolEffect::Destructive,
            _,
            ToolEffectState::NotApplied | ToolEffectState::Applied | ToolEffectState::Unknown,
        ) => Ok(()),
        _ => Err(ModelToolLoopError::InvalidToolResult),
    }
}

fn denied_result(
    run_id: &str,
    turn_id: &str,
    call_id: String,
    tool_name: String,
    code: ToolDenialCode,
) -> ToolResult {
    let code = denial_code(code);
    ToolResult {
        contract_version: TOOL_RESULT_CONTRACT_VERSION,
        run_id: run_id.into(),
        turn_id: turn_id.into(),
        call_id,
        tool_name,
        status: ToolResultStatus::Denied,
        effect_state: ToolEffectState::NotApplied,
        parts: vec![ToolResultPart::Text {
            text: format!("tool_denied:{code}"),
        }],
        code: Some(code.into()),
    }
}

fn denial_code(code: ToolDenialCode) -> &'static str {
    match code {
        ToolDenialCode::UnknownTool => "unknown_tool",
        ToolDenialCode::CapabilityNotGranted => "capability_not_granted",
        ToolDenialCode::InvalidArguments => "invalid_arguments",
        ToolDenialCode::SchemaMismatch => "schema_mismatch",
        ToolDenialCode::TurnCallLimitExceeded => "turn_call_limit_exceeded",
        ToolDenialCode::PolicyToolDisabled => "policy_tool_disabled",
        ToolDenialCode::PolicyCapabilityDenied => "policy_capability_denied",
        ToolDenialCode::UntrustedContentEffect => "untrusted_content_effect",
    }
}

fn ordered_results(pending: &[PendingToolResult]) -> Result<Vec<ToolResult>, ModelToolLoopError> {
    pending
        .iter()
        .map(|item| item.result.clone().ok_or(ModelToolLoopError::InvalidState))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{CHAT_CONTRACT_VERSION, FinishReason, ToolArguments, ToolCall};
    use crate::harness::{
        TOOL_DISPATCH_CONTRACT_VERSION, ToolDecision, ToolDispatch, ToolExecutionMode,
        ToolIdempotency,
    };

    fn state(max_model_turns: u32, max_tool_calls: u32) -> ModelToolLoopState {
        ModelToolLoopState::new(
            "run-1",
            "turn-1",
            ModelToolLoopSettings {
                max_model_turns,
                max_tool_calls,
                ..ModelToolLoopSettings::default()
            },
        )
        .unwrap()
    }

    fn completion(calls: Vec<ToolCall>) -> ChatCompletion {
        ChatCompletion {
            contract_version: CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: None,
            content: "answer".into(),
            reasoning: Some("private reasoning".into()),
            finish_reason: Some(if calls.is_empty() {
                FinishReason::Stop
            } else {
                FinishReason::ToolCalls
            }),
            tool_calls: calls,
            usage: None,
            media_decisions: Vec::new(),
        }
    }

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: ToolArguments::from_json("{}"),
        }
    }

    fn dispatch(calls: &[ToolCall], run_id: &str, turn_id: &str) -> ToolDispatch {
        ToolDispatch {
            contract_version: TOOL_DISPATCH_CONTRACT_VERSION,
            run_id: run_id.into(),
            turn_id: turn_id.into(),
            execution_mode: ToolExecutionMode::Sequential,
            decisions: calls
                .iter()
                .cloned()
                .map(|call| ToolDecision::Invoke {
                    call,
                    schema_version: "test.v1".into(),
                    required_capability: "test.execute".into(),
                    timeout_ms: 1000,
                    effect: ToolEffect::ExternalEffect,
                    idempotency: ToolIdempotency::RequiresKey,
                    idempotency_key: Some("key".into()),
                })
                .collect(),
        }
    }

    fn result(call_id: &str, tool_name: &str) -> ToolResult {
        ToolResult {
            contract_version: TOOL_RESULT_CONTRACT_VERSION,
            run_id: "run-1".into(),
            turn_id: "turn-1".into(),
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            status: ToolResultStatus::Succeeded,
            effect_state: ToolEffectState::Applied,
            code: None,
            parts: vec![ToolResultPart::Text {
                text: format!("result-{call_id}"),
            }],
        }
    }

    #[test]
    fn tool_result_parts_convert_to_a_correlated_chat_message() {
        let mut result = result("call-image", "capture");
        result.parts.push(ToolResultPart::Image {
            media_ref: MediaRef {
                asset_id: "asset-1".into(),
                sha256: "a".repeat(64),
                mime_type: "image/png".into(),
                size_bytes: 12,
                name: "capture.png".into(),
            },
        });
        let message = result.to_chat_message();
        assert_eq!(message.role, MessageRole::Tool);
        assert_eq!(message.tool_call_id.as_deref(), Some("call-image"));
        assert_eq!(message.name.as_deref(), Some("capture"));
        assert!(matches!(
            &message.content,
            MessageContent::Parts(parts)
                if matches!(&parts[1], ContentPart::Image(media) if media.media_ref.asset_id == "asset-1")
        ));
    }

    #[test]
    fn legacy_string_tool_results_and_loop_snapshots_are_rejected() {
        let legacy = serde_json::json!({
            "contract_version": 1,
            "run_id": "run-1",
            "turn_id": "turn-1",
            "call_id": "call-1",
            "tool_name": "lookup",
            "status": "succeeded",
            "effect_state": "not_applicable",
            "output": "old string output"
        });
        assert!(serde_json::from_value::<ToolResult>(legacy).is_err());

        let mut old_state = state(5, 10);
        old_state.contract_version = 2;
        assert!(matches!(
            validate_state(&old_state),
            Err(ModelToolLoopError::UnsupportedLoopContractVersion { received: 2 })
        ));
    }

    #[test]
    fn empty_model_turn_completes_and_strips_reasoning() {
        let state = state(5, 10);
        let (state, action) = accept_model_turn(
            &state,
            completion(Vec::new()),
            dispatch(&[], "run-1", "turn-1"),
        )
        .unwrap();
        assert_eq!(state.phase, ModelToolLoopPhase::Completed);
        assert!(matches!(
            action,
            ModelToolLoopAction::Complete {
                assistant_completion: ChatCompletion {
                    reasoning: None,
                    ..
                },
                model_turns: 1,
                ..
            }
        ));
    }

    #[test]
    fn adapter_results_are_correlated_and_returned_in_call_order() {
        let calls = vec![call("call-1", "write"), call("call-2", "read")];
        let (state, action) = accept_model_turn(
            &state(5, 10),
            completion(calls.clone()),
            dispatch(&calls, "run-1", "turn-1"),
        )
        .unwrap();
        assert!(matches!(action, ModelToolLoopAction::ExecuteTools { .. }));
        let (state, action) = submit_tool_result(&state, result("call-2", "read")).unwrap();
        assert_eq!(state.phase, ModelToolLoopPhase::AwaitingToolResults);
        assert_eq!(
            action,
            ModelToolLoopAction::AwaitingToolResults { remaining: 1 }
        );
        let (_, action) = submit_tool_result(&state, result("call-1", "write")).unwrap();
        assert!(matches!(
            action,
            ModelToolLoopAction::ContinueWithToolResults { results, .. }
                if results.iter().map(|result| result.call_id.as_str()).collect::<Vec<_>>() == ["call-1", "call-2"]
        ));
    }

    #[test]
    fn denied_calls_become_non_effecting_results_without_adapter_execution() {
        let call = call("call-1", "danger");
        let dispatch = ToolDispatch {
            contract_version: TOOL_DISPATCH_CONTRACT_VERSION,
            run_id: "run-1".into(),
            turn_id: "turn-1".into(),
            execution_mode: ToolExecutionMode::ConcurrentReadOnly,
            decisions: vec![ToolDecision::Deny {
                call_id: call.id.clone(),
                tool_name: call.name.clone(),
                code: ToolDenialCode::CapabilityNotGranted,
            }],
        };
        let (state, action) =
            accept_model_turn(&state(4, 10), completion(vec![call]), dispatch).unwrap();
        assert_eq!(state.phase, ModelToolLoopPhase::AwaitingModel);
        assert!(matches!(
            action,
            ModelToolLoopAction::ContinueWithToolResults { results, .. }
                if results[0].status == ToolResultStatus::Denied
                    && results[0].effect_state == ToolEffectState::NotApplied
                    && results[0].parts == [ToolResultPart::Text {
                        text: "tool_denied:capability_not_granted".into()
                    }]
        ));
    }

    #[test]
    fn unknown_effect_outcome_stops_loop_for_reconciliation() {
        let tool_call = call("call-1", "publish");
        let (state, _) = accept_model_turn(
            &state(5, 10),
            completion(vec![tool_call]),
            dispatch(&[call("call-1", "publish")], "run-1", "turn-1"),
        )
        .unwrap();
        let uncertain = ToolResult {
            effect_state: ToolEffectState::Unknown,
            status: ToolResultStatus::TimedOut,
            ..result("call-1", "publish")
        };
        let (state, action) = submit_tool_result(&state, uncertain).unwrap();
        assert_eq!(state.phase, ModelToolLoopPhase::RecoveryRequired);
        assert!(matches!(
            action,
            ModelToolLoopAction::RecoveryRequired { .. }
        ));
    }

    #[test]
    fn cancellation_while_awaiting_model_is_terminal_and_idempotent() {
        let initial = state(5, 10);
        let cancelled = cancel_model_tool_loop(&initial).unwrap();
        assert_eq!(cancelled.state.phase, ModelToolLoopPhase::Cancelled);
        assert!(cancelled.state.cancellation_requested);
        assert!(matches!(
            cancelled.action,
            ModelToolLoopAction::Cancelled { .. }
        ));
        assert_eq!(cancel_model_tool_loop(&cancelled.state).unwrap(), cancelled);
        validate_state(&cancelled.state).unwrap();
    }

    #[test]
    fn cancellation_waits_for_pending_effects_before_terminating() {
        let calls = vec![call("call-1", "write"), call("call-2", "publish")];
        let (running, _) = accept_model_turn(
            &state(5, 10),
            completion(calls.clone()),
            dispatch(&calls, "run-1", "turn-1"),
        )
        .unwrap();
        let cancelling = cancel_model_tool_loop(&running).unwrap();
        assert_eq!(cancelling.state.phase, ModelToolLoopPhase::Cancelling);
        assert_eq!(
            cancelling.action,
            ModelToolLoopAction::CancelPendingTools {
                calls: vec![
                    PendingToolCancellation {
                        call_id: "call-1".into(),
                        tool_name: "write".into(),
                    },
                    PendingToolCancellation {
                        call_id: "call-2".into(),
                        tool_name: "publish".into(),
                    },
                ]
            }
        );

        let (still_cancelling, action) =
            submit_tool_result(&cancelling.state, result("call-2", "publish")).unwrap();
        assert_eq!(still_cancelling.phase, ModelToolLoopPhase::Cancelling);
        assert_eq!(
            action,
            ModelToolLoopAction::AwaitingToolResults { remaining: 1 }
        );
        let (cancelled, action) =
            submit_tool_result(&still_cancelling, result("call-1", "write")).unwrap();
        assert_eq!(cancelled.phase, ModelToolLoopPhase::Cancelled);
        assert!(matches!(action, ModelToolLoopAction::Cancelled { .. }));
        assert_eq!(
            submit_tool_result(&cancelled, result("call-1", "write")).unwrap_err(),
            ModelToolLoopError::NotAwaitingToolResults
        );
        validate_state(&cancelled).unwrap();
    }

    #[test]
    fn cancelled_loop_reconciles_uncertain_effect_without_resuming_model() {
        let tool_call = call("call-1", "publish");
        let (running, _) = accept_model_turn(
            &state(5, 10),
            completion(vec![tool_call.clone()]),
            dispatch(&[tool_call], "run-1", "turn-1"),
        )
        .unwrap();
        let cancelling = cancel_model_tool_loop(&running).unwrap();
        let uncertain = ToolResult {
            effect_state: ToolEffectState::Unknown,
            status: ToolResultStatus::TimedOut,
            ..result("call-1", "publish")
        };
        let (recovery, action) = submit_tool_result(&cancelling.state, uncertain).unwrap();
        assert_eq!(recovery.phase, ModelToolLoopPhase::RecoveryRequired);
        assert!(recovery.cancellation_requested);
        assert!(matches!(
            action,
            ModelToolLoopAction::RecoveryRequired { .. }
        ));

        let reconciled = reconcile_tool_result(
            &recovery,
            ToolResult {
                effect_state: ToolEffectState::Applied,
                status: ToolResultStatus::Succeeded,
                ..result("call-1", "publish")
            },
        )
        .unwrap();
        assert_eq!(reconciled.state.phase, ModelToolLoopPhase::Cancelled);
        assert!(matches!(
            reconciled.action,
            ModelToolLoopAction::Cancelled { .. }
        ));
    }

    #[test]
    fn budgets_stop_before_tool_effects_when_no_model_round_remains() {
        let call = call("call-1", "write");
        let (state, action) = accept_model_turn(
            &state(1, 10),
            completion(vec![call.clone()]),
            dispatch(&[call], "run-1", "turn-1"),
        )
        .unwrap();
        assert_eq!(state.phase, ModelToolLoopPhase::Stopped);
        assert!(matches!(
            action,
            ModelToolLoopAction::Stopped {
                reason: ModelToolLoopStopReason::ModelTurnLimit,
                ..
            }
        ));
    }

    #[test]
    fn mismatched_dispatch_and_stale_results_are_rejected() {
        let call = call("call-1", "read");
        let mut wrong_dispatch = dispatch(std::slice::from_ref(&call), "other-run", "turn-1");
        assert_eq!(
            accept_model_turn(
                &state(5, 10),
                completion(vec![call.clone()]),
                wrong_dispatch.clone()
            )
            .unwrap_err(),
            ModelToolLoopError::DispatchIdentityMismatch
        );
        wrong_dispatch.run_id = "run-1".into();
        let (state, _) =
            accept_model_turn(&state(5, 10), completion(vec![call]), wrong_dispatch).unwrap();
        let (state, _) = submit_tool_result(&state, result("call-1", "read")).unwrap();
        assert_eq!(
            submit_tool_result(&state, result("call-1", "read")).unwrap_err(),
            ModelToolLoopError::NotAwaitingToolResults
        );
    }
}
