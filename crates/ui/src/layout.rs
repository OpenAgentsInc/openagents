//! Base document layout with Tailwind CSS.

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::{TAILWIND_CDN, TAILWIND_THEME};

/// Render a base HTML document with Tailwind CSS.
pub fn base_document(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                // Inline Tailwind CSS (Play CDN)
                script { (PreEscaped(TAILWIND_CDN)) }
                // Custom theme
                style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
            }
            body class="bg-background text-foreground font-mono antialiased" {
                (body)
            }
        }
    }
}
