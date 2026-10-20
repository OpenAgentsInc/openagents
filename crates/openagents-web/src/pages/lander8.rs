//! `/lander8`: `/lander7`'s one-response Grid with the OpenAgents mark and
//! name in the top-left corner, and a real chat over it.
//!
//! The chat (`apps/openagents/web/lander-chat`) is one GPUI module drawn by
//! the shared chat views: the composer at the bottom, a short welcome line
//! over it before the first message, and the conversation in a card that
//! grows as it goes. It talks to the chat API the apps use (`/v1/threads`,
//! its event stream, and stop), signed in by the site's cookie through
//! [`crate::cookie_api`]: a signed-in person's chats are their account's, and
//! a signed-out browser chats as an app's signed-out visitor does
//! ([`crate::visitor_api`], the same chat worker as the home page, with its
//! message and chat limits), until it is asked to sign in. The open chat's
//! address is `/lander8/chat/{id}`, so a reload shows it again.
//!
//! The page itself is the same document for everyone, assembled once per
//! build. [`session`] (`GET /lander8/session`) gives the chat its CSRF value
//! and says whether the person is signed in, and gives a browser with no
//! visitor cookie one, as the home page does.
//!
//! The builds (`apps/openagents/scripts/build-web-lander-chat.sh`) are read
//! from the directory the server was started with (`--lander8 DIR`,
//! default `/srv/lander8`):
//!
//! ```text
//! DIR/webgpu/openagents_web_lander_chat.js         glue, WebGPU-only build
//! DIR/webgpu/openagents_web_lander_chat_bg.wasm    module, WebGPU-only build
//! DIR/webgl/...                                    the same with wgpu's WebGL2 fallback
//! DIR/*/*.br, DIR/*/*.gz                           brotli and gzip copies
//! ```
//!
//! and served at `/lander8/build/{webgpu,webgl}/{file}`. The loader is
//! `/lander7`'s: it waits for the background's first full frame, hands the
//! Grid to a worker, fetches the build at low priority and compiles it as it
//! streams, and instantiates and mounts it in separate idle moments. If
//! anything fails, the canvas is removed and a link to the home page's chat
//! shows instead.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use super::lander5::{Addition, BuildKey, Document, assemble_with, build, build_key, send};
use crate::App;
use crate::chat_owner::Who;

/// The page.
pub(crate) const LANDER8_PATH: &str = "/lander8";
/// What the chat reads before it sends anything.
pub(crate) const LANDER8_SESSION: &str = "/lander8/session";

/// The chat's two builds, by the directory each sits in.
pub(crate) const LANDER8_VARIANTS: [&str; 2] = ["webgpu", "webgl"];
/// Each build's files, as `build-web-lander-chat.sh` names them.
pub(crate) const LANDER8_GLUE: &str = "openagents_web_lander_chat.js";
pub(crate) const LANDER8_WASM: &str = "openagents_web_lander_chat_bg.wasm";

/// `/lander7`'s look: the background, the mark and name in the corner, and
/// the chat's canvas over the whole page, hidden until its first frame. The
/// link to the home page's chat shows only when the canvas can't start.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0a0a0a 22%,#111111 36%,#202020 41.6%,#0d0d0d 42.2%,#0a0a0a 60%,#0a0a0a 100%);\
color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
h1{position:fixed;z-index:2;top:calc(18px + env(safe-area-inset-top));left:calc(18px + env(safe-area-inset-left));\
margin:0;display:flex;align-items:center;gap:10px;font-size:15px;font-weight:600;line-height:20px;letter-spacing:0;\
pointer-events:none;text-shadow:0 1px 12px rgba(0,0,0,.5)}\
h1 svg{width:18px;height:18px;flex:none}\
#gpui-mount{position:fixed;top:env(safe-area-inset-top);right:env(safe-area-inset-right);\
bottom:env(safe-area-inset-bottom);left:env(safe-area-inset-left);\
opacity:0;visibility:hidden;transform:translateY(8px);\
transition:opacity .5s cubic-bezier(.22,.8,.3,1),transform .5s cubic-bezier(.22,.8,.3,1),visibility 0s .5s}\
#gpui-mount canvas{position:absolute}\
#gpui-mount.ready{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
#lander8-fallback{position:fixed;left:50%;bottom:calc(32px + env(safe-area-inset-bottom));transform:translateX(-50%);\
padding:13px 26px;border-radius:999px;background:#ededed;color:#0a0a0a;font-size:15px;font-weight:600;text-decoration:none}\
#lander8-fallback[hidden]{display:none}\
@media (prefers-reduced-motion:reduce){#gpui-mount{transform:none;transition:opacity .15s linear,visibility 0s .15s}}";

/// The mark (`icons/openagents-logo.svg`, as the desktop sidebar draws it),
/// the name as the page's heading, the chat's box, and the link shown when
/// the chat can't start.
const MAIN: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>\
<div id=\"gpui-mount\" role=\"region\" aria-label=\"Chat with OpenAgents\"></div>\
<a id=\"lander8-fallback\" href=\"/\" hidden>Chat with OpenAgents</a>";

/// The loader: `/lander7`'s, for this chat's build. After the background's
/// first full frame it hands the Grid to a worker, then in idle moments
/// fetches the build for this browser at low priority, compiling it as it
/// streams, instantiates it, and mounts it. The box fades in on GPUI's first
/// frame (`gpui-first-frame`). GPUI focuses its text field when it starts;
/// on a touch screen focus goes back where it was, so the keyboard doesn't
/// cover the page until the person taps the composer. That happens after
/// GPUI's frame callback returns (the event fires inside it), since moving
/// focus re-enters GPUI. If the chat can't start, its box goes and the link
/// to the home page's chat shows.
const LOADER: &str = "\
const box=document.getElementById(\"gpui-mount\");\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const touch=matchMedia(\"(pointer: coarse)\").matches;\
const fail=()=>{box.remove();document.getElementById(\"lander8-fallback\").hidden=false};\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
let started=false;\
const load=async()=>{try{\
performance.mark(\"card-start\");\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/lander8/build/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
performance.mark(\"card-requested\");\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(dir+\"openagents_web_lander_chat_bg.wasm\",{priority:\"low\"})),\
import(dir+\"openagents_web_lander_chat.js\")]);\
performance.mark(\"card-compiled\");\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"card-instantiated\");\
await idle(600);\
const before=document.activeElement;\
box.addEventListener(\"gpui-failed\",fail,{once:true});\
box.addEventListener(\"gpui-first-frame\",()=>{\
performance.mark(\"card-first-frame\");\
box.addEventListener(\"transitionend\",e=>{if(e.propertyName===\"opacity\")performance.mark(\"card-visible\")});\
box.classList.add(\"ready\");\
setTimeout(()=>{\
for(const field of document.querySelectorAll(\"body>input\"))field.setAttribute(\"aria-label\",\"Message OpenAgents\");\
const now=document.activeElement;\
if(touch&&now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}})},{once:true});\
glue.mount(still)}catch(e){fail()}};\
const handoff=()=>new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"lander8-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.lander8GridWorker=worker}).catch(done)});\
const go=()=>{if(started)return;started=true;handoff().then(()=>idle(400)).then(load)};\
if(performance.getEntriesByName(\"lander5-presented\").length)go();\
else{new PerformanceObserver((list,observer)=>{\
if(list.getEntriesByName(\"lander5-presented\").length){observer.disconnect();go()}}).observe({type:\"mark\"});\
addEventListener(\"load\",()=>setTimeout(go,1500))}";

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: "",
    script: LOADER,
    runner_tail: super::lander6::RUNNER_TAIL,
    main: Some(MAIN),
    grid: None,
};

/// The last document made, and the background build it came from.
static KEPT: Mutex<Option<(BuildKey, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(LANDER8_PATH, get(page))
        .route("/lander8/chat/{id}", get(chat_page))
        .route(LANDER8_SESSION, get(session))
        .route("/lander8/build/{variant}/{file}", get(build_file))
}

fn document(directory: Option<&Path>) -> Arc<Document> {
    let key = build_key(directory);
    if let Ok(kept) = KEPT.lock()
        && let Some((kept_key, document)) = kept.as_ref()
        && *kept_key == key
    {
        return document.clone();
    }
    let document = Arc::new(assemble_with(build(directory), Some(&ADDITION)));
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, document.clone()));
    }
    document
}

async fn page(State(app): State<App>, request: HeaderMap) -> Response {
    // The runner built with its worker exports, as /lander6 serves it.
    let directory = app.config.lander5.as_deref().map(|directory| {
        let worker = directory.join("worker");
        if worker.join(super::lander5::LANDER5_WASM).is_file() {
            worker
        } else {
            directory.to_path_buf()
        }
    });
    let document = tokio::task::spawn_blocking(move || document(directory.as_deref()))
        .await
        .unwrap_or_else(|_| Arc::new(assemble_with(None, Some(&ADDITION))));
    send(&document, &request)
}

/// An open chat's address: the same page, which reads the chat itself.
async fn chat_page(
    State(app): State<App>,
    UrlPath(id): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    if !crate::chat_store::valid_id(&id) {
        return crate::not_found().await;
    }
    page(State(app), request).await
}

/// `{csrf, signed_in}` for the chat: the value its writes carry in
/// `x-openagents-csrf`, checked as the home page's form checks its own, and
/// whether the chats are a signed-in account's. A browser with no visitor
/// cookie gets one here, as on the home page.
async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    let who = crate::chat_owner::who(&app, &headers).await;
    let (owner, fresh) = match &who {
        Who::Account(owner) | Who::Browser(owner) => (owner.clone(), false),
        Who::Nobody => (crate::ask::new_visitor(), true),
        Who::Unchecked(_) => {
            return crate::coder_sync::refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "We couldn't check your login. Try again in a minute.",
            );
        }
    };
    let body = json!({
        "csrf": super::chat::csrf(&app, &owner),
        "signed_in": who.signed_in(),
    });
    let mut response = (
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    super::chat::cookie(&app, &owner, fresh, &mut response);
    response
}

/// One of the chat's files, brotli or gzip when accepted. Only the two
/// builds' glue and module are served.
async fn build_file(
    State(app): State<App>,
    UrlPath((variant, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        LANDER8_GLUE => "text/javascript; charset=utf-8",
        LANDER8_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.lander8.as_deref() else {
        return crate::not_found().await;
    };
    if !LANDER8_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_the_corner_name_the_chat_box_and_no_demo() {
        let document = assemble_with(None, Some(&ADDITION));
        assert!(document.html.contains("<main><h1><svg"));
        assert!(
            document
                .html
                .contains("</svg>OpenAgents</h1><div id=\"gpui-mount\"")
        );
        assert!(!document.html.contains("Download</a>"));
        assert!(!document.html.to_lowercase().contains("demo"));
        assert!(document.html.contains(LOADER));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("'self' 'wasm-unsafe-eval'"));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
    }

    #[test]
    fn the_loader_is_lander7_s_for_this_build() {
        assert!(LOADER.contains("lander5-presented"));
        assert!(LOADER.contains("requestIdleCallback"));
        assert!(LOADER.contains("priority:\"low\""));
        assert!(LOADER.contains("WebAssembly.compileStreaming"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("runner.pause()"));
        assert!(LOADER.contains(LANDER8_WASM) && LOADER.contains(LANDER8_GLUE));
        assert!(LOADER.contains("glue.mount(still)"));
        assert!(STYLE.contains("prefers-reduced-motion:reduce"));
        assert!(!LOADER.contains("</script"));
        assert_eq!(LOADER.matches("await idle(").count(), 2);
    }
}
