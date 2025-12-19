//! Redacted value story.

use maud::{Markup, html};
use ui::blackbox::atoms::redacted_value;

use super::shared::{code_block, item, row, section, section_title};

pub fn redacted_value_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Redacted Value" }
        p class="text-sm text-muted-foreground mb-6" { "Redaction marker for sensitive values." }

        (section_title("Variants"))
        (section(row(html! {
            (item("API key", redacted_value("api_key")))
            (item("Token", redacted_value("github_token")))
            (item("Env var", redacted_value("env_var")))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::redacted_value;

redacted_value("api_key")
redacted_value("github_token")"#))
    }
}
