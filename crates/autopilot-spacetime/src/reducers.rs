//! Reducer primitives for Spacetime sync state management.

use std::collections::HashMap;

/// Reducer-layer error taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReducerError {
    EmptyField(&'static str),
    AssignmentMissing(String),
    BridgeEventMissing(String),
}

/// Core sync event row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncEvent {
    pub stream_id: String,
    pub seq: u64,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub payload_bytes: Vec<u8>,
    pub committed_at_unix_ms: u64,
    pub confirmed_read: bool,
}

/// Checkpoint row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCheckpoint {
    pub client_id: String,
    pub stream_id: String,
    pub last_applied_seq: u64,
    pub durable_offset: u64,
    pub updated_at_unix_ms: u64,
}

/// Presence row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPresence {
    pub node_id: String,
    pub session_id: String,
    pub status: String,
    pub region: String,
    pub last_seen_unix_ms: u64,
}

/// Provider capability row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapability {
    pub provider_id: String,
    pub capability_json: String,
    pub price_hint_sats: u64,
    pub updated_at_unix_ms: u64,
}

/// Compute assignment row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeAssignment {
    pub request_id: String,
    pub provider_id: String,
    pub status: String,
    pub assignment_json: String,
    pub updated_at_unix_ms: u64,
}

/// Bridge outbox status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeOutboxStatus {
    Pending,
    Sent,
}

/// Bridge outbox row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeOutboxEvent {
    pub event_id: String,
    pub transport: String,
    pub status: BridgeOutboxStatus,
    pub payload_json: String,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

/// Append-event reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendSyncEventRequest {
    pub stream_id: String,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub payload_bytes: Vec<u8>,
    pub committed_at_unix_ms: u64,
    pub confirmed_read: bool,
}

/// Checkpoint reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckCheckpointRequest {
    pub client_id: String,
    pub stream_id: String,
    pub last_applied_seq: u64,
    pub durable_offset: u64,
    pub updated_at_unix_ms: u64,
}

/// Presence reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertPresenceRequest {
    pub node_id: String,
    pub session_id: String,
    pub status: String,
    pub region: String,
    pub last_seen_unix_ms: u64,
}

/// Capability reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishProviderCapabilityRequest {
    pub provider_id: String,
    pub capability_json: String,
    pub price_hint_sats: u64,
    pub updated_at_unix_ms: u64,
}

/// Open assignment reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenComputeAssignmentRequest {
    pub request_id: String,
    pub provider_id: String,
    pub assignment_json: String,
    pub updated_at_unix_ms: u64,
}

/// Update assignment reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateComputeAssignmentRequest {
    pub request_id: String,
    pub status: String,
    pub assignment_json: String,
    pub updated_at_unix_ms: u64,
}

/// Bridge enqueue reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnqueueBridgeEventRequest {
    pub event_id: String,
    pub transport: String,
    pub payload_json: String,
    pub created_at_unix_ms: u64,
}

/// Bridge mark-sent reducer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkBridgeEventSentRequest {
    pub event_id: String,
    pub updated_at_unix_ms: u64,
}

/// In-memory reducer store used by integration and replay tests.
#[derive(Debug, Default, Clone)]
pub struct ReducerStore {
    stream_heads: HashMap<String, u64>,
    stream_events: HashMap<String, Vec<SyncEvent>>,
    checkpoints: HashMap<(String, String), SyncCheckpoint>,
    session_presence: HashMap<String, SessionPresence>,
    provider_capabilities: HashMap<String, ProviderCapability>,
    compute_assignments: HashMap<String, ComputeAssignment>,
    bridge_outbox: HashMap<String, BridgeOutboxEvent>,
}

impl ReducerStore {
    /// Appends an event to a stream with monotonic sequence assignment.
    pub fn append_sync_event(
        &mut self,
        request: AppendSyncEventRequest,
    ) -> Result<SyncEvent, ReducerError> {
        require_non_empty(&request.stream_id, "stream_id")?;
        require_non_empty(&request.idempotency_key, "idempotency_key")?;
        require_non_empty(&request.payload_hash, "payload_hash")?;

        let next_seq = self
            .stream_heads
            .get(request.stream_id.as_str())
            .copied()
            .unwrap_or(0)
            + 1;
        self.stream_heads.insert(request.stream_id.clone(), next_seq);

        let event = SyncEvent {
            stream_id: request.stream_id.clone(),
            seq: next_seq,
            idempotency_key: request.idempotency_key,
            payload_hash: request.payload_hash,
            payload_bytes: request.payload_bytes,
            committed_at_unix_ms: request.committed_at_unix_ms,
            confirmed_read: request.confirmed_read,
        };

        self.stream_events
            .entry(request.stream_id)
            .or_default()
            .push(event.clone());
        Ok(event)
    }

    /// Upserts a client stream checkpoint.
    pub fn ack_checkpoint(
        &mut self,
        request: AckCheckpointRequest,
    ) -> Result<SyncCheckpoint, ReducerError> {
        require_non_empty(&request.client_id, "client_id")?;
        require_non_empty(&request.stream_id, "stream_id")?;

        let checkpoint = SyncCheckpoint {
            client_id: request.client_id.clone(),
            stream_id: request.stream_id.clone(),
            last_applied_seq: request.last_applied_seq,
            durable_offset: request.durable_offset,
            updated_at_unix_ms: request.updated_at_unix_ms,
        };
        self.checkpoints.insert(
            (request.client_id, request.stream_id),
            checkpoint.clone(),
        );
        Ok(checkpoint)
    }

    /// Upserts node/session presence.
    pub fn upsert_presence(
        &mut self,
        request: UpsertPresenceRequest,
    ) -> Result<SessionPresence, ReducerError> {
        require_non_empty(&request.node_id, "node_id")?;
        require_non_empty(&request.session_id, "session_id")?;
        require_non_empty(&request.status, "status")?;
        require_non_empty(&request.region, "region")?;

        let presence = SessionPresence {
            node_id: request.node_id.clone(),
            session_id: request.session_id,
            status: request.status,
            region: request.region,
            last_seen_unix_ms: request.last_seen_unix_ms,
        };
        self.session_presence
            .insert(request.node_id, presence.clone());
        Ok(presence)
    }

    /// Upserts provider capability metadata.
    pub fn publish_provider_capability(
        &mut self,
        request: PublishProviderCapabilityRequest,
    ) -> Result<ProviderCapability, ReducerError> {
        require_non_empty(&request.provider_id, "provider_id")?;
        require_non_empty(&request.capability_json, "capability_json")?;

        let capability = ProviderCapability {
            provider_id: request.provider_id.clone(),
            capability_json: request.capability_json,
            price_hint_sats: request.price_hint_sats,
            updated_at_unix_ms: request.updated_at_unix_ms,
        };
        self.provider_capabilities
            .insert(request.provider_id, capability.clone());
        Ok(capability)
    }

    /// Opens or replaces a compute assignment for request ID.
    pub fn open_compute_assignment(
        &mut self,
        request: OpenComputeAssignmentRequest,
    ) -> Result<ComputeAssignment, ReducerError> {
        require_non_empty(&request.request_id, "request_id")?;
        require_non_empty(&request.provider_id, "provider_id")?;
        require_non_empty(&request.assignment_json, "assignment_json")?;

        let assignment = ComputeAssignment {
            request_id: request.request_id.clone(),
            provider_id: request.provider_id,
            status: "open".to_string(),
            assignment_json: request.assignment_json,
            updated_at_unix_ms: request.updated_at_unix_ms,
        };
        self.compute_assignments
            .insert(request.request_id, assignment.clone());
        Ok(assignment)
    }

    /// Updates an existing compute assignment status/payload.
    pub fn update_compute_assignment(
        &mut self,
        request: UpdateComputeAssignmentRequest,
    ) -> Result<ComputeAssignment, ReducerError> {
        require_non_empty(&request.request_id, "request_id")?;
        require_non_empty(&request.status, "status")?;
        require_non_empty(&request.assignment_json, "assignment_json")?;

        let Some(existing) = self.compute_assignments.get(request.request_id.as_str()) else {
            return Err(ReducerError::AssignmentMissing(request.request_id));
        };

        let assignment = ComputeAssignment {
            request_id: existing.request_id.clone(),
            provider_id: existing.provider_id.clone(),
            status: request.status,
            assignment_json: request.assignment_json,
            updated_at_unix_ms: request.updated_at_unix_ms,
        };
        self.compute_assignments
            .insert(assignment.request_id.clone(), assignment.clone());
        Ok(assignment)
    }

    /// Enqueues a bridge outbox event.
    pub fn enqueue_bridge_event(
        &mut self,
        request: EnqueueBridgeEventRequest,
    ) -> Result<BridgeOutboxEvent, ReducerError> {
        require_non_empty(&request.event_id, "event_id")?;
        require_non_empty(&request.transport, "transport")?;
        require_non_empty(&request.payload_json, "payload_json")?;

        let event = BridgeOutboxEvent {
            event_id: request.event_id.clone(),
            transport: request.transport,
            status: BridgeOutboxStatus::Pending,
            payload_json: request.payload_json,
            created_at_unix_ms: request.created_at_unix_ms,
            updated_at_unix_ms: request.created_at_unix_ms,
        };
        self.bridge_outbox.insert(request.event_id, event.clone());
        Ok(event)
    }

    /// Marks an outbox event as sent.
    pub fn mark_bridge_event_sent(
        &mut self,
        request: MarkBridgeEventSentRequest,
    ) -> Result<BridgeOutboxEvent, ReducerError> {
        require_non_empty(&request.event_id, "event_id")?;

        let Some(existing) = self.bridge_outbox.get(request.event_id.as_str()) else {
            return Err(ReducerError::BridgeEventMissing(request.event_id));
        };

        let event = BridgeOutboxEvent {
            event_id: existing.event_id.clone(),
            transport: existing.transport.clone(),
            status: BridgeOutboxStatus::Sent,
            payload_json: existing.payload_json.clone(),
            created_at_unix_ms: existing.created_at_unix_ms,
            updated_at_unix_ms: request.updated_at_unix_ms,
        };
        self.bridge_outbox.insert(event.event_id.clone(), event.clone());
        Ok(event)
    }

    /// Returns stream events for a stream ID.
    pub fn stream_events(&self, stream_id: &str) -> Vec<SyncEvent> {
        self.stream_events
            .get(stream_id)
            .cloned()
            .unwrap_or_default()
    }
}

fn require_non_empty(value: &str, field: &'static str) -> Result<(), ReducerError> {
    if value.trim().is_empty() {
        return Err(ReducerError::EmptyField(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AckCheckpointRequest, AppendSyncEventRequest, BridgeOutboxStatus, EnqueueBridgeEventRequest,
        MarkBridgeEventSentRequest, OpenComputeAssignmentRequest,
        PublishProviderCapabilityRequest, ReducerStore, UpdateComputeAssignmentRequest,
        UpsertPresenceRequest,
    };

    #[test]
    fn append_sync_event_assigns_monotonic_sequence() {
        let mut store = ReducerStore::default();
        let first = store.append_sync_event(AppendSyncEventRequest {
            stream_id: "runtime.codex.worker.worker_1".to_string(),
            idempotency_key: "idem-1".to_string(),
            payload_hash: "sha256:a".to_string(),
            payload_bytes: vec![1],
            committed_at_unix_ms: 10,
            confirmed_read: false,
        });
        assert!(first.is_ok());
        let first = match first {
            Ok(value) => value,
            Err(_) => {
                assert!(false, "first append should succeed");
                return;
            }
        };
        assert_eq!(first.seq, 1);

        let second = store.append_sync_event(AppendSyncEventRequest {
            stream_id: "runtime.codex.worker.worker_1".to_string(),
            idempotency_key: "idem-2".to_string(),
            payload_hash: "sha256:b".to_string(),
            payload_bytes: vec![2],
            committed_at_unix_ms: 20,
            confirmed_read: true,
        });
        assert!(second.is_ok());
        let second = match second {
            Ok(value) => value,
            Err(_) => {
                assert!(false, "second append should succeed");
                return;
            }
        };
        assert_eq!(second.seq, 2);
    }

    #[test]
    fn checkpoint_presence_capability_reducers_upsert_rows() {
        let mut store = ReducerStore::default();

        let checkpoint = store.ack_checkpoint(AckCheckpointRequest {
            client_id: "client_1".to_string(),
            stream_id: "runtime.codex.worker.worker_1".to_string(),
            last_applied_seq: 99,
            durable_offset: 101,
            updated_at_unix_ms: 555,
        });
        assert!(checkpoint.is_ok());

        let presence = store.upsert_presence(UpsertPresenceRequest {
            node_id: "node_1".to_string(),
            session_id: "sess_1".to_string(),
            status: "online".to_string(),
            region: "us-central1".to_string(),
            last_seen_unix_ms: 777,
        });
        assert!(presence.is_ok());

        let capability = store.publish_provider_capability(PublishProviderCapabilityRequest {
            provider_id: "prov_1".to_string(),
            capability_json: "{\"jobs\":[\"oa.sandbox_run.v1\"]}".to_string(),
            price_hint_sats: 200,
            updated_at_unix_ms: 888,
        });
        assert!(capability.is_ok());
    }

    #[test]
    fn compute_assignment_open_and_update_flow() {
        let mut store = ReducerStore::default();

        let opened = store.open_compute_assignment(OpenComputeAssignmentRequest {
            request_id: "req_1".to_string(),
            provider_id: "prov_1".to_string(),
            assignment_json: "{\"status\":\"open\"}".to_string(),
            updated_at_unix_ms: 100,
        });
        assert!(opened.is_ok());

        let updated = store.update_compute_assignment(UpdateComputeAssignmentRequest {
            request_id: "req_1".to_string(),
            status: "accepted".to_string(),
            assignment_json: "{\"status\":\"accepted\"}".to_string(),
            updated_at_unix_ms: 200,
        });
        assert!(updated.is_ok());
        let updated = match updated {
            Ok(value) => value,
            Err(_) => {
                assert!(false, "update should succeed");
                return;
            }
        };
        assert_eq!(updated.status, "accepted");
    }

    #[test]
    fn bridge_outbox_enqueue_and_mark_sent() {
        let mut store = ReducerStore::default();

        let queued = store.enqueue_bridge_event(EnqueueBridgeEventRequest {
            event_id: "evt_1".to_string(),
            transport: "nostr".to_string(),
            payload_json: "{\"kind\":7000}".to_string(),
            created_at_unix_ms: 1000,
        });
        assert!(queued.is_ok());

        let sent = store.mark_bridge_event_sent(MarkBridgeEventSentRequest {
            event_id: "evt_1".to_string(),
            updated_at_unix_ms: 1100,
        });
        assert!(sent.is_ok());
        let sent = match sent {
            Ok(value) => value,
            Err(_) => {
                assert!(false, "mark sent should succeed");
                return;
            }
        };
        assert_eq!(sent.status, BridgeOutboxStatus::Sent);
    }
}
