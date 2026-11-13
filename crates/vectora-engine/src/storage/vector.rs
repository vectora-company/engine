//! Scoped vector-store contract and LanceDB/Qdrant adapters.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const VECTOR_CONTRACT_VERSION: u16 = 2;
const MAX_VECTOR_DIMENSIONS: usize = 65_536;

fn valid_vector_dimensions(dimensions: usize) -> bool {
    (1..=MAX_VECTOR_DIMENSIONS).contains(&dimensions)
}

fn valid_vector(vector: &[f32]) -> bool {
    valid_vector_dimensions(vector.len()) && vector.iter().all(|value| value.is_finite())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VectorStoreConfig {
    pub backend: super::VectorBackend,
    pub endpoint: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VectorRecord {
    pub id: u64,
    pub scope: String,
    pub vector: Vec<f32>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VectorHit {
    pub id: u64,
    /// Cosine similarity in [-1, 1]; larger scores are closer for every backend.
    pub score: f32,
    pub metadata: Value,
}

fn qdrant_point_id(scope: &str, id: u64) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(scope.as_bytes());
    hasher.update(&[0]);
    hasher.update(&id.to_be_bytes());
    u64::from_be_bytes(
        hasher.finalize().as_bytes()[..8]
            .try_into()
            .expect("fixed digest length"),
    )
}

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("invalid vector request")]
    InvalidInput,
    #[error("vector-store operation failed")]
    Storage,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, collection: &str, record: VectorRecord) -> Result<(), VectorError>;
    async fn delete(&self, collection: &str, scope: &str, id: u64) -> Result<(), VectorError>;
    async fn search(
        &self,
        collection: &str,
        scope: &str,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<VectorHit>, VectorError>;
}

pub struct QdrantStore {
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

pub struct LanceDbStore {
    db: lancedb::Connection,
}

pub enum VectorStoreAdapter {
    LanceDb(LanceDbStore),
    Qdrant(QdrantStore),
}

impl VectorStoreAdapter {
    pub async fn connect(config: VectorStoreConfig) -> Result<Self, VectorError> {
        match config.backend {
            super::VectorBackend::LanceDb => Ok(Self::LanceDb(
                LanceDbStore::connect(&config.endpoint).await?,
            )),
            super::VectorBackend::Qdrant => Ok(Self::Qdrant(QdrantStore::new(
                &config.endpoint,
                config.api_key,
            )?)),
        }
    }
}

#[async_trait]
impl VectorStore for VectorStoreAdapter {
    async fn upsert(&self, collection: &str, record: VectorRecord) -> Result<(), VectorError> {
        match self {
            Self::LanceDb(store) => store.upsert(collection, record).await,
            Self::Qdrant(store) => store.upsert(collection, record).await,
        }
    }
    async fn delete(&self, collection: &str, scope: &str, id: u64) -> Result<(), VectorError> {
        match self {
            Self::LanceDb(store) => store.delete(collection, scope, id).await,
            Self::Qdrant(store) => store.delete(collection, scope, id).await,
        }
    }
    async fn search(
        &self,
        collection: &str,
        scope: &str,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<VectorHit>, VectorError> {
        match self {
            Self::LanceDb(store) => store.search(collection, scope, vector, limit).await,
            Self::Qdrant(store) => store.search(collection, scope, vector, limit).await,
        }
    }
}

impl LanceDbStore {
    pub async fn connect(uri: &str) -> Result<Self, VectorError> {
        let db = lancedb::connect(uri)
            .execute()
            .await
            .map_err(|_| VectorError::Storage)?;
        Ok(Self { db })
    }
}

fn lance_batch(
    record: &VectorRecord,
) -> Result<Box<dyn arrow_array::RecordBatchReader + Send>, VectorError> {
    use arrow_array::{
        ArrayRef, FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, UInt64Array,
        types::Float32Type,
    };
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;
    let dims = i32::try_from(record.vector.len()).map_err(|_| VectorError::InvalidInput)?;
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("scope", DataType::Utf8, false),
        Field::new("metadata", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dims),
            false,
        ),
    ]));
    let vector = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        [Some(
            record.vector.iter().copied().map(Some).collect::<Vec<_>>(),
        )],
        dims,
    );
    let metadata =
        serde_json::to_string(&record.metadata).map_err(|_| VectorError::InvalidInput)?;
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from(vec![record.id])) as ArrayRef,
            Arc::new(StringArray::from(vec![record.scope.as_str()])),
            Arc::new(StringArray::from(vec![metadata.as_str()])),
            Arc::new(vector),
        ],
    )
    .map_err(|_| VectorError::InvalidInput)?;
    Ok(Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema)))
}

async fn validate_lance_dimensions(
    table: &lancedb::Table,
    expected: usize,
) -> Result<(), VectorError> {
    let schema = table.schema().await.map_err(|_| VectorError::Storage)?;
    let field = schema
        .field_with_name("vector")
        .map_err(|_| VectorError::Storage)?;
    match field.data_type() {
        arrow_schema::DataType::FixedSizeList(_, dimensions)
            if usize::try_from(*dimensions).ok() == Some(expected) =>
        {
            Ok(())
        }
        arrow_schema::DataType::FixedSizeList(_, _) => Err(VectorError::InvalidInput),
        _ => Err(VectorError::Storage),
    }
}

#[async_trait]
impl VectorStore for LanceDbStore {
    async fn upsert(&self, collection: &str, record: VectorRecord) -> Result<(), VectorError> {
        QdrantStore::collection_name(collection)?;
        if record.scope.trim().is_empty() || !valid_vector(&record.vector) {
            return Err(VectorError::InvalidInput);
        }
        let names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|_| VectorError::Storage)?;
        let input = lance_batch(&record)?;
        if names.iter().any(|name| name == collection) {
            let table = self
                .db
                .open_table(collection)
                .execute()
                .await
                .map_err(|_| VectorError::Storage)?;
            validate_lance_dimensions(&table, record.vector.len()).await?;
            let mut merge = table.merge_insert(&["scope", "id"]);
            merge
                .when_matched_update_all(None)
                .when_not_matched_insert_all();
            merge
                .execute(input)
                .await
                .map_err(|_| VectorError::Storage)?;
        } else {
            self.db
                .create_table(collection, input)
                .execute()
                .await
                .map_err(|_| VectorError::Storage)?;
        }
        Ok(())
    }
    async fn delete(&self, collection: &str, scope: &str, id: u64) -> Result<(), VectorError> {
        QdrantStore::collection_name(collection)?;
        if scope.trim().is_empty() {
            return Err(VectorError::InvalidInput);
        }
        let table = self
            .db
            .open_table(collection)
            .execute()
            .await
            .map_err(|_| VectorError::Storage)?;
        let scope = scope.replace('\'', "''");
        table
            .delete(&format!("scope = '{scope}' AND id = {id}"))
            .await
            .map_err(|_| VectorError::Storage)?;
        Ok(())
    }
    async fn search(
        &self,
        collection: &str,
        scope: &str,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<VectorHit>, VectorError> {
        use arrow_array::{Float32Array, StringArray, UInt64Array};
        use futures_util::TryStreamExt;
        use lancedb::query::{ExecutableQuery, QueryBase};
        QdrantStore::collection_name(collection)?;
        if scope.trim().is_empty() || !valid_vector(&vector) || !(1..=1000).contains(&limit) {
            return Err(VectorError::InvalidInput);
        }
        let table = self
            .db
            .open_table(collection)
            .execute()
            .await
            .map_err(|_| VectorError::Storage)?;
        validate_lance_dimensions(&table, vector.len()).await?;
        let scope = scope.replace('\'', "''");
        let mut stream = table
            .query()
            .nearest_to(vector)
            .map_err(|_| VectorError::InvalidInput)?
            .distance_type(lancedb::DistanceType::Cosine)
            .only_if(format!("scope = '{scope}'"))
            .limit(limit as usize)
            .execute()
            .await
            .map_err(|_| VectorError::Storage)?;
        let mut hits = Vec::new();
        while let Some(batch) = stream.try_next().await.map_err(|_| VectorError::Storage)? {
            let ids = batch
                .column_by_name("id")
                .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
                .ok_or(VectorError::Storage)?;
            let metadata = batch
                .column_by_name("metadata")
                .and_then(|array| array.as_any().downcast_ref::<StringArray>())
                .ok_or(VectorError::Storage)?;
            let distances = batch
                .column_by_name("_distance")
                .and_then(|array| array.as_any().downcast_ref::<Float32Array>())
                .ok_or(VectorError::Storage)?;
            for idx in 0..batch.num_rows() {
                let value: Value =
                    serde_json::from_str(metadata.value(idx)).map_err(|_| VectorError::Storage)?;
                hits.push(VectorHit {
                    id: ids.value(idx),
                    score: 1.0 - distances.value(idx),
                    metadata: value,
                });
            }
        }
        Ok(hits)
    }
}

impl QdrantStore {
    pub fn new(endpoint: &str, api_key: Option<String>) -> Result<Self, VectorError> {
        let url = url::Url::parse(endpoint).map_err(|_| VectorError::InvalidInput)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(VectorError::InvalidInput);
        }
        Ok(Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            api_key,
        })
    }
    fn collection_name(name: &str) -> Result<(), VectorError> {
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(VectorError::InvalidInput);
        }
        Ok(())
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self
            .client
            .request(method, format!("{}{path}", self.endpoint));
        if let Some(key) = &self.api_key {
            request.header("api-key", key)
        } else {
            request
        }
    }
    pub async fn ensure_collection(
        &self,
        collection: &str,
        dimensions: usize,
    ) -> Result<(), VectorError> {
        Self::collection_name(collection)?;
        if !valid_vector_dimensions(dimensions) {
            return Err(VectorError::InvalidInput);
        }
        let path = format!("/collections/{collection}");
        let existing = self
            .request(reqwest::Method::GET, &path)
            .send()
            .await
            .map_err(|_| VectorError::Storage)?;
        if existing.status().is_success() {
            let body: Value = existing.json().await.map_err(|_| VectorError::Storage)?;
            let existing_dimensions = body
                .pointer("/result/config/params/vectors/size")
                .and_then(Value::as_u64);
            return if existing_dimensions == Some(dimensions as u64) {
                Ok(())
            } else {
                Err(VectorError::InvalidInput)
            };
        }
        if existing.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(VectorError::Storage);
        }
        let response = self
            .request(reqwest::Method::PUT, &path)
            .json(&serde_json::json!({"vectors":{"size":dimensions,"distance":"Cosine"}}))
            .send()
            .await
            .map_err(|_| VectorError::Storage)?;
        if response.status().is_success() || response.status() == reqwest::StatusCode::CONFLICT {
            Ok(())
        } else {
            Err(VectorError::Storage)
        }
    }
}

#[async_trait]
impl VectorStore for QdrantStore {
    async fn upsert(&self, collection: &str, record: VectorRecord) -> Result<(), VectorError> {
        Self::collection_name(collection)?;
        if record.scope.trim().is_empty() || !valid_vector(&record.vector) {
            return Err(VectorError::InvalidInput);
        }
        self.ensure_collection(collection, record.vector.len())
            .await?;
        let response = self.request(reqwest::Method::PUT, &format!("/collections/{collection}/points?wait=true"))
            .json(&serde_json::json!({"points":[{"id":qdrant_point_id(&record.scope, record.id),"vector":record.vector,"payload":{"_vectora_scope":record.scope,"_vectora_original_id":record.id,"_vectora_metadata":record.metadata}}]})).send().await.map_err(|_| VectorError::Storage)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(VectorError::Storage)
        }
    }
    async fn delete(&self, collection: &str, scope: &str, id: u64) -> Result<(), VectorError> {
        Self::collection_name(collection)?;
        if scope.trim().is_empty() {
            return Err(VectorError::InvalidInput);
        }
        let response = self.request(reqwest::Method::POST, &format!("/collections/{collection}/points/delete?wait=true"))
            .json(&serde_json::json!({"points":[qdrant_point_id(scope, id)],"filter":{"must":[{"key":"_vectora_scope","match":{"value":scope}}]}})).send().await.map_err(|_| VectorError::Storage)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(VectorError::Storage)
        }
    }
    async fn search(
        &self,
        collection: &str,
        scope: &str,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<VectorHit>, VectorError> {
        Self::collection_name(collection)?;
        if scope.trim().is_empty() || !valid_vector(&vector) || !(1..=1000).contains(&limit) {
            return Err(VectorError::InvalidInput);
        }
        let dimensions = vector.len() as u64;
        let response = self.request(reqwest::Method::POST, &format!("/collections/{collection}/points/search"))
            .json(&serde_json::json!({"vector":vector,"filter":{"must":[{"key":"_vectora_scope","match":{"value":scope}}]},"limit":limit,"with_payload":true})).send().await.map_err(|_| VectorError::Storage)?;
        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            let collection_response = self
                .request(reqwest::Method::GET, &format!("/collections/{collection}"))
                .send()
                .await
                .map_err(|_| VectorError::Storage)?;
            if collection_response.status().is_success() {
                let config: Value = collection_response
                    .json()
                    .await
                    .map_err(|_| VectorError::Storage)?;
                if config
                    .pointer("/result/config/params/vectors/size")
                    .and_then(Value::as_u64)
                    .is_some_and(|size| size != dimensions)
                {
                    return Err(VectorError::InvalidInput);
                }
            }
        }
        if !response.status().is_success() {
            return Err(VectorError::Storage);
        }
        let body: Value = response.json().await.map_err(|_| VectorError::Storage)?;
        body.get("result")
            .and_then(Value::as_array)
            .ok_or(VectorError::Storage)?
            .iter()
            .map(|item| {
                Ok(VectorHit {
                    id: item
                        .get("payload")
                        .and_then(|p| p.get("_vectora_original_id"))
                        .and_then(Value::as_u64)
                        .ok_or(VectorError::Storage)?,
                    score: item
                        .get("score")
                        .and_then(Value::as_f64)
                        .ok_or(VectorError::Storage)? as f32,
                    metadata: item
                        .get("payload")
                        .and_then(|p| p.get("_vectora_metadata"))
                        .cloned()
                        .unwrap_or(Value::Null),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn qdrant_scoped_upsert_search_and_delete_against_real_service() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_QDRANT_URL") else {
            return;
        };
        let store = QdrantStore::new(&endpoint, None).unwrap();
        let collection = format!("vectora_test_{}", std::process::id());
        store
            .upsert(
                &collection,
                VectorRecord {
                    id: 1,
                    scope: "one".into(),
                    vector: vec![1.0, 0.0, 0.0],
                    metadata: json!({"text":"alpha"}),
                },
            )
            .await
            .unwrap();
        store
            .upsert(
                &collection,
                VectorRecord {
                    id: 1,
                    scope: "two".into(),
                    vector: vec![1.0, 0.0, 0.0],
                    metadata: json!({"text":"private"}),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            store.ensure_collection(&collection, 2).await,
            Err(VectorError::InvalidInput)
        ));
        let hits = store
            .search(&collection, "one", vec![1.0, 0.0, 0.0], 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].metadata, json!({"text":"alpha"}));
        store.delete(&collection, "one", 1).await.unwrap();
        assert!(
            store
                .search(&collection, "one", vec![1.0, 0.0, 0.0], 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn lancedb_scoped_upsert_search_delete_and_metadata() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let path = std::env::temp_dir().join(format!(
            "vectora-lance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = LanceDbStore::connect(path.to_str().unwrap()).await.unwrap();
        store
            .upsert(
                "memories",
                VectorRecord {
                    id: 1,
                    scope: "one".into(),
                    vector: vec![1.0, 0.0, 0.0],
                    metadata: json!({"text":"alpha"}),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            store
                .upsert(
                    "memories",
                    VectorRecord {
                        id: 3,
                        scope: "one".into(),
                        vector: vec![1.0, 0.0],
                        metadata: json!({}),
                    },
                )
                .await,
            Err(VectorError::InvalidInput)
        ));
        store
            .upsert(
                "memories",
                VectorRecord {
                    id: 2,
                    scope: "two".into(),
                    vector: vec![1.0, 0.0, 0.0],
                    metadata: json!({"text":"private"}),
                },
            )
            .await
            .unwrap();
        let hits = store
            .search("memories", "one", vec![1.0, 0.0, 0.0], 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].metadata, json!({"text":"alpha"}));
        store.delete("memories", "one", 1).await.unwrap();
        assert!(
            store
                .search("memories", "one", vec![1.0, 0.0, 0.0], 10)
                .await
                .unwrap()
                .is_empty()
        );
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn vector_contract_rejects_unbounded_and_non_finite_input() {
        assert!(QdrantStore::new("file:///tmp/qdrant", None).is_err());
        let store = QdrantStore::new("http://127.0.0.1:1", None).unwrap();
        let invalid = VectorRecord {
            id: 1,
            scope: "".into(),
            vector: vec![f32::NAN],
            metadata: Value::Null,
        };
        assert!(matches!(
            store.upsert("valid", invalid).await,
            Err(VectorError::InvalidInput)
        ));
    }

    fn record(id: u64, scope: &str, vector: [f32; 3], text: &str) -> VectorRecord {
        VectorRecord {
            id,
            scope: scope.into(),
            vector: vector.to_vec(),
            metadata: json!({"text": text}),
        }
    }

    /// Exercises behavior common to both vector backends, including cosine
    /// ranking for non-unit vectors and a shared score direction.
    async fn exercise_shared_vector_contract(store: &dyn VectorStore, collection: &str) {
        assert!(matches!(
            store
                .search(
                    &format!("{collection}_missing"),
                    "one",
                    vec![1.0, 0.0, 0.0],
                    10,
                )
                .await,
            Err(VectorError::Storage)
        ));
        for item in [
            record(1, "one", [2.0, 0.0, 0.0], "exact"),
            record(2, "one", [1.0, 10.0, 0.0], "near"),
            record(3, "one", [0.0, 1.0, 0.0], "far"),
            record(1, "two", [2.0, 0.0, 0.0], "other-scope"),
        ] {
            store.upsert(collection, item).await.unwrap();
        }
        let query = || vec![2.0, 0.0, 0.0];
        let hits = store.search(collection, "one", query(), 10).await.unwrap();
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert!(
            hits.iter().all(|hit| hit.metadata["text"] != "other-scope"),
            "search must not return another scope"
        );
        assert!(hits[0].score > hits[1].score && hits[1].score > hits[2].score);
        assert!((hits[0].score - 1.0).abs() < 1e-5);
        assert!((hits[2].score).abs() < 1e-5);
        let limited = store.search(collection, "one", query(), 1).await.unwrap();
        assert_eq!(limited.iter().map(|h| h.id).collect::<Vec<_>>(), [1]);

        store
            .upsert(collection, record(1, "one", [1.0, 0.0, 0.0], "replaced"))
            .await
            .unwrap();
        let hits = store.search(collection, "one", query(), 10).await.unwrap();
        assert_eq!(hits.len(), 3, "upserting an id must replace, not duplicate");
        assert_eq!(hits[0].metadata["text"], "replaced");

        store.delete(collection, "one", 1).await.unwrap();
        let other = store.search(collection, "two", query(), 10).await.unwrap();
        assert_eq!(
            other.len(),
            1,
            "deleting in one scope must not affect another"
        );
        assert_eq!(other[0].metadata["text"], "other-scope");
        assert!(
            store
                .search(collection, "one", vec![1.0, 0.0], 10)
                .await
                .is_err(),
            "a query with the wrong dimension must fail"
        );
    }

    #[tokio::test]
    async fn qdrant_shares_the_cosine_score_contract() {
        let Ok(endpoint) = std::env::var("VECTORA_TEST_QDRANT_URL") else {
            return;
        };
        let store = QdrantStore::new(&endpoint, None).unwrap();
        let collection = format!(
            "vectora_shared_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        exercise_shared_vector_contract(&store, &collection).await;
    }

    #[tokio::test]
    async fn qdrant_collection_and_records_survive_service_restart() {
        let (Ok(endpoint), Ok(container)) = (
            std::env::var("VECTORA_TEST_QDRANT_URL"),
            std::env::var("VECTORA_TEST_QDRANT_CONTAINER"),
        ) else {
            return;
        };
        if std::env::var("VECTORA_TEST_PODMAN_RESTART").is_err() {
            return;
        }
        assert!(
            !container.is_empty()
                && container
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'),
            "test container name must be a plain Podman identifier"
        );
        let store = QdrantStore::new(&endpoint, None).unwrap();
        let collection = format!(
            "restart_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        store
            .upsert(
                &collection,
                record(7, "scope", [1.0, 0.0, 0.0], "persisted"),
            )
            .await
            .unwrap();

        let status = std::process::Command::new("podman")
            .args(["restart", "-t", "5", &container])
            .status()
            .expect("podman is required for the opt-in Qdrant restart test");
        assert!(status.success(), "podman restart {container} failed");

        let mut recovered = false;
        for _ in 0..60 {
            if let Ok(hits) = store
                .search(&collection, "scope", vec![1.0, 0.0, 0.0], 10)
                .await
                && hits.len() == 1
                && hits[0].metadata["text"] == "persisted"
            {
                recovered = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        assert!(
            recovered,
            "Qdrant did not recover the collection and point after restart"
        );
    }

    #[tokio::test]
    async fn lancedb_shares_the_cosine_score_contract() {
        let path = std::env::temp_dir().join(format!(
            "vectora-lance-shared-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = LanceDbStore::connect(path.to_str().unwrap()).await.unwrap();
        exercise_shared_vector_contract(&store, "shared").await;
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    fn unreachable_qdrant() -> QdrantStore {
        QdrantStore::new("http://127.0.0.1:1", None).unwrap()
    }

    #[tokio::test]
    async fn search_limit_vector_scope_and_collection_bounds_are_enforced() {
        let store = unreachable_qdrant();
        let query = || vec![1.0_f32, 0.0];
        for limit in [0, 1001, u32::MAX] {
            assert!(
                matches!(
                    store.search("c", "s", query(), limit).await,
                    Err(VectorError::InvalidInput)
                ),
                "limit {limit} must be rejected"
            );
        }
        // Valid bounds pass validation and only then fail reaching the service.
        for limit in [1, 1000] {
            assert!(
                matches!(
                    store.search("c", "s", query(), limit).await,
                    Err(VectorError::Storage)
                ),
                "limit {limit} must be accepted"
            );
        }
        for (collection, scope, vector) in [
            ("c", " ", query()),
            ("c", "s", vec![]),
            ("c", "s", vec![f32::INFINITY]),
            ("", "s", query()),
            ("a/b", "s", query()),
            (&"x".repeat(129), "s", query()),
        ] {
            assert!(
                matches!(
                    store.search(collection, scope, vector, 10).await,
                    Err(VectorError::InvalidInput)
                ),
                "invalid search input must be rejected"
            );
        }
        assert!(matches!(
            store.search(&"x".repeat(128), "s", query(), 10).await,
            Err(VectorError::Storage)
        ));
    }

    #[tokio::test]
    async fn upsert_vector_dimension_bound_is_inclusive_at_65536() {
        let store = unreachable_qdrant();
        let record = |dimensions: usize| VectorRecord {
            id: 1,
            scope: "s".into(),
            vector: vec![0.5; dimensions],
            metadata: Value::Null,
        };
        assert!(matches!(
            store.upsert("c", record(65537)).await,
            Err(VectorError::InvalidInput)
        ));
        assert!(matches!(
            store.upsert("c", record(0)).await,
            Err(VectorError::InvalidInput)
        ));
        assert!(matches!(
            store.upsert("c", record(65536)).await,
            Err(VectorError::Storage)
        ));
    }

    #[tokio::test]
    async fn lancedb_rejects_out_of_range_limits_before_touching_storage() {
        let path = std::env::temp_dir().join(format!(
            "vectora-lance-bounds-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = LanceDbStore::connect(path.to_str().unwrap()).await.unwrap();
        for limit in [0, 1001] {
            assert!(matches!(
                store.search("c", "s", vec![1.0], limit).await,
                Err(VectorError::InvalidInput)
            ));
        }
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }
}
