//! Typed Spacetime client primitives for protocol negotiation, subscribe/resume, and reducers.

use std::sync::{Arc, Mutex};

use reqwest::{Client as HttpClient, StatusCode};
use serde_json::{Value, json};

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

/// Network reducer error classes for runtime observability and retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacetimeReducerErrorClass {
    Auth,
    RateLimited,
    Network,
    Validation,
    Unknown,
}

/// Structured reducer call failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacetimeReducerError {
    pub class: SpacetimeReducerErrorClass,
    pub message: String,
}

/// Shared network client for calling Spacetime reducers over HTTP.
#[derive(Clone)]
pub struct SpacetimeReducerHttpClient {
    client: HttpClient,
    base_url: String,
    database: String,
    auth_token: Option<String>,
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

impl SpacetimeReducerHttpClient {
    pub fn new(base_url: &str, database: &str, auth_token: Option<String>) -> Result<Self, String> {
        let base_url = normalize_http_base_url(base_url)?;
        let database = database.trim().to_string();
        if database.is_empty() {
            return Err("database must not be empty".to_string());
        }
        let auth_token = auth_token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Self {
            client: HttpClient::builder()
                .build()
                .map_err(|error| format!("spacetime http client init failed: {error}"))?,
            base_url,
            database,
            auth_token,
        })
    }

    pub async fn append_sync_event(
        &self,
        request: AppendSyncEventRequest,
    ) -> Result<AppendSyncEventOutcome, SpacetimeReducerError> {
        let payload_json = String::from_utf8(request.payload_bytes.clone()).map_err(|error| {
            SpacetimeReducerError {
                class: SpacetimeReducerErrorClass::Validation,
                message: format!("payload bytes must be valid utf-8 json: {error}"),
            }
        })?;

        let endpoint = format!(
            "{}/v1/database/{}/call/append_sync_event",
            self.base_url, self.database
        );
        let body = json!([
            request.stream_id,
            request.idempotency_key,
            request.payload_hash,
            payload_json,
            request.committed_at_unix_ms,
            request.durable_offset,
            request.confirmed_read,
            request.expected_next_seq.unwrap_or(0),
        ]);

        let mut http_request = self
            .client
            .post(endpoint)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .json(&body);
        if let Some(token) = self.auth_token.as_deref() {
            http_request = http_request.bearer_auth(token);
        }

        let response = http_request
            .send()
            .await
            .map_err(|error| SpacetimeReducerError {
                class: SpacetimeReducerErrorClass::Network,
                message: format!("spacetime reducer call failed: {error}"),
            })?;
        let status = response.status();
        let payload = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SpacetimeReducerError {
                class: classify_network_status(status),
                message: format!(
                    "spacetime reducer call failed status={} body={}",
                    status.as_u16(),
                    payload
                ),
            });
        }

        let parsed = serde_json::from_str::<Value>(payload.as_str()).ok();
        let seq = parsed
            .as_ref()
            .and_then(parse_reducer_seq)
            .or(request.expected_next_seq)
            .unwrap_or(request.durable_offset);

        Ok(AppendSyncEventOutcome::Applied(SyncEvent {
            stream_id: request.stream_id,
            seq,
            idempotency_key: request.idempotency_key,
            payload_hash: request.payload_hash,
            payload_bytes: request.payload_bytes,
            committed_at_unix_ms: request.committed_at_unix_ms,
            durable_offset: request.durable_offset,
            confirmed_read: request.confirmed_read,
        }))
    }

    pub async fn query_sql(&self, query: &str) -> Result<Value, SpacetimeReducerError> {
        let endpoint = format!("{}/v1/database/{}/sql", self.base_url, self.database);
        let mut request = self
            .client
            .post(endpoint.as_str())
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .json(&json!({ "query": query }));
        if let Some(token) = self.auth_token.as_deref() {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|error| SpacetimeReducerError {
                class: SpacetimeReducerErrorClass::Network,
                message: format!("spacetime sql request failed: {error}"),
            })?;

        if response.status().is_success() {
            return response
                .json::<Value>()
                .await
                .map_err(|error| SpacetimeReducerError {
                    class: SpacetimeReducerErrorClass::Validation,
                    message: format!("spacetime sql parse failed: {error}"),
                });
        }

        if response.status() == StatusCode::METHOD_NOT_ALLOWED
            || response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::BAD_REQUEST
        {
            let mut fallback = self
                .client
                .get(endpoint.as_str())
                .header("accept", "application/json")
                .query(&[("query", query)]);
            if let Some(token) = self.auth_token.as_deref() {
                fallback = fallback.bearer_auth(token);
            }
            let fallback_response =
                fallback
                    .send()
                    .await
                    .map_err(|error| SpacetimeReducerError {
                        class: SpacetimeReducerErrorClass::Network,
                        message: format!("spacetime sql GET fallback failed: {error}"),
                    })?;
            if fallback_response.status().is_success() {
                return fallback_response.json::<Value>().await.map_err(|error| {
                    SpacetimeReducerError {
                        class: SpacetimeReducerErrorClass::Validation,
                        message: format!("spacetime sql fallback parse failed: {error}"),
                    }
                });
            }

            let status = fallback_response.status();
            let body = fallback_response.text().await.unwrap_or_default();
            return Err(SpacetimeReducerError {
                class: classify_network_status(status),
                message: format!(
                    "spacetime sql GET fallback failed status={} body={}",
                    status.as_u16(),
                    body
                ),
            });
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(SpacetimeReducerError {
            class: classify_network_status(status),
            message: format!(
                "spacetime sql failed status={} body={}",
                status.as_u16(),
                body
            ),
        })
    }
}

fn normalize_http_base_url(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err("base_url must not be empty".to_string());
    }
    let parsed =
        reqwest::Url::parse(normalized).map_err(|error| format!("invalid base_url: {error}"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported base_url scheme: {scheme}"));
    }
    Ok(normalized.trim_end_matches('/').to_string())
}

fn classify_network_status(status: StatusCode) -> SpacetimeReducerErrorClass {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => SpacetimeReducerErrorClass::Auth,
        StatusCode::TOO_MANY_REQUESTS => SpacetimeReducerErrorClass::RateLimited,
        StatusCode::BAD_REQUEST
        | StatusCode::UNPROCESSABLE_ENTITY
        | StatusCode::CONFLICT
        | StatusCode::NOT_FOUND => SpacetimeReducerErrorClass::Validation,
        s if s.is_server_error() => SpacetimeReducerErrorClass::Network,
        _ => SpacetimeReducerErrorClass::Unknown,
    }
}

fn parse_reducer_seq(value: &Value) -> Option<u64> {
    value
        .pointer("/data/result")
        .and_then(Value::as_u64)
        .or_else(|| value.pointer("/result").and_then(Value::as_u64))
        .or_else(|| value.get("seq").and_then(Value::as_u64))
        .or_else(|| value.pointer("/data/seq").and_then(Value::as_u64))
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::SyncSessionClaims,
        client::{
            ProtocolVersion, ResumeAction, SpacetimeClient, SpacetimeClientConfig,
            SpacetimeClientError, SpacetimeReducerHttpClient, SubscribeRequest, parse_reducer_seq,
        },
        reducers::{AckCheckpointRequest, AppendSyncEventRequest},
    };
    use serde_json::json;

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

    #[test]
    fn reducer_http_client_requires_valid_base_url() {
        let result = SpacetimeReducerHttpClient::new("://bad", "autopilot", None);
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.contains("invalid base_url"));
        }
    }

    #[test]
    fn reducer_seq_parser_accepts_common_shapes() {
        assert_eq!(parse_reducer_seq(&json!({"data":{"result":17}})), Some(17));
        assert_eq!(parse_reducer_seq(&json!({"result":11})), Some(11));
        assert_eq!(parse_reducer_seq(&json!({"seq":9})), Some(9));
    }
}
