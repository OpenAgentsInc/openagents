//! Shared page pieces: the site links, escaping, the full-screen canvas
//! document, and the problem page. Every scrolling page renders through
//! [`crate::ui_page::UiPage`] in the Coder Light / Coder Noir design
//! language; only the full-screen canvas pages ([`fullscreen`]) keep their
//! own design (`static/legacy-demo.css`).

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// The copyright line the footer carries.
pub const COPYRIGHT: &str = "\u{a9} 2026 OpenAgents, Inc.";

/// The source code, linked from every footer.
pub const GITHUB: &str = "https://github.com/OpenAgentsInc/openagents";

/// OpenAgents on X, linked from every footer.
pub const X: &str = "https://x.com/OpenAgentsInc";

/// The sections the navigation links to after the primary ones, in order.
pub const SECTIONS: [(&str, &str); 2] = [("Download", "/download"), ("Docs", "/docs")];

/// Escapes text for HTML content and attribute values.
#[must_use]
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Percent-encodes one URL path segment.
#[must_use]
pub fn segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A page with no header or footer: `body` fills the whole window, and the
/// viewport doesn't zoom, so a canvas in it takes every touch. `/everglade`
/// is one.
#[must_use]
pub fn fullscreen_document(title: &str, body: &str) -> String {
    let title = escape(title);
    format!(
        "<!doctype html><html lang=\"en\" class=\"stage\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, \
user-scalable=no, viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{title} \u{b7} OpenAgents</title>\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\
<link rel=\"stylesheet\" href=\"/static/legacy-demo.css\"></head><body>\
<main id=\"content\">{body}</main></body></html>"
    )
}

/// A full-screen page answered with `200`.
#[must_use]
pub fn fullscreen(title: &str, body: &str) -> Response {
    Html(fullscreen_document(title, body)).into_response()
}

/// A page answered with `status`: a heading, a sentence, and a way back,
/// in the Coder Light / Coder Noir shell ([`crate::ui_page::problem`]).
/// Callers without the request's headers get the system theme; callers with
/// them should use [`crate::ui_page::problem`] so the theme cookie applies.
#[must_use]
pub fn problem(status: StatusCode, title: &str, text: &str, back: (&str, &str)) -> Response {
    crate::ui_page::problem(&axum::http::HeaderMap::new(), status, title, text, back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fullscreen_document_loads_only_the_legacy_demo_stylesheet() {
        let html = fullscreen_document("Everglade", "<div class=\"glade\"></div>");
        assert!(html.contains("<html lang=\"en\" class=\"stage\">"));
        assert!(html.contains("href=\"/static/legacy-demo.css\""));
        assert_eq!(html.matches("rel=\"stylesheet\"").count(), 1);
        assert!(html.contains("<title>Everglade \u{b7} OpenAgents</title>"));
        let css = include_str!("../static/legacy-demo.css");
        for rule in [
            "html.stage",
            ".glade{",
            ".glade-status{",
            ".glade-leave{",
            ".unseen{",
        ] {
            assert!(css.contains(rule), "{rule}");
        }
    }

    #[test]
    fn escaping_covers_markup_and_quotes() {
        assert_eq!(
            escape("<a href=\"x\">'&"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;"
        );
        assert_eq!(segment("a b/c"), "a%20b%2Fc");
    }
}
