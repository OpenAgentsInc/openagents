//! The pages that serve a Markdown document: the terms of service, the
//! privacy policy, and the docs.
//!
//! Every document is compiled into the binary, so a deploy cannot fail to
//! copy one and leave a page answering `404`. The terms and the policy are
//! the text openagents.com published, last updated 2026-10-09.
//! The docs are the user guide to OpenAgents, grouped into sections, in
//! `content/docs/`.

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Redirect, Response};
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::shell::Breadcrumb;

use crate::App;
use crate::layout::escape;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, prose};

/// The terms of service, as Markdown.
pub(crate) const TERMS: &str = include_str!("../../content/legal/terms.md");

/// The privacy policy, as Markdown.
pub(crate) const PRIVACY: &str = include_str!("../../content/legal/privacy.md");

/// The docs, by slug, in reading order.
pub(crate) const DOCS: [(&str, &str); 33] = [
    (
        "what-is-openagents",
        include_str!("../../content/docs/what-is-openagents.md"),
    ),
    ("download", include_str!("../../content/docs/download.md")),
    ("website", include_str!("../../content/docs/website.md")),
    ("mac", include_str!("../../content/docs/mac.md")),
    ("iphone", include_str!("../../content/docs/iphone.md")),
    ("terminal", include_str!("../../content/docs/terminal.md")),
    ("cli", include_str!("../../content/docs/cli.md")),
    ("chat", include_str!("../../content/docs/chat.md")),
    (
        "privacy-and-security",
        include_str!("../../content/docs/privacy-and-security.md"),
    ),
    ("coder", include_str!("../../content/docs/coder.md")),
    (
        "coding-agents",
        include_str!("../../content/docs/coding-agents.md"),
    ),
    (
        "following-coder",
        include_str!("../../content/docs/following-coder.md"),
    ),
    ("traces", include_str!("../../content/docs/traces.md")),
    (
        "worktrees-and-changes",
        include_str!("../../content/docs/worktrees-and-changes.md"),
    ),
    (
        "github-issues",
        include_str!("../../content/docs/github-issues.md"),
    ),
    (
        "ship-from-your-phone",
        include_str!("../../content/docs/ship-from-your-phone.md"),
    ),
    (
        "connect-a-computer",
        include_str!("../../content/docs/connect-a-computer.md"),
    ),
    (
        "manage-computers",
        include_str!("../../content/docs/manage-computers.md"),
    ),
    ("plugins", include_str!("../../content/docs/plugins.md")),
    (
        "write-a-plugin",
        include_str!("../../content/docs/write-a-plugin.md"),
    ),
    (
        "test-a-plugin",
        include_str!("../../content/docs/test-a-plugin.md"),
    ),
    (
        "publish-and-share",
        include_str!("../../content/docs/publish-and-share.md"),
    ),
    (
        "gym-and-xp",
        include_str!("../../content/docs/gym-and-xp.md"),
    ),
    ("verse", include_str!("../../content/docs/verse.md")),
    // Its image is the Grid seen from above as the desktop app's Verse page
    // draws it, captured from the live relay with
    // `crates/verse/examples/overlook_capture.rs`
    // (`OVERLOOK_SIZE=1600x900 … /tmp/grid.png 45 wss://relay.openagents.com 8`)
    // and served from `/static/verse-grid.jpg`.
    ("the-grid", include_str!("../../content/docs/the-grid.md")),
    ("wallet", include_str!("../../content/docs/wallet.md")),
    ("decks", include_str!("../../content/docs/decks.md")),
    ("settings", include_str!("../../content/docs/settings.md")),
    ("pricing", include_str!("../../content/docs/pricing.md")),
    (
        "troubleshooting",
        include_str!("../../content/docs/troubleshooting.md"),
    ),
    ("self-host", include_str!("../../content/docs/self-host.md")),
    ("faq", include_str!("../../content/docs/faq.md")),
    ("glossary", include_str!("../../content/docs/glossary.md")),
];

/// The docs index's sections: each one's title, a line saying what it
/// covers, and the slug of its first guide. A section runs until the next
/// one's first guide, in [`DOCS`] order.
pub(crate) const SECTIONS: [(&str, &str, &str); 9] = [
    (
        "Getting started",
        "What OpenAgents is, and how to get it.",
        "what-is-openagents",
    ),
    (
        "Apps",
        "The website, the Mac and iPhone apps, the Terminal, and the command.",
        "website",
    ),
    (
        "Chat",
        "What the chat answers, and who sees your messages.",
        "chat",
    ),
    (
        "Coder",
        "Coding work on your own computer, with the agents you already use.",
        "coder",
    ),
    (
        "Computers",
        "Connect a computer to your account or your phone, and manage them.",
        "connect-a-computer",
    ),
    (
        "Plugins",
        "Make a plugin, test whether it helps, and publish the result.",
        "plugins",
    ),
    (
        "The Gym, the Verse, and the wallet",
        "XP, the shared world, bitcoin, and decks.",
        "gym-and-xp",
    ),
    (
        "Reference",
        "Settings, pricing, fixes for common problems, and running your own copy.",
        "settings",
    ),
    (
        "Questions and words",
        "Short answers, and what each word means.",
        "faq",
    ),
];

/// A section's anchor on the docs index: its title, lowercased, with runs
/// of other characters as one dash.
pub(crate) fn section_anchor(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// The section a guide sits in: the last section whose first guide is at
/// or before it in [`DOCS`]. The index groups the guides the same way.
pub(crate) fn section_of(slug: &str) -> Option<&'static str> {
    let at = DOCS.iter().position(|(name, _)| *name == slug)?;
    SECTIONS
        .iter()
        .filter_map(|(title, _, first)| {
            DOCS.iter()
                .position(|(name, _)| name == first)
                .map(|start| (start, *title))
        })
        .filter(|(start, _)| *start <= at)
        .max_by_key(|(start, _)| *start)
        .map(|(_, title)| title)
}

/// The trail for a guide: Docs, its section on the index, and the guide.
pub(crate) fn doc_breadcrumb(slug: &str, title: &str) -> Breadcrumb {
    let mut trail = Breadcrumb::new(title).crumb("Docs", "/docs");
    if let Some(section) = section_of(slug) {
        trail = trail.crumb(section, format!("/docs#{}", section_anchor(section)));
    }
    trail
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/docs", get(docs_index))
        .route("/docs/{slug}", get(doc))
        // The download guide's old name.
        .route(
            "/docs/install",
            get(|| async { Redirect::permanent("/docs/download") }),
        )
        // The troubleshooting guide's old name.
        .route(
            "/docs/help",
            get(|| async { Redirect::permanent("/docs/troubleshooting") }),
        )
}

/// `/docs`: every guide, in reading order, under its section's title.
async fn docs_index(headers: HeaderMap) -> Response {
    let mut body = String::from(
        "<h1>Docs</h1><p class=\"oa-page-lead\">Guides to OpenAgents: the chat, Coder, the apps, \
plugins, and the Gym.</p>",
    );
    let mut open = false;
    for (slug, source) in DOCS {
        if let Some((title, lead, _)) = SECTIONS.iter().find(|(_, _, first)| *first == slug) {
            if open {
                body.push_str("</ol>");
            }
            body.push_str(&format!(
                "<h2 id=\"{}\">{}</h2><p class=\"oa-page-meta\">{}</p><ol class=\"oa-item-list\">",
                section_anchor(title),
                escape(title),
                escape(lead)
            ));
            open = true;
        }
        body.push_str(&format!(
            "<li><a href=\"/docs/{slug}\">{}</a></li>",
            escape(&markdown::title(source, slug))
        ));
    }
    if open {
        body.push_str("</ol>");
    }
    body.push_str(
        "<h2 id=\"api\">API</h2><p class=\"oa-page-meta\">Use our models from your own code. Beta.</p>\
<ol class=\"oa-item-list\"><li><a href=\"/docs/api\">API docs</a></li></ol>\
<h2 id=\"progress\">What works and what's next</h2><p class=\"oa-page-meta\">Each thing that \
works today with its proof, and what we're building next.</p><ol class=\"oa-item-list\">\
<li><a href=\"/promises\">What works today</a></li><li><a href=\"/roadmap\">Roadmap</a></li></ol>",
    );
    UiPage::new("Docs")
        .section("/docs")
        .path("/docs")
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(&headers)
}

/// `/docs/{slug}`: one guide, with the previous and next ones under it.
async fn doc(Path(slug): Path<String>, headers: HeaderMap) -> Response {
    // `/docs/{slug}.md`: the guide's Markdown twin.
    if let Some(name) = slug.strip_suffix(".md")
        && let Some((name, source)) = DOCS.iter().find(|(slug, _)| *slug == name)
    {
        return crate::agent_ready::guide_md(name, source);
    }
    let Some(index) = DOCS.iter().position(|(name, _)| *name == slug) else {
        return problem(
            &headers,
            StatusCode::NOT_FOUND,
            "Page not found",
            "No guide has that name.",
            ("/docs", "All docs"),
        );
    };
    let (_, source) = DOCS[index];
    let previous = index.checked_sub(1).map(|i| DOCS[i]);
    let next = DOCS.get(index + 1).copied();
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(markdown::render_document(source))))
        nav.oa-page-actions aria-label="More docs" {
            (action_link("All docs", "/docs"))
            @if let Some((name, text)) = previous {
                (action_link(
                    &format!("\u{2190} {}", markdown::title(text, name)),
                    &format!("/docs/{name}"),
                ))
            }
            @if let Some((name, text)) = next {
                (action_link(
                    &format!("{} \u{2192}", markdown::title(text, name)),
                    &format!("/docs/{name}"),
                ))
            }
        }
    });
    let title = markdown::title(source, &slug);
    UiPage::new(title.clone())
        .breadcrumb(doc_breadcrumb(&slug, &title))
        .section("/docs")
        .path(format!("/docs/{slug}"))
        .description(crate::agent_ready::summary(source))
        .scriptless()
        .content(content)
        .respond(&headers)
}

/// One document in the reading column, with a way home under it.
fn article(title: String, path: &str, source: &str, headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(markdown::render(source))))
        div.oa-page-actions { (action_link("Home", "/")) }
    });
    UiPage::new(title)
        .path(path)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn terms(headers: HeaderMap) -> Response {
    article(
        markdown::title(TERMS, "Terms of Service"),
        "/terms",
        TERMS,
        &headers,
    )
}

async fn privacy(headers: HeaderMap) -> Response {
    article(
        markdown::title(PRIVACY, "Privacy Policy"),
        "/privacy",
        PRIVACY,
        &headers,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both documents say what the Services do with what they collect and
    /// name the way out of it, as the published text does.
    #[test]
    fn the_legal_text_is_the_published_text() {
        assert!(TERMS.starts_with("# Terms of Service\nLast updated: 2026-10-09"));
        assert!(PRIVACY.starts_with("# Privacy Policy\nLast updated: 2026-10-09"));
        assert!(TERMS.contains("train, fine-tune, and evaluate the models and tools we run"));
        assert!(PRIVACY.contains("To train, fine-tune, and evaluate models we run or develop"));
        assert!(
            PRIVACY.contains("On a paid plan, you may ask us not to use the content you submit")
        );
        assert!(
            TERMS.contains("On a paid plan, you may ask us to stop using your content this way")
        );
        for document in [TERMS, PRIVACY] {
            assert!(document.contains("do not sell your"));
            assert!(document.contains("1101 W 34th St. #581, Austin, TX 78705"));
        }
    }

    /// Every section starts at a guide, the first guide starts a section,
    /// the sections are in reading order, and every guide has a title.
    #[test]
    fn the_sections_cover_the_docs_in_order() {
        assert_eq!(SECTIONS[0].2, DOCS[0].0);
        let mut last = None;
        for (title, _, first) in SECTIONS {
            let index = DOCS.iter().position(|(slug, _)| *slug == first);
            assert!(index.is_some(), "{title} starts at {first}");
            assert!(index > last, "{title} is out of order");
            last = index;
        }
        for (slug, source) in DOCS {
            assert!(source.starts_with("# "), "{slug} has a title");
        }
    }

    /// Every guide resolves to exactly the section the index lists it
    /// under, and every section anchor is unique and not the API's.
    #[test]
    fn every_guide_resolves_its_index_section() {
        use maud::Render;
        let mut current = None;
        for (slug, _) in DOCS {
            if let Some((title, _, _)) = SECTIONS.iter().find(|(_, _, first)| *first == slug) {
                current = Some(*title);
            }
            assert_eq!(section_of(slug), current, "{slug}");
            let trail = doc_breadcrumb(slug, "T").render().into_string();
            assert!(trail.contains(r#"href="/docs">Docs</a>"#), "{slug}");
            assert!(trail.contains(r#"aria-current="page""#), "{slug}");
            let anchor = section_anchor(current.unwrap());
            assert!(
                trail.contains(&format!(r#"href="/docs#{anchor}""#)),
                "{slug}"
            );
        }
        let anchors: Vec<_> = SECTIONS.iter().map(|(t, _, _)| section_anchor(t)).collect();
        for (i, a) in anchors.iter().enumerate() {
            assert!(!a.is_empty() && a != "api" && !anchors[..i].contains(a));
        }
        assert_eq!(
            section_anchor("The Gym, the Verse, and the wallet"),
            "the-gym-the-verse-and-the-wallet"
        );
    }

    /// The how-to verbs that start an instruction to the reader.
    const HOW_TO: [&str; 19] = [
        "open", "tap", "click", "install", "run", "download", "go to", "visit", "connect",
        "sign in", "log in", "choose", "pick", "type", "scan", "enter", "paste", "press", "select",
    ];

    /// Instructions that rightly carry no link or command: the guide, the
    /// start of the sentence, and why. Fix the guide instead when you can.
    const UNLINKED: [(&str, &str, &str); 0] = [];

    /// A guide's paragraphs and list items, outside code blocks, headings,
    /// and tables, each on one line. A paragraph ending in `:` just before
    /// a code block is marked with a code span: the block is its command.
    fn passages(source: &str) -> Vec<String> {
        let mut passages: Vec<String> = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        let mut fenced = false;
        let flush = |current: &mut Vec<&str>, passages: &mut Vec<String>| {
            if !current.is_empty() {
                passages.push(current.join(" "));
                current.clear();
            }
        };
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with("```") {
                flush(&mut current, &mut passages);
                if !fenced && let Some(last) = passages.last_mut() {
                    if last.ends_with(':') {
                        last.push_str(" `…`");
                    }
                }
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if line.is_empty() || line.starts_with('#') || line.starts_with('|') {
                flush(&mut current, &mut passages);
                continue;
            }
            let item = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| {
                    let digits = line.find(|c: char| !c.is_ascii_digit())?;
                    (digits > 0).then(|| line[digits..].strip_prefix(". "))?
                });
            if let Some(item) = item {
                flush(&mut current, &mut passages);
                current.push(item.trim_start());
            } else {
                current.push(line.trim_start_matches("> "));
            }
        }
        flush(&mut current, &mut passages);
        passages
    }

    /// The sentence in `passage` that starts with a how-to verb, if any.
    /// A quoted sentence is an example message, not an instruction.
    fn instruction(passage: &str) -> Option<String> {
        passage
            .replace(". ", ".\n")
            .replace("? ", "?\n")
            .replace("! ", "!\n")
            .lines()
            .map(|sentence| sentence.trim_start_matches([' ', '*', '_', '(']))
            .find(|sentence| {
                let lower = sentence.to_ascii_lowercase();
                HOW_TO.iter().any(|verb| {
                    lower.starts_with(verb)
                        && !lower[verb.len()..].starts_with(|c: char| c.is_ascii_alphabetic())
                })
            })
            .map(str::to_owned)
    }

    /// The owner's rule (2026-10-09): advice to do something is always a
    /// link to follow or a command to run. Every paragraph or list item in
    /// the served docs that tells the reader to open, install, run, … must
    /// carry a Markdown link or a code span. A content lint over reviewed
    /// guides, not a reading of anyone's message.
    #[test]
    fn every_instruction_carries_a_link_or_a_command() {
        let guides = DOCS
            .iter()
            .map(|(slug, source)| (format!("docs/{slug}"), *source))
            .chain(
                crate::pages::api_docs::API_DOCS
                    .iter()
                    .map(|(slug, source)| (format!("docs/api/{slug}"), *source)),
            );
        let mut bare = Vec::new();
        for (path, source) in guides {
            for passage in passages(source) {
                if passage.contains("](") || passage.contains('`') {
                    continue;
                }
                let Some(sentence) = instruction(&passage) else {
                    continue;
                };
                let allowed = UNLINKED
                    .iter()
                    .any(|(guide, start, _)| *guide == path && sentence.starts_with(start));
                if !allowed {
                    bare.push(format!("{path}: {sentence}"));
                }
            }
        }
        assert!(
            bare.is_empty(),
            "instructions with no link or command:\n{}",
            bare.join("\n")
        );
    }

    /// The lint finds a bare instruction, and passes one that links or
    /// carries its command.
    #[test]
    fn the_instruction_lint_reads_paragraphs_and_items() {
        let found = passages(
            "Intro.\n\n1. Open the app.\n2. Run `coder login`.\n\nEnter:\n\n```sh\ncoder\n```\n",
        );
        assert_eq!(found[1], "Open the app.");
        assert_eq!(instruction(&found[1]).as_deref(), Some("Open the app."));
        assert!(found[2].contains('`'));
        assert!(found[3].ends_with("`…`"));
        assert_eq!(instruction("The Enter key sends."), None);
        assert_eq!(instruction("Opening it is quick."), None);
        assert_eq!(instruction("Ask \"open my wallet\"."), None);
        assert_eq!(
            instruction("See openagents.com. Run it.").as_deref(),
            Some("Run it.")
        );
    }

    /// The plugin guides use the glossary's one vocabulary: a plugin's
    /// parts are skills, workflows, knowledge, Wasm, and tests, and no
    /// part of a plugin is ever called a tool.
    #[test]
    fn the_plugin_guides_use_one_vocabulary() {
        let plugins = DOCS
            .iter()
            .find(|(slug, _)| *slug == "plugins")
            .map(|(_, source)| *source)
            .unwrap();
        for part in ["Skills", "Workflows", "Knowledge", "Wasm", "Tests"] {
            assert!(plugins.contains(&format!("**{part}.**")), "{part}");
        }
        for (slug, source) in DOCS {
            let words = source
                .split(|c: char| !c.is_ascii_alphabetic())
                .map(str::to_ascii_lowercase);
            for word in words {
                assert!(word != "tool" && word != "tools", "{slug} says {word}");
            }
        }
    }
}
