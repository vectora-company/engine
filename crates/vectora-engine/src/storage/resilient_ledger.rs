//! SQLite-authoritative ledger with a recoverable PostgreSQL projection.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    postgres::{PostgresError, PostgresLedger},
    sqlite::{
        EventDraft, LedgerError, LedgerEvent, PendingProjection, ProjectionCounts, SqliteLedger,
    },
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
    #[error("PostgreSQL projection is unavailable during recovery")]
    ProjectionUnavailable,
    #[error("PostgreSQL projection counts do not match the SQLite authority")]
    ProjectionMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectionRecoveryReport {
    pub projected: usize,
    pub expected: ProjectionCounts,
    pub observed: ProjectionCounts,
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
        let mut local = SqliteLedger::open_persistent(path)?;
        if postgres_url.is_some() {
            local.enable_projection()?;
        }
        Ok(Self {
            local,
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
        if !(1..=1000).contains(&limit) {
            return Err(LedgerError::InvalidInput.into());
        }
        let Some(url) = self.postgres_url.as_deref() else {
            return Ok(0);
        };
        if self.postgres.is_none() {
            if let Ok(pg) = PostgresLedger::connect(url, 4).await {
                self.postgres = Some(pg);
            } else {
                return Ok(0);
            }
        }
        let pending = self.local.pending_projections(limit)?;
        let mut delivered = 0;
        for projection in pending {
            let PendingProjection {
                event,
                checkpoint,
                reservation,
            } = projection;
            let pg = self.postgres.as_ref().expect("initialized above");
            let projected = match reservation {
                Some(reservation) => {
                    pg.project_admission(event.clone(), checkpoint, reservation)
                        .await
                }
                None => pg.project(event.clone(), checkpoint).await,
            };
            match projected {
                Ok(()) => {
                    self.local.mark_outbox_delivered(
                        &event.scope,
                        &event.aggregate,
                        event.sequence,
                    )?;
                    delivered += 1;
                }
                Err(PostgresError::IdempotencyConflict | PostgresError::VersionConflict) => {
                    return Err(ResilientLedgerError::ProjectionConflict);
                }
                Err(_) => {
                    self.postgres = None;
                    break;
                }
            }
        }
        if self.postgres.is_some() && delivered < limit as usize {
            let remaining = (limit as usize - delivered) as u32;
            for reservation in self.local.pending_reservations(remaining)? {
                let pg = self.postgres.as_ref().expect("initialized above");
                match pg.project_reservation(reservation.clone()).await {
                    Ok(()) => {
                        self.local.mark_reservation_delivered(
                            &reservation.scope,
                            &reservation.thread_id,
                            &reservation.turn_id,
                        )?;
                        delivered += 1;
                    }
                    Err(PostgresError::IdempotencyConflict | PostgresError::VersionConflict) => {
                        return Err(ResilientLedgerError::ProjectionConflict);
                    }
                    Err(_) => {
                        self.postgres = None;
                        break;
                    }
                }
            }
        }
        Ok(delivered)
    }

    /// Rebuilds the selected scopes in PostgreSQL from SQLite and verifies row counts.
    /// Callers must pause writers during this operation for a consistent comparison.
    pub async fn rebuild_projection(
        &mut self,
    ) -> Result<ProjectionRecoveryReport, ResilientLedgerError> {
        let url = self
            .postgres_url
            .as_deref()
            .ok_or(ResilientLedgerError::ProjectionUnavailable)?;
        if self.postgres.is_none() {
            self.postgres = Some(
                PostgresLedger::connect(url, 4)
                    .await
                    .map_err(|_| ResilientLedgerError::ProjectionUnavailable)?,
            );
        }
        self.local.requeue_projection()?;
        let mut projected = 0;
        loop {
            let pending = self.pending_count()?;
            if pending == 0 {
                break;
            }
            let delivered = self.flush_outbox(1000).await?;
            projected += delivered;
            if delivered == 0 {
                return Err(ResilientLedgerError::ProjectionUnavailable);
            }
        }
        let scopes = self.local.projection_scopes()?;
        let expected = self.local.projection_counts(&scopes)?;
        let observed = self
            .postgres
            .as_ref()
            .expect("connected before rebuild")
            .projection_counts(&scopes)
            .await
            .map_err(|_| ResilientLedgerError::ProjectionUnavailable)?;
        if expected != observed {
            return Err(ResilientLedgerError::ProjectionMismatch);
        }
        Ok(ProjectionRecoveryReport {
            projected,
            expected,
            observed,
        })
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
    async fn rebuild_restores_a_lost_postgres_projection_from_sqlite() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("projection-rebuild-{suffix}");
        let path = db_path();
        let reservation = super::super::sqlite::RunReservation {
            scope: scope.clone(),
            thread_id: format!("thread-{suffix}"),
            turn_id: "turn-1".into(),
            run_id: format!("run-{suffix}"),
            idempotency_key: format!("request-{suffix}"),
            request: json!({"prompt":"recover from sqlite"}),
        };
        let standalone = super::super::sqlite::RunReservation {
            scope: scope.clone(),
            thread_id: format!("standalone-thread-{suffix}"),
            turn_id: "turn-2".into(),
            run_id: format!("standalone-run-{suffix}"),
            idempotency_key: format!("standalone-request-{suffix}"),
            request: json!({"prompt":"reserved only"}),
        };
        let admission = {
            let mut local = SqliteLedger::open_persistent(&path).unwrap();
            local
                .admit_run_checkpointed(
                    reservation.clone(),
                    EventDraft {
                        scope: scope.clone(),
                        aggregate: reservation.thread_id.clone(),
                        event_id: format!("admission-event-{suffix}"),
                        idempotency_key: format!("admission-{suffix}"),
                        event_type: "run_admitted".into(),
                        payload: json!({"run_id":reservation.run_id}),
                        expected_version: 0,
                    },
                    EventDraft {
                        scope: scope.clone(),
                        aggregate: reservation.run_id.clone(),
                        event_id: format!("lifecycle-event-{suffix}"),
                        idempotency_key: format!("lifecycle-{suffix}"),
                        event_type: "run_started".into(),
                        payload: json!({"status":"planned"}),
                        expected_version: 0,
                    },
                    format!("checkpoint-{suffix}"),
                    json!({"status":"planned"}),
                )
                .unwrap()
        };
        {
            let mut local = SqliteLedger::open_persistent(&path).unwrap();
            local.reserve_run(standalone.clone()).unwrap();
        }
        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(ledger.flush_outbox(100).await.unwrap(), 3);
        assert_eq!(ledger.pending_count().unwrap(), 0);

        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        for table in [
            "ve_run_admissions",
            "ve_ledger_checkpoints",
            "ve_run_reservations",
            "ve_ledger_events",
            "ve_ledger_versions",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE scope=$1"))
                .bind(&scope)
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();

        let report = ledger.rebuild_projection().await.unwrap();
        assert_eq!(report.expected.events, 2);
        assert_eq!(report.expected.aggregates, 2);
        assert_eq!(report.expected.checkpoints, 1);
        assert_eq!(report.expected.reservations, 2);
        assert_eq!(report.expected.admissions, 1);
        assert_eq!(report.observed, report.expected);
        assert_eq!(report.projected, 3);
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        let admission_events = pg
            .read(&scope, &reservation.thread_id, 0, 10)
            .await
            .unwrap();
        assert_eq!(admission_events.len(), 1);
        assert_eq!(admission_events[0], admission.admission_event);
        let lifecycle_events = pg.read(&scope, &reservation.run_id, 0, 10).await.unwrap();
        assert_eq!(lifecycle_events.len(), 1);
        assert_eq!(lifecycle_events[0], admission.lifecycle.event);
        assert_eq!(
            pg.read_run_reservation(&scope, &standalone.thread_id, &standalone.turn_id)
                .await
                .unwrap(),
            Some(standalone)
        );
        assert_eq!(
            pg.read_run_admission(&scope, &reservation.run_id)
                .await
                .unwrap(),
            Some(super::super::sqlite::Admission {
                reservation,
                event: admission.admission_event,
            })
        );
        drop(pool);
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn flush_outbox_validates_limit_even_without_postgres() {
        let path = db_path();
        let mut ledger = ResilientLedger::open(&path, None).unwrap();
        assert!(matches!(
            ledger.flush_outbox(0).await,
            Err(ResilientLedgerError::Sqlite(LedgerError::InvalidInput))
        ));
        assert!(matches!(
            ledger.flush_outbox(1001).await,
            Err(ResilientLedgerError::Sqlite(LedgerError::InvalidInput))
        ));
        assert_eq!(ledger.flush_outbox(100).await.unwrap(), 0);
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
    async fn checkpointed_store_state_projects_atomically_to_postgres() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let path = db_path();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("checkpoint-mirror-{suffix}");
        let aggregate = format!("run-{suffix}");
        drop(ResilientLedger::open(&path, Some(url.clone())).unwrap());
        let mut local = SqliteLedger::open_persistent(&path).unwrap();
        let saved = local
            .append_checkpointed_projected(
                EventDraft {
                    scope: scope.clone(),
                    aggregate: aggregate.clone(),
                    event_id: format!("event-{suffix}"),
                    idempotency_key: format!("idem-{suffix}"),
                    event_type: "run_lifecycle_transition".into(),
                    payload: json!({"state":{"status":"running"}}),
                    expected_version: 0,
                },
                format!("checkpoint-{suffix}"),
                json!({"contract_version":1,"status":"running"}),
            )
            .unwrap();
        drop(local);

        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(ledger.pending_count().unwrap(), 1);
        assert_eq!(ledger.flush_outbox(10).await.unwrap(), 1);
        assert_eq!(ledger.pending_count().unwrap(), 0);
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        assert_eq!(
            pg.read(&scope, &aggregate, 0, 10).await.unwrap(),
            vec![saved.event]
        );
        assert_eq!(
            pg.latest_checkpoint(&scope, &aggregate).await.unwrap(),
            Some(saved.checkpoint)
        );
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn reservation_and_admission_backfill_to_queryable_postgres_rows() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let path = db_path();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let scope = format!("admission-projection-{suffix}");
        let reservation = super::super::sqlite::RunReservation {
            scope: scope.clone(),
            thread_id: format!("thread-{suffix}"),
            turn_id: "turn-1".into(),
            run_id: format!("run-{suffix}"),
            idempotency_key: format!("request-{suffix}"),
            request: json!({"prompt":"persist locally first"}),
        };
        let admission = {
            let mut local = SqliteLedger::open_persistent(&path).unwrap();
            local
                .admit_run_checkpointed(
                    reservation.clone(),
                    EventDraft {
                        scope: scope.clone(),
                        aggregate: reservation.thread_id.clone(),
                        event_id: format!("admission-event-{suffix}"),
                        idempotency_key: format!("admission-{suffix}"),
                        event_type: "run_admitted".into(),
                        payload: json!({"run_id":reservation.run_id}),
                        expected_version: 0,
                    },
                    EventDraft {
                        scope: scope.clone(),
                        aggregate: reservation.run_id.clone(),
                        event_id: format!("lifecycle-event-{suffix}"),
                        idempotency_key: format!("lifecycle-{suffix}"),
                        event_type: "run_started".into(),
                        payload: json!({"status":"planned"}),
                        expected_version: 0,
                    },
                    format!("checkpoint-{suffix}"),
                    json!({"status":"planned"}),
                )
                .unwrap()
        };

        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(ledger.pending_count().unwrap(), 2);
        assert_eq!(ledger.flush_outbox(10).await.unwrap(), 2);
        assert_eq!(ledger.pending_count().unwrap(), 0);

        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        assert_eq!(
            pg.read_run_reservation(&scope, &reservation.thread_id, &reservation.turn_id)
                .await
                .unwrap(),
            Some(reservation.clone())
        );
        assert_eq!(
            pg.read_run_admission(&scope, &reservation.run_id)
                .await
                .unwrap(),
            Some(super::super::sqlite::Admission {
                reservation,
                event: admission.admission_event,
            })
        );
        assert_eq!(
            pg.latest_checkpoint(&scope, &admission.lifecycle.event.aggregate)
                .await
                .unwrap(),
            Some(admission.lifecycle.checkpoint)
        );
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn standalone_reservation_backfill_projects_to_postgres() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let path = db_path();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let reservation = super::super::sqlite::RunReservation {
            scope: format!("reservation-projection-{suffix}"),
            thread_id: format!("thread-{suffix}"),
            turn_id: "turn-1".into(),
            run_id: format!("run-{suffix}"),
            idempotency_key: format!("request-{suffix}"),
            request: json!({"prompt":"reserve before PostgreSQL is available"}),
        };
        {
            let mut local = SqliteLedger::open_persistent(&path).unwrap();
            local.reserve_run(reservation.clone()).unwrap();
        }

        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(ledger.pending_count().unwrap(), 1);
        assert_eq!(ledger.flush_outbox(10).await.unwrap(), 1);
        assert_eq!(ledger.pending_count().unwrap(), 0);
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        assert_eq!(
            pg.read_run_reservation(
                &reservation.scope,
                &reservation.thread_id,
                &reservation.turn_id
            )
            .await
            .unwrap(),
            Some(reservation)
        );
        drop(ledger);
        drop(pg);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn admission_without_checkpoint_projects_with_its_reservation() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let path = db_path();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let reservation = super::super::sqlite::RunReservation {
            scope: format!("admission-only-{suffix}"),
            thread_id: format!("thread-{suffix}"),
            turn_id: "turn-1".into(),
            run_id: format!("run-{suffix}"),
            idempotency_key: format!("request-{suffix}"),
            request: json!({"prompt":"no lifecycle checkpoint"}),
        };
        let admission = {
            let mut local = SqliteLedger::open_persistent(&path).unwrap();
            local
                .admit_run(
                    reservation.clone(),
                    EventDraft {
                        scope: reservation.scope.clone(),
                        aggregate: reservation.thread_id.clone(),
                        event_id: format!("admission-event-{suffix}"),
                        idempotency_key: format!("admission-{suffix}"),
                        event_type: "run_admitted".into(),
                        payload: json!({"run_id":reservation.run_id}),
                        expected_version: 0,
                    },
                )
                .unwrap()
        };
        let mut ledger = ResilientLedger::open(&path, Some(url.clone())).unwrap();
        assert_eq!(ledger.pending_count().unwrap(), 1);
        assert_eq!(ledger.flush_outbox(10).await.unwrap(), 1);
        let pg = PostgresLedger::connect(&url, 2).await.unwrap();
        assert_eq!(
            pg.read_run_admission(&reservation.scope, &reservation.run_id)
                .await
                .unwrap(),
            Some(admission)
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
