//! The document shell every page shares: the header with the wordmark and
//! the sections, the page's own `<main>`, and the footer with the terms and
//! the privacy policy. App pages such as the chat keep the header and drop
//! the footer.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// The copyright line the footer carries.
pub const COPYRIGHT: &str = "\u{a9} 2026 OpenAgents, Inc.";

/// The source code, linked from every footer.
pub const GITHUB: &str = "https://github.com/OpenAgentsInc/openagents";

/// OpenAgents on X, linked from every footer.
pub const X: &str = "https://x.com/OpenAgentsInc";

/// The sections the header links to, in order.
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

/// A whole page: `title` names it in the tab, `section` marks the header
/// link it belongs to, and `body` is the trusted markup inside `<main>`.
#[must_use]
pub fn document(title: &str, section: Option<&str>, body: &str) -> String {
    shell(
        title,
        section,
        &format!(
            "<div class=\"scroller\"><main id=\"content\" tabindex=\"-1\">{body}</main>\
<footer class=\"site-footer\"><span class=\"copyright\">{COPYRIGHT}</span>\
<nav aria-label=\"Legal and links\"><a href=\"/terms\">Terms</a>\
<span class=\"sep\" aria-hidden=\"true\">\u{b7}</span><a href=\"/privacy\">Privacy</a>\
<span class=\"sep\" aria-hidden=\"true\">\u{b7}</span><a href=\"{GITHUB}\" rel=\"noopener\">GitHub</a>\
<span class=\"sep\" aria-hidden=\"true\">\u{b7}</span><a href=\"{X}\" rel=\"noopener\">X</a></nav>\
</footer></div>"
        ),
    )
}

/// A page with the header and no footer: `<main class="app">` fills the
/// window under the header and doesn't scroll, so `body` places its own
/// scrolling regions. The chat page is one.
#[must_use]
pub fn app_document(title: &str, section: Option<&str>, body: &str) -> String {
    shell(
        title,
        section,
        &format!("<main id=\"content\" class=\"app\" tabindex=\"-1\">{body}</main>"),
    )
}

/// The head and the header, then `rest`.
///
/// Pages built this way predate the Coder Light / Coder Noir design language,
/// so they pin `data-theme="dark"` until they move to `openagents_ui::shell`,
/// whose document follows the theme cookie and the system setting.
fn shell(title: &str, section: Option<&str>, rest: &str) -> String {
    let title = if title == "OpenAgents" {
        "OpenAgents".to_owned()
    } else {
        format!("{} \u{b7} OpenAgents", escape(title))
    };
    let mut nav = String::new();
    for (name, href) in SECTIONS {
        let current = if section == Some(href) {
            " aria-current=\"page\""
        } else {
            ""
        };
        nav.push_str(&format!("<li><a href=\"{href}\"{current}>{name}</a></li>"));
    }
    format!(
        "<!doctype html><html lang=\"en\" data-theme=\"dark\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{title}</title>\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\
{ui}<link rel=\"stylesheet\" href=\"/static/site.css\">\
<link rel=\"stylesheet\" href=\"/static/tailwind.css\"></head><body>\
<a class=\"skip\" href=\"#content\">Skip to content</a>\
<header class=\"site-header\"><nav aria-label=\"Main\"><a class=\"wordmark\" href=\"/\">OpenAgents</a>\
<ul class=\"navlinks\">{nav}</ul></nav></header>{rest}</body></html>",
        ui = crate::theme::style_tag(),
    )
}

/// A page answered with `200`.
#[must_use]
pub fn page(title: &str, section: Option<&str>, body: &str) -> Response {
    Html(document(title, section, body)).into_response()
}

/// A footerless [`app_document`] answered with `200`.
#[must_use]
pub fn app(title: &str, section: Option<&str>, body: &str) -> Response {
    Html(app_document(title, section, body)).into_response()
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
<link rel=\"stylesheet\" href=\"/static/site.css\"></head><body>\
<main id=\"content\">{body}</main></body></html>"
    )
}

/// A full-screen page answered with `200`.
#[must_use]
pub fn fullscreen(title: &str, body: &str) -> Response {
    Html(fullscreen_document(title, body)).into_response()
}

/// A page answered with `status`: a heading, a sentence, and a way back.
#[must_use]
pub fn problem(status: StatusCode, title: &str, text: &str, back: (&str, &str)) -> Response {
    let body = format!(
        "<h1>{}</h1><p>{}</p><p><a href=\"{}\">[ {} ]</a></p>",
        escape(title),
        escape(text),
        back.0,
        escape(back.1)
    );
    (status, Html(document(title, None, &body))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_links_to_the_terms_and_the_privacy_policy_once() {
        let html = document("Download OpenAgents", Some("/download"), "<p>x</p>");
        assert_eq!(html.matches("href=\"/terms\"").count(), 1);
        assert_eq!(html.matches("href=\"/privacy\"").count(), 1);
        assert!(html.contains(
            "<a href=\"https://github.com/OpenAgentsInc/openagents\" rel=\"noopener\">GitHub</a>"
        ));
        assert!(html.contains("<a href=\"https://x.com/OpenAgentsInc\" rel=\"noopener\">X</a>"));
        assert!(html.contains("<a href=\"/download\" aria-current=\"page\">Download</a>"));
        assert!(html.contains("<a href=\"/docs\">Docs</a>"));
        assert!(!html.contains("/pilot"));
        assert!(html.contains("width=device-width"));
        assert!(!html.to_ascii_lowercase().contains("<script"));
    }

    #[test]
    fn only_the_region_under_the_header_scrolls() {
        let html = document("Download OpenAgents", Some("/download"), "<p>x</p>");
        let header = html.find("</header>").unwrap();
        let scroller = html.find("<div class=\"scroller\">").unwrap();
        assert!(header < scroller, "the header sits outside the scroller");
        assert!(html.ends_with("</footer></div></body></html>"));
        let css = include_str!("../static/site.css");
        assert!(css.contains("html,body{height:100%;overflow:hidden}"));
        assert!(css.contains(".scroller{flex:1;min-height:0;overflow-y:auto"));
    }

    #[test]
    fn app_pages_keep_the_header_and_drop_the_footer() {
        let html = app_document("Chat", None, "<p>x</p>");
        assert!(html.contains("<header class=\"site-header\">"));
        assert!(
            html.contains("<main id=\"content\" class=\"app\" tabindex=\"-1\"><p>x</p></main>")
        );
        assert!(!html.contains("site-footer") && !html.contains("href=\"/terms\""));
        assert!(!html.contains("class=\"scroller\""));
        let css = include_str!("../static/site.css");
        assert!(css.contains("main.app{flex:1;min-height:0;"));
        assert!(css.contains(".app .thread{flex:1;min-height:0;overflow-y:auto;"));
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
