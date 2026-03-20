//! Chat message classifier — pure, stateless, no async.
//!
//! Assigns a [`ChatMessageClass`] to each relay [`Event`] so the renderer
//! can decide how to display it without embedding presence-detection logic.
//!
//! Wiring into the renderer is tracked in ticket A-2.

use nostr::Event;
use serde_json::Value;

use crate::autopilot_peer_roster::AUTOPILOT_COMPUTE_PRESENCE_TYPE;

const KIND_CHANNEL_CREATE: u16 = 40;
const KIND_CHANNEL_METADATA: u16 = 41;
const KIND_CHANNEL_MESSAGE: u16 = 42;

/// High-level display category for a single relay event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMessageClass {
    /// A kind-42 message authored by a human (no recognised machine payload).
    HumanMessage,
    /// A kind-42 message whose JSON payload carries `"type": "oa.autopilot.presence.v1"`.
    PresenceEvent,
    /// A kind-41 channel-metadata update.
    SystemNotice,
    /// Anything else, unknown kinds, or parse failures — never panics.
    DebugEvent,
}

/// Classify `event` into a [`ChatMessageClass`].
///
/// # Guarantees
/// - Pure: no side effects, no I/O, no state mutation.
/// - Deterministic: same `event` always yields the same result.
/// - Infallible: malformed content returns [`ChatMessageClass::DebugEvent`] or
///   [`ChatMessageClass::HumanMessage`]; never panics.
pub fn classify(event: &Event) -> ChatMessageClass {
    match event.kind {
        KIND_CHANNEL_MESSAGE => classify_channel_message(event),
        KIND_CHANNEL_METADATA => ChatMessageClass::SystemNotice,
        // kind-40 (channel create) is fetched by the lane but not displayed.
        _ => ChatMessageClass::DebugEvent,
    }
}

fn classify_channel_message(event: &Event) -> ChatMessageClass {
    if is_presence_content(&event.content) {
        ChatMessageClass::PresenceEvent
    } else {
        ChatMessageClass::HumanMessage
    }
}

/// Returns `true` when `content` is a JSON object with
/// `"type": "oa.autopilot.presence.v1"`.
///
/// Uses the same type constant as `autopilot_peer_roster` to avoid drift.
fn is_presence_content(content: &str) -> bool {
    match serde_json::from_str::<Value>(content.trim()) {
        Ok(Value::Object(map)) => map
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == AUTOPILOT_COMPUTE_PRESENCE_TYPE),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: u16, content: &str) -> Event {
        Event {
            id: "a".repeat(64),
            pubkey: "b".repeat(64),
            created_at: 1_700_000_000,
            kind,
            tags: vec![],
            content: content.to_string(),
            sig: "c".repeat(128),
        }
    }

    fn presence_json() -> String {
        format!(
            r#"{{"type":"{}","pubkey":"{}","mode":"provider-online","capabilities":[]}}"#,
            AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "d".repeat(64),
        )
    }

    // HumanMessage cases

    #[test]
    fn kind42_plain_text_is_human_message() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_MESSAGE, "hello world")),
            ChatMessageClass::HumanMessage
        );
    }

    #[test]
    fn kind42_empty_content_is_human_message() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_MESSAGE, "")),
            ChatMessageClass::HumanMessage
        );
    }

    #[test]
    fn kind42_unrelated_json_is_human_message() {
        assert_eq!(
            classify(&make_event(
                KIND_CHANNEL_MESSAGE,
                r#"{"type":"something.else","data":1}"#
            )),
            ChatMessageClass::HumanMessage
        );
    }

    // PresenceEvent cases

    #[test]
    fn kind42_presence_json_is_presence_event() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_MESSAGE, &presence_json())),
            ChatMessageClass::PresenceEvent
        );
    }

    #[test]
    fn kind42_presence_json_with_surrounding_whitespace_is_presence_event() {
        let padded = format!("  {}  ", presence_json());
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_MESSAGE, &padded)),
            ChatMessageClass::PresenceEvent
        );
    }

    // SystemNotice cases

    #[test]
    fn kind41_is_system_notice() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_METADATA, r#"{"name":"ops-channel"}"#)),
            ChatMessageClass::SystemNotice
        );
    }

    // DebugEvent cases

    #[test]
    fn kind40_is_debug_event() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_CREATE, "")),
            ChatMessageClass::DebugEvent
        );
    }

    #[test]
    fn unknown_kind_is_debug_event() {
        assert_eq!(
            classify(&make_event(9999, "some content")),
            ChatMessageClass::DebugEvent
        );
    }

    // Robustness — never panics

    #[test]
    fn kind42_malformed_json_does_not_panic_and_is_human_message() {
        assert_eq!(
            classify(&make_event(KIND_CHANNEL_MESSAGE, "{not valid json {{{{{")),
            ChatMessageClass::HumanMessage
        );
    }

    #[test]
    fn is_presence_content_rejects_json_array() {
        assert!(!is_presence_content(r#"["foo","bar"]"#));
    }

    #[test]
    fn is_presence_content_rejects_empty_string() {
        assert!(!is_presence_content(""));
        assert!(!is_presence_content("   "));
    }

    // Determinism

    #[test]
    fn classify_is_deterministic() {
        let ev = make_event(KIND_CHANNEL_MESSAGE, &presence_json());
        let first = classify(&ev);
        for _ in 0..10 {
            assert_eq!(classify(&ev), first);
        }
    }
}
