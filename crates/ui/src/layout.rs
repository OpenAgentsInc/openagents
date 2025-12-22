//! Base document layout with Tailwind CSS.

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::{TAILWIND_CDN, TAILWIND_THEME};

/// Render a base HTML document with Tailwind CSS.
///
/// # Examples
///
/// ```
/// use ui::base_document;
/// use maud::html;
///
/// let page = base_document("My App", html! {
///     div class="p-8" {
///         h1 class="text-2xl font-bold" { "Welcome" }
///         p class="text-muted-foreground mt-2" {
///             "This is a server-rendered page with Tailwind CSS."
///         }
///     }
/// });
/// ```
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
