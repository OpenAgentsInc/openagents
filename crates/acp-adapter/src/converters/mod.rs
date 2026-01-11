//! Type converters between ACP, SDK, and rlog formats
//!
//! This module provides bidirectional conversion between:
//! - ACP protocol types and codex-agent-sdk event types
//! - ACP protocol types and rlog line format

pub mod codex;
pub mod rlog;

// Re-export main conversion functions
pub use codex::thread_event_to_notifications;
pub use rlog::{notification_to_rlog_line, rlog_line_to_notification};
