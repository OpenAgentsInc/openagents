//! NIP-20: Command Results
//!
//! **DEPRECATED:** This NIP has been moved to NIP-01.
//!
//! NIP-20 originally defined OK and NOTICE messages for relay-to-client communication.
//! This functionality is now part of the core Nostr protocol specification (NIP-01).
//!
//! ## OK Messages
//!
//! Used to indicate acceptance or denial of an EVENT message:
//!
//! ```json
//! ["OK", "<event_id>", true, ""]
//! ["OK", "<event_id>", false, "blocked: you are banned from posting here"]
//! ```
//!
//! Format: `["OK", <event_id>, <true|false>, <message>]`
//!
//! - `event_id`: The ID of the event being responded to
//! - `true|false`: Whether the event was accepted (true) or rejected (false)
//! - `message`: Empty string on success, or machine-readable prefix + human message on failure
//!
//! ## NOTICE Messages
//!
//! Used to send human-readable messages to clients:
//!
//! ```json
//! ["NOTICE", "Connection closed due to rate limiting"]
//! ```
//!
//! Format: `["NOTICE", <message>]`
//!
//! ## Standardized Prefixes
//!
//! Machine-readable prefixes for OK and CLOSED messages:
//! - `duplicate`: Event already exists
//! - `pow`: Proof-of-work difficulty not met
//! - `blocked`: User is blocked/banned
//! - `rate-limited`: Too many requests
//! - `invalid`: Event is malformed or invalid
//! - `restricted`: Operation not allowed
//! - `mute`: User is muted
//! - `error`: Generic error (when none of the others fit)
//!
//! # Example
//!
//! ```
//! use nostr_core::nip20::{CommandResult, OkMessage, NoticeMessage};
//!
//! // Create a successful OK message
//! let ok = OkMessage::accepted("event-id-123");
//! assert!(ok.accepted);
//! assert_eq!(ok.message, "");
//!
//! // Create a rejection with a standard prefix
//! let rejected = OkMessage::blocked("event-id-456", "you are banned");
//! assert!(!rejected.accepted);
//! assert!(rejected.message.starts_with("blocked:"));
//!
//! // Create a NOTICE message
//! let notice = NoticeMessage::new("Server is under maintenance");
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during NIP-20 operations.
#[derive(Debug, Error)]
pub enum Nip20Error {
    #[error("invalid command result format")]
    InvalidFormat,

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Command result message types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandResult {
    /// OK message indicating event acceptance/rejection
    Ok(OkMessage),
    /// NOTICE message with human-readable information
    Notice(NoticeMessage),
}

/// OK message indicating event acceptance or rejection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OkMessage {
    /// The event ID being responded to
    pub event_id: String,
    /// Whether the event was accepted (true) or rejected (false)
    pub accepted: bool,
    /// Empty on success, or machine-readable prefix + human message on failure
    pub message: String,
}

impl OkMessage {
    /// Create an OK message indicating acceptance.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::OkMessage;
    ///
    /// let ok = OkMessage::accepted("event-id-123");
    /// assert!(ok.accepted);
    /// assert_eq!(ok.message, "");
    /// ```
    pub fn accepted(event_id: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            accepted: true,
            message: String::new(),
        }
    }

    /// Create an OK message indicating rejection.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::OkMessage;
    ///
    /// let ok = OkMessage::rejected("event-id", "error", "something went wrong");
    /// assert!(!ok.accepted);
    /// assert_eq!(ok.message, "error: something went wrong");
    /// ```
    pub fn rejected(event_id: impl Into<String>, prefix: &str, message: &str) -> Self {
        Self {
            event_id: event_id.into(),
            accepted: false,
            message: format!("{}: {}", prefix, message),
        }
    }

    /// Create a rejection with the "duplicate" prefix.
    pub fn duplicate(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "duplicate", message)
    }

    /// Create a rejection with the "pow" prefix.
    pub fn pow(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "pow", message)
    }

    /// Create a rejection with the "blocked" prefix.
    pub fn blocked(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "blocked", message)
    }

    /// Create a rejection with the "rate-limited" prefix.
    pub fn rate_limited(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "rate-limited", message)
    }

    /// Create a rejection with the "invalid" prefix.
    pub fn invalid(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "invalid", message)
    }

    /// Create a rejection with the "restricted" prefix.
    pub fn restricted(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "restricted", message)
    }

    /// Create a rejection with the "mute" prefix.
    pub fn mute(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "mute", message)
    }

    /// Create a rejection with the "error" prefix.
    pub fn error(event_id: impl Into<String>, message: &str) -> Self {
        Self::rejected(event_id, "error", message)
    }

    /// Get the machine-readable prefix from the message.
    ///
    /// Returns `None` if the message is empty or doesn't contain a prefix.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::OkMessage;
    ///
    /// let ok = OkMessage::blocked("id", "you are banned");
    /// assert_eq!(ok.get_prefix(), Some("blocked"));
    /// ```
    pub fn get_prefix(&self) -> Option<&str> {
        if self.message.is_empty() {
            return None;
        }
        self.message.split(':').next()
    }

    /// Get the human-readable part of the message.
    ///
    /// Returns the part after the colon and space, or the full message if no prefix exists.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::OkMessage;
    ///
    /// let ok = OkMessage::blocked("id", "you are banned");
    /// assert_eq!(ok.get_human_message(), "you are banned");
    /// ```
    pub fn get_human_message(&self) -> &str {
        if let Some(colon_pos) = self.message.find(':') {
            self.message[colon_pos + 1..].trim_start()
        } else {
            &self.message
        }
    }

    /// Serialize to JSON array format.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::OkMessage;
    ///
    /// let ok = OkMessage::accepted("event-id");
    /// let json = ok.to_json().unwrap();
    /// // ["OK", "event-id", true, ""]
    /// ```
    pub fn to_json(&self) -> Result<String, Nip20Error> {
        let array = serde_json::json!(["OK", self.event_id, self.accepted, self.message]);
        Ok(serde_json::to_string(&array)?)
    }
}

/// NOTICE message with human-readable information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeMessage {
    /// Human-readable message
    pub message: String,
}

impl NoticeMessage {
    /// Create a new NOTICE message.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::NoticeMessage;
    ///
    /// let notice = NoticeMessage::new("Connection closed");
    /// assert_eq!(notice.message, "Connection closed");
    /// ```
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Serialize to JSON array format.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip20::NoticeMessage;
    ///
    /// let notice = NoticeMessage::new("Server maintenance");
    /// let json = notice.to_json().unwrap();
    /// // ["NOTICE", "Server maintenance"]
    /// ```
    pub fn to_json(&self) -> Result<String, Nip20Error> {
        let array = serde_json::json!(["NOTICE", self.message]);
        Ok(serde_json::to_string(&array)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_accepted() {
        let ok = OkMessage::accepted("event-id-123");
        assert!(ok.accepted);
        assert_eq!(ok.event_id, "event-id-123");
        assert_eq!(ok.message, "");
    }

    #[test]
    fn test_ok_rejected() {
        let ok = OkMessage::rejected("event-id", "error", "something went wrong");
        assert!(!ok.accepted);
        assert_eq!(ok.event_id, "event-id");
        assert_eq!(ok.message, "error: something went wrong");
    }

    #[test]
    fn test_ok_duplicate() {
        let ok = OkMessage::duplicate("id", "event already exists");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("duplicate:"));
    }

    #[test]
    fn test_ok_pow() {
        let ok = OkMessage::pow("id", "difficulty not met");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("pow:"));
    }

    #[test]
    fn test_ok_blocked() {
        let ok = OkMessage::blocked("id", "you are banned");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("blocked:"));
    }

    #[test]
    fn test_ok_rate_limited() {
        let ok = OkMessage::rate_limited("id", "too many requests");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("rate-limited:"));
    }

    #[test]
    fn test_ok_invalid() {
        let ok = OkMessage::invalid("id", "malformed event");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("invalid:"));
    }

    #[test]
    fn test_ok_restricted() {
        let ok = OkMessage::restricted("id", "operation not allowed");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("restricted:"));
    }

    #[test]
    fn test_ok_mute() {
        let ok = OkMessage::mute("id", "user is muted");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("mute:"));
    }

    #[test]
    fn test_ok_error() {
        let ok = OkMessage::error("id", "generic error");
        assert!(!ok.accepted);
        assert!(ok.message.starts_with("error:"));
    }

    #[test]
    fn test_get_prefix() {
        let ok = OkMessage::blocked("id", "banned");
        assert_eq!(ok.get_prefix(), Some("blocked"));

        let ok_accepted = OkMessage::accepted("id");
        assert_eq!(ok_accepted.get_prefix(), None);
    }

    #[test]
    fn test_get_human_message() {
        let ok = OkMessage::blocked("id", "you are banned from posting");
        assert_eq!(ok.get_human_message(), "you are banned from posting");

        let ok_no_prefix = OkMessage {
            event_id: "id".to_string(),
            accepted: false,
            message: "plain message".to_string(),
        };
        assert_eq!(ok_no_prefix.get_human_message(), "plain message");
    }

    #[test]
    fn test_ok_to_json() {
        let ok = OkMessage::accepted("test-id");
        let json = ok.to_json().unwrap();
        assert!(json.contains("OK"));
        assert!(json.contains("test-id"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_ok_rejected_to_json() {
        let ok = OkMessage::blocked("test-id", "banned");
        let json = ok.to_json().unwrap();
        assert!(json.contains("OK"));
        assert!(json.contains("test-id"));
        assert!(json.contains("false"));
        assert!(json.contains("blocked"));
    }

    #[test]
    fn test_notice_new() {
        let notice = NoticeMessage::new("Connection closed");
        assert_eq!(notice.message, "Connection closed");
    }

    #[test]
    fn test_notice_to_json() {
        let notice = NoticeMessage::new("Server maintenance");
        let json = notice.to_json().unwrap();
        assert!(json.contains("NOTICE"));
        assert!(json.contains("Server maintenance"));
    }
}
