use maud::{Markup, html};

fn format_tokens(count: u32) -> String {
    if count >= 1000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else {
        count.to_string()
    }
}

pub fn token_badge(
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: Option<u32>,
) -> Markup {
    html! {
        span
            title={
                "Prompt: " (prompt_tokens) ", Completion: " (completion_tokens)
                @if let Some(cached) = cached_tokens {
                    ", Cached: " (cached)
                }
            }
            class="text-xs text-muted-foreground"
        {
            span class="opacity-60" {
                (format_tokens(prompt_tokens))
                " in \u{00B7} "
                (format_tokens(completion_tokens))
                " out"
            }
            @if let Some(cached) = cached_tokens {
                span class="opacity-40" {
                    " (" (format_tokens(cached)) " cached)"
                }
            }
        }
    }
}
