//! Result arrow story.

use maud::{Markup, html};
use ui::recorder::atoms::result_arrow;

use super::shared::{code_block, item, row, section, section_title};

pub fn result_arrow_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Result Arrow" }
        p class="text-sm text-muted-foreground mb-6" { "Separator arrow for results." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Arrow", result_arrow()))
            (item("With result", html! { (result_arrow()) "[ok]" }))
            (item("With count", html! { (result_arrow()) "[186 lines]" }))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::recorder::atoms::result_arrow;

result_arrow()"#))
    }
}
