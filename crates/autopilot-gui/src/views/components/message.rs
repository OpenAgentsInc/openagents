//! Message bubble component

use maud::{html, Markup};

/// Message bubble for chat interface
pub fn message_bubble(role: &str, content: &str) -> Markup {
    let (bg_color, text_color, alignment) = match role {
        "user" => ("#2a4a7c", "#e0e0e0", "flex-end"),
        "assistant" => ("#2a2a2a", "#e0e0e0", "flex-start"),
        "system" => ("#3a3a1a", "#d0d0a0", "center"),
        _ => ("#2a2a2a", "#e0e0e0", "flex-start"),
    };

    html! {
        div style=(format!(
            "display: flex; justify-content: {}; margin-bottom: 1rem;",
            alignment
        )) {
            div style=(format!(
                "background: {}; color: {}; padding: 0.75rem 1rem; max-width: 70%; border: 1px solid #3a3a3a;",
                bg_color, text_color
            )) {
                div style="font-size: 0.75rem; opacity: 0.7; margin-bottom: 0.25rem;" {
                    (role)
                }
                div style="white-space: pre-wrap; word-break: break-word;" {
                    (content)
                }
            }
        }
    }
}
