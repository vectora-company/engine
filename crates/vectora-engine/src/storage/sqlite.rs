//! SQLite event ledger. The caller owns authorization and redaction of payloads.

use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const SCHEMA_VERSION: i64 = 1;
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
            Some(SCHEMA_VERSION) => {}
            Some(_) => return Err(LedgerError::SchemaVersion),
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
        tx.commit()?;
        Ok(())
    }

    /// Appends exactly once for an aggregate/idempotency key within SQLite.
    pub fn append(&mut self, draft: EventDraft) -> Result<LedgerEvent, LedgerError> {
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
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
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
        tx.commit()?;
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
}
