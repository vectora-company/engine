//! SQLite event ledger. The caller owns authorization and redaction of payloads.

use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const SCHEMA_VERSION: i64 = 4;
pub const CHECKPOINT_VERSION: u16 = 1;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("invalid ledger input")]
    InvalidInput,
    #[error("unsupported ledger schema version")]
    SchemaVersion,
    #[error("idempotency key was reused with different content")]
    IdempotencyConflict,
    #[error("ledger aggregate version conflict")]
    VersionConflict,
    #[error("ledger storage operation failed")]
    Storage,
}

impl From<rusqlite::Error> for LedgerError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventDraft {
    pub scope: String,
    pub aggregate: String,
    pub event_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload: Value,
    pub expected_version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LedgerEvent {
    pub scope: String,
    pub aggregate: String,
    pub event_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload: Value,
    pub sequence: u64,
}

/// Request identity and payload are supplied by a trusted, authorizing host.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RunReservation {
    pub scope: String,
    pub thread_id: String,
    pub turn_id: String,
    pub run_id: String,
    pub idempotency_key: String,
    pub request: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Admission {
    pub reservation: RunReservation,
    pub event: LedgerEvent,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub contract_version: u16,
    pub scope: String,
    pub aggregate: String,
    pub checkpoint_id: String,
    pub sequence: u64,
    pub state: Value,
}

pub struct SqliteLedger {
    connection: Connection,
}

impl SqliteLedger {
    /// Open a file-backed ledger. Required when operations open separate connections.
    pub fn open_persistent(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() || path == Path::new(":memory:") {
            return Err(LedgerError::InvalidInput);
        }
        Self::open(path)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys=ON;")?;
        let mut ledger = Self { connection };
        ledger.migrate()?;
        Ok(ledger)
    }

    fn migrate(&mut self) -> Result<(), LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS ve_ledger_meta (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL);")?;
        let version: Option<i64> = tx
            .query_row("SELECT version FROM ve_ledger_meta WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()?;
        match version {
            Some(v) if v > SCHEMA_VERSION || v < 1 => return Err(LedgerError::SchemaVersion),
            Some(_) => {}
            None => {
                tx.execute_batch(
                    "CREATE TABLE ve_ledger_events (
                    scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, sequence INTEGER NOT NULL,
                    event_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, event_type TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    PRIMARY KEY(scope, aggregate_id, sequence),
                    UNIQUE(scope, aggregate_id, idempotency_key), UNIQUE(scope, event_id)
                ); INSERT INTO ve_ledger_meta(id,version) VALUES (1,1);",
                )?;
            }
        }
        let mut current = version.unwrap_or(1);
        if version.is_none() {
            current = 1;
        }
        if current < 2 {
            tx.execute_batch("CREATE TABLE ve_run_reservations (
                scope TEXT NOT NULL, thread_id TEXT NOT NULL, turn_id TEXT NOT NULL,
                run_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, request TEXT NOT NULL,
                PRIMARY KEY(scope,thread_id,turn_id), UNIQUE(scope,idempotency_key), UNIQUE(scope,run_id)
            );
            CREATE TABLE ve_ledger_checkpoints (
                scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, checkpoint_id TEXT NOT NULL,
                sequence INTEGER NOT NULL, contract_version INTEGER NOT NULL, state TEXT NOT NULL,
                PRIMARY KEY(scope,aggregate_id,checkpoint_id), UNIQUE(scope,aggregate_id,sequence)
            );
            UPDATE ve_ledger_meta SET version=2 WHERE id=1;")?;
            current = 2;
        }
        if current < 3 {
            tx.execute_batch(
                "CREATE TABLE ve_run_admissions (
                scope TEXT NOT NULL, run_id TEXT NOT NULL, event_id TEXT NOT NULL,
                PRIMARY KEY(scope,run_id), UNIQUE(scope,event_id)
            ); UPDATE ve_ledger_meta SET version=3 WHERE id=1;",
            )?;
            current = 3;
        }
        if current < 4 {
            tx.execute_batch("CREATE TABLE ve_ledger_outbox (
                scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, sequence INTEGER NOT NULL,
                event_json TEXT NOT NULL, delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN (0,1)),
                PRIMARY KEY(scope,aggregate_id,sequence)
            ); CREATE INDEX ve_ledger_outbox_pending ON ve_ledger_outbox(delivered,scope,aggregate_id,sequence);
            UPDATE ve_ledger_meta SET version=4 WHERE id=1;")?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Appends exactly once for an aggregate/idempotency key within SQLite.
    pub fn append(&mut self, draft: EventDraft) -> Result<LedgerEvent, LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let event = Self::append_in(&tx, draft)?;
        tx.commit()?;
        Ok(event)
    }

    /// Appends locally and queues a durable mirror record in the same transaction.
    pub fn append_with_outbox(&mut self, draft: EventDraft) -> Result<LedgerEvent, LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let event = Self::append_in(&tx, draft)?;
        let serialized = serde_json::to_string(&event).map_err(|_| LedgerError::InvalidInput)?;
        tx.execute("INSERT INTO ve_ledger_outbox(scope,aggregate_id,sequence,event_json) VALUES(?1,?2,?3,?4) ON CONFLICT(scope,aggregate_id,sequence) DO NOTHING", params![event.scope,event.aggregate,event.sequence as i64,serialized])?;
        tx.commit()?;
        Ok(event)
    }

    pub fn pending_outbox(&self, limit: u32) -> Result<Vec<LedgerEvent>, LedgerError> {
        if !(1..=1000).contains(&limit) {
            return Err(LedgerError::InvalidInput);
        }
        let mut stmt = self.connection.prepare("SELECT event_json FROM ve_ledger_outbox WHERE delivered=0 ORDER BY scope,aggregate_id,sequence LIMIT ?1")?;
        let rows = stmt.query_map([limit], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(|_| LedgerError::Storage))
            .collect()
    }

    pub fn pending_outbox_count(&self) -> Result<u64, LedgerError> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM ve_ledger_outbox WHERE delivered=0",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn mark_outbox_delivered(
        &mut self,
        scope: &str,
        aggregate: &str,
        sequence: u64,
    ) -> Result<(), LedgerError> {
        if scope.trim().is_empty() || aggregate.trim().is_empty() || sequence > i64::MAX as u64 {
            return Err(LedgerError::InvalidInput);
        }
        self.connection.execute("UPDATE ve_ledger_outbox SET delivered=1 WHERE scope=?1 AND aggregate_id=?2 AND sequence=?3", params![scope,aggregate,sequence as i64])?;
        Ok(())
    }

    fn append_in(tx: &Transaction<'_>, draft: EventDraft) -> Result<LedgerEvent, LedgerError> {
        if [
            &draft.scope,
            &draft.aggregate,
            &draft.event_id,
            &draft.idempotency_key,
            &draft.event_type,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || draft.expected_version >= i64::MAX as u64
        {
            return Err(LedgerError::InvalidInput);
        }
        let payload =
            serde_json::to_string(&draft.payload).map_err(|_| LedgerError::InvalidInput)?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(LedgerError::InvalidInput);
        }
        let existing: Option<(i64, String, String, String)> = tx.query_row(
            "SELECT sequence,event_id,event_type,payload FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2 AND idempotency_key=?3",
            params![draft.scope, draft.aggregate, draft.idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?;
        let sequence = if let Some((sequence, id, kind, previous)) = existing {
            let previous: Value =
                serde_json::from_str(&previous).map_err(|_| LedgerError::Storage)?;
            if id != draft.event_id || kind != draft.event_type || previous != draft.payload {
                return Err(LedgerError::IdempotencyConflict);
            }
            sequence as u64
        } else {
            let id_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM ve_ledger_events WHERE scope=?1 AND event_id=?2)",
                params![draft.scope, draft.event_id],
                |row| row.get(0),
            )?;
            if id_exists {
                return Err(LedgerError::IdempotencyConflict);
            }
            let version: i64 = tx.query_row("SELECT COALESCE(MAX(sequence),0) FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2", params![draft.scope, draft.aggregate], |row| row.get(0))?;
            if version as u64 != draft.expected_version {
                return Err(LedgerError::VersionConflict);
            }
            let next = version + 1;
            tx.execute("INSERT INTO ve_ledger_events(scope,aggregate_id,sequence,event_id,idempotency_key,event_type,payload) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![draft.scope,draft.aggregate,next,draft.event_id,draft.idempotency_key,draft.event_type,payload])?;
            next as u64
        };
        Ok(LedgerEvent {
            scope: draft.scope,
            aggregate: draft.aggregate,
            event_id: draft.event_id,
            idempotency_key: draft.idempotency_key,
            event_type: draft.event_type,
            payload: draft.payload,
            sequence,
        })
    }

    pub fn read(
        &self,
        scope: &str,
        aggregate: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<LedgerEvent>, LedgerError> {
        if scope.trim().is_empty()
            || aggregate.trim().is_empty()
            || after > i64::MAX as u64
            || !(1..=1000).contains(&limit)
        {
            return Err(LedgerError::InvalidInput);
        }
        let mut statement = self.connection.prepare("SELECT sequence,event_id,idempotency_key,event_type,payload FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2 AND sequence>?3 ORDER BY sequence LIMIT ?4")?;
        let rows = statement.query_map(params![scope, aggregate, after as i64, limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (sequence, event_id, idempotency_key, event_type, payload) = row?;
            events.push(LedgerEvent {
                scope: scope.into(),
                aggregate: aggregate.into(),
                event_id,
                idempotency_key,
                event_type,
                payload: serde_json::from_str(&payload).map_err(|_| LedgerError::Storage)?,
                sequence: sequence as u64,
            });
        }
        Ok(events)
    }

    /// Atomically admit a run or return its identical earlier reservation.
    pub fn reserve_run(&mut self, input: RunReservation) -> Result<RunReservation, LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = Self::reserve_run_in(&tx, input)?;
        tx.commit()?;
        Ok(result)
    }

    /// Reserve a run and append its admission event in one SQLite transaction.
    pub fn admit_run(
        &mut self,
        reservation: RunReservation,
        event: EventDraft,
    ) -> Result<Admission, LedgerError> {
        if reservation.scope != event.scope || reservation.thread_id != event.aggregate {
            return Err(LedgerError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let reservation = Self::reserve_run_in(&tx, reservation)?;
        let prior: Option<(String, String)> = tx.query_row(
            "SELECT run_id,event_id FROM ve_run_admissions WHERE scope=?1 AND (run_id=?2 OR event_id=?3) LIMIT 1",
            params![reservation.scope, reservation.run_id, event.event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        if prior.as_ref().is_some_and(|(run_id, event_id)| {
            run_id != &reservation.run_id || event_id != &event.event_id
        }) {
            return Err(LedgerError::IdempotencyConflict);
        }
        let event = Self::append_in(&tx, event)?;
        if prior.is_none() {
            tx.execute(
                "INSERT INTO ve_run_admissions(scope,run_id,event_id) VALUES(?1,?2,?3)",
                params![reservation.scope, reservation.run_id, event.event_id],
            )?;
        }
        tx.commit()?;
        Ok(Admission { reservation, event })
    }

    fn reserve_run_in(
        tx: &Transaction<'_>,
        input: RunReservation,
    ) -> Result<RunReservation, LedgerError> {
        if [
            &input.scope,
            &input.thread_id,
            &input.turn_id,
            &input.run_id,
            &input.idempotency_key,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(LedgerError::InvalidInput);
        }
        let request =
            serde_json::to_string(&input.request).map_err(|_| LedgerError::InvalidInput)?;
        if request.len() > MAX_PAYLOAD_BYTES {
            return Err(LedgerError::InvalidInput);
        }
        let existing: Option<(String, String, String, String, String)> = tx.query_row(
            "SELECT thread_id,turn_id,run_id,idempotency_key,request FROM ve_run_reservations WHERE scope=?1 AND (idempotency_key=?2 OR (thread_id=?3 AND turn_id=?4) OR run_id=?5) LIMIT 1",
            params![input.scope, input.idempotency_key, input.thread_id, input.turn_id, input.run_id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).optional()?;
        if let Some((thread, turn, run, key, prior)) = existing {
            let prior: Value = serde_json::from_str(&prior).map_err(|_| LedgerError::Storage)?;
            if thread != input.thread_id
                || turn != input.turn_id
                || run != input.run_id
                || key != input.idempotency_key
                || prior != input.request
            {
                return Err(LedgerError::IdempotencyConflict);
            }
        } else {
            tx.execute("INSERT INTO ve_run_reservations(scope,thread_id,turn_id,run_id,idempotency_key,request) VALUES(?1,?2,?3,?4,?5,?6)", params![input.scope,input.thread_id,input.turn_id,input.run_id,input.idempotency_key,request])?;
        }
        Ok(input)
    }

    /// Save an immutable checkpoint for an existing event sequence.
    pub fn save_checkpoint(&mut self, checkpoint: Checkpoint) -> Result<Checkpoint, LedgerError> {
        if checkpoint.contract_version != CHECKPOINT_VERSION {
            return Err(LedgerError::SchemaVersion);
        }
        if [
            &checkpoint.scope,
            &checkpoint.aggregate,
            &checkpoint.checkpoint_id,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || checkpoint.sequence == 0
            || checkpoint.sequence > i64::MAX as u64
        {
            return Err(LedgerError::InvalidInput);
        }
        let state =
            serde_json::to_string(&checkpoint.state).map_err(|_| LedgerError::InvalidInput)?;
        if state.len() > MAX_PAYLOAD_BYTES {
            return Err(LedgerError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2 AND sequence=?3)", params![checkpoint.scope,checkpoint.aggregate,checkpoint.sequence as i64], |row| row.get(0))?;
        if !exists {
            return Err(LedgerError::VersionConflict);
        }
        let prior: Option<(String,i64,i64,String)> = tx.query_row("SELECT checkpoint_id,sequence,contract_version,state FROM ve_ledger_checkpoints WHERE scope=?1 AND aggregate_id=?2 AND (checkpoint_id=?3 OR sequence=?4) LIMIT 1", params![checkpoint.scope,checkpoint.aggregate,checkpoint.checkpoint_id,checkpoint.sequence as i64], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
        if let Some((id, sequence, version, prior)) = prior {
            let prior: Value = serde_json::from_str(&prior).map_err(|_| LedgerError::Storage)?;
            if id != checkpoint.checkpoint_id
                || sequence as u64 != checkpoint.sequence
                || version != i64::from(checkpoint.contract_version)
                || prior != checkpoint.state
            {
                return Err(LedgerError::IdempotencyConflict);
            }
        } else {
            tx.execute("INSERT INTO ve_ledger_checkpoints(scope,aggregate_id,checkpoint_id,sequence,contract_version,state) VALUES(?1,?2,?3,?4,?5,?6)", params![checkpoint.scope,checkpoint.aggregate,checkpoint.checkpoint_id,checkpoint.sequence as i64,checkpoint.contract_version,state])?;
        }
        tx.commit()?;
        Ok(checkpoint)
    }

    pub fn latest_checkpoint(
        &self,
        scope: &str,
        aggregate: &str,
    ) -> Result<Option<Checkpoint>, LedgerError> {
        if scope.trim().is_empty() || aggregate.trim().is_empty() {
            return Err(LedgerError::InvalidInput);
        }
        let prior: Option<(String,i64,i64,String)> = self.connection.query_row("SELECT checkpoint_id,sequence,contract_version,state FROM ve_ledger_checkpoints WHERE scope=?1 AND aggregate_id=?2 ORDER BY sequence DESC LIMIT 1", params![scope,aggregate], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
        prior
            .map(|(checkpoint_id, sequence, version, state)| {
                if version != i64::from(CHECKPOINT_VERSION) {
                    return Err(LedgerError::SchemaVersion);
                }
                Ok(Checkpoint {
                    contract_version: CHECKPOINT_VERSION,
                    scope: scope.into(),
                    aggregate: aggregate.into(),
                    checkpoint_id,
                    sequence: u64::try_from(sequence).map_err(|_| LedgerError::Storage)?,
                    state: serde_json::from_str(&state).map_err(|_| LedgerError::Storage)?,
                })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn append_retries_are_idempotent_and_scope_isolated() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let draft = EventDraft {
            scope: "a".into(),
            aggregate: "run".into(),
            event_id: "e1".into(),
            idempotency_key: "key".into(),
            event_type: "started".into(),
            payload: json!({"state": "started"}),
            expected_version: 0,
        };
        assert_eq!(ledger.append(draft.clone()).unwrap().sequence, 1);
        assert_eq!(ledger.append(draft.clone()).unwrap().sequence, 1);
        let mut changed = draft.clone();
        changed.payload = json!({"state": "changed"});
        assert!(matches!(
            ledger.append(changed),
            Err(LedgerError::IdempotencyConflict)
        ));
        let mut stale = draft.clone();
        stale.event_id = "e2".into();
        stale.idempotency_key = "key2".into();
        assert!(matches!(
            ledger.append(stale),
            Err(LedgerError::VersionConflict)
        ));
        assert!(ledger.read("b", "run", 0, 10).unwrap().is_empty());
        assert_eq!(ledger.read("a", "run", 0, 10).unwrap().len(), 1);
        assert!(ledger.read("a", "run", 1, 10).unwrap().is_empty());
    }

    #[test]
    fn reservation_and_checkpoint_are_immutable_and_scoped() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let reservation = RunReservation {
            scope: "a".into(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "key".into(),
            request: json!({"prompt":"redacted"}),
        };
        assert_eq!(
            ledger.reserve_run(reservation.clone()).unwrap(),
            reservation
        );
        assert_eq!(
            ledger.reserve_run(reservation.clone()).unwrap(),
            reservation
        );
        let mut changed = reservation.clone();
        changed.request = json!({"prompt":"different"});
        assert!(matches!(
            ledger.reserve_run(changed),
            Err(LedgerError::IdempotencyConflict)
        ));
        let draft = EventDraft {
            scope: "a".into(),
            aggregate: "run".into(),
            event_id: "one".into(),
            idempotency_key: "event".into(),
            event_type: "started".into(),
            payload: json!({}),
            expected_version: 0,
        };
        ledger.append(draft).unwrap();
        let checkpoint = Checkpoint {
            contract_version: 1,
            scope: "a".into(),
            aggregate: "run".into(),
            checkpoint_id: "cp".into(),
            sequence: 1,
            state: json!({"status":"running"}),
        };
        assert_eq!(
            ledger.save_checkpoint(checkpoint.clone()).unwrap(),
            checkpoint
        );
        assert_eq!(
            ledger.save_checkpoint(checkpoint.clone()).unwrap(),
            checkpoint
        );
        assert_eq!(
            ledger.latest_checkpoint("a", "run").unwrap(),
            Some(checkpoint.clone())
        );
        assert_eq!(ledger.latest_checkpoint("b", "run").unwrap(), None);
        let mut changed = checkpoint;
        changed.state = json!({"status":"other"});
        assert!(matches!(
            ledger.save_checkpoint(changed),
            Err(LedgerError::IdempotencyConflict)
        ));
    }

    #[test]
    fn migrates_v1_without_deleting_events() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        ledger
            .append(EventDraft {
                scope: "a".into(),
                aggregate: "run".into(),
                event_id: "e".into(),
                idempotency_key: "k".into(),
                event_type: "started".into(),
                payload: json!({}),
                expected_version: 0,
            })
            .unwrap();
        ledger.connection.execute_batch("DROP TABLE ve_run_reservations; DROP TABLE ve_ledger_checkpoints; DROP TABLE ve_run_admissions; DROP TABLE ve_ledger_outbox; UPDATE ve_ledger_meta SET version=1;").unwrap();
        ledger.migrate().unwrap();
        let version: i64 = ledger
            .connection
            .query_row("SELECT version FROM ve_ledger_meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(ledger.read("a", "run", 0, 10).unwrap().len(), 1);
        assert!(ledger.latest_checkpoint("a", "run").unwrap().is_none());
    }

    #[test]
    fn admission_commits_reservation_and_event_together_and_rejects_changed_retry() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let reservation = RunReservation {
            scope: "a".into(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "key".into(),
            request: json!({}),
        };
        let event = EventDraft {
            scope: "a".into(),
            aggregate: "thread".into(),
            event_id: "e1".into(),
            idempotency_key: "event-key".into(),
            event_type: "admitted".into(),
            payload: json!({"run_id":"run"}),
            expected_version: 0,
        };
        let mut stale = event.clone();
        stale.expected_version = 1;
        assert!(matches!(
            ledger.admit_run(reservation.clone(), stale),
            Err(LedgerError::VersionConflict)
        ));
        let count: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM ve_run_reservations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        let first = ledger
            .admit_run(reservation.clone(), event.clone())
            .unwrap();
        assert_eq!(
            ledger
                .admit_run(reservation.clone(), event.clone())
                .unwrap(),
            first
        );
        let mut changed = event;
        changed.event_id = "e2".into();
        changed.idempotency_key = "other".into();
        changed.expected_version = 1;
        assert!(matches!(
            ledger.admit_run(reservation, changed),
            Err(LedgerError::IdempotencyConflict)
        ));
        assert_eq!(ledger.read("a", "thread", 0, 10).unwrap().len(), 1);
    }

    #[test]
    fn concurrent_run_admissions_allow_only_one_distinct_payload() {
        use std::sync::{Arc, Barrier};
        use std::time::{SystemTime, UNIX_EPOCH};
        let path = std::env::temp_dir().join(format!(
            "vectora-ledger-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(SqliteLedger::open_persistent(&path).unwrap());
        let barrier = Arc::new(Barrier::new(2));
        let threads: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|run| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut ledger = SqliteLedger::open_persistent(path).unwrap();
                    barrier.wait();
                    ledger.reserve_run(RunReservation {
                        scope: "scope".into(),
                        thread_id: "thread".into(),
                        turn_id: "turn".into(),
                        run_id: run.into(),
                        idempotency_key: "key".into(),
                        request: json!({"run":run}),
                    })
                })
            })
            .collect();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(LedgerError::IdempotencyConflict)))
                .count(),
            1
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn outbox_is_atomic_with_ledger_and_delivery_is_idempotent() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let draft = EventDraft {
            scope: "s".into(),
            aggregate: "a".into(),
            event_id: "e".into(),
            idempotency_key: "k".into(),
            event_type: "accepted".into(),
            payload: json!({"v":1}),
            expected_version: 0,
        };
        let event = ledger.append_with_outbox(draft.clone()).unwrap();
        assert_eq!(ledger.pending_outbox(10).unwrap(), vec![event.clone()]);
        assert_eq!(ledger.append_with_outbox(draft).unwrap(), event);
        assert_eq!(ledger.pending_outbox(10).unwrap().len(), 1);
        ledger.mark_outbox_delivered("s", "a", 1).unwrap();
        assert!(ledger.pending_outbox(10).unwrap().is_empty());
        assert_eq!(ledger.read("s", "a", 0, 10).unwrap().len(), 1);
    }
}
