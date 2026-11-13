//! Durable consumer policy snapshots and approval receipts for one run turn.
//!
//! This store records policy inputs and host-validated grants. It does not
//! authenticate the host or approver, and it never executes tool adapters.

use std::{collections::HashSet, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::{
    chat::ChatCompletion,
    harness::{
        ApprovalGrant, ApprovalRequest, PolicyEvaluation, PolicySnapshot, RunLifecycle,
        TOOL_POLICY_CONTRACT_VERSION, ToolContract, ToolDispatchSettings, ToolPolicyError,
        authorize_tool_dispatch, validate_approval_grant, validate_policy_snapshot,
    },
};

use super::sqlite::{EventDraft, LedgerError, SqliteLedger};

pub const SQLITE_TOOL_POLICY_STORE_CONTRACT_VERSION: u16 = 1;
const MAX_OPERATION_ID_BYTES: usize = 256;
const MAX_STATE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableToolPolicyState {
    pub contract_version: u16,
    pub scope: String,
    pub ledger_sequence: u64,
    pub snapshot: PolicySnapshot,
    pub approval_requests: Vec<ApprovalRequest>,
    pub approval_grants: Vec<ApprovalGrant>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DurablePolicyEvaluation {
    pub state: DurableToolPolicyState,
    pub evaluation: PolicyEvaluation,
}

#[derive(Debug, Error)]
pub enum SqliteToolPolicyStoreError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Policy(#[from] ToolPolicyError),
    #[error(transparent)]
    Run(#[from] crate::harness::RunError),
    #[error("invalid durable tool policy state")]
    InvalidState,
    #[error("unsupported durable tool policy store contract version: {0}")]
    UnsupportedContractVersion(u16),
    #[error("invalid tool policy operation identifier")]
    InvalidOperationId,
    #[error("durable tool policy sequence overflow")]
    SequenceOverflow,
    #[error("durable tool policy state exceeds the storage size limit")]
    StateTooLarge,
    #[error("the policy snapshot changed without changing its digest or revision")]
    PolicyDigestReuse,
    #[error("approval request is not pending for this policy snapshot")]
    ApprovalRequestMismatch,
    #[error("approval binding conflicts with a previously stored grant")]
    ApprovalConflict,
}

pub struct SqliteToolPolicyStore {
    scope: String,
    ledger: SqliteLedger,
}

impl SqliteToolPolicyStore {
    pub fn open(
        path: impl AsRef<Path>,
        scope: impl Into<String>,
    ) -> Result<Self, SqliteToolPolicyStoreError> {
        let scope = scope.into();
        if scope.trim().is_empty() {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        Ok(Self {
            scope,
            ledger: SqliteLedger::open_persistent(path)?,
        })
    }

    pub fn load_latest(
        &self,
        run_id: &str,
        turn_id: &str,
    ) -> Result<Option<DurableToolPolicyState>, SqliteToolPolicyStoreError> {
        let aggregate = policy_aggregate_id(run_id, turn_id);
        let Some(checkpoint) = self.ledger.latest_checkpoint(&self.scope, &aggregate)? else {
            return Ok(None);
        };
        let state: DurableToolPolicyState = serde_json::from_value(checkpoint.state)
            .map_err(|_| SqliteToolPolicyStoreError::InvalidState)?;
        self.validate_state(&state)?;
        if state.snapshot.run_id != run_id
            || state.snapshot.turn_id != turn_id
            || checkpoint.aggregate != aggregate
            || state.ledger_sequence != checkpoint.sequence
        {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        Ok(Some(state))
    }

    /// Persist the host's freshly compiled policy before evaluating tools.
    /// Any change under an unchanged policy digest and revision is rejected.
    pub fn save_snapshot(
        &mut self,
        run: &RunLifecycle,
        snapshot: PolicySnapshot,
        operation_id: &str,
        now_ms: u64,
    ) -> Result<DurableToolPolicyState, SqliteToolPolicyStoreError> {
        validate_operation_id(operation_id)?;
        run.validate()?;
        validate_policy_snapshot(run, &snapshot, now_ms)?;
        let prior = self.load_latest(&snapshot.run_id, &snapshot.turn_id)?;

        if let Some(prior) = prior {
            if prior.snapshot == snapshot {
                return Ok(prior);
            }
            if prior.snapshot.policy_digest == snapshot.policy_digest
                && prior.snapshot.revision == snapshot.revision
            {
                return Err(SqliteToolPolicyStoreError::PolicyDigestReuse);
            }
            let state = DurableToolPolicyState {
                contract_version: SQLITE_TOOL_POLICY_STORE_CONTRACT_VERSION,
                scope: self.scope.clone(),
                ledger_sequence: prior
                    .ledger_sequence
                    .checked_add(1)
                    .ok_or(SqliteToolPolicyStoreError::SequenceOverflow)?,
                snapshot,
                approval_requests: Vec::new(),
                approval_grants: Vec::new(),
            };
            return self.persist(state, operation_id, "tool_policy_replaced");
        }

        let state = DurableToolPolicyState {
            contract_version: SQLITE_TOOL_POLICY_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: 1,
            snapshot,
            approval_requests: Vec::new(),
            approval_grants: Vec::new(),
        };
        self.persist(state, operation_id, "tool_policy_saved")
    }

    /// Reevaluate the complete batch against the latest persisted policy and
    /// grants. Approval requests are checkpointed before being returned.
    // Keep the inputs explicit: they are separate versioned engine contracts
    // and should not be hidden inside a loosely coupled options bag.
    #[allow(clippy::too_many_arguments)]
    pub fn authorize(
        &mut self,
        state: &DurableToolPolicyState,
        run: &RunLifecycle,
        completion: &ChatCompletion,
        contracts: &[ToolContract],
        settings: ToolDispatchSettings,
        operation_id: &str,
        now_ms: u64,
    ) -> Result<DurablePolicyEvaluation, SqliteToolPolicyStoreError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        run.validate()?;
        let latest = self
            .load_latest(&state.snapshot.run_id, &state.snapshot.turn_id)?
            .ok_or(SqliteToolPolicyStoreError::InvalidState)?;
        if latest != *state {
            return Err(SqliteToolPolicyStoreError::Ledger(
                LedgerError::VersionConflict,
            ));
        }

        let without_grants = authorize_tool_dispatch(
            run,
            completion,
            contracts,
            settings.clone(),
            &state.snapshot,
            &[],
            now_ms,
        )?;
        let requests = approval_requests(&without_grants);
        let request_digests: HashSet<&str> = requests
            .iter()
            .map(|request| request.binding_digest.as_str())
            .collect();
        let grants: Vec<_> = state
            .approval_grants
            .iter()
            .filter(|grant| {
                grant.expires_at_ms > now_ms
                    && request_digests.contains(grant.binding_digest.as_str())
            })
            .cloned()
            .collect();
        let evaluation = authorize_tool_dispatch(
            run,
            completion,
            contracts,
            settings,
            &state.snapshot,
            &grants,
            now_ms,
        )?;

        let mut next = state.clone();
        let grants_changed = next.approval_grants != grants;
        next.approval_requests = requests;
        next.approval_grants = grants;
        let state = if next.approval_requests != state.approval_requests || grants_changed {
            next.ledger_sequence = state
                .ledger_sequence
                .checked_add(1)
                .ok_or(SqliteToolPolicyStoreError::SequenceOverflow)?;
            self.persist(next, operation_id, "tool_approval_requests_evaluated")?
        } else {
            state.clone()
        };

        Ok(DurablePolicyEvaluation { state, evaluation })
    }

    /// Record one host-authenticated approval receipt for a persisted request.
    /// The engine checks the binding and shape, but cannot authenticate the host.
    pub fn record_approval(
        &mut self,
        state: &DurableToolPolicyState,
        request: &ApprovalRequest,
        grant: ApprovalGrant,
        operation_id: &str,
        now_ms: u64,
    ) -> Result<DurableToolPolicyState, SqliteToolPolicyStoreError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let latest = self
            .load_latest(&state.snapshot.run_id, &state.snapshot.turn_id)?
            .ok_or(SqliteToolPolicyStoreError::InvalidState)?;
        if !state.approval_requests.contains(request)
            || request.contract_version != TOOL_POLICY_CONTRACT_VERSION
            || request.run_id != state.snapshot.run_id
            || request.thread_id != state.snapshot.thread_id
            || request.turn_id != state.snapshot.turn_id
            || request.scope_id != state.snapshot.scope_id
            || request.policy_digest != state.snapshot.policy_digest
            || grant.binding_digest != request.binding_digest
        {
            return Err(SqliteToolPolicyStoreError::ApprovalRequestMismatch);
        }
        validate_approval_grant(&grant, now_ms)?;
        if latest != *state {
            if let Some(prior) = latest
                .approval_grants
                .iter()
                .find(|prior| prior.binding_digest == grant.binding_digest)
            {
                if prior == &grant {
                    return Ok(latest);
                }
                return Err(SqliteToolPolicyStoreError::ApprovalConflict);
            }
            return Err(SqliteToolPolicyStoreError::Ledger(
                LedgerError::VersionConflict,
            ));
        }
        if let Some(prior) = state
            .approval_grants
            .iter()
            .find(|prior| prior.binding_digest == grant.binding_digest)
        {
            if prior == &grant {
                return Ok(state.clone());
            }
            return Err(SqliteToolPolicyStoreError::ApprovalConflict);
        }

        let mut next = state.clone();
        next.ledger_sequence = next
            .ledger_sequence
            .checked_add(1)
            .ok_or(SqliteToolPolicyStoreError::SequenceOverflow)?;
        next.approval_grants.push(grant);
        self.persist(next, operation_id, "tool_approval_granted")
    }

    fn persist(
        &mut self,
        state: DurableToolPolicyState,
        operation_id: &str,
        event_type: &str,
    ) -> Result<DurableToolPolicyState, SqliteToolPolicyStoreError> {
        self.validate_state(&state)?;
        let expected_version = state
            .ledger_sequence
            .checked_sub(1)
            .ok_or(SqliteToolPolicyStoreError::InvalidState)?;
        let aggregate = policy_aggregate_id(&state.snapshot.run_id, &state.snapshot.turn_id);
        let payload =
            serde_json::to_value(&state).map_err(|_| SqliteToolPolicyStoreError::InvalidState)?;
        let draft = EventDraft {
            scope: self.scope.clone(),
            aggregate,
            event_id: format!(
                "{}:{operation_id}",
                policy_aggregate_id(&state.snapshot.run_id, &state.snapshot.turn_id)
            ),
            idempotency_key: operation_id.to_owned(),
            event_type: event_type.to_owned(),
            payload: json!({"state": payload}),
            expected_version,
        };
        let checkpoint_state =
            serde_json::to_value(&state).map_err(|_| SqliteToolPolicyStoreError::InvalidState)?;
        let saved = self.ledger.append_checkpointed(
            draft,
            format!("tool-policy:{operation_id}"),
            checkpoint_state.clone(),
        )?;
        if saved.event.sequence != state.ledger_sequence
            || saved.checkpoint.sequence != state.ledger_sequence
            || saved.checkpoint.state != checkpoint_state
        {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        Ok(state)
    }

    fn validate_state(
        &self,
        state: &DurableToolPolicyState,
    ) -> Result<(), SqliteToolPolicyStoreError> {
        if state.contract_version != SQLITE_TOOL_POLICY_STORE_CONTRACT_VERSION {
            return Err(SqliteToolPolicyStoreError::UnsupportedContractVersion(
                state.contract_version,
            ));
        }
        if state.scope != self.scope
            || state.ledger_sequence == 0
            || state.snapshot.contract_version != TOOL_POLICY_CONTRACT_VERSION
            || state.snapshot.run_id.trim().is_empty()
            || state.snapshot.thread_id.trim().is_empty()
            || state.snapshot.turn_id.trim().is_empty()
            || state.snapshot.policy_digest.trim().is_empty()
            || state.snapshot.revision.trim().is_empty()
        {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        let mut request_digests = HashSet::new();
        if state.approval_requests.iter().any(|request| {
            request.contract_version != TOOL_POLICY_CONTRACT_VERSION
                || request.run_id != state.snapshot.run_id
                || request.thread_id != state.snapshot.thread_id
                || request.turn_id != state.snapshot.turn_id
                || request.scope_id != state.snapshot.scope_id
                || request.policy_digest != state.snapshot.policy_digest
                || request.binding_digest.trim().is_empty()
                || !request_digests.insert(request.binding_digest.as_str())
        }) {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        let mut grant_digests = HashSet::new();
        if state.approval_grants.iter().any(|grant| {
            grant.contract_version != TOOL_POLICY_CONTRACT_VERSION
                || grant.approved_by.trim().is_empty()
                || grant.binding_digest.trim().is_empty()
                || grant.expires_at_ms <= grant.issued_at_ms
                || !request_digests.contains(grant.binding_digest.as_str())
                || !grant_digests.insert(grant.binding_digest.as_str())
        }) {
            return Err(SqliteToolPolicyStoreError::InvalidState);
        }
        let size = serde_json::to_vec(state)
            .map_err(|_| SqliteToolPolicyStoreError::InvalidState)?
            .len();
        if size > MAX_STATE_BYTES {
            return Err(SqliteToolPolicyStoreError::StateTooLarge);
        }
        Ok(())
    }
}

fn approval_requests(evaluation: &PolicyEvaluation) -> Vec<ApprovalRequest> {
    evaluation
        .decisions
        .iter()
        .filter_map(|decision| match decision {
            crate::harness::ToolAuthorizationDecision::RequireApproval { request } => {
                Some(request.clone())
            }
            _ => None,
        })
        .collect()
}

fn validate_operation_id(operation_id: &str) -> Result<(), SqliteToolPolicyStoreError> {
    if operation_id.trim().is_empty()
        || operation_id.len() > MAX_OPERATION_ID_BYTES
        || operation_id.chars().any(char::is_control)
    {
        return Err(SqliteToolPolicyStoreError::InvalidOperationId);
    }
    Ok(())
}

fn policy_aggregate_id(run_id: &str, turn_id: &str) -> String {
    format!(
        "tool-policy:{}:{run_id}:{}:{turn_id}",
        run_id.len(),
        turn_id.len()
    )
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::json;

    use crate::{
        chat::{CHAT_CONTRACT_VERSION, ToolArguments, ToolCall},
        harness::{
            ApprovalMode, RUN_CONTRACT_VERSION, RunIdentity, RunStatus, TOOL_CONTRACT_VERSION,
            ToolEffect,
        },
    };

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

    fn database_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "vectora-engine-policy-{}-{}.sqlite",
            std::process::id(),
            NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

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
            capabilities: vec!["file.write".into()],
        })
        .unwrap();
        run.transition(RunStatus::Running).unwrap();
        run
    }

    fn snapshot() -> PolicySnapshot {
        PolicySnapshot {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            run_id: "run-1".into(),
            thread_id: "thread-1".into(),
            turn_id: "turn-1".into(),
            actor_id: Some("actor-1".into()),
            scope_id: Some("workspace-1".into()),
            policy_digest: "policy-a".into(),
            revision: "rev-a".into(),
            disabled_tools: Vec::new(),
            denied_capabilities: Vec::new(),
            untrusted_content: false,
            approval_mode: ApprovalMode::RequireAllEffects,
            expires_at_ms: Some(10_000),
        }
    }

    fn completion(path: &str) -> ChatCompletion {
        ChatCompletion {
            contract_version: CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: None,
            content: String::new(),
            reasoning: None,
            finish_reason: None,
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "write_file".into(),
                arguments: ToolArguments::from_json(&json!({"path": path}).to_string()),
            }],
            usage: None,
            media_decisions: Vec::new(),
        }
    }

    fn contract() -> ToolContract {
        ToolContract {
            contract_version: TOOL_CONTRACT_VERSION,
            schema_version: "write.v1".into(),
            name: "write_file".into(),
            description: None,
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
            required_capability: "file.write".into(),
            timeout_ms: 1000,
            effect: ToolEffect::ExternalEffect,
            idempotency: crate::harness::ToolIdempotency::RequiresKey,
        }
    }

    #[test]
    fn approval_request_is_checkpointed_then_grant_replays_after_reopen() {
        let path = database_path();
        let run = active_run();
        let mut store = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let policy = store
            .save_snapshot(&run, snapshot(), "policy", 100)
            .unwrap();
        let pending = store
            .authorize(
                &policy,
                &run,
                &completion("src/lib.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "evaluate",
                100,
            )
            .unwrap();
        assert_eq!(
            pending.evaluation.status,
            crate::harness::PolicyEvaluationStatus::AwaitingApproval
        );
        assert_eq!(pending.state.approval_requests.len(), 1);
        let request = pending.state.approval_requests[0].clone();
        let grant = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 100,
            expires_at_ms: 500,
        };
        let approved = store
            .record_approval(&pending.state, &request, grant, "approval", 101)
            .unwrap();
        drop(store);

        let mut store = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let restored = store.load_latest("run-1", "turn-1").unwrap().unwrap();
        assert_eq!(restored, approved);
        let ready = store
            .authorize(
                &restored,
                &run,
                &completion("src/lib.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "reevaluate",
                150,
            )
            .unwrap();
        assert_eq!(
            ready.evaluation.status,
            crate::harness::PolicyEvaluationStatus::Ready
        );
        assert!(ready.evaluation.dispatch.is_some());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn changed_policy_or_arguments_cannot_reuse_a_persisted_grant() {
        let path = database_path();
        let run = active_run();
        let mut store = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let policy = store
            .save_snapshot(&run, snapshot(), "policy", 100)
            .unwrap();
        let pending = store
            .authorize(
                &policy,
                &run,
                &completion("src/lib.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "evaluate",
                100,
            )
            .unwrap();
        let request = pending.state.approval_requests[0].clone();
        let grant = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 100,
            expires_at_ms: 500,
        };
        let approved = store
            .record_approval(&pending.state, &request, grant, "approval", 101)
            .unwrap();

        let changed_arguments = store
            .authorize(
                &approved,
                &run,
                &completion("src/other.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "changed-arguments",
                120,
            )
            .unwrap();
        assert_eq!(
            changed_arguments.evaluation.status,
            crate::harness::PolicyEvaluationStatus::AwaitingApproval
        );
        assert!(changed_arguments.state.approval_grants.is_empty());

        let mut changed_policy = snapshot();
        changed_policy.policy_digest = "policy-b".into();
        changed_policy.revision = "rev-b".into();
        let replaced = store
            .save_snapshot(&run, changed_policy, "policy-update", 130)
            .unwrap();
        assert!(replaced.approval_requests.is_empty());
        assert!(replaced.approval_grants.is_empty());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_approval_retries_are_idempotent_but_invalid_or_expired_grants_fail() {
        let path = database_path();
        let run = active_run();
        let mut store = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let policy = store
            .save_snapshot(&run, snapshot(), "policy", 100)
            .unwrap();
        let pending = store
            .authorize(
                &policy,
                &run,
                &completion("src/lib.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "evaluate",
                100,
            )
            .unwrap();
        let request = pending.state.approval_requests[0].clone();
        let grant = ApprovalGrant {
            contract_version: TOOL_POLICY_CONTRACT_VERSION,
            binding_digest: request.binding_digest.clone(),
            approved_by: "user-1".into(),
            issued_at_ms: 100,
            expires_at_ms: 500,
        };
        let saved = store
            .record_approval(&pending.state, &request, grant.clone(), "grant", 101)
            .unwrap();
        let replay = store
            .record_approval(&pending.state, &request, grant.clone(), "grant", 101)
            .unwrap();
        assert_eq!(replay, saved);

        let mut expired = grant.clone();
        expired.expires_at_ms = 101;
        assert!(matches!(
            store.record_approval(&saved, &request, expired, "expired", 101),
            Err(SqliteToolPolicyStoreError::Policy(
                ToolPolicyError::InvalidApproval
            ))
        ));

        let mut mismatched = request.clone();
        mismatched.binding_digest = "different-binding".into();
        assert!(matches!(
            store.record_approval(&saved, &mismatched, grant, "mismatch", 101),
            Err(SqliteToolPolicyStoreError::ApprovalRequestMismatch)
        ));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn competing_policy_writers_reject_a_stale_snapshot() {
        let path = database_path();
        let run = active_run();
        let mut first = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let policy = first
            .save_snapshot(&run, snapshot(), "policy", 100)
            .unwrap();
        let mut second = SqliteToolPolicyStore::open(&path, "workspace-1").unwrap();
        let first_pending = first
            .authorize(
                &policy,
                &run,
                &completion("src/lib.rs"),
                &[contract()],
                ToolDispatchSettings::default(),
                "evaluate-first",
                100,
            )
            .unwrap();
        let stale_attempt = second.authorize(
            &policy,
            &run,
            &completion("src/lib.rs"),
            &[contract()],
            ToolDispatchSettings::default(),
            "evaluate-stale",
            100,
        );
        assert!(matches!(
            stale_attempt,
            Err(SqliteToolPolicyStoreError::Ledger(
                LedgerError::VersionConflict
            ))
        ));
        drop(first);
        drop(second);
        let _ = first_pending;
        std::fs::remove_file(path).unwrap();
    }
}
