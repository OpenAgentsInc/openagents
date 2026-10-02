//! The pages that serve a Markdown document: the terms of service, the
//! privacy policy, and the docs.
//!
//! Every document is compiled into the binary, so a deploy cannot fail to
//! copy one and leave a page answering `404`. The terms and the policy are
//! the text openagents.com published, last updated 2026-09-03, unchanged.
//! The docs are short guides to the apps we launch and to plugins, in
//! `content/docs/`.

use axum::Router;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::{escape, page, problem};
use crate::markdown;

/// The terms of service, as Markdown.
pub(crate) const TERMS: &str = include_str!("../../content/legal/terms.md");

/// The privacy policy, as Markdown.
pub(crate) const PRIVACY: &str = include_str!("../../content/legal/privacy.md");

/// The docs, by slug, in reading order.
pub(crate) const DOCS: [(&str, &str); 13] = [
    (
        "what-is-openagents",
        include_str!("../../content/docs/what-is-openagents.md"),
    ),
    ("download", include_str!("../../content/docs/download.md")),
    (
        "connect-a-computer",
        include_str!("../../content/docs/connect-a-computer.md"),
    ),
    ("chat", include_str!("../../content/docs/chat.md")),
    ("coder", include_str!("../../content/docs/coder.md")),
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
    ("verse", include_str!("../../content/docs/verse.md")),
    // Its image is the Grid seen from above as the desktop app's backdrop
    // draws it, captured from the live relay with
    // `crates/verse/examples/overlook_capture.rs`
    // (`OVERLOOK_SIZE=1600x900 … /tmp/grid.png 45 wss://relay.openagents.com 8`)
    // and served from `/static/verse-grid.jpg`.
    ("the-grid", include_str!("../../content/docs/the-grid.md")),
    (
        "privacy-and-security",
        include_str!("../../content/docs/privacy-and-security.md"),
    ),
    ("help", include_str!("../../content/docs/help.md")),
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
}

/// `/docs`: every guide, in reading order.
async fn docs_index() -> Response {
    let mut body = String::from(
        "<h1>Docs</h1><p class=\"label\">Short guides to OpenAgents for iPhone and Mac, and to plugins.</p>\
<ol class=\"list docs\">",
    );
    for (slug, source) in DOCS {
        body.push_str(&format!(
            "<li><a class=\"title\" href=\"/docs/{slug}\">{}</a></li>",
            escape(&markdown::title(source, slug))
        ));
    }
    body.push_str("</ol>");
    page("Docs", Some("/docs"), &body)
}

/// `/docs/{slug}`: one guide, with the previous and next ones under it.
async fn doc(Path(slug): Path<String>) -> Response {
    let Some(index) = DOCS.iter().position(|(name, _)| *name == slug) else {
        return problem(
            StatusCode::NOT_FOUND,
            "Page not found",
            "No guide has that name.",
            ("/docs", "All docs"),
        );
    };
    let (_, source) = DOCS[index];
    let mut links = String::from("<a href=\"/docs\">[ All docs ]</a>");
    if let Some((previous, text)) = index.checked_sub(1).map(|i| DOCS[i]) {
        links.push_str(&format!(
            " <a href=\"/docs/{previous}\">[ \u{2190} {} ]</a>",
            escape(&markdown::title(text, previous))
        ));
    }
    if let Some((next, text)) = DOCS.get(index + 1) {
        links.push_str(&format!(
            " <a href=\"/docs/{next}\">[ {} \u{2192} ]</a>",
            escape(&markdown::title(text, next))
        ));
    }
    let title = markdown::title(source, &slug);
    page(
        &title,
        Some("/docs"),
        &format!(
            "<article class=\"md\">{}</article><p class=\"meta\">{links}</p>",
            markdown::render_document(source)
        ),
    )
}

/// One document in its frame, with a way home under it.
fn article(source: &str) -> String {
    format!(
        "<article class=\"md\">{}</article><p class=\"meta\"><a href=\"/\">[ Home ]</a></p>",
        markdown::render(source)
    )
}

async fn terms() -> Response {
    page(
        &markdown::title(TERMS, "Terms of Service"),
        None,
        &article(TERMS),
    )
}

async fn privacy() -> Response {
    page(
        &markdown::title(PRIVACY, "Privacy Policy"),
        None,
        &article(PRIVACY),
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
