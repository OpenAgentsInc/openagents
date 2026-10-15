//! `/cad`: the CAD engine in the browser (#11649, `docs/cad-lab/README.md`,
//! "In the browser").
//!
//! A sample plate with holes, rebuilt, checked and drawn on the visitor's
//! own device by `crates/cad-wasm` each time a slider moves: no request
//! reaches this server after the page and the engine's files. The engine
//! runs in a worker; the plate's numbers stay in the browser's storage.
//!
//! The build (`scripts/build-cad-web.sh`) is read from the directory the
//! server was started with (`--cad DIR`, or `/srv/cad` when the image has
//! it):
//!
//! ```text
//! DIR/cad_wasm.js         wasm-bindgen's `--target web` glue
//! DIR/cad_wasm_bg.wasm    the engine, size-optimized
//! DIR/worker.js           runs the engine off the page's thread
//! DIR/start.js            the page's script
//! DIR/cad.css             the page's styles
//! DIR/*.br, DIR/*.gz      brotli and gzip copies, sent when asked for
//! ```
//!
//! Only this page loads the engine. Without the build the page says so and
//! runs no script.

use std::path::Path;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::actions::Button;
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::Slider;

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// The page.
pub(crate) const CAD_PATH: &str = "/cad";

/// The build's files, as `scripts/build-cad-web.sh` names them.
pub(crate) const CAD_FILES: [&str; 5] = [
    "cad_wasm.js",
    "cad_wasm_bg.wasm",
    "worker.js",
    "start.js",
    "cad.css",
];

/// The page's policy: the site's, plus its own scripts compiling
/// WebAssembly and starting a worker from this site.
pub(crate) const CAD_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

const TITLE: &str = "CAD in your browser";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(CAD_PATH, get(page))
        .route("/cad/{file}", get(build_file))
}

/// The build directory, when it holds every file.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.cad.as_deref()?;
    CAD_FILES
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The plate's sliders: (id, label, min, max, step, starting value).
const SLIDERS: [(&str, &str, f64, f64, f64, f64); 5] = [
    ("length", "Length", 40.0, 160.0, 1.0, 80.0),
    ("width", "Width", 30.0, 100.0, 1.0, 50.0),
    ("thickness", "Thickness", 3.0, 20.0, 0.5, 6.0),
    ("radius", "Corner radius", 0.0, 14.0, 0.5, 5.0),
    ("hole", "Hole diameter", 2.0, 12.0, 0.2, 6.0),
];

/// The downloads: (format, label).
const FILES: [(&str, &str); 4] = [
    ("step", "STEP"),
    ("stl", "STL"),
    ("pdf", "Drawing (PDF)"),
    ("dxf", "Drawing (DXF)"),
];

fn content() -> Markup {
    html! {
        section.cad-page aria-labelledby="cad-title" {
            (MarkdownRoot::new(html! {
                h1 id="cad-title" { (TITLE) }
                p.oa-page-lead {
                    "Move a slider and we rebuild this plate, check it and draw it right here on your device. Nothing you change is sent to us."
                }
            }))
            div.cad-demo {
                form.cad-controls id="cad-form" aria-label="The plate" {
                    @for (id, label, min, max, step, value) in SLIDERS {
                        (Slider::new(id)
                            .id(format!("cad-{id}"))
                            .label(label)
                            .unit("mm")
                            .range(min, max)
                            .step(step)
                            .value(value)
                            .disabled(true))
                    }
                    div.cad-files {
                        @for (format, label) in FILES {
                            (Button::new(label)
                                .attr("data-cad-file", format)
                                .disabled(true))
                        }
                        (Button::new("Start over").id("cad-reset").disabled(true))
                    }
                }
                div.cad-result {
                    div.cad-sheet id="cad-drawing" role="img" aria-label="The plate's drawing: front, top and isometric views" {}
                    p.cad-status id="cad-status" aria-live="polite" {
                        "Loading the CAD engine\u{2026}"
                    }
                    noscript { p.cad-status { "Turn on JavaScript to change the plate." } }
                    dl.cad-numbers id="cad-numbers" hidden {
                        div { dt { "Mass" } dd id="cad-mass" {} }
                        div { dt { "Volume" } dd id="cad-volume" {} }
                        div { dt { "Size" } dd id="cad-size" {} }
                        div { dt { "Rebuilt in" } dd id="cad-time" {} }
                    }
                }
            }
            p.cad-note {
                "This is the engine behind CAD Lab, where you design parts and assemblies by talking: "
                "every change is rebuilt from its history, checked for closed solids, measured, and drawn "
                "with hidden lines dashed. The plate is aluminum; the files are yours to keep."
            }
        }
    }
}

/// The page without the build: it runs no script.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="cad-title" {
            (MarkdownRoot::new(html! {
                h1 id="cad-title" { (TITLE) }
                p.oa-page-lead { "The CAD engine can't run here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(CAD_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    if build(&app).is_none() {
        return unavailable(&headers);
    }
    let head = html! {
        link rel="stylesheet" href="/cad/cad.css";
        script type="module" src="/cad/start.js" {}
    };
    let mut response = UiPage::new(TITLE)
        .path(CAD_PATH)
        .description("Change a part and watch it rebuild, check and draw itself in your browser.")
        .head(head)
        .content(PageColumn::new(content()).wide())
        .respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CAD_POLICY),
    );
    response
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let Some(directory) = build(&app) else {
        return crate::not_found().await;
    };
    let content_type = if file == "cad.css" {
        Some("text/css; charset=utf-8")
    } else {
        super::everglade::build_type(&file)
    };
    let Some(content_type) = content_type else {
        return crate::not_found().await;
    };
    let mut response =
        super::everglade::serve_build(directory, &file, content_type, &request).await;
    // A worker runs under its own script's policy: the page's, so it may
    // fetch and compile the engine.
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CAD_POLICY),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_starts_waiting_for_the_engine() {
        let html = content().into_string();
        let lower = html.to_ascii_lowercase();
        assert!(!lower.contains("<script"));
        assert!(!lower.contains(" style="));
        for (id, ..) in SLIDERS {
            assert!(html.contains(&format!("id=\"cad-{id}\"")), "{id}");
        }
        for (format, _) in FILES {
            assert!(
                html.contains(&format!("data-cad-file=\"{format}\"")),
                "{format}"
            );
        }
        // Nothing can be pressed before the engine is there to answer it.
        assert_eq!(
            lower.matches(" disabled").count(),
            SLIDERS.len() + FILES.len() + 1,
            "{html}"
        );
        assert!(html.contains("Loading the CAD engine"));
        assert!(CAD_POLICY.contains("script-src 'self' 'wasm-unsafe-eval'"));
        assert!(CAD_POLICY.contains("worker-src 'self'"));
    }
}
