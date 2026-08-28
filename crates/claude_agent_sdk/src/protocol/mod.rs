//! Protocol types for communicating with Claude Code CLI.
//!
//! This module defines the JSONL message types exchanged over stdin/stdout
//! with the Claude Code CLI process.

mod control;
mod messages;
#[cfg(test)]
mod tests;

pub use control::*;
pub use messages::*;

use crate::error::{Error, Result as SdkResult};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Messages sent from Claude Code CLI to SDK (stdout).
///
/// Deserialize is custom so control/keepalive frames are not swallowed by
/// [`SdkMessage::Unknown`]. Serialize stays untagged (inner JSON only).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum StdoutMessage {
    /// SDK message (assistant, user, result, system, stream_event, etc.)
    Message(SdkMessage),
    /// Control request from CLI (e.g., permission request)
    ControlRequest(SdkControlRequest),
    /// Control response (to a request we sent)
    ControlResponse(SdkControlResponse),
    /// Keep-alive ping
    KeepAlive(KeepAliveMessage),
}

impl StdoutMessage {
    /// Parse one CLI stdout JSONL line.
    ///
    /// Invalid JSON becomes [`Error::UnrecognizedMessage`] with `type_name: None`.
    /// Unknown `type` strings become [`SdkMessage::Unknown`] and still succeed.
    /// Neither case implies the stream is finished.
    pub fn parse_jsonl(line: &str) -> SdkResult<Self> {
        let value: Value = serde_json::from_str(line).map_err(|_| Error::UnrecognizedMessage {
            type_name: None,
            raw: line.to_string(),
        })?;
        Ok(Self::from_wire_value(value))
    }

    /// Classify a parsed JSON object as an SDK, control, or keepalive frame.
    pub fn from_wire_value(value: Value) -> Self {
        match value.get("type").and_then(Value::as_str) {
            Some("control_request") => {
                match serde_json::from_value::<SdkControlRequest>(value.clone()) {
                    Ok(req) => StdoutMessage::ControlRequest(req),
                    Err(_) => unknown_sdk(value),
                }
            }
            Some("control_response") => {
                match serde_json::from_value::<SdkControlResponse>(value.clone()) {
                    Ok(resp) => StdoutMessage::ControlResponse(resp),
                    Err(_) => unknown_sdk(value),
                }
            }
            Some("keep_alive") => match serde_json::from_value::<KeepAliveMessage>(value.clone()) {
                Ok(ka) => StdoutMessage::KeepAlive(ka),
                Err(_) => unknown_sdk(value),
            },
            _ => StdoutMessage::Message(SdkMessage::from_wire_value(value)),
        }
    }
}

fn unknown_sdk(value: Value) -> StdoutMessage {
    let type_name = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    StdoutMessage::Message(SdkMessage::Unknown {
        type_name,
        raw: value,
    })
}

impl<'de> Deserialize<'de> for StdoutMessage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(StdoutMessage::from_wire_value(value))
    }
}

/// Parse one CLI stdout JSONL line. See [`StdoutMessage::parse_jsonl`].
pub fn parse_stdout_line(line: &str) -> SdkResult<StdoutMessage> {
    StdoutMessage::parse_jsonl(line)
}

/// Messages sent from SDK to Claude Code CLI (stdin).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StdinMessage {
    /// User message to send
    UserMessage(SdkUserMessage),
    /// Control request (e.g., interrupt, set_permission_mode)
    ControlRequest(SdkControlRequest),
    /// Control response (e.g., responding to permission request)
    ControlResponse(SdkControlResponse),
    /// Keep-alive ping
    KeepAlive(KeepAliveMessage),
}

/// Keep-alive message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAliveMessage {
    #[serde(rename = "type")]
    pub msg_type: KeepAliveType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeepAliveType {
    KeepAlive,
}
