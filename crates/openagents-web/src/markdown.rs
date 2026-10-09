//! Markdown to HTML for the legal pages, the docs, and the homepage
//! terminal's answers.
//!
//! Raw HTML in the source is shown as text, never as markup, so a document
//! or an answer cannot write into the page. A link keeps its target only
//! when it is `http`, `https`, `mailto`, a site path, or an anchor; any
//! other link is drawn as its text. An image draws as its alt text, except
//! in a document this site ships ([`render_document`]) whose image is one
//! of the site's own files under `/static/`.

use maud::Render;
use openagents_ui::actions::{ButtonVariant, Color, ControlSize, CopyButton};
use openagents_ui::content::CodeBlock;
use pulldown_cmark::{Alignment, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// Renders `source` as HTML, every image as its alt text.
#[must_use]
pub fn render(source: &str) -> String {
    rendered(source, false, false)
}

/// Renders an assistant reply: as [`render`], but every code block is an
/// `openagents-ui` [`CodeBlock`] whose header carries a "Copy" button (a
/// CopyButton holding the code, handled by the site's component script), so
/// a command in a reply copies in one click. An inline code span with a
/// space in it (a command with arguments, like `coder login`) is followed by
/// a small icon-only copy button; a single-word span gets none.
#[must_use]
pub fn render_reply(source: &str) -> String {
    rendered(source, false, true)
}

/// Renders a document this site ships: as [`render`], but an image whose
/// source is one of the site's own files under `/static/` draws.
#[must_use]
pub fn render_document(source: &str) -> String {
    rendered(source, true, false)
}

fn rendered(source: &str, own_images: bool, copyable_code: bool) -> String {
    let mut kept_images = Vec::new();
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
        // An image draws as its alt text unless it is the site's own file
        // in a document the site ships; the site's policy loads images from
        // its own origin only.
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let kept = own_images
                && dest_url.starts_with("/static/")
                && !dest_url.contains("..")
                && !dest_url.contains("//");
            kept_images.push(kept);
            kept.then_some(Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::End(TagEnd::Image) => kept_images
            .pop()
            .unwrap_or(false)
            .then_some(Event::End(TagEnd::Image)),
        other => Some(other),
    });
    let mut out = String::new();
    if copyable_code {
        html::push_html(&mut out, with_copyable_code(events).into_iter());
    } else {
        html::push_html(&mut out, events);
    }
    out
}

/// Replaces each code block with a [`CodeBlock`]: the escaped code, a
/// language label from a fenced block's info string, and a Copy button.
/// Follows each inline code span that holds a space with an icon-only
/// [`CopyButton`].
fn with_copyable_code<'a>(events: impl Iterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut open: Option<(Option<String>, String)> = None;
    for event in events {
        match (event, open.as_mut()) {
            (Event::Start(Tag::CodeBlock(kind)), None) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().map(str::to_owned)
                    }
                    CodeBlockKind::Indented => None,
                };
                open = Some((language, String::new()));
            }
            (Event::End(TagEnd::CodeBlock), Some(_)) => {
                let (language, code) = open.take().unwrap_or_default();
                let mut block = CodeBlock::new(code.strip_suffix('\n').unwrap_or(&code));
                if let Some(language) = language {
                    block = block.language(&language);
                }
                out.push(Event::Html(CowStr::from(block.render().into_string())));
            }
            (Event::Text(text), Some((_, code))) => code.push_str(&text),
            (Event::Code(text), None) if text.trim().contains(char::is_whitespace) => {
                let button = CopyButton::new(text.trim())
                    .aria_label("Copy command")
                    .copied_label("Copied")
                    .size(ControlSize::Xs3)
                    .variant(ButtonVariant::Ghost)
                    .color(Color::Secondary)
                    .class("oa-inline-copy")
                    .render()
                    .into_string();
                out.push(Event::Code(text));
                out.push(Event::InlineHtml(CowStr::from(button)));
            }
            (other, _) => out.push(other),
        }
    }
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
    fn only_a_shipped_document_draws_the_sites_own_image() {
        let own = "![The Grid](/static/verse-grid.jpg)";
        assert!(
            render_document(own).contains("<img src=\"/static/verse-grid.jpg\" alt=\"The Grid\"")
        );
        // An answer, or any other image, is its alt text.
        assert_eq!(render(own), "<p>The Grid</p>\n");
        for elsewhere in [
            "![x](https://example.com/a.png)",
            "![x](//example.com/a.png)",
            "![x](/static/../secret)",
            "![x](/u/me.png)",
        ] {
            assert!(!render_document(elsewhere).contains("<img"), "{elsewhere}");
        }
    }

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
    fn a_replys_code_block_has_a_copy_button() {
        let command = "curl -fsSL https://openagents.com/cli/install.sh | bash";
        let html = render_reply(&format!(
            "Install it:\n\n```bash\n{command}\n```\n\nThen <b>run</b> it."
        ));
        assert!(
            html.contains("class=\"oa-code-block\" data-language=\"bash\""),
            "{html}"
        );
        assert!(
            html.contains(&format!("data-oa-copy=\"{command}\"")),
            "{html}"
        );
        assert!(html.contains("<span>Copy</span>"), "{html}");
        assert!(html.contains(&format!(">{command}</code>")), "{html}");
        // No inline script or handler, and raw HTML stays text.
        assert!(
            !html.contains("<script") && !html.contains("onclick"),
            "{html}"
        );
        assert!(html.contains("&lt;b&gt;run&lt;/b&gt;"), "{html}");
        // Code is escaped in the block and in its copy value.
        let html = render_reply("```\n<script>alert(1)</script>\n```");
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("data-oa-copy=\"&lt;script&gt;"), "{html}");
        // An indented block is copyable too; pages that are not replies keep
        // plain blocks.
        assert!(render_reply("    ls -la\n").contains("data-oa-copy=\"ls -la\""));
        assert!(!render("```\nx\n```").contains("data-oa-copy"));
    }

    #[test]
    fn an_inline_command_in_a_reply_has_a_copy_button() {
        let html = render_reply(
            "On macOS or Linux, run `curl -fsSL https://openagents.com/cli/install.sh | bash`; then run `coder login`, type `/sync on`, and use `coder`.",
        );
        for command in [
            "curl -fsSL https://openagents.com/cli/install.sh | bash",
            "coder login",
            "/sync on",
        ] {
            let span = format!("<code>{command}</code><button");
            assert!(html.contains(&span), "{command}: {html}");
            assert!(
                html.contains(&format!("data-oa-copy=\"{command}\"")),
                "{html}"
            );
        }
        assert_eq!(
            html.matches("aria-label=\"Copy command\"").count(),
            3,
            "{html}"
        );
        assert!(html.contains("oa-inline-copy"), "{html}");
        // A single word gets no button; nothing inline runs.
        assert!(!html.contains("data-oa-copy=\"coder\""), "{html}");
        assert!(html.contains("<code>coder</code>."), "{html}");
        assert!(
            !html.contains("<script") && !html.contains("onclick"),
            "{html}"
        );
        // Escaped, and plain pages get no inline buttons.
        let html = render_reply("run `echo \"<b>\" > x`");
        assert!(
            html.contains("data-oa-copy=\"echo &quot;&lt;b&gt;&quot; &gt; x\""),
            "{html}"
        );
        assert!(!html.contains("<b>"), "{html}");
        assert!(!render("run `coder login`").contains("data-oa-copy"));
    }

    #[test]
    fn titles() {
        let source = "# Terms of Service\nLast updated: 2026-09-03\n\nFirst line.\n";
        assert_eq!(title(source, "x"), "Terms of Service");
        assert_eq!(title("no heading", "fallback"), "fallback");
    }
}
