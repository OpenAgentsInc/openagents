//! Shared Verse presentation. Rust controllers retain account and project authority.
use crate::App;
use axum::{Router, http::header, routing::get};

const CSS: &str = include_str!("../../content/verse-ui/ui.css");
const JS: &str = include_str!("../../content/verse-ui/ui.js");
const STYLE: &str = "<link rel=\"stylesheet\" href=\"/new-lander/ui.css\">";
const SCRIPT: &str = "<script type=\"module\" src=\"/new-lander/ui.js\"></script>";

/// Draw a labelled control's decorative HUD icon without a browser dependency.
pub(super) fn icon(name: &str) -> maud::Markup {
    let path = match name {
        "map" => "m3 5 6-2 6 2 6-2v16l-6 2-6-2-6 2V5Zm6-2v16m6-14v16",
        "reload" => "M20 7v5h-5M20 12a8 8 0 1 0-2 6",
        "mesh" => "m12 3 9 5v8l-9 5-9-5V8l9-5Zm0 0v18M3 8l9 5 9-5M3 16l9-3 9 3",
        "edit" => "m15 4 5 5M4 20l5-1L21 7a2 2 0 0 0-5-5L4 15v5Z",
        "save" => "M5 3h12l4 4v14H3V3h2Zm2 0v7h10V3M7 21v-7h10v7",
        "expand" => "M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6",
        "objects" => "m12 3 9 5-9 5-9-5 9-5Zm-9 5v9l9 5 9-5V8m-9 5v9",
        "materials" => {
            "M12 3a9 9 0 1 0 0 18h2a2 2 0 0 0 0-4h-1a2 2 0 0 1 0-4h3a5 5 0 0 0 0-10h-4ZM7 8h.01M12 6h.01M6 13h.01"
        }
        "request" => "M4 4h16v12H9l-5 4V4Zm4 5h8m-8 3h5",
        "compile" => "m8 5-6 7 6 7m8-14 6 7-6 7m-3-16-2 18",
        "history" => "M3 5v5h5M3 10a9 9 0 1 1 1 7m8-11v6l4 2",
        "mic" => "M9 5a3 3 0 0 1 6 0v7a3 3 0 0 1-6 0V5Zm-3 6v1a6 6 0 0 0 12 0v-1m-6 7v4m-4 0h8",
        "stop" => "M6 6h12v12H6V6Z",
        "play" => "m8 4 12 8-12 8V4Z",
        "share" => "M14 3h7v7m0-7L10 14m-1-9H3v16h16v-6",
        _ => "M12 3v18M3 12h18",
    };
    maud::html! {
        svg.hud-icon viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" { path d=(path) {} }
    }
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/new-lander/ui.css",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/css; charset=utf-8"),
                        (header::CACHE_CONTROL, "no-cache"),
                    ],
                    CSS,
                )
            }),
        )
        .route(
            "/new-lander/ui.js",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
                        (header::CACHE_CONTROL, "no-cache"),
                    ],
                    JS,
                )
            }),
        )
}

/// Add presentation without replacing controls, forms, canvases, or the engine frame.
pub(crate) fn document(body: String, surface: &str) -> String {
    assert!(matches!(
        surface,
        "marketing" | "universe" | "studio" | "player" | "setup" | "invite" | "auth"
    ));
    let body = body.replacen("<html ", &format!("<html data-verse-ui=\"{surface}\" "), 1);
    let assets = if matches!(surface, "marketing" | "auth") {
        STYLE.into()
    } else {
        format!("{STYLE}{SCRIPT}")
    };
    let output = body.replacen("</head>", &format!("{assets}</head>"), 1);
    #[cfg(test)]
    if let Ok(directory) = std::env::var("OPENAGENTS_VERSE_UI_CAPTURES") {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).expect("review capture directory");
        std::fs::write(
            directory.join(format!(
                "{surface}-{}.html",
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            &output,
        )
        .expect("synthetic review capture");
    }
    output
}

/// Keep the Verse sign-in handoff on its presentation without changing authentication.
pub(crate) fn authentication(
    page: crate::ui_page::UiPage,
    headers: &axum::http::HeaderMap,
    status: axum::http::StatusCode,
    return_to: &str,
) -> axum::response::Response {
    use axum::response::{Html, IntoResponse};
    if return_to == "/new-lander"
        || return_to.starts_with("/new-lander/")
        || return_to.starts_with("/new-lander?")
    {
        let body = page.without_toggle().render(headers).into_string();
        (status, Html(document(body, "auth"))).into_response()
    } else {
        page.respond(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_keeps_native_identity_and_does_not_start_marketing() {
        let input = "<html lang=\"en\"><head></head><body><iframe id=\"engine\"></iframe><input name=\"csrf\" type=\"hidden\" value=\"fixture\"><canvas id=\"face-canvas\"></canvas></body></html>";
        let page = document(input.into(), "studio");
        assert!(page.contains("data-verse-ui=\"studio\""));
        for retained in ["id=\"engine\"", "name=\"csrf\"", "id=\"face-canvas\""] {
            assert!(page.contains(retained));
        }
        assert_eq!(page.matches("src=\"/new-lander/ui.js\"").count(), 1);
        let marketing = document(
            "<html lang=\"en\"><head></head><body></body></html>".into(),
            "marketing",
        );
        assert!(marketing.contains("/new-lander/ui.css"));
        assert!(!marketing.contains("<script"));
    }
    #[tokio::test]
    async fn verse_login_keeps_authentication_fields_and_status_without_loading_the_engine() {
        use axum::{
            body::to_bytes,
            http::{HeaderMap, StatusCode},
        };
        use oa_auth::workos::Method;
        let response = crate::workos_auth::render_login(
            &HeaderMap::new(),
            false,
            "/new-lander/house",
            Some("Try again"),
            Some("test@example.com"),
            &[Method::Password],
            "test-csrf",
            StatusCode::UNAUTHORIZED,
        );
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let text = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(text.contains("data-verse-ui=\"auth\""));
        assert!(text.contains("name=\"csrf\" value=\"test-csrf\""));
        assert!(text.contains("name=\"return_to\" value=\"/new-lander/house\""));
        assert!(text.contains("/new-lander/ui.css"));
        assert!(!text.contains("/new-lander/ui.js"));
        assert!(!text.contains("<iframe"));
        assert!(!text.contains("oa-theme-toggle-form"));
    }
}
