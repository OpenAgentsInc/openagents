//! The pages that serve a Markdown document: the terms of service and the
//! privacy policy.
//!
//! Both documents are compiled into the binary, so a deploy cannot fail to
//! copy one and leave a legal page answering `404`. They are the text
//! openagents.com published, last updated 2026-09-03, unchanged.

use axum::Router;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::page;
use crate::markdown;

/// The terms of service, as Markdown.
pub(crate) const TERMS: &str = include_str!("../../content/legal/terms.md");

/// The privacy policy, as Markdown.
pub(crate) const PRIVACY: &str = include_str!("../../content/legal/privacy.md");

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
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
}
