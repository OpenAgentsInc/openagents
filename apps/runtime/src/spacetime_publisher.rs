use std::sync::Arc;

use autopilot_spacetime::mapping::topic_to_stream_id;
use autopilot_spacetime::reducers::{
    AppendSyncEventOutcome, AppendSyncEventRequest, ReducerError, ReducerStore, SyncEvent,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpacetimePublisherMetrics {
    pub published: u64,
    pub duplicates: u64,
    pub failed: u64,
    pub stream_count: u64,
    pub event_count: u64,
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
    store: Arc<Mutex<ReducerStore>>,
    metrics: Arc<Mutex<SpacetimePublisherMetrics>>,
}

impl SpacetimePublisher {
    #[must_use]
    pub fn new(store: Arc<Mutex<ReducerStore>>) -> Self {
        Self {
            store,
            metrics: Arc::new(Mutex::new(SpacetimePublisherMetrics::default())),
        }
    }

    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(Arc::new(Mutex::new(ReducerStore::default())))
    }

    #[must_use]
    pub fn backend_name() -> &'static str {
        "spacetime"
    }

    pub async fn metrics(&self) -> SpacetimePublisherMetrics {
        self.metrics.lock().await.clone()
    }

    pub async fn stream_events(&self, stream_id: &str) -> Vec<SyncEvent> {
        self.store.lock().await.stream_events(stream_id)
    }

    pub async fn publish(&self, message: &RuntimeSyncMessage) -> Result<(), String> {
        let payload = projected_payload(message);
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("encode payload failed: {error}"))?;
        let payload_hash = protocol::hash::canonical_hash(&payload)
            .map_err(|error| format!("hash failed: {error}"))?;
        let stream_id = topic_to_stream_id(message.topic.as_str());
        let idempotency_key = format!(
            "topic:{}:seq:{}:kind:{}",
            message.topic, message.sequence, message.kind
        );

        let request = AppendSyncEventRequest {
            stream_id: stream_id.clone(),
            idempotency_key,
            payload_hash: payload_hash.clone(),
            payload_bytes: payload_bytes.clone(),
            committed_at_unix_ms: message.published_at.timestamp_millis().max(0) as u64,
            durable_offset: message.sequence,
            confirmed_read: true,
            expected_next_seq: Some(message.sequence),
        };

        let mut attempts = 0_u8;
        loop {
            attempts = attempts.saturating_add(1);
            let outcome = self.store.lock().await.append_sync_event(request.clone());
            match outcome {
                Ok(AppendSyncEventOutcome::Applied(event)) => {
                    if !parity_matches(message, &payload_hash, &payload_bytes, &stream_id, &event) {
                        let mut metrics = self.metrics.lock().await;
                        metrics.failed = metrics.failed.saturating_add(1);
                        return Err("spacetime parity mismatch".to_string());
                    }
                    let mut metrics = self.metrics.lock().await;
                    metrics.published = metrics.published.saturating_add(1);
                    metrics.event_count = metrics.event_count.saturating_add(1);
                    if event.seq == 1 {
                        metrics.stream_count = metrics.stream_count.saturating_add(1);
                    }
                    return Ok(());
                }
                Ok(AppendSyncEventOutcome::Duplicate(_event)) => {
                    let mut metrics = self.metrics.lock().await;
                    metrics.duplicates = metrics.duplicates.saturating_add(1);
                    return Ok(());
                }
                Err(ReducerError::SequenceConflict { .. }) if attempts < 3 => {
                    tokio::time::sleep(std::time::Duration::from_millis(10 * u64::from(attempts)))
                        .await;
                    continue;
                }
                Err(error) => {
                    let mut metrics = self.metrics.lock().await;
                    metrics.failed = metrics.failed.saturating_add(1);
                    return Err(format!("append failed: {error:?}"));
                }
            }
        }
    }
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

fn parity_matches(
    message: &RuntimeSyncMessage,
    payload_hash: &str,
    payload_bytes: &[u8],
    stream_id: &str,
    event: &SyncEvent,
) -> bool {
    !payload_hash.is_empty()
        && event.seq == message.sequence
        && event.stream_id == stream_id
        && event.payload_hash == payload_hash
        && event.payload_bytes == payload_bytes
        && event.durable_offset == message.sequence
        && event.confirmed_read
}

pub fn stream_id_for_topic(topic: &str) -> String {
    topic_to_stream_id(topic)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use crate::spacetime_publisher::{RuntimeSyncMessage, SpacetimePublisher, stream_id_for_topic};

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
        let publisher = SpacetimePublisher::in_memory();
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

        let stream_id = stream_id_for_topic("run:job-1:events");
        let stored = publisher.stream_events(stream_id.as_str()).await;
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].seq, 1);
        assert_eq!(stored[0].durable_offset, 1);
        assert!(stored[0].confirmed_read);
    }

    #[tokio::test]
    async fn publisher_rejects_out_of_order_sequence_for_same_topic() {
        let publisher = SpacetimePublisher::in_memory();
        publisher
            .publish(&message("run:job-2:events", 2))
            .await
            .expect_err("out of order sequence must fail");
        let metrics = publisher.metrics().await;
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.stream_count, 0);
        assert_eq!(metrics.event_count, 0);
    }
}
