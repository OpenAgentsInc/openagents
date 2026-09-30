//! The pages that serve a Markdown document: the terms of service and the
//! privacy policy, the docs, and the blog.
//!
//! Every document is compiled into the binary, so a deploy cannot fail to
//! copy one and leave a legal page answering `404`. The terms and the
//! policy are the text openagents.com published, last updated 2026-09-03,
//! unchanged. The docs and the blog post are the public Coder pages the
//! same site served.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::{escape, page, problem};
use crate::markdown;

/// The terms of service, as Markdown.
pub(crate) const TERMS: &str = include_str!("../../content/legal/terms.md");

/// The privacy policy, as Markdown.
pub(crate) const PRIVACY: &str = include_str!("../../content/legal/privacy.md");

/// The docs, by slug, in the order the index lists them.
const DOCS: [(&str, &str); 7] = [
    ("about", include_str!("../../content/docs/about.md")),
    ("install", include_str!("../../content/docs/install.md")),
    ("plugins", include_str!("../../content/docs/plugins.md")),
    ("changelog", include_str!("../../content/docs/changelog.md")),
    (
        "release-notes-0.5.0",
        include_str!("../../content/docs/release-notes-0.5.0.md"),
    ),
    (
        "release-notes-0.4.1",
        include_str!("../../content/docs/release-notes-0.4.1.md"),
    ),
    (
        "release-notes-0.4.0",
        include_str!("../../content/docs/release-notes-0.4.0.md"),
    ),
];

/// The blog's posts, newest first.
const POSTS: [(&str, &str); 1] = [(
    "introducing-coder",
    include_str!("../../content/blog/introducing-coder.md"),
)];

/// The page whose channel rows carry the version each channel names.
const CHANNEL_PAGE: &str = "install";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/docs", get(docs_index))
        .route("/docs/{slug}", get(doc))
        .route("/doc", get(|| async { Redirect::permanent("/docs") }))
        .route(
            "/doc/{slug}",
            get(|Path(slug): Path<String>| async move {
                Redirect::permanent(&format!("/docs/{}", crate::layout::segment(&slug)))
            }),
        )
        .route("/blog", get(blog_index))
        .route("/blog/{slug}", get(post))
}

/// One document in its frame, with the links under it.
fn article(source: &str, link_base: &str, footer: &str) -> String {
    format!(
        "<article class=\"md\">{}</article><p class=\"meta\">{footer}</p>",
        markdown::render(source, link_base)
    )
}

async fn terms() -> Response {
    let title = markdown::title(TERMS, "Terms of Service");
    page(
        &title,
        None,
        &article(TERMS, "/docs", "<a href=\"/\">[ Home ]</a>"),
    )
}

async fn privacy() -> Response {
    let title = markdown::title(PRIVACY, "Privacy Policy");
    page(
        &title,
        None,
        &article(PRIVACY, "/docs", "<a href=\"/\">[ Home ]</a>"),
    )
}

/// A listing of documents under `base`.
fn listing(heading: &str, lead: &str, base: &str, items: &[(&str, &str)]) -> String {
    let mut body = format!(
        "<h1>{}</h1><p class=\"label\">{}</p><ul class=\"list\">",
        escape(heading),
        escape(lead)
    );
    for (slug, source) in items {
        let description = markdown::description(source);
        body.push_str(&format!(
            "<li><a class=\"title\" href=\"{base}/{slug}\">{}</a>",
            escape(&markdown::title(source, slug))
        ));
        if !description.is_empty() {
            body.push_str(&format!("<p>{}</p>", escape(&description)));
        }
        body.push_str("</li>");
    }
    body.push_str("</ul>");
    body
}

async fn docs_index() -> Response {
    page(
        "Documentation",
        Some("/docs"),
        &listing(
            "OpenAgents Documentation",
            "Guides, references, and system documentation.",
            "/docs",
            &DOCS,
        ),
    )
}

async fn doc(State(app): State<App>, Path(slug): Path<String>) -> Response {
    let Some((_, source)) = DOCS.iter().find(|(name, _)| *name == slug) else {
        return problem(
            StatusCode::NOT_FOUND,
            "Documentation page not found",
            "No documentation page has that name.",
            ("/docs", "Back to Docs"),
        );
    };
    let source = if slug == CHANNEL_PAGE {
        let (stable, rc) = super::releases::channel_versions(&app).await;
        substitute_channels(source, &stable, &rc)
    } else {
        (*source).to_owned()
    };
    page(
        &markdown::title(&source, &slug),
        Some("/docs"),
        &article(
            &source,
            "/docs",
            "<a href=\"/docs\">[ Back to Docs ]</a> \u{b7} <a href=\"/\">[ Home ]</a>",
        ),
    )
    .into_response()
}

/// Fills the install page's two channel placeholders. These are the only
/// tokens replaced, and only on that page, so no other document's text is
/// rewritten from the network; the render escapes what it inserts.
pub(crate) fn substitute_channels(source: &str, stable: &str, rc: &str) -> String {
    source.replace("{{stable}}", stable).replace("{{rc}}", rc)
}

async fn blog_index() -> Response {
    page(
        "Blog",
        Some("/blog"),
        &listing(
            "OpenAgents Blog",
            "News and posts from OpenAgents.",
            "/blog",
            &POSTS,
        ),
    )
}

async fn post(Path(slug): Path<String>) -> Response {
    let Some((_, source)) = POSTS.iter().find(|(name, _)| *name == slug) else {
        return problem(
            StatusCode::NOT_FOUND,
            "Post not found",
            "No blog post has that name.",
            ("/blog", "Back to Blog"),
        );
    };
    page(
        &markdown::title(source, &slug),
        Some("/blog"),
        &article(
            source,
            "/docs",
            "<a href=\"/blog\">[ Back to Blog ]</a> \u{b7} <a href=\"/\">[ Home ]</a>",
        ),
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

    #[test]
    fn the_install_page_carries_the_channel_placeholders() {
        let (_, install) = DOCS.iter().find(|(s, _)| *s == CHANNEL_PAGE).unwrap();
        assert!(install.contains("{{stable}}") && install.contains("{{rc}}"));
        let filled = substitute_channels(install, "0.4.0", "unknown");
        assert!(!filled.contains("{{"));
        assert!(filled.contains("`0.4.0`"));
    }
}
