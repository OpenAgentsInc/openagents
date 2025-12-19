use maud::{Markup, html};

use super::super::atoms::{cost_badge, token_badge};

pub fn metrics_footer(
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: Option<u32>,
    cost: f64,
) -> Markup {
    html! {
        div class="flex items-center gap-3 p-2 border-t border-border text-xs text-muted-foreground" {
            span { "tokens:" }
            (token_badge(prompt_tokens, completion_tokens, cached_tokens))
            span class="opacity-60" { "\u{00B7}" }
            (cost_badge(cost))
        }
    }
}
