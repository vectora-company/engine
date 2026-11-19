//! Versioned execution identity and deterministic run lifecycle primitives.
//!
//! This module is an experimental contract foundation. It does not persist
//! state, reserve idempotency keys, authorize tools, or execute effects.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod events;
mod model_tool_loop;
mod policy;
mod tool_dispatch;
mod verification;

pub use events::{
    CONVERSATION_EVENT_CONTRACT_VERSION, ConversationEvent, ConversationEventError,
    ConversationEventKind, ConversationTerminalStatus, conversation_event_for_run_transition,
};
pub use model_tool_loop::{
    MODEL_TOOL_LOOP_CONTRACT_VERSION, ModelToolLoopAction, ModelToolLoopError, ModelToolLoopPhase,
    ModelToolLoopSettings, ModelToolLoopState, ModelToolLoopStopReason, ModelToolLoopTransition,
    PendingToolCancellation, PendingToolResult, TOOL_RESULT_CONTRACT_VERSION, ToolEffectState,
    ToolResult, ToolResultPart, ToolResultStatus, accept_model_turn, advance_model_turn,
    advance_tool_result, cancel_model_tool_loop, reconcile_tool_result, recover_model_tool_loop,
    submit_tool_result, validate_model_tool_loop_state,
};
pub use policy::{
    ApprovalGrant, ApprovalMode, ApprovalRequest, PolicyEvaluation, PolicyEvaluationStatus,
    PolicySnapshot, TOOL_POLICY_CONTRACT_VERSION, ToolAuthorizationDecision, ToolPolicyError,
    authorize_tool_dispatch, authorize_tool_dispatch_with_identity, validate_approval_grant,
    validate_policy_snapshot,
};
pub use tool_dispatch::{
    TOOL_CONTRACT_VERSION, TOOL_DISPATCH_CONTRACT_VERSION, ToolContract, ToolDecision,
    ToolDenialCode, ToolDispatch, ToolDispatchError, ToolDispatchSettings, ToolEffect,
    ToolExecutionMode, ToolIdempotency, ToolOutputValidationError, prepare_tool_dispatch,
    tool_chat_definitions, validate_tool_result_output,
};
pub use verification::{
    EVIDENCE_GATE_CONTRACT_VERSION, EvidenceDisposition, EvidenceDispositionPolicy,
    FailureDisposition, GateEvaluation, GateVerdict, VerificationCheck, VerificationError,
    VerificationEvidence, VerificationOutcome, VerificationReport, VerificationStatus,
    evaluate_verification,
};

/// Version of the public run lifecycle contract.
pub const RUN_CONTRACT_VERSION: u16 = 1;
/// Version of the host-issued identity and capability grant contract.
pub const HOST_IDENTITY_CONTRACT_VERSION: u16 = 1;
const MAX_HOST_IDENTITY_GRANTS: usize = 1024;

/// A capability granted by the trusted host for one scope and policy revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrant {
    pub capability: String,
    pub scope_id: String,
    pub policy_digest: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Product-agnostic identity assertion supplied by a host that has already
/// authenticated its issuer. Validation here checks structure and freshness;
/// it does not authenticate the host or grant capabilities by itself.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostIdentityContext {
    pub contract_version: u16,
    pub issuer: String,
    pub subject: String,
    pub identity_id: String,
    pub identity_revision: String,
    pub identity_digest: String,
    pub scope_id: String,
    pub policy_digest: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(default)]
    pub grants: Vec<CapabilityGrant>,
}

/// Validate a host-authenticated context immediately before using its grants.
pub fn validate_host_identity_context(
    context: &HostIdentityContext,
    now_ms: u64,
) -> Result<(), HostIdentityError> {
    if context.contract_version != HOST_IDENTITY_CONTRACT_VERSION {
        return Err(HostIdentityError::UnsupportedContractVersion {
            received: context.contract_version,
        });
    }
    for value in [
        &context.issuer,
        &context.subject,
        &context.identity_id,
        &context.identity_revision,
        &context.identity_digest,
        &context.scope_id,
        &context.policy_digest,
    ] {
        if value.trim().is_empty() {
            return Err(HostIdentityError::EmptyField);
        }
    }
    if context.issued_at_ms > now_ms || context.expires_at_ms <= now_ms {
        return Err(HostIdentityError::ExpiredOrNotYetValid);
    }
    if context.grants.len() > MAX_HOST_IDENTITY_GRANTS {
        return Err(HostIdentityError::TooManyGrants);
    }
    let mut seen = std::collections::HashSet::with_capacity(context.grants.len());
    for grant in &context.grants {
        if grant.capability.trim().is_empty()
            || grant.scope_id != context.scope_id
            || grant.policy_digest != context.policy_digest
            || grant.issued_at_ms > now_ms
            || grant.expires_at_ms <= now_ms
            || grant.expires_at_ms > context.expires_at_ms
            || !seen.insert(grant.capability.as_str())
        {
            return Err(HostIdentityError::InvalidGrant);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum HostIdentityError {
    #[error("unsupported host identity contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("host identity context contains an empty required field")]
    EmptyField,
    #[error("host identity context is expired or not yet valid")]
    ExpiredOrNotYetValid,
    #[error("host identity capability grant is invalid")]
    InvalidGrant,
    #[error("host identity context contains too many capability grants")]
    TooManyGrants,
}

/// Product-agnostic identity and context supplied by a consumer.
///
/// Validation checks shape and version only; it does not authenticate this
/// data or authorize any capability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunIdentity {
    pub contract_version: u16,
    pub run_id: String,
    pub idempotency_key: String,
    pub thread_id: String,
    pub turn_id: String,
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl RunIdentity {
    fn validate(&self) -> Result<(), RunError> {
        if self.contract_version != RUN_CONTRACT_VERSION {
            return Err(RunError::UnsupportedContractVersion {
                received: self.contract_version,
            });
        }
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("idempotency_key", self.idempotency_key.as_str()),
            ("thread_id", self.thread_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(RunError::EmptyRequiredField(field));
            }
        }
        if self
            .capabilities
            .iter()
            .any(|capability| capability.trim().is_empty())
        {
            return Err(RunError::EmptyCapability);
        }
        Ok(())
    }
}

/// Lifecycle states represented by the experimental run contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Planned,
    Running,
    WaitingApproval,
    WaitingProvider,
    ToolRunning,
    Verifying,
    Completed,
    Degraded,
    Failed,
    Cancelled,
    RecoveryRequired,
}

impl RunStatus {
    fn can_transition_to(self, next: Self) -> bool {
        use RunStatus as S;
        matches!(
            (self, next),
            (S::Planned, S::Running | S::Cancelled)
                | (
                    S::Running,
                    S::WaitingApproval
                        | S::WaitingProvider
                        | S::ToolRunning
                        | S::Verifying
                        | S::Degraded
                        | S::Failed
                        | S::Cancelled
                        | S::RecoveryRequired
                )
                | (
                    S::WaitingApproval,
                    S::Running | S::Cancelled | S::RecoveryRequired
                )
                | (
                    S::WaitingProvider,
                    S::Running | S::Failed | S::Cancelled | S::RecoveryRequired
                )
                | (
                    S::ToolRunning,
                    S::Running
                        | S::WaitingApproval
                        | S::Verifying
                        | S::Failed
                        | S::Cancelled
                        | S::RecoveryRequired
                )
                | (
                    S::Verifying,
                    S::Completed | S::Degraded | S::Failed | S::Cancelled | S::RecoveryRequired
                )
                | (S::RecoveryRequired, S::Running | S::Failed | S::Cancelled)
        )
    }
}

/// Current lifecycle snapshot. Persist this atomically in the consumer before
/// relying on it across process restarts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunLifecycle {
    pub contract_version: u16,
    pub identity: RunIdentity,
    pub status: RunStatus,
    pub sequence: u64,
}

impl RunLifecycle {
    /// Create a validated run snapshot in the planned state.
    pub fn new(identity: RunIdentity) -> Result<Self, RunError> {
        identity.validate()?;
        Ok(Self {
            contract_version: RUN_CONTRACT_VERSION,
            identity,
            status: RunStatus::Planned,
            sequence: 0,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), RunError> {
        if self.contract_version != RUN_CONTRACT_VERSION {
            return Err(RunError::UnsupportedContractVersion {
                received: self.contract_version,
            });
        }
        self.identity.validate()
    }

    /// Apply a valid state transition; repeating the current state is a no-op.
    pub fn transition(&mut self, next: RunStatus) -> Result<Option<RunTransition>, RunError> {
        self.validate()?;
        if next == self.status {
            return Ok(None);
        }
        if !self.status.can_transition_to(next) {
            return Err(RunError::InvalidTransition {
                from: self.status,
                to: next,
            });
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(RunError::SequenceOverflow)?;
        let transition = RunTransition {
            contract_version: RUN_CONTRACT_VERSION,
            run_id: self.identity.run_id.clone(),
            sequence,
            from: self.status,
            to: next,
        };
        self.status = next;
        self.sequence = transition.sequence;
        Ok(Some(transition))
    }
}

/// A deterministic, ordered lifecycle transition without wall-clock data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunTransition {
    pub contract_version: u16,
    pub run_id: String,
    pub sequence: u64,
    pub from: RunStatus,
    pub to: RunStatus,
}

/// Errors returned by run identity validation and lifecycle transitions.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RunError {
    #[error("unsupported run contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("required run identity field {0} is empty")]
    EmptyRequiredField(&'static str),
    #[error("run capabilities must not contain empty values")]
    EmptyCapability,
    #[error("invalid run transition from {from:?} to {to:?}")]
    InvalidTransition { from: RunStatus, to: RunStatus },
    #[error("run lifecycle sequence overflow")]
    SequenceOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> RunIdentity {
        RunIdentity {
            contract_version: RUN_CONTRACT_VERSION,
            run_id: "run-1".into(),
            idempotency_key: "turn-1".into(),
            thread_id: "thread-1".into(),
            turn_id: "turn-1".into(),
            actor_id: None,
            scope_id: None,
            provider: Some("test-provider".into()),
            model: Some("test-model".into()),
            capabilities: vec!["chat".into()],
        }
    }

    fn host_context() -> HostIdentityContext {
        HostIdentityContext {
            contract_version: HOST_IDENTITY_CONTRACT_VERSION,
            issuer: "host:test".into(),
            subject: "user-1".into(),
            identity_id: "assistant-1".into(),
            identity_revision: "revision-3".into(),
            identity_digest: "sha256:identity".into(),
            scope_id: "workspace-1".into(),
            policy_digest: "sha256:policy".into(),
            issued_at_ms: 100,
            expires_at_ms: 1000,
            grants: vec![CapabilityGrant {
                capability: "filesystem.read".into(),
                scope_id: "workspace-1".into(),
                policy_digest: "sha256:policy".into(),
                issued_at_ms: 100,
                expires_at_ms: 900,
            }],
        }
    }

    #[test]
    fn validates_fresh_host_identity_and_scoped_grants() {
        let context = host_context();
        validate_host_identity_context(&context, 500).unwrap();
        let encoded = serde_json::to_string(&context).unwrap();
        assert_eq!(
            serde_json::from_str::<HostIdentityContext>(&encoded).unwrap(),
            context
        );
    }

    #[test]
    fn rejects_expired_context_and_mismatched_or_duplicate_grants() {
        assert_eq!(
            validate_host_identity_context(&host_context(), 1000),
            Err(HostIdentityError::ExpiredOrNotYetValid)
        );
        let mut context = host_context();
        context.grants[0].scope_id = "other-workspace".into();
        assert_eq!(
            validate_host_identity_context(&context, 500),
            Err(HostIdentityError::InvalidGrant)
        );
        let mut context = host_context();
        context.grants.push(context.grants[0].clone());
        assert_eq!(
            validate_host_identity_context(&context, 500),
            Err(HostIdentityError::InvalidGrant)
        );
    }

    #[test]
    fn rejects_context_issued_in_the_future_and_unknown_version() {
        let mut context = host_context();
        context.issued_at_ms = 501;
        assert_eq!(
            validate_host_identity_context(&context, 500),
            Err(HostIdentityError::ExpiredOrNotYetValid)
        );
        let mut context = host_context();
        context.contract_version += 1;
        assert!(matches!(
            validate_host_identity_context(&context, 500),
            Err(HostIdentityError::UnsupportedContractVersion { .. })
        ));
    }

    #[test]
    fn rejects_unknown_context_fields() {
        let mut value = serde_json::to_value(host_context()).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<HostIdentityContext>(value).is_err());
    }

    #[test]
    fn bounds_host_identity_grant_count() {
        let mut context = host_context();
        context.grants = (0..=MAX_HOST_IDENTITY_GRANTS)
            .map(|index| CapabilityGrant {
                capability: format!("capability-{index}"),
                scope_id: "workspace-1".into(),
                policy_digest: "sha256:policy".into(),
                issued_at_ms: 100,
                expires_at_ms: 900,
            })
            .collect();
        assert_eq!(
            validate_host_identity_context(&context, 500),
            Err(HostIdentityError::TooManyGrants)
        );
    }

    #[test]
    fn creates_versioned_planned_run_and_advances_sequence() {
        let mut run = RunLifecycle::new(identity()).unwrap();
        assert_eq!(run.status, RunStatus::Planned);
        assert_eq!(run.sequence, 0);
        let event = run.transition(RunStatus::Running).unwrap().unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.from, RunStatus::Planned);
        assert_eq!(event.to, RunStatus::Running);
        assert_eq!(run.sequence, 1);
    }

    #[test]
    fn repeating_current_state_is_idempotent() {
        let mut run = RunLifecycle::new(identity()).unwrap();
        assert_eq!(run.transition(RunStatus::Planned).unwrap(), None);
        assert_eq!(run.sequence, 0);
    }

    #[test]
    fn rejects_invalid_and_terminal_transitions() {
        let mut run = RunLifecycle::new(identity()).unwrap();
        assert!(matches!(
            run.transition(RunStatus::Completed),
            Err(RunError::InvalidTransition { .. })
        ));
        run.transition(RunStatus::Running).unwrap();
        run.transition(RunStatus::Verifying).unwrap();
        run.transition(RunStatus::Completed).unwrap();
        assert!(matches!(
            run.transition(RunStatus::Running),
            Err(RunError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn rejects_wrong_contract_version_and_missing_identity() {
        let mut invalid = identity();
        invalid.contract_version += 1;
        assert!(matches!(
            RunLifecycle::new(invalid),
            Err(RunError::UnsupportedContractVersion { .. })
        ));
        let mut invalid = identity();
        invalid.turn_id.clear();
        assert_eq!(
            RunLifecycle::new(invalid),
            Err(RunError::EmptyRequiredField("turn_id"))
        );
    }

    #[test]
    fn serializes_status_names_as_stable_snake_case() {
        assert_eq!(
            serde_json::to_string(&RunStatus::WaitingApproval).unwrap(),
            "\"waiting_approval\""
        );
    }
}
