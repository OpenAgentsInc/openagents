//! `/new-lander/flow`: a clickable prototype of Kiki's playtest flow (landing,
//! login, the galaxy map, the Haunted House, edit mode, publish and share,
//! planet setup, and a walkable planet). Behind the private preview like the
//! rest of `/new-lander`. The house screens frame the real house page
//! ([`super::new_lander_house::HOUSE`]), which allows same-origin framing; the
//! script hides that page's own controls so only the flow's show.
//!
//! The same two files are published as a Claude artifact, where the house is
//! drawn instead; the script checks its path to choose.

use axum::{
    Router,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};

use crate::App;

pub(crate) const PATH: &str = "/new-lander/flow";
const SCRIPT_PATH: &str = "/new-lander/flow.js";
const BODY: &str = include_str!("../../content/verse-flow/page.html");
const SCRIPT: &str = include_str!("../../content/verse-flow/flow.js");
const POLICY: &str = "default-src 'none'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; script-src 'self'; frame-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(page))
        .route(SCRIPT_PATH, get(script))
}

async fn page() -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\"><meta name=\"robots\" content=\"noindex\">{BODY}</html>"
    );
    let mut response = axum::response::Html(html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        SCRIPT,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flow_loads_its_script_and_may_frame_the_house() {
        assert!(BODY.contains(r#"<script src="flow.js"></script>"#));
        assert!(
            SCRIPT.contains("'/new-lander/house'")
                && SCRIPT.contains("startsWith('/new-lander/flow')")
        );
        assert!(POLICY.contains("frame-src 'self'") && POLICY.contains("script-src 'self';"));
        assert!(crate::preview::gated(PATH) && crate::preview::gated(SCRIPT_PATH));
    }
}
