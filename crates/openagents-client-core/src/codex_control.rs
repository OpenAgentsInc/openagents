use serde_json::{Value, json};

pub use openagents_codex_control::{
    ControlMethod, ControlRequestEnvelope, ControlRequestParseError, RequestReplayState,
    TargetResolution, TargetResolutionError, TargetResolutionInput, WorkerReceipt, error_receipt,
    extract_control_request, extract_ios_user_message, handshake_dedupe_key,
    ios_message_dedupe_key, request_dedupe_key, resolve_request_target, success_receipt,
    terminal_receipt_dedupe_key,
};

pub fn extract_control_request_json(payload_json: &str) -> Option<String> {
    let payload = serde_json::from_str::<Value>(payload_json).ok()?;
    let envelope = match extract_control_request(&payload) {
        Ok(Some(envelope)) => envelope,
        Ok(None) => return None,
        Err(_) => return None,
    };

    let encoded = json!({
        "request_id": envelope.request_id,
        "method": envelope.method.as_str(),
        "params": envelope.params,
        "source": envelope.source,
        "request_version": envelope.request_version,
        "sent_at": envelope.sent_at,
        "session_id": envelope.session_id,
        "thread_id": envelope.thread_id,
    });

    serde_json::to_string(&encoded).ok()
}

#[cfg(test)]
mod tests {
    use super::extract_control_request_json;

    #[test]
    fn extract_control_request_json_returns_normalized_request_shape() {
        let raw = r#"{"eventType":"worker.request","payload":{"request_id":"req_1","method":"thread/list","params":{"limit":10}}}"#;

        let parsed = extract_control_request_json(raw);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap_or_else(|| unreachable!());
        assert!(parsed.contains("\"request_id\":\"req_1\""));
        assert!(parsed.contains("\"method\":\"thread/list\""));
    }
}
