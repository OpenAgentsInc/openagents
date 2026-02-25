use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use autopilot_spacetime::client::{SpacetimeReducerErrorClass, SpacetimeReducerHttpClient};
use autopilot_spacetime::mapping::topic_to_stream_id;
use autopilot_spacetime::reducers::{
    AppendSyncEventOutcome, AppendSyncEventRequest, ReducerError, ReducerStore, SyncEvent,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

const ENV_SPACETIME_HTTP_BASE_URL: &str = "RUNTIME_SPACETIME_HTTP_BASE_URL";
const ENV_SPACETIME_DATABASE: &str = "RUNTIME_SPACETIME_DATABASE";
const ENV_SPACETIME_AUTH_TOKEN: &str = "RUNTIME_SPACETIME_AUTH_TOKEN";
const ENV_SPACETIME_OUTBOX_PATH: &str = "RUNTIME_SPACETIME_OUTBOX_PATH";
const DEFAULT_OUTBOX_PATH: &str = "output/runtime/spacetime-outbox.jsonl";
const HTTP_PUBLISH_MAX_RETRIES: u8 = 3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpacetimePublisherMetrics {
    pub published: u64,
    pub duplicates: u64,
    pub failed: u64,
    pub stream_count: u64,
    pub event_count: u64,
    pub outbox_depth: u64,
    pub auth_failures: u64,
    pub rate_limited_failures: u64,
    pub network_failures: u64,
    pub validation_failures: u64,
    pub unknown_failures: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSyncMessage {
    pub topic: String,
    pub sequence: u64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub published_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
pub struct SpacetimePublisher {
    backend: PublishBackend,
    metrics: Arc<Mutex<SpacetimePublisherMetrics>>,
    seen_streams: Arc<Mutex<HashSet<String>>>,
    outbox: Arc<Mutex<VecDeque<RuntimeSyncMessage>>>,
    outbox_path: PathBuf,
}

#[derive(Clone)]
enum PublishBackend {
    InMemory(Arc<Mutex<ReducerStore>>),
    Http(SpacetimeHttpPublisher),
}

#[derive(Clone)]
struct SpacetimeHttpPublisher {
    reducer_client: SpacetimeReducerHttpClient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishFailureClass {
    Auth,
    RateLimited,
    Network,
    Validation,
    Unknown,
}

impl SpacetimePublisher {
    #[must_use]
    pub fn new(store: Arc<Mutex<ReducerStore>>) -> Self {
        Self::with_backend(
            PublishBackend::InMemory(store),
            PathBuf::from(DEFAULT_OUTBOX_PATH),
        )
    }

    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(Arc::new(Mutex::new(ReducerStore::default())))
    }

    pub fn from_env() -> Result<Self, String> {
        let base_url = std::env::var(ENV_SPACETIME_HTTP_BASE_URL)
            .map_err(|_| format!("missing required env: {ENV_SPACETIME_HTTP_BASE_URL}"))?;
        let database = std::env::var(ENV_SPACETIME_DATABASE)
            .map_err(|_| format!("missing required env: {ENV_SPACETIME_DATABASE}"))?;
        let base_url = normalize_base_url(base_url.trim(), ENV_SPACETIME_HTTP_BASE_URL)?;
        let database = normalize_required(database.trim(), ENV_SPACETIME_DATABASE)?;
        let auth_token = std::env::var(ENV_SPACETIME_AUTH_TOKEN)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let outbox_path = std::env::var(ENV_SPACETIME_OUTBOX_PATH)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTBOX_PATH));

        let reducer_client =
            SpacetimeReducerHttpClient::new(base_url.as_str(), database.as_str(), auth_token)
                .map_err(|error| format!("spacetime reducer client init failed: {error}"))?;
        let http = SpacetimeHttpPublisher { reducer_client };

        Ok(Self::with_backend(PublishBackend::Http(http), outbox_path))
    }

    fn with_backend(backend: PublishBackend, outbox_path: PathBuf) -> Self {
        let outbox = load_outbox(&outbox_path).unwrap_or_default();
        let mut metrics = SpacetimePublisherMetrics::default();
        metrics.outbox_depth = outbox.len() as u64;

        Self {
            backend,
            metrics: Arc::new(Mutex::new(metrics)),
            seen_streams: Arc::new(Mutex::new(HashSet::new())),
            outbox: Arc::new(Mutex::new(outbox)),
            outbox_path,
        }
    }

    #[must_use]
    pub fn backend_name() -> &'static str {
        "spacetime"
    }

    pub async fn metrics(&self) -> SpacetimePublisherMetrics {
        self.metrics.lock().await.clone()
    }

    pub async fn stream_events(&self, stream_id: &str) -> Vec<SyncEvent> {
        match &self.backend {
            PublishBackend::InMemory(store) => store.lock().await.stream_events(stream_id),
            PublishBackend::Http(_) => Vec::new(),
        }
    }

    pub async fn publish(&self, message: &RuntimeSyncMessage) -> Result<(), String> {
        self.flush_outbox().await?;

        match self.try_publish_with_retry(message).await {
            Ok(outcome) => {
                self.record_publish_success(message, outcome).await;
                Ok(())
            }
            Err((class, reason)) => {
                self.record_publish_failure(class).await;
                self.enqueue_outbox(message.clone()).await;
                Err(reason)
            }
        }
    }

    async fn flush_outbox(&self) -> Result<(), String> {
        loop {
            let pending = {
                let guard = self.outbox.lock().await;
                guard.front().cloned()
            };

            let Some(message) = pending else {
                self.set_outbox_depth_metric().await;
                return Ok(());
            };

            match self.try_publish_with_retry(&message).await {
                Ok(outcome) => {
                    self.record_publish_success(&message, outcome).await;
                    let mut guard = self.outbox.lock().await;
                    let _ = guard.pop_front();
                    persist_outbox(&self.outbox_path, &guard)?;
                }
                Err((class, reason)) => {
                    self.record_publish_failure(class).await;
                    self.set_outbox_depth_metric().await;
                    return Err(format!("outbox flush failed: {reason}"));
                }
            }
        }
    }

    async fn try_publish_with_retry(
        &self,
        message: &RuntimeSyncMessage,
    ) -> Result<AppendSyncEventOutcome, (PublishFailureClass, String)> {
        let mut attempt: u8 = 0;

        loop {
            attempt = attempt.saturating_add(1);
            match self.try_publish_once(message).await {
                Ok(outcome) => return Ok(outcome),
                Err((class, reason)) => {
                    if attempt >= HTTP_PUBLISH_MAX_RETRIES
                        || class == PublishFailureClass::Validation
                    {
                        return Err((class, reason));
                    }
                    let delay_ms = u64::from(attempt).saturating_mul(150);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    async fn try_publish_once(
        &self,
        message: &RuntimeSyncMessage,
    ) -> Result<AppendSyncEventOutcome, (PublishFailureClass, String)> {
        match &self.backend {
            PublishBackend::InMemory(store) => publish_in_memory(store, message).await,
            PublishBackend::Http(http) => publish_http(http, message).await,
        }
    }

    async fn record_publish_success(
        &self,
        message: &RuntimeSyncMessage,
        outcome: AppendSyncEventOutcome,
    ) {
        let stream_id = stream_id_for_topic(message.topic.as_str());

        let mut seen_streams = self.seen_streams.lock().await;
        let is_new_stream = seen_streams.insert(stream_id);

        let mut metrics = self.metrics.lock().await;
        match outcome {
            AppendSyncEventOutcome::Applied(_) => {
                metrics.published = metrics.published.saturating_add(1);
                metrics.event_count = metrics.event_count.saturating_add(1);
                if is_new_stream {
                    metrics.stream_count = metrics.stream_count.saturating_add(1);
                }
            }
            AppendSyncEventOutcome::Duplicate(_) => {
                metrics.duplicates = metrics.duplicates.saturating_add(1);
            }
        }
    }

    async fn record_publish_failure(&self, class: PublishFailureClass) {
        let mut metrics = self.metrics.lock().await;
        metrics.failed = metrics.failed.saturating_add(1);
        match class {
            PublishFailureClass::Auth => {
                metrics.auth_failures = metrics.auth_failures.saturating_add(1)
            }
            PublishFailureClass::RateLimited => {
                metrics.rate_limited_failures = metrics.rate_limited_failures.saturating_add(1)
            }
            PublishFailureClass::Network => {
                metrics.network_failures = metrics.network_failures.saturating_add(1)
            }
            PublishFailureClass::Validation => {
                metrics.validation_failures = metrics.validation_failures.saturating_add(1)
            }
            PublishFailureClass::Unknown => {
                metrics.unknown_failures = metrics.unknown_failures.saturating_add(1)
            }
        }
    }

    async fn enqueue_outbox(&self, message: RuntimeSyncMessage) {
        let mut guard = self.outbox.lock().await;
        guard.push_back(message);
        if let Err(error) = persist_outbox(&self.outbox_path, &guard) {
            tracing::warn!(reason = %error, "failed to persist spacetime outbox");
        }
        drop(guard);
        self.set_outbox_depth_metric().await;
    }

    async fn set_outbox_depth_metric(&self) {
        let depth = self.outbox.lock().await.len() as u64;
        let mut metrics = self.metrics.lock().await;
        metrics.outbox_depth = depth;
    }
}

async fn publish_in_memory(
    store: &Arc<Mutex<ReducerStore>>,
    message: &RuntimeSyncMessage,
) -> Result<AppendSyncEventOutcome, (PublishFailureClass, String)> {
    let projected = projected_payload(message);
    let payload_json = serde_json::to_string(&projected).map_err(|error| {
        (
            PublishFailureClass::Validation,
            format!("encode payload failed: {error}"),
        )
    })?;
    let payload_hash = protocol::hash::canonical_hash(&projected).map_err(|error| {
        (
            PublishFailureClass::Validation,
            format!("hash failed: {error}"),
        )
    })?;
    let stream_id = topic_to_stream_id(message.topic.as_str());

    let request = AppendSyncEventRequest {
        stream_id,
        idempotency_key: format!(
            "topic:{}:seq:{}:kind:{}",
            message.topic, message.sequence, message.kind
        ),
        payload_hash,
        payload_bytes: payload_json.into_bytes(),
        committed_at_unix_ms: message.published_at.timestamp_millis().max(0) as u64,
        durable_offset: message.sequence,
        confirmed_read: true,
        expected_next_seq: Some(message.sequence),
    };

    let outcome = store
        .lock()
        .await
        .append_sync_event(request)
        .map_err(|error| match error {
            ReducerError::SequenceConflict { .. } => (
                PublishFailureClass::Validation,
                format!("sequence conflict while publishing in-memory event: {error:?}"),
            ),
            _ => (
                PublishFailureClass::Unknown,
                format!("in-memory append failed: {error:?}"),
            ),
        })?;

    Ok(outcome)
}

async fn publish_http(
    http: &SpacetimeHttpPublisher,
    message: &RuntimeSyncMessage,
) -> Result<AppendSyncEventOutcome, (PublishFailureClass, String)> {
    let projected = projected_payload(message);
    let payload_json = serde_json::to_string(&projected).map_err(|error| {
        (
            PublishFailureClass::Validation,
            format!("encode payload failed: {error}"),
        )
    })?;
    let payload_hash = protocol::hash::canonical_hash(&projected).map_err(|error| {
        (
            PublishFailureClass::Validation,
            format!("hash failed: {error}"),
        )
    })?;
    let stream_id = topic_to_stream_id(message.topic.as_str());
    let idempotency_key = format!(
        "topic:{}:seq:{}:kind:{}",
        message.topic, message.sequence, message.kind
    );
    let request = AppendSyncEventRequest {
        stream_id,
        idempotency_key,
        payload_hash,
        payload_bytes: payload_json.into_bytes(),
        committed_at_unix_ms: message.published_at.timestamp_millis().max(0) as u64,
        durable_offset: message.sequence,
        confirmed_read: true,
        expected_next_seq: Some(message.sequence),
    };
    http.reducer_client
        .append_sync_event(request)
        .await
        .map_err(|error| {
            (
                match error.class {
                    SpacetimeReducerErrorClass::Auth => PublishFailureClass::Auth,
                    SpacetimeReducerErrorClass::RateLimited => PublishFailureClass::RateLimited,
                    SpacetimeReducerErrorClass::Network => PublishFailureClass::Network,
                    SpacetimeReducerErrorClass::Validation => PublishFailureClass::Validation,
                    SpacetimeReducerErrorClass::Unknown => PublishFailureClass::Unknown,
                },
                error.message,
            )
        })
}

fn projected_payload(message: &RuntimeSyncMessage) -> serde_json::Value {
    json!({
        "topic": message.topic,
        "sequence": message.sequence,
        "kind": message.kind,
        "published_at": message.published_at.to_rfc3339(),
        "payload": message.payload,
    })
}

fn load_outbox(path: &PathBuf) -> Result<VecDeque<RuntimeSyncMessage>, String> {
    if !path.exists() {
        return Ok(VecDeque::new());
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read outbox {}: {error}", path.display()))?;

    let mut outbox = VecDeque::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let message = serde_json::from_str::<RuntimeSyncMessage>(trimmed).map_err(|error| {
            format!(
                "failed to parse outbox line {} in {}: {error}",
                index + 1,
                path.display()
            )
        })?;
        outbox.push_back(message);
    }

    Ok(outbox)
}

fn persist_outbox(path: &PathBuf, queue: &VecDeque<RuntimeSyncMessage>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create outbox directory {}: {error}",
                parent.display()
            )
        })?;
    }

    if queue.is_empty() {
        if path.exists() {
            fs::remove_file(path).map_err(|error| {
                format!("failed to remove empty outbox {}: {error}", path.display())
            })?;
        }
        return Ok(());
    }

    let mut buffer = String::new();
    for message in queue {
        let line = serde_json::to_string(message)
            .map_err(|error| format!("failed to serialize outbox message: {error}"))?;
        buffer.push_str(line.as_str());
        buffer.push('\n');
    }

    fs::write(path, buffer)
        .map_err(|error| format!("failed to persist outbox {}: {error}", path.display()))
}

fn normalize_required(value: &str, key: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(format!("{} must not be empty", key));
    }
    Ok(normalized)
}

fn normalize_base_url(value: &str, key: &str) -> Result<String, String> {
    let normalized = normalize_required(value, key)?;
    let parsed = reqwest::Url::parse(normalized.as_str())
        .map_err(|error| format!("invalid {}: {error}", key))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("invalid {}: unsupported scheme {}", key, scheme));
    }
    Ok(normalized.trim_end_matches('/').to_string())
}

pub fn stream_id_for_topic(topic: &str) -> String {
    topic_to_stream_id(topic)
}

#[cfg(test)]
mod tests {
    use autopilot_spacetime::client::SpacetimeReducerHttpClient;
    use autopilot_spacetime::reducers::ReducerStore;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    use crate::spacetime_publisher::{
        PublishBackend, SpacetimeHttpPublisher, RuntimeSyncMessage, SpacetimePublisher,
        normalize_base_url, stream_id_for_topic,
    };

    fn isolated_in_memory_publisher() -> SpacetimePublisher {
        let outbox_path = tempdir()
            .expect("tempdir should create")
            .keep()
            .join("runtime-outbox.jsonl");
        SpacetimePublisher::with_backend(
            PublishBackend::InMemory(Arc::new(Mutex::new(ReducerStore::default()))),
            outbox_path,
        )
    }

    fn message(topic: &str, sequence: u64) -> RuntimeSyncMessage {
        RuntimeSyncMessage {
            topic: topic.to_string(),
            sequence,
            kind: "runtime.event".to_string(),
            payload: json!({"ok": true, "sequence": sequence}),
            published_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn stream_mapping_covers_core_runtime_topics() {
        assert_eq!(
            stream_id_for_topic("run:abc123:events"),
            "runtime.run.abc123.events"
        );
        assert_eq!(
            stream_id_for_topic("worker:desktop:owner:lifecycle"),
            "runtime.worker.desktop:owner.lifecycle"
        );
        assert_eq!(
            stream_id_for_topic("fleet:user:42:workers"),
            "runtime.fleet.user.42.workers"
        );
    }

    #[tokio::test]
    async fn publisher_is_idempotent_on_duplicate_publish() {
        let publisher = isolated_in_memory_publisher();
        let first = message("run:job-1:events", 1);

        publisher
            .publish(&first)
            .await
            .expect("first publish should apply");
        publisher
            .publish(&first)
            .await
            .expect("duplicate publish should be idempotent");

        let metrics = publisher.metrics().await;
        assert_eq!(metrics.published, 1);
        assert_eq!(metrics.duplicates, 1);
        assert_eq!(metrics.failed, 0);
        assert_eq!(metrics.stream_count, 1);
        assert_eq!(metrics.event_count, 1);
    }

    #[tokio::test]
    async fn publisher_rejects_out_of_order_sequence_for_same_topic() {
        let publisher = isolated_in_memory_publisher();
        publisher
            .publish(&message("run:job-2:events", 2))
            .await
            .expect_err("out of order sequence must fail");
        let metrics = publisher.metrics().await;
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.validation_failures, 1);
        assert_eq!(metrics.stream_count, 0);
        assert_eq!(metrics.event_count, 0);
    }

    #[test]
    fn normalize_base_url_rejects_invalid_scheme() {
        let error = normalize_base_url("ftp://example.test", "RUNTIME_SPACETIME_HTTP_BASE_URL")
            .expect_err("non-http(s) scheme should fail");
        assert!(error.contains("unsupported scheme"));
    }

    #[tokio::test]
    async fn http_publish_failure_queues_outbox_for_retry() {
        let temp = tempdir().expect("tempdir should create");
        let outbox_path = temp.path().join("runtime-outbox.jsonl");
        let reducer_client = SpacetimeReducerHttpClient::new("http://127.0.0.1:9", "autopilot-dev", None)
            .expect("reducer client should initialize");
        let publisher = SpacetimePublisher::with_backend(
            PublishBackend::Http(SpacetimeHttpPublisher { reducer_client }),
            outbox_path.clone(),
        );

        let result = publisher
            .publish(&message("run:job-http-fail:events", 1))
            .await;
        assert!(result.is_err());

        let metrics = publisher.metrics().await;
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.outbox_depth, 1);
        assert!(metrics.network_failures >= 1);

        let outbox_raw = std::fs::read_to_string(outbox_path).expect("outbox file should exist");
        assert!(!outbox_raw.trim().is_empty());
    }
}
