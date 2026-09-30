//! Markdown to HTML for the legal pages and the ask box's answers.
//!
//! Raw HTML in the source is shown as text, never as markup, so a document
//! or an answer cannot write into the page. A link keeps its target only
//! when it is `http`, `https`, `mailto`, a site path, or an anchor; any
//! other link is drawn as its text.

use pulldown_cmark::{Alignment, CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// Renders `source` as HTML.
#[must_use]
pub fn render(source: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut kept_links = Vec::new();
    let events = Parser::new_ext(source, options).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => match resolve(&dest_url) {
            Some(target) => {
                kept_links.push(true);
                Some(Event::Start(Tag::Link {
                    link_type,
                    dest_url: CowStr::from(target),
                    title,
                    id,
                }))
            }
            None => {
                kept_links.push(false);
                None
            }
        },
        Event::End(TagEnd::Link) => {
            if kept_links.pop().unwrap_or(false) {
                Some(Event::End(TagEnd::Link))
            } else {
                None
            }
        }
        // Alignment would be an inline style, which the site's policy
        // refuses; every cell aligns left.
        Event::Start(Tag::Table(alignments)) => Some(Event::Start(Tag::Table(
            alignments.iter().map(|_| Alignment::None).collect(),
        ))),
        // An image draws as its alt text: the site's policy loads images
        // from its own origin only, and no document here ships one.
        Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => None,
        other => Some(other),
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// Where a link may point, or `None` when it is drawn as plain text.
fn resolve(target: &str) -> Option<String> {
    let lower = target.to_ascii_lowercase();
    let web = lower.starts_with("https://") || lower.starts_with("http://");
    let site = target.starts_with('#') || (target.starts_with('/') && !target.starts_with("//"));
    (web || lower.starts_with("mailto:") || site).then(|| target.to_owned())
}

/// The first level-one heading, or `fallback`.
#[must_use]
pub fn title(source: &str, fallback: &str) -> String {
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix("# ").map(|t| t.trim().to_owned()))
        .unwrap_or_else(|| fallback.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_html_is_text() {
        let html = render("Hi <script>alert(1)</script>\n\n<div onclick=x>y</div>");
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<div"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn links_resolve_or_draw_as_text() {
        let html = render(
            "[a](install.md) [b](#top) [c](../roadmap.md) [d](javascript:alert(1)) [e](https://x.example) [f](/terms) [g](mailto:a@b.example)",
        );
        assert!(!html.contains("install.md"), "{html}");
        assert!(html.contains("href=\"#top\""), "{html}");
        assert!(html.contains("href=\"mailto:a@b.example\""), "{html}");
        assert!(!html.contains("roadmap"), "{html}");
        assert!(html.contains(">c<") || html.contains(" c "), "{html}");
        assert!(!html.contains("javascript"), "{html}");
        assert!(html.contains("href=\"https://x.example\""), "{html}");
        assert!(html.contains("href=\"/terms\""), "{html}");
    }

    #[test]
    fn titles() {
        let source = "# Terms of Service\nLast updated: 2026-09-03\n\nFirst line.\n";
        assert_eq!(title(source, "x"), "Terms of Service");
        assert_eq!(title("no heading", "fallback"), "fallback");
    }
}
