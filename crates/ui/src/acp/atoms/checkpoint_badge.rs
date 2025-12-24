//! Checkpoint availability badge.

use maud::{Markup, html};

/// Render a checkpoint badge indicating a restorable state.
///
/// # Arguments
/// * `sha` - Git commit SHA (first 7 chars shown)
pub fn checkpoint_badge(sha: &str) -> Markup {
    let short_sha = if sha.len() > 7 { &sha[..7] } else { sha };

    html! {
        span
            class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-mono bg-secondary border border-border text-muted-foreground"
            title={ "Checkpoint: " (sha) }
        {
            span class="text-xs text-cyan" { "@" }
            span { (short_sha) }
        }
    }
}
