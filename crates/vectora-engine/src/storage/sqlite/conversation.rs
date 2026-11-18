//! Durable conversation event log on top of the SQLite ledger.
//!
//! Each conversation is one ledger aggregate. The store assigns the sequence
//! inside an immediate transaction, so concurrent writers on the same file get
//! contiguous, unique positions. The producer's `event_id` is the idempotency
//! key: retrying the same draft returns the stored event and its sequence,
//! while reusing the ID with different content is a conflict. Appends are
//! queued for the PostgreSQL projection exactly like other ledger events:
//! only once projection has been enabled on the ledger.
//!
//! History is read-only: reading a page never changes producer or consumer
//! state. Cursors are bound to the scope, the conversation and the contract
//! version. They are not signed; the host authorizes who may read a scope.

use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EventDraft, LedgerError, SqliteLedger};
use crate::harness::{
    CONVERSATION_EVENT_CONTRACT_VERSION, ConversationEvent, ConversationEventError,
    ConversationEventKind,
};

/// Version of the durable conversation log contract (drafts, cursors, pages).
pub const CONVERSATION_STORE_CONTRACT_VERSION: u16 = 1;
const EVENT_TYPE: &str = "conversation_event.v1";
const AGGREGATE_PREFIX: &str = "conversation_events:";
const MAX_HISTORY_PAGE: u32 = 1000;

/// A conversation event before the store assigns its sequence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationEventDraft {
    pub contract_version: u16,
    pub event_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    /// Producer timestamp in Unix epoch milliseconds; ordering comes from the
    /// store sequence, not from this value.
    pub timestamp_ms: u64,
    pub source: String,
    pub kind: ConversationEventKind,
}

impl ConversationEventDraft {
    fn with_sequence(self, sequence: u64) -> ConversationEvent {
        ConversationEvent {
            contract_version: self.contract_version,
            event_id: self.event_id,
            conversation_id: self.conversation_id,
            turn_id: self.turn_id,
            run_id: self.run_id,
            step_id: self.step_id,
            sequence,
            timestamp_ms: self.timestamp_ms,
            source: self.source,
            kind: self.kind,
        }
    }
}

/// Position after which the next history page starts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationCursor {
    pub contract_version: u16,
    pub scope: String,
    pub conversation_id: String,
    pub after_sequence: u64,
}

/// How history reads treat missing sequences.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationReadMode {
    /// A missing sequence is an error ([`ConversationStoreError::Gap`]).
    #[default]
    Strict,
    /// A missing sequence is reported in the page `gaps` and reading goes on.
    Tolerant,
}

/// Inclusive range of sequences that are not stored (never written, expired
/// or lost). Consumers must not treat the range as delivered.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationGap {
    pub from_sequence: u64,
    pub to_sequence: u64,
}

/// One page of history in sequence order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationHistoryPage {
    pub events: Vec<ConversationEvent>,
    /// Missing sequences found while reading the page; always empty in
    /// [`ConversationReadMode::Strict`].
    #[serde(default)]
    pub gaps: Vec<ConversationGap>,
    /// Cursor to request the following page; it equals the request cursor when
    /// the page is empty.
    pub next_cursor: ConversationCursor,
    /// Highest sequence stored for the conversation when the page was read.
    pub latest_sequence: u64,
}

#[derive(Debug, Error)]
pub enum ConversationStoreError {
    #[error(transparent)]
    Event(#[from] ConversationEventError),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("conversation store contract version {0} is not supported")]
    UnsupportedContractVersion(u16),
    #[error("conversation cursor does not belong to this scope or conversation")]
    CursorMismatch,
    #[error("conversation cursor is ahead of the stored history")]
    CursorAhead,
    #[error("event id is already used by conversation {conversation_id}")]
    EventIdReused { conversation_id: String },
    #[error("run {run_id} already has a terminal event in this conversation")]
    RunAlreadyTerminal { run_id: String },
    #[error("conversation history has a gap: expected sequence {expected}, found {found}")]
    Gap { expected: u64, found: u64 },
    #[error("conversation has no stored event with this id")]
    EventNotFound,
    #[error("stored conversation event is unreadable")]
    Corrupt,
}

fn aggregate(conversation_id: &str) -> String {
    format!("{AGGREGATE_PREFIX}{conversation_id}")
}

fn require_scope(scope: &str) -> Result<(), ConversationStoreError> {
    if scope.trim().is_empty() {
        Err(LedgerError::InvalidInput.into())
    } else {
        Ok(())
    }
}

impl SqliteLedger {
    /// Appends a validated conversation event and returns it with the sequence
    /// assigned by the store. Retrying an identical draft is idempotent.
    pub fn append_conversation_event(
        &mut self,
        scope: &str,
        draft: ConversationEventDraft,
    ) -> Result<ConversationEvent, ConversationStoreError> {
        require_scope(scope)?;
        if draft.contract_version != CONVERSATION_EVENT_CONTRACT_VERSION {
            return Err(ConversationEventError::UnsupportedContractVersion {
                received: draft.contract_version,
            }
            .into());
        }
        // Validate structure and correlation with a provisional sequence; the
        // stored event is validated again with its real sequence on read.
        draft.clone().with_sequence(1).validate()?;
        let payload = serde_json::to_value(&draft).map_err(|_| LedgerError::InvalidInput)?;
        let aggregate = aggregate(&draft.conversation_id);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(LedgerError::from)?;
        let current: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sequence),0) FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2",
                rusqlite::params![scope, aggregate],
                |row| row.get(0),
            )
            .map_err(LedgerError::from)?;
        let owner: Option<String> = tx
            .query_row(
                "SELECT aggregate_id FROM ve_ledger_events WHERE scope=?1 AND event_id=?2",
                rusqlite::params![scope, draft.event_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(LedgerError::from)?;
        let already_stored = owner.is_some();
        if let Some(owner) = owner.filter(|owner| *owner != aggregate) {
            return Err(ConversationStoreError::EventIdReused {
                conversation_id: owner
                    .strip_prefix(AGGREGATE_PREFIX)
                    .unwrap_or(&owner)
                    .to_owned(),
            });
        }
        if !already_stored && let Some(run_id) = draft.run_id.as_deref() {
            let terminal: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2 \
                     AND json_extract(payload,'$.run_id')=?3 \
                     AND json_extract(payload,'$.kind.type')='terminal'",
                    rusqlite::params![scope, aggregate, run_id],
                    |row| row.get(0),
                )
                .map_err(LedgerError::from)?;
            if terminal > 0 {
                return Err(ConversationStoreError::RunAlreadyTerminal {
                    run_id: run_id.to_owned(),
                });
            }
        }
        let event = Self::append_in(
            &tx,
            EventDraft {
                scope: scope.to_owned(),
                aggregate,
                event_id: draft.event_id.clone(),
                idempotency_key: draft.event_id.clone(),
                event_type: EVENT_TYPE.to_owned(),
                payload,
                expected_version: current as u64,
            },
        )?;
        let projection_enabled: bool = tx
            .query_row(
                "SELECT projection_enabled FROM ve_ledger_meta WHERE id=1",
                [],
                |row| row.get(0),
            )
            .map_err(LedgerError::from)?;
        if projection_enabled {
            Self::queue_projection_in(&tx, &event, None, None)?;
        }
        tx.commit().map_err(LedgerError::from)?;
        let stored = draft.with_sequence(event.sequence);
        stored.validate()?;
        Ok(stored)
    }

    /// Whether the scope already stores an event with this ID.
    pub(crate) fn conversation_event_exists(
        &self,
        scope: &str,
        event_id: &str,
    ) -> Result<bool, ConversationStoreError> {
        require_scope(scope)?;
        let exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ve_ledger_events WHERE scope=?1 AND event_id=?2 AND event_type=?3)",
                rusqlite::params![scope, event_id, EVENT_TYPE],
                |row| row.get(0),
            )
            .map_err(LedgerError::from)?;
        Ok(exists)
    }

    /// Reads up to `limit` events after the cursor (or from the start),
    /// failing on a missing sequence.
    pub fn conversation_history(
        &self,
        scope: &str,
        conversation_id: &str,
        cursor: Option<&ConversationCursor>,
        limit: u32,
    ) -> Result<ConversationHistoryPage, ConversationStoreError> {
        self.conversation_history_with_mode(
            scope,
            conversation_id,
            cursor,
            limit,
            ConversationReadMode::Strict,
        )
    }

    /// Cursor that resumes right after a stored event, for reconnecting
    /// consumers that only remember the last `event_id` they processed.
    pub fn conversation_cursor_after_event(
        &self,
        scope: &str,
        conversation_id: &str,
        event_id: &str,
    ) -> Result<ConversationCursor, ConversationStoreError> {
        require_scope(scope)?;
        if conversation_id.trim().is_empty() || event_id.trim().is_empty() {
            return Err(LedgerError::InvalidInput.into());
        }
        let sequence: Option<i64> = self
            .connection
            .query_row(
                "SELECT sequence FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2 AND event_id=?3 AND event_type=?4",
                rusqlite::params![scope, aggregate(conversation_id), event_id, EVENT_TYPE],
                |row| row.get(0),
            )
            .optional()
            .map_err(LedgerError::from)?;
        let sequence = sequence.ok_or(ConversationStoreError::EventNotFound)?;
        Ok(ConversationCursor {
            contract_version: CONVERSATION_STORE_CONTRACT_VERSION,
            scope: scope.to_owned(),
            conversation_id: conversation_id.to_owned(),
            after_sequence: sequence as u64,
        })
    }

    /// Like [`Self::conversation_history`], choosing how missing sequences are
    /// handled. A cursor that points before the oldest retained event shows up
    /// as a leading gap in tolerant mode.
    pub fn conversation_history_with_mode(
        &self,
        scope: &str,
        conversation_id: &str,
        cursor: Option<&ConversationCursor>,
        limit: u32,
        mode: ConversationReadMode,
    ) -> Result<ConversationHistoryPage, ConversationStoreError> {
        require_scope(scope)?;
        if conversation_id.trim().is_empty() || !(1..=MAX_HISTORY_PAGE).contains(&limit) {
            return Err(LedgerError::InvalidInput.into());
        }
        let after = match cursor {
            None => 0,
            Some(cursor) => {
                if cursor.contract_version != CONVERSATION_STORE_CONTRACT_VERSION {
                    return Err(ConversationStoreError::UnsupportedContractVersion(
                        cursor.contract_version,
                    ));
                }
                if cursor.scope != scope || cursor.conversation_id != conversation_id {
                    return Err(ConversationStoreError::CursorMismatch);
                }
                cursor.after_sequence
            }
        };
        let aggregate = aggregate(conversation_id);
        // One read transaction keeps `latest_sequence` and the page consistent.
        let snapshot = self
            .connection
            .unchecked_transaction()
            .map_err(LedgerError::from)?;
        let latest: i64 = snapshot
            .query_row(
                "SELECT COALESCE(MAX(sequence),0) FROM ve_ledger_events WHERE scope=?1 AND aggregate_id=?2",
                rusqlite::params![scope, aggregate],
                |row| row.get(0),
            )
            .map_err(LedgerError::from)?;
        let latest = latest as u64;
        if after > latest {
            return Err(ConversationStoreError::CursorAhead);
        }
        let mut events = Vec::new();
        let mut gaps = Vec::new();
        let mut expected = after + 1;
        for stored in self.read(scope, &aggregate, after, limit)? {
            if stored.sequence != expected {
                if mode == ConversationReadMode::Strict {
                    return Err(ConversationStoreError::Gap {
                        expected,
                        found: stored.sequence,
                    });
                }
                gaps.push(ConversationGap {
                    from_sequence: expected,
                    to_sequence: stored.sequence - 1,
                });
            }
            if stored.event_type != EVENT_TYPE {
                return Err(ConversationStoreError::Corrupt);
            }
            let draft: ConversationEventDraft = serde_json::from_value(stored.payload)
                .map_err(|_| ConversationStoreError::Corrupt)?;
            if draft.conversation_id != conversation_id || draft.event_id != stored.event_id {
                return Err(ConversationStoreError::Corrupt);
            }
            let event = draft.with_sequence(stored.sequence);
            event.validate()?;
            events.push(event);
            expected = stored.sequence + 1;
        }
        drop(snapshot);
        Ok(ConversationHistoryPage {
            next_cursor: ConversationCursor {
                contract_version: CONVERSATION_STORE_CONTRACT_VERSION,
                scope: scope.to_owned(),
                conversation_id: conversation_id.to_owned(),
                after_sequence: expected - 1,
            },
            events,
            gaps,
            latest_sequence: latest,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Barrier},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "vectora-conversation-{label}-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn draft(conversation: &str, event_id: &str, text: &str) -> ConversationEventDraft {
        ConversationEventDraft {
            contract_version: CONVERSATION_EVENT_CONTRACT_VERSION,
            event_id: event_id.into(),
            conversation_id: conversation.into(),
            turn_id: Some("turn-1".into()),
            run_id: Some("run-1".into()),
            step_id: None,
            timestamp_ms: 1_700_000_000_000,
            source: "test".into(),
            kind: ConversationEventKind::MessageDelta { text: text.into() },
        }
    }

    #[test]
    fn store_assigns_contiguous_sequences_and_retries_are_idempotent() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let first = ledger
            .append_conversation_event("scope", draft("c1", "e1", "a"))
            .unwrap();
        let second = ledger
            .append_conversation_event("scope", draft("c1", "e2", "b"))
            .unwrap();
        assert_eq!((first.sequence, second.sequence), (1, 2));
        let retried = ledger
            .append_conversation_event("scope", draft("c1", "e1", "a"))
            .unwrap();
        assert_eq!(retried, first);
        assert!(matches!(
            ledger.append_conversation_event("scope", draft("c1", "e1", "changed")),
            Err(ConversationStoreError::Ledger(
                LedgerError::IdempotencyConflict
            ))
        ));
        assert!(matches!(
            ledger.append_conversation_event("scope", draft("c2", "e1", "a")),
            Err(ConversationStoreError::EventIdReused { conversation_id }) if conversation_id == "c1"
        ));
        let other_conversation = ledger
            .append_conversation_event("scope", draft("c2", "e3", "c"))
            .unwrap();
        assert_eq!(other_conversation.sequence, 1);
        let other_scope = ledger
            .append_conversation_event("other", draft("c1", "e1", "a"))
            .unwrap();
        assert_eq!(other_scope.sequence, 1);
    }

    #[test]
    fn invalid_drafts_are_rejected_before_storage() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        let mut missing_run = draft("c1", "e1", "a");
        missing_run.run_id = None;
        assert!(matches!(
            ledger.append_conversation_event("scope", missing_run),
            Err(ConversationStoreError::Event(
                ConversationEventError::MissingCorrelation("run_id")
            ))
        ));
        let mut version = draft("c1", "e1", "a");
        version.contract_version = 2;
        assert!(matches!(
            ledger.append_conversation_event("scope", version),
            Err(ConversationStoreError::Event(
                ConversationEventError::UnsupportedContractVersion { received: 2 }
            ))
        ));
        assert!(
            ledger
                .append_conversation_event(" ", draft("c1", "e1", "a"))
                .is_err()
        );
        let oversized = draft("c1", "e1", &"x".repeat(1024 * 1024 + 1));
        assert!(matches!(
            ledger.append_conversation_event("scope", oversized),
            Err(ConversationStoreError::Ledger(LedgerError::InvalidInput))
        ));
        let page = ledger
            .conversation_history("scope", "c1", None, 10)
            .unwrap();
        assert!(page.events.is_empty());
        assert_eq!(page.latest_sequence, 0);
    }

    #[test]
    fn history_pages_follow_the_cursor_without_changing_state() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        for n in 1..=5 {
            ledger
                .append_conversation_event("scope", draft("c1", &format!("e{n}"), "t"))
                .unwrap();
        }
        let first = ledger.conversation_history("scope", "c1", None, 2).unwrap();
        assert_eq!(
            first.events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(first.latest_sequence, 5);
        let second = ledger
            .conversation_history("scope", "c1", Some(&first.next_cursor), 2)
            .unwrap();
        assert_eq!(
            second.events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [3, 4]
        );
        let again = ledger
            .conversation_history("scope", "c1", Some(&first.next_cursor), 2)
            .unwrap();
        assert_eq!(again, second, "reading a page must not change state");
        let last = ledger
            .conversation_history("scope", "c1", Some(&second.next_cursor), 10)
            .unwrap();
        assert_eq!(
            last.events
                .iter()
                .map(|e| e.event_id.as_str())
                .collect::<Vec<_>>(),
            ["e5"]
        );
        let empty = ledger
            .conversation_history("scope", "c1", Some(&last.next_cursor), 10)
            .unwrap();
        assert!(empty.events.is_empty());
        assert_eq!(empty.next_cursor, last.next_cursor);
    }

    #[test]
    fn cursors_are_bound_to_scope_conversation_version_and_history() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        ledger
            .append_conversation_event("scope", draft("c1", "e1", "t"))
            .unwrap();
        let cursor = ledger
            .conversation_history("scope", "c1", None, 10)
            .unwrap()
            .next_cursor;
        assert!(matches!(
            ledger.conversation_history("other", "c1", Some(&cursor), 10),
            Err(ConversationStoreError::CursorMismatch)
        ));
        assert!(matches!(
            ledger.conversation_history("scope", "c2", Some(&cursor), 10),
            Err(ConversationStoreError::CursorMismatch)
        ));
        let mut future = cursor.clone();
        future.contract_version = 2;
        assert!(matches!(
            ledger.conversation_history("scope", "c1", Some(&future), 10),
            Err(ConversationStoreError::UnsupportedContractVersion(2))
        ));
        let mut ahead = cursor;
        ahead.after_sequence = 7;
        assert!(matches!(
            ledger.conversation_history("scope", "c1", Some(&ahead), 10),
            Err(ConversationStoreError::CursorAhead)
        ));
        for limit in [0, MAX_HISTORY_PAGE + 1] {
            assert!(
                ledger
                    .conversation_history("scope", "c1", None, limit)
                    .is_err()
            );
        }
    }

    #[test]
    fn missing_sequences_are_reported_as_gaps() {
        let path = temp_path("gap");
        let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
        for n in 1..=3 {
            ledger
                .append_conversation_event("scope", draft("c1", &format!("e{n}"), "t"))
                .unwrap();
        }
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "DELETE FROM ve_ledger_events WHERE aggregate_id=?1 AND sequence=2",
                [aggregate("c1")],
            )
            .unwrap();
        assert!(matches!(
            ledger.conversation_history("scope", "c1", None, 10),
            Err(ConversationStoreError::Gap {
                expected: 2,
                found: 3
            })
        ));
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn tolerant_reads_report_gaps_and_keep_the_cursor_on_the_last_event() {
        let path = temp_path("tolerant");
        let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
        for n in 1..=5 {
            ledger
                .append_conversation_event("scope", draft("c1", &format!("e{n}"), "t"))
                .unwrap();
        }
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "DELETE FROM ve_ledger_events WHERE aggregate_id=?1 AND sequence IN (1,2,4)",
                [aggregate("c1")],
            )
            .unwrap();
        let page = ledger
            .conversation_history_with_mode("scope", "c1", None, 10, ConversationReadMode::Tolerant)
            .unwrap();
        assert_eq!(
            page.events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [3, 5]
        );
        assert_eq!(
            page.gaps,
            [
                ConversationGap {
                    from_sequence: 1,
                    to_sequence: 2
                },
                ConversationGap {
                    from_sequence: 4,
                    to_sequence: 4
                }
            ]
        );
        assert_eq!(page.next_cursor.after_sequence, 5);
        assert_eq!(page.latest_sequence, 5);
        let strict = ledger.conversation_history("scope", "c1", None, 10);
        assert!(matches!(
            strict,
            Err(ConversationStoreError::Gap {
                expected: 1,
                found: 3
            })
        ));
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reconnecting_with_the_last_event_id_resumes_without_duplicates() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        for n in 1..=4 {
            ledger
                .append_conversation_event("scope", draft("c1", &format!("e{n}"), "t"))
                .unwrap();
        }
        ledger
            .append_conversation_event("scope", draft("c2", "other", "t"))
            .unwrap();
        let cursor = ledger
            .conversation_cursor_after_event("scope", "c1", "e2")
            .unwrap();
        assert_eq!(cursor.after_sequence, 2);
        let page = ledger
            .conversation_history("scope", "c1", Some(&cursor), 10)
            .unwrap();
        assert_eq!(
            page.events
                .iter()
                .map(|e| e.event_id.as_str())
                .collect::<Vec<_>>(),
            ["e3", "e4"]
        );
        assert!(matches!(
            ledger.conversation_cursor_after_event("scope", "c1", "other"),
            Err(ConversationStoreError::EventNotFound)
        ));
        assert!(matches!(
            ledger.conversation_cursor_after_event("scope", "c1", "missing"),
            Err(ConversationStoreError::EventNotFound)
        ));
        assert!(matches!(
            ledger.conversation_cursor_after_event("other-scope", "c1", "e2"),
            Err(ConversationStoreError::EventNotFound)
        ));
    }

    #[test]
    fn sequences_survive_reopen_and_appends_are_queued_for_projection() {
        let path = temp_path("restart");
        {
            let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
            ledger
                .append_conversation_event("scope", draft("c1", "e1", "t"))
                .unwrap();
            assert!(
                ledger.pending_outbox(10).unwrap().is_empty(),
                "nothing is queued before projection is enabled"
            );
            ledger.enable_projection().unwrap();
            assert_eq!(
                ledger.pending_outbox(10).unwrap().len(),
                1,
                "enabling projection backfills existing events"
            );
            ledger
                .append_conversation_event("scope", draft("c1", "e1b", "t"))
                .unwrap();
            assert_eq!(ledger.pending_outbox(10).unwrap().len(), 2);
            ledger
                .append_conversation_event("scope", draft("c1", "e1b", "t"))
                .unwrap();
            assert_eq!(ledger.pending_outbox(10).unwrap().len(), 2);
        }
        let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
        let next = ledger
            .append_conversation_event("scope", draft("c1", "e2", "t"))
            .unwrap();
        assert_eq!(next.sequence, 3);
        assert_eq!(
            ledger
                .conversation_history("scope", "c1", None, 10)
                .unwrap()
                .events
                .len(),
            3
        );
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_writers_get_unique_contiguous_sequences() {
        let path = temp_path("concurrent");
        drop(SqliteLedger::open_persistent(&path).unwrap());
        let writers = 8;
        let barrier = Arc::new(Barrier::new(writers));
        let handles: Vec<_> = (0..writers)
            .map(|n| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut ledger = SqliteLedger::open_persistent(path).unwrap();
                    barrier.wait();
                    ledger
                        .append_conversation_event("scope", draft("c1", &format!("e{n}"), "t"))
                        .unwrap()
                        .sequence
                })
            })
            .collect();
        let mut sequences: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        sequences.sort_unstable();
        assert_eq!(sequences, (1..=writers as u64).collect::<Vec<_>>());
        let ledger = SqliteLedger::open_persistent(&path).unwrap();
        assert_eq!(
            ledger
                .conversation_history("scope", "c1", None, 100)
                .unwrap()
                .latest_sequence,
            writers as u64
        );
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

    fn terminal(conversation: &str, event_id: &str, run_id: &str) -> ConversationEventDraft {
        let mut draft = draft(conversation, event_id, "t");
        draft.run_id = Some(run_id.into());
        draft.kind = ConversationEventKind::Terminal {
            status: crate::harness::ConversationTerminalStatus::Completed,
        };
        draft
    }

    #[test]
    fn a_run_accepts_no_event_after_its_terminal_event() {
        let mut ledger = SqliteLedger::open(":memory:").unwrap();
        ledger
            .append_conversation_event("scope", draft("c1", "e1", "a"))
            .unwrap();
        let end = ledger
            .append_conversation_event("scope", terminal("c1", "end", "run-1"))
            .unwrap();
        let retried = ledger
            .append_conversation_event("scope", terminal("c1", "end", "run-1"))
            .unwrap();
        assert_eq!(retried, end, "retrying the terminal event stays idempotent");
        assert!(matches!(
            ledger.append_conversation_event("scope", draft("c1", "late", "b")),
            Err(ConversationStoreError::RunAlreadyTerminal { run_id }) if run_id == "run-1"
        ));
        assert!(matches!(
            ledger.append_conversation_event("scope", terminal("c1", "end-2", "run-1")),
            Err(ConversationStoreError::RunAlreadyTerminal { .. })
        ));
        let mut other_run = draft("c1", "other", "c");
        other_run.run_id = Some("run-2".into());
        ledger
            .append_conversation_event("scope", other_run)
            .unwrap();
    }
}
