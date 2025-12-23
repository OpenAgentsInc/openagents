//! Base document layout with Tailwind CSS.

use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::{TAILWIND_CDN, TAILWIND_THEME};

fn vera_mono_font_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let regular = BASE64_STANDARD.encode(include_bytes!("../../../src/gui/assets/fonts/VeraMono.ttf"));
        let italic = BASE64_STANDARD.encode(include_bytes!("../../../src/gui/assets/fonts/VeraMono-Italic.ttf"));
        let bold = BASE64_STANDARD.encode(include_bytes!("../../../src/gui/assets/fonts/VeraMono-Bold.ttf"));
        let bold_italic = BASE64_STANDARD.encode(include_bytes!(
            "../../../src/gui/assets/fonts/VeraMono-Bold-Italic.ttf"
        ));

        format!(
            r#"
@font-face {{
    font-family: 'Vera Mono';
    src: url("data:font/ttf;base64,{regular}") format("truetype");
    font-weight: 400;
    font-style: normal;
}}

@font-face {{
    font-family: 'Vera Mono';
    src: url("data:font/ttf;base64,{italic}") format("truetype");
    font-weight: 400;
    font-style: italic;
}}

@font-face {{
    font-family: 'Vera Mono';
    src: url("data:font/ttf;base64,{bold}") format("truetype");
    font-weight: 700;
    font-style: normal;
}}

@font-face {{
    font-family: 'Vera Mono';
    src: url("data:font/ttf;base64,{bold_italic}") format("truetype");
    font-weight: 700;
    font-style: italic;
}}

:root {{
    --font-mono: 'Vera Mono', ui-monospace, monospace;
}}
"#,
            regular = regular,
            italic = italic,
            bold = bold,
            bold_italic = bold_italic
        )
    })
}

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
                // Embedded Vera Mono font
                style { (PreEscaped(vera_mono_font_css())) }
            }
            body class="bg-background text-foreground font-mono antialiased" {
                (body)
            }
        }
    }
}
