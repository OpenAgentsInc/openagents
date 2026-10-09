//! The Open Responses WebSocket transport's messages (spec 2026-04-24,
//! "WebSocket Transport"): what a client sends and the error envelope.
//!
//! A client starts each turn with a `response.create` message: the usual
//! request body plus `"type": "response.create"`, without the HTTP-only
//! `stream`, `stream_options`, and `background`. The server answers with
//! the same event objects a stream carries, one JSON message each, and
//! answers a failure with `{"type": "error", "status": ..., "error":
//! {"code", "message", "param"}}`. One response at a time per connection;
//! a connection lasts at most [`CONNECTION_LIMIT`].

use std::time::Duration;

use serde_json::{Value, json};

use crate::error::{ApiError, ErrorType};
use crate::request::CreateResponse;

/// How long one connection may stay open.
pub const CONNECTION_LIMIT: Duration = Duration::from_secs(60 * 60);

/// The error code sent when a connection reaches [`CONNECTION_LIMIT`].
pub const LIMIT_CODE: &str = "websocket_connection_limit_reached";

/// The request in a client message.
///
/// # Errors
///
/// `400 invalid_request` for a message that is not JSON, not
/// `response.create`, carries an HTTP-only field, or is not a valid
/// request.
pub fn parse_create(text: &str) -> Result<CreateResponse, ApiError> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| {
        ApiError::invalid_request(
            "type",
            "Each message must be a JSON `response.create` event.",
        )
    })?;
    let Some(fields) = value.as_object_mut() else {
        return Err(ApiError::invalid_request(
            "type",
            "Each message must be a JSON `response.create` event.",
        ));
    };
    match fields.shift_remove("type").as_ref().and_then(Value::as_str) {
        Some("response.create") => {}
        Some(other) => {
            let shown: String = other.chars().take(60).collect();
            return Err(ApiError::invalid_request(
                "type",
                format!("Unknown message type `{shown}`; send `response.create`."),
            ));
        }
        None => {
            return Err(ApiError::invalid_request(
                "type",
                "Each message needs `\"type\": \"response.create\"`.",
            ));
        }
    }
    for field in ["stream", "stream_options", "background"] {
        if fields.contains_key(field) {
            return Err(ApiError::invalid_request(
                field,
                format!("`{field}` is an HTTP field; leave it out on a WebSocket."),
            ));
        }
    }
    serde_json::from_value(value).map_err(|why| {
        ApiError::invalid_request("body", format!("The message isn't a valid request: {why}"))
    })
}

/// The error envelope for `error`.
#[must_use]
pub fn error_envelope(error: &ApiError) -> String {
    let code = error
        .code
        .clone()
        .unwrap_or_else(|| error.kind.as_str().to_owned());
    json!({
        "type": "error",
        "status": error.status(),
        "error": {
            "type": error.kind,
            "code": code,
            "message": error.message,
            "param": error.param,
        },
    })
    .to_string()
}

/// The error a connection gets when it reaches [`CONNECTION_LIMIT`].
#[must_use]
pub fn limit_reached() -> ApiError {
    ApiError::new(
        ErrorType::InvalidRequest,
        "This connection reached its 60-minute limit. Open a new one.",
    )
    .with_code(LIMIT_CODE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_messages_parse_and_http_fields_are_refused() {
        let request = parse_create(
            r#"{"type": "response.create", "model": "m", "store": false, "input": "hi"}"#,
        )
        .unwrap();
        assert_eq!(request.model.as_deref(), Some("m"));
        assert!(request.stream.is_none());
        for bad in [
            r#"{"type": "response.create", "model": "m", "stream": true}"#,
            r#"{"type": "response.cancel"}"#,
            r#"{"model": "m"}"#,
            "not json",
            "[1]",
        ] {
            assert_eq!(parse_create(bad).unwrap_err().status(), 400, "{bad}");
        }
    }

    #[test]
    fn the_error_envelope_has_status_and_code() {
        let envelope: Value = serde_json::from_str(&error_envelope(
            &crate::session::previous_not_found("resp_abc"),
        ))
        .unwrap();
        assert_eq!(envelope["type"], "error");
        assert_eq!(envelope["status"], 400);
        assert_eq!(envelope["error"]["code"], "previous_response_not_found");
        assert_eq!(envelope["error"]["param"], "previous_response_id");
        let limit: Value = serde_json::from_str(&error_envelope(&limit_reached())).unwrap();
        assert_eq!(limit["error"]["code"], LIMIT_CODE);
    }
}
