use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::{normalize_email, normalize_verification_code};
use crate::codex_control::{extract_control_request_json, ControlMethod};
use crate::codex_worker::extract_desktop_handshake_ack_id;
use crate::command::normalize_thread_message_text;
use crate::ios_codex_state::{
    RuntimeCodexControlCoordinator, RuntimeCodexControlReceipt, RuntimeCodexControlReceiptOutcome,
    RuntimeCodexControlRequestState, RuntimeCodexControlRequestTracker,
    RuntimeCodexWorkerActionRequest,
};
use crate::ios_khala_session::{IosKhalaSession, SessionStep};
use crate::khala_protocol::parse_phoenix_frame;

pub const OA_CLIENT_CORE_FFI_CONTRACT_VERSION: u32 = 1;

fn with_c_string_input(input: *const c_char) -> Option<String> {
    if input.is_null() {
        return None;
    }

    // SAFETY: Caller guarantees `input` points to a valid NUL-terminated string.
    let c_str = unsafe { CStr::from_ptr(input) };
    c_str.to_str().ok().map(ToString::to_string)
}

fn into_raw_c_string(output: String) -> *mut c_char {
    CString::new(output)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn encode_json<T: serde::Serialize>(value: &T) -> Option<String> {
    serde_json::to_string(value).ok()
}

pub fn normalize_email_string(input: &str) -> Option<String> {
    normalize_email(input).ok()
}

pub fn normalize_verification_code_string(input: &str) -> Option<String> {
    normalize_verification_code(input).ok()
}

pub fn normalize_message_text_string(input: &str) -> Option<String> {
    normalize_thread_message_text(input).ok()
}

pub fn parse_khala_frame_json(raw_frame: &str) -> Option<String> {
    let frame = parse_phoenix_frame(raw_frame)?;
    let encoded = json!({
        "join_ref": frame.join_ref,
        "reference": frame.reference,
        "topic": frame.topic,
        "event": frame.event,
        "payload": frame.payload,
    });

    serde_json::to_string(&encoded).ok()
}

pub fn extract_desktop_ack_id_json(payload_json: &str) -> Option<String> {
    let payload = serde_json::from_str::<Value>(payload_json).ok()?;
    extract_desktop_handshake_ack_id(&payload)
}

pub fn extract_control_request_from_payload_json(payload_json: &str) -> Option<String> {
    extract_control_request_json(payload_json)
}

pub fn khala_session_step_json(step: &SessionStep) -> Option<String> {
    encode_json(step)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum ControlCoordinatorCommand {
    Enqueue {
        worker_id: String,
        request: ControlRequestPayload,
        occurred_at: String,
    },
    MarkRunning {
        request_id: String,
        occurred_at: String,
    },
    Requeue {
        request_id: String,
        message: Option<String>,
        occurred_at: String,
    },
    MarkDispatchError {
        request_id: String,
        code: String,
        message: String,
        retryable: bool,
        occurred_at: String,
    },
    MarkTimeout {
        request_id: String,
        occurred_at: String,
    },
    Reconcile {
        worker_id: String,
        receipt: ControlReceiptPayload,
    },
    Snapshots,
    Queued,
}

#[derive(Debug, Clone, Deserialize)]
struct ControlRequestPayload {
    request_id: String,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    request_version: Option<String>,
    #[serde(default)]
    sent_at: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    thread_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ControlReceiptPayload {
    request_id: String,
    method: String,
    #[serde(default)]
    occurred_at: Option<String>,
    outcome: ControlReceiptOutcomePayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ControlReceiptOutcomePayload {
    Success {
        response: Option<Value>,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
        #[serde(default)]
        details: Option<Value>,
    },
}

#[derive(Debug, Serialize)]
struct ControlCoordinatorResult {
    tracker: Option<ControlRequestTrackerOutput>,
    snapshots: Vec<ControlRequestTrackerOutput>,
    queued: Vec<ControlRequestTrackerOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct ControlRequestOutput {
    request_id: String,
    method: String,
    params: Value,
    request_version: Option<String>,
    sent_at: Option<String>,
    source: Option<String>,
    session_id: Option<String>,
    thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ControlRequestTrackerOutput {
    worker_id: String,
    request: ControlRequestOutput,
    created_at: String,
    last_updated_at: String,
    state: String,
    sent_at: Option<String>,
    receipt_at: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    retryable: bool,
    response: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ControlReceiptEnvelopeOutput {
    event_type: String,
    receipt: ControlReceiptPayload,
}

#[derive(Debug, Deserialize)]
struct ControlSuccessContextInput {
    method: String,
    #[serde(default)]
    response: Option<Value>,
    #[serde(default)]
    fallback_thread_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ControlSuccessContextOutput {
    thread_id: Option<String>,
    turn_id: Option<String>,
    clear_turn: bool,
}

fn control_state_string(state: RuntimeCodexControlRequestState) -> &'static str {
    match state {
        RuntimeCodexControlRequestState::Queued => "queued",
        RuntimeCodexControlRequestState::Running => "running",
        RuntimeCodexControlRequestState::Success => "success",
        RuntimeCodexControlRequestState::Error => "error",
    }
}

fn request_to_output(request: &RuntimeCodexWorkerActionRequest) -> ControlRequestOutput {
    ControlRequestOutput {
        request_id: request.request_id.clone(),
        method: request.method.as_str().to_string(),
        params: request.params.clone(),
        request_version: request.request_version.clone(),
        sent_at: request.sent_at.clone(),
        source: request.source.clone(),
        session_id: request.session_id.clone(),
        thread_id: request.thread_id.clone(),
    }
}

fn tracker_to_output(tracker: RuntimeCodexControlRequestTracker) -> ControlRequestTrackerOutput {
    ControlRequestTrackerOutput {
        worker_id: tracker.worker_id,
        request: request_to_output(&tracker.request),
        created_at: tracker.created_at,
        last_updated_at: tracker.last_updated_at,
        state: control_state_string(tracker.state).to_string(),
        sent_at: tracker.sent_at,
        receipt_at: tracker.receipt_at,
        error_code: tracker.error_code,
        error_message: tracker.error_message,
        retryable: tracker.retryable,
        response: tracker.response,
    }
}

fn request_from_payload(payload: ControlRequestPayload) -> Option<RuntimeCodexWorkerActionRequest> {
    let method = ControlMethod::from_raw(payload.method.as_str())?;
    let params = match payload.params {
        Value::Null => json!({}),
        value => value,
    };

    Some(RuntimeCodexWorkerActionRequest {
        request_id: payload.request_id,
        method,
        params,
        request_version: payload.request_version,
        sent_at: payload.sent_at,
        source: payload.source,
        session_id: payload.session_id,
        thread_id: payload.thread_id,
    })
}

fn receipt_from_payload(payload: ControlReceiptPayload) -> RuntimeCodexControlReceipt {
    let outcome = match payload.outcome {
        ControlReceiptOutcomePayload::Success { response } => {
            RuntimeCodexControlReceiptOutcome::Success { response }
        }
        ControlReceiptOutcomePayload::Error {
            code,
            message,
            retryable,
            details,
        } => RuntimeCodexControlReceiptOutcome::Error {
            code,
            message,
            retryable,
            details,
        },
    };

    RuntimeCodexControlReceipt {
        request_id: payload.request_id,
        method: payload.method,
        occurred_at: payload.occurred_at,
        outcome,
    }
}

fn apply_control_coordinator_command_json(
    coordinator: &mut RuntimeCodexControlCoordinator,
    command_json: &str,
) -> Option<String> {
    let command = serde_json::from_str::<ControlCoordinatorCommand>(command_json).ok()?;
    let tracker = match command {
        ControlCoordinatorCommand::Enqueue {
            worker_id,
            request,
            occurred_at,
        } => {
            let request = request_from_payload(request)?;
            Some(coordinator.enqueue(worker_id, request, occurred_at))
        }
        ControlCoordinatorCommand::MarkRunning {
            request_id,
            occurred_at,
        } => coordinator.mark_running(request_id.as_str(), occurred_at),
        ControlCoordinatorCommand::Requeue {
            request_id,
            message,
            occurred_at,
        } => coordinator.requeue(
            request_id.as_str(),
            normalized_owned(message.as_deref()),
            occurred_at,
        ),
        ControlCoordinatorCommand::MarkDispatchError {
            request_id,
            code,
            message,
            retryable,
            occurred_at,
        } => coordinator.mark_dispatch_error(
            request_id.as_str(),
            code,
            message,
            retryable,
            occurred_at,
        ),
        ControlCoordinatorCommand::MarkTimeout {
            request_id,
            occurred_at,
        } => coordinator.mark_timeout(request_id.as_str(), occurred_at),
        ControlCoordinatorCommand::Reconcile { worker_id, receipt } => {
            coordinator.reconcile(worker_id.as_str(), receipt_from_payload(receipt))
        }
        ControlCoordinatorCommand::Snapshots => None,
        ControlCoordinatorCommand::Queued => None,
    };

    let snapshots = coordinator
        .snapshots()
        .into_iter()
        .map(tracker_to_output)
        .collect();
    let queued = coordinator
        .queued_requests()
        .into_iter()
        .map(tracker_to_output)
        .collect();
    encode_json(&ControlCoordinatorResult {
        tracker: tracker.map(tracker_to_output),
        snapshots,
        queued,
    })
}

fn normalized_owned(value: Option<&str>) -> Option<String> {
    let value = value?;
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn first_non_empty_object_string(
    object: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let normalized = value.trim();
            if !normalized.is_empty() {
                return Some(normalized.to_string());
            }
        }
    }
    None
}

fn decode_control_receipt_envelope(payload: &Value) -> Option<ControlReceiptEnvelopeOutput> {
    let object = payload.as_object()?;
    let event_type = first_non_empty_object_string(object, &["eventType", "event_type"])?;
    if event_type != "worker.response" && event_type != "worker.error" {
        return None;
    }

    let worker_payload = object.get("payload")?.as_object()?;
    let request_id = first_non_empty_object_string(worker_payload, &["request_id", "requestId"])?;
    let method = first_non_empty_object_string(worker_payload, &["method"])?;
    let occurred_at = first_non_empty_object_string(worker_payload, &["occurred_at"]);

    let outcome = if event_type == "worker.response" {
        ControlReceiptOutcomePayload::Success {
            response: worker_payload.get("response").cloned(),
        }
    } else {
        ControlReceiptOutcomePayload::Error {
            code: first_non_empty_object_string(worker_payload, &["code"])
                .unwrap_or_else(|| "internal_error".to_string()),
            message: first_non_empty_object_string(worker_payload, &["message"])
                .unwrap_or_else(|| "control request failed".to_string()),
            retryable: worker_payload
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            details: worker_payload.get("details").cloned(),
        }
    };

    Some(ControlReceiptEnvelopeOutput {
        event_type,
        receipt: ControlReceiptPayload {
            request_id,
            method,
            occurred_at,
            outcome,
        },
    })
}

fn decode_control_receipt_json(payload_json: &str) -> Option<String> {
    let payload = serde_json::from_str::<Value>(payload_json).ok()?;
    let envelope = decode_control_receipt_envelope(&payload)?;
    encode_json(&envelope)
}

fn extract_thread_id_from_control_response(response: Option<&Value>) -> Option<String> {
    let object = response.and_then(Value::as_object)?;

    if let Some(thread_id) = first_non_empty_object_string(object, &["thread_id", "threadId"]) {
        return Some(thread_id);
    }

    if let Some(thread) = object.get("thread").and_then(Value::as_object) {
        if let Some(thread_id) = first_non_empty_object_string(thread, &["id"]) {
            return Some(thread_id);
        }
    }

    if let Some(turn) = object.get("turn").and_then(Value::as_object) {
        if let Some(thread_id) = first_non_empty_object_string(turn, &["thread_id", "threadId"]) {
            return Some(thread_id);
        }
    }

    None
}

fn extract_turn_id_from_control_response(response: Option<&Value>) -> Option<String> {
    let object = response.and_then(Value::as_object)?;

    if let Some(turn_id) = first_non_empty_object_string(object, &["turn_id", "turnId"]) {
        return Some(turn_id);
    }

    if let Some(turn) = object.get("turn").and_then(Value::as_object) {
        if let Some(turn_id) = first_non_empty_object_string(turn, &["id"]) {
            return Some(turn_id);
        }
    }

    None
}

fn extract_control_success_context_json(input_json: &str) -> Option<String> {
    let input = serde_json::from_str::<ControlSuccessContextInput>(input_json).ok()?;
    let method = ControlMethod::from_raw(input.method.as_str())?;
    let response = input.response.as_ref();
    let fallback_thread_id = normalized_owned(input.fallback_thread_id.as_deref());

    let context = match method {
        ControlMethod::ThreadStart | ControlMethod::ThreadResume => ControlSuccessContextOutput {
            thread_id: extract_thread_id_from_control_response(response).or(fallback_thread_id),
            turn_id: None,
            clear_turn: false,
        },
        ControlMethod::TurnStart => ControlSuccessContextOutput {
            thread_id: extract_thread_id_from_control_response(response).or(fallback_thread_id),
            turn_id: extract_turn_id_from_control_response(response),
            clear_turn: false,
        },
        ControlMethod::TurnInterrupt => ControlSuccessContextOutput {
            thread_id: extract_thread_id_from_control_response(response).or(fallback_thread_id),
            turn_id: None,
            clear_turn: true,
        },
        ControlMethod::ThreadList | ControlMethod::ThreadRead => ControlSuccessContextOutput {
            thread_id: extract_thread_id_from_control_response(response),
            turn_id: None,
            clear_turn: false,
        },
    };

    encode_json(&context)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_normalize_email(input: *const c_char) -> *mut c_char {
    let Some(input) = with_c_string_input(input) else {
        return std::ptr::null_mut();
    };

    normalize_email_string(&input)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_normalize_verification_code(
    input: *const c_char,
) -> *mut c_char {
    let Some(input) = with_c_string_input(input) else {
        return std::ptr::null_mut();
    };

    normalize_verification_code_string(&input)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_normalize_message_text(
    input: *const c_char,
) -> *mut c_char {
    let Some(input) = with_c_string_input(input) else {
        return std::ptr::null_mut();
    };

    normalize_message_text_string(&input)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_extract_desktop_handshake_ack_id(
    payload_json: *const c_char,
) -> *mut c_char {
    let Some(payload_json) = with_c_string_input(payload_json) else {
        return std::ptr::null_mut();
    };

    extract_desktop_ack_id_json(&payload_json)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_extract_control_request(
    payload_json: *const c_char,
) -> *mut c_char {
    let Some(payload_json) = with_c_string_input(payload_json) else {
        return std::ptr::null_mut();
    };

    extract_control_request_from_payload_json(&payload_json)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_parse_khala_frame(raw_frame: *const c_char) -> *mut c_char {
    let Some(raw_frame) = with_c_string_input(raw_frame) else {
        return std::ptr::null_mut();
    };

    parse_khala_frame_json(&raw_frame)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_create(
    worker_id: *const c_char,
    worker_events_topic: *const c_char,
    resume_after: u64,
) -> *mut IosKhalaSession {
    let Some(worker_id) = with_c_string_input(worker_id) else {
        return std::ptr::null_mut();
    };
    let Some(worker_events_topic) = with_c_string_input(worker_events_topic) else {
        return std::ptr::null_mut();
    };

    Box::into_raw(Box::new(IosKhalaSession::new(
        worker_id,
        worker_events_topic,
        resume_after,
    )))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_start(
    session: *mut IosKhalaSession,
) -> *mut c_char {
    if session.is_null() {
        return std::ptr::null_mut();
    }
    let session = unsafe { &mut *session };
    let step = session.start();
    khala_session_step_json(&step)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_on_frame(
    session: *mut IosKhalaSession,
    raw_frame: *const c_char,
) -> *mut c_char {
    if session.is_null() {
        return std::ptr::null_mut();
    }
    let Some(raw_frame) = with_c_string_input(raw_frame) else {
        return std::ptr::null_mut();
    };
    let session = unsafe { &mut *session };
    let step = session.handle_frame_raw(&raw_frame);
    khala_session_step_json(&step)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_heartbeat(
    session: *mut IosKhalaSession,
) -> *mut c_char {
    if session.is_null() {
        return std::ptr::null_mut();
    }
    let session = unsafe { &mut *session };
    match session.heartbeat_frame() {
        Some(frame) => {
            encode_json(&json!({ "frame": frame, "watermark": session.latest_watermark() }))
                .map(into_raw_c_string)
                .unwrap_or(std::ptr::null_mut())
        }
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_latest_watermark(
    session: *const IosKhalaSession,
) -> u64 {
    if session.is_null() {
        return 0;
    }
    let session = unsafe { &*session };
    session.latest_watermark()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_khala_session_free(session: *mut IosKhalaSession) {
    if session.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(session) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_control_coordinator_create(
) -> *mut RuntimeCodexControlCoordinator {
    Box::into_raw(Box::new(RuntimeCodexControlCoordinator::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_control_coordinator_apply(
    coordinator: *mut RuntimeCodexControlCoordinator,
    command_json: *const c_char,
) -> *mut c_char {
    if coordinator.is_null() {
        return std::ptr::null_mut();
    }
    let Some(command_json) = with_c_string_input(command_json) else {
        return std::ptr::null_mut();
    };
    let coordinator = unsafe { &mut *coordinator };
    apply_control_coordinator_command_json(coordinator, &command_json)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_control_coordinator_free(
    coordinator: *mut RuntimeCodexControlCoordinator,
) {
    if coordinator.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(coordinator) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_decode_control_receipt(
    payload_json: *const c_char,
) -> *mut c_char {
    let Some(payload_json) = with_c_string_input(payload_json) else {
        return std::ptr::null_mut();
    };
    decode_control_receipt_json(&payload_json)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_extract_control_success_context(
    input_json: *const c_char,
) -> *mut c_char {
    let Some(input_json) = with_c_string_input(input_json) else {
        return std::ptr::null_mut();
    };
    extract_control_success_context_json(&input_json)
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn oa_client_core_free_string(raw: *mut c_char) {
    if raw.is_null() {
        return;
    }

    // SAFETY: `raw` must originate from `CString::into_raw` in this crate.
    let _ = unsafe { CString::from_raw(raw) };
}

#[unsafe(no_mangle)]
pub extern "C" fn oa_client_core_ffi_contract_version() -> u32 {
    OA_CLIENT_CORE_FFI_CONTRACT_VERSION
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::ptr;

    use super::{
        apply_control_coordinator_command_json, decode_control_receipt_json,
        extract_control_request_from_payload_json, extract_control_success_context_json,
        extract_desktop_ack_id_json, normalize_email_string, normalize_message_text_string,
        normalize_verification_code_string, oa_client_core_ffi_contract_version,
        oa_client_core_normalize_email, parse_khala_frame_json, RuntimeCodexControlCoordinator,
        OA_CLIENT_CORE_FFI_CONTRACT_VERSION,
    };

    #[test]
    fn ffi_helpers_normalize_auth_and_message_inputs() {
        assert_eq!(
            normalize_email_string("  ChrIS@OpenAgents.com "),
            Some("chris@openagents.com".to_string())
        );
        assert_eq!(
            normalize_verification_code_string("Code: 123 456."),
            Some("123456".to_string())
        );
        assert_eq!(
            normalize_message_text_string("  who are you?  "),
            Some("who are you?".to_string())
        );
        assert_eq!(normalize_message_text_string("   "), None);
    }

    #[test]
    fn ffi_helpers_extract_desktop_ack_id() {
        let payload = r#"{"eventType":"worker.event","payload":{"source":"autopilot-desktop","method":"desktop/handshake_ack","handshake_id":"hs-42","desktop_session_id":"session-1","occurred_at":"2026-02-20T00:00:02Z"}}"#;
        assert_eq!(
            extract_desktop_ack_id_json(payload).as_deref(),
            Some("hs-42")
        );
    }

    #[test]
    fn ffi_helpers_extract_control_request() {
        let payload = r#"{"eventType":"worker.request","payload":{"request_id":"req_42","method":"thread/list","params":{"limit":5}}}"#;
        let parsed = extract_control_request_from_payload_json(payload);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap_or_else(|| unreachable!());
        assert!(parsed.contains("\"request_id\":\"req_42\""));
        assert!(parsed.contains("\"method\":\"thread/list\""));
    }

    #[test]
    fn ffi_helpers_parse_khala_frame_json() {
        let raw = r#"["1","2","sync:v1","sync:heartbeat",{"watermarks":[{"topic":"runtime.codex_worker_events","watermark":33}]}]"#;
        let parsed = parse_khala_frame_json(raw).expect("frame should parse");
        assert!(parsed.contains("\"sync:v1\""));
        assert!(parsed.contains("\"sync:heartbeat\""));
    }

    #[test]
    fn ffi_helpers_control_coordinator_queue_running_timeout_paths() {
        let mut coordinator = RuntimeCodexControlCoordinator::default();
        let enqueue = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"enqueue","worker_id":"desktopw:shared","occurred_at":"2026-02-22T00:00:00Z","request":{"request_id":"req_1","method":"turn/start","params":{"thread_id":"thread-1","text":"hello"},"request_version":"v1","sent_at":"2026-02-22T00:00:00Z","source":"autopilot-ios","session_id":"session-1","thread_id":"thread-1"}}"#,
        )
        .expect("enqueue should succeed");
        assert!(enqueue.contains("\"state\":\"queued\""));

        let running = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"mark_running","request_id":"req_1","occurred_at":"2026-02-22T00:00:01Z"}"#,
        )
        .expect("running should succeed");
        assert!(running.contains("\"state\":\"running\""));

        let timeout = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"mark_timeout","request_id":"req_1","occurred_at":"2026-02-22T00:01:31Z"}"#,
        )
        .expect("timeout should succeed");
        assert!(timeout.contains("\"state\":\"error\""));
        assert!(timeout.contains("\"error_code\":\"timeout\""));
    }

    #[test]
    fn ffi_helpers_control_coordinator_reconcile_dedupes_terminal_receipts() {
        let mut coordinator = RuntimeCodexControlCoordinator::default();
        let _ = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"enqueue","worker_id":"desktopw:shared","occurred_at":"2026-02-22T00:00:00Z","request":{"request_id":"req_2","method":"thread/start","params":{},"request_version":"v1","sent_at":"2026-02-22T00:00:00Z","source":"autopilot-ios","session_id":"session-1","thread_id":null}}"#,
        );
        let _ = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"mark_running","request_id":"req_2","occurred_at":"2026-02-22T00:00:01Z"}"#,
        );

        let first = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"reconcile","worker_id":"desktopw:shared","receipt":{"request_id":"req_2","method":"thread/start","occurred_at":"2026-02-22T00:00:02Z","outcome":{"kind":"success","response":{"thread_id":"thread-xyz"}}}}"#,
        )
        .expect("first reconcile should succeed");
        assert!(first.contains("\"state\":\"success\""));

        let duplicate = apply_control_coordinator_command_json(
            &mut coordinator,
            r#"{"op":"reconcile","worker_id":"desktopw:shared","receipt":{"request_id":"req_2","method":"thread/start","occurred_at":"2026-02-22T00:00:03Z","outcome":{"kind":"success","response":{"thread_id":"thread-xyz"}}}}"#,
        )
        .expect("duplicate reconcile should still return snapshots");
        assert!(duplicate.contains("\"tracker\":null"));
    }

    #[test]
    fn ffi_helpers_decode_control_receipt_json() {
        let success = decode_control_receipt_json(
            r#"{"eventType":"worker.response","payload":{"request_id":"req_3","method":"thread/start","response":{"thread_id":"thread-a"},"occurred_at":"2026-02-22T00:00:03Z"}}"#,
        )
        .expect("should parse success receipt");
        assert!(success.contains("\"event_type\":\"worker.response\""));
        assert!(success.contains("\"kind\":\"success\""));

        let failure = decode_control_receipt_json(
            r#"{"eventType":"worker.error","payload":{"request_id":"req_3","method":"thread/start","code":"invalid_request","message":"bad params","retryable":false,"occurred_at":"2026-02-22T00:00:04Z"}}"#,
        )
        .expect("should parse error receipt");
        assert!(failure.contains("\"event_type\":\"worker.error\""));
        assert!(failure.contains("\"kind\":\"error\""));
    }

    #[test]
    fn ffi_helpers_extract_control_success_context_json() {
        let start = extract_control_success_context_json(
            r#"{"method":"turn/start","response":{"thread":{"id":"thread-1"},"turn":{"id":"turn-1"}},"fallback_thread_id":"thread-fallback"}"#,
        )
        .expect("turn/start context should parse");
        assert!(start.contains("\"thread_id\":\"thread-1\""));
        assert!(start.contains("\"turn_id\":\"turn-1\""));
        assert!(start.contains("\"clear_turn\":false"));

        let interrupt = extract_control_success_context_json(
            r#"{"method":"turn/interrupt","response":{"status":"interrupted"},"fallback_thread_id":"thread-fallback"}"#,
        )
        .expect("turn/interrupt context should parse");
        assert!(interrupt.contains("\"thread_id\":\"thread-fallback\""));
        assert!(interrupt.contains("\"clear_turn\":true"));
    }

    #[test]
    fn ffi_contract_version_export_is_stable() {
        assert_eq!(OA_CLIENT_CORE_FFI_CONTRACT_VERSION, 1);
        assert_eq!(
            oa_client_core_ffi_contract_version(),
            OA_CLIENT_CORE_FFI_CONTRACT_VERSION
        );
    }

    #[test]
    fn ffi_returns_null_for_invalid_pointer_input() {
        // SAFETY: null input pointer is intentionally exercised and checked for null output.
        let result = unsafe { oa_client_core_normalize_email(ptr::null()) };
        assert!(result.is_null());

        let embedded_nul = CString::new("test\0value").err();
        assert!(embedded_nul.is_some());
    }
}
