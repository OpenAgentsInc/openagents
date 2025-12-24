//! Keyboard shortcut hints.

use maud::{Markup, html};

/// Render a keyboard shortcut hint.
///
/// # Arguments
/// * `key` - The key to display (e.g., "y", "Y", "n", "N", "Enter")
/// * `action` - Description of what the key does
pub fn keybinding_hint(key: &str, action: &str) -> Markup {
    html! {
        span class="inline-flex items-center gap-1 text-xs text-muted-foreground" {
            kbd class="px-1 py-0.5 bg-secondary border border-border font-mono text-[10px]" {
                (key)
            }
            span { (action) }
        }
    }
}
