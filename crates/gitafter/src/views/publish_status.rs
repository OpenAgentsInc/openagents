//! UI components for displaying publish status and errors

use crate::nostr::{ErrorCategory, PublishResult};
use maud::{Markup, html};

/// Render a publish status notification
#[allow(dead_code)]
pub fn publish_status_notification(result: &PublishResult) -> Markup {
    if result.success {
        publish_success_notification(result)
    } else {
        publish_error_notification(result)
    }
}

/// Render a success notification
#[allow(dead_code)]
fn publish_success_notification(result: &PublishResult) -> Markup {
    let has_failures = !result.failures.is_empty();

    html! {
        div.notification.notification-success {
            div.notification-header {
                span.notification-icon { "✓" }
                strong { "Event Published" }
            }
            p.notification-message { (result.message) }
            @if has_failures {
                details.notification-details {
                    summary { "Show relay details" }
                    div.relay-status-list {
                        @for failure in &result.failures {
                            (relay_failure_item(failure))
                        }
                    }
                }
            }
        }
    }
}

/// Render an error notification
#[allow(dead_code)]
fn publish_error_notification(result: &PublishResult) -> Markup {
    html! {
        div.notification.notification-error {
            div.notification-header {
                span.notification-icon { "⚠" }
                strong { "Failed to Publish Event" }
            }
            p.notification-message { (result.message) }
            @if !result.failures.is_empty() {
                div.error-details {
                    h4 { "Error Details:" }
                    div.relay-status-list {
                        @for failure in &result.failures {
                            (relay_failure_item(failure))
                        }
                    }
                    div.error-actions {
                        button.retry-button { "Retry" }
                        button.dismiss-button { "Dismiss" }
                    }
                }
            }
        }
    }
}

/// Render a single relay failure item
#[allow(dead_code)]
fn relay_failure_item(failure: &crate::nostr::RelayFailure) -> Markup {
    let category_class = match failure.category {
        ErrorCategory::Timeout => "error-timeout",
        ErrorCategory::Rejected => "error-rejected",
        ErrorCategory::Network => "error-network",
        ErrorCategory::Auth => "error-auth",
        ErrorCategory::RateLimit => "error-ratelimit",
        ErrorCategory::Unknown => "error-unknown",
    };

    html! {
        div class={"relay-failure " (category_class)} {
            div.relay-url { (failure.relay_url) }
            div.relay-error {
                span.error-category { (failure.category.description()) }
                span.error-message { (failure.error) }
            }
        }
    }
}

/// Render CSS styles for publish status components
#[allow(dead_code)]
pub fn publish_status_styles() -> &'static str {
    r#"
/* Notification styles */
.notification {
    padding: 1rem;
    margin: 1rem 0;
    border-left: 4px solid;
    background: var(--bg-secondary);
}

.notification-success {
    border-color: #10b981;
}

.notification-error {
    border-color: #ef4444;
}

.notification-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
}

.notification-icon {
    font-size: 1.5rem;
    font-weight: bold;
}

.notification-success .notification-icon {
    color: #10b981;
}

.notification-error .notification-icon {
    color: #ef4444;
}

.notification-message {
    margin: 0.5rem 0;
    color: var(--text-secondary);
}

/* Relay status list */
.relay-status-list {
    margin-top: 0.5rem;
}

.relay-failure {
    padding: 0.5rem;
    margin: 0.25rem 0;
    background: var(--bg-tertiary);
    border-left: 2px solid;
}

.error-timeout { border-color: #f59e0b; }
.error-rejected { border-color: #ef4444; }
.error-network { border-color: #6366f1; }
.error-auth { border-color: #8b5cf6; }
.error-ratelimit { border-color: #f59e0b; }
.error-unknown { border-color: #6b7280; }

.relay-url {
    font-family: monospace;
    font-size: 0.875rem;
    color: var(--text-primary);
}

.relay-error {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.25rem;
    font-size: 0.875rem;
}

.error-category {
    font-weight: 600;
    color: var(--text-secondary);
}

.error-message {
    color: var(--text-tertiary);
}

/* Error actions */
.error-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
}

.retry-button,
.dismiss-button {
    padding: 0.5rem 1rem;
    border: none;
    cursor: pointer;
    font-weight: 500;
}

.retry-button {
    background: var(--accent);
    color: var(--text-primary);
}

.retry-button:hover {
    opacity: 0.9;
}

.dismiss-button {
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
}

.dismiss-button:hover {
    background: var(--bg-tertiary);
}

/* Notification details toggle */
.notification-details summary {
    cursor: pointer;
    color: var(--accent);
    margin-top: 0.5rem;
    user-select: none;
}

.notification-details summary:hover {
    text-decoration: underline;
}
"#
}
