use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde_json::{Value, json};

use crate::auth::{normalize_email, normalize_verification_code};
use crate::codex_control::extract_control_request_json;
use crate::codex_worker::extract_desktop_handshake_ack_id;
use crate::command::normalize_thread_message_text;
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
        OA_CLIENT_CORE_FFI_CONTRACT_VERSION, extract_control_request_from_payload_json,
        extract_desktop_ack_id_json, normalize_email_string, normalize_message_text_string,
        normalize_verification_code_string, oa_client_core_ffi_contract_version,
        oa_client_core_normalize_email, parse_khala_frame_json,
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
