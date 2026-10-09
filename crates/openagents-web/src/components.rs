//! Public, synthetic examples of the shared Coder presentation library.

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use coder_ui::catalog::{self, FixtureState};
use rust_native::view::{Element, Node};
use serde::Deserialize;

use crate::App;
use crate::layout::{escape, segment};

const POLICY: &str = "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
const GLUE: &str = "coder_components_web.js";
const WASM: &str = "coder_components_web_bg.wasm";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/components", get(index))
        .route("/components/{component}", get(component))
        .route("/components/manifest.json", get(manifest))
        .route("/components/assets/{file}", get(asset))
}

#[derive(Default, Deserialize)]
struct Options {
    variant: Option<String>,
    width: Option<u16>,
    height: Option<u16>,
    phase: Option<u64>,
    elapsed: Option<u64>,
    scroll: Option<usize>,
    fullscreen: Option<bool>,
}

async fn index(State(app): State<App>, Query(options): Query<Options>) -> Response {
    render(&app, "screen.main", options)
}

async fn component(
    State(app): State<App>,
    Path(component): Path<String>,
    Query(options): Query<Options>,
) -> Response {
    render(&app, &component, options)
}

fn render(app: &App, component: &str, options: Options) -> Response {
    let entries = catalog::entries();
    let Some(entry) = entries.iter().find(|entry| entry.id == component) else {
        return crate::layout::problem(
            StatusCode::NOT_FOUND,
            "Component not found",
            "Choose a registered component from the catalog.",
            ("/components", "Components"),
        );
    };
    let requested = options.variant.as_deref();
    let Some(variant) = requested
        .and_then(|id| entry.variants.iter().find(|variant| variant.id == id))
        .or_else(|| {
            requested
                .is_none()
                .then(|| entry.variants.first())
                .flatten()
        })
    else {
        return (StatusCode::BAD_REQUEST, "Unknown component variant").into_response();
    };
    let mut state = FixtureState::default_for(component, &variant.id);
    if let Some(width) = options.width {
        if !(24..=160).contains(&width) {
            return (StatusCode::BAD_REQUEST, "Width must be 24 to 160 columns").into_response();
        }
        state.width = width;
    }
    if let Some(height) = options.height {
        if !(12..=80).contains(&height) {
            return (StatusCode::BAD_REQUEST, "Height must be 12 to 80 rows").into_response();
        }
        state.height = height;
    }
    if let Some(phase) = options.phase {
        state.phase = (phase % 8) as u8;
    }
    if let Some(elapsed) = options.elapsed {
        state.elapsed = elapsed.min(86_400);
    }
    if let Some(scroll) = options.scroll {
        if scroll > 500_000 {
            return (StatusCode::BAD_REQUEST, "Scroll exceeds the fixture limit").into_response();
        }
        state.scroll = scroll;
    }
    let view = catalog::view(component, &variant.id, &state);
    let mut controls = Vec::new();
    inspect(&view.root, &mut controls);
    let preview = match view
        .validate()
        .map_err(|error| error.to_string())
        .and_then(|validated| {
            rust_native_web::render(&validated).map_err(|error| error.to_string())
        }) {
        Ok(html) => html,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Component fixture failed validation: {error}"),
            )
                .into_response();
        }
    };
    let interactive = app
        .config
        .components_build
        .as_ref()
        .is_some_and(|dir| dir.join(GLUE).is_file() && dir.join(WASM).is_file());
    let initial = escape(&serde_json::to_string(&state).expect("Fixture state serializes"));
    let title = escape(&entry.title);
    let fullscreen = options.fullscreen.unwrap_or(false);
    let sources = entry
        .sources
        .iter()
        .map(|source| {
            format!(
                "<li><a href=\"https://github.com/OpenAgentsInc/openagents/blob/{revision}/{path}\" rel=\"noopener\">{path} · {symbol}</a><span> — {branch}</span></li>",
                revision = catalog::SOURCE_REVISION,
                path = escape(&source.path),
                symbol = escape(&source.symbol),
                branch = escape(&source.branch),
            )
        })
        .collect::<String>();
    let variant_options = entry
        .variants
        .iter()
        .map(|item| {
            format!(
                "<option value=\"{}\"{}>{}</option>",
                escape(&item.id),
                if item.id == variant.id {
                    " selected"
                } else {
                    ""
                },
                escape(&item.label),
            )
        })
        .collect::<String>();
    let mut nav = String::new();
    let mut family = String::new();
    for item in &entries {
        if item.family != family {
            if !family.is_empty() {
                nav.push_str("</section>");
            }
            family.clone_from(&item.family);
            nav.push_str(&format!(
                "<section><h2 class=\"catalog-group\">{}</h2>",
                escape(&family),
            ));
        }
        nav.push_str(&format!(
            "<a class=\"catalog-entry\" data-catalog-entry data-search=\"{}\" href=\"/components/{}\"{}>{}<small>{}</small></a>",
            escape(&format!("{} {} {}", item.title, item.family, item.id).to_lowercase()),
            segment(&item.id),
            if item.id == component { " aria-current=\"page\"" } else { "" },
            escape(&item.title),
            item.variants.len(),
        ));
    }
    if !family.is_empty() {
        nav.push_str("</section>");
    }
    let total_variants: usize = entries.iter().map(|entry| entry.variants.len()).sum();
    let metadata = escape(&serde_json::to_string_pretty(&serde_json::json!({
        "component": entry,
        "properties": {"columns": state.width, "rows": state.height, "phase": state.phase, "elapsed_seconds": state.elapsed, "earlier_rows":state.scroll},
        "view_schema": "rust-native.view.v3",
        "event_identity": ["instance", "revision", "node"],
        "controls_and_resolved_styles": controls,
    })).expect("Catalog entry serializes"));
    let status = if interactive {
        "Loading Rust interaction…"
    } else {
        "HTML preview · Build the browser module to enable local interaction"
    };
    let preview_html = format!(
        "<div id=\"catalog-preview\" class=\"catalog-preview coder-profile\" style=\"width:{}px;min-height:{}px\" data-columns=\"{}\" data-rows=\"{}\">{preview}</div>",
        state.width * 9,
        state.height * 20,
        state.width,
        state.height,
    );
    let body = if fullscreen {
        format!("<main class=\"catalog-preview-page\">{preview_html}</main>")
    } else {
        format!(
            "<header class=\"catalog-header\"><div class=\"catalog-brand\"><a href=\"/\">OpenAgents</a><span>/ components</span></div><nav aria-label=\"Catalog links\"><a href=\"/components/manifest.json\">Manifest</a><a href=\"/docs\">Docs</a><a href=\"https://github.com/OpenAgentsInc/openagents\" rel=\"noopener\">Source ↗</a></nav></header>\
<div class=\"catalog-layout\"><aside class=\"catalog-sidebar\" aria-label=\"Components\"><label class=\"catalog-search\"><span>Find a component</span><input id=\"catalog-search\" type=\"search\" placeholder=\"Search components…\" autocomplete=\"off\"></label><p class=\"catalog-count\">{count} components · {total_variants} variants</p><nav class=\"catalog-nav\" id=\"catalog-nav\">{nav}</nav></aside>\
<main class=\"catalog-main\"><div class=\"catalog-titlebar\"><h1>{title}</h1><a id=\"catalog-fullscreen\" class=\"catalog-fullscreen-link\" href=\"/components/{id}?variant={variant}&amp;fullscreen=true\">Open full screen ↗</a></div><p class=\"catalog-description\">{description}</p><p class=\"catalog-source\">Rust Native / Coder · public source at <code>{revision}</code> · synthetic fixtures</p>\
<form id=\"catalog-controls\" class=\"catalog-controls\" method=\"get\"><label>Variant<select id=\"catalog-variant\" class=\"catalog-variant\" name=\"variant\">{variant_options}</select></label><label>Columns<input id=\"catalog-width\" name=\"width\" type=\"number\" min=\"24\" max=\"160\" value=\"{width}\"></label><label>Rows<input id=\"catalog-height\" name=\"height\" type=\"number\" min=\"12\" max=\"80\" value=\"{height}\"></label><label>Phase<input id=\"catalog-phase\" name=\"phase\" type=\"number\" min=\"0\" max=\"7\" value=\"{phase}\"></label><label>Elapsed seconds<input id=\"catalog-elapsed\" name=\"elapsed\" type=\"number\" min=\"0\" max=\"86400\" value=\"{elapsed}\"></label><label>Earlier rows<input id=\"catalog-scroll\" name=\"scroll\" type=\"number\" min=\"0\" max=\"500000\" value=\"{scroll}\"></label><button type=\"submit\">Apply preview</button><button type=\"button\" id=\"catalog-tick\">Next frame</button><button type=\"button\" id=\"catalog-reset\">Reset</button></form>\
<section class=\"catalog-preview-frame\" aria-label=\"Component preview\"><div class=\"catalog-preview-caption\"><span>CODER TERMINAL PROFILE</span><span id=\"catalog-dimensions\">{width} × {height} cells · 9 × 20 px</span></div>{preview_html}</section><div class=\"catalog-footer\"><span id=\"catalog-status\" role=\"status\">{status}</span><span>Demo controls affect this preview only</span></div>\
<div class=\"catalog-inspectors\"><details><summary>Properties, intents, and source inventory</summary><pre id=\"catalog-properties\">{metadata}</pre><ul class=\"catalog-source-list\">{sources}</ul></details><details open><summary>Local interaction events</summary><pre id=\"catalog-events\">No interaction yet.</pre></details></div><p class=\"catalog-limit\">Every example uses shared Rust components. Sending, testing, saving, resuming, and approving here simulate fixture state.</p></main></div>",
            count = entries.len(),
            id = segment(component),
            variant = segment(&variant.id),
            description = escape(&entry.description),
            width = state.width,
            height = state.height,
            revision = catalog::SOURCE_REVISION,
            phase = state.phase,
            elapsed = state.elapsed,
            scroll = state.scroll,
        )
    };
    let script = if interactive {
        "<script type=\"module\" src=\"/components/assets/start.js\"></script>"
    } else {
        ""
    };
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>{title} · Components · OpenAgents</title><link rel=\"icon\" href=\"/favicon.svg\"><link rel=\"stylesheet\" href=\"/components/assets/components.css\"><link rel=\"stylesheet\" href=\"/components/assets/native.css\"></head><body>{body}<pre id=\"catalog-initial\" hidden>{initial}</pre>{script}</body></html>"
    );
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    response
}

async fn manifest() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "schema": "openagents.coder.component-catalog.v1",
        "source_revision": catalog::SOURCE_REVISION,
        "retained_transcript_fixture": catalog::transcript_fixture_stats(),
        "entries": catalog::entries(),
    }))
}

async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let (content_type, body): (&str, Vec<u8>) = match file.as_str() {
        "components.css" => (
            "text/css; charset=utf-8",
            crate::with_fonts(&crate::palette::stylesheet(include_str!(
                "../static/components.css"
            )))
            .into_bytes(),
        ),
        "native.css" => (
            "text/css; charset=utf-8",
            format!(
                "{}\n{}",
                rust_native_web::CSS,
                coder_ui::source_theme::WEB_PROFILE_CSS
            )
            .into_bytes(),
        ),
        "start.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/components-start.js").to_vec(),
        ),
        GLUE | WASM => {
            let Some(directory) = &app.config.components_build else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let Ok(body) = tokio::fs::read(directory.join(&file)).await else {
                return StatusCode::NOT_FOUND.into_response();
            };
            (
                if file == WASM {
                    "application/wasm"
                } else {
                    "text/javascript; charset=utf-8"
                },
                body,
            )
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

fn inspect(node: &Node<catalog::CatalogIntent>, out: &mut Vec<serde_json::Value>) {
    let control = match &node.element {
        Element::Button {
            label,
            enabled,
            intent,
            ..
        }
        | Element::Choice {
            label,
            enabled,
            intent,
            ..
        } => Some(serde_json::json!({"label":label,"enabled":enabled,"intent":intent})),
        Element::Field {
            label,
            secret,
            max_bytes,
            enabled,
            on_change,
            ..
        } => Some(
            serde_json::json!({"label":label,"secret":secret,"max_bytes":max_bytes,"enabled":enabled,"on_change":on_change}),
        ),
        Element::Dialog {
            label, on_close, ..
        } => Some(serde_json::json!({"label":label,"on_close":on_close})),
        _ => None,
    };
    out.push(serde_json::json!({"node":node.key,"style":node.style,"control":control}));
    match &node.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Choice { children, .. }
        | Element::Dialog { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. } => {
            for child in children {
                inspect(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
