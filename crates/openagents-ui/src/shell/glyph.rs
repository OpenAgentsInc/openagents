//! Minimal stroke glyphs the shell uses when the caller passes no icon. Every
//! icon slot accepts any `impl Render`, so the `Icon` set replaces these once
//! it lands.

use maud::{Markup, PreEscaped, html};

fn svg(size: u16, paths: &str) -> Markup {
    html! {
        svg class="oa-shell-glyph" width=(size) height=(size) viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round"
            stroke-linejoin="round" aria-hidden="true" focusable="false" {
            (PreEscaped(paths))
        }
    }
}

pub(crate) fn close() -> Markup {
    svg(20, r#"<path d="M6 6l12 12M18 6L6 18"/>"#)
}

pub(crate) fn arrow_up() -> Markup {
    svg(18, r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#)
}

pub(crate) fn chevron_down() -> Markup {
    svg(14, r#"<path d="M6 9l6 6 6-6"/>"#)
}

pub(crate) fn sun() -> Markup {
    svg(
        18,
        r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#,
    )
}

pub(crate) fn moon() -> Markup {
    svg(
        18,
        r#"<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>"#,
    )
}
