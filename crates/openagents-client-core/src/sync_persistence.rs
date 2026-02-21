use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SYNC_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSyncState {
    pub schema_version: u32,
    #[serde(default)]
    pub topic_watermarks: BTreeMap<String, u64>,
    #[serde(default)]
    pub subscribed_topics: Vec<String>,
    pub updated_at_unix_ms: u64,
}

impl Default for PersistedSyncState {
    fn default() -> Self {
        Self {
            schema_version: SYNC_STATE_SCHEMA_VERSION,
            topic_watermarks: BTreeMap::new(),
            subscribed_topics: Vec::new(),
            updated_at_unix_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncPersistenceDecodeError {
    #[error("invalid JSON payload")]
    InvalidJson,
    #[error("unsupported sync schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid sync state shape")]
    InvalidShape,
}

#[derive(Debug, Clone, Deserialize)]
struct PersistedSyncStateLegacyV0 {
    #[serde(default)]
    topic_watermarks: BTreeMap<String, u64>,
    #[serde(default)]
    subscribed_topics: Vec<String>,
}

pub fn decode_sync_state(
    raw: &str,
) -> Result<(PersistedSyncState, bool), SyncPersistenceDecodeError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| SyncPersistenceDecodeError::InvalidJson)?;
    let object = value
        .as_object()
        .ok_or(SyncPersistenceDecodeError::InvalidShape)?;

    let schema_version = object
        .get("schema_version")
        .or_else(|| object.get("schemaVersion"))
        .and_then(serde_json::Value::as_u64);

    match schema_version {
        Some(version) if version == u64::from(SYNC_STATE_SCHEMA_VERSION) => {
            let state: PersistedSyncState = serde_json::from_value(value)
                .map_err(|_| SyncPersistenceDecodeError::InvalidShape)?;
            Ok((normalize_state(state), false))
        }
        Some(version) => {
            let converted = u32::try_from(version)
                .map_err(|_| SyncPersistenceDecodeError::UnsupportedSchema(u32::MAX))?;
            Err(SyncPersistenceDecodeError::UnsupportedSchema(converted))
        }
        None => {
            let legacy: PersistedSyncStateLegacyV0 = serde_json::from_value(value)
                .map_err(|_| SyncPersistenceDecodeError::InvalidShape)?;
            let migrated = PersistedSyncState {
                schema_version: SYNC_STATE_SCHEMA_VERSION,
                topic_watermarks: legacy.topic_watermarks,
                subscribed_topics: normalize_topics(legacy.subscribed_topics),
                updated_at_unix_ms: 0,
            };
            Ok((normalize_state(migrated), true))
        }
    }
}

pub fn encode_sync_state(state: &PersistedSyncState) -> Result<String, SyncPersistenceDecodeError> {
    serde_json::to_string(&normalize_state(state.clone()))
        .map_err(|_| SyncPersistenceDecodeError::InvalidShape)
}

pub fn normalized_topics(topics: Vec<String>) -> Vec<String> {
    normalize_topics(topics)
}

pub fn resume_after_map(
    topics: &[String],
    topic_watermarks: &BTreeMap<String, u64>,
) -> BTreeMap<String, u64> {
    topics
        .iter()
        .map(|topic| {
            (
                topic.clone(),
                topic_watermarks.get(topic).copied().unwrap_or(0),
            )
        })
        .collect()
}

fn normalize_state(mut state: PersistedSyncState) -> PersistedSyncState {
    state.schema_version = SYNC_STATE_SCHEMA_VERSION;
    state.subscribed_topics = normalize_topics(state.subscribed_topics);
    state
}

fn normalize_topics(topics: Vec<String>) -> Vec<String> {
    let mut normalized = topics
        .into_iter()
        .map(|topic| topic.trim().to_string())
        .filter(|topic| !topic.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_current_schema_roundtrip() {
        let mut state = PersistedSyncState::default();
        state
            .topic_watermarks
            .insert("runtime.codex_worker_events".to_string(), 77);
        state.subscribed_topics = vec![
            "runtime.codex_worker_events".to_string(),
            "runtime.codex_worker_events".to_string(),
        ];
        state.updated_at_unix_ms = 1234;

        let encoded = encode_sync_state(&state).expect("state should encode");
        let (decoded, migrated) = decode_sync_state(&encoded).expect("state should decode");
        assert!(!migrated);
        assert_eq!(decoded.schema_version, SYNC_STATE_SCHEMA_VERSION);
        assert_eq!(decoded.topic_watermarks["runtime.codex_worker_events"], 77);
        assert_eq!(
            decoded.subscribed_topics,
            vec!["runtime.codex_worker_events".to_string()]
        );
    }

    #[test]
    fn decode_legacy_schema_and_migrate() {
        let legacy = r#"{"topic_watermarks":{"runtime.codex_worker_events":19},"subscribed_topics":["runtime.codex_worker_events"]}"#;
        let (decoded, migrated) = decode_sync_state(legacy).expect("legacy should decode");
        assert!(migrated);
        assert_eq!(decoded.schema_version, SYNC_STATE_SCHEMA_VERSION);
        assert_eq!(decoded.topic_watermarks["runtime.codex_worker_events"], 19);
    }

    #[test]
    fn decode_invalid_payload_fails() {
        let error = decode_sync_state("this-is-not-json").expect_err("invalid payload must fail");
        assert_eq!(error, SyncPersistenceDecodeError::InvalidJson);
    }

    #[test]
    fn decode_unsupported_schema_fails() {
        let payload = r#"{"schema_version":9,"topic_watermarks":{"runtime.codex_worker_events":1},"subscribed_topics":[],"updated_at_unix_ms":0}"#;
        let error = decode_sync_state(payload).expect_err("unsupported schema must fail");
        assert_eq!(error, SyncPersistenceDecodeError::UnsupportedSchema(9));
    }

    #[test]
    fn normalized_topics_trim_and_dedupe() {
        let topics = normalized_topics(vec![
            " runtime.codex_worker_events ".to_string(),
            "runtime.codex_worker_events".to_string(),
            String::new(),
        ]);
        assert_eq!(topics, vec!["runtime.codex_worker_events".to_string()]);
    }

    #[test]
    fn resume_after_map_uses_persisted_watermarks() {
        let mut watermarks = BTreeMap::new();
        watermarks.insert("runtime.codex_worker_events".to_string(), 44);
        let topics = vec![
            "runtime.codex_worker_events".to_string(),
            "runtime.other_topic".to_string(),
        ];

        let resume_after = resume_after_map(&topics, &watermarks);
        assert_eq!(resume_after["runtime.codex_worker_events"], 44);
        assert_eq!(resume_after["runtime.other_topic"], 0);
    }
}
