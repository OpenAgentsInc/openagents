//! The pages that serve a Markdown document: the terms of service, the
//! privacy policy, and the docs.
//!
//! Every document is compiled into the binary, so a deploy cannot fail to
//! copy one and leave a page answering `404`. The terms and the policy are
//! the text openagents.com published, last updated 2026-09-03, unchanged.
//! The docs are the user guide to OpenAgents, grouped into sections, in
//! `content/docs/`.

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Redirect, Response};
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::layout::escape;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, prose};

/// The terms of service, as Markdown.
pub(crate) const TERMS: &str = include_str!("../../content/legal/terms.md");

/// The privacy policy, as Markdown.
pub(crate) const PRIVACY: &str = include_str!("../../content/legal/privacy.md");

/// The docs, by slug, in reading order.
pub(crate) const DOCS: [(&str, &str); 30] = [
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
    (
        "troubleshooting",
        include_str!("../../content/docs/troubleshooting.md"),
    ),
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
        "Connect your phone to a computer, and manage them.",
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
        "Settings and fixes for common problems.",
        "settings",
    ),
    (
        "Questions and words",
        "Short answers, and what each word means.",
        "faq",
    ),
];

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
                "<h2>{}</h2><p class=\"oa-page-meta\">{}</p><ol class=\"oa-item-list\">",
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
    UiPage::new("Docs")
        .section("/docs")
        .path("/docs")
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(&headers)
}

/// `/docs/{slug}`: one guide, with the previous and next ones under it.
async fn doc(Path(slug): Path<String>, headers: HeaderMap) -> Response {
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
    UiPage::new(markdown::title(source, &slug))
        .section("/docs")
        .path(format!("/docs/{slug}"))
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
        assert!(TERMS.starts_with("# Terms of Service\nLast updated: 2026-09-03"));
        assert!(PRIVACY.starts_with("# Privacy Policy\nLast updated: 2026-09-03"));
        assert!(TERMS.contains("train, fine-tune, and evaluate the models and tools we run"));
        assert!(PRIVACY.contains("To train, fine-tune, and evaluate models we run or develop"));
        assert!(PRIVACY.contains("You may ask us not to use the content you submit"));
        assert!(TERMS.contains("You may ask us to stop using your content this way"));
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
