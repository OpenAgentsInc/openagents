//! Progressive GPUI entry for account memory and schedules. The HTML forms
//! remain usable without JavaScript or a WebGPU adapter.
use crate::App;
use axum::http::{HeaderMap, header};
use axum::response::IntoResponse;
use axum::routing::get;
use maud::{Markup, html};

const SCRIPT: &str = "/settings/account-prompts.js";
const SOURCE: &str = r#"
const page = new URL(import.meta.url).searchParams.get('page');
if ((page === 'memory' || page === 'schedules') && navigator.gpu) {
  navigator.gpu.requestAdapter().then(adapter => {
    if (adapter) location.replace('/gpui?ui=gpui&settings=' + page);
  }).catch(() => {});
}
"#;

pub(crate) fn routes() -> axum::Router<App> {
    axum::Router::new().route(
        SCRIPT,
        get(|| async {
            (
                [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                SOURCE,
            )
                .into_response()
        }),
    )
}

pub(crate) fn entry(app: &App, headers: &HeaderMap, query: Option<&str>, page: &str) -> Markup {
    if crate::pages::gpui_chat::choice(headers, query).serves(app, headers) {
        html! { script type="module" src=(format!("{SCRIPT}?page={page}")) {} }
    } else {
        html! {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enhancement_keeps_the_html_form_as_fallback() {
        assert!(SOURCE.contains("navigator.gpu.requestAdapter()"));
        assert!(SOURCE.contains("/gpui?ui=gpui&settings="));
        assert!(!SOURCE.contains("document.body"));
    }
}
