//! Consumer-supplied execution policy and content-bound approval gate.
//!
//! This module evaluates policy snapshots supplied by a trusted host. It does
//! not authenticate that host, identify a human approver, or execute tools.

use std::collections::{BTreeMap, HashMap, HashSet};

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::chat::{ChatCompletion, ToolArguments, ToolCall};

use super::{
    RunLifecycle, ToolContract, ToolDecision, ToolDenialCode, ToolDispatch, ToolDispatchError,
    ToolDispatchSettings, ToolEffect, prepare_tool_dispatch,
};

/// Version of the policy evaluation and approval binding contract.
pub const TOOL_POLICY_CONTRACT_VERSION: u16 = 1;
const MAX_POLICY_ITEMS: usize = 4096;
const MAX_POLICY_VALUE_BYTES: usize = 4096;

/// Generic approval strategy supplied by the consumer's effective policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Require a host-validated approval for every tool with external effects.
    RequireAllEffects,
    /// Require approval only for tools classified as destructive.
    RequireDestructive,
    /// Capabilities are sufficient; the host assumes the approval decision.
    CapabilityOnly,
}

/// Immutable effective policy for exactly one run and turn.
///
/// The host compiles identity, product policy, workspace restrictions and
/// trusted capabilities before constructing this snapshot. The engine checks
/// its binding and applies its restrictions but cannot authenticate its issuer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicySnapshot {
    pub contract_version: u16,
    pub run_id: String,
    pub thread_id: String,
    pub turn_id: String,
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
    /// Opaque, stable digest of the complete policy compiled by the host.
    pub policy_digest: String,
    pub revision: String,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub denied_capabilities: Vec<String>,
    #[serde(default)]
    pub untrusted_content: bool,
    /// Defaults to the stricter strategy when omitted from serialized input.
    #[serde(default = "default_approval_mode")]
    pub approval_mode: ApprovalMode,
    #[serde(default)]
    pub expires_at_ms: Option<u64>,
}

const fn default_approval_mode() -> ApprovalMode {
    ApprovalMode::RequireAllEffects
}

/// An approval request bound to one exact model-proposed tool call.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalRequest {
    pub contract_version: u16,
    pub run_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub scope_id: Option<String>,
    pub policy_digest: String,
    pub arguments_digest: String,
    pub binding_digest: String,
}

/// Host-validated approval receipt for one exact approval request.
///
/// The consumer must authenticate the approver and source of this receipt
/// before passing it to the engine. Matching a digest is not authentication.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalGrant {
    pub contract_version: u16,
    pub binding_digest: String,
    pub approved_by: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Final state of evaluating the whole ordered tool batch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEvaluationStatus {
    Ready,
    AwaitingApproval,
}

/// Per-call result; arguments and policy internals are deliberately omitted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ToolAuthorizationDecision {
    Allow {
        call_id: String,
        tool_name: String,
    },
    RequireApproval {
        request: ApprovalRequest,
    },
    Deny {
        call_id: String,
        tool_name: String,
        code: ToolDenialCode,
    },
}

/// Result of enforcing policy before a consumer executes any adapter.
///
/// `dispatch` is present only when all required approvals have been supplied.
/// Consumers must not execute calls merely because an individual decision is
/// `allow` while the batch status is `awaiting_approval`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PolicyEvaluation {
    pub contract_version: u16,
    pub run_id: String,
    pub turn_id: String,
    pub status: PolicyEvaluationStatus,
    pub decisions: Vec<ToolAuthorizationDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<ToolDispatch>,
}

/// Invalid, stale or cross-run policy and approval inputs.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ToolPolicyError {
    #[error("unsupported tool policy contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("invalid policy snapshot field {0}")]
    InvalidSnapshot(&'static str),
    #[error("policy snapshot does not match the active run")]
    SnapshotRunMismatch,
    #[error("policy snapshot has expired")]
    SnapshotExpired,
    #[error("policy contains too many entries")]
    TooManyEntries,
    #[error("policy entry is invalid")]
    InvalidPolicyEntry,
    #[error("approval grants must have unique call IDs")]
    DuplicateApproval,
    #[error("approval grant is invalid or expired")]
    InvalidApproval,
    #[error("approval grant does not match a pending tool call")]
    UnusedApproval,
    #[error(transparent)]
    Dispatch(#[from] ToolDispatchError),
    #[error(transparent)]
    Run(#[from] super::RunError),
}

/// Prepare and enforce one tool batch against a run-bound policy snapshot.
///
/// Call this immediately before execution. If any call still needs approval,
/// no dispatch is returned for the batch. After the host obtains and validates
/// approval, it calls this function again with grants bound to the returned
/// approval requests. A changed call or policy produces a different digest.
pub fn authorize_tool_dispatch(
    run: &RunLifecycle,
    completion: &ChatCompletion,
    contracts: &[ToolContract],
    settings: ToolDispatchSettings,
    snapshot: &PolicySnapshot,
    grants: &[ApprovalGrant],
    now_ms: u64,
) -> Result<PolicyEvaluation, ToolPolicyError> {
    run.validate()?;
    validate_policy_snapshot(run, snapshot, now_ms)?;
    let dispatch = prepare_tool_dispatch(run, completion, contracts, settings)?;
    let contracts_by_name: HashMap<&str, &ToolContract> = contracts
        .iter()
        .map(|contract| (contract.name.as_str(), contract))
        .collect();
    let disabled_tools: HashSet<&str> =
        snapshot.disabled_tools.iter().map(String::as_str).collect();
    let denied_capabilities: HashSet<&str> = snapshot
        .denied_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let mut grants_by_call = HashMap::with_capacity(grants.len());

    for grant in grants {
        validate_approval_grant(grant, now_ms)?;
        if grants_by_call
            .insert(grant.binding_digest.as_str(), grant)
            .is_some()
        {
            return Err(ToolPolicyError::DuplicateApproval);
        }
    }

    let mut consumed_grants = HashSet::with_capacity(grants.len());
    let mut decisions = Vec::with_capacity(dispatch.decisions.len());
    let mut authorized_dispatch_decisions = Vec::with_capacity(dispatch.decisions.len());
    let mut awaiting_approval = false;

    for decision in dispatch.decisions {
        match decision {
            ToolDecision::Deny {
                call_id,
                tool_name,
                code,
            } => {
                decisions.push(ToolAuthorizationDecision::Deny {
                    call_id: call_id.clone(),
                    tool_name: tool_name.clone(),
                    code,
                });
                authorized_dispatch_decisions.push(ToolDecision::Deny {
                    call_id,
                    tool_name,
                    code,
                });
            }
            invocation @ ToolDecision::Invoke { .. } => {
                let ToolDecision::Invoke { call, .. } = &invocation else {
                    unreachable!("matched invocation")
                };
                let contract = contracts_by_name
                    .get(call.name.as_str())
                    .copied()
                    .ok_or(ToolPolicyError::InvalidSnapshot("tool contract"))?;
                let call_id = call.id.clone();
                let tool_name = call.name.clone();
                let denial = if disabled_tools.contains(tool_name.as_str()) {
                    Some(ToolDenialCode::PolicyToolDisabled)
                } else if denied_capabilities.contains(contract.required_capability.as_str()) {
                    Some(ToolDenialCode::PolicyCapabilityDenied)
                } else if snapshot.untrusted_content && contract.effect != ToolEffect::ReadOnly {
                    Some(ToolDenialCode::UntrustedContentEffect)
                } else {
                    None
                };

                if let Some(code) = denial {
                    decisions.push(ToolAuthorizationDecision::Deny {
                        call_id: call_id.clone(),
                        tool_name: tool_name.clone(),
                        code,
                    });
                    authorized_dispatch_decisions.push(ToolDecision::Deny {
                        call_id,
                        tool_name,
                        code,
                    });
                    continue;
                }

                let approval_request = approval_required(snapshot.approval_mode, contract.effect)
                    .then(|| make_approval_request(run, call, contract, snapshot));

                if let Some(request) = approval_request {
                    match grants_by_call.get(request.binding_digest.as_str()) {
                        Some(grant) => {
                            if grant.binding_digest != request.binding_digest {
                                return Err(ToolPolicyError::InvalidApproval);
                            }
                            consumed_grants.insert(request.binding_digest.clone());
                        }
                        None => {
                            awaiting_approval = true;
                            decisions.push(ToolAuthorizationDecision::RequireApproval { request });
                            continue;
                        }
                    }
                }

                decisions.push(ToolAuthorizationDecision::Allow { call_id, tool_name });
                authorized_dispatch_decisions.push(invocation);
            }
        }
    }

    if consumed_grants.len() != grants.len() {
        return Err(ToolPolicyError::UnusedApproval);
    }

    let status = if awaiting_approval {
        PolicyEvaluationStatus::AwaitingApproval
    } else {
        PolicyEvaluationStatus::Ready
    };
    let authorized_dispatch = (status == PolicyEvaluationStatus::Ready).then_some(ToolDispatch {
        contract_version: dispatch.contract_version,
        run_id: dispatch.run_id.clone(),
        turn_id: dispatch.turn_id.clone(),
        execution_mode: dispatch.execution_mode,
        decisions: authorized_dispatch_decisions,
    });

    Ok(PolicyEvaluation {
        contract_version: TOOL_POLICY_CONTRACT_VERSION,
        run_id: dispatch.run_id,
        turn_id: dispatch.turn_id,
        status,
        decisions,
        dispatch: authorized_dispatch,
    })
}

pub fn validate_policy_snapshot(
    run: &RunLifecycle,
    snapshot: &PolicySnapshot,
    now_ms: u64,
) -> Result<(), ToolPolicyError> {
    if snapshot.contract_version != TOOL_POLICY_CONTRACT_VERSION {
        return Err(ToolPolicyError::UnsupportedContractVersion {
            received: snapshot.contract_version,
        });
    }
    if snapshot.policy_digest.trim().is_empty() {
        return Err(ToolPolicyError::InvalidSnapshot("policy_digest"));
    }
    if snapshot.revision.trim().is_empty() {
        return Err(ToolPolicyError::InvalidSnapshot("revision"));
    }
    let identity = &run.identity;
    if snapshot.run_id != identity.run_id
        || snapshot.thread_id != identity.thread_id
        || snapshot.turn_id != identity.turn_id
        || snapshot.actor_id != identity.actor_id
        || snapshot.scope_id != identity.scope_id
    {
        return Err(ToolPolicyError::SnapshotRunMismatch);
    }
    if snapshot
        .expires_at_ms
        .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
    {
        return Err(ToolPolicyError::SnapshotExpired);
    }
    if snapshot.disabled_tools.len() > MAX_POLICY_ITEMS
        || snapshot.denied_capabilities.len() > MAX_POLICY_ITEMS
    {
        return Err(ToolPolicyError::TooManyEntries);
    }
    if snapshot
        .disabled_tools
        .iter()
        .chain(snapshot.denied_capabilities.iter())
        .any(|value| {
            value.trim().is_empty()
                || value.len() > MAX_POLICY_VALUE_BYTES
                || value.chars().any(char::is_control)
        })
    {
        return Err(ToolPolicyError::InvalidPolicyEntry);
    }
    Ok(())
}

pub fn validate_approval_grant(grant: &ApprovalGrant, now_ms: u64) -> Result<(), ToolPolicyError> {
    if grant.contract_version != TOOL_POLICY_CONTRACT_VERSION
        || grant.approved_by.trim().is_empty()
        || grant.binding_digest.trim().is_empty()
        || grant.issued_at_ms > now_ms
        || grant.expires_at_ms <= now_ms
        || grant.expires_at_ms <= grant.issued_at_ms
    {
        return Err(ToolPolicyError::InvalidApproval);
    }
    Ok(())
}

fn approval_required(mode: ApprovalMode, effect: ToolEffect) -> bool {
    match mode {
        ApprovalMode::RequireAllEffects => effect != ToolEffect::ReadOnly,
        ApprovalMode::RequireDestructive => effect == ToolEffect::Destructive,
        ApprovalMode::CapabilityOnly => false,
    }
}

fn make_approval_request(
    run: &RunLifecycle,
    call: &ToolCall,
    contract: &ToolContract,
    snapshot: &PolicySnapshot,
) -> ApprovalRequest {
    let arguments = match &call.arguments {
        ToolArguments::Object { value } => Value::Object(
            value
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        ToolArguments::InvalidJson { raw } => Value::String(raw.clone()),
    };
    let arguments_digest = digest_value(&arguments);
    let binding = serde_json::json!({
        "contract_version": TOOL_POLICY_CONTRACT_VERSION,
        "run_id": run.identity.run_id,
        "thread_id": run.identity.thread_id,
        "turn_id": run.identity.turn_id,
        "call_id": call.id,
        "tool_name": call.name,
        "scope_id": snapshot.scope_id,
        "policy_digest": snapshot.policy_digest,
        "arguments_digest": arguments_digest,
        "required_capability": contract.required_capability,
        "effect": contract.effect,
        "schema_version": contract.schema_version,
    });
    let binding_digest = digest_value(&binding);
    ApprovalRequest {
        contract_version: TOOL_POLICY_CONTRACT_VERSION,
        run_id: run.identity.run_id.clone(),
        thread_id: run.identity.thread_id.clone(),
        turn_id: run.identity.turn_id.clone(),
        call_id: call.id.clone(),
        tool_name: call.name.clone(),
        scope_id: snapshot.scope_id.clone(),
        policy_digest: snapshot.policy_digest.clone(),
        arguments_digest,
        binding_digest,
    }
}

fn digest_value(value: &Value) -> String {
    let mut hasher = Hasher::new();
    hasher.update(b"vectora-engine:tool-approval:v1\0");
    hasher.update(canonical_json(value).as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("string is serializable"),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let ordered: BTreeMap<_, _> = values.iter().collect();
            format!(
                "{{{}}}",
                ordered
                    .iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("key is serializable"),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{CHAT_CONTRACT_VERSION, FinishReason};
    use crate::harness::{RUN_CONTRACT_VERSION, RunIdentity, RunStatus};
    use serde_json::json;

    fn active_run() -> RunLifecycle {
        let mut run = RunLifecycle::new(RunIdentity {
            contract_version: RUN_CONTRACT_VERSION,
            run_id: "run-1".into(),
            idempotency_key: "key-1".into(),
            thread_id: "thread-1".into(),
            turn_id: "turn-1".into(),
            actor_id: Some("actor-1".into()),
            scope_id: Some("workspace-1".into()),
            provider: None,
            model: None,
            capabilities: vec!["file.write".into(), "file.read".into()],
        })
        .unwrap();
        run.transition(RunStatus::Running).unwrap();
        run
    }

    fn contract(effect: ToolEffect) -> ToolContract {
        ToolContract {
            contract_version: super::super::TOOL_CONTRACT_VERSION,
            schema_version: "write.v1".into(),
            name: "write_file".into(),
            description: None,
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
            required_capability: "file.write".into(),
            timeout_ms: 5000,
            effect,
            idempotency: super::super::ToolIdempotency::RequiresKey,
        }
    }

    fn completion(path: &str) -> ChatCompletion {
        ChatCompletion {
            contract_version: CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: None,
            content: String::new(),
            reasoning: None,
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "write_file".into(),
                arguments: ToolArguments::Object {
                    value: [("path".into(), Value::String(path.into()))]
                        .into_iter()
                        .collect(),
                },
            }],
            finish_reason: Some(FinishReason::ToolCalls),
            usage: None,
            media_decisions: Vec::new(),
        }
    }

    fn snapshot() -> PolicySnapshot {
        PolicySnapshot {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            run_id: "run-1".into(),
            thread_id: "thread-1".into(),
            turn_id: "turn-1".into(),
            actor_id: Some("actor-1".into()),
            scope_id: Some("workspace-1".into()),
            policy_digest: "sha256:policy-a".into(),
            revision: "7".into(),
            disabled_tools: Vec::new(),
            denied_capabilities: Vec::new(),
            untrusted_content: false,
            approval_mode: ApprovalMode::RequireAllEffects,
            expires_at_ms: Some(5000),
        }
    }

    fn evaluate(
        run: &RunLifecycle,
        completion: &ChatCompletion,
        contract: &ToolContract,
        snapshot: &PolicySnapshot,
        grants: &[ApprovalGrant],
        now_ms: u64,
    ) -> Result<PolicyEvaluation, ToolPolicyError> {
        authorize_tool_dispatch(
            run,
            completion,
            std::slice::from_ref(contract),
            ToolDispatchSettings::default(),
            snapshot,
            grants,
            now_ms,
        )
    }

    #[test]
    fn requires_approval_and_withholds_dispatch_until_receipt_matches() {
        let run = active_run();
        let call = completion("src/main.rs");
        let contract = contract(ToolEffect::ExternalEffect);
        let pending = evaluate(&run, &call, &contract, &snapshot(), &[], 1000).unwrap();
        assert_eq!(pending.status, PolicyEvaluationStatus::AwaitingApproval);
        assert!(pending.dispatch.is_none());
        let ToolAuthorizationDecision::RequireApproval { request } = &pending.decisions[0] else {
            panic!("expected approval request");
        };

        let grant = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 1001,
            expires_at_ms: 2000,
        };
        let ready = evaluate(&run, &call, &contract, &snapshot(), &[grant], 1500).unwrap();
        assert_eq!(ready.status, PolicyEvaluationStatus::Ready);
        assert!(matches!(
            ready.dispatch.unwrap().decisions[0],
            ToolDecision::Invoke { .. }
        ));
    }

    #[test]
    fn changed_arguments_policy_or_turn_invalidates_approval() {
        let run = active_run();
        let contract = contract(ToolEffect::Destructive);
        let original = completion("src/main.rs");
        let pending = evaluate(&run, &original, &contract, &snapshot(), &[], 1000).unwrap();
        let ToolAuthorizationDecision::RequireApproval { request } = &pending.decisions[0] else {
            panic!("expected approval request");
        };
        let grant = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };
        assert!(matches!(
            evaluate(
                &run,
                &completion("src/other.rs"),
                &contract,
                &snapshot(),
                std::slice::from_ref(&grant),
                1500
            ),
            Err(ToolPolicyError::UnusedApproval)
        ));
        let mut changed_policy = snapshot();
        changed_policy.policy_digest = "sha256:policy-b".into();
        assert!(matches!(
            evaluate(&run, &original, &contract, &changed_policy, &[grant], 1500),
            Err(ToolPolicyError::UnusedApproval)
        ));
    }

    #[test]
    fn disabled_tools_and_untrusted_side_effects_fail_closed() {
        let run = active_run();
        let completion = completion("src/main.rs");
        let contract = contract(ToolEffect::ExternalEffect);
        let mut policy = snapshot();
        policy.disabled_tools = vec!["write_file".into()];
        let result = evaluate(&run, &completion, &contract, &policy, &[], 1000).unwrap();
        assert!(matches!(
            result.decisions[0],
            ToolAuthorizationDecision::Deny {
                code: ToolDenialCode::PolicyToolDisabled,
                ..
            }
        ));
        assert_eq!(result.status, PolicyEvaluationStatus::Ready);
        assert!(result.dispatch.is_some());

        policy.disabled_tools.clear();
        policy.untrusted_content = true;
        let result = evaluate(&run, &completion, &contract, &policy, &[], 1000).unwrap();
        assert!(matches!(
            result.decisions[0],
            ToolAuthorizationDecision::Deny {
                code: ToolDenialCode::UntrustedContentEffect,
                ..
            }
        ));
    }

    #[test]
    fn policy_snapshot_is_bound_to_run_scope_and_expiry() {
        let run = active_run();
        let call = completion("src/main.rs");
        let contract = contract(ToolEffect::ReadOnly);
        let mut policy = snapshot();
        policy.scope_id = Some("different-scope".into());
        assert_eq!(
            evaluate(&run, &call, &contract, &policy, &[], 1000),
            Err(ToolPolicyError::SnapshotRunMismatch)
        );
        policy = snapshot();
        assert_eq!(
            evaluate(&run, &call, &contract, &policy, &[], 5000),
            Err(ToolPolicyError::SnapshotExpired)
        );
    }

    #[test]
    fn canonical_argument_digest_does_not_depend_on_object_key_order() {
        let a = json!({"outer":{"b":2,"a":1},"list":[{"z":0,"y":1}]});
        let b: Value =
            serde_json::from_str(r#"{"list":[{"y":1,"z":0}],"outer":{"a":1,"b":2}}"#).unwrap();
        assert_eq!(digest_value(&a), digest_value(&b));
    }

    #[test]
    fn rejects_expired_or_unused_approval_receipts() {
        let run = active_run();
        let call = completion("src/main.rs");
        let contract = contract(ToolEffect::Destructive);
        let pending = evaluate(&run, &call, &contract, &snapshot(), &[], 1000).unwrap();
        let ToolAuthorizationDecision::RequireApproval { request } = &pending.decisions[0] else {
            panic!("expected approval request");
        };
        let expired = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 1000,
            expires_at_ms: 1000,
        };
        assert_eq!(
            evaluate(&run, &call, &contract, &snapshot(), &[expired], 1000),
            Err(ToolPolicyError::InvalidApproval)
        );
    }
}
