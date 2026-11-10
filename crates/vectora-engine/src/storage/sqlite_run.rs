//! Durable run admission and lifecycle snapshots backed by the SQLite ledger.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::harness::{
    EVIDENCE_GATE_CONTRACT_VERSION, EvidenceDispositionPolicy, GateEvaluation, GateVerdict,
    RUN_CONTRACT_VERSION, RunIdentity, RunLifecycle, RunStatus, RunTransition, VerificationError,
    VerificationReport, evaluate_verification,
};

use super::sqlite::{CheckpointedAdmission, EventDraft, LedgerError, RunReservation, SqliteLedger};

pub const SQLITE_RUN_STORE_CONTRACT_VERSION: u16 = 1;
const MAX_OPERATION_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableRunLifecycleState {
    pub contract_version: u16,
    pub scope: String,
    /// Sequence in the run's ledger aggregate, including the admission event.
    pub ledger_sequence: u64,
    pub lifecycle: RunLifecycle,
    /// Result and policy snapshot that authorized the terminal run status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<DurableVerificationRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableVerificationRecord {
    pub operation_id: String,
    pub report: VerificationReport,
    pub policy: EvidenceDispositionPolicy,
    pub evaluation: GateEvaluation,
    pub transition: RunTransition,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DurableRunAdmission {
    pub reservation: RunReservation,
    pub admission_event: crate::storage::sqlite::LedgerEvent,
    pub state: DurableRunLifecycleState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DurableRunTransition {
    pub state: DurableRunLifecycleState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<RunTransition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DurableRunVerification {
    pub state: DurableRunLifecycleState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<RunTransition>,
}

#[derive(Debug, Error)]
pub enum SqliteRunStoreError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Run(#[from] crate::harness::RunError),
    #[error("invalid durable run lifecycle state")]
    InvalidState,
    #[error("unsupported durable run store contract version: {0}")]
    UnsupportedContractVersion(u16),
    #[error("invalid run operation identifier")]
    InvalidOperationId,
    #[error("durable run sequence overflow")]
    SequenceOverflow,
    #[error("durable run serialization failed")]
    Serialization,
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("verification report run ID does not match lifecycle run ID")]
    VerificationRunMismatch,
    #[error("run lifecycle must be in verifying state to record verification")]
    VerificationRequiresVerifyingState,
    #[error("completed run requires a persisted passing verification")]
    PassingVerificationRequired,
    #[error("verification operation ID was already used with different input")]
    VerificationOperationConflict,
    #[error("persisted verification does not match its report, policy, or lifecycle")]
    InvalidVerificationRecord,
}

pub struct SqliteRunLifecycleStore {
    scope: String,
    ledger: SqliteLedger,
}

impl SqliteRunLifecycleStore {
    pub fn open(
        path: impl AsRef<std::path::Path>,
        scope: impl Into<String>,
    ) -> Result<Self, SqliteRunStoreError> {
        let scope = scope.into();
        if scope.trim().is_empty() {
            return Err(SqliteRunStoreError::InvalidState);
        }
        Ok(Self {
            scope,
            ledger: SqliteLedger::open_persistent(path)?,
        })
    }

    /// Reserve a thread/turn and persist its initial planned lifecycle atomically.
    pub fn admit(
        &mut self,
        identity: RunIdentity,
        request: Value,
    ) -> Result<DurableRunAdmission, SqliteRunStoreError> {
        if identity
            .scope_id
            .as_ref()
            .is_some_and(|identity_scope| identity_scope != &self.scope)
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        let lifecycle = RunLifecycle::new(identity.clone())?;
        let reservation = RunReservation {
            scope: self.scope.clone(),
            thread_id: identity.thread_id.clone(),
            turn_id: identity.turn_id.clone(),
            run_id: identity.run_id.clone(),
            idempotency_key: identity.idempotency_key.clone(),
            request,
        };
        let lifecycle_aggregate = run_aggregate_id(&identity.run_id);
        let state = DurableRunLifecycleState {
            contract_version: SQLITE_RUN_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: 1,
            lifecycle,
            verification: None,
        };
        let state_value =
            serde_json::to_value(&state).map_err(|_| SqliteRunStoreError::Serialization)?;
        let admission_event = EventDraft {
            scope: self.scope.clone(),
            aggregate: identity.thread_id.clone(),
            event_id: format!("run-admitted:{}:{}", identity.run_id.len(), identity.run_id),
            idempotency_key: format!("run-admission:{}", identity.idempotency_key),
            event_type: "run_admitted".into(),
            payload: json!({
                "run_id": identity.run_id,
                "turn_id": identity.turn_id,
                "idempotency_key": identity.idempotency_key,
            }),
            expected_version: 0,
        };
        let lifecycle_event = EventDraft {
            scope: self.scope.clone(),
            aggregate: lifecycle_aggregate.clone(),
            event_id: format!("{lifecycle_aggregate}:admitted"),
            idempotency_key: format!("run-lifecycle-admission:{}", identity.idempotency_key),
            event_type: "run_lifecycle_admitted".into(),
            payload: state_value.clone(),
            expected_version: 0,
        };
        let admitted: CheckpointedAdmission = self.ledger.admit_run_checkpointed(
            reservation,
            admission_event,
            lifecycle_event,
            format!("{lifecycle_aggregate}:checkpoint:1"),
            state_value.clone(),
        )?;
        if admitted.lifecycle.event.sequence != 1
            || admitted.lifecycle.checkpoint.state != state_value
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        Ok(DurableRunAdmission {
            reservation: admitted.reservation,
            admission_event: admitted.admission_event,
            state,
        })
    }

    pub fn load_latest(
        &self,
        run_id: &str,
    ) -> Result<Option<DurableRunLifecycleState>, SqliteRunStoreError> {
        let aggregate = run_aggregate_id(run_id);
        let Some(checkpoint) = self.ledger.latest_checkpoint(&self.scope, &aggregate)? else {
            return Ok(None);
        };
        let state: DurableRunLifecycleState = serde_json::from_value(checkpoint.state)
            .map_err(|_| SqliteRunStoreError::Serialization)?;
        self.validate_state(&state)?;
        if state.lifecycle.identity.run_id != run_id
            || checkpoint.aggregate != aggregate
            || state.ledger_sequence != checkpoint.sequence
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        Ok(Some(state))
    }

    pub fn transition(
        &mut self,
        state: &DurableRunLifecycleState,
        next: RunStatus,
        operation_id: &str,
    ) -> Result<DurableRunTransition, SqliteRunStoreError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        if state.lifecycle.status == RunStatus::Verifying
            && next == RunStatus::Completed
            && !state.verification.as_ref().is_some_and(|record| {
                record.evaluation.verdict == GateVerdict::Passed
                    && record.transition.to == RunStatus::Completed
                    && record.transition.sequence == state.lifecycle.sequence
            })
        {
            return Err(SqliteRunStoreError::PassingVerificationRequired);
        }
        let mut lifecycle = state.lifecycle.clone();
        let transition = lifecycle.transition(next)?;
        let Some(transition) = transition else {
            if self.load_latest(&state.lifecycle.identity.run_id)?.as_ref() != Some(state) {
                return Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict));
            }
            return Ok(DurableRunTransition {
                state: state.clone(),
                transition: None,
            });
        };
        let next_state = DurableRunLifecycleState {
            contract_version: SQLITE_RUN_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: state
                .ledger_sequence
                .checked_add(1)
                .ok_or(SqliteRunStoreError::SequenceOverflow)?,
            lifecycle,
            verification: state.verification.clone(),
        };
        let state_value =
            serde_json::to_value(&next_state).map_err(|_| SqliteRunStoreError::Serialization)?;
        let aggregate = run_aggregate_id(&next_state.lifecycle.identity.run_id);
        let draft = EventDraft {
            scope: self.scope.clone(),
            aggregate: aggregate.clone(),
            event_id: format!("{aggregate}:transition:{operation_id}"),
            idempotency_key: operation_id.into(),
            event_type: "run_lifecycle_transition".into(),
            payload: json!({"transition": transition, "state": state_value}),
            expected_version: state.ledger_sequence,
        };
        let latest = self
            .load_latest(&state.lifecycle.identity.run_id)?
            .ok_or(SqliteRunStoreError::InvalidState)?;
        if latest != *state {
            let aggregate = run_aggregate_id(&state.lifecycle.identity.run_id);
            let replay = self
                .ledger
                .read(&self.scope, &aggregate, state.ledger_sequence, 1)?
                .into_iter()
                .next()
                .is_some_and(|event| {
                    event.sequence == next_state.ledger_sequence
                        && event.scope == draft.scope
                        && event.aggregate == draft.aggregate
                        && event.event_id == draft.event_id
                        && event.idempotency_key == draft.idempotency_key
                        && event.event_type == draft.event_type
                        && event.payload == draft.payload
                });
            if replay {
                return Ok(DurableRunTransition {
                    state: next_state,
                    transition: Some(transition),
                });
            }
            return Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict));
        }
        let saved = self.ledger.append_checkpointed(
            draft,
            format!("{aggregate}:checkpoint:{operation_id}"),
            state_value.clone(),
        )?;
        if saved.event.sequence != next_state.ledger_sequence
            || saved.checkpoint.state != state_value
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        Ok(DurableRunTransition {
            state: next_state,
            transition: Some(transition),
        })
    }

    /// Evaluate and durably record a verification report before applying its
    /// gate verdict to the run lifecycle in the same ledger/checkpoint write.
    pub fn verify(
        &mut self,
        state: &DurableRunLifecycleState,
        report: VerificationReport,
        policy: EvidenceDispositionPolicy,
        operation_id: &str,
    ) -> Result<DurableRunVerification, SqliteRunStoreError> {
        self.validate_state(state)?;
        validate_operation_id(operation_id)?;
        if report.run_id != state.lifecycle.identity.run_id {
            return Err(SqliteRunStoreError::VerificationRunMismatch);
        }
        let evaluation = evaluate_verification(&report, &policy)?;

        let latest = self
            .load_latest(&state.lifecycle.identity.run_id)?
            .ok_or(SqliteRunStoreError::InvalidState)?;
        if latest != *state {
            if let Some(record) = latest.verification.as_ref()
                && record.operation_id == operation_id
            {
                if record.report == report && record.policy == policy {
                    return Ok(DurableRunVerification {
                        state: latest,
                        transition: None,
                    });
                }
                return Err(SqliteRunStoreError::VerificationOperationConflict);
            }
            return Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict));
        }
        if state.lifecycle.status != RunStatus::Verifying {
            return Err(SqliteRunStoreError::VerificationRequiresVerifyingState);
        }
        if state
            .verification
            .as_ref()
            .is_some_and(|record| record.operation_id == operation_id)
        {
            return Err(SqliteRunStoreError::VerificationOperationConflict);
        }

        let next_status = match evaluation.verdict {
            GateVerdict::Passed => RunStatus::Completed,
            GateVerdict::Degraded => RunStatus::Degraded,
            GateVerdict::Blocked => RunStatus::Failed,
            GateVerdict::Unverified => RunStatus::RecoveryRequired,
        };
        let mut lifecycle = state.lifecycle.clone();
        let transition = lifecycle
            .transition(next_status)?
            .ok_or(SqliteRunStoreError::InvalidState)?;
        let verification = DurableVerificationRecord {
            operation_id: operation_id.to_owned(),
            report,
            policy,
            evaluation,
            transition: transition.clone(),
        };
        let next_state = DurableRunLifecycleState {
            contract_version: SQLITE_RUN_STORE_CONTRACT_VERSION,
            scope: self.scope.clone(),
            ledger_sequence: state
                .ledger_sequence
                .checked_add(1)
                .ok_or(SqliteRunStoreError::SequenceOverflow)?,
            lifecycle,
            verification: Some(verification.clone()),
        };
        let state_value =
            serde_json::to_value(&next_state).map_err(|_| SqliteRunStoreError::Serialization)?;
        let aggregate = run_aggregate_id(&next_state.lifecycle.identity.run_id);
        let draft = EventDraft {
            scope: self.scope.clone(),
            aggregate: aggregate.clone(),
            event_id: format!("{aggregate}:verification:{operation_id}"),
            idempotency_key: format!("run-verification:{operation_id}"),
            event_type: "run_verification_recorded".into(),
            payload: json!({
                "transition": transition,
                "verification": verification,
                "state": state_value,
            }),
            expected_version: state.ledger_sequence,
        };
        let saved = self.ledger.append_checkpointed(
            draft,
            format!("{aggregate}:checkpoint:verification:{operation_id}"),
            state_value.clone(),
        )?;
        if saved.event.sequence != next_state.ledger_sequence
            || saved.checkpoint.state != state_value
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        Ok(DurableRunVerification {
            state: next_state,
            transition: Some(transition),
        })
    }

    fn validate_state(&self, state: &DurableRunLifecycleState) -> Result<(), SqliteRunStoreError> {
        if state.contract_version != SQLITE_RUN_STORE_CONTRACT_VERSION {
            return Err(SqliteRunStoreError::UnsupportedContractVersion(
                state.contract_version,
            ));
        }
        state.lifecycle.validate()?;
        if state.scope != self.scope
            || state.lifecycle.contract_version != RUN_CONTRACT_VERSION
            || state.ledger_sequence == 0
            || state.ledger_sequence != state.lifecycle.sequence.saturating_add(1)
        {
            return Err(SqliteRunStoreError::InvalidState);
        }
        if state.lifecycle.status == RunStatus::Completed
            && !state.verification.as_ref().is_some_and(|record| {
                record.evaluation.verdict == GateVerdict::Passed
                    && record.transition.to == RunStatus::Completed
            })
        {
            return Err(SqliteRunStoreError::PassingVerificationRequired);
        }
        if let Some(record) = state.verification.as_ref() {
            validate_operation_id(&record.operation_id)?;
            if record.report.run_id != state.lifecycle.identity.run_id
                || record.evaluation != evaluate_verification(&record.report, &record.policy)?
                || record.evaluation.contract_version != EVIDENCE_GATE_CONTRACT_VERSION
                || record.transition.contract_version != RUN_CONTRACT_VERSION
                || record.transition.run_id != state.lifecycle.identity.run_id
                || record.transition.from != RunStatus::Verifying
                || record.transition.to != run_status_for_verdict(record.evaluation.verdict)
                || record.transition.sequence > state.lifecycle.sequence
                || (record.transition.sequence == state.lifecycle.sequence
                    && record.transition.to != state.lifecycle.status)
            {
                return Err(SqliteRunStoreError::InvalidVerificationRecord);
            }
        }
        Ok(())
    }
}

fn run_status_for_verdict(verdict: GateVerdict) -> RunStatus {
    match verdict {
        GateVerdict::Passed => RunStatus::Completed,
        GateVerdict::Degraded => RunStatus::Degraded,
        GateVerdict::Blocked => RunStatus::Failed,
        GateVerdict::Unverified => RunStatus::RecoveryRequired,
    }
}

fn run_aggregate_id(run_id: &str) -> String {
    format!("run-lifecycle:{}:{run_id}", run_id.len())
}

fn validate_operation_id(operation_id: &str) -> Result<(), SqliteRunStoreError> {
    if operation_id.trim().is_empty() || operation_id.len() > MAX_OPERATION_ID_BYTES {
        return Err(SqliteRunStoreError::InvalidOperationId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

    fn database_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "vectora-engine-run-{}-{}.sqlite",
            std::process::id(),
            NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn identity(run_id: &str, turn_id: &str, key: &str) -> RunIdentity {
        RunIdentity {
            contract_version: RUN_CONTRACT_VERSION,
            run_id: run_id.into(),
            idempotency_key: key.into(),
            thread_id: "thread-1".into(),
            turn_id: turn_id.into(),
            actor_id: Some("actor-1".into()),
            scope_id: Some("workspace-1".into()),
            provider: Some("provider-1".into()),
            model: Some("model-1".into()),
            capabilities: vec!["tools.read".into()],
        }
    }

    fn verification_report(
        run_id: &str,
        outcome: crate::harness::VerificationOutcome,
    ) -> VerificationReport {
        VerificationReport {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            run_id: run_id.into(),
            evaluated_at_unix_ms: 100,
            checks: vec![crate::harness::VerificationCheck {
                check_id: "tests".into(),
                outcome,
                evidence: vec![crate::harness::VerificationEvidence {
                    evidence_id: "tests-artifact".into(),
                    origin: "test_runner".into(),
                    artifact_ref: "artifact:tests".into(),
                    sha256: Some("a".repeat(64)),
                    redacted: true,
                    expires_at_unix_ms: None,
                }],
            }],
        }
    }

    fn verification_policy() -> EvidenceDispositionPolicy {
        EvidenceDispositionPolicy {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            required_checks: vec!["tests".into()],
            require_evidence: true,
            failed_check: crate::harness::FailureDisposition::Blocked,
            inconclusive: crate::harness::EvidenceDisposition::Unverified,
        }
    }

    fn verifying_state(
        store: &mut SqliteRunLifecycleStore,
        run_id: &str,
    ) -> DurableRunLifecycleState {
        let admitted = store
            .admit(
                identity(
                    run_id,
                    &format!("{run_id}-turn"),
                    &format!("{run_id}-admit"),
                ),
                json!({}),
            )
            .unwrap();
        let running = store
            .transition(
                &admitted.state,
                RunStatus::Running,
                &format!("{run_id}-running"),
            )
            .unwrap();
        store
            .transition(
                &running.state,
                RunStatus::Verifying,
                &format!("{run_id}-verifying"),
            )
            .unwrap()
            .state
    }

    #[test]
    fn admission_is_idempotent_and_persists_lifecycle_atomically() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let run_identity = identity("run-1", "turn-1", "admit-1");
        let request = json!({"input_digest":"safe-digest"});
        let first = store.admit(run_identity.clone(), request.clone()).unwrap();
        assert_eq!(first.state.lifecycle.status, RunStatus::Planned);
        assert_eq!(first.state.ledger_sequence, 1);
        assert_eq!(first.admission_event.sequence, 1);
        assert_eq!(
            store.load_latest("run-1").unwrap(),
            Some(first.state.clone())
        );

        let retry = store.admit(run_identity, request).unwrap();
        assert_eq!(retry, first);

        let second = store
            .admit(
                identity("run-2", "turn-2", "admit-2"),
                json!({"input_digest":"safe-digest-2"}),
            )
            .unwrap();
        assert_eq!(second.admission_event.sequence, 2);
        assert_eq!(second.state.ledger_sequence, 1);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn lifecycle_transitions_survive_reopen_and_reject_stale_snapshots() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let admitted = store
            .admit(identity("run-1", "turn-1", "admit-1"), json!({}))
            .unwrap();
        let running = store
            .transition(&admitted.state, RunStatus::Running, "running-1")
            .unwrap();
        assert_eq!(running.state.lifecycle.status, RunStatus::Running);
        assert_eq!(running.state.ledger_sequence, 2);
        assert_eq!(
            store
                .transition(&admitted.state, RunStatus::Running, "running-1")
                .unwrap(),
            running
        );
        assert!(matches!(
            store.transition(&admitted.state, RunStatus::Cancelled, "stale-cancel"),
            Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict))
        ));
        let mut altered_snapshot = running.state.clone();
        altered_snapshot.lifecycle.identity.provider = Some("unpersisted-provider".into());
        assert!(matches!(
            store.transition(&altered_snapshot, RunStatus::Verifying, "forged-snapshot"),
            Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict))
        ));
        drop(store);

        let store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        assert_eq!(store.load_latest("run-1").unwrap(), Some(running.state));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn admission_rejects_scope_mismatch_without_reserving_a_run() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let mut mismatched_identity = identity("run-1", "turn-1", "admit-1");
        mismatched_identity.scope_id = Some("workspace-2".into());
        assert!(matches!(
            store.admit(mismatched_identity, json!({})),
            Err(SqliteRunStoreError::InvalidState)
        ));
        assert_eq!(store.load_latest("run-1").unwrap(), None);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_report_policy_verdict_and_terminal_transition_are_atomic_and_durable() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let state = verifying_state(&mut store, "run-verified");
        let report =
            verification_report("run-verified", crate::harness::VerificationOutcome::Passed);
        let policy = verification_policy();
        let first = store
            .verify(&state, report.clone(), policy.clone(), "verify-passed")
            .unwrap();
        assert_eq!(first.state.lifecycle.status, RunStatus::Completed);
        assert_eq!(first.state.ledger_sequence, state.ledger_sequence + 1);
        assert_eq!(
            first
                .state
                .verification
                .as_ref()
                .unwrap()
                .evaluation
                .verdict,
            GateVerdict::Passed
        );
        assert_eq!(
            first.transition.as_ref().unwrap().from,
            RunStatus::Verifying
        );
        assert_eq!(first.transition.as_ref().unwrap().to, RunStatus::Completed);

        let replay = store
            .verify(&state, report.clone(), policy.clone(), "verify-passed")
            .unwrap();
        assert_eq!(replay.state, first.state);
        assert!(replay.transition.is_none());
        assert!(
            store
                .verify(&state, report.clone(), policy.clone(), "verify-passed")
                .is_ok()
        );
        assert!(matches!(
            store.verify(&state, report.clone(), policy.clone(), "verify-other"),
            Err(SqliteRunStoreError::Ledger(LedgerError::VersionConflict))
        ));
        drop(store);

        let store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let loaded = store.load_latest("run-verified").unwrap().unwrap();
        assert_eq!(loaded, first.state);
        assert_eq!(loaded.verification.unwrap().report, report);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_maps_nonpassing_verdicts_without_marking_run_completed() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let failed_state = verifying_state(&mut store, "run-failed-gate");
        let failed = store
            .verify(
                &failed_state,
                verification_report(
                    "run-failed-gate",
                    crate::harness::VerificationOutcome::Failed,
                ),
                verification_policy(),
                "verify-failed",
            )
            .unwrap();
        assert_eq!(failed.state.lifecycle.status, RunStatus::Failed);
        assert_eq!(
            failed
                .state
                .verification
                .as_ref()
                .unwrap()
                .evaluation
                .verdict,
            GateVerdict::Blocked
        );

        let unknown_state = verifying_state(&mut store, "run-unverified-gate");
        let unverified = store
            .verify(
                &unknown_state,
                verification_report(
                    "run-unverified-gate",
                    crate::harness::VerificationOutcome::InfraError,
                ),
                verification_policy(),
                "verify-infra-error",
            )
            .unwrap();
        assert_eq!(
            unverified.state.lifecycle.status,
            RunStatus::RecoveryRequired
        );
        assert_eq!(
            unverified
                .state
                .verification
                .as_ref()
                .unwrap()
                .evaluation
                .verdict,
            GateVerdict::Unverified
        );

        let resumed = store
            .transition(
                &unverified.state,
                RunStatus::Running,
                "resume-after-verification",
            )
            .unwrap();
        let verifying_again = store
            .transition(&resumed.state, RunStatus::Verifying, "verify-again")
            .unwrap();
        let passed = store
            .verify(
                &verifying_again.state,
                verification_report(
                    "run-unverified-gate",
                    crate::harness::VerificationOutcome::Passed,
                ),
                verification_policy(),
                "verify-after-recovery",
            )
            .unwrap();
        assert_eq!(passed.state.lifecycle.status, RunStatus::Completed);
        assert_eq!(
            passed
                .state
                .verification
                .as_ref()
                .unwrap()
                .evaluation
                .verdict,
            GateVerdict::Passed
        );
        assert_eq!(
            store.load_latest("run-unverified-gate").unwrap(),
            Some(passed.state)
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn durable_lifecycle_cannot_complete_a_verifying_run_without_passing_gate() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let state = verifying_state(&mut store, "run-no-gate");
        assert!(matches!(
            store.transition(&state, RunStatus::Completed, "complete-without-gate"),
            Err(SqliteRunStoreError::PassingVerificationRequired)
        ));
        assert_eq!(store.load_latest("run-no-gate").unwrap(), Some(state));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_rejects_cross_run_reports_and_conflicting_operation_replay() {
        let path = database_path();
        let mut store = SqliteRunLifecycleStore::open(&path, "workspace-1").unwrap();
        let state = verifying_state(&mut store, "run-bound");
        assert!(matches!(
            store.verify(
                &state,
                verification_report("another-run", crate::harness::VerificationOutcome::Passed),
                verification_policy(),
                "verify-cross-run",
            ),
            Err(SqliteRunStoreError::VerificationRunMismatch)
        ));
        let report = verification_report("run-bound", crate::harness::VerificationOutcome::Passed);
        let policy = verification_policy();
        store
            .verify(&state, report.clone(), policy.clone(), "verify-bound")
            .unwrap();
        let changed_report =
            verification_report("run-bound", crate::harness::VerificationOutcome::Failed);
        assert!(matches!(
            store.verify(&state, changed_report, policy, "verify-bound"),
            Err(SqliteRunStoreError::VerificationOperationConflict)
        ));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
