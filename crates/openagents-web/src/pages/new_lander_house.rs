//! Haunted House review shell for the pinned, separately hosted private engine.

use super::medieval_town::{Release, response};
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
            nav aria-label="World controls" {
                a id="world-map" href=(MAP) { "Back to planets" }
                span { "Click to look · WASD to walk · Escape to release the pointer" }
                a id="world-retry" href=(HOUSE) { "Reload world" }
                button id="world-edit" type="button" { "Edit" }
                button id="world-fullscreen" type="button" { "Full screen" }
            }
            aside id="edit-panel" hidden aria-labelledby="edit-title" {
                header { h2 id="edit-title" { "Edit" } button id="edit-close" type="button" aria-label="Close Edit" { "×" } }
                p id="edit-target" { "Click an object in the house." }
                form id="edit-form" {
                    div class="position-fields" {
                        @for (id, label) in [("edit-x", "X"), ("edit-y", "Y"), ("edit-z", "Z")] {
                            label { (label) input id=(id) type="number" step="0.1" required value="0"; }
                        }
                    }
                    label for="edit-note" { "Notes" } textarea id="edit-note" maxlength="48000" {}
                    input id="edit-submit" type="submit" value="Move object" disabled;
                }
                p id="edit-status" role="status" aria-live="polite" {}
            }
            dialog id="edit-auth" hidden {
                section role="dialog" aria-modal="true" aria-labelledby="edit-auth-title" {
                    h2 id="edit-auth-title" { "Sign in to make this change" }
                    p id="edit-auth-description" { "Your view and requested change will stay here." }
                    a id="edit-signin" href="/login?return_to=%2Fnew-lander%2Fhouse" { "Sign in" }
                    button id="edit-confirm" type="button" hidden { "Confirm change" }
                    button id="edit-cancel" type="button" { "Cancel" }
                }
            }
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
    response(
        StatusCode::OK,
        format!(
            "try{{const {{default:init,mount_studio}}=await import({url});await init();window.hauntDemo=mount_studio();}}catch{{const n=document.getElementById('world-notice');n.hidden=false;n.dataset.state='error';n.textContent='The house could not open. Reload to try again.';}}"
        ),
        "text/javascript; charset=utf-8",
        Some(&r.origin()),
    )
}

async fn style() -> Response {
    response(
        StatusCode::OK,
        "input,textarea{box-sizing:border-box;color:inherit;background:#172027;border:1px solid #727b80;border-radius:4px;padding:8px;width:100%}button,input[type=submit]{cursor:pointer}input:disabled{opacity:.45;cursor:default}#edit-panel{position:fixed;top:24px;right:24px;width:min(320px,80vw);padding:20px;background:#131d23ed;border:1px solid #727b8080;border-radius:10px}#edit-panel header{display:flex;justify-content:space-between;align-items:center}#edit-panel h2{margin:0}#edit-panel label{display:block;margin:12px 0 6px}.position-fields{display:flex;gap:10px}.position-fields label{width:30%}#edit-auth{margin:0;padding:0;border:0;max-width:none;max-height:none;width:100%;height:100%;color:inherit;position:fixed;inset:0;display:grid;place-items:center;background:#0005}#edit-auth section{max-width:360px;padding:32px;background:#131d23eb;border:1px solid #727b8080;border-radius:14px;backdrop-filter:blur(18px)}#edit-auth button,#edit-auth a{margin:8px 6px 0 0;display:inline-block}#edit-auth[hidden],#edit-panel[hidden]{display:none}html,body{height:100%;margin:0;background:#0c1014;color:#f0e9df;font:14px system-ui}main,iframe{position:fixed;inset:0;width:100%;height:100%;border:0}nav{position:fixed;bottom:16px;left:16px;right:16px;display:flex;align-items:center;gap:16px;pointer-events:none}nav>*{pointer-events:auto}nav span{flex:1;text-shadow:0 2px 4px #000}a,button{color:inherit;background:#10171cdd;border:1px solid #727b8080;border-radius:5px;padding:10px 14px;text-decoration:none}a:focus-visible,button:focus-visible{outline:2px solid white;outline-offset:3px}#world-notice{position:fixed;top:40%;left:50%;transform:translateX(-50%);padding:24px;max-width:400px;background:#131d23ee;border:1px solid #727b80}#world-notice[hidden]{display:none}@media(max-width:700px){nav{flex-wrap:wrap;gap:8px}nav span{order:5;flex-basis:100%;font-size:12px}}",
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
