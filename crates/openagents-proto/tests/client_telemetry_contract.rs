use openagents_proto::wire::openagents::sync::v1::{
    ClientAuthFailure, ClientDeliveryError, ClientReconnect, ClientReplay, ClientSurface,
    ClientTelemetryEvent, client_telemetry_event,
};
use prost::Message;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/protocol/fixtures/client-telemetry-v1.json"
    ))
    .expect("client telemetry fixture JSON must parse")
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

fn parse_surface(value: &Value) -> i32 {
    match string_field(value, "surface").as_str() {
        "CLIENT_SURFACE_WEB" => ClientSurface::Web as i32,
        "CLIENT_SURFACE_DESKTOP" => ClientSurface::Desktop as i32,
        "CLIENT_SURFACE_IOS" => ClientSurface::Ios as i32,
        "CLIENT_SURFACE_ONYX" => ClientSurface::Onyx as i32,
        _ => ClientSurface::Unspecified as i32,
    }
}

fn parse_event_payload(value: &Value) -> client_telemetry_event::Event {
    if let Some(reconnect) = value.get("reconnect") {
        return client_telemetry_event::Event::Reconnect(ClientReconnect {
            attempt: u64_field(reconnect, "attempt") as u32,
            backoff_ms: u64_field(reconnect, "backoff_ms") as u32,
            resumed: bool_field(reconnect, "resumed"),
            last_applied_seq: u64_field(reconnect, "last_applied_seq"),
            reason_code: string_field(reconnect, "reason_code"),
        });
    }

    if let Some(auth_failure) = value.get("auth_failure") {
        return client_telemetry_event::Event::AuthFailure(ClientAuthFailure {
            http_status: u64_field(auth_failure, "http_status") as u32,
            reason_code: string_field(auth_failure, "reason_code"),
            refresh_attempted: bool_field(auth_failure, "refresh_attempted"),
        });
    }

    if let Some(replay) = value.get("replay") {
        return client_telemetry_event::Event::Replay(ClientReplay {
            status: string_field(replay, "status"),
            requested_after_seq: u64_field(replay, "requested_after_seq"),
            oldest_available_seq: u64_field(replay, "oldest_available_seq"),
            head_seq: u64_field(replay, "head_seq"),
            catchup_duration_ms: u64_field(replay, "catchup_duration_ms"),
        });
    }

    if let Some(delivery_error) = value.get("delivery_error") {
        return client_telemetry_event::Event::DeliveryError(ClientDeliveryError {
            channel: string_field(delivery_error, "channel"),
            reason_code: string_field(delivery_error, "reason_code"),
            rolling_error_count_5m: u64_field(delivery_error, "rolling_error_count_5m") as u32,
        });
    }

    panic!("fixture event must include reconnect/auth_failure/replay/delivery_error payload");
}

fn parse_wire_event(value: &Value) -> ClientTelemetryEvent {
    let oneof_keys = ["reconnect", "auth_failure", "replay", "delivery_error"];
    let oneof_count = oneof_keys
        .iter()
        .filter(|key| value.get(**key).is_some())
        .count();
    assert_eq!(
        oneof_count, 1,
        "fixture event must set exactly one event payload"
    );

    let forbidden_keys = ["user_id", "org_id", "email", "device_uuid"];
    for forbidden_key in forbidden_keys {
        assert!(
            value.get(forbidden_key).is_none(),
            "fixture event must not include privacy-sensitive field '{forbidden_key}'"
        );
    }

    ClientTelemetryEvent {
        schema_version: string_field(value, "schema_version"),
        event_id: string_field(value, "event_id"),
        occurred_at_unix_ms: u64_field(value, "occurred_at_unix_ms") as i64,
        surface: parse_surface(value),
        client_build_id: string_field(value, "client_build_id"),
        app_version: string_field(value, "app_version"),
        protocol_version: string_field(value, "protocol_version"),
        topic: string_field(value, "topic"),
        topic_class: string_field(value, "topic_class"),
        actor_scope_hash: string_field(value, "actor_scope_hash"),
        session_id: string_field(value, "session_id"),
        event: Some(parse_event_payload(value)),
    }
}

#[test]
fn client_telemetry_fixture_matches_wire_contract() {
    let root = fixture();
    assert_eq!(
        root.get("schema").and_then(Value::as_str),
        Some("openagents.sync.client_telemetry_fixture.v1")
    );

    let events = root
        .get("events")
        .and_then(Value::as_array)
        .expect("fixture must provide events array");
    assert!(!events.is_empty(), "fixture events must not be empty");

    for event in events {
        let wire = parse_wire_event(event);
        assert_eq!(
            wire.schema_version, "openagents.sync.client_telemetry.v1",
            "schema version mismatch"
        );
        assert!(
            !wire.actor_scope_hash.is_empty() && wire.actor_scope_hash.starts_with("sha256:"),
            "actor_scope_hash must be present and hashed"
        );

        let encoded = wire.encode_to_vec();
        let decoded = ClientTelemetryEvent::decode(encoded.as_slice())
            .expect("wire payload must decode after encode round-trip");
        assert_eq!(decoded.event_id, wire.event_id);
        assert_eq!(
            decoded.event.as_ref().map(discriminant),
            wire.event.as_ref().map(discriminant)
        );
    }
}

fn discriminant(event: &client_telemetry_event::Event) -> &'static str {
    match event {
        client_telemetry_event::Event::Reconnect(_) => "reconnect",
        client_telemetry_event::Event::AuthFailure(_) => "auth_failure",
        client_telemetry_event::Event::Replay(_) => "replay",
        client_telemetry_event::Event::DeliveryError(_) => "delivery_error",
    }
}
