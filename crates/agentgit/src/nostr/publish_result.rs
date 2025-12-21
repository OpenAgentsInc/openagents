//! Result types for event publishing with detailed error information

use serde::{Deserialize, Serialize};

/// Result of publishing an event to relays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// Event ID that was published
    pub event_id: String,
    /// Number of successful relay confirmations
    pub confirmations: usize,
    /// Total number of relays attempted
    pub relays_attempted: usize,
    /// Whether the publish succeeded (met minimum confirmation threshold)
    pub success: bool,
    /// User-friendly status message
    pub message: String,
    /// Detailed error information for failed relays
    pub failures: Vec<RelayFailure>,
}

/// Information about a failed relay publish attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayFailure {
    /// Relay URL that failed
    pub relay_url: String,
    /// Error message
    pub error: String,
    /// Error category for UI display
    pub category: ErrorCategory,
}

/// Categories of publish errors for user-friendly display
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Connection timeout
    Timeout,
    /// Relay rejected the event
    Rejected,
    /// Network connectivity issue
    Network,
    /// Authentication required
    Auth,
    /// Rate limited
    RateLimit,
    /// Unknown error
    Unknown,
}

impl ErrorCategory {
    /// Get a user-friendly description of the error category
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::Timeout => "Connection timed out",
            ErrorCategory::Rejected => "Event rejected by relay",
            ErrorCategory::Network => "Network error",
            ErrorCategory::Auth => "Authentication required",
            ErrorCategory::RateLimit => "Rate limited",
            ErrorCategory::Unknown => "Unknown error",
        }
    }

    /// Determine error category from error message
    pub fn from_error_message(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            ErrorCategory::Timeout
        } else if msg_lower.contains("rejected") || msg_lower.contains("invalid") {
            ErrorCategory::Rejected
        } else if msg_lower.contains("auth") || msg_lower.contains("authentication") {
            ErrorCategory::Auth
        } else if msg_lower.contains("rate limit") || msg_lower.contains("too many") {
            ErrorCategory::RateLimit
        } else if msg_lower.contains("connection") || msg_lower.contains("network") {
            ErrorCategory::Network
        } else {
            ErrorCategory::Unknown
        }
    }
}

impl PublishResult {
    /// Create a successful publish result
    pub fn success(event_id: String, confirmations: usize, relays_attempted: usize) -> Self {
        Self {
            event_id: event_id.clone(),
            confirmations,
            relays_attempted,
            success: true,
            message: format!(
                "Event published successfully to {}/{} relays",
                confirmations, relays_attempted
            ),
            failures: Vec::new(),
        }
    }

    /// Create a partial success result (some relays failed but minimum threshold met)
    #[allow(dead_code)]
    pub fn partial_success(
        event_id: String,
        confirmations: usize,
        relays_attempted: usize,
        failures: Vec<RelayFailure>,
    ) -> Self {
        Self {
            event_id: event_id.clone(),
            confirmations,
            relays_attempted,
            success: true,
            message: format!(
                "Event published to {}/{} relays ({} failed)",
                confirmations,
                relays_attempted,
                failures.len()
            ),
            failures,
        }
    }

    /// Create a failure result
    pub fn failure(
        event_id: String,
        confirmations: usize,
        relays_attempted: usize,
        failures: Vec<RelayFailure>,
    ) -> Self {
        let primary_error = failures
            .first()
            .map(|f| f.error.as_str())
            .unwrap_or("No relays available");

        Self {
            event_id,
            confirmations,
            relays_attempted,
            success: false,
            message: format!(
                "Failed to publish event ({}/{} relays): {}",
                confirmations, relays_attempted, primary_error
            ),
            failures,
        }
    }
}
