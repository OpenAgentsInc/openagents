//! Streaming content indicator.

use maud::{Markup, html};

/// Render a streaming indicator showing content is being generated.
///
/// # Arguments
/// * `label` - Optional custom label (default: "Generating...")
pub fn streaming_indicator(label: Option<&str>) -> Markup {
    let text = label.unwrap_or("Generating...");

    html! {
        span class="inline-flex items-center gap-2 text-xs text-muted-foreground animate-pulse" {
            span class="w-2 h-2 bg-cyan" { }
            span { (text) }
        }
    }
}
