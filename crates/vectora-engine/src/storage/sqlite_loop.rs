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
        ModelToolLoopAction, ModelToolLoopError, ModelToolLoopSettings, ModelToolLoopState,
        ModelToolLoopTransition, ToolDispatch, ToolResult, advance_model_turn, advance_tool_result,
        reconcile_tool_result, recover_model_tool_loop, validate_model_tool_loop_state,
    },
};

use super::sqlite::{EventDraft, LedgerError, SqliteLedger};

pub const SQLITE_LOOP_STORE_CONTRACT_VERSION: u16 = 1;
const MAX_OPERATION_ID_BYTES: usize = 256;

/// Durable envelope carrying the event sequence needed for optimistic writes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableModelToolLoopState {
    pub contract_version: u16,
    pub scope: String,
    pub ledger_sequence: u64,
    pub loop_state: ModelToolLoopState,
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
        let saved = self
            .ledger
            .append_checkpointed(draft, checkpoint_id, state_value.clone())?;
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
        if state.contract_version != SQLITE_LOOP_STORE_CONTRACT_VERSION {
            return Err(SqliteModelToolLoopError::UnsupportedContractVersion(
                state.contract_version,
            ));
        }
        if state.scope != self.scope || state.ledger_sequence == 0 {
            return Err(SqliteModelToolLoopError::InvalidState);
        }
        validate_model_tool_loop_state(&state.loop_state)?;
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

    use crate::harness::{MODEL_TOOL_LOOP_CONTRACT_VERSION, ToolExecutionMode};

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
        store
            .start(
                "run-1",
                "turn-2",
                ModelToolLoopSettings::default(),
                "start-2",
            )
            .unwrap();

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
}
