use psionic_models::PromptMessage;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

const MEMORY_STORAGE: &str = "memory_only";
const MEMORY_RETENTION_SCOPE: &str = "process_lifetime";
const FILE_STORAGE: &str = "json_file";
const FILE_RETENTION_SCOPE: &str = "best_effort_local_durable";
const PROMPT_REPLAY_CACHE_BEHAVIOR: &str = "prompt_replay_only";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseStateCapability {
    pub storage: String,
    pub retention_scope: String,
    pub cache_behavior: String,
    pub continuation_modes: Vec<String>,
    pub max_responses: usize,
    pub max_conversations: usize,
    pub max_items_per_conversation: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseStateRetentionPolicy {
    pub max_responses: usize,
    pub max_conversations: usize,
    pub max_items_per_conversation: usize,
}

impl Default for ResponseStateRetentionPolicy {
    fn default() -> Self {
        Self {
            max_responses: 128,
            max_conversations: 64,
            max_items_per_conversation: 64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseConversationRef {
    pub id: String,
    pub revision: u64,
    pub item_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseStateContext {
    pub model_key: Option<String>,
    pub worker_id: Option<String>,
    pub conversation_id: Option<String>,
    pub prompt_history: Vec<PromptMessage>,
    pub previous_response_id: Option<String>,
    pub replayed_prompt_messages: usize,
    pub conversation_item_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseStateRecord {
    pub response_id: String,
    pub model_key: String,
    pub worker_id: String,
    pub conversation_id: Option<String>,
    pub prompt_history: Vec<PromptMessage>,
}

#[derive(Debug, Error)]
pub enum ResponseStateError {
    #[error("response state `{response_id}` is unknown or expired")]
    UnknownResponseState { response_id: String },
    #[error("conversation state `{conversation_id}` is unknown or expired")]
    UnknownConversationState { conversation_id: String },
    #[error(
        "response-state conversation exceeds the bounded retention limit of {max_items_per_conversation} prompt messages (got {actual_items})"
    )]
    ConversationTooLarge {
        max_items_per_conversation: usize,
        actual_items: usize,
    },
    #[error("failed to read response-state backend `{path}`: {source}")]
    IoRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write response-state backend `{path}`: {source}")]
    IoWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to deserialize response-state backend `{path}`: {source}")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize response-state backend `{path}`: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub trait ResponseStateBackend: std::fmt::Debug {
    fn capability(&self) -> ResponseStateCapability;
    fn load_context(
        &self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ResponseStateContext, ResponseStateError>;
    fn record_response(
        &mut self,
        record: ResponseStateRecord,
    ) -> Result<Option<ResponseConversationRef>, ResponseStateError>;
    fn invalidate_references(
        &mut self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<Vec<String>, ResponseStateError>;
}

#[derive(Debug)]
pub struct ResponseStateStore {
    backend: Box<dyn ResponseStateBackend + Send>,
}

impl ResponseStateStore {
    #[must_use]
    pub fn in_memory(retention: ResponseStateRetentionPolicy) -> Self {
        Self::with_backend(InMemoryResponseStateBackend::new(retention))
    }

    pub fn file_backed(
        path: impl Into<PathBuf>,
        retention: ResponseStateRetentionPolicy,
    ) -> Result<Self, ResponseStateError> {
        Ok(Self::with_backend(JsonFileResponseStateBackend::new(
            path.into(),
            retention,
        )?))
    }

    #[must_use]
    pub fn with_backend(backend: impl ResponseStateBackend + Send + 'static) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    #[must_use]
    pub fn capability(&self) -> ResponseStateCapability {
        self.backend.capability()
    }

    pub fn load_context(
        &self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ResponseStateContext, ResponseStateError> {
        self.backend
            .load_context(previous_response_id, conversation_id)
    }

    pub fn record_response(
        &mut self,
        record: ResponseStateRecord,
    ) -> Result<Option<ResponseConversationRef>, ResponseStateError> {
        self.backend.record_response(record)
    }

    pub fn invalidate_references(
        &mut self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<Vec<String>, ResponseStateError> {
        self.backend
            .invalidate_references(previous_response_id, conversation_id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredResponseState {
    response_id: String,
    model_key: String,
    worker_id: String,
    conversation_id: Option<String>,
    prompt_history: Vec<PromptMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredConversationState {
    conversation_id: String,
    model_key: String,
    worker_id: String,
    prompt_history: Vec<PromptMessage>,
    revision: u64,
    last_response_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResponseStateSnapshot {
    retention: ResponseStateRetentionPolicy,
    responses: BTreeMap<String, StoredResponseState>,
    response_order: VecDeque<String>,
    conversations: BTreeMap<String, StoredConversationState>,
    conversation_order: VecDeque<String>,
}

impl ResponseStateSnapshot {
    fn new(retention: ResponseStateRetentionPolicy) -> Self {
        Self {
            retention,
            responses: BTreeMap::new(),
            response_order: VecDeque::new(),
            conversations: BTreeMap::new(),
            conversation_order: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
struct InMemoryResponseStateBackend {
    snapshot: ResponseStateSnapshot,
    capability: ResponseStateCapability,
}

impl InMemoryResponseStateBackend {
    fn new(retention: ResponseStateRetentionPolicy) -> Self {
        Self {
            snapshot: ResponseStateSnapshot::new(retention),
            capability: capability_for_retention(MEMORY_STORAGE, MEMORY_RETENTION_SCOPE, retention),
        }
    }
}

impl ResponseStateBackend for InMemoryResponseStateBackend {
    fn capability(&self) -> ResponseStateCapability {
        self.capability.clone()
    }

    fn load_context(
        &self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ResponseStateContext, ResponseStateError> {
        load_context_from_snapshot(&self.snapshot, previous_response_id, conversation_id)
    }

    fn record_response(
        &mut self,
        record: ResponseStateRecord,
    ) -> Result<Option<ResponseConversationRef>, ResponseStateError> {
        record_response_in_snapshot(&mut self.snapshot, record)
    }

    fn invalidate_references(
        &mut self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<Vec<String>, ResponseStateError> {
        Ok(invalidate_references_in_snapshot(
            &mut self.snapshot,
            previous_response_id,
            conversation_id,
        ))
    }
}

#[derive(Debug)]
struct JsonFileResponseStateBackend {
    path: PathBuf,
    snapshot: ResponseStateSnapshot,
    capability: ResponseStateCapability,
}

impl JsonFileResponseStateBackend {
    fn new(
        path: PathBuf,
        retention: ResponseStateRetentionPolicy,
    ) -> Result<Self, ResponseStateError> {
        let mut snapshot = if path.exists() {
            load_snapshot(&path)?
        } else {
            ResponseStateSnapshot::new(retention)
        };
        snapshot.retention = retention;
        normalize_snapshot(&mut snapshot);
        trim_snapshot(&mut snapshot);
        Ok(Self {
            path,
            snapshot,
            capability: capability_for_retention(FILE_STORAGE, FILE_RETENTION_SCOPE, retention),
        })
    }

    fn mutate<R>(
        &mut self,
        apply: impl FnOnce(&mut ResponseStateSnapshot) -> Result<R, ResponseStateError>,
    ) -> Result<R, ResponseStateError> {
        let mut next = self.snapshot.clone();
        let result = apply(&mut next)?;
        persist_snapshot(&self.path, &next)?;
        self.snapshot = next;
        Ok(result)
    }
}

impl ResponseStateBackend for JsonFileResponseStateBackend {
    fn capability(&self) -> ResponseStateCapability {
        self.capability.clone()
    }

    fn load_context(
        &self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<ResponseStateContext, ResponseStateError> {
        load_context_from_snapshot(&self.snapshot, previous_response_id, conversation_id)
    }

    fn record_response(
        &mut self,
        record: ResponseStateRecord,
    ) -> Result<Option<ResponseConversationRef>, ResponseStateError> {
        self.mutate(|snapshot| record_response_in_snapshot(snapshot, record))
    }

    fn invalidate_references(
        &mut self,
        previous_response_id: Option<&str>,
        conversation_id: Option<&str>,
    ) -> Result<Vec<String>, ResponseStateError> {
        self.mutate(|snapshot| {
            Ok(invalidate_references_in_snapshot(
                snapshot,
                previous_response_id,
                conversation_id,
            ))
        })
    }
}

fn capability_for_retention(
    storage: &str,
    retention_scope: &str,
    retention: ResponseStateRetentionPolicy,
) -> ResponseStateCapability {
    ResponseStateCapability {
        storage: storage.to_string(),
        retention_scope: retention_scope.to_string(),
        cache_behavior: PROMPT_REPLAY_CACHE_BEHAVIOR.to_string(),
        continuation_modes: vec![String::from("append_turn")],
        max_responses: retention.max_responses,
        max_conversations: retention.max_conversations,
        max_items_per_conversation: retention.max_items_per_conversation,
    }
}

fn load_context_from_snapshot(
    snapshot: &ResponseStateSnapshot,
    previous_response_id: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<ResponseStateContext, ResponseStateError> {
    if let Some(previous_response_id) = previous_response_id {
        let stored = snapshot
            .responses
            .get(previous_response_id)
            .ok_or_else(|| ResponseStateError::UnknownResponseState {
                response_id: previous_response_id.to_string(),
            })?;
        return Ok(ResponseStateContext {
            model_key: Some(stored.model_key.clone()),
            worker_id: Some(stored.worker_id.clone()),
            conversation_id: stored.conversation_id.clone(),
            prompt_history: stored.prompt_history.clone(),
            previous_response_id: Some(stored.response_id.clone()),
            replayed_prompt_messages: stored.prompt_history.len(),
            conversation_item_count: stored.prompt_history.len(),
        });
    }
    if let Some(conversation_id) = conversation_id {
        let stored = snapshot.conversations.get(conversation_id).ok_or_else(|| {
            ResponseStateError::UnknownConversationState {
                conversation_id: conversation_id.to_string(),
            }
        })?;
        return Ok(ResponseStateContext {
            model_key: Some(stored.model_key.clone()),
            worker_id: Some(stored.worker_id.clone()),
            conversation_id: Some(stored.conversation_id.clone()),
            prompt_history: stored.prompt_history.clone(),
            previous_response_id: Some(stored.last_response_id.clone()),
            replayed_prompt_messages: stored.prompt_history.len(),
            conversation_item_count: stored.prompt_history.len(),
        });
    }
    Ok(ResponseStateContext::default())
}

fn record_response_in_snapshot(
    snapshot: &mut ResponseStateSnapshot,
    record: ResponseStateRecord,
) -> Result<Option<ResponseConversationRef>, ResponseStateError> {
    if record.prompt_history.len() > snapshot.retention.max_items_per_conversation {
        return Err(ResponseStateError::ConversationTooLarge {
            max_items_per_conversation: snapshot.retention.max_items_per_conversation,
            actual_items: record.prompt_history.len(),
        });
    }
    let stored_response = StoredResponseState {
        response_id: record.response_id.clone(),
        model_key: record.model_key.clone(),
        worker_id: record.worker_id.clone(),
        conversation_id: record.conversation_id.clone(),
        prompt_history: record.prompt_history.clone(),
    };
    snapshot
        .responses
        .insert(record.response_id.clone(), stored_response);
    touch_fifo_key(&mut snapshot.response_order, record.response_id.clone());
    trim_snapshot(snapshot);

    let Some(conversation_id) = record.conversation_id else {
        return Ok(None);
    };
    let revision = snapshot
        .conversations
        .get(&conversation_id)
        .map_or(1, |conversation| conversation.revision + 1);
    snapshot.conversations.insert(
        conversation_id.clone(),
        StoredConversationState {
            conversation_id: conversation_id.clone(),
            model_key: record.model_key,
            worker_id: record.worker_id,
            prompt_history: record.prompt_history,
            revision,
            last_response_id: record.response_id,
        },
    );
    touch_fifo_key(&mut snapshot.conversation_order, conversation_id.clone());
    trim_snapshot(snapshot);
    let stored = snapshot
        .conversations
        .get(&conversation_id)
        .expect("stored conversation should exist");
    Ok(Some(ResponseConversationRef {
        id: conversation_id,
        revision: stored.revision,
        item_count: stored.prompt_history.len(),
    }))
}

fn invalidate_references_in_snapshot(
    snapshot: &mut ResponseStateSnapshot,
    previous_response_id: Option<&str>,
    conversation_id: Option<&str>,
) -> Vec<String> {
    let mut invalidated = Vec::new();
    if let Some(previous_response_id) = previous_response_id
        && snapshot.responses.remove(previous_response_id).is_some()
    {
        snapshot
            .response_order
            .retain(|candidate| candidate != previous_response_id);
        invalidated.push(format!("response:{previous_response_id}"));
    }
    if let Some(conversation_id) = conversation_id
        && snapshot.conversations.remove(conversation_id).is_some()
    {
        snapshot
            .conversation_order
            .retain(|candidate| candidate != conversation_id);
        invalidated.push(format!("conversation:{conversation_id}"));
    }
    invalidated
}

fn trim_snapshot(snapshot: &mut ResponseStateSnapshot) {
    drop_oversized_entries(snapshot);
    while snapshot.responses.len() > snapshot.retention.max_responses {
        if let Some(evicted) = snapshot.response_order.pop_front() {
            snapshot.responses.remove(&evicted);
        }
    }
    while snapshot.conversations.len() > snapshot.retention.max_conversations {
        if let Some(evicted) = snapshot.conversation_order.pop_front() {
            snapshot.conversations.remove(&evicted);
        }
    }
    normalize_snapshot(snapshot);
}

fn drop_oversized_entries(snapshot: &mut ResponseStateSnapshot) {
    let max_items = snapshot.retention.max_items_per_conversation;
    snapshot
        .responses
        .retain(|_, state| state.prompt_history.len() <= max_items);
    snapshot
        .conversations
        .retain(|_, state| state.prompt_history.len() <= max_items);
}

fn normalize_snapshot(snapshot: &mut ResponseStateSnapshot) {
    normalize_order(&mut snapshot.response_order, snapshot.responses.keys());
    normalize_order(
        &mut snapshot.conversation_order,
        snapshot.conversations.keys(),
    );
}

fn normalize_order<'a>(order: &mut VecDeque<String>, keys: impl Iterator<Item = &'a String>) {
    let all_keys = keys.cloned().collect::<Vec<_>>();
    order.retain(|candidate| all_keys.contains(candidate));
    for key in all_keys {
        if !order.iter().any(|candidate| candidate == &key) {
            order.push_back(key);
        }
    }
}

fn touch_fifo_key(order: &mut VecDeque<String>, key: String) {
    order.retain(|candidate| candidate != &key);
    order.push_back(key);
}

fn load_snapshot(path: &Path) -> Result<ResponseStateSnapshot, ResponseStateError> {
    let raw = fs::read_to_string(path).map_err(|source| ResponseStateError::IoRead {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(raw.as_str()).map_err(|source| ResponseStateError::Deserialize {
        path: path.to_path_buf(),
        source,
    })
}

fn persist_snapshot(
    path: &Path,
    snapshot: &ResponseStateSnapshot,
) -> Result<(), ResponseStateError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| ResponseStateError::IoWrite {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let payload =
        serde_json::to_vec_pretty(snapshot).map_err(|source| ResponseStateError::Serialize {
            path: path.to_path_buf(),
            source,
        })?;
    let temp_path = temp_snapshot_path(path);
    fs::write(&temp_path, payload).map_err(|source| ResponseStateError::IoWrite {
        path: temp_path.clone(),
        source,
    })?;
    fs::rename(&temp_path, path).map_err(|source| ResponseStateError::IoWrite {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn temp_snapshot_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("response-state.json");
    path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::{
        ResponseStateContext, ResponseStateError, ResponseStateRecord,
        ResponseStateRetentionPolicy, ResponseStateStore,
    };
    use psionic_models::{PromptMessage, PromptMessageRole};

    #[test]
    fn in_memory_response_state_roundtrips_and_invalidates()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut store = ResponseStateStore::in_memory(ResponseStateRetentionPolicy {
            max_responses: 4,
            max_conversations: 4,
            max_items_per_conversation: 8,
        });
        let capability = store.capability();
        assert_eq!(capability.storage, "memory_only");
        assert_eq!(capability.retention_scope, "process_lifetime");

        let conversation = store.record_response(ResponseStateRecord {
            response_id: String::from("resp-1"),
            model_key: String::from("tiny-llama"),
            worker_id: String::from("worker-a"),
            conversation_id: Some(String::from("conv-1")),
            prompt_history: vec![
                PromptMessage::new(PromptMessageRole::User, "hello"),
                PromptMessage::new(PromptMessageRole::Assistant, "world"),
            ],
        })?;
        assert_eq!(conversation.expect("conversation should exist").revision, 1);

        let context = store.load_context(Some("resp-1"), None)?;
        assert_eq!(
            context,
            ResponseStateContext {
                model_key: Some(String::from("tiny-llama")),
                worker_id: Some(String::from("worker-a")),
                conversation_id: Some(String::from("conv-1")),
                prompt_history: vec![
                    PromptMessage::new(PromptMessageRole::User, "hello"),
                    PromptMessage::new(PromptMessageRole::Assistant, "world"),
                ],
                previous_response_id: Some(String::from("resp-1")),
                replayed_prompt_messages: 2,
                conversation_item_count: 2,
            }
        );

        let invalidated = store.invalidate_references(Some("resp-1"), Some("conv-1"))?;
        assert_eq!(
            invalidated,
            vec![
                String::from("response:resp-1"),
                String::from("conversation:conv-1")
            ]
        );
        assert!(matches!(
            store.load_context(Some("resp-1"), None),
            Err(ResponseStateError::UnknownResponseState { .. })
        ));
        Ok(())
    }

    #[test]
    fn file_backed_response_state_survives_restart() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("response-state.json");
        let retention = ResponseStateRetentionPolicy {
            max_responses: 4,
            max_conversations: 4,
            max_items_per_conversation: 8,
        };

        {
            let mut store = ResponseStateStore::file_backed(&path, retention)?;
            let capability = store.capability();
            assert_eq!(capability.storage, "json_file");
            assert_eq!(capability.retention_scope, "best_effort_local_durable");
            let _ = store.record_response(ResponseStateRecord {
                response_id: String::from("resp-1"),
                model_key: String::from("tiny-llama"),
                worker_id: String::from("worker-a"),
                conversation_id: Some(String::from("conv-1")),
                prompt_history: vec![
                    PromptMessage::new(PromptMessageRole::User, "hello"),
                    PromptMessage::new(PromptMessageRole::Assistant, "world"),
                ],
            })?;
        }

        let store = ResponseStateStore::file_backed(&path, retention)?;
        let context = store.load_context(None, Some("conv-1"))?;
        assert_eq!(context.previous_response_id.as_deref(), Some("resp-1"));
        assert_eq!(context.worker_id.as_deref(), Some("worker-a"));
        assert_eq!(context.prompt_history.len(), 2);
        Ok(())
    }

    #[test]
    fn file_backed_response_state_trims_to_retention() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("response-state.json");
        let retention = ResponseStateRetentionPolicy {
            max_responses: 1,
            max_conversations: 1,
            max_items_per_conversation: 4,
        };
        let mut store = ResponseStateStore::file_backed(&path, retention)?;
        for index in 0..2 {
            let _ = store.record_response(ResponseStateRecord {
                response_id: format!("resp-{index}"),
                model_key: String::from("tiny-llama"),
                worker_id: String::from("worker-a"),
                conversation_id: Some(format!("conv-{index}")),
                prompt_history: vec![
                    PromptMessage::new(PromptMessageRole::User, format!("hello-{index}")),
                    PromptMessage::new(PromptMessageRole::Assistant, "world"),
                ],
            })?;
        }
        drop(store);

        let store = ResponseStateStore::file_backed(&path, retention)?;
        assert!(matches!(
            store.load_context(Some("resp-0"), None),
            Err(ResponseStateError::UnknownResponseState { .. })
        ));
        assert!(matches!(
            store.load_context(None, Some("conv-0")),
            Err(ResponseStateError::UnknownConversationState { .. })
        ));
        assert_eq!(
            store
                .load_context(Some("resp-1"), None)?
                .previous_response_id
                .as_deref(),
            Some("resp-1")
        );
        Ok(())
    }
}
