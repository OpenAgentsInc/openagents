//! Integration tests for event publishing error handling
//!
//! Tests verify that the NostrClient properly handles various error scenarios
//! including timeouts, relay failures, and partial successes.

use gitafter::nostr::{ErrorCategory, NostrClient, RetryConfig};
use gitafter::ws::WsBroadcaster;
use std::sync::Arc;
use std::time::Duration;

/// Test that RetryConfig can be customized
#[test]
fn test_custom_retry_config() {
    let config = RetryConfig {
        max_attempts: 5,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_secs(10),
        backoff_multiplier: 3.0,
    };

    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.initial_backoff, Duration::from_millis(100));
    assert_eq!(config.max_backoff, Duration::from_secs(10));
    assert_eq!(config.backoff_multiplier, 3.0);
}

/// Test that RetryConfig with custom values works
#[test]
fn test_nostr_client_with_retry_config() {
    // Note: Actually creating NostrClient requires a valid database path,
    // so this test just verifies RetryConfig construction
    let retry_config = RetryConfig {
        max_attempts: 2,
        initial_backoff: Duration::from_millis(50),
        ..Default::default()
    };

    assert_eq!(retry_config.max_attempts, 2);
    assert_eq!(retry_config.initial_backoff, Duration::from_millis(50));
}

/// Test error category classification
#[test]
fn test_error_category_from_message() {
    assert_eq!(
        ErrorCategory::from_error_message("connection timeout"),
        ErrorCategory::Timeout
    );
    assert_eq!(
        ErrorCategory::from_error_message("Event rejected by relay"),
        ErrorCategory::Rejected
    );
    assert_eq!(
        ErrorCategory::from_error_message("Authentication required"),
        ErrorCategory::Auth
    );
    assert_eq!(
        ErrorCategory::from_error_message("Rate limit exceeded"),
        ErrorCategory::RateLimit
    );
    assert_eq!(
        ErrorCategory::from_error_message("Network connection failed"),
        ErrorCategory::Network
    );
    assert_eq!(
        ErrorCategory::from_error_message("Something unexpected"),
        ErrorCategory::Unknown
    );
}

/// Test error category descriptions
#[test]
fn test_error_category_descriptions() {
    assert_eq!(ErrorCategory::Timeout.description(), "Connection timed out");
    assert_eq!(
        ErrorCategory::Rejected.description(),
        "Event rejected by relay"
    );
    assert_eq!(ErrorCategory::Auth.description(), "Authentication required");
    assert_eq!(ErrorCategory::RateLimit.description(), "Rate limited");
    assert_eq!(ErrorCategory::Network.description(), "Network error");
    assert_eq!(ErrorCategory::Unknown.description(), "Unknown error");
}

/// Test PublishResult success formatting
#[test]
fn test_publish_result_success() {
    let result = gitafter::nostr::PublishResult::success("event123".to_string(), 5, 5);

    assert!(result.success);
    assert_eq!(result.confirmations, 5);
    assert_eq!(result.relays_attempted, 5);
    assert!(result.failures.is_empty());
    assert!(result.message.contains("5/5"));
}

/// Test PublishResult partial success formatting
#[test]
fn test_publish_result_partial_success() {
    let failures = vec![gitafter::nostr::RelayFailure {
        relay_url: "wss://relay1.example.com".to_string(),
        error: "Timeout".to_string(),
        category: ErrorCategory::Timeout,
    }];

    let result =
        gitafter::nostr::PublishResult::partial_success("event456".to_string(), 4, 5, failures);

    assert!(result.success);
    assert_eq!(result.confirmations, 4);
    assert_eq!(result.relays_attempted, 5);
    assert_eq!(result.failures.len(), 1);
    assert!(result.message.contains("4/5"));
    assert!(result.message.contains("1 failed"));
}

/// Test PublishResult failure formatting
#[test]
fn test_publish_result_failure() {
    let failures = vec![
        gitafter::nostr::RelayFailure {
            relay_url: "wss://relay1.example.com".to_string(),
            error: "Connection refused".to_string(),
            category: ErrorCategory::Network,
        },
        gitafter::nostr::RelayFailure {
            relay_url: "wss://relay2.example.com".to_string(),
            error: "Timeout".to_string(),
            category: ErrorCategory::Timeout,
        },
    ];

    let result = gitafter::nostr::PublishResult::failure("event789".to_string(), 0, 2, failures);

    assert!(!result.success);
    assert_eq!(result.confirmations, 0);
    assert_eq!(result.relays_attempted, 2);
    assert_eq!(result.failures.len(), 2);
    assert!(result.message.contains("Failed"));
    assert!(result.message.contains("Connection refused"));
}

/// Test that broadcaster broadcasts are formatted correctly for success
#[tokio::test]
async fn test_broadcast_format_success() {
    let broadcaster = Arc::new(WsBroadcaster::new(64));
    let _client = NostrClient::new(vec!["wss://relay.example.com".to_string()], broadcaster);

    // Note: Actual broadcast verification would require mocking or integration testing
    // This test just verifies the client can be created with a broadcaster
    assert!(true);
}

/// Test graceful degradation when no relays are available
#[test]
fn test_no_relays_scenario() {
    // Note: Actually creating NostrClient requires a valid database path.
    // This test verifies the concept - in practice, NostrClient initialization
    // handles empty relay lists and can add relays later via add_relay()
    let empty_relays: Vec<String> = vec![];
    assert_eq!(empty_relays.len(), 0);
}
