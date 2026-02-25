use openagents_proto::wire::openagents::sync::v2::{
    Error as WireError, ResumeToken as WireResumeToken, StaleCursorRecovery as WireStaleRecovery,
    StreamCheckpoint as WireCheckpoint, StreamCursor as WireStreamCursor,
    Subscribe as WireSubscribe, SubscribeApplied as WireSubscribeApplied, SyncErrorCode,
    TransactionBatch as WireTransactionBatch, TransactionUpdate as WireTransactionUpdate,
};
use prost::Message;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/protocol/fixtures/sync-v2-envelope-v1.json"
    ))
    .expect("sync v2 fixture JSON must parse")
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn u32_field(value: &Value, key: &str) -> u32 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0) as u32
}

fn parse_cursor(value: &Value) -> WireStreamCursor {
    WireStreamCursor {
        stream_id: string_field(value, "stream_id"),
        watermark: u64_field(value, "watermark"),
    }
}

fn parse_checkpoint(value: &Value) -> WireCheckpoint {
    WireCheckpoint {
        stream_id: string_field(value, "stream_id"),
        last_applied_seq: u64_field(value, "last_applied_seq"),
        durable_offset: u64_field(value, "durable_offset"),
    }
}

fn parse_resume_token(value: &Value) -> Option<WireResumeToken> {
    let token = value.get("resume_token")?;
    let checkpoints = token
        .get("checkpoints")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_checkpoint).collect())
        .unwrap_or_default();

    Some(WireResumeToken {
        token_id: string_field(token, "token_id"),
        checkpoints,
        issued_at_unix_ms: u64_field(token, "issued_at_unix_ms"),
        expires_at_unix_ms: u64_field(token, "expires_at_unix_ms"),
    })
}

fn parse_subscribe(value: &Value) -> WireSubscribe {
    let stream_ids = value
        .get("stream_ids")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    let resume_after = value
        .get("resume_after")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_cursor).collect())
        .unwrap_or_default();

    WireSubscribe {
        stream_ids,
        resume_after,
        request_id: string_field(value, "request_id"),
        client_build_id: string_field(value, "client_build_id"),
        protocol_version: string_field(value, "protocol_version"),
        schema_version: u32_field(value, "schema_version"),
        resume_token: parse_resume_token(value),
    }
}

fn parse_subscribe_applied(value: &Value) -> WireSubscribeApplied {
    let current_watermarks = value
        .get("current_watermarks")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_cursor).collect())
        .unwrap_or_default();
    let accepted_checkpoints = value
        .get("accepted_checkpoints")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_checkpoint).collect())
        .unwrap_or_default();

    WireSubscribeApplied {
        subscription_id: string_field(value, "subscription_id"),
        current_watermarks,
        replay_complete: bool_field(value, "replay_complete"),
        accepted_checkpoints,
    }
}

fn parse_update(value: &Value) -> WireTransactionUpdate {
    WireTransactionUpdate {
        stream_id: string_field(value, "stream_id"),
        seq: u64_field(value, "seq"),
        payload: string_field(value, "payload").into_bytes(),
        payload_hash: string_field(value, "payload_hash").into_bytes(),
        replayed: bool_field(value, "replayed"),
        confirmed_read: bool_field(value, "confirmed_read"),
    }
}

fn parse_transaction_batch(value: &Value) -> WireTransactionBatch {
    let updates = value
        .get("updates")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_update).collect())
        .unwrap_or_default();
    let head_watermarks = value
        .get("head_watermarks")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_cursor).collect())
        .unwrap_or_default();

    WireTransactionBatch {
        updates,
        replay_complete: bool_field(value, "replay_complete"),
        head_watermarks,
    }
}

fn parse_error_code(raw: &str) -> i32 {
    match raw {
        "SYNC_ERROR_CODE_UNAUTHORIZED" => SyncErrorCode::Unauthorized as i32,
        "SYNC_ERROR_CODE_FORBIDDEN_STREAM" => SyncErrorCode::ForbiddenStream as i32,
        "SYNC_ERROR_CODE_BAD_SUBSCRIPTION" => SyncErrorCode::BadSubscription as i32,
        "SYNC_ERROR_CODE_STALE_CURSOR" => SyncErrorCode::StaleCursor as i32,
        "SYNC_ERROR_CODE_PAYLOAD_TOO_LARGE" => SyncErrorCode::PayloadTooLarge as i32,
        "SYNC_ERROR_CODE_RATE_LIMITED" => SyncErrorCode::RateLimited as i32,
        "SYNC_ERROR_CODE_UNSUPPORTED_PROTOCOL_VERSION" => {
            SyncErrorCode::UnsupportedProtocolVersion as i32
        }
        "SYNC_ERROR_CODE_UNSUPPORTED_SCHEMA_VERSION" => {
            SyncErrorCode::UnsupportedSchemaVersion as i32
        }
        "SYNC_ERROR_CODE_UPGRADE_REQUIRED" => SyncErrorCode::UpgradeRequired as i32,
        _ => SyncErrorCode::Internal as i32,
    }
}

fn parse_stale_recovery(value: &Value) -> WireStaleRecovery {
    WireStaleRecovery {
        stream_id: string_field(value, "stream_id"),
        requested_watermark: u64_field(value, "requested_watermark"),
        retention_floor: u64_field(value, "retention_floor"),
        replay_budget_floor: u64_field(value, "replay_budget_floor"),
        full_resync_required: bool_field(value, "full_resync_required"),
        reason_code: string_field(value, "reason_code"),
    }
}

fn parse_error(value: &Value) -> WireError {
    let stale_cursor_recovery = value
        .get("stale_cursor_recovery")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_stale_recovery).collect())
        .unwrap_or_default();

    WireError {
        code: parse_error_code(string_field(value, "code").as_str()),
        message: string_field(value, "message"),
        retry_after_ms: u32_field(value, "retry_after_ms"),
        full_resync_required: bool_field(value, "full_resync_required"),
        upgrade_required: bool_field(value, "upgrade_required"),
        min_client_build_id: string_field(value, "min_client_build_id"),
        max_client_build_id: string_field(value, "max_client_build_id"),
        min_schema_version: u32_field(value, "min_schema_version"),
        max_schema_version: u32_field(value, "max_schema_version"),
        protocol_version: string_field(value, "protocol_version"),
        stale_cursor_recovery,
    }
}

#[test]
fn sync_v2_fixture_matches_wire_contract_roundtrips() {
    let fixture = fixture();
    assert_eq!(
        fixture.get("schema").and_then(Value::as_str),
        Some("openagents.sync.v2.fixture.v1")
    );

    let subscribe = parse_subscribe(
        fixture
            .get("subscribe")
            .expect("fixture must contain subscribe payload"),
    );
    let subscribe_applied = parse_subscribe_applied(
        fixture
            .get("subscribe_applied")
            .expect("fixture must contain subscribe_applied payload"),
    );
    let transaction_batch = parse_transaction_batch(
        fixture
            .get("transaction_batch")
            .expect("fixture must contain transaction_batch payload"),
    );
    let error = parse_error(
        fixture
            .get("error")
            .expect("fixture must contain error payload"),
    );

    let subscribe_encoded = subscribe.encode_to_vec();
    let subscribe_decoded = WireSubscribe::decode(subscribe_encoded.as_slice())
        .expect("subscribe should decode after encode");
    assert_eq!(subscribe_decoded.request_id, subscribe.request_id);
    assert_eq!(
        subscribe_decoded.resume_after.len(),
        subscribe.resume_after.len()
    );

    let applied_encoded = subscribe_applied.encode_to_vec();
    let applied_decoded = WireSubscribeApplied::decode(applied_encoded.as_slice())
        .expect("subscribe_applied should decode after encode");
    assert_eq!(
        applied_decoded.subscription_id,
        subscribe_applied.subscription_id
    );
    assert_eq!(
        applied_decoded.accepted_checkpoints.len(),
        subscribe_applied.accepted_checkpoints.len()
    );

    let batch_encoded = transaction_batch.encode_to_vec();
    let batch_decoded = WireTransactionBatch::decode(batch_encoded.as_slice())
        .expect("transaction batch should decode after encode");
    assert_eq!(batch_decoded.updates.len(), transaction_batch.updates.len());
    assert_eq!(batch_decoded.head_watermarks.len(), 1);

    let error_encoded = error.encode_to_vec();
    let error_decoded =
        WireError::decode(error_encoded.as_slice()).expect("error should decode after encode");
    assert_eq!(error_decoded.code, error.code);
    assert_eq!(
        error_decoded.stale_cursor_recovery.len(),
        error.stale_cursor_recovery.len()
    );
}

