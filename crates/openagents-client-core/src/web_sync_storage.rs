use serde::{Deserialize, Serialize};

use crate::sync_persistence::{
    PersistedSyncState, SyncPersistenceDecodeError, decode_sync_state, encode_sync_state,
};

pub const WEB_SYNC_DB_NAME: &str = "openagents.web.sync";
pub const WEB_SYNC_DB_VERSION: u32 = 2;
pub const VIEW_STATE_SCHEMA_VERSION: u32 = 1;

#[cfg(target_arch = "wasm32")]
const STORE_SYNC_STATE: &str = "sync_state";
#[cfg(target_arch = "wasm32")]
const STORE_VIEW_STATE: &str = "view_state";
#[cfg(target_arch = "wasm32")]
const STORE_META: &str = "meta";
#[cfg(target_arch = "wasm32")]
const RECORD_KEY_PRIMARY: &str = "primary";
#[cfg(target_arch = "wasm32")]
const META_KEY_VIEW_SCHEMA: &str = "view_schema_version";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedViewState {
    pub schema_version: u32,
    #[serde(default)]
    pub active_worker_id: Option<String>,
    #[serde(default)]
    pub last_seq: Option<u64>,
    pub updated_at_unix_ms: u64,
}

impl Default for PersistedViewState {
    fn default() -> Self {
        Self {
            schema_version: VIEW_STATE_SCHEMA_VERSION,
            active_worker_id: None,
            last_seq: None,
            updated_at_unix_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PersistedSyncSnapshot {
    pub sync_state: PersistedSyncState,
    pub view_state: PersistedViewState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSyncSnapshot {
    pub snapshot: PersistedSyncSnapshot,
    pub migrated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncStorageError {
    #[error("indexeddb is unavailable")]
    IndexedDbUnavailable,
    #[error("unsupported sync schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid persistence payload")]
    InvalidPayload,
    #[error("storage operation failed: {0}")]
    OperationFailed(String),
}

impl From<SyncPersistenceDecodeError> for SyncStorageError {
    fn from(error: SyncPersistenceDecodeError) -> Self {
        match error {
            SyncPersistenceDecodeError::UnsupportedSchema(version) => {
                SyncStorageError::UnsupportedSchema(version)
            }
            SyncPersistenceDecodeError::InvalidJson | SyncPersistenceDecodeError::InvalidShape => {
                SyncStorageError::InvalidPayload
            }
        }
    }
}

pub fn encode_sync_snapshot_records(
    snapshot: &PersistedSyncSnapshot,
) -> Result<(String, String), SyncStorageError> {
    let sync_payload = encode_sync_state(&snapshot.sync_state)?;
    let view_payload = encode_view_state(&snapshot.view_state)?;
    Ok((sync_payload, view_payload))
}

pub fn decode_sync_snapshot_records(
    sync_payload: &str,
    view_payload: Option<&str>,
) -> Result<LoadedSyncSnapshot, SyncStorageError> {
    let (sync_state, sync_migrated) = decode_sync_state(sync_payload)?;

    let (view_state, view_migrated) = match view_payload {
        Some(raw) => decode_view_state(raw)?,
        None => (PersistedViewState::default(), true),
    };

    Ok(LoadedSyncSnapshot {
        snapshot: PersistedSyncSnapshot {
            sync_state,
            view_state,
        },
        migrated: sync_migrated || view_migrated,
    })
}

fn encode_view_state(view_state: &PersistedViewState) -> Result<String, SyncStorageError> {
    let mut normalized = view_state.clone();
    normalized.schema_version = VIEW_STATE_SCHEMA_VERSION;
    serde_json::to_string(&normalized).map_err(|_| SyncStorageError::InvalidPayload)
}

fn decode_view_state(raw: &str) -> Result<(PersistedViewState, bool), SyncStorageError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| SyncStorageError::InvalidPayload)?;
    let object = value.as_object().ok_or(SyncStorageError::InvalidPayload)?;

    let schema_version = object
        .get("schema_version")
        .or_else(|| object.get("schemaVersion"))
        .and_then(serde_json::Value::as_u64);

    match schema_version {
        Some(version) if version == u64::from(VIEW_STATE_SCHEMA_VERSION) => {
            let mut state: PersistedViewState =
                serde_json::from_value(value).map_err(|_| SyncStorageError::InvalidPayload)?;
            state.schema_version = VIEW_STATE_SCHEMA_VERSION;
            Ok((state, false))
        }
        Some(version) => {
            let version = u32::try_from(version)
                .map_err(|_| SyncStorageError::UnsupportedSchema(u32::MAX))?;
            Err(SyncStorageError::UnsupportedSchema(version))
        }
        None => {
            #[derive(Debug, Clone, Deserialize)]
            struct LegacyViewStateV0 {
                #[serde(default)]
                active_worker_id: Option<String>,
                #[serde(default)]
                last_seq: Option<u64>,
                #[serde(default)]
                updated_at_unix_ms: u64,
            }

            let legacy: LegacyViewStateV0 =
                serde_json::from_value(value).map_err(|_| SyncStorageError::InvalidPayload)?;
            Ok((
                PersistedViewState {
                    schema_version: VIEW_STATE_SCHEMA_VERSION,
                    active_worker_id: legacy.active_worker_id,
                    last_seq: legacy.last_seq,
                    updated_at_unix_ms: legacy.updated_at_unix_ms,
                },
                true,
            ))
        }
    }
}

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Promise};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    DomException, Event, IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransaction,
    IdbTransactionMode,
};

#[cfg(target_arch = "wasm32")]
pub async fn load_sync_snapshot_from_indexeddb(
    db_name: &str,
) -> Result<Option<LoadedSyncSnapshot>, SyncStorageError> {
    let db = open_db(db_name).await?;
    let tx = open_transaction(
        &db,
        IdbTransactionMode::Readonly,
        &[STORE_SYNC_STATE, STORE_VIEW_STATE],
    )?;
    let sync_store = tx
        .object_store(STORE_SYNC_STATE)
        .map_err(js_operation_error)?;
    let view_store = tx
        .object_store(STORE_VIEW_STATE)
        .map_err(js_operation_error)?;

    let sync_payload = store_get_string(&sync_store, RECORD_KEY_PRIMARY).await?;
    let view_payload = store_get_string(&view_store, RECORD_KEY_PRIMARY).await?;
    await_transaction(tx).await?;

    let Some(sync_payload) = sync_payload else {
        return Ok(None);
    };

    decode_sync_snapshot_records(&sync_payload, view_payload.as_deref()).map(Some)
}

#[cfg(target_arch = "wasm32")]
pub async fn persist_sync_snapshot_to_indexeddb(
    db_name: &str,
    snapshot: &PersistedSyncSnapshot,
) -> Result<(), SyncStorageError> {
    let (sync_payload, view_payload) = encode_sync_snapshot_records(snapshot)?;
    let db = open_db(db_name).await?;
    let tx = open_transaction(
        &db,
        IdbTransactionMode::Readwrite,
        &[STORE_SYNC_STATE, STORE_VIEW_STATE, STORE_META],
    )?;
    let sync_store = tx
        .object_store(STORE_SYNC_STATE)
        .map_err(js_operation_error)?;
    let view_store = tx
        .object_store(STORE_VIEW_STATE)
        .map_err(js_operation_error)?;
    let meta_store = tx.object_store(STORE_META).map_err(js_operation_error)?;

    store_put_string(&sync_store, RECORD_KEY_PRIMARY, &sync_payload).await?;
    store_put_string(&view_store, RECORD_KEY_PRIMARY, &view_payload).await?;
    store_put_u32(&meta_store, META_KEY_VIEW_SCHEMA, VIEW_STATE_SCHEMA_VERSION).await?;

    await_transaction(tx).await
}

#[cfg(target_arch = "wasm32")]
pub async fn clear_sync_snapshot_in_indexeddb(db_name: &str) -> Result<(), SyncStorageError> {
    let db = open_db(db_name).await?;
    let tx = open_transaction(
        &db,
        IdbTransactionMode::Readwrite,
        &[STORE_SYNC_STATE, STORE_VIEW_STATE, STORE_META],
    )?;
    let sync_store = tx
        .object_store(STORE_SYNC_STATE)
        .map_err(js_operation_error)?;
    let view_store = tx
        .object_store(STORE_VIEW_STATE)
        .map_err(js_operation_error)?;
    let meta_store = tx.object_store(STORE_META).map_err(js_operation_error)?;

    store_delete(&sync_store, RECORD_KEY_PRIMARY).await?;
    store_delete(&view_store, RECORD_KEY_PRIMARY).await?;
    store_delete(&meta_store, META_KEY_VIEW_SCHEMA).await?;

    await_transaction(tx).await
}

#[cfg(target_arch = "wasm32")]
async fn open_db(db_name: &str) -> Result<IdbDatabase, SyncStorageError> {
    let factory = idb_factory()?;
    let open_request = factory
        .open_with_u32(db_name, WEB_SYNC_DB_VERSION)
        .map_err(js_operation_error)?;

    let upgrade_request = open_request.clone();
    let on_upgrade = Closure::wrap(Box::new(move |_event: Event| {
        if let Ok(result) = upgrade_request.result() {
            if let Ok(db) = result.dyn_into::<IdbDatabase>() {
                ensure_object_store(&db, STORE_SYNC_STATE);
                ensure_object_store(&db, STORE_VIEW_STATE);
                ensure_object_store(&db, STORE_META);
            }
        }
    }) as Box<dyn FnMut(_)>);
    open_request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    on_upgrade.forget();

    let request = open_request
        .dyn_into::<IdbRequest>()
        .map_err(|err| js_cast_error(err.into()))?;
    let result = request_result(request).await?;
    result.dyn_into::<IdbDatabase>().map_err(js_cast_error)
}

#[cfg(target_arch = "wasm32")]
fn ensure_object_store(db: &IdbDatabase, store_name: &str) {
    let _ = db.create_object_store(store_name);
}

#[cfg(target_arch = "wasm32")]
fn open_transaction(
    db: &IdbDatabase,
    mode: IdbTransactionMode,
    stores: &[&str],
) -> Result<IdbTransaction, SyncStorageError> {
    let list = Array::new();
    for store in stores {
        list.push(&JsValue::from_str(store));
    }
    db.transaction_with_str_sequence_and_mode(&list, mode)
        .map_err(js_operation_error)
}

#[cfg(target_arch = "wasm32")]
async fn store_get_string(
    store: &IdbObjectStore,
    key: &str,
) -> Result<Option<String>, SyncStorageError> {
    let request = store
        .get(&JsValue::from_str(key))
        .map_err(js_operation_error)?;
    let result = request_result(request).await?;
    if result.is_null() || result.is_undefined() {
        return Ok(None);
    }

    result
        .as_string()
        .map(Some)
        .ok_or(SyncStorageError::InvalidPayload)
}

#[cfg(target_arch = "wasm32")]
async fn store_put_string(
    store: &IdbObjectStore,
    key: &str,
    value: &str,
) -> Result<(), SyncStorageError> {
    let request = store
        .put_with_key(&JsValue::from_str(value), &JsValue::from_str(key))
        .map_err(js_operation_error)?;
    let _ = request_result(request).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn store_put_u32(
    store: &IdbObjectStore,
    key: &str,
    value: u32,
) -> Result<(), SyncStorageError> {
    let request = store
        .put_with_key(
            &JsValue::from_f64(f64::from(value)),
            &JsValue::from_str(key),
        )
        .map_err(js_operation_error)?;
    let _ = request_result(request).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn store_delete(store: &IdbObjectStore, key: &str) -> Result<(), SyncStorageError> {
    let request = store
        .delete(&JsValue::from_str(key))
        .map_err(js_operation_error)?;
    let _ = request_result(request).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn request_result(request: IdbRequest) -> Result<JsValue, SyncStorageError> {
    let promise = request_promise(&request);
    JsFuture::from(promise).await.map_err(js_operation_error)
}

#[cfg(target_arch = "wasm32")]
async fn await_transaction(tx: IdbTransaction) -> Result<(), SyncStorageError> {
    let promise = transaction_promise(&tx);
    JsFuture::from(promise).await.map_err(js_operation_error)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn request_promise(request: &IdbRequest) -> Promise {
    let request = request.clone();
    Promise::new(&mut |resolve, reject| {
        let success_request = request.clone();
        let reject_on_success = reject.clone();
        let on_success = Closure::once(move |_event: Event| match success_request.result() {
            Ok(result) => {
                let _ = resolve.call1(&JsValue::NULL, &result);
            }
            Err(err) => {
                let _ = reject_on_success
                    .call1(&JsValue::NULL, &JsValue::from_str(&js_error_message(err)));
            }
        });

        let error_request = request.clone();
        let on_error = Closure::once(move |_event: Event| {
            let message = match error_request.error() {
                Ok(Some(err)) => err.message(),
                Ok(None) => "indexeddb request error".to_string(),
                Err(err) => js_error_message(err),
            };
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&message));
        });

        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_success.forget();
        on_error.forget();
    })
}

#[cfg(target_arch = "wasm32")]
fn transaction_promise(tx: &IdbTransaction) -> Promise {
    let tx = tx.clone();
    Promise::new(&mut |resolve, reject| {
        let on_complete = Closure::once(move |_event: Event| {
            let _ = resolve.call0(&JsValue::NULL);
        });

        let reject_on_error = reject.clone();
        let on_error = Closure::once(move |_event: Event| {
            let _ = reject_on_error.call1(&JsValue::NULL, &JsValue::from_str("indexeddb tx error"));
        });

        let on_abort = Closure::once(move |_event: Event| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("indexeddb tx aborted"));
        });

        tx.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
        tx.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        tx.set_onabort(Some(on_abort.as_ref().unchecked_ref()));
        on_complete.forget();
        on_error.forget();
        on_abort.forget();
    })
}

#[cfg(target_arch = "wasm32")]
fn idb_factory() -> Result<IdbFactory, SyncStorageError> {
    let Some(window) = web_sys::window() else {
        return Err(SyncStorageError::IndexedDbUnavailable);
    };

    window
        .indexed_db()
        .map_err(js_operation_error)?
        .ok_or(SyncStorageError::IndexedDbUnavailable)
}

#[cfg(target_arch = "wasm32")]
fn js_operation_error(err: JsValue) -> SyncStorageError {
    SyncStorageError::OperationFailed(js_error_message(err))
}

#[cfg(target_arch = "wasm32")]
fn js_cast_error(err: JsValue) -> SyncStorageError {
    SyncStorageError::OperationFailed(js_error_message(err))
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(err: JsValue) -> String {
    if let Ok(dom_exception) = err.clone().dyn_into::<DomException>() {
        return dom_exception.message();
    }
    if let Some(message) = err.as_string() {
        return message;
    }
    format!("{err:?}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn decode_current_snapshot_records_round_trip() {
        let mut sync_state = PersistedSyncState::default();
        sync_state
            .topic_watermarks
            .insert("runtime.codex_worker_events".to_string(), 42);
        sync_state.subscribed_topics = vec!["runtime.codex_worker_events".to_string()];
        sync_state.updated_at_unix_ms = 111;

        let view_state = PersistedViewState {
            schema_version: VIEW_STATE_SCHEMA_VERSION,
            active_worker_id: Some("desktop:worker-1".to_string()),
            last_seq: Some(42),
            updated_at_unix_ms: 222,
        };

        let snapshot = PersistedSyncSnapshot {
            sync_state,
            view_state,
        };

        let (sync_payload, view_payload) =
            encode_sync_snapshot_records(&snapshot).expect("encoding should succeed");
        let decoded = decode_sync_snapshot_records(&sync_payload, Some(&view_payload))
            .expect("decoding should succeed");

        assert!(!decoded.migrated);
        assert_eq!(decoded.snapshot, snapshot);
    }

    #[test]
    fn decode_legacy_view_payload_migrates() {
        let mut topic_watermarks = BTreeMap::new();
        topic_watermarks.insert("runtime.codex_worker_events".to_string(), 7);

        let sync_payload = serde_json::json!({
            "schema_version": 1,
            "topic_watermarks": topic_watermarks,
            "subscribed_topics": ["runtime.codex_worker_events"],
            "updated_at_unix_ms": 5
        })
        .to_string();

        let legacy_view_payload = serde_json::json!({
            "active_worker_id": "desktop:worker-2",
            "last_seq": 7,
            "updated_at_unix_ms": 9
        })
        .to_string();

        let decoded = decode_sync_snapshot_records(&sync_payload, Some(&legacy_view_payload))
            .expect("legacy payload should decode");
        assert!(decoded.migrated);
        assert_eq!(
            decoded.snapshot.view_state.schema_version,
            VIEW_STATE_SCHEMA_VERSION
        );
        assert_eq!(
            decoded.snapshot.view_state.active_worker_id.as_deref(),
            Some("desktop:worker-2")
        );
    }

    #[test]
    fn decode_missing_view_payload_defaults_and_migrates() {
        let sync_payload = serde_json::json!({
            "schema_version": 1,
            "topic_watermarks": {},
            "subscribed_topics": [],
            "updated_at_unix_ms": 1
        })
        .to_string();

        let decoded = decode_sync_snapshot_records(&sync_payload, None)
            .expect("sync-only payload should decode");
        assert!(decoded.migrated);
        assert_eq!(decoded.snapshot.view_state, PersistedViewState::default());
    }

    #[test]
    fn decode_rejects_unsupported_sync_schema() {
        let sync_payload = serde_json::json!({
            "schema_version": 99,
            "topic_watermarks": {},
            "subscribed_topics": [],
            "updated_at_unix_ms": 1
        })
        .to_string();

        let error = decode_sync_snapshot_records(&sync_payload, None)
            .expect_err("unsupported sync schema should fail");
        assert_eq!(error, SyncStorageError::UnsupportedSchema(99));
    }

    #[test]
    fn decode_rejects_unsupported_view_schema() {
        let sync_payload = serde_json::json!({
            "schema_version": 1,
            "topic_watermarks": {},
            "subscribed_topics": [],
            "updated_at_unix_ms": 1
        })
        .to_string();

        let view_payload = serde_json::json!({
            "schema_version": 8,
            "active_worker_id": null,
            "last_seq": null,
            "updated_at_unix_ms": 2
        })
        .to_string();

        let error = decode_sync_snapshot_records(&sync_payload, Some(&view_payload))
            .expect_err("unsupported view schema should fail");
        assert_eq!(error, SyncStorageError::UnsupportedSchema(8));
    }

    #[test]
    fn decode_rejects_corrupted_payload() {
        let error = decode_sync_snapshot_records("not-json", Some("still-not-json"))
            .expect_err("corrupted payload should fail");
        assert_eq!(error, SyncStorageError::InvalidPayload);
    }
}
