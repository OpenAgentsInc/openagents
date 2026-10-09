//! Markdown to HTML for the legal pages, the docs, and the homepage
//! terminal's answers.
//!
//! Raw HTML in the source is shown as text, never as markup, so a document
//! or an answer cannot write into the page. A link keeps its target only
//! when it is `http`, `https`, `mailto`, a site path, or an anchor; any
//! other link is drawn as its text. A link to our own site
//! (`https://openagents.com/...`) becomes a site path, so it opens in the
//! same tab (and the page's `hx-boost` swaps it in); any other web link
//! opens in a new tab with `rel="noopener noreferrer"`. An image draws as
//! its alt text, except in a document this site ships ([`render_document`])
//! whose image is one of the site's own files under `/static/`.
//!
//! A reply ([`render_reply`]) also links bare URLs
//! ([`markdown_stream::autolink`]) and draws each fenced `openui-lang`
//! block as the components it names ([`crate::answer_ui`], #11187).

use std::ops::Range;

use markdown_stream::autolink;
use maud::Render;
use openagents_ui::actions::{ButtonVariant, Color, ControlSize, CopyButton};
use openagents_ui::content::CodeBlock;
use openui_lang::embed::Segment;
use pulldown_cmark::{Alignment, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// Who is reading a reply, for the parts of it that depend on that.
#[derive(Clone, Debug, Default)]
pub struct Reader {
    /// Whether the reader is signed in, when the page knows.
    pub signed_in: Option<bool>,
    /// A name unique on the page for this reply, such as its message id;
    /// it keeps each reply's tab groups apart.
    pub id: String,
}

/// Renders `source` as HTML, every image as its alt text.
#[must_use]
pub fn render(source: &str) -> String {
    rendered(source, false, false, false)
}

/// Renders an assistant reply: as [`render`], but every code block is an
/// `openagents-ui` [`CodeBlock`] whose header carries a "Copy" button (a
/// CopyButton holding the code, handled by the site's component script), so
/// a command in a reply copies in one click. An inline code span with a
/// space in it (a command with arguments, like `coder login`) is followed by
/// a small icon-only copy button; a single-word span gets none. Bare URLs
/// are links, and each `openui-lang` block draws as its components.
#[must_use]
pub fn render_reply(source: &str) -> String {
    render_reply_for(source, &Reader::default())
}

/// [`render_reply`] for a known reader.
#[must_use]
pub fn render_reply_for(source: &str, reader: &Reader) -> String {
    reply(source, reader, false)
}

/// Renders a reply still streaming in: [`render_reply`] of the part that
/// renders cleanly so far ([`markdown_stream::renderable`]), so half-written
/// syntax never shows and a table or list never flashes raw. A URL still
/// being written is not linked yet, and a component block still streaming
/// shows only its finished parts ([`openui_lang::Stream`]). A finished
/// reply renders with [`render_reply`], so its last render is the one-shot
/// one.
#[must_use]
pub fn render_streaming(partial: &str) -> String {
    render_streaming_for(partial, &Reader::default())
}

/// [`render_streaming`] for a known reader.
#[must_use]
pub fn render_streaming_for(partial: &str, reader: &Reader) -> String {
    reply(partial, reader, true)
}

/// Renders a document this site ships: as [`render`], but an image whose
/// source is one of the site's own files under `/static/` draws.
#[must_use]
pub fn render_document(source: &str) -> String {
    rendered(source, true, false, false)
}

fn reply(source: &str, reader: &Reader, streaming: bool) -> String {
    let segments = openui_lang::embed::segments(source);
    let last = segments.len().saturating_sub(1);
    let mut out = String::new();
    for (n, segment) in segments.into_iter().enumerate() {
        match segment {
            Segment::Markdown(markdown) if streaming && n == last => {
                out.push_str(&rendered(
                    &markdown_stream::renderable(markdown),
                    false,
                    true,
                    true,
                ));
            }
            Segment::Markdown(markdown) => out.push_str(&rendered(markdown, false, true, false)),
            Segment::Ui { source, closed } => {
                let document = if closed {
                    openui_lang::parse(source)
                } else {
                    openui_lang::parse_partial(source)
                };
                if let Some(root) = document.root {
                    let id = format!(
                        "{}-ui{n}",
                        if reader.id.is_empty() {
                            "reply"
                        } else {
                            &reader.id
                        }
                    );
                    out.push_str(
                        &crate::answer_ui::render(&root, reader.signed_in, &id).into_string(),
                    );
                }
            }
        }
    }
    out
}

fn rendered(source: &str, own_images: bool, reply: bool, streaming: bool) -> String {
    let mut kept_images = Vec::new();
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut kept_links = Vec::new();
    let events = Parser::new_ext(source, options)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            let event = match event {
                Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
                Event::Start(Tag::Link { dest_url, .. }) => match resolve(&dest_url) {
                    Some(target) => {
                        kept_links.push(true);
                        Event::InlineHtml(CowStr::from(link_tag(&target, reply)))
                    }
                    None => {
                        kept_links.push(false);
                        return None;
                    }
                },
                Event::End(TagEnd::Link) => {
                    if kept_links.pop().unwrap_or(false) {
                        Event::InlineHtml(CowStr::from("</a>"))
                    } else {
                        return None;
                    }
                }
                // Alignment would be an inline style, which the site's policy
                // refuses; every cell aligns left.
                Event::Start(Tag::Table(alignments)) => Event::Start(Tag::Table(
                    alignments.iter().map(|_| Alignment::None).collect(),
                )),
                // An image draws as its alt text unless it is the site's own
                // file in a document the site ships; the site's policy loads
                // images from its own origin only.
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
                    if !kept {
                        return None;
                    }
                    Event::Start(Tag::Image {
                        link_type,
                        dest_url,
                        title,
                        id,
                    })
                }
                Event::End(TagEnd::Image) => {
                    if !kept_images.pop().unwrap_or(false) {
                        return None;
                    }
                    Event::End(TagEnd::Image)
                }
                other => other,
            };
            Some((event, range))
        });
    let mut out = String::new();
    if reply {
        let events = with_autolinks(events, source, streaming);
        html::push_html(&mut out, with_copyable_code(events.into_iter()).into_iter());
    } else {
        html::push_html(&mut out, events.map(|(event, _)| event));
    }
    out
}

/// The opening tag of a link to `target`, which [`resolve`] allowed: a link
/// to our own site as a site path, in the same tab; any other web link in a
/// new tab with `rel="noopener noreferrer"`.
pub(crate) fn open_link(target: &str) -> String {
    link_tag(target, true)
}

/// [`open_link`], keeping an absolute link to our own site as written (and
/// so in a new tab) unless `same_tab`.
fn link_tag(target: &str, same_tab: bool) -> String {
    let own = if same_tab {
        autolink::same_site(target)
    } else {
        None
    };
    let (href, external) = match own {
        Some(path) => (path, false),
        None => {
            let lower = target.to_ascii_lowercase();
            let web = lower.starts_with("https://") || lower.starts_with("http://");
            (target.to_owned(), web)
        }
    };
    let href = escape(&href);
    if external {
        format!("<a href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\">")
    } else {
        format!("<a href=\"{href}\">")
    }
}

fn escape(text: &str) -> String {
    maud::html! { (text) }.into_string()
}

/// `text` as HTML with its bare URLs linked ([`open_link`]); with
/// `streaming`, a URL that runs to the end is still plain text.
pub(crate) fn autolinked(text: &str, streaming: bool) -> String {
    let mut out = String::new();
    let mut at = 0;
    for link in autolink::find(text, streaming) {
        out.push_str(&escape(&text[at..link.range.start]));
        out.push_str(&open_link(&link.href));
        out.push_str(&escape(&text[link.range.clone()]));
        out.push_str("</a>");
        at = link.range.end;
    }
    out.push_str(&escape(&text[at..]));
    out
}

/// Links the bare URLs in text outside code and links. Adjacent text events
/// are joined first, since the parser may split a URL at `_` or `*`. With
/// `streaming`, a URL at the very end of the source is left as text.
fn with_autolinks<'a>(
    events: impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &str,
    streaming: bool,
) -> Vec<Event<'a>> {
    let end = source.trim_end().len();
    let mut out: Vec<Event<'a>> = Vec::new();
    let mut pending: Option<(String, Range<usize>)> = None;
    let mut code = 0usize;
    let mut link = 0usize;
    let flush = |pending: &mut Option<(String, Range<usize>)>, out: &mut Vec<Event<'a>>| {
        if let Some((text, range)) = pending.take() {
            let at_end = streaming && range.end >= end;
            if autolink::find(&text, at_end).is_empty() {
                out.push(Event::Text(CowStr::from(text)));
            } else {
                out.push(Event::InlineHtml(CowStr::from(autolinked(&text, at_end))));
            }
        }
    };
    for (event, range) in events {
        match &event {
            Event::Text(text) if code == 0 && link == 0 => {
                match &mut pending {
                    Some((joined, span)) => {
                        joined.push_str(text);
                        span.end = range.end;
                    }
                    None => pending = Some((text.to_string(), range)),
                }
                continue;
            }
            _ => {}
        }
        flush(&mut pending, &mut out);
        match &event {
            Event::Start(Tag::CodeBlock(_)) => code += 1,
            Event::End(TagEnd::CodeBlock) => code = code.saturating_sub(1),
            Event::InlineHtml(html) if html.starts_with("<a ") => link += 1,
            Event::InlineHtml(html) if html.starts_with("</a>") => link = link.saturating_sub(1),
            _ => {}
        }
        out.push(event);
    }
    flush(&mut pending, &mut out);
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

    #[test]
    fn a_replys_bare_urls_are_links() {
        let html = render_reply(
            "Sign in at https://openagents.com/projects, approve it at openagents.com/device, and read https://github.com/OpenAgentsInc/openagents.\n\nRun `curl -fsSL https://openagents.com/cli/install.sh | bash`.",
        );
        assert!(
            html.contains("<a href=\"/projects\">https://openagents.com/projects</a>,"),
            "{html}"
        );
        assert!(
            html.contains("<a href=\"/device\">openagents.com/device</a>,"),
            "{html}"
        );
        assert!(
            html.contains("<a href=\"https://github.com/OpenAgentsInc/openagents\" target=\"_blank\" rel=\"noopener noreferrer\">https://github.com/OpenAgentsInc/openagents</a>."),
            "{html}"
        );
        // A URL inside code stays code.
        assert!(!html.contains("<a href=\"/cli/install.sh\""), "{html}");
        // A written link to our site stays in the tab; others open a new one.
        let html = render_reply(
            "[Download](https://openagents.com/download) or [docs](https://docs.rs/x).",
        );
        assert!(
            html.contains("<a href=\"/download\">Download</a>"),
            "{html}"
        );
        assert!(
            html.contains("href=\"https://docs.rs/x\" target=\"_blank\""),
            "{html}"
        );
        // A URL split by emphasis-like characters is still one link.
        let html = render_reply("See https://example.com/a_b_c and https://example.com/x*y*z now.");
        assert!(html.contains(">https://example.com/a_b_c</a>"), "{html}");
        // Pages that are not replies do not link bare URLs.
        assert!(!render("See https://example.com now.").contains("<a "));
    }

    #[test]
    fn a_url_still_streaming_is_not_linked() {
        let html = render_streaming("Open https://openagents.com/proj");
        assert!(!html.contains("<a "), "{html}");
        assert!(html.contains("https://openagents.com/proj"), "{html}");
        let html = render_streaming("Open https://openagents.com/projects to start");
        assert!(html.contains("<a href=\"/projects\">"), "{html}");
    }

    const UI_REPLY: &str = "Connect it on the web at https://openagents.com/projects, or on your computer.\n\n```openui-lang\nroot = Columns([web, computer])\nweb = Card(\"On the web\", [Button(\"Connect GitHub\", href=\"/projects\")])\ncomputer = Card(\"On your computer\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\", windows=\"irm https://openagents.com/cli/install.ps1 | iex\")])\n```\n";

    #[test]
    fn a_replys_component_block_draws_as_components() {
        let html = render_reply_for(
            UI_REPLY,
            &Reader {
                signed_in: Some(true),
                id: "chat-message-1".into(),
            },
        );
        assert!(
            html.contains("<a href=\"/projects\">https://openagents.com/projects</a>"),
            "{html}"
        );
        assert!(html.contains("class=\"oa-answer-columns\""), "{html}");
        assert!(html.contains("name=\"chat-message-1-ui1-tabs1\""), "{html}");
        assert!(html.contains("hx-boost=\"false\""), "{html}");
        assert!(
            !html.contains("openui-lang") && !html.contains("root ="),
            "{html}"
        );
        // Streaming, with the block cut mid-line: the finished cards so far,
        // never a raw statement or a half-written value.
        let cut = &UI_REPLY[..UI_REPLY.find("windows=").unwrap()];
        let html = render_streaming(cut);
        assert!(html.contains("Connect GitHub"), "{html}");
        assert!(
            !html.contains("root =") && !html.contains("Command("),
            "{html}"
        );
        // Every cut point renders without panicking and shows no syntax.
        for (at, _) in UI_REPLY.char_indices() {
            let html = render_streaming(&UI_REPLY[..at]);
            assert!(
                !html.contains("= Card(") && !html.contains("```"),
                "at {at}: {html}"
            );
        }
    }
}
