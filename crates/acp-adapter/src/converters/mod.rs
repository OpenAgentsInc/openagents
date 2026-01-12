//! Type converters between ACP, SDK, and rlog formats
//!
//! This module provides bidirectional conversion between ACP protocol types
//! and rlog line format.

pub mod rlog;

// Re-export main conversion functions
pub use rlog::{notification_to_rlog_line, rlog_line_to_notification};
