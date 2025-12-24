//! Model identifier badge.

use maud::{Markup, html};

/// Render a model badge showing the current model name.
///
/// # Arguments
/// * `model_id` - The model identifier (e.g., "claude-sonnet-4-20250514")
/// * `compact` - If true, show abbreviated version
pub fn model_badge(model_id: &str, compact: bool) -> Markup {
    let display_name = if compact {
        // Extract short name from model ID
        // e.g., "claude-sonnet-4-20250514" -> "sonnet-4"
        extract_short_name(model_id)
    } else {
        model_id.to_string()
    };

    html! {
        span
            class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-mono bg-secondary border border-border text-muted-foreground"
            title=(model_id)
        {
            span class="text-xs" { "[M]" }
            span { (display_name) }
        }
    }
}

/// Extract a short display name from a full model ID.
fn extract_short_name(model_id: &str) -> String {
    // Handle common patterns
    if model_id.contains("sonnet") {
        if model_id.contains("-4-") {
            return "sonnet-4".to_string();
        }
        return "sonnet".to_string();
    }
    if model_id.contains("opus") {
        if model_id.contains("-4-") {
            return "opus-4".to_string();
        }
        return "opus".to_string();
    }
    if model_id.contains("haiku") {
        return "haiku".to_string();
    }

    // Fallback: take first 12 chars
    if model_id.len() > 12 {
        format!("{}...", &model_id[..12])
    } else {
        model_id.to_string()
    }
}
