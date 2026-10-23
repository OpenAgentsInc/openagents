//! Haunted House review shell for the pinned, separately hosted private engine.

use super::medieval_town::{Release, response as engine_response};
fn response(
    status: StatusCode,
    body: impl Into<String>,
    kind: &'static str,
    origin: Option<&str>,
) -> Response {
    let mut result = engine_response(status, body.into(), kind, origin);
    // Voice input belongs to this host. The cross-origin engine receives no mic access.
    result.headers_mut().insert(
        "permissions-policy",
        axum::http::HeaderValue::from_static(
            "camera=(), microphone=(self), geolocation=(), payment=()",
        ),
    );
    if let Some(value) = result
        .headers()
        .get(axum::http::header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
    {
        let value = value.replace(
            "connect-src 'self'",
            "img-src data: blob:; connect-src 'self' wss://relay.openagents.com",
        );
        if let Ok(value) = value.parse() {
            result
                .headers_mut()
                .insert(axum::http::header::CONTENT_SECURITY_POLICY, value);
        }
    }
    result
}
use crate::App;
use axum::{Router, http::StatusCode, response::Response, routing::get};
use maud::{DOCTYPE, PreEscaped, html};

pub(crate) const HOUSE: &str = "/new-lander/house";
const MAP: &str = "/new-lander/verse?world=haunted-house";

fn configured(text: &str) -> Option<Release> {
    let r = Release::parse(text)?;
    (r.project.id == "haunted-house" && r.project.branch == "main").then_some(r)
}
fn release() -> Option<Release> {
    let text = std::env::var("OPENAGENTS_HAUNT_RELEASE")
        .unwrap_or_else(|_| include_str!("../../content/demos/haunted-house.json").into());
    configured(&text)
}

pub(crate) fn engine_release() -> Option<String> {
    release().map(|r| r.release)
}

pub(crate) fn available() -> bool {
    release().is_some()
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(HOUSE, get(page))
        .route("/new-lander/house/boot.js", get(boot))
        .route("/new-lander/house/style.css", get(style))
        .route(
            "/new-lander/house/{*file}",
            get(|| async { StatusCode::NOT_FOUND }),
        )
}

async fn page() -> Response {
    let Some(r) = release() else {
        return response(StatusCode::SERVICE_UNAVAILABLE, html! {
            (DOCTYPE) html lang="en" { head { meta name="viewport" content="width=device-width,initial-scale=1"; title { "Haunted House · OpenAgents" } }
                body { h1 { "The house isn't open here yet." } p { "You can return to the planets and keep exploring." }
                    a href=(HOUSE) { "Try again" } " · " a href=(MAP) { "Back to planets" }
                }
            }
        }.into_string(), "text/html; charset=utf-8", None);
    };
    let config = serde_json::json!({"release":r.release,"engine_abi":r.engine_abi,"project_format":r.project_format,"project":r.project});
    let text = serde_json::to_string(&config)
        .expect("configuration")
        .replace('<', "\\u003c");
    // Only public catalog metadata enters the host. The engine independently checks availability.
    let assets: Vec<aethyrforge_host::CreatorAsset> = aethyrforge_host::FantasyCatalog::decode(
        include_bytes!("../../../../docs/verse/fantasy-props-catalog.json"),
    )
    .map(|c| {
        c.runtime
            .into_iter()
            .filter(|a| !a.id.starts_with("medieval-town:"))
            .map(|a| aethyrforge_host::CreatorAsset {
                id: a.id,
                name: a.name.replace('_', " "),
            })
            .collect()
    })
    .unwrap_or_default();
    let assets_json = serde_json::to_string(&assets)
        .expect("asset metadata")
        .replace('<', "\\u003c");
    let body = html! {
        (DOCTYPE) html lang="en" { head {
            meta id="engine-release" content=(r.release);
            meta charset="utf-8"; meta name="viewport" content="width=device-width,initial-scale=1";
            title { "Haunted House · OpenAgents" }
            link rel="stylesheet" href="/new-lander/house/style.css";
        } body {
            main {
                iframe id="engine" title="Haunted House" data-engine-url=(r.engine_url) {}
                p id="world-notice" role="status" aria-live="polite" { "Opening the house…" }
            }
            nav id="world-controls" aria-label="World controls" {
                a id="world-map" href=(MAP) { "Back to planets" }
                p id="world-touch-help" { "Left side: move · Right side: look" }
                span { "Click to look · WASD to walk · Escape to release the pointer" }
                a id="world-retry" href=(HOUSE) { "Reload world" }
                button id="world-mesh" type="button" hidden aria-pressed="false" { "Mesh overlay" }
                button id="world-edit" type="button" { "Edit" }
                button id="world-fullscreen" type="button" { "Full screen" }
            }
            aside id="world-voice" hidden aria-label="Voice controls" {
                button id="edit-voice-start" type="button" hidden { "Listen" }
                button id="edit-voice-stop" type="button" { "Stop / mute" }
                p id="edit-voice-status" role="status" aria-live="polite" { "Microphone off" }
                p id="voice-heard" {}
                p id="voice-response" role="status" aria-live="polite" {}
                p class="voice-help" { "Browser speech service · say Show options, Review change, or Confirm change" }
            }
                        aside id="request-choices" hidden aria-label="Request options" {
                            @for (id,label) in [("request-inspect","Inspect world"),("request-mesh","Show mesh"),("request-map","Back to planets"),("request-compile","Compile world effect"),("request-undo","Undo")] {
                                button id=(id) type="button" { (label) }
                            }
                        }
            div id="edit-shadow" hidden aria-hidden="true" {}
            aside id="edit-panel" hidden role="dialog" aria-modal="true" aria-labelledby="edit-title" data-active="objects" {
                header class="workspace-header" {
                    div { p class="eyebrow" { "OPENAGENTS / STUDIO" } h1 id="edit-title" { "Jarvis" } }
                    button id="edit-close" type="button" { "Inspect world" }
                }
                div id="jarvis-toolbar" hidden role="tablist" aria-label="Editing tools" {
                    @for (id,label) in [("objects","Object"),("materials","Materials"),("request","Request"),("compile","Compile"),("history","History")] {
                        button id=(format!("tab-{id}")) type="button" role="tab" aria-controls=(format!("panel-{id}")) aria-selected="false" { (label) }
                    }
                }
                section id="panel-objects" class="tool-window" aria-labelledby="objects-title" {
                    header { h2 id="objects-title" { "01 / Object" } button id="close-objects" class="window-close" type="button" aria-label="Close Object" { "×" } }
                    p id="edit-target" { "Click an object in the house." }
                    button id="edit-pick" type="button" { "Select in world" }
                    div id="prop-tools" hidden {
                    label for="edit-place-asset" { "Prop to add at X, Y, Z below" }
                    input id="edit-place-asset" type="text" list="edit-props" maxlength="128";
                    datalist id="edit-props" {@for asset in &assets {option value=(&asset.id) {(&asset.name)}}}
                    button id="edit-place" type="button" { "Add prop" }
                    }
                    form id="edit-form" {
                        div class="position-fields" {
                            @for (id,label) in [("edit-x","X"),("edit-y","Y"),("edit-z","Z")] { label { (label) input id=(id) type="number" step="any" required value="0"; } }
                        }
                        label for="edit-note" { "Notes" } textarea id="edit-note" maxlength="48000" rows="3" {}
                        input id="edit-submit" type="submit" value="Move object" disabled;
                    }
                }
                div id="author-tools" hidden {
                    section id="panel-materials" class="tool-window" aria-labelledby="materials-title" {
                        header { h2 id="materials-title" { "02 / Materials" } button id="close-materials" class="window-close" type="button" aria-label="Close Materials" { "×" } }
                        label for="edit-material" { "Shared material ID (all surfaces using this ID)" } input id="edit-material" type="text" maxlength="128";
                        p id="edit-material-ids" {}
                        label for="edit-parameter" { "Parameter: red, green, blue, roughness, metallic" } input id="edit-parameter" type="text" value="roughness";
                        label for="edit-value" { "Value" } input id="edit-value" type="number" step="any" value="0.8";
                        button id="edit-material-apply" type="button" { "Apply material" }
                        label for="edit-fog" { "Fog density (0–0.06)" } input id="edit-fog" type="number" step="0.001" min="0" max="0.06" value="0.01";
                        button id="edit-fog-apply" type="button" { "Apply fog" }
                    }
                    section id="panel-request" class="tool-window" aria-labelledby="request-title" {
                        header { h2 id="request-title" { "03 / Kiln" } button id="close-request" class="window-close" type="button" aria-label="Close Request" { "×" } }
                        label for="edit-kiln-request" { "Request" } textarea id="edit-kiln-request" maxlength="4800" rows="5" {}
                        label class="check-label" { input id="edit-kiln-world" type="checkbox"; "Allow a world effect (all materials)" }
                        button id="edit-request-review" type="button" { "Review request" }
                        button id="edit-kiln-ask" type="button" hidden { "Ask Kiln" }
                        button id="request-options" type="button" { "Show options" }
                        button id="edit-kiln-stop" type="button" { "Stop" }
                        button id="edit-kiln-review" type="button" { "Review Kiln's change" }
                        p id="edit-kiln-status" role="status" {}
                    }
                    section id="panel-compile" class="tool-window" aria-labelledby="compile-title" {
                        header { h2 id="compile-title" { "04 / World effect" } button id="close-compile" class="window-close" type="button" aria-label="Close Compile" { "×" } }
                        p { "Applies to every material in the house." }
                        label for="edit-source" { "WGSL source" } textarea id="edit-source" maxlength="4096" rows="8" spellcheck="false" {}
                        button id="edit-source-draft" type="button" { "Keep world-effect draft" }
                        button id="edit-source-compile" type="button" { "Compile world effect" }
                        p id="edit-source-status" role="status" {}
                        button id="edit-capture" type="button" { "Capture preview" }
                        img id="edit-capture-image" hidden alt="Captured view of the house" width="280";
                    }
                    section id="panel-history" class="tool-window" aria-labelledby="history-title" {
                        header { h2 id="history-title" { "05 / This visit" } button id="close-history" class="window-close" type="button" aria-label="Close History" { "×" } }
                        button id="edit-undo" type="button" { "Undo" } button id="edit-redo" type="button" { "Redo" }
                        ol id="edit-history-entries" {}
                        p { "Earnings: " strong { "Not recorded" } }
                    }
                }
                p id="edit-status" role="status" aria-live="polite" {}
            }
            dialog id="edit-auth" hidden {
                section role="dialog" aria-modal="true" aria-labelledby="edit-auth-title" {
                    h2 id="edit-auth-title" { "Sign in to make this change" }
                    p id="edit-auth-description" { "Your view and requested change will stay here." }
                    ul id="edit-auth-changes" {}
                    a id="edit-signin" href="/login?return_to=%2Fnew-lander%2Fhouse" { "Sign in" }
                    button id="edit-confirm" type="button" hidden { "Confirm change" }
                    button id="edit-auth-voice-stop" type="button" { "Stop / mute" }
                    button id="edit-cancel" type="button" { "Cancel" }
                }
            }
            script id="creator-assets" type="application/json" {(PreEscaped(assets_json))}
            script id="engine-config" type="application/json" { (PreEscaped(text)) }
            script type="module" src="/new-lander/house/boot.js" {}
        } }
    }
    .into_string();
    response(
        StatusCode::OK,
        body,
        "text/html; charset=utf-8",
        Some(&r.origin()),
    )
}

async fn boot() -> Response {
    let Some(r) = release() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "",
            "text/javascript; charset=utf-8",
            None,
        );
    };
    let url = serde_json::to_string(&r.sdk_url).expect("SDK URL");
    let kiln_url = serde_json::to_string(&r.sdk_url.replace("aethyrforge_host.js", "kiln.js"))
        .expect("Kiln URL");
    response(
        StatusCode::OK,
        format!(
            "window.loadVerseKiln=async context=>{{const k=await import({kiln_url});await k.default();await k.start_verse('/kiln/',context);}};try{{const {{default:init,mount_studio}}=await import({url});await init();window.hauntDemo=mount_studio();}}catch{{const n=document.getElementById('world-notice');n.hidden=false;n.dataset.state='error';n.textContent='The house could not open. Reload to try again.';}}"
        ),
        "text/javascript; charset=utf-8",
        Some(&r.origin()),
    )
}

async fn style() -> Response {
    response(
        StatusCode::OK,
        include_str!("../../content/new-lander-house.css"),
        "text/css; charset=utf-8",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn new_lander_house_never_serves_legacy_engine_or_source_files() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        for file in [
            "verse/index.html",
            "material.wgsl",
            "package.json",
            "src/main.ts",
            "assets/old.js",
            "kiln/pkg/kiln.js",
        ] {
            let uri = format!("{HOUSE}/{file}");
            let result = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&uri)
                        .header(header::HOST, "127.0.0.1:4300")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(result.status(), StatusCode::NOT_FOUND);
            assert!(crate::upstream::owned(&uri));
            assert!(to_bytes(result.into_body(), 4096).await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn new_lander_house_gates_creator_tools_and_names_effect_scope() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        let response = app
            .oneshot(
                Request::builder()
                    .uri(HOUSE)
                    .header(header::HOST, "127.0.0.1:4300")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(csp.contains("wss://relay.openagents.com"));
        assert_eq!(
            response.headers()["permissions-policy"],
            "camera=(), microphone=(self), geolocation=(), payment=()"
        );
        let read_only =
            super::super::medieval_town::response(StatusCode::OK, "", "text/html", None);
        assert_eq!(
            read_only.headers()["permissions-policy"],
            "camera=(), microphone=(), geolocation=(), payment=()"
        );
        let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("id=\"author-tools\" hidden"));
        assert!(text.contains("World effect"));
        for panel in ["objects", "materials", "request", "compile", "history"] {
            assert!(text.contains(&format!("id=\"panel-{panel}\"")));
            assert!(text.contains(&format!("id=\"tab-{panel}\"")));
        }
        assert!(text.contains("Earnings: "));
        assert!(text.contains("Not recorded"));
        assert!(text.contains("Applies to every material in the house."));
        assert!(text.contains("Allow a world effect (all materials)"));
        assert!(!text.contains("id=\"edit-kiln-world\" type=\"checkbox\" checked"));
    }

    #[test]
    fn new_lander_house_refuses_another_world_or_branch() {
        let base = serde_json::json!({"engine_url":"https://engine.example/","sdk_url":"https://engine.example/sdk/hash/aethyrforge_host.js","release":"house-1","engine_abi":"abi-1","project_format":1,"project":{"id":"haunted-house","branch":"main","revision":0}});
        assert!(configured(&base.to_string()).is_some());
        for (field, value) in [("id", "medieval-town-lab"), ("branch", "someone-else")] {
            let mut bad = base.clone();
            bad["project"][field] = value.into();
            assert!(configured(&bad.to_string()).is_none());
        }
    }
}
