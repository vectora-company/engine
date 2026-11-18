//! Durable conversation event log on top of the SQLite ledger.
//!
//! Each conversation is one ledger aggregate. The store assigns the sequence
//! inside an immediate transaction, so concurrent writers on the same file get
//! contiguous, unique positions. The producer's `event_id` is the idempotency
//! key: retrying the same draft returns the stored event and its sequence,
//! while reusing the ID with different content is a conflict. Appends are
//! queued for the PostgreSQL projection exactly like other ledger events.
//!
//! History is read-only: reading a page never changes producer or consumer
//! state. Cursors are bound to the scope, the conversation and the contract
//! version. They are not signed; the host authorizes who may read a scope.

use rusqlite::TransactionBehavior;
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

/// One page of history in sequence order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationHistoryPage {
    pub events: Vec<ConversationEvent>,
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
    #[error("conversation history has a gap: expected sequence {expected}, found {found}")]
    Gap { expected: u64, found: u64 },
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
        Self::queue_projection_in(&tx, &event, None, None)?;
        tx.commit().map_err(LedgerError::from)?;
        let stored = draft.with_sequence(event.sequence);
        stored.validate()?;
        Ok(stored)
    }

    /// Reads up to `limit` events after the cursor (or from the start).
    pub fn conversation_history(
        &self,
        scope: &str,
        conversation_id: &str,
        cursor: Option<&ConversationCursor>,
        limit: u32,
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
        let latest: i64 = self
            .connection
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
        let mut expected = after + 1;
        for stored in self.read(scope, &aggregate, after, limit)? {
            if stored.sequence != expected {
                return Err(ConversationStoreError::Gap {
                    expected,
                    found: stored.sequence,
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
            expected += 1;
        }
        Ok(ConversationHistoryPage {
            next_cursor: ConversationCursor {
                contract_version: CONVERSATION_STORE_CONTRACT_VERSION,
                scope: scope.to_owned(),
                conversation_id: conversation_id.to_owned(),
                after_sequence: expected - 1,
            },
            events,
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
            Err(ConversationStoreError::Ledger(
                LedgerError::IdempotencyConflict
            ))
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
    fn sequences_survive_reopen_and_appends_are_queued_for_projection() {
        let path = temp_path("restart");
        {
            let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
            ledger
                .append_conversation_event("scope", draft("c1", "e1", "t"))
                .unwrap();
            assert_eq!(ledger.pending_outbox(10).unwrap().len(), 1);
        }
        let mut ledger = SqliteLedger::open_persistent(&path).unwrap();
        let next = ledger
            .append_conversation_event("scope", draft("c1", "e2", "t"))
            .unwrap();
        assert_eq!(next.sequence, 2);
        assert_eq!(
            ledger
                .conversation_history("scope", "c1", None, 10)
                .unwrap()
                .events
                .len(),
            2
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
}
