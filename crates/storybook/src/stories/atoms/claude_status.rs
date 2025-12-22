//! ClaudeStatus component story

use maud::{Markup, html};
use ui::ClaudeStatus;

fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" {
            (title)
        }
    }
}

fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" {
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn claude_status_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ClaudeStatus"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Displays Claude authentication status. Shows email, organization, subscription type, and token source."
        }

        (section_title("Not Logged In"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::not_logged_in().build())
            }
        }))

        (section_title("Logged In - Pro User"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::logged_in(
                    "user@example.com",
                    Some("Anthropic".to_string()),
                    Some("pro".to_string()),
                    Some("oauth".to_string()),
                    None,
                ).build())
            }
        }))

        (section_title("Logged In - API Key"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::logged_in(
                    "developer@company.com",
                    Some("Acme Corp".to_string()),
                    Some("enterprise".to_string()),
                    None,
                    Some("environment".to_string()),
                ).build())
            }
        }))

        (section_title("Logged In - Minimal"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::logged_in(
                    "test@test.com",
                    None,
                    None,
                    None,
                    None,
                ).build())
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::ClaudeStatus;

// Not logged in
ClaudeStatus::not_logged_in().build()

// Logged in with full info
ClaudeStatus::logged_in(
    "user@example.com",
    Some("Anthropic".to_string()),
    Some("pro".to_string()),
    Some("oauth".to_string()),
    None,
).build()

// For positioned version (fixed bottom-right)
ClaudeStatus::logged_in(...).build_positioned()"#))
    }
}
