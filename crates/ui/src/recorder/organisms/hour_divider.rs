use maud::{Markup, html};

pub fn hour_divider(title: &str) -> Markup {
    html! {
        div class="py-4 my-4" {
            div class="border-y-2 border-border py-2 text-center" {
                span class="text-xs font-semibold tracking-widest uppercase text-muted-foreground" {
                    (title)
                }
            }
        }
    }
}
