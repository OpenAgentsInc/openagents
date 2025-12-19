//! Button component story

use maud::{Markup, html};

/// Button component with inline CSS
pub fn button(label: &str, variant: &str, size: &str, disabled: bool) -> Markup {
    let (bg, fg, border) = match variant {
        "primary" => ("#fff", "#000", "#fff"),
        "secondary" => ("#333", "#fff", "#555"),
        "ghost" => ("transparent", "#888", "#555"),
        _ => ("#fff", "#000", "#fff"),
    };

    let padding = match size {
        "small" => "0.25rem 0.5rem",
        "large" => "0.75rem 1.5rem",
        _ => "0.5rem 1rem",
    };

    let font_size = match size {
        "small" => "0.75rem",
        "large" => "1rem",
        _ => "0.875rem",
    };

    let opacity = if disabled { "0.5" } else { "1" };
    let cursor = if disabled { "not-allowed" } else { "pointer" };

    html! {
        button
            style=(format!(
                "background: {}; color: {}; border: 1px solid {}; padding: {}; font-size: {}; font-family: inherit; cursor: {}; opacity: {};",
                bg, fg, border, padding, font_size, cursor, opacity
            ))
            disabled[disabled]
        {
            (label)
        }
    }
}

fn section_title(title: &str) -> Markup {
    html! {
        h2 style="font-size: 1rem; font-weight: 500; color: #aaa; margin-top: 2rem; margin-bottom: 1rem;" {
            (title)
        }
    }
}

fn section(content: Markup) -> Markup {
    html! {
        div style="padding: 1rem; border: 1px solid #333; background: #161616; margin-bottom: 1rem;" {
            (content)
        }
    }
}

fn row(content: Markup) -> Markup {
    html! {
        div style="display: flex; gap: 1rem; align-items: center; flex-wrap: wrap;" {
            (content)
        }
    }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div style="display: flex; flex-direction: column; gap: 0.5rem;" {
            span style="font-size: 0.75rem; color: #666;" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre style="font-size: 0.75rem; background: #0a0a0a; border: 1px solid #333; padding: 1rem; overflow-x: auto; color: #888;" {
            code { (code) }
        }
    }
}

pub fn button_story() -> Markup {
    html! {
        h1 style="font-size: 1.5rem; font-weight: bold; margin-bottom: 0.5rem; padding-bottom: 0.5rem; border-bottom: 1px solid #333;" {
            "Button"
        }
        p style="font-size: 0.875rem; color: #666; margin-bottom: 1.5rem;" {
            "A button component with variants, sizes, and states."
        }

        (section_title("Variants"))
        (section(row(html! {
            (item("Primary", button("Primary", "primary", "default", false)))
            (item("Secondary", button("Secondary", "secondary", "default", false)))
            (item("Ghost", button("Ghost", "ghost", "default", false)))
        })))

        (section_title("Sizes"))
        (section(row(html! {
            (item("Small", button("Small", "primary", "small", false)))
            (item("Default", button("Default", "primary", "default", false)))
            (item("Large", button("Large", "primary", "large", false)))
        })))

        (section_title("States"))
        (section(row(html! {
            (item("Active", button("Click me", "primary", "default", false)))
            (item("Disabled", button("Disabled", "primary", "default", true)))
        })))

        (section_title("Usage"))
        (code_block(r#"use storybook::stories::button::button;

// Basic usage
button("Submit", "primary", "default", false)

// Variants: "primary", "secondary", "ghost"
// Sizes: "small", "default", "large"
// Disabled: true or false"#))
    }
}
