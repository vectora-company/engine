//! SQLite-backed persistence for model/tool loop transitions.
//!
//! A transition is committed to the event ledger and checkpoint before its
//! adapter-facing action is returned to the host.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    chat::ChatCompletion,
    harness::{
        EVIDENCE_GATE_CONTRACT_VERSION, EvidenceDispositionPolicy, GateEvaluation,
        ModelToolLoopAction, ModelToolLoopError, ModelToolLoopPhase, ModelToolLoopSettings,
        ModelToolLoopState, ModelToolLoopTransition, ToolDispatch, ToolResult, VerificationError,
        VerificationReport, advance_model_turn, advance_tool_result, cancel_model_tool_loop,
        evaluate_verification, reconcile_tool_result, recover_model_tool_loop,
        validate_model_tool_loop_state,
    },
};

use super::sqlite::{EventDraft, LedgerError, SqliteLedger};

pub const SQLITE_LOOP_STORE_CONTRACT_VERSION: u16 = 2;
const MAX_OPERATION_ID_BYTES: usize = 256;

/// Durable envelope carrying the event sequence needed for optimistic writes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableModelToolLoopState {
    pub contract_version: u16,
    pub scope: String,
    pub ledger_sequence: u64,
    pub loop_state: ModelToolLoopState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<DurableLoopVerificationRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableLoopVerificationRecord {
    pub operation_id: String,
    pub report: VerificationReport,
    pub policy: EvidenceDispositionPolicy,
    pub evaluation: GateEvaluation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DurableModelToolLoopTransition {
    pub state: DurableModelToolLoopState,
    pub action: ModelToolLoopAction,
}

#[derive(Debug, Error)]
pub enum SqliteModelToolLoopError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Loop(#[from] ModelToolLoopError),
    #[error("invalid durable model/tool loop state")]
    InvalidState,
    #[error("unsupported durable loop store contract version: {0}")]
    UnsupportedContractVersion(u16),
    #[error("invalid loop operation identifier")]
    InvalidOperationId,
    #[error("durable loop sequence overflow")]
    SequenceOverflow,
    #[error("durable loop serialization failed")]
    Serialization,
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("verification report run ID does not match the loop run ID")]
    VerificationRunMismatch,
    #[error("model/tool loop must be complete before it can be verified")]
    VerificationRequiresCompletedLoop,
    #[error("verification operation ID was already used with different input")]
    VerificationOperationConflict,
    #[error("verification has already been recorded for this model/tool loop")]
    VerificationAlreadyRecorded,
    #[error("persisted loop verification does not match its report, policy, or state")]
    InvalidVerificationRecord,
}

pub struct SqliteModelToolLoopStore {
    scope: String,
    ledger: SqliteLedger,
}

impl SqliteModelToolLoopStore {
    pub fn open(
        path: impl AsRef<std::path::Path>,
        scope: impl Into<String>,
    ) -> Result<Self, SqliteModelToolLoopError> {
        let scope = scope.into();
        if scope.trim().is_empty() {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        Ok(Self {
            scope,
            ledger: SqliteLedger::open_persistent(path)?,
        })
    }

    /// Persist a new loop before the host starts requesting model output.
    pub fn start(
        &mut self,
        run_id: impl Into<String>,
        turn_id: impl Into<String>,
        settings: ModelToolLoopSettings,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopState, SqliteModelToolLoopError> {
        validate_operation_id(operation_id)?;
        let loop_state = ModelToolLoopState::new(run_id, turn_id, settings)?;
        let state = DurableModelToolLoopState {
            contract_version: SQLITE_LOOP_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: 1,
            loop_state,
            verification: None,
        };
        let payload = json!({
            "contract_version": SQLITE_LOOP_STORE_CONTRACT_VERSION,
            "operation_id": operation_id,
            "run_id": state.loop_state.run_id,
            "turn_id": state.loop_state.turn_id,
        });
        self.persist(state, operation_id, "model_tool_loop_started", payload)
    }

    pub fn load_latest(
        &self,
        run_id: &str,
        turn_id: &str,
    ) -> Result<Option<DurableModelToolLoopState>, SqliteModelToolLoopError> {
        let aggregate = loop_aggregate_id(run_id, turn_id);
        let Some(checkpoint) = self.ledger.latest_checkpoint(&self.scope, &aggregate)? else {
            return Ok(None);
        };
        let state: DurableModelToolLoopState = serde_json::from_value(checkpoint.state)
            .map_err(|_| SqliteModelToolLoopError::Serialization)?;
        self.validate_state(&state)?;
        if state.loop_state.run_id != run_id
            || state.loop_state.turn_id != turn_id
            || checkpoint.aggregate != aggregate
            || state.ledger_sequence != checkpoint.sequence
        {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        Ok(Some(state))
    }

    pub fn accept_model_turn(
        &mut self,
        state: &DurableModelToolLoopState,
        completion: ChatCompletion,
        dispatch: ToolDispatch,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let transition = advance_model_turn(&state.loop_state, completion, dispatch)?;
        self.persist_transition(state, transition, operation_id, "model_turn_accepted")
    }

    pub fn submit_tool_result(
        &mut self,
        state: &DurableModelToolLoopState,
        result: ToolResult,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let transition = advance_tool_result(&state.loop_state, result)?;
        self.persist_transition(state, transition, operation_id, "tool_result_recorded")
    }

    /// Persist a cancellation request before the host interrupts pending tasks.
    pub fn cancel(
        &mut self,
        state: &DurableModelToolLoopState,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let transition = cancel_model_tool_loop(&state.loop_state)?;
        let event_type = if transition.state.phase == ModelToolLoopPhase::Cancelled {
            "model_tool_loop_cancelled"
        } else {
            "model_tool_loop_cancel_requested"
        };
        self.persist_transition(state, transition, operation_id, event_type)
    }

    pub fn recover(
        &mut self,
        state: &DurableModelToolLoopState,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let transition = recover_model_tool_loop(&state.loop_state)?;
        self.persist_transition(state, transition, operation_id, "loop_recovery_required")
    }

    pub fn reconcile_tool_result(
        &mut self,
        state: &DurableModelToolLoopState,
        result: ToolResult,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        let transition = reconcile_tool_result(&state.loop_state, result)?;
        self.persist_transition(state, transition, operation_id, "tool_result_reconciled")
    }

    /// Evaluate and persist a gate only after the durable model/tool loop completes.
    pub fn verify(
        &mut self,
        state: &DurableModelToolLoopState,
        report: VerificationReport,
        policy: EvidenceDispositionPolicy,
        operation_id: &str,
    ) -> Result<DurableModelToolLoopState, SqliteModelToolLoopError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        if report.run_id != state.loop_state.run_id {
            return Err(SqliteModelToolLoopError::VerificationRunMismatch);
        }
        if state.loop_state.phase != ModelToolLoopPhase::Completed {
            return Err(SqliteModelToolLoopError::VerificationRequiresCompletedLoop);
        }
        let evaluation = evaluate_verification(&report, &policy)?;

        let latest = self
            .load_latest(&state.loop_state.run_id, &state.loop_state.turn_id)?
            .ok_or(SqliteModelToolLoopError::InvalidState)?;
        if latest != *state {
            if let Some(prior) = latest.verification.as_ref()
                && prior.operation_id == operation_id
            {
                if prior.report == report && prior.policy == policy {
                    return Ok(latest);
                }
                return Err(SqliteModelToolLoopError::VerificationOperationConflict);
            }
            return Err(SqliteModelToolLoopError::Ledger(
                LedgerError::VersionConflict,
            ));
        }
        if let Some(prior) = state.verification.as_ref() {
            if prior.operation_id == operation_id {
                if prior.report == report && prior.policy == policy {
                    return Ok(state.clone());
                }
                return Err(SqliteModelToolLoopError::VerificationOperationConflict);
            }
            return Err(SqliteModelToolLoopError::VerificationAlreadyRecorded);
        }

        let record = DurableLoopVerificationRecord {
            operation_id: operation_id.to_owned(),
            report,
            policy,
            evaluation,
        };
        let mut next = state.clone();
        next.contract_version = SQLITE_LOOP_STORE_CONTRACT_VERSION;
        next.ledger_sequence = next
            .ledger_sequence
            .checked_add(1)
            .ok_or(SqliteModelToolLoopError::SequenceOverflow)?;
        next.verification = Some(record);
        let payload =
            serde_json::to_value(&next).map_err(|_| SqliteModelToolLoopError::Serialization)?;
        self.persist(next, operation_id, "model_tool_loop_verified", payload)
    }

    fn persist_transition(
        &mut self,
        prior: &DurableModelToolLoopState,
        transition: ModelToolLoopTransition,
        operation_id: &str,
        event_type: &str,
    ) -> Result<DurableModelToolLoopTransition, SqliteModelToolLoopError> {
        let mut state = DurableModelToolLoopState {
            contract_version: SQLITE_LOOP_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: prior
                .ledger_sequence
                .checked_add(1)
                .ok_or(SqliteModelToolLoopError::SequenceOverflow)?,
            loop_state: transition.state,
            verification: prior.verification.clone(),
        };
        let payload = serde_json::to_value(&DurableModelToolLoopTransition {
            state: state.clone(),
            action: transition.action.clone(),
        })
        .map_err(|_| SqliteModelToolLoopError::Serialization)?;
        state = self.persist(state, operation_id, event_type, payload)?;
        Ok(DurableModelToolLoopTransition {
            state,
            action: transition.action,
        })
    }

    fn persist(
        &mut self,
        state: DurableModelToolLoopState,
        operation_id: &str,
        event_type: &str,
        payload: Value,
    ) -> Result<DurableModelToolLoopState, SqliteModelToolLoopError> {
        validate_operation_id(operation_id)?;
        if state.contract_version != SQLITE_LOOP_STORE_CONTRACT_VERSION || state.scope != self.scope
        {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        validate_model_tool_loop_state(&state.loop_state)?;
        let expected_version = state
            .ledger_sequence
            .checked_sub(1)
            .ok_or(SqliteModelToolLoopError::InvalidState)?;
        let aggregate = loop_aggregate_id(&state.loop_state.run_id, &state.loop_state.turn_id);
        let event_id = format!("{}:{operation_id}", aggregate);
        let checkpoint_id = format!("loop:{operation_id}");
        let draft = EventDraft {
            scope: self.scope.clone(),
            aggregate,
            event_id,
            idempotency_key: operation_id.to_owned(),
            event_type: event_type.to_owned(),
            payload,
            expected_version,
        };
        let state_value =
            serde_json::to_value(&state).map_err(|_| SqliteModelToolLoopError::Serialization)?;
        let saved = self.ledger.append_checkpointed_with_outbox(
            draft,
            checkpoint_id,
            state_value.clone(),
        )?;
        if saved.event.sequence != state.ledger_sequence
            || saved.checkpoint.sequence != state.ledger_sequence
            || saved.checkpoint.state != state_value
        {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        Ok(state)
    }

    fn validate_state(
        &self,
        state: &DurableModelToolLoopState,
    ) -> Result<(), SqliteModelToolLoopError> {
        // Version 1 snapshots predate the optional verification record. They
        // remain readable and can be upgraded by their next persisted write.
        if !matches!(
            state.contract_version,
            1 | SQLITE_LOOP_STORE_CONTRACT_VERSION
        ) {
            return Err(SqliteModelToolLoopError::UnsupportedContractVersion(
                state.contract_version,
            ));
        }
        if state.contract_version == 1 && state.verification.is_some() {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        if state.scope != self.scope || state.ledger_sequence == 0 {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        validate_model_tool_loop_state(&state.loop_state)?;
        if let Some(record) = state.verification.as_ref() {
            validate_operation_id(&record.operation_id)?;
            if state.loop_state.phase != ModelToolLoopPhase::Completed
                || record.report.run_id != state.loop_state.run_id
                || record.evaluation != evaluate_verification(&record.report, &record.policy)?
                || record.evaluation.contract_version != EVIDENCE_GATE_CONTRACT_VERSION
            {
                return Err(SqliteModelToolLoopError::InvalidVerificationRecord);
            }
        }
        Ok(())
    }
}

fn loop_aggregate_id(run_id: &str, turn_id: &str) -> String {
    format!(
        "model-tool-loop:{}:{run_id}{}:{turn_id}",
        run_id.len(),
        turn_id.len()
    )
}

fn validate_operation_id(operation_id: &str) -> Result<(), SqliteModelToolLoopError> {
    if operation_id.trim().is_empty() || operation_id.len() > MAX_OPERATION_ID_BYTES {
        return Err(SqliteModelToolLoopError::InvalidOperationId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::harness::{
        EVIDENCE_GATE_CONTRACT_VERSION, EvidenceDisposition, FailureDisposition,
        MODEL_TOOL_LOOP_CONTRACT_VERSION, ToolExecutionMode, VerificationCheck,
        VerificationOutcome, VerificationReport,
    };

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

    fn database_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "vectora-engine-loop-{}-{}.sqlite",
            std::process::id(),
            NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn empty_completion() -> ChatCompletion {
        ChatCompletion {
            contract_version: crate::chat::CHAT_CONTRACT_VERSION,
            id: None,
            selected_model_id: None,
            content: "done".into(),
            reasoning: None,
            tool_calls: Vec::new(),
            finish_reason: None,
            usage: None,
            media_decisions: Vec::new(),
        }
    }

    fn empty_dispatch() -> ToolDispatch {
        ToolDispatch {
            contract_version: crate::harness::TOOL_DISPATCH_CONTRACT_VERSION,
            run_id: "run-1".into(),
            turn_id: "turn-1".into(),
            execution_mode: ToolExecutionMode::ConcurrentReadOnly,
            decisions: Vec::new(),
        }
    }

    fn verification_report(run_id: &str, outcome: VerificationOutcome) -> VerificationReport {
        VerificationReport {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            run_id: run_id.into(),
            evaluated_at_unix_ms: 100,
            checks: vec![VerificationCheck {
                check_id: "tests".into(),
                outcome,
                evidence: Vec::new(),
            }],
        }
    }

    fn verification_policy() -> EvidenceDispositionPolicy {
        EvidenceDispositionPolicy {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            required_checks: vec!["tests".into()],
            require_evidence: false,
            failed_check: FailureDisposition::Blocked,
            inconclusive: EvidenceDisposition::Unverified,
        }
    }

    fn completed_loop(store: &mut SqliteModelToolLoopStore) -> DurableModelToolLoopState {
        let state = store
            .start(
                "run-verify",
                "turn-verify",
                ModelToolLoopSettings::default(),
                "start-verify",
            )
            .unwrap();
        store
            .accept_model_turn(
                &state,
                empty_completion(),
                ToolDispatch {
                    run_id: "run-verify".into(),
                    turn_id: "turn-verify".into(),
                    ..empty_dispatch()
                },
                "complete-verify",
            )
            .unwrap()
            .state
    }

    #[test]
    fn persists_and_loads_independent_turns_for_one_run() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        store
            .start(
                "run-1",
                "turn-1",
                ModelToolLoopSettings::default(),
                "start-1",
            )
            .unwrap();
        assert_eq!(store.ledger.pending_outbox_count().unwrap(), 1);
        store
            .start(
                "run-1",
                "turn-2",
                ModelToolLoopSettings::default(),
                "start-2",
            )
            .unwrap();
        assert_eq!(store.ledger.pending_outbox_count().unwrap(), 2);

        let first = store.load_latest("run-1", "turn-1").unwrap().unwrap();
        let second = store.load_latest("run-1", "turn-2").unwrap().unwrap();
        assert_eq!(first.loop_state.turn_id, "turn-1");
        assert_eq!(second.loop_state.turn_id, "turn-2");
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn retrying_start_with_same_operation_returns_existing_snapshot() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let first = store
            .start(
                "run-1",
                "turn-1",
                ModelToolLoopSettings::default(),
                "start-1",
            )
            .unwrap();
        let retry = store
            .start(
                "run-1",
                "turn-1",
                ModelToolLoopSettings::default(),
                "start-1",
            )
            .unwrap();
        assert_eq!(first, retry);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cancellation_is_checkpointed_before_returning_to_host() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let initial = store
            .start(
                "run-cancel",
                "turn-cancel",
                ModelToolLoopSettings::default(),
                "start-cancel",
            )
            .unwrap();

        let transition = store.cancel(&initial, "cancel-request").unwrap();
        assert_eq!(
            transition.state.loop_state.phase,
            ModelToolLoopPhase::Cancelled
        );
        assert_eq!(
            transition.state.ledger_sequence,
            initial.ledger_sequence + 1
        );
        assert!(matches!(
            transition.action,
            ModelToolLoopAction::Cancelled { .. }
        ));
        drop(store);

        let store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let restored = store
            .load_latest("run-cancel", "turn-cancel")
            .unwrap()
            .unwrap();
        assert_eq!(restored, transition.state);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_transition_fails_without_overwriting_newer_checkpoint() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let initial = store
            .start(
                "run-1",
                "turn-1",
                ModelToolLoopSettings::default(),
                "start-1",
            )
            .unwrap();
        store
            .accept_model_turn(&initial, empty_completion(), empty_dispatch(), "complete-1")
            .unwrap();

        assert!(matches!(
            store.accept_model_turn(
                &initial,
                empty_completion(),
                empty_dispatch(),
                "stale-complete"
            ),
            Err(SqliteModelToolLoopError::Ledger(
                LedgerError::VersionConflict
            ))
        ));
        let latest = store.load_latest("run-1", "turn-1").unwrap().unwrap();
        assert_eq!(latest.ledger_sequence, 2);
        assert_eq!(
            latest.loop_state.contract_version,
            MODEL_TOOL_LOOP_CONTRACT_VERSION
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_is_gated_by_completed_loop_and_persisted_for_replay() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let initial = store
            .start(
                "run-verify",
                "turn-verify",
                ModelToolLoopSettings::default(),
                "start-verify",
            )
            .unwrap();
        let report = verification_report("run-verify", VerificationOutcome::Passed);
        let policy = verification_policy();
        assert!(matches!(
            store.verify(
                &initial,
                report.clone(),
                policy.clone(),
                "verify-before-complete"
            ),
            Err(SqliteModelToolLoopError::VerificationRequiresCompletedLoop)
        ));
        assert!(matches!(
            store.verify(
                &initial,
                verification_report("another-run", VerificationOutcome::Passed),
                policy.clone(),
                "verify-wrong-run",
            ),
            Err(SqliteModelToolLoopError::VerificationRunMismatch)
        ));

        let completed = store
            .accept_model_turn(
                &initial,
                empty_completion(),
                ToolDispatch {
                    run_id: "run-verify".into(),
                    turn_id: "turn-verify".into(),
                    ..empty_dispatch()
                },
                "complete-verify",
            )
            .unwrap()
            .state;
        let verified = store
            .verify(&completed, report.clone(), policy.clone(), "verify-passed")
            .unwrap();
        assert_eq!(verified.ledger_sequence, completed.ledger_sequence + 1);
        assert_eq!(
            verified.verification.as_ref().unwrap().evaluation.verdict,
            crate::harness::GateVerdict::Passed
        );
        assert_eq!(
            store
                .verify(&completed, report.clone(), policy.clone(), "verify-passed")
                .unwrap(),
            verified
        );
        assert!(matches!(
            store.verify(
                &completed,
                verification_report("run-verify", VerificationOutcome::Failed),
                policy.clone(),
                "verify-passed",
            ),
            Err(SqliteModelToolLoopError::VerificationOperationConflict)
        ));
        drop(store);

        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let restored = store
            .load_latest("run-verify", "turn-verify")
            .unwrap()
            .unwrap();
        assert_eq!(restored, verified);
        assert!(matches!(
            store.verify(&restored, report, policy, "verify-second-operation",),
            Err(SqliteModelToolLoopError::VerificationAlreadyRecorded)
        ));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn loop_verification_records_nonpassing_gate_without_claiming_success() {
        let path = database_path();
        let mut store = SqliteModelToolLoopStore::open(&path, "workspace-1").unwrap();
        let completed = completed_loop(&mut store);
        let verified = store
            .verify(
                &completed,
                verification_report("run-verify", VerificationOutcome::InfraError),
                verification_policy(),
                "verify-infra-error",
            )
            .unwrap();
        let evaluation = &verified.verification.as_ref().unwrap().evaluation;
        assert_eq!(
            evaluation.verification_status,
            crate::harness::VerificationStatus::InfraError
        );
        assert_eq!(evaluation.verdict, crate::harness::GateVerdict::Unverified);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
