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
            "Displays Claude authentication status. Shows model, version, sessions, messages, and token usage."
        }

        (section_title("Not Logged In"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::not_logged_in().build())
            }
        }))

        (section_title("Authenticated - Pro User"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::authenticated()
                    .model("opus-4-5")
                    .version("1.0.0")
                    .total_sessions(42)
                    .total_messages(156)
                    .today_tokens(125000)
                    .build())
            }
        }))

        (section_title("Authenticated - With Model Usage"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::authenticated()
                    .model("sonnet-4-5")
                    .version("1.0.0")
                    .total_sessions(128)
                    .total_messages(512)
                    .today_tokens(250000)
                    .add_model_usage("sonnet-4-5".to_string(), 15000000, 3500000, 5000000, 1000000, 0, 1.25, 200000)
                    .add_model_usage("opus-4-5".to_string(), 8000000, 2000000, 3000000, 500000, 0, 0.75, 200000)
                    .build())
            }
        }))

        (section_title("Authenticated - Minimal"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::authenticated()
                    .model("haiku-4")
                    .build())
            }
        }))

        (section_title("Loading"))
        (section(html! {
            div class="bg-black p-4" {
                (ClaudeStatus::loading().build())
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::ClaudeStatus;

// Not logged in
ClaudeStatus::not_logged_in().build()

// Authenticated with full info
ClaudeStatus::authenticated()
    .model("opus-4-5")
    .version("1.0.0")
    .total_sessions(42)
    .total_messages(156)
    .today_tokens(125000)
    .add_model_usage("sonnet-4-5".to_string(), 15000000, 3500000, 5000000, 1000000)
    .build()

// For positioned version (fixed bottom-right with HTMX polling)
ClaudeStatus::authenticated()
    .model("sonnet-4-5")
    .build_positioned()"#))
    }
}
