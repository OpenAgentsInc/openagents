//! The Coder Light / Coder Noir design-language assets from `openagents-ui`
//! and the no-JavaScript theme toggle.
//!
//! With JavaScript, `openagents_ui::script()` flips `<html data-theme>` in
//! place and writes the [`THEME_COOKIE`]. Without it, the toggle's fallback
//! form posts here; the answer sets the same cookie and returns to the page.

use axum::Form;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use openagents_ui::shell::{THEME_COOKIE, Theme};
use serde::Deserialize;

/// Served stylesheet, script, and vendored Alpine.js CSP build.
pub const STYLESHEET_PATH: &str = "/static/ui.css";
pub const SCRIPT_PATH: &str = "/static/ui.js";
pub const ALPINE_PATH: &str = "/static/vendor/alpine-csp.js";
pub const TOGGLE_PATH: &str = "/theme";

/// The design-language stylesheet link. Every page carries it, including
/// pages that must run no script.
#[must_use]
pub fn style_tag() -> String {
    format!(
        "<link rel=\"stylesheet\" href=\"{STYLESHEET_PATH}?v={}\">",
        openagents_ui::stylesheet_version()
    )
}

/// The component script and Alpine, for pages that allow scripts. The
/// component script loads first so its `Alpine.data` registrations exist
/// when Alpine starts. Pages that must run no script omit these; their theme
/// toggle falls back to a form that posts to [`TOGGLE_PATH`].
#[must_use]
pub fn script_tags() -> String {
    format!(
        "<script src=\"{SCRIPT_PATH}?v={}\" defer></script>\
<script src=\"{ALPINE_PATH}\" defer></script>",
        openagents_ui::script_version(),
    )
}

/// The explicit theme the request's cookie names, if any.
#[must_use]
pub fn from_headers(headers: &HeaderMap) -> Option<Theme> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == THEME_COOKIE)
        .and_then(|(_, value)| Theme::from_cookie(value))
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    )
        .into_response()
}

pub async fn stylesheet() -> Response {
    asset("text/css; charset=utf-8", openagents_ui::stylesheet())
}

pub async fn script() -> Response {
    asset("text/javascript; charset=utf-8", openagents_ui::script())
}

pub async fn alpine() -> Response {
    asset(
        "text/javascript; charset=utf-8",
        openagents_ui::assets::ALPINE_CSP_JS,
    )
}

#[derive(Deserialize)]
pub struct Toggle {
    #[serde(default)]
    return_to: String,
}

/// Flip the stored theme and go back. With no stored choice the browser's
/// `Sec-CH-Prefers-Color-Scheme` hint decides what "the opposite" is; without
/// the hint the system setting is unknown and the toggle chooses light, since
/// the site's legacy pages are dark.
pub async fn toggle(headers: HeaderMap, Form(form): Form<Toggle>) -> Response {
    let next = match from_headers(&headers) {
        Some(Theme::Dark) => Theme::Light,
        Some(Theme::Light) => Theme::Dark,
        None => match headers
            .get("sec-ch-prefers-color-scheme")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim_matches('"'))
        {
            Some("light") => Theme::Dark,
            _ => Theme::Light,
        },
    };
    let back = safe_return(&form.return_to);
    let cookie = format!(
        "{THEME_COOKIE}={}; Path=/; Max-Age=31536000; SameSite=Lax",
        next.as_str()
    );
    let mut response = (StatusCode::SEE_OTHER, [(header::LOCATION, back)]).into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

/// Only same-site paths: `/x` but never `//host`, `/\host`, or a scheme.
fn safe_return(path: &str) -> String {
    let ok = path.starts_with('/')
        && !path.starts_with("//")
        && !path.starts_with("/\\")
        && !path.chars().any(|c| c.is_control());
    if ok { path.to_owned() } else { "/".to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_paths_stay_on_this_site() {
        assert_eq!(safe_return("/chat?x=1"), "/chat?x=1");
        for bad in [
            "//evil.example",
            "/\\evil",
            "https://evil",
            "",
            "chat",
            "/a\nb",
        ] {
            assert_eq!(safe_return(bad), "/", "{bad:?}");
        }
    }

    #[test]
    fn cookie_theme_is_read_among_other_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; oa_theme=light; b=2"),
        );
        assert_eq!(from_headers(&headers), Some(Theme::Light));
        headers.insert(header::COOKIE, HeaderValue::from_static("oa_theme=bogus"));
        assert_eq!(from_headers(&headers), None);
    }

    #[test]
    fn script_tags_load_components_before_alpine_and_inline_nothing() {
        let tags = format!("{}{}", style_tag(), script_tags());
        let ui = tags.find(SCRIPT_PATH).unwrap();
        let alpine = tags.find(ALPINE_PATH).unwrap();
        assert!(ui < alpine);
        assert!(!tags.contains("<script>") && !tags.contains("style="));
    }
}
