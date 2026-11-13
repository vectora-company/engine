//! PostgreSQL adapter for the relational ledger contract.

use serde_json::Value;
use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;
use thiserror::Error;

use super::sqlite::{
    Admission, CHECKPOINT_VERSION, Checkpoint, EventDraft, LedgerEvent, ProjectionCounts,
    RunReservation,
};

#[derive(Debug, Error)]
pub enum PostgresError {
    #[error("invalid PostgreSQL configuration")]
    InvalidConfig,
    #[error("unsupported PostgreSQL ledger schema version")]
    SchemaVersion,
    #[error("idempotency key was reused with different content")]
    IdempotencyConflict,
    #[error("ledger aggregate version conflict")]
    VersionConflict,
    #[error("PostgreSQL storage operation failed")]
    Storage,
}

impl From<sqlx::Error> for PostgresError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}

async fn save_checkpoint_tx(
    tx: &mut Transaction<'_, Postgres>,
    checkpoint: &Checkpoint,
) -> Result<(), PostgresError> {
    if checkpoint.contract_version != CHECKPOINT_VERSION
        || checkpoint.scope.trim().is_empty()
        || checkpoint.aggregate.trim().is_empty()
        || checkpoint.checkpoint_id.trim().is_empty()
        || checkpoint.sequence == 0
        || checkpoint.sequence > i64::MAX as u64
    {
        return Err(PostgresError::InvalidConfig);
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ve_ledger_events WHERE scope=$1 AND aggregate_id=$2 AND sequence=$3)")
        .bind(&checkpoint.scope)
        .bind(&checkpoint.aggregate)
        .bind(checkpoint.sequence as i64)
        .fetch_one(&mut **tx)
        .await?;
    if !exists {
        return Err(PostgresError::VersionConflict);
    }
    let prior = sqlx::query("SELECT checkpoint_id,sequence,contract_version,state FROM ve_ledger_checkpoints WHERE scope=$1 AND aggregate_id=$2 AND (checkpoint_id=$3 OR sequence=$4) FOR UPDATE")
        .bind(&checkpoint.scope)
        .bind(&checkpoint.aggregate)
        .bind(&checkpoint.checkpoint_id)
        .bind(checkpoint.sequence as i64)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(row) = prior {
        if row.try_get::<String, _>("checkpoint_id")? != checkpoint.checkpoint_id
            || row.try_get::<i64, _>("sequence")? as u64 != checkpoint.sequence
            || row.try_get::<i32, _>("contract_version")? != i32::from(checkpoint.contract_version)
            || row.try_get::<Value, _>("state")? != checkpoint.state
        {
            return Err(PostgresError::IdempotencyConflict);
        }
    } else {
        sqlx::query("INSERT INTO ve_ledger_checkpoints(scope,aggregate_id,checkpoint_id,sequence,contract_version,state) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&checkpoint.scope)
            .bind(&checkpoint.aggregate)
            .bind(&checkpoint.checkpoint_id)
            .bind(checkpoint.sequence as i64)
            .bind(i32::from(checkpoint.contract_version))
            .bind(&checkpoint.state)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub struct PostgresLedger {
    pool: PgPool,
}

impl PostgresLedger {
    /// Connect with bounded pool size. The URL is never included in errors or logs.
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, PostgresError> {
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://")
            || max_connections == 0
        {
            return Err(PostgresError::InvalidConfig);
        }
        let options =
            PgConnectOptions::from_str(database_url).map_err(|_| PostgresError::InvalidConfig)?;
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect_lazy_with(options);
        let ledger = Self { pool };
        ledger.ping().await?;
        ledger.migrate().await?;
        Ok(ledger)
    }

    async fn migrate(&self) -> Result<(), PostgresError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(0x564543544f5241_i64)
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS ve_ledger_meta (id SMALLINT PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL);
            INSERT INTO ve_ledger_meta(id,version) VALUES(1,1) ON CONFLICT(id) DO NOTHING;
            CREATE TABLE IF NOT EXISTS ve_ledger_versions (scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, version BIGINT NOT NULL, PRIMARY KEY(scope,aggregate_id));
            CREATE TABLE IF NOT EXISTS ve_ledger_events (scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, sequence BIGINT NOT NULL, event_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, event_type TEXT NOT NULL, payload JSONB NOT NULL, PRIMARY KEY(scope,aggregate_id,sequence), UNIQUE(scope,aggregate_id,idempotency_key), UNIQUE(scope,event_id));
            CREATE TABLE IF NOT EXISTS ve_run_reservations (scope TEXT NOT NULL, thread_id TEXT NOT NULL, turn_id TEXT NOT NULL, run_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, request JSONB NOT NULL, PRIMARY KEY(scope,thread_id,turn_id), UNIQUE(scope,idempotency_key), UNIQUE(scope,run_id));
            CREATE TABLE IF NOT EXISTS ve_ledger_checkpoints (scope TEXT NOT NULL, aggregate_id TEXT NOT NULL, checkpoint_id TEXT NOT NULL, sequence BIGINT NOT NULL, contract_version INTEGER NOT NULL, state JSONB NOT NULL, PRIMARY KEY(scope,aggregate_id,checkpoint_id), UNIQUE(scope,aggregate_id,sequence));
            CREATE TABLE IF NOT EXISTS ve_run_admissions (scope TEXT NOT NULL, run_id TEXT NOT NULL, event_id TEXT NOT NULL, PRIMARY KEY(scope,run_id), UNIQUE(scope,event_id));")
            .execute(&mut *tx)
            .await?;
        let version: i32 = sqlx::query_scalar("SELECT version FROM ve_ledger_meta WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
        if version != 1 {
            return Err(PostgresError::SchemaVersion);
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn append(&self, draft: EventDraft) -> Result<LedgerEvent, PostgresError> {
        let mut tx = self.pool.begin().await?;
        let event = append_tx(&mut tx, draft).await?;
        tx.commit().await?;
        Ok(event)
    }

    /// Projects a committed SQLite event and its checkpoint atomically.
    pub async fn project(
        &self,
        event: LedgerEvent,
        checkpoint: Option<Checkpoint>,
    ) -> Result<(), PostgresError> {
        self.project_record(event, checkpoint, None).await
    }

    /// Projects a reservation, its admission event and an optional checkpoint atomically.
    pub async fn project_admission(
        &self,
        event: LedgerEvent,
        checkpoint: Option<Checkpoint>,
        reservation: RunReservation,
    ) -> Result<(), PostgresError> {
        if reservation.scope != event.scope || reservation.thread_id != event.aggregate {
            return Err(PostgresError::InvalidConfig);
        }
        self.project_record(event, checkpoint, Some(reservation))
            .await
    }

    async fn project_record(
        &self,
        event: LedgerEvent,
        checkpoint: Option<Checkpoint>,
        reservation: Option<RunReservation>,
    ) -> Result<(), PostgresError> {
        if checkpoint.as_ref().is_some_and(|checkpoint| {
            checkpoint.scope != event.scope
                || checkpoint.aggregate != event.aggregate
                || checkpoint.sequence != event.sequence
        }) {
            return Err(PostgresError::InvalidConfig);
        }
        let mut tx = self.pool.begin().await?;
        let draft = EventDraft {
            scope: event.scope.clone(),
            aggregate: event.aggregate.clone(),
            event_id: event.event_id.clone(),
            idempotency_key: event.idempotency_key.clone(),
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            expected_version: event.sequence.saturating_sub(1),
        };
        if let Some(reservation) = reservation {
            let reservation = reserve_run_tx(&mut tx, reservation).await?;
            let prior = sqlx::query("SELECT run_id,event_id FROM ve_run_admissions WHERE scope=$1 AND (run_id=$2 OR event_id=$3) FOR UPDATE")
                .bind(&reservation.scope).bind(&reservation.run_id).bind(&event.event_id).fetch_optional(&mut *tx).await?;
            if let Some(row) = &prior
                && (row.try_get::<String, _>("run_id")? != reservation.run_id
                    || row.try_get::<String, _>("event_id")? != event.event_id)
            {
                return Err(PostgresError::IdempotencyConflict);
            }
            if prior.is_none() {
                sqlx::query(
                    "INSERT INTO ve_run_admissions(scope,run_id,event_id) VALUES($1,$2,$3)",
                )
                .bind(&reservation.scope)
                .bind(&reservation.run_id)
                .bind(&event.event_id)
                .execute(&mut *tx)
                .await?;
            }
        }
        if append_tx(&mut tx, draft).await? != event {
            return Err(PostgresError::IdempotencyConflict);
        }
        if let Some(checkpoint) = checkpoint {
            save_checkpoint_tx(&mut tx, &checkpoint).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn read(
        &self,
        scope: &str,
        aggregate: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<LedgerEvent>, PostgresError> {
        if scope.trim().is_empty()
            || aggregate.trim().is_empty()
            || after > i64::MAX as u64
            || !(1..=1000).contains(&limit)
        {
            return Err(PostgresError::InvalidConfig);
        }
        let rows = sqlx::query("SELECT sequence,event_id,idempotency_key,event_type,payload FROM ve_ledger_events WHERE scope=$1 AND aggregate_id=$2 AND sequence>$3 ORDER BY sequence LIMIT $4")
            .bind(scope).bind(aggregate).bind(after as i64).bind(i64::from(limit)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(LedgerEvent {
                    scope: scope.into(),
                    aggregate: aggregate.into(),
                    sequence: row.try_get::<i64, _>("sequence")? as u64,
                    event_id: row.try_get("event_id")?,
                    idempotency_key: row.try_get("idempotency_key")?,
                    event_type: row.try_get("event_type")?,
                    payload: row.try_get("payload")?,
                })
            })
            .collect()
    }

    /// Counts rows in the Engine-owned projection tables for recovery verification.
    pub async fn projection_counts(
        &self,
        scopes: &[String],
    ) -> Result<ProjectionCounts, PostgresError> {
        if scopes.is_empty() {
            return Ok(ProjectionCounts::default());
        }
        let row = sqlx::query(
            "SELECT (SELECT COUNT(*) FROM ve_ledger_versions WHERE scope=ANY($1)) AS aggregates,
                    (SELECT COUNT(*) FROM ve_ledger_events WHERE scope=ANY($1)) AS events,
                    (SELECT COUNT(*) FROM ve_ledger_checkpoints WHERE scope=ANY($1)) AS checkpoints,
                    (SELECT COUNT(*) FROM ve_run_reservations WHERE scope=ANY($1)) AS reservations,
                    (SELECT COUNT(*) FROM ve_run_admissions WHERE scope=ANY($1)) AS admissions",
        )
        .bind(scopes)
        .fetch_one(&self.pool)
        .await?;
        Ok(ProjectionCounts {
            aggregates: row.try_get("aggregates")?,
            events: row.try_get("events")?,
            checkpoints: row.try_get("checkpoints")?,
            reservations: row.try_get("reservations")?,
            admissions: row.try_get("admissions")?,
        })
    }

    pub async fn read_run_reservation(
        &self,
        scope: &str,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<Option<RunReservation>, PostgresError> {
        if [scope, thread_id, turn_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(PostgresError::InvalidConfig);
        }
        let row = sqlx::query("SELECT run_id,idempotency_key,request FROM ve_run_reservations WHERE scope=$1 AND thread_id=$2 AND turn_id=$3")
            .bind(scope).bind(thread_id).bind(turn_id).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(RunReservation {
                scope: scope.to_owned(),
                thread_id: thread_id.to_owned(),
                turn_id: turn_id.to_owned(),
                run_id: row.try_get("run_id")?,
                idempotency_key: row.try_get("idempotency_key")?,
                request: row.try_get("request")?,
            })
        })
        .transpose()
    }

    pub async fn read_run_admission(
        &self,
        scope: &str,
        run_id: &str,
    ) -> Result<Option<Admission>, PostgresError> {
        if scope.trim().is_empty() || run_id.trim().is_empty() {
            return Err(PostgresError::InvalidConfig);
        }
        let row = sqlx::query("SELECT r.thread_id,r.turn_id,r.run_id,r.idempotency_key,r.request,e.aggregate_id,e.sequence,e.event_id,e.idempotency_key AS event_idempotency_key,e.event_type,e.payload FROM ve_run_admissions a JOIN ve_run_reservations r ON r.scope=a.scope AND r.run_id=a.run_id JOIN ve_ledger_events e ON e.scope=a.scope AND e.event_id=a.event_id WHERE a.scope=$1 AND a.run_id=$2")
            .bind(scope).bind(run_id).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(Admission {
                reservation: RunReservation {
                    scope: scope.to_owned(),
                    thread_id: row.try_get("thread_id")?,
                    turn_id: row.try_get("turn_id")?,
                    run_id: row.try_get("run_id")?,
                    idempotency_key: row.try_get("idempotency_key")?,
                    request: row.try_get("request")?,
                },
                event: LedgerEvent {
                    scope: scope.to_owned(),
                    aggregate: row.try_get("aggregate_id")?,
                    sequence: u64::try_from(row.try_get::<i64, _>("sequence")?)
                        .map_err(|_| PostgresError::Storage)?,
                    event_id: row.try_get("event_id")?,
                    idempotency_key: row.try_get("event_idempotency_key")?,
                    event_type: row.try_get("event_type")?,
                    payload: row.try_get("payload")?,
                },
            })
        })
        .transpose()
    }

    pub async fn reserve_run(
        &self,
        input: RunReservation,
    ) -> Result<RunReservation, PostgresError> {
        let mut tx = self.pool.begin().await?;
        reserve_run_tx(&mut tx, input.clone()).await?;
        tx.commit().await?;
        Ok(input)
    }

    /// Applies one SQLite-authoritative reservation to PostgreSQL idempotently.
    pub async fn project_reservation(
        &self,
        reservation: RunReservation,
    ) -> Result<(), PostgresError> {
        self.reserve_run(reservation).await.map(|_| ())
    }

    pub async fn admit_run(
        &self,
        reservation: RunReservation,
        event: EventDraft,
    ) -> Result<Admission, PostgresError> {
        if reservation.scope != event.scope || reservation.thread_id != event.aggregate {
            return Err(PostgresError::InvalidConfig);
        }
        let mut tx = self.pool.begin().await?;
        let reservation = reserve_run_tx(&mut tx, reservation).await?;
        let prior = sqlx::query("SELECT run_id,event_id FROM ve_run_admissions WHERE scope=$1 AND (run_id=$2 OR event_id=$3) FOR UPDATE")
            .bind(&reservation.scope).bind(&reservation.run_id).bind(&event.event_id).fetch_optional(&mut *tx).await?;
        let is_new_admission = prior.is_none();
        if let Some(row) = prior
            && (row.try_get::<String, _>("run_id")? != reservation.run_id
                || row.try_get::<String, _>("event_id")? != event.event_id)
        {
            return Err(PostgresError::IdempotencyConflict);
        }
        let ledger_event = append_tx(&mut tx, event.clone()).await?;
        if is_new_admission {
            sqlx::query("INSERT INTO ve_run_admissions(scope,run_id,event_id) VALUES($1,$2,$3)")
                .bind(&reservation.scope)
                .bind(&reservation.run_id)
                .bind(&event.event_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Admission {
            reservation,
            event: ledger_event,
        })
    }

    pub async fn save_checkpoint(
        &self,
        checkpoint: Checkpoint,
    ) -> Result<Checkpoint, PostgresError> {
        let mut tx = self.pool.begin().await?;
        save_checkpoint_tx(&mut tx, &checkpoint).await?;
        tx.commit().await?;
        Ok(checkpoint)
    }

    pub async fn latest_checkpoint(
        &self,
        scope: &str,
        aggregate: &str,
    ) -> Result<Option<Checkpoint>, PostgresError> {
        if scope.trim().is_empty() || aggregate.trim().is_empty() {
            return Err(PostgresError::InvalidConfig);
        }
        let row = sqlx::query("SELECT checkpoint_id,sequence,contract_version,state FROM ve_ledger_checkpoints WHERE scope=$1 AND aggregate_id=$2 ORDER BY sequence DESC LIMIT 1")
            .bind(scope).bind(aggregate).fetch_optional(&self.pool).await?;
        row.map(|row| {
            let version: i32 = row.try_get("contract_version")?;
            if version != i32::from(CHECKPOINT_VERSION) {
                return Err(PostgresError::InvalidConfig);
            }
            Ok(Checkpoint {
                contract_version: CHECKPOINT_VERSION,
                scope: scope.into(),
                aggregate: aggregate.into(),
                checkpoint_id: row.try_get("checkpoint_id")?,
                sequence: u64::try_from(row.try_get::<i64, _>("sequence")?)
                    .map_err(|_| PostgresError::Storage)?,
                state: row.try_get("state")?,
            })
        })
        .transpose()
    }

    pub async fn ping(&self) -> Result<(), PostgresError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

fn event_from(draft: EventDraft, sequence: u64) -> LedgerEvent {
    LedgerEvent {
        scope: draft.scope,
        aggregate: draft.aggregate,
        event_id: draft.event_id,
        idempotency_key: draft.idempotency_key,
        event_type: draft.event_type,
        payload: draft.payload,
        sequence,
    }
}

async fn reserve_run_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: RunReservation,
) -> Result<RunReservation, PostgresError> {
    if [
        &input.scope,
        &input.thread_id,
        &input.turn_id,
        &input.run_id,
        &input.idempotency_key,
    ]
    .iter()
    .any(|v| v.trim().is_empty())
    {
        return Err(PostgresError::InvalidConfig);
    }
    let serialized =
        serde_json::to_string(&input.request).map_err(|_| PostgresError::InvalidConfig)?;
    if serialized.len() > 1024 * 1024 {
        return Err(PostgresError::InvalidConfig);
    }
    sqlx::query("INSERT INTO ve_run_reservations(scope,thread_id,turn_id,run_id,idempotency_key,request) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING")
        .bind(&input.scope).bind(&input.thread_id).bind(&input.turn_id).bind(&input.run_id).bind(&input.idempotency_key).bind(&input.request).execute(&mut **tx).await?;
    let row = sqlx::query("SELECT thread_id,turn_id,run_id,idempotency_key,request FROM ve_run_reservations WHERE scope=$1 AND (idempotency_key=$2 OR (thread_id=$3 AND turn_id=$4) OR run_id=$5) FOR UPDATE")
        .bind(&input.scope).bind(&input.idempotency_key).bind(&input.thread_id).bind(&input.turn_id).bind(&input.run_id).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Err(PostgresError::Storage);
    };
    if row.try_get::<String, _>("thread_id")? != input.thread_id
        || row.try_get::<String, _>("turn_id")? != input.turn_id
        || row.try_get::<String, _>("run_id")? != input.run_id
        || row.try_get::<String, _>("idempotency_key")? != input.idempotency_key
        || row.try_get::<Value, _>("request")? != input.request
    {
        return Err(PostgresError::IdempotencyConflict);
    }
    Ok(input)
}

async fn append_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft: EventDraft,
) -> Result<LedgerEvent, PostgresError> {
    if [
        &draft.scope,
        &draft.aggregate,
        &draft.event_id,
        &draft.idempotency_key,
        &draft.event_type,
    ]
    .iter()
    .any(|v| v.trim().is_empty())
        || draft.expected_version >= i64::MAX as u64
    {
        return Err(PostgresError::InvalidConfig);
    }
    let payload =
        serde_json::to_string(&draft.payload).map_err(|_| PostgresError::InvalidConfig)?;
    if payload.len() > 1024 * 1024 {
        return Err(PostgresError::InvalidConfig);
    }
    sqlx::query("INSERT INTO ve_ledger_versions(scope,aggregate_id,version) VALUES($1,$2,0) ON CONFLICT DO NOTHING")
        .bind(&draft.scope).bind(&draft.aggregate).execute(&mut **tx).await?;
    let version: i64 = sqlx::query_scalar(
        "SELECT version FROM ve_ledger_versions WHERE scope=$1 AND aggregate_id=$2 FOR UPDATE",
    )
    .bind(&draft.scope)
    .bind(&draft.aggregate)
    .fetch_one(&mut **tx)
    .await?;
    if let Some(row) = sqlx::query(
        "SELECT aggregate_id,idempotency_key FROM ve_ledger_events WHERE scope=$1 AND event_id=$2",
    )
    .bind(&draft.scope)
    .bind(&draft.event_id)
    .fetch_optional(&mut **tx)
    .await?
        && (row.try_get::<String, _>("aggregate_id")? != draft.aggregate
            || row.try_get::<String, _>("idempotency_key")? != draft.idempotency_key)
    {
        return Err(PostgresError::IdempotencyConflict);
    }
    if let Some(row) = sqlx::query("SELECT sequence,event_id,event_type,payload FROM ve_ledger_events WHERE scope=$1 AND aggregate_id=$2 AND idempotency_key=$3").bind(&draft.scope).bind(&draft.aggregate).bind(&draft.idempotency_key).fetch_optional(&mut **tx).await? {
        if row.try_get::<String,_>("event_id")? != draft.event_id || row.try_get::<String,_>("event_type")? != draft.event_type || row.try_get::<Value,_>("payload")? != draft.payload { return Err(PostgresError::IdempotencyConflict); }
        let sequence: i64 = row.try_get("sequence")?;
        return Ok(event_from(draft, sequence as u64));
    }
    if version as u64 != draft.expected_version {
        return Err(PostgresError::VersionConflict);
    }
    let sequence = version + 1;
    let insert = sqlx::query("INSERT INTO ve_ledger_events(scope,aggregate_id,sequence,event_id,idempotency_key,event_type,payload) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(&draft.scope).bind(&draft.aggregate).bind(sequence).bind(&draft.event_id).bind(&draft.idempotency_key).bind(&draft.event_type).bind(&draft.payload).execute(&mut **tx).await;
    match insert {
        Ok(_) => {}
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(PostgresError::IdempotencyConflict);
        }
        Err(error) => return Err(error.into()),
    }
    sqlx::query("UPDATE ve_ledger_versions SET version=$3 WHERE scope=$1 AND aggregate_id=$2")
        .bind(&draft.scope)
        .bind(&draft.aggregate)
        .bind(sequence)
        .execute(&mut **tx)
        .await?;
    Ok(event_from(draft, sequence as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn draft() -> EventDraft {
        EventDraft {
            scope: "integration".into(),
            aggregate: "thread".into(),
            event_id: "event-1".into(),
            idempotency_key: "idem-1".into(),
            event_type: "started".into(),
            payload: json!({"ok":true}),
            expected_version: 0,
        }
    }

    #[tokio::test]
    async fn postgres_ledger_contract_against_real_service() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let ledger = PostgresLedger::connect(&url, 4).await.unwrap();
        ledger.ping().await.unwrap();
        let first = ledger.append(draft()).await.unwrap();
        assert_eq!(first.sequence, 1);
        assert_eq!(ledger.append(draft()).await.unwrap(), first);
        let mut conflict = draft();
        conflict.payload = json!({"ok":false});
        assert!(matches!(
            ledger.append(conflict).await,
            Err(PostgresError::IdempotencyConflict)
        ));
        assert_eq!(
            ledger.read("integration", "thread", 0, 10).await.unwrap(),
            vec![first]
        );
    }

    #[tokio::test]
    async fn postgres_admission_and_checkpoint_contract_against_real_service() {
        let Ok(url) = std::env::var("VECTORA_TEST_POSTGRES_URL") else {
            return;
        };
        let ledger = PostgresLedger::connect(&url, 4).await.unwrap();
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let reservation = RunReservation {
            scope: suffix.clone(),
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            run_id: "run".into(),
            idempotency_key: "request".into(),
            request: json!({"ok":true}),
        };
        assert_eq!(
            ledger.reserve_run(reservation.clone()).await.unwrap(),
            reservation
        );
        let event = EventDraft {
            scope: suffix.clone(),
            aggregate: "thread".into(),
            event_id: "event".into(),
            idempotency_key: "event-key".into(),
            event_type: "accepted".into(),
            payload: json!({"run":"run"}),
            expected_version: 0,
        };
        let admitted = ledger
            .admit_run(reservation.clone(), event.clone())
            .await
            .unwrap();
        assert_eq!(
            ledger.admit_run(reservation, event).await.unwrap(),
            admitted
        );
        assert_eq!(
            ledger
                .read_run_reservation(&suffix, "thread", "turn")
                .await
                .unwrap(),
            Some(admitted.reservation.clone())
        );
        assert_eq!(
            ledger.read_run_admission(&suffix, "run").await.unwrap(),
            Some(admitted.clone())
        );
        let checkpoint = Checkpoint {
            contract_version: CHECKPOINT_VERSION,
            scope: suffix.clone(),
            aggregate: "thread".into(),
            checkpoint_id: "checkpoint".into(),
            sequence: 1,
            state: json!({"status":"running"}),
        };
        assert_eq!(
            ledger.save_checkpoint(checkpoint.clone()).await.unwrap(),
            checkpoint
        );
        assert_eq!(
            ledger.latest_checkpoint(&suffix, "thread").await.unwrap(),
            Some(checkpoint)
        );
    }
}
