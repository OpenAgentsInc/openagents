use maud::{Markup, html};

pub fn attempt_badge(attempt: u8, max_attempts: u8) -> Markup {
    html! {
        span
            title={ "Attempt " (attempt) " of " (max_attempts) }
            class="text-xs text-orange tabular-nums"
        {
            (attempt) "/" (max_attempts)
        }
    }
}
