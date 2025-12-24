//! Keybinding hint story.

use maud::{Markup, html};
use ui::acp::atoms::keybinding_hint;

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

fn row(content: Markup) -> Markup {
    html! { div class="flex gap-6 items-center flex-wrap" { (content) } }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn keybinding_hint_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Keybinding Hint"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Keyboard shortcut hints for actions."
        }

        (section_title("Permission Keybindings"))
        (section(row(html! {
            (keybinding_hint("y", "allow"))
            (keybinding_hint("Y", "always allow"))
            (keybinding_hint("n", "reject"))
            (keybinding_hint("N", "always reject"))
        })))

        (section_title("Other Examples"))
        (section(row(html! {
            (keybinding_hint("Enter", "send"))
            (keybinding_hint("Esc", "cancel"))
            (keybinding_hint("Ctrl+C", "copy"))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::keybinding_hint;

keybinding_hint("y", "allow")
keybinding_hint("Enter", "send message")"#))
    }
}
