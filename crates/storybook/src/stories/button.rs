//! Button component story

use maud::{Markup, html};
use ui::{Button, ButtonSize, ButtonVariant};

fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" {
            (title)
        }
    }
}

fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" {
            (content)
        }
    }
}

fn row(content: Markup) -> Markup {
    html! {
        div class="flex gap-4 items-center flex-wrap" {
            (content)
        }
    }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn button_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Button"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "A button component with variants, sizes, and states."
        }

        (section_title("Variants"))
        (section(row(html! {
            (item("Primary", Button::new("Primary").render()))
            (item("Secondary", Button::new("Secondary").variant(ButtonVariant::Secondary).render()))
            (item("Ghost", Button::new("Ghost").variant(ButtonVariant::Ghost).render()))
        })))

        (section_title("Sizes"))
        (section(row(html! {
            (item("Small", Button::new("Small").size(ButtonSize::Small).render()))
            (item("Default", Button::new("Default").render()))
            (item("Large", Button::new("Large").size(ButtonSize::Large).render()))
        })))

        (section_title("States"))
        (section(row(html! {
            (item("Active", Button::new("Click me").render()))
            (item("Disabled", Button::new("Disabled").disabled(true).render()))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::{Button, ButtonSize, ButtonVariant};

// Basic usage
Button::new("Submit").render()

// With variant
Button::new("Cancel")
    .variant(ButtonVariant::Secondary)
    .render()

// With size
Button::new("Small")
    .size(ButtonSize::Small)
    .render()

// Disabled
Button::new("Disabled")
    .disabled(true)
    .render()"#))
    }
}
