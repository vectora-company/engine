//! Versioned storage selection contracts. Adapters are introduced separately.
//!
//! Selection describes intent only: it does not open a connection, authenticate
//! a caller, or establish the guarantees of a remote service.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod sqlite;

pub const STORAGE_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SqlBackend {
    Sqlite,
    Postgresql,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorBackend {
    LanceDb,
    Qdrant,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KvBackend {
    Redis,
    NatsJetstream,
    Memory,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueBackend {
    RedisStreams,
    NatsJetstream,
    Memory,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PubSubBackend {
    Redis,
    NatsCore,
    Memory,
}

/// Independent selections; absence means the capability is not requested.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageSelection {
    pub contract_version: u16,
    #[serde(default)]
    pub sql: Option<SqlBackend>,
    #[serde(default)]
    pub vector: Option<VectorBackend>,
    #[serde(default)]
    pub kv: Option<KvBackend>,
    #[serde(default)]
    pub queue: Option<QueueBackend>,
    #[serde(default)]
    pub pubsub: Option<PubSubBackend>,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("unsupported storage contract version: {0}")]
    UnsupportedVersion(u16),
    #[error("storage scope must not be empty")]
    EmptyScope,
}

impl StorageSelection {
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.contract_version != STORAGE_CONTRACT_VERSION {
            return Err(StorageError::UnsupportedVersion(self.contract_version));
        }
        Ok(())
    }
}

/// Supplied by the trusted host; validation does not authenticate the caller.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageScope {
    pub namespace: String,
}

impl StorageScope {
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.namespace.trim().is_empty() {
            return Err(StorageError::EmptyScope);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_are_independent_and_versions_checked() {
        let selection: StorageSelection = serde_json::from_str(
            r#"{"contract_version":1,"sql":"sqlite","vector":"qdrant","queue":"nats_jetstream","pubsub":"redis"}"#,
        ).unwrap();
        assert!(selection.validate().is_ok());
        assert_eq!(selection.vector, Some(VectorBackend::Qdrant));
        assert_eq!(selection.kv, None);
        let mut unsupported = selection;
        unsupported.contract_version = 2;
        assert!(matches!(
            unsupported.validate(),
            Err(StorageError::UnsupportedVersion(2))
        ));
        assert!(
            StorageScope {
                namespace: "  ".into()
            }
            .validate()
            .is_err()
        );
    }
}
