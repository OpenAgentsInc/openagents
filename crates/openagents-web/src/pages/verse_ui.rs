//! Shared Verse presentation. Rust controllers retain account and project authority.
use crate::App;
use axum::{Router, http::header, routing::get};

const CSS: &str = include_str!("../../content/verse-ui/ui.css");
const JS: &str = include_str!("../../content/verse-ui/ui.js");
const STYLE: &str = "<link rel=\"stylesheet\" href=\"/new-lander/ui.css\">";
const SCRIPT: &str = "<script type=\"module\" src=\"/new-lander/ui.js\"></script>";

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
        "marketing" | "universe" | "studio" | "player" | "setup" | "invite"
    ));
    let mut body = body.replacen("<html ", &format!("<html data-verse-ui=\"{surface}\" "), 1);
    for old in [
        "/new-lander/house/style.css",
        "/new-lander/play/style.css",
        "/new-lander/planet/style.css",
    ] {
        body = body.replace(&format!("<link rel=\"stylesheet\" href=\"{old}\">"), "");
    }
    let assets = if surface == "marketing" {
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
}
