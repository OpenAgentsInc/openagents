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

use serde::{Deserialize, Serialize};

/// Messages sent from Claude Code CLI to SDK (stdout).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
