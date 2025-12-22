//! ClaudeStatus component.
//!
//! Shows Claude login/authentication status in a compact card.
//! Displays email, organization, subscription type, and token source.

use maud::{Markup, html};

/// Claude authentication status display.
pub struct ClaudeStatus {
    /// User's email address
    pub email: Option<String>,
    /// Organization name
    pub organization: Option<String>,
    /// Subscription type (pro, enterprise, etc.)
    pub subscription_type: Option<String>,
    /// Token source (api_key, oauth, etc.)
    pub token_source: Option<String>,
    /// API key source (environment, config, etc.)
    pub api_key_source: Option<String>,
}

impl ClaudeStatus {
    /// Create a new status display with no data (not logged in).
    pub fn not_logged_in() -> Self {
        Self {
            email: None,
            organization: None,
            subscription_type: None,
            token_source: None,
            api_key_source: None,
        }
    }

    /// Create a new status display with account info.
    pub fn logged_in(
        email: impl Into<String>,
        organization: Option<String>,
        subscription_type: Option<String>,
        token_source: Option<String>,
        api_key_source: Option<String>,
    ) -> Self {
        Self {
            email: Some(email.into()),
            organization,
            subscription_type,
            token_source,
            api_key_source,
        }
    }

    /// Render the component for positioning (call this for the full positioned version).
    /// Includes HTMX polling to refresh status.
    pub fn build_positioned(self) -> Markup {
        html! {
            div
                id="claude-status"
                style="position: fixed; bottom: 1rem; right: 1rem;"
                hx-get="/api/claude/status"
                hx-trigger="load, every 2s"
                hx-swap="innerHTML"
            {
                (self.build())
            }
        }
    }

    /// Render just the card (for embedding or storybook).
    pub fn build(self) -> Markup {
        let is_logged_in = self.email.is_some();

        html! {
            div
                class="claude-status-card"
                style="
                    background: #111;
                    border: 1px solid #333;
                    padding: 0.75rem 1rem;
                    font-family: 'Berkeley Mono', ui-monospace, monospace;
                    font-size: 0.7rem;
                    min-width: 200px;
                "
            {
                // Header row
                div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem;" {
                    // Status dot
                    span style={
                        "width: 6px; height: 6px; display: inline-block; "
                        @if is_logged_in { "background: #00A645;" } @else { "background: #FF0000;" }
                    } {}
                    span style="color: #888; text-transform: uppercase; letter-spacing: 0.05em;" {
                        "CLAUDE"
                    }
                }

                @if is_logged_in {
                    // Email
                    @if let Some(ref email) = self.email {
                        div style="color: #fafafa; margin-bottom: 0.25rem;" {
                            (email)
                        }
                    }

                    // Organization
                    @if let Some(ref org) = self.organization {
                        div style="color: #666; margin-bottom: 0.25rem;" {
                            (org)
                        }
                    }

                    // Subscription + Source row
                    div style="display: flex; gap: 0.75rem; color: #555; margin-top: 0.5rem;" {
                        @if let Some(ref sub) = self.subscription_type {
                            span {
                                (sub.to_uppercase())
                            }
                        }
                        @if let Some(ref source) = self.token_source {
                            span {
                                (source)
                            }
                        } @else if let Some(ref source) = self.api_key_source {
                            span {
                                (source)
                            }
                        }
                    }
                } @else {
                    div style="color: #666;" {
                        "Not authenticated"
                    }
                }
            }
        }
    }
}
