use maud::{Markup, html};

pub fn redacted_value(label: &str) -> Markup {
    html! {
        span
            title={ "Redacted: " (label) }
            class="text-xs text-red bg-destructive/10 px-1.5 py-0.5"
        {
            "[redacted:" (label) "]"
        }
    }
}
