//! Scope and stream-grant enforcement for sync subscriptions and reducers.

use std::collections::HashSet;

use crate::reducers::{AckCheckpointRequest, AppendSyncEventRequest};
use crate::subscriptions::SubscriptionQuerySet;

/// Canonical claims shape used by sync reducer/subscription authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSessionClaims {
    pub session_id: String,
    pub scopes: HashSet<String>,
    pub allowed_streams: Option<HashSet<String>>,
    pub issued_at_unix_ms: u64,
    pub not_before_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

/// Authorization error taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAuthorizationError {
    NotYetValid,
    Expired,
    MissingScope { required_scope: String },
    StreamNotGranted { stream_id: String },
}

const SCOPE_SYNC_SUBSCRIBE: &str = "sync.subscribe";
const SCOPE_SYNC_APPEND: &str = "sync.append";
const SCOPE_SYNC_CHECKPOINT_WRITE: &str = "sync.checkpoint.write";

/// Authorizes a subscription query set.
pub fn authorize_subscription(
    claims: &SyncSessionClaims,
    query: &SubscriptionQuerySet,
    now_unix_ms: u64,
) -> Result<(), SyncAuthorizationError> {
    ensure_active(claims, now_unix_ms)?;
    ensure_scope(claims, SCOPE_SYNC_SUBSCRIBE)?;

    match query {
        SubscriptionQuerySet::StreamEvents { stream_id, .. } => ensure_stream_grant(claims, stream_id),
        _ => Ok(()),
    }
}

/// Authorizes append reducer calls for stream event writes.
pub fn authorize_append_sync_event(
    claims: &SyncSessionClaims,
    request: &AppendSyncEventRequest,
    now_unix_ms: u64,
) -> Result<(), SyncAuthorizationError> {
    ensure_active(claims, now_unix_ms)?;
    ensure_scope(claims, SCOPE_SYNC_APPEND)?;
    ensure_stream_grant(claims, &request.stream_id)
}

/// Authorizes checkpoint reducers for stream apply acknowledgements.
pub fn authorize_ack_checkpoint(
    claims: &SyncSessionClaims,
    request: &AckCheckpointRequest,
    now_unix_ms: u64,
) -> Result<(), SyncAuthorizationError> {
    ensure_active(claims, now_unix_ms)?;
    ensure_scope(claims, SCOPE_SYNC_CHECKPOINT_WRITE)?;
    ensure_stream_grant(claims, &request.stream_id)
}

fn ensure_active(
    claims: &SyncSessionClaims,
    now_unix_ms: u64,
) -> Result<(), SyncAuthorizationError> {
    if now_unix_ms < claims.not_before_unix_ms {
        return Err(SyncAuthorizationError::NotYetValid);
    }
    if now_unix_ms >= claims.expires_at_unix_ms {
        return Err(SyncAuthorizationError::Expired);
    }
    Ok(())
}

fn ensure_scope(
    claims: &SyncSessionClaims,
    required_scope: &str,
) -> Result<(), SyncAuthorizationError> {
    if claims.scopes.contains(required_scope) {
        Ok(())
    } else {
        Err(SyncAuthorizationError::MissingScope {
            required_scope: required_scope.to_string(),
        })
    }
}

fn ensure_stream_grant(
    claims: &SyncSessionClaims,
    stream_id: &str,
) -> Result<(), SyncAuthorizationError> {
    if let Some(grants) = claims.allowed_streams.as_ref()
        && !grants.contains(stream_id)
    {
        return Err(SyncAuthorizationError::StreamNotGranted {
            stream_id: stream_id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        auth::{
            SyncAuthorizationError, SyncSessionClaims, authorize_ack_checkpoint,
            authorize_append_sync_event, authorize_subscription,
        },
        reducers::{AckCheckpointRequest, AppendSyncEventRequest},
        subscriptions::SubscriptionQuerySet,
    };

    fn claims_with_scope(scope: &str) -> SyncSessionClaims {
        SyncSessionClaims {
            session_id: "sess_1".to_string(),
            scopes: HashSet::from([scope.to_string()]),
            allowed_streams: Some(HashSet::from(["runtime.codex.worker.worker_1".to_string()])),
            issued_at_unix_ms: 1_000,
            not_before_unix_ms: 1_000,
            expires_at_unix_ms: 2_000,
        }
    }

    #[test]
    fn subscription_rejects_missing_scope() {
        let claims = claims_with_scope("sync.append");
        let result = authorize_subscription(
            &claims,
            &SubscriptionQuerySet::StreamEvents {
                stream_id: "runtime.codex.worker.worker_1".to_string(),
                after_seq: 0,
            },
            1_100,
        );
        assert_eq!(
            result,
            Err(SyncAuthorizationError::MissingScope {
                required_scope: "sync.subscribe".to_string(),
            })
        );
    }

    #[test]
    fn append_rejects_stream_outside_grant() {
        let claims = claims_with_scope("sync.append");
        let request = AppendSyncEventRequest {
            stream_id: "runtime.codex.worker.worker_2".to_string(),
            idempotency_key: "idempo_1".to_string(),
            payload_hash: "abc123".to_string(),
            payload_bytes: vec![1, 2, 3],
            committed_at_unix_ms: 1_100,
            durable_offset: 9,
            confirmed_read: true,
            expected_next_seq: Some(1),
        };
        let result = authorize_append_sync_event(&claims, &request, 1_100);
        assert_eq!(
            result,
            Err(SyncAuthorizationError::StreamNotGranted {
                stream_id: "runtime.codex.worker.worker_2".to_string(),
            })
        );
    }

    #[test]
    fn checkpoint_rejects_expired_claims() {
        let claims = claims_with_scope("sync.checkpoint.write");
        let request = AckCheckpointRequest {
            client_id: "desktop_1".to_string(),
            stream_id: "runtime.codex.worker.worker_1".to_string(),
            last_applied_seq: 12,
            durable_offset: 12,
            updated_at_unix_ms: 1_500,
        };
        let result = authorize_ack_checkpoint(&claims, &request, 2_000);
        assert_eq!(result, Err(SyncAuthorizationError::Expired));
    }

    #[test]
    fn subscription_accepts_valid_scope_stream_and_time_window() {
        let claims = claims_with_scope("sync.subscribe");
        let result = authorize_subscription(
            &claims,
            &SubscriptionQuerySet::StreamEvents {
                stream_id: "runtime.codex.worker.worker_1".to_string(),
                after_seq: 12,
            },
            1_100,
        );
        assert!(result.is_ok());
    }
}
