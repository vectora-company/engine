//! Deterministic, default-deny preparation of model-proposed tool calls.
//!
//! This module validates contracts and arguments but never executes effects.

use std::collections::{HashMap, HashSet};

use jsonschema::validator_for;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::chat::{CHAT_CONTRACT_VERSION, ChatCompletion, ToolArguments, ToolCall, ToolDefinition};

use super::{RunLifecycle, RunStatus};

/// Version of the tool description supplied by a consumer adapter.
pub const TOOL_CONTRACT_VERSION: u16 = 1;
/// Version of the prepared dispatch batch returned to a consumer.
pub const TOOL_DISPATCH_CONTRACT_VERSION: u16 = 1;

const DEFAULT_MAX_CALLS_PER_TURN: usize = 32;
const MAX_TOOL_NAME_BYTES: usize = 128;
const MAX_CAPABILITY_BYTES: usize = 256;
const MAX_SCHEMA_BYTES: usize = 256 * 1024;
const MAX_TIMEOUT_MS: u64 = 30 * 60 * 1000;

/// Side-effect classification declared by the trusted tool adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    ReadOnly,
    ExternalEffect,
    Destructive,
}

/// Retry guarantee the adapter can actually provide for this operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolIdempotency {
    NotGuaranteed,
    Idempotent,
    RequiresKey,
}

/// Versioned input and execution contract for one consumer-provided tool.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolContract {
    pub contract_version: u16,
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Self-contained JSON Schema. Remote references are not fetched.
    pub input_schema: Value,
    pub required_capability: String,
    pub timeout_ms: u64,
    pub effect: ToolEffect,
    pub idempotency: ToolIdempotency,
}

impl ToolContract {
    fn validate(&self) -> Result<(), ToolDispatchError> {
        if self.contract_version != TOOL_CONTRACT_VERSION {
            return Err(ToolDispatchError::UnsupportedToolContractVersion {
                received: self.contract_version,
            });
        }
        if !valid_identifier(&self.name, MAX_TOOL_NAME_BYTES) {
            return Err(ToolDispatchError::InvalidToolContract("name"));
        }
        if self.schema_version.trim().is_empty() {
            return Err(ToolDispatchError::InvalidToolContract("schema_version"));
        }
        if !valid_identifier(&self.required_capability, MAX_CAPABILITY_BYTES) {
            return Err(ToolDispatchError::InvalidToolContract(
                "required_capability",
            ));
        }
        if self.timeout_ms == 0 || self.timeout_ms > MAX_TIMEOUT_MS {
            return Err(ToolDispatchError::InvalidToolContract("timeout_ms"));
        }
        let schema_bytes = serde_json::to_vec(&self.input_schema)
            .map_err(|_| ToolDispatchError::InvalidToolContract("input_schema"))?;
        if schema_bytes.len() > MAX_SCHEMA_BYTES {
            return Err(ToolDispatchError::InvalidToolContract("input_schema_size"));
        }
        if self.input_schema.get("type") != Some(&Value::String("object".into())) {
            return Err(ToolDispatchError::InvalidToolContract("input_schema_type"));
        }
        validator_for(&self.input_schema)
            .map_err(|_| ToolDispatchError::InvalidToolContract("input_schema"))?;
        Ok(())
    }

    /// Convert the validated contract to the provider-neutral chat schema.
    pub fn chat_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        }
    }
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

/// Limits for one model turn's dispatch preparation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolDispatchSettings {
    #[serde(default = "default_max_calls_per_turn")]
    pub max_calls_per_turn: usize,
}

const fn default_max_calls_per_turn() -> usize {
    DEFAULT_MAX_CALLS_PER_TURN
}

impl Default for ToolDispatchSettings {
    fn default() -> Self {
        Self {
            max_calls_per_turn: default_max_calls_per_turn(),
        }
    }
}

/// Whether allowed calls may safely be run concurrently by the consumer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionMode {
    ConcurrentReadOnly,
    Sequential,
}

/// One authorized invocation or a stable denial for a proposed tool call.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ToolDecision {
    Invoke {
        call: ToolCall,
        schema_version: String,
        required_capability: String,
        timeout_ms: u64,
        effect: ToolEffect,
        idempotency: ToolIdempotency,
        #[serde(skip_serializing_if = "Option::is_none")]
        idempotency_key: Option<String>,
    },
    Deny {
        call_id: String,
        tool_name: String,
        code: ToolDenialCode,
    },
}

/// Machine-readable reason for denying a model-proposed invocation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDenialCode {
    UnknownTool,
    CapabilityNotGranted,
    InvalidArguments,
    SchemaMismatch,
    TurnCallLimitExceeded,
    PolicyToolDisabled,
    PolicyCapabilityDenied,
    UntrustedContentEffect,
}

/// Prepared, ordered decisions for one assistant turn.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolDispatch {
    pub contract_version: u16,
    pub run_id: String,
    pub turn_id: String,
    pub execution_mode: ToolExecutionMode,
    pub decisions: Vec<ToolDecision>,
}

/// Structural or contract errors that prevent dispatch preparation.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ToolDispatchError {
    #[error("unsupported tool contract version {received}")]
    UnsupportedToolContractVersion { received: u16 },
    #[error("invalid tool contract field {0}")]
    InvalidToolContract(&'static str),
    #[error("tool registry contains duplicate name {0}")]
    DuplicateToolName(String),
    #[error("tool call IDs must be unique within a turn")]
    DuplicateToolCallId,
    #[error("tool call ID must not be empty")]
    EmptyToolCallId,
    #[error("tool dispatch requires a running run")]
    RunNotRunning,
    #[error("unsupported chat contract version {received}")]
    UnsupportedChatContractVersion { received: u16 },
    #[error(transparent)]
    InvalidRun(#[from] super::RunError),
    #[error("max_calls_per_turn must be greater than zero")]
    InvalidCallLimit,
}

/// Validate a completed model turn and prepare authorized tool invocations.
///
/// The run identity and its capabilities must come from a trusted host. This
/// function is a dispatch gate, not a substitute for authenticating that host.
/// Tool adapters remain responsible for executing the returned invocations.
pub fn prepare_tool_dispatch(
    run: &RunLifecycle,
    completion: &ChatCompletion,
    contracts: &[ToolContract],
    settings: ToolDispatchSettings,
) -> Result<ToolDispatch, ToolDispatchError> {
    run.validate()?;
    if run.status != RunStatus::Running {
        return Err(ToolDispatchError::RunNotRunning);
    }
    if completion.contract_version != CHAT_CONTRACT_VERSION {
        return Err(ToolDispatchError::UnsupportedChatContractVersion {
            received: completion.contract_version,
        });
    }
    if settings.max_calls_per_turn == 0 {
        return Err(ToolDispatchError::InvalidCallLimit);
    }

    struct ValidatedContract<'a> {
        contract: &'a ToolContract,
        validator: jsonschema::Validator,
    }

    let mut registry = HashMap::with_capacity(contracts.len());
    for contract in contracts {
        contract.validate()?;
        let validator = validator_for(&contract.input_schema)
            .map_err(|_| ToolDispatchError::InvalidToolContract("input_schema"))?;
        if registry
            .insert(
                contract.name.as_str(),
                ValidatedContract {
                    contract,
                    validator,
                },
            )
            .is_some()
        {
            return Err(ToolDispatchError::DuplicateToolName(contract.name.clone()));
        }
    }

    let mut seen_ids = HashSet::with_capacity(completion.tool_calls.len());
    for call in &completion.tool_calls {
        if call.id.is_empty() {
            return Err(ToolDispatchError::EmptyToolCallId);
        }
        if !seen_ids.insert(call.id.as_str()) {
            return Err(ToolDispatchError::DuplicateToolCallId);
        }
    }

    let capabilities: HashSet<&str> = run
        .identity
        .capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let mut allowed_effect = ToolEffect::ReadOnly;
    let mut decisions = Vec::with_capacity(completion.tool_calls.len());

    for (index, call) in completion.tool_calls.iter().enumerate() {
        if index >= settings.max_calls_per_turn {
            decisions.push(deny(call, ToolDenialCode::TurnCallLimitExceeded));
            continue;
        }
        let Some(validated) = registry.get(call.name.as_str()) else {
            decisions.push(deny(call, ToolDenialCode::UnknownTool));
            continue;
        };
        let contract = validated.contract;
        if !capabilities.contains(contract.required_capability.as_str()) {
            decisions.push(deny(call, ToolDenialCode::CapabilityNotGranted));
            continue;
        }
        let arguments = match to_json_object(&call.arguments) {
            Some(arguments) => Value::Object(arguments),
            None => {
                decisions.push(deny(call, ToolDenialCode::InvalidArguments));
                continue;
            }
        };
        if validated.validator.iter_errors(&arguments).next().is_some() {
            decisions.push(deny(call, ToolDenialCode::SchemaMismatch));
            continue;
        }
        if contract.effect != ToolEffect::ReadOnly {
            allowed_effect = contract.effect;
        }
        let idempotency_key = (contract.idempotency == ToolIdempotency::RequiresKey)
            .then(|| format!("{}:tool:{}", run.identity.run_id, call.id));
        decisions.push(ToolDecision::Invoke {
            call: call.clone(),
            schema_version: contract.schema_version.clone(),
            required_capability: contract.required_capability.clone(),
            timeout_ms: contract.timeout_ms,
            effect: contract.effect,
            idempotency: contract.idempotency,
            idempotency_key,
        });
    }

    let execution_mode = if allowed_effect == ToolEffect::ReadOnly {
        ToolExecutionMode::ConcurrentReadOnly
    } else {
        ToolExecutionMode::Sequential
    };
    Ok(ToolDispatch {
        contract_version: TOOL_DISPATCH_CONTRACT_VERSION,
        run_id: run.identity.run_id.clone(),
        turn_id: run.identity.turn_id.clone(),
        execution_mode,
        decisions,
    })
}

/// Convert contracts to model-visible schemas after validating their registry.
pub fn tool_chat_definitions(
    contracts: &[ToolContract],
) -> Result<Vec<ToolDefinition>, ToolDispatchError> {
    let mut names = HashSet::with_capacity(contracts.len());
    let mut definitions = Vec::with_capacity(contracts.len());
    for contract in contracts {
        contract.validate()?;
        if !names.insert(contract.name.as_str()) {
            return Err(ToolDispatchError::DuplicateToolName(contract.name.clone()));
        }
        definitions.push(contract.chat_definition());
    }
    Ok(definitions)
}

fn deny(call: &ToolCall, code: ToolDenialCode) -> ToolDecision {
    ToolDecision::Deny {
        call_id: call.id.clone(),
        tool_name: call.name.clone(),
        code,
    }
}

fn to_json_object(arguments: &ToolArguments) -> Option<Map<String, Value>> {
    match arguments {
        ToolArguments::Object { value } => Some(
            value
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        ToolArguments::InvalidJson { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{CHAT_CONTRACT_VERSION, FinishReason};
    use serde_json::json;

    fn active_run(capabilities: Vec<&str>) -> RunLifecycle {
        let mut run = RunLifecycle::new(super::super::RunIdentity {
            contract_version: super::super::RUN_CONTRACT_VERSION,
            run_id: "run-1".into(),
            idempotency_key: "turn-1".into(),
            thread_id: "thread-1".into(),
            turn_id: "turn-1".into(),
            actor_id: None,
            scope_id: None,
            provider: Some("test".into()),
            model: Some("model".into()),
            capabilities: capabilities.into_iter().map(str::to_owned).collect(),
        })
        .unwrap();
        run.transition(RunStatus::Running).unwrap();
        run
    }

    fn contract(effect: ToolEffect, idempotency: ToolIdempotency) -> ToolContract {
        ToolContract {
            contract_version: TOOL_CONTRACT_VERSION,
            schema_version: "lookup.v1".into(),
            name: "lookup".into(),
            description: Some("Read one item".into()),
            input_schema: json!({
                "type": "object",
                "properties": {"key": {"type": "string", "minLength": 1}},
                "required": ["key"],
                "additionalProperties": false
            }),
            required_capability: "data.read".into(),
            timeout_ms: 5_000,
            effect,
            idempotency,
        }
    }

    fn completion(arguments: ToolArguments) -> ChatCompletion {
        ChatCompletion {
            contract_version: CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: None,
            content: String::new(),
            reasoning: None,
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "lookup".into(),
                arguments,
            }],
            finish_reason: Some(FinishReason::ToolCalls),
            usage: None,
            media_decisions: Vec::new(),
        }
    }

    fn object_args(value: Value) -> ToolArguments {
        ToolArguments::from_json(&value.to_string())
    }

    #[test]
    fn authorizes_schema_valid_read_only_call() {
        let run = active_run(vec!["data.read"]);
        let result = prepare_tool_dispatch(
            &run,
            &completion(object_args(json!({"key": "alpha"}))),
            &[contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent)],
            ToolDispatchSettings::default(),
        )
        .unwrap();
        assert_eq!(result.execution_mode, ToolExecutionMode::ConcurrentReadOnly);
        assert!(matches!(result.decisions[0], ToolDecision::Invoke { .. }));
    }

    #[test]
    fn denies_missing_capability_unknown_tool_and_invalid_arguments() {
        let run = active_run(vec![]);
        let denied = prepare_tool_dispatch(
            &run,
            &completion(object_args(json!({"key": "alpha"}))),
            &[contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent)],
            ToolDispatchSettings::default(),
        )
        .unwrap();
        assert!(matches!(
            denied.decisions[0],
            ToolDecision::Deny {
                code: ToolDenialCode::CapabilityNotGranted,
                ..
            }
        ));

        let authorized_run = active_run(vec!["data.read"]);
        let malformed = prepare_tool_dispatch(
            &authorized_run,
            &completion(ToolArguments::InvalidJson { raw: "nope".into() }),
            &[contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent)],
            ToolDispatchSettings::default(),
        )
        .unwrap();
        assert!(matches!(
            malformed.decisions[0],
            ToolDecision::Deny {
                code: ToolDenialCode::InvalidArguments,
                ..
            }
        ));
    }

    #[test]
    fn schema_mismatch_is_denied_without_exposing_arguments() {
        let run = active_run(vec!["data.read"]);
        let result = prepare_tool_dispatch(
            &run,
            &completion(object_args(json!({"key": "", "secret": "do-not-log"}))),
            &[contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent)],
            ToolDispatchSettings::default(),
        )
        .unwrap();
        assert!(matches!(
            &result.decisions[0],
            ToolDecision::Deny {
                code: ToolDenialCode::SchemaMismatch,
                ..
            }
        ));
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("do-not-log")
        );
    }

    #[test]
    fn side_effects_are_sequential_and_keyed_when_contract_requires_it() {
        let run = active_run(vec!["data.read"]);
        let result = prepare_tool_dispatch(
            &run,
            &completion(object_args(json!({"key": "alpha"}))),
            &[contract(
                ToolEffect::ExternalEffect,
                ToolIdempotency::RequiresKey,
            )],
            ToolDispatchSettings::default(),
        )
        .unwrap();
        assert_eq!(result.execution_mode, ToolExecutionMode::Sequential);
        assert!(matches!(
            &result.decisions[0],
            ToolDecision::Invoke {
                idempotency_key: Some(key),
                ..
            } if key == "run-1:tool:call-1"
        ));
    }

    #[test]
    fn rejects_invalid_run_state_contract_and_duplicate_registry_entries() {
        let run = active_run(vec!["data.read"]);
        let planned = RunLifecycle::new(run.identity.clone()).unwrap();
        assert_eq!(
            prepare_tool_dispatch(
                &planned,
                &completion(object_args(json!({"key": "alpha"}))),
                &[contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent)],
                ToolDispatchSettings::default(),
            ),
            Err(ToolDispatchError::RunNotRunning)
        );
        let mut running = planned;
        running.transition(RunStatus::Running).unwrap();
        let valid = contract(ToolEffect::ReadOnly, ToolIdempotency::Idempotent);
        assert_eq!(
            prepare_tool_dispatch(
                &running,
                &completion(object_args(json!({"key": "alpha"}))),
                &[valid.clone(), valid],
                ToolDispatchSettings::default(),
            ),
            Err(ToolDispatchError::DuplicateToolName("lookup".into()))
        );
    }
}
