//! Markdown to HTML for the pages that serve a document: the docs, the
//! blog, and the legal pages.
//!
//! Raw HTML in the source is shown as text, never as markup, so a document
//! cannot write into the page. A link keeps its
//! target only when it is `http`, `https`, `mailto`, a site path, or an
//! anchor. A link to a sibling Markdown document (`install.md`) points at
//! that document's page under `link_base`. A relative link that leaves the
//! document's folder (`../roadmap.md`) points into a repository the site
//! does not serve, so it is drawn as its text without a link.

use pulldown_cmark::{Alignment, CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// Renders `source` with sibling `.md` links resolved under `link_base`
/// (for example `/docs`).
#[must_use]
pub fn render(source: &str, link_base: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut kept_links = Vec::new();
    let events = Parser::new_ext(source, options).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => match resolve(&dest_url, link_base) {
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
fn resolve(target: &str, link_base: &str) -> Option<String> {
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
    {
        return Some(target.to_owned());
    }
    if target.starts_with('#') || (target.starts_with('/') && !target.starts_with("//")) {
        return Some(target.to_owned());
    }
    if lower.contains(':') || target.contains('/') || target.contains('\\') {
        return None;
    }
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (target, None),
    };
    let slug = path.strip_suffix(".md")?;
    if slug.is_empty() {
        return None;
    }
    Some(match anchor {
        Some(anchor) => format!("{link_base}/{slug}#{anchor}"),
        None => format!("{link_base}/{slug}"),
    })
}

/// The first level-one heading, or `fallback`.
#[must_use]
pub fn title(source: &str, fallback: &str) -> String {
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix("# ").map(|t| t.trim().to_owned()))
        .unwrap_or_else(|| fallback.to_owned())
}

/// The first paragraph line after the first heading, for a listing.
#[must_use]
pub fn description(source: &str) -> String {
    let mut past_heading = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            past_heading = true;
            continue;
        }
        if past_heading
            && !trimmed.is_empty()
            && !trimmed.starts_with("```")
            && !trimmed.starts_with("---")
            && !trimmed.starts_with("Last updated")
        {
            return trimmed.to_owned();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_html_is_text() {
        let html = render(
            "Hi <script>alert(1)</script>\n\n<div onclick=x>y</div>",
            "/docs",
        );
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<div"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn links_resolve_or_draw_as_text() {
        let html = render(
            "[a](install.md) [b](install.md#windows) [c](../roadmap.md) [d](javascript:alert(1)) [e](https://x.example) [f](/terms)",
            "/docs",
        );
        assert!(html.contains("href=\"/docs/install\""), "{html}");
        assert!(html.contains("href=\"/docs/install#windows\""), "{html}");
        assert!(!html.contains("roadmap"), "{html}");
        assert!(html.contains(">c<") || html.contains(" c "), "{html}");
        assert!(!html.contains("javascript"), "{html}");
        assert!(html.contains("href=\"https://x.example\""), "{html}");
        assert!(html.contains("href=\"/terms\""), "{html}");
    }

    #[test]
    fn titles_and_descriptions() {
        let source = "# Terms of Service\nLast updated: 2026-09-03\n\nFirst line.\n";
        assert_eq!(title(source, "x"), "Terms of Service");
        assert_eq!(description(source), "First line.");
        assert_eq!(title("no heading", "fallback"), "fallback");
    }
}
