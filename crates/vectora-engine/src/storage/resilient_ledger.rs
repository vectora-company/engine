//! SQLite-authoritative ledger with a recoverable PostgreSQL projection.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    postgres::{PostgresError, PostgresLedger},
    sqlite::{EventDraft, LedgerError, LedgerEvent, SqliteLedger},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MirrorState {
    Synchronized,
    Pending,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DurableAppend {
    pub event: LedgerEvent,
    pub mirror: MirrorState,
}

#[derive(Debug, Error)]
pub enum ResilientLedgerError {
    #[error(transparent)]
    Sqlite(#[from] LedgerError),
    #[error("PostgreSQL projection rejected a committed SQLite ledger event")]
    ProjectionConflict,
}

/// SQLite is the durable local authority. PostgreSQL receives an outbox-backed
/// projection, so a remote outage cannot prevent recording a ledger event.
pub struct ResilientLedger {
    local: SqliteLedger,
    postgres_url: Option<String>,
    postgres: Option<PostgresLedger>,
}

impl ResilientLedger {
    pub fn open(path: impl AsRef<Path>, postgres_url: Option<String>) -> Result<Self, LedgerError> {
        Ok(Self {
            local: SqliteLedger::open_persistent(path)?,
            postgres_url,
            postgres: None,
        })
    }

    pub async fn append(
        &mut self,
        draft: EventDraft,
    ) -> Result<DurableAppend, ResilientLedgerError> {
        let event = self.local.append_with_outbox(draft)?;
        let conflict = match self.flush_outbox(100).await {
            Ok(_) => false,
            Err(ResilientLedgerError::ProjectionConflict) => true,
            Err(error) => return Err(error),
        };
        let mirror = if conflict {
            MirrorState::Conflict
        } else if self.local.pending_outbox(1)?.is_empty() {
            MirrorState::Synchronized
        } else {
            MirrorState::Pending
        };
        Ok(DurableAppend { event, mirror })
    }

    pub async fn flush_outbox(&mut self, limit: u32) -> Result<usize, ResilientLedgerError> {
        let Some(url) = self.postgres_url.as_deref() else {
            return Ok(0);
        };
        if !(1..=1000).contains(&limit) {
            return Err(LedgerError::InvalidInput.into());
        }
        if self.postgres.is_none() {
            if let Ok(pg) = PostgresLedger::connect(url, 4).await {
                self.postgres = Some(pg);
            } else {
                return Ok(0);
            }
        }
        let pending = self.local.pending_outbox(limit)?;
        let mut delivered = 0;
        for event in pending {
            let pg = self.postgres.as_ref().expect("initialized above");
            let draft = EventDraft {
                scope: event.scope.clone(),
                aggregate: event.aggregate.clone(),
                event_id: event.event_id.clone(),
                idempotency_key: event.idempotency_key.clone(),
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                expected_version: event.sequence.saturating_sub(1),
            };
            match pg.append(draft).await {
                Ok(projected) if projected == event => {
                    self.local.mark_outbox_delivered(
                        &event.scope,
                        &event.aggregate,
                        event.sequence,
                    )?;
                    delivered += 1;
                }
                Ok(_)
                | Err(PostgresError::IdempotencyConflict | PostgresError::VersionConflict) => {
                    return Err(ResilientLedgerError::ProjectionConflict);
                }
                Err(_) => {
                    self.postgres = None;
                    break;
                }
            }
        }
        Ok(delivered)
    }

    pub fn read_local(
        &self,
        scope: &str,
        aggregate: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<LedgerEvent>, LedgerError> {
        self.local.read(scope, aggregate, after, limit)
    }

    pub fn pending_count(&self) -> Result<usize, LedgerError> {
        usize::try_from(self.local.pending_outbox_count()?).map_err(|_| LedgerError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "vectora-resilient-ledger-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn sqlite_accepts_events_while_postgres_is_unavailable_and_persists_retry_queue() {
        let path = db_path();
        let mut ledger =
            ResilientLedger::open(&path, Some("postgres://bad:bad@127.0.0.1:1/absent".into()))
                .unwrap();
        let append = ledger
            .append(EventDraft {
                scope: "s".into(),
                aggregate: "a".into(),
                event_id: "e".into(),
                idempotency_key: "k".into(),
                event_type: "accepted".into(),
                payload: json!({"x":1}),
                expected_version: 0,
            })
            .await
            .unwrap();
        assert_eq!(append.mirror, MirrorState::Pending);
        assert_eq!(
            ledger.read_local("s", "a", 0, 10).unwrap(),
            vec![append.event]
        );
        assert_eq!(ledger.pending_count().unwrap(), 1);
        drop(ledger);
        let ledger = ResilientLedger::open(&path, None).unwrap();
        assert_eq!(ledger.pending_count().unwrap(), 1);
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn outbox_projects_to_postgres_after_local_commit_against_real_services() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let path = db_path();
        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("mirror-{suffix}");
        let appended = ledger
            .append(EventDraft {
                scope: scope.clone(),
                aggregate: "thread".into(),
                event_id: format!("event-{suffix}"),
                idempotency_key: format!("idem-{suffix}"),
                event_type: "accepted".into(),
                payload: json!({"source":"sqlite"}),
                expected_version: 0,
            })
            .await
            .unwrap();
        assert_eq!(appended.mirror, MirrorState::Synchronized);
        assert_eq!(ledger.pending_count().unwrap(), 0);
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        assert_eq!(
            pg.read(&scope, "thread", 0, 10).await.unwrap(),
            vec![appended.event]
        );
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn projection_conflict_keeps_local_commit_and_reports_conflict() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("conflict-{suffix}");
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        pg.append(EventDraft {
            scope: scope.clone(),
            aggregate: "thread".into(),
            event_id: "shared-event".into(),
            idempotency_key: "remote-idem".into(),
            event_type: "remote".into(),
            payload: serde_json::json!({"origin":"postgres"}),
            expected_version: 0,
        })
        .await
        .unwrap();

        let path = db_path();
        let mut ledger = ResilientLedger::open(&path, Some(url)).unwrap();
        let appended = ledger
            .append(EventDraft {
                scope: scope.clone(),
                aggregate: "thread".into(),
                event_id: "shared-event".into(),
                idempotency_key: "local-idem".into(),
                event_type: "local".into(),
                payload: serde_json::json!({"origin":"sqlite"}),
                expected_version: 0,
            })
            .await
            .unwrap();
        assert_eq!(appended.mirror, MirrorState::Conflict);
        assert_eq!(ledger.pending_count().unwrap(), 1);
        assert_eq!(
            ledger.read_local(&scope, "thread", 0, 10).unwrap(),
            vec![appended.event]
        );
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    fn podman(action: &str, container: &str) {
        let status = std::process::Command::new("podman")
            .args([action, container])
            .status()
            .expect("podman is required for service restart tests");
        assert!(status.success(), "podman {action} {container} failed");
    }

    #[tokio::test]
    async fn projection_resumes_after_postgres_is_stopped_and_started() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        if std::env::var("VECTORA_TEST_PODMAN_RESTART").is_err() {
            return;
        }
        // `podman stop` deletes a container created with auto-remove, which
        // would destroy the operator's service instead of pausing it.
        let auto_remove = std::process::Command::new("podman")
            .args([
                "inspect",
                "--format",
                "{{.HostConfig.AutoRemove}}",
                "vectora-engine-pg-test",
            ])
            .output()
            .expect("podman is required for service restart tests");
        if String::from_utf8_lossy(&auto_remove.stdout).trim() != "false" {
            return;
        }
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("outage-{suffix}");
        let draft = |n: u64| EventDraft {
            scope: scope.clone(),
            aggregate: "thread".into(),
            event_id: format!("event-{n}-{suffix}"),
            idempotency_key: format!("idem-{n}-{suffix}"),
            event_type: "step".into(),
            payload: json!({"n": n}),
            expected_version: n - 1,
        };
        let path = db_path();
        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(
            ledger.append(draft(1)).await.unwrap().mirror,
            MirrorState::Synchronized
        );

        podman("stop", "vectora-engine-pg-test");
        let during_outage = ledger.append(draft(2)).await.unwrap();
        assert_eq!(during_outage.mirror, MirrorState::Pending);
        assert_eq!(ledger.pending_count().unwrap(), 1);
        assert_eq!(
            ledger.read_local(&scope, "thread", 0, 10).unwrap().len(),
            2,
            "the local commit must not depend on PostgreSQL"
        );

        podman("start", "vectora-engine-pg-test");
        let mut recovered = false;
        for _ in 0..40 {
            if ledger.flush_outbox(100).await.is_ok() && ledger.pending_count().unwrap() == 0 {
                recovered = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        assert!(recovered, "outbox must drain after PostgreSQL returns");
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        let projected = pg.read(&scope, "thread", 0, 10).await.unwrap();
        assert_eq!(
            projected.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(projected[1], during_outage.event);
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }
}
