//! `/landernew`: Kiki's playtest trail on the real Aethyr worlds, next to
//! the `/new-lander` review routes, which stay as they are. Behind the
//! private preview like them.
//!
//! - `/landernew` is the landing page and the login card from the flow
//!   prototype ([`super::verse_flow`]); both lead to the map.
//! - `/landernew/map` and `/landernew/house` are the real universe and
//!   Haunted House pages ([`super::new_lander`], [`super::new_lander_house`]),
//!   marked `kiki` for the Verse UI's glass presentation, with their links
//!   pointed back into this trail.
//! - Entering sets [`COOKIE`], so the personal-planet, invitation, visit and
//!   player pages the house leads to (under `/new-lander/`) get the same
//!   presentation through [`carry`].

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::Response,
    routing::get,
};

use crate::App;

pub(crate) const HOME: &str = "/landernew";
pub(crate) const MAP: &str = "/landernew/map";
pub(crate) const HOUSE: &str = "/landernew/house";
const SCRIPT_PATH: &str = "/landernew/flow.js";
const COOKIE_NAME: &str = "oa_lander";
const COOKIE: &str = "oa_lander=kiki; Path=/; Max-Age=86400; SameSite=Lax; Secure; HttpOnly";
/// The `/new-lander/` pages the trail continues into.
const CARRIED: [&str; 4] = [
    "/new-lander/planet/",
    "/new-lander/invite/",
    "/new-lander/visit/",
    "/new-lander/play/",
];
/// Largest page [`carry`] rewrites; anything bigger passes through.
const MOST_CARRIED: usize = 4 << 20;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(HOME, get(home))
        .route(SCRIPT_PATH, get(super::verse_flow::script))
        .route(MAP, get(map))
        .route(HOUSE, get(house))
}

/// The landing page and login card. `#lander` tells the script where they
/// lead and what the login button says.
async fn home(State(app): State<App>) -> Response {
    let google = crate::workos_auth::on(&app)
        .is_some_and(|(_, w)| w.client.settings().offers(oa_auth::workos::Method::Google));
    let back = "%2Flandernew%2Fmap%3Fworld%3Dhaunted-house";
    let (login, label) = if google {
        (format!("/auth/workos/google?return_to={back}"), "")
    } else {
        (format!("/login?return_to={back}"), "Log in")
    };
    let meta = format!(
        "<meta id=\"lander\" data-map=\"{MAP}?world=haunted-house\" data-login=\"{login}\" data-login-label=\"{label}\">"
    );
    let body = super::verse_flow::BODY.replacen(
        "<script src=\"flow.js\"></script>",
        &format!("{meta}<script src=\"{SCRIPT_PATH}\"></script>"),
        1,
    );
    let mut response = super::verse_flow::page_response(&body);
    mark(&mut response);
    response
}

async fn map(state: State<App>, headers: HeaderMap) -> Response {
    let mut response = kiki(super::new_lander::universe(state, headers).await).await;
    mark(&mut response);
    response
}

async fn house(state: State<App>, headers: HeaderMap) -> Response {
    let mut response = kiki(super::new_lander_house::page(state, headers).await).await;
    mark(&mut response);
    response
}

fn mark(response: &mut Response) {
    response
        .headers_mut()
        .append(header::SET_COOKIE, HeaderValue::from_static(COOKIE));
}

fn entered(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .any(|pair| pair.trim() == format!("{COOKIE_NAME}=kiki"))
}

/// Middleware: a trail page under `/new-lander/` gets the `kiki`
/// presentation when this browser entered through `/landernew`.
pub(crate) async fn carry(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if !(CARRIED.iter().any(|p| path.starts_with(p)) && entered(request.headers())) {
        return next.run(request).await;
    }
    kiki(next.run(request).await).await
}

/// Mark an HTML page `kiki` and point its trail links into `/landernew`.
async fn kiki(response: Response) -> Response {
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !html {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MOST_CARRIED).await else {
        return Response::from_parts(parts, Body::empty());
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(rewrite(text)))
}

fn rewrite(text: &str) -> String {
    let text = if text.contains("<body class=\"") {
        text.replacen("<body class=\"", "<body class=\"kiki ", 1)
    } else {
        text.replacen("<body", "<body class=\"kiki\"", 1)
    };
    text.replacen("<html ", "<html class=\"kiki\" ", 1)
        .replace("\"/new-lander/verse", "\"/landernew/map")
        .replace("%2Fnew-lander%2Fverse", "%2Flandernew%2Fmap")
        .replace("\"/new-lander/house\"", "\"/landernew/house\"")
        .replace("%2Fnew-lander%2Fhouse", "%2Flandernew%2Fhouse")
        .replace("href=\"/new-lander\"", "href=\"/landernew\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_pages_are_marked_and_point_back_into_the_trail() {
        let page = r#"<html lang="en"><body class="verse-page"><a href="/new-lander">Home</a><a id="world-map" href="/new-lander/verse?world=haunted-house">Map</a><a href="/login?return_to=%2Fnew-lander%2Fhouse">Sign in</a><script>{"hauntPath":"/new-lander/house","mapPath":"/new-lander/verse"}</script><script src="/new-lander/house/boot.js"></script></body></html>"#;
        let out = rewrite(page);
        assert!(out.starts_with(r#"<html class="kiki" lang="en"><body class="kiki verse-page">"#));
        assert!(out.contains(r#"href="/landernew""#));
        assert!(out.contains(r#"href="/landernew/map?world=haunted-house""#));
        assert!(out.contains("return_to=%2Flandernew%2Fhouse"));
        assert!(out.contains(r#""hauntPath":"/landernew/house","mapPath":"/landernew/map""#));
        // The engine's boot script keeps its address.
        assert!(out.contains(r#"src="/new-lander/house/boot.js""#));
        assert!(
            rewrite("<html lang=\"en\"><head></head><body><main>")
                .contains("<body class=\"kiki\"><main>")
        );
    }

    #[test]
    fn the_trail_is_private_and_remembered() {
        for path in [HOME, MAP, HOUSE, SCRIPT_PATH] {
            assert!(crate::preview::gated(path), "{path}");
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("a=b; oa_lander=kiki"),
        );
        assert!(entered(&headers));
        headers.insert(header::COOKIE, HeaderValue::from_static("oa_lander=kikiX"));
        assert!(!entered(&headers));
        assert!(super::super::verse_flow::BODY.contains("<script src=\"flow.js\"></script>"));
    }
}
