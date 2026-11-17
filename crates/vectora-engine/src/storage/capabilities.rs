//! Static, declarative capabilities of the selected storage backends.
//!
//! The report describes what each adapter implements; it does not open a
//! connection, probe a service, or prove operational guarantees. Backends in
//! the same pair are meant to share a functional base; a difference reported
//! here is either a gap still to close or a specialisation whose
//! incompatibility reason is recorded.

use serde::{Deserialize, Serialize};

use super::{
    GraphBackend, KvBackend, PubSubBackend, QueueBackend, STORAGE_CONTRACT_VERSION, SqlBackend,
    StorageSelection, VectorBackend,
};

/// Direction in which a vector score improves.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoreDirection {
    /// Smaller values are closer (distance).
    LowerIsCloser,
    /// Larger values are closer (similarity).
    HigherIsCloser,
}

/// Capability facts for one selected backend.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackendCapabilities {
    /// `sql`, `vector`, `kv`, `queue`, `pubsub` or `graph`.
    pub kind: String,
    /// Serialized backend name as used in `StorageSelection`.
    pub backend: String,
    /// Adapters implemented for this backend, by contract name.
    pub implemented: Vec<String>,
    /// Contracts a consumer might expect that this backend does not provide.
    pub not_implemented: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_direction: Option<ScoreDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durable: Option<bool>,
    /// Behaviour that differs from sibling backends.
    pub notes: Vec<String>,
}

/// Report for a whole selection. Service-level guarantees depend on how the
/// operator configures the service and are not part of this report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageCapabilities {
    pub contract_version: u16,
    pub backends: Vec<BackendCapabilities>,
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[allow(clippy::too_many_arguments)]
fn entry(
    kind: &str,
    backend: &str,
    implemented: &[&str],
    not_implemented: &[&str],
    ttl: Option<bool>,
    score_direction: Option<ScoreDirection>,
    durable: Option<bool>,
    notes: &[&str],
) -> BackendCapabilities {
    BackendCapabilities {
        kind: kind.to_owned(),
        backend: backend.to_owned(),
        implemented: strings(implemented),
        not_implemented: strings(not_implemented),
        ttl,
        score_direction,
        durable,
        notes: strings(notes),
    }
}

impl StorageSelection {
    /// Describes the adapters behind the selected backends.
    pub fn capabilities(&self) -> StorageCapabilities {
        let mut backends = Vec::new();
        match self.sql {
            Some(SqlBackend::Sqlite) => backends.push(entry(
                "sql",
                "sqlite",
                &[
                    "ledger",
                    "ledger_backup_online",
                    "ledger_backup_offline",
                    "ledger_backup_verify",
                    "ledger_restore",
                    "checkpoints",
                    "run_lifecycle_store",
                    "model_tool_loop_store",
                    "tool_policy_store",
                ],
                &["generic_sql"],
                None,
                None,
                Some(true),
                &["Local authority for the resilient ledger append."],
            )),
            Some(SqlBackend::Postgresql) => backends.push(entry(
                "sql",
                "postgresql",
                &["ledger_projection", "ledger_projection_rebuild_verify"],
                &[
                    "generic_sql",
                    "engine_managed_postgresql_backup",
                    "run_lifecycle_store",
                    "model_tool_loop_store",
                    "tool_policy_store",
                ],
                None,
                None,
                None,
                &[
                    "Receives the ledger projection; SQLite stays authoritative for appends.",
                    "PostgreSQL backup is operator-managed (for example pg_dump); the Engine rebuilds and verifies its projection from SQLite.",
                ],
            )),
            None => {}
        }
        match self.vector {
            Some(VectorBackend::LanceDb) => backends.push(entry(
                "vector",
                "lance_db",
                &["vector_store"],
                &[
                    "metadata_filter",
                    "configurable_ann_index",
                    "quantization",
                    "lexical_search",
                    "hybrid_search",
                ],
                None,
                Some(ScoreDirection::HigherIsCloser),
                None,
                &[
                    "Cosine similarity is normalized to [-1, 1]; higher is closer.",
                    "Metadata filters, configurable ANN index parameters and quantization are not exposed by the adapter.",
                ],
            )),
            Some(VectorBackend::Qdrant) => backends.push(entry(
                "vector",
                "qdrant",
                &["vector_store"],
                &[
                    "metadata_filter",
                    "configurable_ann_index",
                    "quantization",
                    "lexical_search",
                    "hybrid_search",
                ],
                None,
                Some(ScoreDirection::HigherIsCloser),
                None,
                &[
                    "Cosine similarity is in [-1, 1]; higher is closer.",
                    "Metadata filters, configurable ANN index parameters and quantization are not exposed by the adapter.",
                ],
            )),
            None => {}
        }
        match self.kv {
            Some(KvBackend::Redis) => backends.push(entry(
                "kv",
                "redis",
                &[
                    "kv_store",
                    "kv_list_prefix",
                    "kv_compare_and_set",
                    "kv_increment",
                    "kv_watch",
                ],
                &[],
                Some(true),
                None,
                None,
                &[
                    "TTL is per key, rounded up to whole seconds.",
                    "KV watch requires Redis keyspace notifications (K$gxe or KA); the adapter does not change server configuration.",
                ],
            )),
            Some(KvBackend::NatsJetstream) => backends.push(entry(
                "kv",
                "nats_jetstream",
                &[
                    "kv_store",
                    "kv_list_prefix",
                    "kv_compare_and_set",
                    "kv_increment",
                    "kv_watch",
                ],
                &[],
                Some(true),
                None,
                None,
                &[
                    "Per-key TTL uses JetStream message TTL; NATS Server 2.11+ is required.",
                    "KV watch observes changes recorded in the selected JetStream bucket.",
                ],
            )),
            None => {}
        }
        match self.queue {
            Some(QueueBackend::RedisStreams) => backends.push(entry(
                "queue",
                "redis_streams",
                &[
                    "durable_queue",
                    "idempotent_enqueue_configurable_window",
                    "negative_ack",
                    "scheduled_retry_within_visibility_timeout",
                    "dead_letter_queue",
                    "queue_replay_from_cursor",
                ],
                &[
                    "graceful_consumer_shutdown",
                    "atomic_dead_letter_redrive",
                ],
                None,
                None,
                Some(true),
                &[
                    "Unacknowledged deliveries are reclaimed with XAUTOCLAIM after the visibility timeout.",
                    "NACK keeps the stream entry pending and schedules reclaim up to the configured visibility timeout.",
                    "Idempotent enqueue uses an atomic Redis Lua script and retains the key for the configured window.",
                    "Source stream retention is capped by max_messages (default 100000); oldest entries can be trimmed, including pending deliveries.",
                    "After max_attempts (default 5), unacknowledged jobs move to a separately consumable dead-letter stream.",
                    "Dead-letter transfer and source ACK are atomic in Redis.",
                ],
            )),
            Some(QueueBackend::NatsJetstream) => backends.push(entry(
                "queue",
                "nats_jetstream",
                &[
                    "durable_queue",
                    "idempotent_enqueue_configurable_window",
                    "negative_ack",
                    "scheduled_retry_within_visibility_timeout",
                    "dead_letter_queue",
                    "queue_replay_from_cursor",
                ],
                &[
                    "graceful_consumer_shutdown",
                    "atomic_dead_letter_redrive",
                ],
                None,
                None,
                Some(true),
                &[
                    "Durable consumers redeliver after ack_wait.",
                    "NAK schedules redelivery up to the configured ack_wait.",
                    "Idempotent enqueue uses the NATS message ID header and the configured stream duplicate window.",
                    "Source stream retention is capped by max_messages (default 100000); oldest entries can be removed, including pending deliveries.",
                    "After max_attempts (default 5), unacknowledged jobs move to a separately consumable dead-letter stream.",
                    "Dead-letter publication is at-least-once before the source ACK and may duplicate if the ACK is lost.",
                ],
            )),
            None => {}
        }
        match self.pubsub {
            Some(PubSubBackend::Redis) => backends.push(entry(
                "pubsub",
                "redis",
                &["ephemeral_pubsub"],
                &["durable_delivery", "replay"],
                None,
                None,
                Some(false),
                &["Messages published without a subscriber are dropped."],
            )),
            Some(PubSubBackend::NatsCore) => backends.push(entry(
                "pubsub",
                "nats_core",
                &["ephemeral_pubsub"],
                &["durable_delivery", "replay"],
                None,
                None,
                Some(false),
                &["Messages published without a subscriber are dropped."],
            )),
            None => {}
        }
        match self.graph {
            Some(GraphBackend::Falkordb) => backends.push(entry(
                "graph",
                "falkordb",
                &["property_graph", "parameterized_cypher"],
                &["portable_full_text_search", "portable_vector_search"],
                None,
                None,
                None,
                &["Cypher follows FalkorDB's OpenCypher-based dialect; compatibility with Neo4j is not implied."],
            )),
            None => {}
        }
        StorageCapabilities {
            contract_version: STORAGE_CONTRACT_VERSION,
            backends,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(json: &str) -> StorageSelection {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn reports_only_selected_backends_and_their_differences() {
        let report = selection(
            r#"{"contract_version":1,"vector":"qdrant","kv":"nats_jetstream","queue":"redis_streams"}"#,
        )
        .capabilities();
        let kinds: Vec<_> = report.backends.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, ["vector", "kv", "queue"]);
        assert_eq!(
            report.backends[0].score_direction,
            Some(ScoreDirection::HigherIsCloser)
        );
        assert_eq!(report.backends[1].ttl, Some(true));
        assert!(
            report.backends[2]
                .implemented
                .contains(&"dead_letter_queue".to_owned())
        );
        assert!(
            report.backends[2]
                .implemented
                .contains(&"queue_replay_from_cursor".to_owned())
        );
        assert!(
            report.backends[2]
                .not_implemented
                .contains(&"atomic_dead_letter_redrive".to_owned())
        );
        let nats = selection(r#"{"contract_version":1,"queue":"nats_jetstream"}"#).capabilities();
        assert!(
            nats.backends[0]
                .implemented
                .contains(&"dead_letter_queue".to_owned())
        );
        assert!(
            nats.backends[0]
                .implemented
                .contains(&"queue_replay_from_cursor".to_owned())
        );
    }

    #[test]
    fn reports_shared_cosine_score_direction_and_ttl() {
        let redis =
            selection(r#"{"contract_version":1,"kv":"redis","vector":"lance_db"}"#).capabilities();
        assert_eq!(redis.backends[1].ttl, Some(true));
        assert_eq!(
            redis.backends[0].score_direction,
            Some(ScoreDirection::HigherIsCloser)
        );
    }

    #[test]
    fn postgresql_does_not_claim_generic_stores_and_pubsub_is_not_durable() {
        let report = selection(r#"{"contract_version":1,"sql":"postgresql","pubsub":"nats_core"}"#)
            .capabilities();
        assert!(
            report.backends[0]
                .not_implemented
                .contains(&"tool_policy_store".to_owned())
        );
        assert_eq!(report.backends[1].durable, Some(false));
        assert!(
            report.backends[0]
                .notes
                .iter()
                .any(|note| note.contains("operator-managed"))
        );
        assert!(
            report.backends[0]
                .implemented
                .contains(&"ledger_projection_rebuild_verify".to_owned())
        );
        assert!(
            report.backends[0]
                .not_implemented
                .contains(&"engine_managed_postgresql_backup".to_owned())
        );
    }

    #[test]
    fn reports_backup_watch_prerequisites_and_unimplemented_storage_features() {
        let report = selection(
            r#"{"contract_version":1,"sql":"sqlite","kv":"redis","vector":"lance_db","queue":"redis_streams"}"#,
        )
        .capabilities();
        let sql = &report.backends[0];
        for capability in [
            "ledger_backup_online",
            "ledger_backup_offline",
            "ledger_backup_verify",
            "ledger_restore",
        ] {
            assert!(sql.implemented.iter().any(|item| item == capability));
        }
        let vector = &report.backends[1];
        for capability in ["metadata_filter", "configurable_ann_index", "quantization"] {
            assert!(vector.not_implemented.iter().any(|item| item == capability));
        }
        let kv = &report.backends[2];
        assert!(
            kv.notes
                .iter()
                .any(|note| note.contains("keyspace notifications"))
        );
        let queue = &report.backends[3];
        assert!(
            queue
                .implemented
                .iter()
                .any(|item| item == "scheduled_retry_within_visibility_timeout")
        );
        assert!(
            queue
                .implemented
                .iter()
                .any(|item| item == "idempotent_enqueue_configurable_window")
        );
        assert!(
            queue
                .not_implemented
                .iter()
                .any(|item| item == "atomic_dead_letter_redrive")
        );
    }

    #[test]
    fn empty_selection_reports_nothing() {
        assert!(
            selection(r#"{"contract_version":1}"#)
                .capabilities()
                .backends
                .is_empty()
        );
    }

    #[test]
    fn reports_falkordb_as_an_independent_graph_capability() {
        let report = selection(r#"{"contract_version":1,"graph":"falkordb"}"#).capabilities();
        assert_eq!(report.backends.len(), 1);
        assert_eq!(report.backends[0].kind, "graph");
        assert!(
            report.backends[0]
                .implemented
                .contains(&"property_graph".into())
        );
    }
}
