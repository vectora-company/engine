//! Portable property-graph operations and the FalkorDB Cypher extension.

use std::{collections::HashMap, fmt, time::Duration};

use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt, future::BoxFuture};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub const GRAPH_CONTRACT_VERSION: u16 = 1;
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_ROWS: usize = 1_000;
const DEFAULT_MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;

/// Connection settings for one named FalkorDB graph.
#[derive(Clone)]
pub struct GraphStoreConfig {
    pub endpoint: String,
    pub graph_name: String,
    pub query_timeout: Duration,
    pub max_rows: usize,
    pub max_result_bytes: usize,
}

impl GraphStoreConfig {
    pub fn new(endpoint: impl Into<String>, graph_name: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            graph_name: graph_name.into(),
            query_timeout: DEFAULT_QUERY_TIMEOUT,
            max_rows: DEFAULT_MAX_ROWS,
            max_result_bytes: DEFAULT_MAX_RESULT_BYTES,
        }
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        if self.endpoint.trim().is_empty()
            || self.graph_name.trim().is_empty()
            || self.graph_name.len() > 256
            || self.graph_name.chars().any(char::is_control)
            || self.query_timeout.is_zero()
            || self.query_timeout.as_millis() > i64::MAX as u128
            || self.max_rows == 0
            || self.max_result_bytes == 0
        {
            return Err(GraphError::InvalidInput);
        }
        Ok(())
    }
}

impl fmt::Debug for GraphStoreConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphStoreConfig")
            .field("endpoint", &"<redacted>")
            .field("graph_name", &self.graph_name)
            .field("query_timeout", &self.query_timeout)
            .field("max_rows", &self.max_rows)
            .field("max_result_bytes", &self.max_result_bytes)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphRelationship {
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub properties: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphNeighbor {
    pub node: GraphNode,
    pub relationship: GraphRelationship,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("invalid graph request")]
    InvalidInput,
    #[error("graph result exceeded configured limits")]
    ResultTooLarge,
    #[error("graph operation failed")]
    Storage,
}

/// Common, typed property-graph operations. A relationship is unique by
/// (source, target, kind); upserting the tuple replaces its properties.
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn upsert_node(&self, node: GraphNode) -> Result<(), GraphError>;
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>, GraphError>;
    async fn delete_node(&self, id: &str) -> Result<(), GraphError>;
    async fn upsert_relationship(&self, edge: GraphRelationship) -> Result<(), GraphError>;
    async fn neighbors(
        &self,
        node_id: &str,
        direction: GraphDirection,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphNeighbor>, GraphError>;
}

/// Raw Cypher extension. Parameters are safely encoded by the official client;
/// this deliberately exposes FalkorDB's OpenCypher dialect to opt-in callers.
#[async_trait]
pub trait FalkorCypher: Send + Sync {
    async fn query(
        &self,
        cypher: &str,
        params: Map<String, Value>,
    ) -> Result<GraphQueryResult, GraphError>;
}

pub struct FalkorDbStore {
    client: falkordb::FalkorAsyncClient,
    graph_name: String,
    timeout_ms: i64,
    max_rows: usize,
    max_result_bytes: usize,
}

impl FalkorDbStore {
    pub async fn connect(config: GraphStoreConfig) -> Result<Self, GraphError> {
        config.validate()?;
        let connection = falkordb::FalkorConnectionInfo::try_from(config.endpoint.as_str())
            .map_err(|_| GraphError::InvalidInput)?;
        let client = falkordb::FalkorClientBuilder::new_async()
            .with_connection_info(connection)
            .with_retry_policy(falkordb::RetryPolicy::disabled())
            .build()
            .await
            .map_err(|_| GraphError::Storage)?;
        Ok(Self {
            client,
            graph_name: config.graph_name,
            timeout_ms: config.query_timeout.as_millis() as i64,
            max_rows: config.max_rows,
            max_result_bytes: config.max_result_bytes,
        })
    }

    fn execute<'a>(
        &'a self,
        query: String,
        params: Map<String, Value>,
    ) -> BoxFuture<'a, Result<GraphQueryResult, GraphError>> {
        async move {
            if query.trim().is_empty() {
                return Err(GraphError::InvalidInput);
            }
            let mut graph = self.client.select_graph(&self.graph_name);
            let mut builder = graph.query(&query).with_timeout(self.timeout_ms);
            for (name, value) in params {
                builder = builder.with_param(&name, to_falkor_value(value)?);
            }
            let mut result = builder.execute().await.map_err(|_| GraphError::Storage)?;
            let columns = result.header.iter().cloned().collect::<Vec<_>>();
            let mut rows = Vec::new();
            let mut bytes = 0usize;
            while let Some(row) = result.data.next().await {
                if rows.len() >= self.max_rows {
                    return Err(GraphError::ResultTooLarge);
                }
                let row = row.map_err(|_| GraphError::Storage)?;
                let mut values = Vec::with_capacity(row.len());
                for value in row.into_values() {
                    let json = falkordb::from_falkor_value::<Value>(value)
                        .map_err(|_| GraphError::Storage)?;
                    bytes = bytes
                        .checked_add(
                            serde_json::to_vec(&json)
                                .map_err(|_| GraphError::Storage)?
                                .len(),
                        )
                        .ok_or(GraphError::ResultTooLarge)?;
                    if bytes > self.max_result_bytes {
                        return Err(GraphError::ResultTooLarge);
                    }
                    values.push(json);
                }
                rows.push(values);
            }
            Ok(GraphQueryResult { columns, rows })
        }
        .boxed()
    }
}

#[async_trait]
impl FalkorCypher for FalkorDbStore {
    async fn query(
        &self,
        cypher: &str,
        params: Map<String, Value>,
    ) -> Result<GraphQueryResult, GraphError> {
        self.execute(cypher.to_owned(), params).await
    }
}

#[async_trait]
impl GraphStore for FalkorDbStore {
    async fn upsert_node(&self, node: GraphNode) -> Result<(), GraphError> {
        validate_node(&node)?;
        let payload = serde_json::to_string(&node).map_err(|_| GraphError::InvalidInput)?;
        let mut graph_properties = node.properties.clone();
        graph_properties.insert("id".into(), Value::String(node.id.clone()));
        graph_properties.insert(
            "labels".into(),
            serde_json::to_value(&node.labels).map_err(|_| GraphError::InvalidInput)?,
        );
        graph_properties.insert("payload".into(), Value::String(payload));
        let mut params = Map::new();
        params.insert("id".into(), Value::String(node.id));
        params.insert("properties".into(), Value::Object(graph_properties));
        self.execute(
            "MERGE (n:__VectoraNode {id: $id}) SET n = $properties RETURN n.id AS id".into(),
            params,
        )
        .await?;
        Ok(())
    }

    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>, GraphError> {
        validate_text(id)?;
        let mut params = Map::new();
        params.insert("id".into(), Value::String(id.to_owned()));
        let result = self
            .execute(
                "MATCH (n:__VectoraNode {id: $id}) RETURN n.payload AS payload LIMIT 1".into(),
                params,
            )
            .await?;
        match result.rows.first().and_then(|row| row.first()) {
            Some(Value::String(payload)) => serde_json::from_str(payload)
                .map(Some)
                .map_err(|_| GraphError::Storage),
            Some(_) => Err(GraphError::Storage),
            None => Ok(None),
        }
    }

    async fn delete_node(&self, id: &str) -> Result<(), GraphError> {
        validate_text(id)?;
        let mut params = Map::new();
        params.insert("id".into(), Value::String(id.to_owned()));
        self.execute(
            "MATCH (n:__VectoraNode {id: $id}) DETACH DELETE n RETURN 1 AS deleted".into(),
            params,
        )
        .await?;
        Ok(())
    }

    async fn upsert_relationship(&self, edge: GraphRelationship) -> Result<(), GraphError> {
        validate_edge(&edge)?;
        let payload = serde_json::to_string(&edge).map_err(|_| GraphError::InvalidInput)?;
        let mut graph_properties = edge.properties.clone();
        graph_properties.insert("kind".into(), Value::String(edge.kind.clone()));
        graph_properties.insert("payload".into(), Value::String(payload));
        let mut params = Map::new();
        params.insert("source".into(), Value::String(edge.source_id));
        params.insert("target".into(), Value::String(edge.target_id));
        params.insert("kind".into(), Value::String(edge.kind));
        params.insert("properties".into(), Value::Object(graph_properties));
        let result = self
            .execute(
                "MATCH (a:__VectoraNode {id: $source}), (b:__VectoraNode {id: $target}) MERGE (a)-[r:__VectoraRelationship {kind: $kind}]->(b) SET r = $properties RETURN r.kind AS kind".into(),
                params,
            )
            .await?;
        if result.rows.is_empty() {
            return Err(GraphError::InvalidInput);
        }
        Ok(())
    }

    async fn neighbors(
        &self,
        node_id: &str,
        direction: GraphDirection,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphNeighbor>, GraphError> {
        validate_text(node_id)?;
        if limit == 0 || limit > self.max_rows {
            return Err(GraphError::InvalidInput);
        }
        if let Some(kind) = kind {
            validate_text(kind)?;
        }
        let (pattern, predicate, target) = match direction {
            GraphDirection::Outgoing => (
                "(a)-[r:__VectoraRelationship]->(b)",
                "a.id = $id",
                "b.payload",
            ),
            GraphDirection::Incoming => (
                "(a)-[r:__VectoraRelationship]->(b)",
                "b.id = $id",
                "a.payload",
            ),
            GraphDirection::Both => (
                "(a)-[r:__VectoraRelationship]->(b)",
                "(a.id = $id OR b.id = $id)",
                "CASE WHEN a.id = $id THEN b.payload ELSE a.payload END",
            ),
        };
        let query = format!(
            "MATCH {pattern} WHERE {predicate} AND ($kind IS NULL OR r.kind = $kind) RETURN {target} AS node, r.payload AS relationship LIMIT {limit}"
        );
        let mut params = Map::new();
        params.insert("id".into(), Value::String(node_id.to_owned()));
        params.insert(
            "kind".into(),
            kind.map_or(Value::Null, |v| Value::String(v.into())),
        );
        let result = self.execute(query, params).await?;
        result
            .rows
            .into_iter()
            .map(|row| {
                let node = row
                    .first()
                    .and_then(Value::as_str)
                    .ok_or(GraphError::Storage)?;
                let relationship = row
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or(GraphError::Storage)?;
                Ok(GraphNeighbor {
                    node: serde_json::from_str(node).map_err(|_| GraphError::Storage)?,
                    relationship: serde_json::from_str(relationship)
                        .map_err(|_| GraphError::Storage)?,
                })
            })
            .collect()
    }
}

fn to_falkor_value(value: Value) -> Result<falkordb::FalkorValue, GraphError> {
    use falkordb::FalkorValue as F;
    Ok(match value {
        Value::Null => F::None,
        Value::Bool(v) => F::Bool(v),
        Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                F::I64(i)
            } else if let Some(u) = v.as_u64() {
                F::I64(i64::try_from(u).map_err(|_| GraphError::InvalidInput)?)
            } else {
                let f = v.as_f64().ok_or(GraphError::InvalidInput)?;
                if !f.is_finite() {
                    return Err(GraphError::InvalidInput);
                }
                F::F64(f)
            }
        }
        Value::String(v) => F::String(v),
        Value::Array(values) => F::Array(
            values
                .into_iter()
                .map(to_falkor_value)
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => F::Map(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, to_falkor_value(value)?)))
                .collect::<Result<HashMap<_, _>, GraphError>>()?,
        ),
    })
}

fn validate_text(value: &str) -> Result<(), GraphError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        Err(GraphError::InvalidInput)
    } else {
        Ok(())
    }
}

fn validate_node(node: &GraphNode) -> Result<(), GraphError> {
    validate_text(&node.id)?;
    if node
        .labels
        .iter()
        .any(|label| validate_text(label).is_err())
    {
        return Err(GraphError::InvalidInput);
    }
    validate_properties(&node.properties, &["id", "labels", "payload"])
}

fn validate_edge(edge: &GraphRelationship) -> Result<(), GraphError> {
    validate_text(&edge.source_id)?;
    validate_text(&edge.target_id)?;
    validate_text(&edge.kind)?;
    validate_properties(&edge.properties, &["kind", "payload"])
}

fn validate_properties(
    properties: &Map<String, Value>,
    reserved: &[&str],
) -> Result<(), GraphError> {
    for (key, value) in properties {
        validate_text(key)?;
        if reserved.contains(&key.as_str()) || !is_graph_property(value) {
            return Err(GraphError::InvalidInput);
        }
    }
    Ok(())
}

fn is_graph_property(value: &Value) -> bool {
    match value {
        Value::Null | Value::Object(_) => false,
        Value::Array(items) => items
            .iter()
            .all(|item| matches!(item, Value::Bool(_) | Value::Number(_) | Value::String(_))),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_config_redacts_credentials() {
        let config = GraphStoreConfig::new("redis://user:secret@localhost:6379", "tenant-a");
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("tenant-a"));
    }

    #[test]
    fn graph_config_and_identifiers_are_validated() {
        assert!(
            GraphStoreConfig::new("redis://localhost:6379", "tenant")
                .validate()
                .is_ok()
        );
        assert!(GraphStoreConfig::new(" ", "tenant").validate().is_err());
        assert!(validate_text("\n").is_err());
    }

    fn restart_test_container(name: &str) -> bool {
        if std::env::var("VECTORA_TEST_PODMAN_RESTART").is_err() {
            return false;
        }
        let status = std::process::Command::new("podman")
            .args(["restart", "-t", "5", name])
            .status()
            .expect("podman is required for service restart tests");
        assert!(status.success(), "podman restart {name} failed");
        true
    }

    async fn eventually_graph<T, F, Fut>(mut operation: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, GraphError>>,
    {
        let mut last_error = None;
        for _ in 0..40 {
            match tokio::time::timeout(Duration::from_secs(5), operation()).await {
                Ok(Ok(value)) => return value,
                Ok(Err(error)) => last_error = Some(error.to_string()),
                Err(_) => last_error = Some("timed out".into()),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("FalkorDB did not recover after restart: {last_error:?}");
    }

    #[test]
    fn parameters_keep_json_types_and_reject_unsigned_overflow() {
        assert!(matches!(
            to_falkor_value(Value::Bool(true)).unwrap(),
            falkordb::FalkorValue::Bool(true)
        ));
        assert!(to_falkor_value(serde_json::json!(u64::MAX)).is_err());
        assert!(
            validate_properties(
                serde_json::json!({"nested":{"value":1}})
                    .as_object()
                    .unwrap(),
                &[],
            )
            .is_err()
        );
        assert!(
            validate_properties(
                serde_json::json!({"id":"reserved"}).as_object().unwrap(),
                &["id"],
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn portable_graph_and_parameterized_cypher_against_falkordb() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_FALKORDB_URL") else {
            return;
        };
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let store = FalkorDbStore::connect(GraphStoreConfig::new(
            endpoint,
            format!("vectora_test_{suffix}"),
        ))
        .await
        .unwrap();
        let left = GraphNode {
            id: "esquerda-東京".into(),
            labels: vec!["Pessoa".into()],
            properties: serde_json::json!({"nome":"Ana"})
                .as_object()
                .unwrap()
                .clone(),
        };
        let right = GraphNode {
            id: "direita".into(),
            labels: vec!["Pessoa".into()],
            properties: serde_json::json!({"nome":"Bia"})
                .as_object()
                .unwrap()
                .clone(),
        };
        store.upsert_node(left.clone()).await.unwrap();
        store.upsert_node(right.clone()).await.unwrap();
        store
            .upsert_relationship(GraphRelationship {
                source_id: left.id.clone(),
                target_id: right.id.clone(),
                kind: "CONHECE".into(),
                properties: serde_json::json!({"desde":2024})
                    .as_object()
                    .unwrap()
                    .clone(),
            })
            .await
            .unwrap();

        assert_eq!(store.get_node(&left.id).await.unwrap(), Some(left.clone()));
        let mut node_params = Map::new();
        node_params.insert("id".into(), Value::String(left.id.clone()));
        let node_properties = store
            .query(
                "MATCH (n:__VectoraNode {id: $id}) RETURN n.nome AS nome, n.labels AS labels",
                node_params.clone(),
            )
            .await
            .unwrap();
        assert_eq!(node_properties.rows[0][0], Value::String("Ana".into()));
        assert_eq!(node_properties.rows[0][1], serde_json::json!(["Pessoa"]));
        if restart_test_container("vectora-engine-falkordb-test") {
            let persisted = eventually_graph(|| store.get_node(&left.id)).await;
            assert_eq!(persisted, Some(left.clone()));
            let persisted_edge =
                eventually_graph(|| store.neighbors(&left.id, GraphDirection::Outgoing, None, 10))
                    .await;
            assert_eq!(persisted_edge[0].node.id, right.id);
            store.upsert_node(right.clone()).await.unwrap();
        }
        store
            .upsert_node(GraphNode {
                id: left.id.clone(),
                labels: vec!["Pessoa".into()],
                properties: serde_json::json!({"cidade":"São Paulo"})
                    .as_object()
                    .unwrap()
                    .clone(),
            })
            .await
            .unwrap();
        let replaced = store
            .query(
                "MATCH (n:__VectoraNode {id: $id}) RETURN n.nome AS nome, n.cidade AS cidade",
                node_params,
            )
            .await
            .unwrap();
        assert_eq!(replaced.rows[0][0], Value::Null);
        assert_eq!(replaced.rows[0][1], Value::String("São Paulo".into()));
        let outgoing = store
            .neighbors(&left.id, GraphDirection::Outgoing, Some("CONHECE"), 10)
            .await
            .unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].node.id, right.id);
        let incoming = store
            .neighbors(&right.id, GraphDirection::Incoming, None, 10)
            .await
            .unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].node.id, left.id);
        let both = store
            .neighbors(&right.id, GraphDirection::Both, None, 10)
            .await
            .unwrap();
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].node.id, "esquerda-東京");

        let mut params = Map::new();
        params.insert("answer".into(), Value::String("parâmetro".into()));
        let cypher = store
            .query("RETURN $answer AS answer", params)
            .await
            .unwrap();
        assert_eq!(cypher.rows, vec![vec![Value::String("parâmetro".into())]]);
        if let Ok(auth_endpoint) = std::env::var("VECTORA_TEST_FALKORDB_AUTH_URL") {
            let auth_store = FalkorDbStore::connect(GraphStoreConfig::new(
                auth_endpoint,
                format!("vectora_auth_{suffix}"),
            ))
            .await
            .unwrap();
            assert_eq!(
                auth_store
                    .query("RETURN 1 AS authenticated", Map::new())
                    .await
                    .unwrap()
                    .rows,
                vec![vec![Value::from(1)]]
            );
        }
        let mut row_limit_config = GraphStoreConfig::new(
            std::env::var("VECTORA_TEST_FALKORDB_URL").unwrap(),
            format!("vectora_row_limit_{suffix}"),
        );
        row_limit_config.max_rows = 1;
        let row_limit_store = FalkorDbStore::connect(row_limit_config).await.unwrap();
        assert!(matches!(
            row_limit_store
                .query("UNWIND [1, 2] AS n RETURN n", Map::new())
                .await,
            Err(GraphError::ResultTooLarge)
        ));
        let mut byte_limit_config = GraphStoreConfig::new(
            std::env::var("VECTORA_TEST_FALKORDB_URL").unwrap(),
            format!("vectora_byte_limit_{suffix}"),
        );
        byte_limit_config.max_result_bytes = 1;
        let byte_limit_store = FalkorDbStore::connect(byte_limit_config).await.unwrap();
        assert!(matches!(
            byte_limit_store
                .query("RETURN 'larger than the limit' AS value", Map::new())
                .await,
            Err(GraphError::ResultTooLarge)
        ));
        let mut timeout_config = GraphStoreConfig::new(
            std::env::var("VECTORA_TEST_FALKORDB_URL").unwrap(),
            format!("vectora_timeout_{suffix}"),
        );
        timeout_config.query_timeout = Duration::from_millis(1);
        let timeout_store = FalkorDbStore::connect(timeout_config).await.unwrap();
        let timeout_result = timeout_store
            .query("UNWIND range(1, 1000000) AS n RETURN sum(n)", Map::new())
            .await;
        assert!(matches!(timeout_result, Err(GraphError::Storage)));
        store.delete_node(&left.id).await.unwrap();
        assert_eq!(store.get_node(&left.id).await.unwrap(), None);
    }
}
