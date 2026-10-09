//! Conversation content: the Markdown root and its elements, code blocks,
//! the table family, source chips with favicons, and the sticky action bar.
//! Styles live in `static/components/content.css`. Document-page layout
//! (the reading column, facts) lives in `page.rs` and `page.css`.
//!
//! Markdown keeps its existing renderer (`openagents-web/src/markdown.rs`).
//! [`MarkdownRoot`] wraps that renderer's HTML in `.oa-markdown`, and the
//! stylesheet styles the plain elements it writes (`p`, `h1`-`h6`, lists,
//! `code`, `pre > code`, tables, quotes, links). The other builders emit
//! the same look with explicit classes for pages that build content in Rust.
//!
//! Every builder escapes the text it takes. Links go through [`safe_href`],
//! the same rule the Markdown renderer applies.

mod code;
mod markdown;
mod page;
mod source;
mod table;

pub use code::{CodeBlock, StickyActionBar};
pub use markdown::{Heading, InlineCode, List, ListItem, MarkdownRoot, MarkdownSize, Paragraph};
pub use page::{Facts, PageColumn};
pub use source::{Favicon, Source, SourceVariant};
pub use table::{ColSize, Table};

/// Where a link may point, or `None` when it must draw as plain text:
/// `http`, `https`, `mailto`, a site path (`/x`, not `//x`), or an anchor.
/// This matches the Markdown renderer's rule.
#[must_use]
pub fn safe_href(target: &str) -> Option<&str> {
    let target = target.trim();
    let lower = target.to_ascii_lowercase();
    let web = lower.starts_with("https://") || lower.starts_with("http://");
    let site = target.starts_with('#')
        || (target.starts_with('/') && !target.starts_with("//") && !target.starts_with("/\\"));
    let clean = !target.chars().any(|c| c.is_control() || c.is_whitespace());
    ((web || lower.starts_with("mailto:") || site) && clean).then_some(target)
}

/// Whether `target` leaves the site (and so opens with `noopener`).
fn is_external(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_are_constrained_like_the_renderer() {
        for ok in [
            "https://x.example/a",
            "HTTP://x.example",
            "mailto:a@b.example",
            "/terms",
            "#top",
        ] {
            assert_eq!(safe_href(ok), Some(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,x",
            "//evil.example",
            "/\\evil.example",
            "install.md",
            "../x",
            "https://x.example/a b",
            "",
        ] {
            assert_eq!(safe_href(bad), None, "{bad}");
        }
    }
}
