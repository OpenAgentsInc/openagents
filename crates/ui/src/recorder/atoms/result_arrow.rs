use maud::{Markup, html};

pub fn result_arrow() -> Markup {
    html! {
        span class="text-muted-foreground opacity-60 mx-2" {
            "\u{2192}"
        }
    }
}
