//! Typed Spacetime client primitives for protocol negotiation, subscribe/resume, and reducers.

use std::sync::{Arc, Mutex};

use crate::{
    auth::{
        SyncAuthorizationError, SyncSessionClaims, authorize_ack_checkpoint,
        authorize_append_sync_event, authorize_subscription,
    },
    mapping::{CursorContinuity, StreamCursor, StreamWindow, evaluate_cursor_continuity},
    reducers::{
        AckCheckpointRequest, AppendSyncEventOutcome, AppendSyncEventRequest, ReducerError,
        ReducerStore, SyncCheckpoint, SyncEvent,
    },
    subscriptions::SubscriptionQuerySet,
};

/// Supported websocket protocol variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolVersion {
    V2Bsatn,
    V1Bsatn,
}

impl ProtocolVersion {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V2Bsatn => "v2.bsatn.spacetimedb",
            Self::V1Bsatn => "v1.bsatn.spacetimedb",
        }
    }
}

/// Client error taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpacetimeClientError {
    ProtocolNegotiationFailed {
        client_supported: Vec<String>,
        server_supported: Vec<String>,
    },
    Unauthorized {
        reason: String,
    },
    Forbidden {
        reason: String,
    },
    StaleCursor {
        stream_id: String,
        requested_after_seq: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
    },
    SequenceConflict {
        stream_id: String,
        expected_next_seq: u64,
        actual_next_seq: u64,
    },
    InvalidRequest(String),
    Internal(String),
}

impl SpacetimeClientError {
    fn from_auth(error: SyncAuthorizationError) -> Self {
        match error {
            SyncAuthorizationError::NotYetValid => Self::Unauthorized {
                reason: "token_not_yet_valid".to_string(),
            },
            SyncAuthorizationError::Expired => Self::Unauthorized {
                reason: "token_expired".to_string(),
            },
            SyncAuthorizationError::MissingScope { required_scope } => Self::Forbidden {
                reason: format!("missing_scope:{required_scope}"),
            },
            SyncAuthorizationError::StreamNotGranted { stream_id } => Self::Forbidden {
                reason: format!("stream_not_granted:{stream_id}"),
            },
        }
    }

    fn from_reducer(error: ReducerError) -> Self {
        match error {
            ReducerError::SequenceConflict {
                stream_id,
                expected_next_seq,
                actual_next_seq,
            } => Self::SequenceConflict {
                stream_id,
                expected_next_seq,
                actual_next_seq,
            },
            other => Self::Internal(format!("{other:?}")),
        }
    }
}

/// Client configuration.
#[derive(Debug, Clone)]
pub struct SpacetimeClientConfig {
    pub supported_protocols: Vec<ProtocolVersion>,
    pub replay_budget_events: u64,
    pub reconnect_base_backoff_ms: u64,
    pub reconnect_max_backoff_ms: u64,
}

impl Default for SpacetimeClientConfig {
    fn default() -> Self {
        Self {
            supported_protocols: vec![ProtocolVersion::V2Bsatn, ProtocolVersion::V1Bsatn],
            replay_budget_events: 20_000,
            reconnect_base_backoff_ms: 200,
            reconnect_max_backoff_ms: 5_000,
        }
    }
}

/// Subscribe request shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeRequest {
    pub stream_id: String,
    pub after_seq: u64,
    pub confirmed_read_durable_floor: Option<u64>,
}

/// Subscribe result shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeResult {
    pub stream_id: String,
    pub protocol: ProtocolVersion,
    pub snapshot_events: Vec<SyncEvent>,
    pub next_after_seq: u64,
}

/// Resume planning outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeAction {
    Resume,
    Rebootstrap,
}

/// Reconnect/resume plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePlan {
    pub stream_cursor: StreamCursor,
    pub action: ResumeAction,
    pub reason_codes: Vec<String>,
}

/// Typed in-memory Spacetime client.
#[derive(Clone)]
pub struct SpacetimeClient {
    store: Arc<Mutex<ReducerStore>>,
    config: SpacetimeClientConfig,
    negotiated_protocol: Arc<Mutex<Option<ProtocolVersion>>>,
}

impl SpacetimeClient {
    #[must_use]
    pub fn new(store: Arc<Mutex<ReducerStore>>, config: SpacetimeClientConfig) -> Self {
        Self {
            store,
            config,
            negotiated_protocol: Arc::new(Mutex::new(None)),
        }
    }

    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(Mutex::new(ReducerStore::default())),
            SpacetimeClientConfig::default(),
        )
    }

    pub fn negotiate_protocol(
        &self,
        server_supported: &[ProtocolVersion],
    ) -> Result<ProtocolVersion, SpacetimeClientError> {
        for candidate in &self.config.supported_protocols {
            if server_supported.iter().any(|server| server == candidate) {
                let selected = candidate.clone();
                if let Ok(mut guard) = self.negotiated_protocol.lock() {
                    *guard = Some(selected.clone());
                }
                return Ok(selected);
            }
        }

        Err(SpacetimeClientError::ProtocolNegotiationFailed {
            client_supported: self
                .config
                .supported_protocols
                .iter()
                .map(|value| value.as_str().to_string())
                .collect(),
            server_supported: server_supported
                .iter()
                .map(|value| value.as_str().to_string())
                .collect(),
        })
    }

    pub fn subscribe(
        &self,
        claims: &SyncSessionClaims,
        request: SubscribeRequest,
        now_unix_ms: u64,
    ) -> Result<SubscribeResult, SpacetimeClientError> {
        let stream_id = request.stream_id.trim().to_string();
        if stream_id.is_empty() {
            return Err(SpacetimeClientError::InvalidRequest(
                "stream_id is required".to_string(),
            ));
        }
        authorize_subscription(
            claims,
            &SubscriptionQuerySet::StreamEvents {
                stream_id: stream_id.clone(),
                after_seq: request.after_seq,
            },
            now_unix_ms,
        )
        .map_err(SpacetimeClientError::from_auth)?;

        let store = self
            .store
            .lock()
            .map_err(|_| SpacetimeClientError::Internal("store mutex poisoned".to_string()))?;
        let window = stream_window(
            &store,
            stream_id.as_str(),
            self.config.replay_budget_events.max(1),
        );
        let continuity = evaluate_cursor_continuity(
            StreamCursor {
                stream_id: stream_id.clone(),
                after_seq: request.after_seq,
            },
            window,
        );
        let events = match continuity {
            CursorContinuity::Resume(cursor) => store.deliverable_stream_events(
                cursor.stream_id.as_str(),
                cursor.after_seq,
                request.confirmed_read_durable_floor,
            ),
            CursorContinuity::Rebootstrap {
                stream_id,
                requested_after_seq,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
            } => {
                return Err(SpacetimeClientError::StaleCursor {
                    stream_id,
                    requested_after_seq,
                    oldest_available_cursor,
                    head_cursor,
                    reason_codes,
                    replay_lag,
                    replay_budget_events,
                });
            }
        };
        let next_after_seq = events
            .last()
            .map(|event| event.seq)
            .unwrap_or(request.after_seq);
        let protocol = self
            .negotiated_protocol
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or(ProtocolVersion::V2Bsatn);

        Ok(SubscribeResult {
            stream_id: request.stream_id,
            protocol,
            snapshot_events: events,
            next_after_seq,
        })
    }

    pub fn append_sync_event(
        &self,
        claims: &SyncSessionClaims,
        request: AppendSyncEventRequest,
        now_unix_ms: u64,
    ) -> Result<AppendSyncEventOutcome, SpacetimeClientError> {
        authorize_append_sync_event(claims, &request, now_unix_ms)
            .map_err(SpacetimeClientError::from_auth)?;
        self.store
            .lock()
            .map_err(|_| SpacetimeClientError::Internal("store mutex poisoned".to_string()))?
            .append_sync_event(request)
            .map_err(SpacetimeClientError::from_reducer)
    }

    pub fn ack_checkpoint(
        &self,
        claims: &SyncSessionClaims,
        request: AckCheckpointRequest,
        now_unix_ms: u64,
    ) -> Result<SyncCheckpoint, SpacetimeClientError> {
        authorize_ack_checkpoint(claims, &request, now_unix_ms)
            .map_err(SpacetimeClientError::from_auth)?;
        self.store
            .lock()
            .map_err(|_| SpacetimeClientError::Internal("store mutex poisoned".to_string()))?
            .ack_checkpoint(request)
            .map_err(SpacetimeClientError::from_reducer)
    }

    #[must_use]
    pub fn resume_plan(&self, cursor: StreamCursor, window: Option<StreamWindow>) -> ResumePlan {
        match evaluate_cursor_continuity(cursor.clone(), window) {
            CursorContinuity::Resume(_) => ResumePlan {
                stream_cursor: cursor,
                action: ResumeAction::Resume,
                reason_codes: Vec::new(),
            },
            CursorContinuity::Rebootstrap { reason_codes, .. } => ResumePlan {
                stream_cursor: StreamCursor {
                    stream_id: cursor.stream_id,
                    after_seq: 0,
                },
                action: ResumeAction::Rebootstrap,
                reason_codes,
            },
        }
    }

    #[must_use]
    pub fn reconnect_backoff_ms(&self, attempt: u32) -> u64 {
        let capped_attempt = attempt.min(8);
        let multiplier = 1_u64 << capped_attempt;
        let backoff = self
            .config
            .reconnect_base_backoff_ms
            .saturating_mul(multiplier);
        backoff.min(self.config.reconnect_max_backoff_ms)
    }
}

fn stream_window(
    store: &ReducerStore,
    stream_id: &str,
    replay_budget_events: u64,
) -> Option<StreamWindow> {
    let events = store.stream_events(stream_id);
    let oldest = events.first()?.seq;
    let head = events.last()?.seq;
    Some(StreamWindow {
        stream_id: stream_id.to_string(),
        oldest_seq: oldest,
        head_seq: head,
        replay_budget_events,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::SyncSessionClaims,
        client::{
            ProtocolVersion, ResumeAction, SpacetimeClient, SpacetimeClientConfig,
            SpacetimeClientError, SubscribeRequest,
        },
        reducers::{AckCheckpointRequest, AppendSyncEventRequest},
    };

    fn claims(scopes: &[&str], streams: &[&str]) -> SyncSessionClaims {
        SyncSessionClaims {
            session_id: "sess_client".to_string(),
            scopes: scopes.iter().map(|value| (*value).to_string()).collect(),
            allowed_streams: Some(streams.iter().map(|value| (*value).to_string()).collect()),
            issued_at_unix_ms: 1_000,
            not_before_unix_ms: 1_000,
            expires_at_unix_ms: 10_000,
        }
    }

    #[test]
    fn protocol_negotiation_selects_first_supported_client_protocol() {
        let client = SpacetimeClient::in_memory();
        let negotiated = client
            .negotiate_protocol(&[ProtocolVersion::V1Bsatn, ProtocolVersion::V2Bsatn])
            .expect("protocol should negotiate");
        assert_eq!(negotiated, ProtocolVersion::V2Bsatn);
    }

    #[test]
    fn protocol_negotiation_rejects_when_no_overlap() {
        let client = SpacetimeClient::new(
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::reducers::ReducerStore::default(),
            )),
            SpacetimeClientConfig {
                supported_protocols: vec![ProtocolVersion::V2Bsatn],
                replay_budget_events: 20_000,
                reconnect_base_backoff_ms: 100,
                reconnect_max_backoff_ms: 1_000,
            },
        );
        let error = client
            .negotiate_protocol(&[ProtocolVersion::V1Bsatn])
            .expect_err("protocol mismatch expected");
        assert!(matches!(
            error,
            SpacetimeClientError::ProtocolNegotiationFailed { .. }
        ));
    }

    #[test]
    fn subscribe_rejects_stale_cursor() {
        let client = SpacetimeClient::new(
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::reducers::ReducerStore::default(),
            )),
            SpacetimeClientConfig {
                supported_protocols: vec![ProtocolVersion::V2Bsatn],
                replay_budget_events: 2,
                reconnect_base_backoff_ms: 100,
                reconnect_max_backoff_ms: 1_000,
            },
        );
        let auth_claims = claims(
            &["sync.append", "sync.subscribe", "sync.checkpoint.write"],
            &["runtime.run.job-1.events"],
        );
        for seq in 1..=5 {
            let _ = client.append_sync_event(
                &auth_claims,
                AppendSyncEventRequest {
                    stream_id: "runtime.run.job-1.events".to_string(),
                    idempotency_key: format!("idem-{seq}"),
                    payload_hash: format!("hash-{seq}"),
                    payload_bytes: vec![seq as u8],
                    committed_at_unix_ms: 1_200 + seq,
                    durable_offset: seq,
                    confirmed_read: true,
                    expected_next_seq: Some(seq),
                },
                1_500,
            );
        }

        let result = client.subscribe(
            &claims(&["sync.subscribe"], &["runtime.run.job-1.events"]),
            SubscribeRequest {
                stream_id: "runtime.run.job-1.events".to_string(),
                after_seq: 0,
                confirmed_read_durable_floor: None,
            },
            2_000,
        );
        assert!(matches!(
            result,
            Err(SpacetimeClientError::StaleCursor { .. })
        ));
    }

    #[test]
    fn reducer_calls_enforce_scope_and_stream_grants() {
        let client = SpacetimeClient::in_memory();
        let denied = client.append_sync_event(
            &claims(&["sync.subscribe"], &["runtime.run.job-1.events"]),
            AppendSyncEventRequest {
                stream_id: "runtime.run.job-2.events".to_string(),
                idempotency_key: "idem-denied".to_string(),
                payload_hash: "hash-denied".to_string(),
                payload_bytes: vec![1],
                committed_at_unix_ms: 1_500,
                durable_offset: 1,
                confirmed_read: true,
                expected_next_seq: Some(1),
            },
            1_600,
        );
        assert!(matches!(
            denied,
            Err(SpacetimeClientError::Forbidden { .. })
        ));
    }

    #[test]
    fn reconnect_resume_helpers_plan_rebootstrap_and_backoff() {
        let client = SpacetimeClient::new(
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::reducers::ReducerStore::default(),
            )),
            SpacetimeClientConfig {
                supported_protocols: vec![ProtocolVersion::V2Bsatn],
                replay_budget_events: 5,
                reconnect_base_backoff_ms: 50,
                reconnect_max_backoff_ms: 500,
            },
        );
        let plan = client.resume_plan(
            crate::mapping::StreamCursor {
                stream_id: "runtime.run.resume.events".to_string(),
                after_seq: 1,
            },
            Some(crate::mapping::StreamWindow {
                stream_id: "runtime.run.resume.events".to_string(),
                oldest_seq: 20,
                head_seq: 40,
                replay_budget_events: 5,
            }),
        );
        assert_eq!(plan.action, ResumeAction::Rebootstrap);
        assert_eq!(plan.stream_cursor.after_seq, 0);
        assert!(!plan.reason_codes.is_empty());
        assert_eq!(client.reconnect_backoff_ms(0), 50);
        assert_eq!(client.reconnect_backoff_ms(5), 500);
    }

    #[test]
    fn checkpoint_ack_requires_scope_and_can_succeed() {
        let client = SpacetimeClient::in_memory();
        let auth_claims = claims(
            &["sync.append", "sync.subscribe", "sync.checkpoint.write"],
            &["runtime.run.job-ack.events"],
        );
        let append = client.append_sync_event(
            &auth_claims,
            AppendSyncEventRequest {
                stream_id: "runtime.run.job-ack.events".to_string(),
                idempotency_key: "idem-ack".to_string(),
                payload_hash: "hash-ack".to_string(),
                payload_bytes: vec![9],
                committed_at_unix_ms: 2_000,
                durable_offset: 1,
                confirmed_read: true,
                expected_next_seq: Some(1),
            },
            2_100,
        );
        assert!(append.is_ok());

        let checkpoint = client
            .ack_checkpoint(
                &claims(&["sync.checkpoint.write"], &["runtime.run.job-ack.events"]),
                AckCheckpointRequest {
                    client_id: "desktop-main".to_string(),
                    stream_id: "runtime.run.job-ack.events".to_string(),
                    last_applied_seq: 1,
                    durable_offset: 1,
                    updated_at_unix_ms: 2_200,
                },
                2_200,
            )
            .expect("checkpoint ack should succeed");
        assert_eq!(checkpoint.last_applied_seq, 1);
    }

    #[test]
    fn multi_client_subscribe_preserves_ordering_for_shared_stream() {
        let shared_store = std::sync::Arc::new(std::sync::Mutex::new(
            crate::reducers::ReducerStore::default(),
        ));
        let client_a = SpacetimeClient::new(shared_store.clone(), SpacetimeClientConfig::default());
        let client_b = SpacetimeClient::new(shared_store, SpacetimeClientConfig::default());
        let auth_claims = claims(
            &["sync.append", "sync.subscribe", "sync.checkpoint.write"],
            &["runtime.run.ordering.events"],
        );

        for seq in 1..=4 {
            let appended = client_a.append_sync_event(
                &auth_claims,
                AppendSyncEventRequest {
                    stream_id: "runtime.run.ordering.events".to_string(),
                    idempotency_key: format!("ordering-{seq}"),
                    payload_hash: format!("ordering-hash-{seq}"),
                    payload_bytes: vec![seq as u8],
                    committed_at_unix_ms: 1_200 + seq,
                    durable_offset: seq,
                    confirmed_read: true,
                    expected_next_seq: Some(seq),
                },
                1_500,
            );
            assert!(appended.is_ok());
        }

        let subscribe_a = client_a
            .subscribe(
                &claims(&["sync.subscribe"], &["runtime.run.ordering.events"]),
                SubscribeRequest {
                    stream_id: "runtime.run.ordering.events".to_string(),
                    after_seq: 0,
                    confirmed_read_durable_floor: None,
                },
                1_600,
            )
            .expect("client A subscribe should succeed");
        let subscribe_b = client_b
            .subscribe(
                &claims(&["sync.subscribe"], &["runtime.run.ordering.events"]),
                SubscribeRequest {
                    stream_id: "runtime.run.ordering.events".to_string(),
                    after_seq: 0,
                    confirmed_read_durable_floor: None,
                },
                1_600,
            )
            .expect("client B subscribe should succeed");

        let seqs_a = subscribe_a
            .snapshot_events
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>();
        let seqs_b = subscribe_b
            .snapshot_events
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>();
        assert_eq!(seqs_a, vec![1, 2, 3, 4]);
        assert_eq!(seqs_b, seqs_a);

        let hashes_a = subscribe_a
            .snapshot_events
            .iter()
            .map(|event| event.payload_hash.clone())
            .collect::<Vec<_>>();
        let hashes_b = subscribe_b
            .snapshot_events
            .iter()
            .map(|event| event.payload_hash.clone())
            .collect::<Vec<_>>();
        assert_eq!(hashes_b, hashes_a);
    }

    #[test]
    fn reconnect_storm_resubscribe_keeps_duplicate_delivery_deterministic() {
        let client = SpacetimeClient::in_memory();
        let auth_claims = claims(
            &["sync.append", "sync.subscribe", "sync.checkpoint.write"],
            &["runtime.run.reconnect.events"],
        );

        for seq in 1..=3 {
            let appended = client.append_sync_event(
                &auth_claims,
                AppendSyncEventRequest {
                    stream_id: "runtime.run.reconnect.events".to_string(),
                    idempotency_key: format!("reconnect-{seq}"),
                    payload_hash: format!("reconnect-hash-{seq}"),
                    payload_bytes: vec![seq as u8],
                    committed_at_unix_ms: 1_200 + seq,
                    durable_offset: seq,
                    confirmed_read: true,
                    expected_next_seq: Some(seq),
                },
                1_500,
            );
            assert!(appended.is_ok());
        }

        for _ in 0..5 {
            let replay_batch = client
                .subscribe(
                    &claims(&["sync.subscribe"], &["runtime.run.reconnect.events"]),
                    SubscribeRequest {
                        stream_id: "runtime.run.reconnect.events".to_string(),
                        after_seq: 0,
                        confirmed_read_durable_floor: None,
                    },
                    1_600,
                )
                .expect("subscribe should succeed");
            let seqs = replay_batch
                .snapshot_events
                .iter()
                .map(|event| event.seq)
                .collect::<Vec<_>>();
            assert_eq!(seqs, vec![1, 2, 3]);
            assert_eq!(replay_batch.next_after_seq, 3);
        }

        let resumed = client
            .subscribe(
                &claims(&["sync.subscribe"], &["runtime.run.reconnect.events"]),
                SubscribeRequest {
                    stream_id: "runtime.run.reconnect.events".to_string(),
                    after_seq: 3,
                    confirmed_read_durable_floor: None,
                },
                1_600,
            )
            .expect("resume subscribe should succeed");
        assert!(resumed.snapshot_events.is_empty());
        assert_eq!(resumed.next_after_seq, 3);
    }
}
