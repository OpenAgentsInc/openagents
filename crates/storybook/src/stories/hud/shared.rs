use maud::{Markup, html};

pub fn story_header(title: &str, description: &str) -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { (title) }
        p class="text-sm text-muted-foreground mb-6" { (description) }
    }
}

pub fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) }
    }
}

pub fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" { (content) }
    }
}

pub fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}
