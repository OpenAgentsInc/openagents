use maud::{Markup, html};

pub fn step_badge(step: u32) -> Markup {
    html! {
        span
            title={ "Step " (step) }
            class="inline-flex items-center px-1.5 py-0.5 text-xs bg-secondary text-muted-foreground cursor-pointer"
        {
            "[" (step) "]"
        }
    }
}
