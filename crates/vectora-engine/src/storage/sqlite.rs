//! SQLite event ledger. The caller owns authorization and redaction of payloads.

use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

mod backup;
pub use backup::{BackupReport, verify_backup};

const SCHEMA_VERSION: i64 = 8;
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
    #[error("ledger file is in use by another writer")]
    Busy,
    #[error("ledger backup is corrupt or is not a ledger")]
    BackupInvalid,
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
pub struct CheckpointedAdmission {
    pub reservation: RunReservation,
    pub admission_event: LedgerEvent,
    pub lifecycle: CheckpointedEvent,
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CheckpointedEvent {
    pub event: LedgerEvent,
    pub checkpoint: Checkpoint,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PendingProjection {
    pub event: LedgerEvent,
    pub checkpoint: Option<Checkpoint>,
    pub reservation: Option<RunReservation>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectionCounts {
    pub aggregates: i64,
    pub events: i64,
    pub checkpoints: i64,
    pub reservations: i64,
    pub admissions: i64,
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
            Some(v) if !(1..=SCHEMA_VERSION).contains(&v) => {
                return Err(LedgerError::SchemaVersion);
            }
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
            current = 4;
        }
        if current < 5 {
            tx.execute_batch(
                "ALTER TABLE ve_ledger_outbox ADD COLUMN checkpoint_json TEXT;
            UPDATE ve_ledger_meta SET version=5 WHERE id=1;",
            )?;
            current = 5;
        }
        if current < 6 {
            tx.execute_batch(
                "ALTER TABLE ve_ledger_meta ADD COLUMN projection_enabled INTEGER NOT NULL DEFAULT 0 CHECK(projection_enabled IN (0,1));
            UPDATE ve_ledger_meta SET version=6 WHERE id=1;",
            )?;
            current = 6;
        }
        if current < 7 {
            tx.execute_batch(
                "ALTER TABLE ve_ledger_outbox ADD COLUMN reservation_json TEXT;
            ALTER TABLE ve_ledger_meta ADD COLUMN admission_backfill_complete INTEGER NOT NULL DEFAULT 0 CHECK(admission_backfill_complete IN (0,1));
            UPDATE ve_ledger_meta SET version=7 WHERE id=1;",
            )?;
            current = 7;
        }
        if current < 8 {
            tx.execute_batch(
                "CREATE TABLE ve_run_reservation_outbox (
                scope TEXT NOT NULL, thread_id TEXT NOT NULL, turn_id TEXT NOT NULL,
                reservation_json TEXT NOT NULL, delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN (0,1)),
                PRIMARY KEY(scope,thread_id,turn_id)
            );
            CREATE INDEX ve_run_reservation_outbox_pending ON ve_run_reservation_outbox(delivered,scope,thread_id,turn_id);
            ALTER TABLE ve_ledger_meta ADD COLUMN reservation_backfill_complete INTEGER NOT NULL DEFAULT 0 CHECK(reservation_backfill_complete IN (0,1));
            UPDATE ve_ledger_meta SET version=8 WHERE id=1;",
            )?;
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

    /// Appends an event and stores its recovery snapshot in the same transaction.
    pub fn append_checkpointed(
        &mut self,
        draft: EventDraft,
        checkpoint_id: String,
        state: Value,
    ) -> Result<CheckpointedEvent, LedgerError> {
        if checkpoint_id.trim().is_empty() {
            return Err(LedgerError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = Self::append_checkpointed_in(&tx, draft, checkpoint_id, state)?;
        tx.commit()?;
        Ok(result)
    }

    /// Appends an event and checkpoint and queues their projection atomically.
    pub fn append_checkpointed_with_outbox(
        &mut self,
        draft: EventDraft,
        checkpoint_id: String,
        state: Value,
    ) -> Result<CheckpointedEvent, LedgerError> {
        if checkpoint_id.trim().is_empty() {
            return Err(LedgerError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = Self::append_checkpointed_in(&tx, draft, checkpoint_id, state)?;
        Self::queue_projection_in(&tx, &result.event, Some(&result.checkpoint), None)?;
        tx.commit()?;
        Ok(result)
    }

    /// Appends a checkpointed event and queues it only when PostgreSQL projection is enabled.
    pub fn append_checkpointed_projected(
        &mut self,
        draft: EventDraft,
        checkpoint_id: String,
        state: Value,
    ) -> Result<CheckpointedEvent, LedgerError> {
        if self.projection_enabled()? {
            self.append_checkpointed_with_outbox(draft, checkpoint_id, state)
        } else {
            self.append_checkpointed(draft, checkpoint_id, state)
        }
    }

    /// Enables projection once and backfills existing events/checkpoints into the local outbox.
    pub fn enable_projection(&mut self) -> Result<(), LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let enabled: bool = tx.query_row(
            "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if !enabled {
            loop {
                let projections = {
                    let mut statement = tx.prepare("SELECT e.scope,e.aggregate_id,e.sequence,e.event_id,e.idempotency_key,e.event_type,e.payload,c.checkpoint_id,c.contract_version,c.state FROM ve_ledger_events e LEFT JOIN ve_ledger_checkpoints c ON c.scope=e.scope AND c.aggregate_id=e.aggregate_id AND c.sequence=e.sequence LEFT JOIN ve_ledger_outbox o ON o.scope=e.scope AND o.aggregate_id=e.aggregate_id AND o.sequence=e.sequence WHERE o.scope IS NULL OR (c.checkpoint_id IS NOT NULL AND o.checkpoint_json IS NULL) ORDER BY e.rowid LIMIT 500")?;
                    let rows = statement.query_map([], |row| {
                        Ok((
                            LedgerEvent {
                                scope: row.get(0)?,
                                aggregate: row.get(1)?,
                                sequence: row.get::<_, i64>(2)? as u64,
                                event_id: row.get(3)?,
                                idempotency_key: row.get(4)?,
                                event_type: row.get(5)?,
                                payload: serde_json::from_str(&row.get::<_, String>(6)?)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            },
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<i64>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                        ))
                    })?;
                    let mut projections = Vec::new();
                    for row in rows {
                        let (event, checkpoint_id, checkpoint_version, state) = row?;
                        let checkpoint = match (checkpoint_id, checkpoint_version, state) {
                            (Some(checkpoint_id), Some(contract_version), Some(state)) => {
                                Some(Checkpoint {
                                    contract_version: u16::try_from(contract_version)
                                        .map_err(|_| LedgerError::Storage)?,
                                    scope: event.scope.clone(),
                                    aggregate: event.aggregate.clone(),
                                    checkpoint_id,
                                    sequence: event.sequence,
                                    state: serde_json::from_str(&state)
                                        .map_err(|_| LedgerError::Storage)?,
                                })
                            }
                            (None, None, None) => None,
                            _ => return Err(LedgerError::Storage),
                        };
                        projections.push((event, checkpoint));
                    }
                    projections
                };
                if projections.is_empty() {
                    break;
                }
                for (event, checkpoint) in projections {
                    Self::queue_projection_in(&tx, &event, checkpoint.as_ref(), None)?;
                }
            }
            tx.execute(
                "UPDATE ve_ledger_meta SET projection_enabled=1 WHERE id=1",
                [],
            )?;
        }
        let admission_backfill_complete: bool = tx.query_row(
            "SELECT admission_backfill_complete FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if !admission_backfill_complete {
            loop {
                let admissions = {
                    let mut statement = tx.prepare("SELECT e.scope,e.aggregate_id,e.sequence,e.event_id,e.idempotency_key,e.event_type,e.payload,r.thread_id,r.turn_id,r.run_id,r.idempotency_key,r.request FROM ve_run_admissions a JOIN ve_run_reservations r ON r.scope=a.scope AND r.run_id=a.run_id JOIN ve_ledger_events e ON e.scope=a.scope AND e.event_id=a.event_id LEFT JOIN ve_ledger_outbox o ON o.scope=e.scope AND o.aggregate_id=e.aggregate_id AND o.sequence=e.sequence WHERE o.scope IS NULL OR o.reservation_json IS NULL ORDER BY e.rowid LIMIT 500")?;
                    let rows = statement.query_map([], |row| {
                        Ok((
                            LedgerEvent {
                                scope: row.get(0)?,
                                aggregate: row.get(1)?,
                                sequence: row.get::<_, i64>(2)? as u64,
                                event_id: row.get(3)?,
                                idempotency_key: row.get(4)?,
                                event_type: row.get(5)?,
                                payload: serde_json::from_str(&row.get::<_, String>(6)?)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            },
                            RunReservation {
                                scope: row.get(0)?,
                                thread_id: row.get(7)?,
                                turn_id: row.get(8)?,
                                run_id: row.get(9)?,
                                idempotency_key: row.get(10)?,
                                request: serde_json::from_str(&row.get::<_, String>(11)?)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            },
                        ))
                    })?;
                    let mut admissions = Vec::new();
                    for row in rows {
                        admissions.push(row?);
                    }
                    admissions
                };
                if admissions.is_empty() {
                    break;
                }
                for (event, reservation) in admissions {
                    Self::queue_projection_in(&tx, &event, None, Some(&reservation))?;
                }
            }
            tx.execute(
                "UPDATE ve_ledger_meta SET admission_backfill_complete=1 WHERE id=1",
                [],
            )?;
        }
        let reservation_backfill_complete: bool = tx.query_row(
            "SELECT reservation_backfill_complete FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if !reservation_backfill_complete {
            loop {
                let reservations = {
                    let mut statement = tx.prepare("SELECT r.scope,r.thread_id,r.turn_id,r.run_id,r.idempotency_key,r.request FROM ve_run_reservations r LEFT JOIN ve_run_admissions a ON a.scope=r.scope AND a.run_id=r.run_id LEFT JOIN ve_run_reservation_outbox o ON o.scope=r.scope AND o.thread_id=r.thread_id AND o.turn_id=r.turn_id WHERE a.run_id IS NULL AND o.scope IS NULL ORDER BY r.rowid LIMIT 500")?;
                    let rows = statement.query_map([], |row| {
                        Ok(RunReservation {
                            scope: row.get(0)?,
                            thread_id: row.get(1)?,
                            turn_id: row.get(2)?,
                            run_id: row.get(3)?,
                            idempotency_key: row.get(4)?,
                            request: serde_json::from_str(&row.get::<_, String>(5)?)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        })
                    })?;
                    let mut reservations = Vec::new();
                    for row in rows {
                        reservations.push(row?);
                    }
                    reservations
                };
                if reservations.is_empty() {
                    break;
                }
                for reservation in reservations {
                    Self::queue_reservation_projection_in(&tx, &reservation)?;
                }
            }
            tx.execute(
                "UPDATE ve_ledger_meta SET reservation_backfill_complete=1 WHERE id=1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Requeues the complete local authority for an explicit PostgreSQL rebuild.
    pub fn requeue_projection(&mut self) -> Result<(), LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let enabled: bool = tx.query_row(
            "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if !enabled {
            return Err(LedgerError::InvalidInput);
        }
        tx.execute_batch(
            "DELETE FROM ve_ledger_outbox;
             DELETE FROM ve_run_reservation_outbox;
             UPDATE ve_ledger_meta SET projection_enabled=0,
                 admission_backfill_complete=0,
                 reservation_backfill_complete=0 WHERE id=1;",
        )?;
        loop {
            let projections = {
                let mut statement = tx.prepare("SELECT e.scope,e.aggregate_id,e.sequence,e.event_id,e.idempotency_key,e.event_type,e.payload,c.checkpoint_id,c.contract_version,c.state FROM ve_ledger_events e LEFT JOIN ve_ledger_checkpoints c ON c.scope=e.scope AND c.aggregate_id=e.aggregate_id AND c.sequence=e.sequence LEFT JOIN ve_ledger_outbox o ON o.scope=e.scope AND o.aggregate_id=e.aggregate_id AND o.sequence=e.sequence WHERE o.scope IS NULL ORDER BY e.rowid LIMIT 500")?;
                let rows = statement.query_map([], |row| {
                    Ok((
                        LedgerEvent {
                            scope: row.get(0)?,
                            aggregate: row.get(1)?,
                            sequence: row.get::<_, i64>(2)? as u64,
                            event_id: row.get(3)?,
                            idempotency_key: row.get(4)?,
                            event_type: row.get(5)?,
                            payload: serde_json::from_str(&row.get::<_, String>(6)?)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        },
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                })?;
                let mut projections = Vec::new();
                for row in rows {
                    let (event, checkpoint_id, checkpoint_version, state) = row?;
                    let checkpoint = match (checkpoint_id, checkpoint_version, state) {
                        (Some(checkpoint_id), Some(contract_version), Some(state)) => {
                            Some(Checkpoint {
                                contract_version: u16::try_from(contract_version)
                                    .map_err(|_| LedgerError::Storage)?,
                                scope: event.scope.clone(),
                                aggregate: event.aggregate.clone(),
                                checkpoint_id,
                                sequence: event.sequence,
                                state: serde_json::from_str(&state)
                                    .map_err(|_| LedgerError::Storage)?,
                            })
                        }
                        (None, None, None) => None,
                        _ => return Err(LedgerError::Storage),
                    };
                    projections.push((event, checkpoint));
                }
                projections
            };
            if projections.is_empty() {
                break;
            }
            for (event, checkpoint) in projections {
                Self::queue_projection_in(&tx, &event, checkpoint.as_ref(), None)?;
            }
        }
        tx.execute(
            "UPDATE ve_ledger_meta SET projection_enabled=1 WHERE id=1",
            [],
        )?;
        tx.commit()?;
        self.enable_projection()
    }

    /// Counts authoritative rows covered by PostgreSQL projection.
    pub fn projection_scopes(&self) -> Result<Vec<String>, LedgerError> {
        let mut statement = self.connection.prepare(
            "SELECT scope FROM ve_ledger_events
             UNION SELECT scope FROM ve_run_reservations ORDER BY scope",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn projection_counts(&self, scopes: &[String]) -> Result<ProjectionCounts, LedgerError> {
        let mut counts = ProjectionCounts::default();
        for scope in scopes {
            let per_scope = self.connection.query_row(
                "SELECT (SELECT COUNT(DISTINCT aggregate_id) FROM ve_ledger_events WHERE scope=?1),
                        (SELECT COUNT(*) FROM ve_ledger_events WHERE scope=?1),
                        (SELECT COUNT(*) FROM ve_ledger_checkpoints WHERE scope=?1),
                        (SELECT COUNT(*) FROM ve_run_reservations WHERE scope=?1),
                        (SELECT COUNT(*) FROM ve_run_admissions WHERE scope=?1)",
                [scope],
                |row| {
                    Ok(ProjectionCounts {
                        aggregates: row.get(0)?,
                        events: row.get(1)?,
                        checkpoints: row.get(2)?,
                        reservations: row.get(3)?,
                        admissions: row.get(4)?,
                    })
                },
            )?;
            counts.aggregates += per_scope.aggregates;
            counts.events += per_scope.events;
            counts.checkpoints += per_scope.checkpoints;
            counts.reservations += per_scope.reservations;
            counts.admissions += per_scope.admissions;
        }
        Ok(counts)
    }

    pub fn projection_enabled(&self) -> Result<bool, LedgerError> {
        self.connection
            .query_row(
                "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// Appends locally and queues a durable mirror record in the same transaction.
    pub fn append_with_outbox(&mut self, draft: EventDraft) -> Result<LedgerEvent, LedgerError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let event = Self::append_in(&tx, draft)?;
        Self::queue_projection_in(&tx, &event, None, None)?;
        tx.commit()?;
        Ok(event)
    }

    fn queue_projection_in(
        tx: &Transaction<'_>,
        event: &LedgerEvent,
        checkpoint: Option<&Checkpoint>,
        reservation: Option<&RunReservation>,
    ) -> Result<(), LedgerError> {
        let event_json = serde_json::to_string(event).map_err(|_| LedgerError::InvalidInput)?;
        let checkpoint_json = checkpoint
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| LedgerError::InvalidInput)?;
        let reservation_json = reservation
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| LedgerError::InvalidInput)?;
        tx.execute(
            "INSERT INTO ve_ledger_outbox(scope,aggregate_id,sequence,event_json,checkpoint_json,reservation_json) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(scope,aggregate_id,sequence) DO UPDATE SET checkpoint_json=COALESCE(ve_ledger_outbox.checkpoint_json,excluded.checkpoint_json),reservation_json=COALESCE(ve_ledger_outbox.reservation_json,excluded.reservation_json),delivered=CASE WHEN (excluded.checkpoint_json IS NOT NULL AND ve_ledger_outbox.checkpoint_json IS NULL) OR (excluded.reservation_json IS NOT NULL AND ve_ledger_outbox.reservation_json IS NULL) THEN 0 ELSE ve_ledger_outbox.delivered END",
            params![event.scope, event.aggregate, event.sequence as i64, event_json, checkpoint_json, reservation_json],
        )?;
        Ok(())
    }

    fn queue_reservation_projection_in(
        tx: &Transaction<'_>,
        reservation: &RunReservation,
    ) -> Result<(), LedgerError> {
        let reservation_json =
            serde_json::to_string(reservation).map_err(|_| LedgerError::InvalidInput)?;
        tx.execute(
            "INSERT INTO ve_run_reservation_outbox(scope,thread_id,turn_id,reservation_json) VALUES(?1,?2,?3,?4) ON CONFLICT(scope,thread_id,turn_id) DO UPDATE SET reservation_json=excluded.reservation_json,delivered=0",
            params![reservation.scope,reservation.thread_id,reservation.turn_id,reservation_json],
        )?;
        Ok(())
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

    pub fn pending_projections(&self, limit: u32) -> Result<Vec<PendingProjection>, LedgerError> {
        if !(1..=1000).contains(&limit) {
            return Err(LedgerError::InvalidInput);
        }
        let mut stmt = self.connection.prepare("SELECT event_json,checkpoint_json,reservation_json FROM ve_ledger_outbox WHERE delivered=0 ORDER BY scope,aggregate_id,sequence LIMIT ?1")?;
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (event, checkpoint, reservation) = row?;
            Ok(PendingProjection {
                event: serde_json::from_str(&event).map_err(|_| LedgerError::Storage)?,
                checkpoint: checkpoint
                    .map(|json| serde_json::from_str(&json).map_err(|_| LedgerError::Storage))
                    .transpose()?,
                reservation: reservation
                    .map(|json| serde_json::from_str(&json).map_err(|_| LedgerError::Storage))
                    .transpose()?,
            })
        })
        .collect()
    }

    pub fn pending_outbox_count(&self) -> Result<u64, LedgerError> {
        self.connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM ve_ledger_outbox WHERE delivered=0) + (SELECT COUNT(*) FROM ve_run_reservation_outbox WHERE delivered=0)",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn pending_reservations(&self, limit: u32) -> Result<Vec<RunReservation>, LedgerError> {
        if !(1..=1000).contains(&limit) {
            return Err(LedgerError::InvalidInput);
        }
        let mut statement = self.connection.prepare("SELECT reservation_json FROM ve_run_reservation_outbox WHERE delivered=0 ORDER BY scope,thread_id,turn_id LIMIT ?1")?;
        let rows = statement.query_map([limit], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(|_| LedgerError::Storage))
            .collect()
    }

    pub fn mark_reservation_delivered(
        &mut self,
        scope: &str,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<(), LedgerError> {
        if [scope, thread_id, turn_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(LedgerError::InvalidInput);
        }
        self.connection.execute(
            "DELETE FROM ve_run_reservation_outbox WHERE scope=?1 AND thread_id=?2 AND turn_id=?3",
            params![scope, thread_id, turn_id],
        )?;
        Ok(())
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
        self.connection.execute(
            "DELETE FROM ve_ledger_outbox WHERE scope=?1 AND aggregate_id=?2 AND sequence=?3",
            params![scope, aggregate, sequence as i64],
        )?;
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

    fn append_checkpointed_in(
        tx: &Transaction<'_>,
        draft: EventDraft,
        checkpoint_id: String,
        state: Value,
    ) -> Result<CheckpointedEvent, LedgerError> {
        if checkpoint_id.trim().is_empty() {
            return Err(LedgerError::InvalidInput);
        }
        let event = Self::append_in(tx, draft)?;
        let checkpoint = Checkpoint {
            contract_version: CHECKPOINT_VERSION,
            scope: event.scope.clone(),
            aggregate: event.aggregate.clone(),
            checkpoint_id,
            sequence: event.sequence,
            state,
        };
        Self::save_checkpoint_in(tx, &checkpoint)?;
        Ok(CheckpointedEvent { event, checkpoint })
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
        let projection_enabled: bool = tx.query_row(
            "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if projection_enabled {
            Self::queue_reservation_projection_in(&tx, &result)?;
        }
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
        let projection_enabled: bool = tx.query_row(
            "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if projection_enabled {
            tx.execute(
                "DELETE FROM ve_run_reservation_outbox WHERE scope=?1 AND thread_id=?2 AND turn_id=?3",
                params![reservation.scope, reservation.thread_id, reservation.turn_id],
            )?;
            Self::queue_projection_in(&tx, &event, None, Some(&reservation))?;
        }
        tx.commit()?;
        Ok(Admission { reservation, event })
    }

    /// Atomically reserve a run, append its thread admission and persist its
    /// initial lifecycle checkpoint in the run aggregate.
    pub fn admit_run_checkpointed(
        &mut self,
        reservation: RunReservation,
        mut admission_event: EventDraft,
        lifecycle_event: EventDraft,
        checkpoint_id: String,
        state: Value,
    ) -> Result<CheckpointedAdmission, LedgerError> {
        if reservation.scope != admission_event.scope
            || reservation.thread_id != admission_event.aggregate
            || reservation.scope != lifecycle_event.scope
            || lifecycle_event.aggregate == admission_event.aggregate
        {
            return Err(LedgerError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let reservation = Self::reserve_run_in(&tx, reservation)?;
        let prior: Option<(String, String)> = tx.query_row(
            "SELECT run_id,event_id FROM ve_run_admissions WHERE scope=?1 AND (run_id=?2 OR event_id=?3) LIMIT 1",
            params![reservation.scope, reservation.run_id, admission_event.event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        if prior.as_ref().is_some_and(|(run_id, event_id)| {
            run_id != &reservation.run_id || event_id != &admission_event.event_id
        }) {
            return Err(LedgerError::IdempotencyConflict);
        }
        let current_thread_sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence),0) FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2",
            params![admission_event.scope, admission_event.aggregate],
            |row| row.get(0),
        )?;
        admission_event.expected_version = current_thread_sequence as u64;
        let admission_event = Self::append_in(&tx, admission_event)?;
        if prior.is_none() {
            tx.execute(
                "INSERT INTO ve_run_admissions(scope,run_id,event_id) VALUES(?1,?2,?3)",
                params![
                    reservation.scope,
                    reservation.run_id,
                    admission_event.event_id
                ],
            )?;
        }
        let lifecycle = Self::append_checkpointed_in(&tx, lifecycle_event, checkpoint_id, state)?;
        let projection_enabled: bool = tx.query_row(
            "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if projection_enabled {
            tx.execute(
                "DELETE FROM ve_run_reservation_outbox WHERE scope=?1 AND thread_id=?2 AND turn_id=?3",
                params![reservation.scope, reservation.thread_id, reservation.turn_id],
            )?;
            Self::queue_projection_in(&tx, &admission_event, None, Some(&reservation))?;
            Self::queue_projection_in(&tx, &lifecycle.event, Some(&lifecycle.checkpoint), None)?;
        }
        tx.commit()?;
        Ok(CheckpointedAdmission {
            reservation,
            admission_event,
            lifecycle,
        })
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
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::save_checkpoint_in(&tx, &checkpoint)?;
        tx.commit()?;
        Ok(checkpoint)
    }

    fn save_checkpoint_in(
        tx: &Transaction<'_>,
        checkpoint: &Checkpoint,
    ) -> Result<(), LedgerError> {
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
        Ok(())
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
    fn checkpointed_append_commits_event_and_snapshot_atomically() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let draft = EventDraft {
            scope: "scope".into(),
            aggregate: "thread".into(),
            event_id: "event-1".into(),
            idempotency_key: "effect-started-1".into(),
            event_type: "tool_effect_started".into(),
            payload: json!({"call_id":"call-1"}),
            expected_version: 0,
        };
        let saved = ledger
            .append_checkpointed(
                draft.clone(),
                "checkpoint-1".into(),
                json!({"phase":"effect_started","call_id":"call-1"}),
            )
            .unwrap();
        assert_eq!(saved.event.sequence, 1);
        assert_eq!(saved.checkpoint.sequence, saved.event.sequence);
        assert_eq!(
            ledger.latest_checkpoint("scope", "thread").unwrap(),
            Some(saved.checkpoint.clone())
        );
        assert_eq!(
            ledger
                .append_checkpointed(
                    draft,
                    "checkpoint-1".into(),
                    json!({"phase":"effect_started","call_id":"call-1"})
                )
                .unwrap(),
            saved
        );
    }

    #[test]
    fn checkpointed_append_rolls_back_event_when_snapshot_is_invalid() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let draft = EventDraft {
            scope: "scope".into(),
            aggregate: "thread".into(),
            event_id: "event-1".into(),
            idempotency_key: "effect-started-1".into(),
            event_type: "tool_effect_started".into(),
            payload: json!({}),
            expected_version: 0,
        };
        let oversized_state = json!({"state":"x".repeat(MAX_PAYLOAD_BYTES + 1)});
        assert!(matches!(
            ledger.append_checkpointed(draft, "checkpoint-1".into(), oversized_state),
            Err(LedgerError::InvalidInput)
        ));
        assert!(ledger.read("scope", "thread", 0, 10).unwrap().is_empty());
        assert!(
            ledger
                .latest_checkpoint("scope", "thread")
                .unwrap()
                .is_none()
        );
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
        ledger.connection.execute_batch("DROP TABLE ve_run_reservation_outbox; DROP TABLE ve_run_reservations; DROP TABLE ve_ledger_checkpoints; DROP TABLE ve_run_admissions; ALTER TABLE ve_ledger_outbox DROP COLUMN reservation_json; DROP TABLE ve_ledger_outbox; ALTER TABLE ve_ledger_meta DROP COLUMN reservation_backfill_complete; ALTER TABLE ve_ledger_meta DROP COLUMN admission_backfill_complete; ALTER TABLE ve_ledger_meta DROP COLUMN projection_enabled; UPDATE ve_ledger_meta SET version=1;").unwrap();
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

    #[test]
    fn checkpointed_outbox_keeps_event_and_snapshot_together() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let draft = EventDraft {
            scope: "s".into(),
            aggregate: "run:1:x".into(),
            event_id: "event-1".into(),
            idempotency_key: "operation-1".into(),
            event_type: "run_lifecycle_transition".into(),
            payload: json!({"state":{"status":"running"}}),
            expected_version: 0,
        };
        let checkpoint_state = json!({"contract_version":1,"status":"running"});
        let saved = ledger
            .append_checkpointed_with_outbox(
                draft.clone(),
                "checkpoint-1".into(),
                checkpoint_state.clone(),
            )
            .unwrap();
        let pending = ledger.pending_projections(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].event, saved.event);
        assert_eq!(pending[0].checkpoint, Some(saved.checkpoint.clone()));
        assert_eq!(
            ledger
                .append_checkpointed_with_outbox(draft, "checkpoint-1".into(), checkpoint_state,)
                .unwrap(),
            saved
        );
        assert_eq!(ledger.pending_outbox_count().unwrap(), 1);
    }

    #[test]
    fn projection_enable_backfills_history_and_queues_future_checkpoints() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let event = EventDraft {
            scope: "s".into(),
            aggregate: "run".into(),
            event_id: "event-1".into(),
            idempotency_key: "operation-1".into(),
            event_type: "run_started".into(),
            payload: json!({"status":"running"}),
            expected_version: 0,
        };
        let first = ledger
            .append_checkpointed(event.clone(), "checkpoint-1".into(), json!({"n":1}))
            .unwrap();
        assert!(!ledger.projection_enabled().unwrap());
        assert_eq!(ledger.pending_outbox_count().unwrap(), 0);

        ledger.enable_projection().unwrap();
        assert!(ledger.projection_enabled().unwrap());
        assert_eq!(ledger.pending_outbox_count().unwrap(), 1);
        assert_eq!(
            ledger.pending_projections(10).unwrap()[0].checkpoint,
            Some(first.checkpoint)
        );

        let second = EventDraft {
            event_id: "event-2".into(),
            idempotency_key: "operation-2".into(),
            event_type: "run_finished".into(),
            payload: json!({"status":"completed"}),
            expected_version: 1,
            ..event
        };
        ledger
            .append_checkpointed_projected(second, "checkpoint-2".into(), json!({"n":2}))
            .unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 2);
        ledger.enable_projection().unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 2);
    }

    #[test]
    fn projection_backfills_and_queues_reservation_with_admission() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let reservation = RunReservation {
            scope: "scope".into(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "request-1".into(),
            request: json!({"prompt":"hello"}),
        };
        let event = EventDraft {
            scope: "scope".into(),
            aggregate: "thread".into(),
            event_id: "admission-event".into(),
            idempotency_key: "admission-1".into(),
            event_type: "run_admitted".into(),
            payload: json!({"run_id":"run"}),
            expected_version: 0,
        };
        ledger
            .admit_run_checkpointed(
                reservation.clone(),
                event,
                EventDraft {
                    scope: "scope".into(),
                    aggregate: "run".into(),
                    event_id: "lifecycle-event".into(),
                    idempotency_key: "lifecycle-1".into(),
                    event_type: "run_started".into(),
                    payload: json!({"status":"planned"}),
                    expected_version: 0,
                },
                "run-checkpoint".into(),
                json!({"status":"planned"}),
            )
            .unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 0);

        ledger.enable_projection().unwrap();
        let pending = ledger.pending_projections(10).unwrap();
        assert_eq!(pending.len(), 2);
        let admission = pending
            .iter()
            .find(|projection| projection.event.event_id == "admission-event")
            .unwrap();
        assert_eq!(admission.reservation, Some(reservation.clone()));
        assert!(admission.checkpoint.is_none());
        let lifecycle = pending
            .iter()
            .find(|projection| projection.event.event_id == "lifecycle-event")
            .unwrap();
        assert_eq!(lifecycle.reservation, None);
        assert!(lifecycle.checkpoint.is_some());

        ledger.enable_projection().unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 2);
    }

    #[test]
    fn standalone_run_reservations_are_backfilled_and_queued_after_opt_in() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let reservation = RunReservation {
            scope: "scope".into(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "request-1".into(),
            request: json!({"prompt":"hello"}),
        };
        ledger.reserve_run(reservation.clone()).unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 0);

        ledger.enable_projection().unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 1);
        assert_eq!(ledger.pending_reservations(10).unwrap(), vec![reservation]);
        ledger.enable_projection().unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 1);
    }

    #[test]
    fn admission_without_checkpoint_queues_event_and_reservation_together() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        ledger.enable_projection().unwrap();
        let reservation = RunReservation {
            scope: "scope".into(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "request-1".into(),
            request: json!({"prompt":"hello"}),
        };
        let admission = ledger
            .admit_run(
                reservation.clone(),
                EventDraft {
                    scope: "scope".into(),
                    aggregate: "thread".into(),
                    event_id: "admission-event".into(),
                    idempotency_key: "admission-1".into(),
                    event_type: "run_admitted".into(),
                    payload: json!({"run_id":"run"}),
                    expected_version: 0,
                },
            )
            .unwrap();
        assert_eq!(admission.reservation, reservation);
        let pending = ledger.pending_projections(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].event, admission.event);
        assert_eq!(pending[0].reservation, Some(admission.reservation));
        assert_eq!(
            ledger.pending_reservations(10).unwrap(),
            Vec::<RunReservation>::new()
        );
    }

    #[test]
    fn upgrading_enabled_v6_projection_backfills_reservations_once() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let standalone = RunReservation {
            scope: "scope".into(),
            thread_id: "standalone-thread".into(),
            turn_id: "turn-standalone".into(),
            run_id: "run-standalone".into(),
            idempotency_key: "request-standalone".into(),
            request: json!({"prompt":"standalone"}),
        };
        let admitted = RunReservation {
            scope: "scope".into(),
            thread_id: "admitted-thread".into(),
            turn_id: "turn-admitted".into(),
            run_id: "run-admitted".into(),
            idempotency_key: "request-admitted".into(),
            request: json!({"prompt":"admitted"}),
        };
        ledger.reserve_run(standalone.clone()).unwrap();
        ledger
            .admit_run(
                admitted.clone(),
                EventDraft {
                    scope: admitted.scope.clone(),
                    aggregate: admitted.thread_id.clone(),
                    event_id: "admission-event".into(),
                    idempotency_key: "admission-operation".into(),
                    event_type: "run_admitted".into(),
                    payload: json!({"run_id":"run-admitted"}),
                    expected_version: 0,
                },
            )
            .unwrap();
        ledger.enable_projection().unwrap();
        ledger
            .connection
            .execute_batch("DELETE FROM ve_ledger_outbox; DELETE FROM ve_run_reservation_outbox; DROP TABLE ve_run_reservation_outbox; ALTER TABLE ve_ledger_outbox DROP COLUMN reservation_json; ALTER TABLE ve_ledger_meta DROP COLUMN admission_backfill_complete; ALTER TABLE ve_ledger_meta DROP COLUMN reservation_backfill_complete; UPDATE ve_ledger_meta SET version=6;")
            .unwrap();

        ledger.migrate().unwrap();
        ledger.enable_projection().unwrap();
        let events = ledger.pending_projections(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].reservation, Some(admitted));
        assert_eq!(ledger.pending_reservations(10).unwrap(), vec![standalone]);
        assert_eq!(ledger.pending_outbox_count().unwrap(), 2);

        ledger.enable_projection().unwrap();
        assert_eq!(ledger.pending_outbox_count().unwrap(), 2);
    }

    fn temp_ledger_path(label: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        std::env::temp_dir().join(format!(
            "vectora-ledger-{label}-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn draft(event_id: &str, key: &str, expected_version: u64) -> EventDraft {
        EventDraft {
            scope: "scope".into(),
            aggregate: "run".into(),
            event_id: event_id.into(),
            idempotency_key: key.into(),
            event_type: "step".into(),
            payload: json!({"id": event_id}),
            expected_version,
        }
    }

    #[test]
    fn concurrent_appends_with_same_expected_version_commit_exactly_one() {
        use std::sync::{Arc, Barrier};
        let path = temp_ledger_path("version");
        drop(SqliteLedger::open_persistent(&path).unwrap());
        let barrier = Arc::new(Barrier::new(2));
        let threads: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|id| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut ledger = SqliteLedger::open_persistent(path).unwrap();
                    barrier.wait();
                    ledger.append(draft(id, id, 0))
                })
            })
            .collect();
        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(LedgerError::VersionConflict)))
                .count(),
            1
        );
        let ledger = SqliteLedger::open_persistent(&path).unwrap();
        assert_eq!(ledger.read("scope", "run", 0, 10).unwrap().len(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_identical_retries_return_one_event() {
        use std::sync::{Arc, Barrier};
        let path = temp_ledger_path("retry");
        drop(SqliteLedger::open_persistent(&path).unwrap());
        let barrier = Arc::new(Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut ledger = SqliteLedger::open_persistent(path).unwrap();
                    barrier.wait();
                    ledger.append(draft("e1", "key", 0))
                })
            })
            .collect();
        for thread in threads {
            assert_eq!(thread.join().unwrap().unwrap().sequence, 1);
        }
        let ledger = SqliteLedger::open_persistent(&path).unwrap();
        assert_eq!(ledger.read("scope", "run", 0, 10).unwrap().len(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn persisted_events_and_idempotency_survive_reopen() {
        let path = temp_ledger_path("restart");
        {
            let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
            ledger.append(draft("e1", "k1", 0)).unwrap();
            ledger.append(draft("e2", "k2", 1)).unwrap();
        }
        let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
        let events = ledger.read("scope", "run", 0, 10).unwrap();
        assert_eq!(
            events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(ledger.append(draft("e1", "k1", 0)).unwrap().sequence, 1);
        assert!(matches!(
            ledger.append(draft("e3", "k3", 1)),
            Err(LedgerError::VersionConflict)
        ));
        assert_eq!(ledger.append(draft("e3", "k3", 2)).unwrap().sequence, 3);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_rejects_invalid_cursor_and_limit() {
        let ledger = SqliteLedger::open(":memory:").unwrap();
        for (after, limit) in [(0, 0), (0, 1001), (i64::MAX as u64 + 1, 10)] {
            assert!(matches!(
                ledger.read("scope", "run", after, limit),
                Err(LedgerError::InvalidInput)
            ));
        }
        assert!(matches!(
            ledger.read(" ", "run", 0, 10),
            Err(LedgerError::InvalidInput)
        ));
    }

    /// Child side of the multi-process test: appends one event to the ledger
    /// named by the environment and reports the outcome on stdout.
    #[test]
    fn multiprocess_child_writer() {
        let (Ok(path), Ok(id)) = (
            std::env::var("VECTORA_LEDGER_CHILD_PATH"),
            std::env::var("VECTORA_LEDGER_CHILD_ID"),
        ) else {
            return;
        };
        let mut ledger = SqliteLedger::open_persistent(path).unwrap();
        let outcome = match ledger.append(draft(&id, &id, 0)) {
            Ok(event) => format!("committed:{}", event.sequence),
            Err(LedgerError::VersionConflict) => "version_conflict".to_owned(),
            Err(other) => format!("error:{other}"),
        };
        println!("CHILD_RESULT {outcome}");
    }

    #[test]
    fn independent_processes_commit_exactly_one_event_for_the_same_version() {
        let path = temp_ledger_path("processes");
        drop(SqliteLedger::open_persistent(&path).unwrap());
        let executable = std::env::current_exe().unwrap();
        let handles: Vec<_> = ["a", "b", "c"]
            .into_iter()
            .map(|id| {
                let executable = executable.clone();
                let path = path.clone();
                std::thread::spawn(move || {
                    std::process::Command::new(executable)
                        .args([
                            "storage::sqlite::tests::multiprocess_child_writer",
                            "--exact",
                            "--nocapture",
                            "--test-threads=1",
                        ])
                        .env("VECTORA_LEDGER_CHILD_PATH", path)
                        .env("VECTORA_LEDGER_CHILD_ID", id)
                        .output()
                        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
                        .unwrap()
                })
            })
            .collect();
        let outputs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let committed = outputs
            .iter()
            .filter(|out| out.contains("CHILD_RESULT committed:1"))
            .count();
        let conflicts = outputs
            .iter()
            .filter(|out| out.contains("CHILD_RESULT version_conflict"))
            .count();
        assert_eq!((committed, conflicts), (1, 2), "outputs: {outputs:?}");
        let ledger = SqliteLedger::open_persistent(&path).unwrap();
        assert_eq!(ledger.read("scope", "run", 0, 10).unwrap().len(), 1);
        std::fs::remove_file(path).unwrap();
    }
}
