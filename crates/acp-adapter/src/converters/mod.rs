//! Type converters between ACP, SDK, and rlog formats
//!
//! This module provides bidirectional conversion between:
//! - ACP protocol types and claude-agent-sdk message types
//! - ACP protocol types and codex-agent-sdk event types
//! - ACP protocol types and rlog line format

pub mod acp_to_sdk;
pub mod codex;
pub mod rlog;
pub mod sdk_to_acp;

// Re-export main conversion functions
pub use acp_to_sdk::notification_to_sdk_message;
pub use codex::thread_event_to_notifications;
pub use rlog::{notification_to_rlog_line, rlog_line_to_notification};
pub use sdk_to_acp::sdk_message_to_notification;
