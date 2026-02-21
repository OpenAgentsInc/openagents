use serde::Deserialize;
use serde_json::Value;

const WORKER_EVENT_TYPE: &str = "worker.event";
const IOS_HANDSHAKE_SOURCE: &str = "autopilot-ios";
const IOS_HANDSHAKE_METHOD: &str = "ios/handshake";
const DESKTOP_ACK_SOURCE: &str = "autopilot-desktop";
const DESKTOP_ACK_METHOD: &str = "desktop/handshake_ack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeKind {
    IosHandshake,
    DesktopHandshakeAck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandshakeEnvelope {
    kind: HandshakeKind,
    handshake_id: String,
}

#[derive(Debug, Deserialize)]
struct WorkerEventEnvelope {
    #[serde(rename = "eventType", alias = "event_type")]
    event_type: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct WorkerHandshakePayload {
    source: String,
    method: String,
    #[serde(rename = "handshake_id", alias = "handshakeId")]
    handshake_id: String,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    desktop_session_id: Option<String>,
    #[serde(default)]
    occurred_at: Option<String>,
}

pub fn extract_ios_handshake_id(payload: &Value) -> Option<String> {
    let envelope = extract_handshake_envelope(payload)?;
    if envelope.kind == HandshakeKind::IosHandshake {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

pub fn extract_desktop_handshake_ack_id(payload: &Value) -> Option<String> {
    let envelope = extract_handshake_envelope(payload)?;
    if envelope.kind == HandshakeKind::DesktopHandshakeAck {
        Some(envelope.handshake_id)
    } else {
        None
    }
}

fn extract_handshake_envelope(payload: &Value) -> Option<HandshakeEnvelope> {
    let envelope = serde_json::from_value::<WorkerEventEnvelope>(payload.clone()).ok()?;
    if envelope.event_type != WORKER_EVENT_TYPE {
        return None;
    }

    let worker_payload = serde_json::from_value::<WorkerHandshakePayload>(envelope.payload).ok()?;
    let handshake_id = non_empty(worker_payload.handshake_id.as_str())?.to_string();

    match (
        worker_payload.source.as_str(),
        worker_payload.method.as_str(),
    ) {
        (IOS_HANDSHAKE_SOURCE, IOS_HANDSHAKE_METHOD)
            if required_value(worker_payload.device_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(HandshakeEnvelope {
                kind: HandshakeKind::IosHandshake,
                handshake_id,
            })
        }
        (DESKTOP_ACK_SOURCE, DESKTOP_ACK_METHOD)
            if required_value(worker_payload.desktop_session_id.as_deref())
                && required_value(worker_payload.occurred_at.as_deref()) =>
        {
            Some(HandshakeEnvelope {
                kind: HandshakeKind::DesktopHandshakeAck,
                handshake_id,
            })
        }
        _ => None,
    }
}

fn required_value(raw: Option<&str>) -> bool {
    non_empty(raw.unwrap_or_default()).is_some()
}

fn non_empty(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_desktop_handshake_ack_id, extract_ios_handshake_id};
    use serde_json::json;

    #[test]
    fn extract_ios_handshake_id_requires_proto_fields() {
        let payload = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-123",
                "device_id": "ios-device",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        assert_eq!(
            extract_ios_handshake_id(&payload).as_deref(),
            Some("hs-123")
        );

        let invalid = json!({
            "eventType": "worker.event",
            "payload": {
                "source": "autopilot-ios",
                "method": "ios/handshake",
                "handshake_id": "hs-123",
                "occurred_at": "2026-02-20T00:00:00Z"
            }
        });

        assert_eq!(extract_ios_handshake_id(&invalid), None);
    }

    #[test]
    fn extract_desktop_handshake_ack_id_requires_proto_fields() {
        let payload = json!({
            "event_type": "worker.event",
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-123",
                "desktop_session_id": "session-42",
                "occurred_at": "2026-02-20T00:00:02Z"
            }
        });

        assert_eq!(
            extract_desktop_handshake_ack_id(&payload).as_deref(),
            Some("hs-123")
        );

        let invalid = json!({
            "event_type": "worker.event",
            "payload": {
                "source": "autopilot-desktop",
                "method": "desktop/handshake_ack",
                "handshake_id": "hs-123",
                "occurred_at": "2026-02-20T00:00:02Z"
            }
        });

        assert_eq!(extract_desktop_handshake_ack_id(&invalid), None);
    }
}
