//! `/lander7`: `/lander5`'s one-response twilight Grid with the OpenAgents
//! mark and name in the top-left corner, then an AgentGraph demo that loads
//! after it and fades in once its first frame is drawn.
//!
//! The demo (`apps/openagents/web/graph-card`) is one GPUI module: a demo
//! chat drawn by the shared chat views at the center of a graph of the
//! parts it uses (GitHub, Gmail, Google Drive, Google Calendar, web search,
//! a relay, a Pylon, and a Spark wallet). As the scripted chat calls a tool,
//! data moves along that link and back; when it pays the Pylon for a
//! decision, sats move from the wallet and its balance counts down. The
//! chat composer's text field sits at the bottom and sends nothing. Its
//! builds (`apps/openagents/scripts/build-web-graph-card.sh`) are read from
//! the directory the server was started with (`--lander7 DIR`, default
//! `/srv/lander7`):
//!
//! ```text
//! DIR/webgpu/openagents_web_graph_card.js         glue, WebGPU-only build
//! DIR/webgpu/openagents_web_graph_card_bg.wasm    module, WebGPU-only build
//! DIR/webgl/...                                   the same with wgpu's WebGL2 fallback
//! DIR/*/*.br, DIR/*/*.gz                          brotli and gzip copies
//! ```
//!
//! and served at `/lander7/graph/{webgpu,webgl}/{file}`. The loader waits
//! for the background's first full frame, hands the Grid to a worker as
//! `/lander6` does (so GPUI's main-thread work can't hold up its frames),
//! then in an idle moment fetches
//! the build for this browser at low priority and compiles it as it
//! streams, starts it in later idle moments so the background keeps its
//! frames, and fades the canvas in on its first frame. If anything fails,
//! the canvas is removed and the page stays the Grid and the name.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;

use super::lander5::{Addition, BuildKey, Document, assemble_with, build, build_key, send};
use crate::App;

/// The page.
pub(crate) const LANDER7_PATH: &str = "/lander7";

/// The demo's two builds, by the directory each sits in.
pub(crate) const LANDER7_VARIANTS: [&str; 2] = ["webgpu", "webgl"];
/// Each build's files, as `build-web-graph-card.sh` names them.
pub(crate) const LANDER7_GLUE: &str = "openagents_web_graph_card.js";
pub(crate) const LANDER7_WASM: &str = "openagents_web_graph_card_bg.wasm";

/// `/lander5`'s background, the mark and name in the corner where the
/// desktop chat's sidebar has them (an 18 px mark, 10 px apart from the
/// name at 15 px, semibold), and the demo's canvas over the whole page,
/// hidden until its first frame.
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
@media (prefers-reduced-motion:reduce){#gpui-mount{transform:none;transition:opacity .15s linear,visibility 0s .15s}}";

/// The mark (`icons/openagents-logo.svg`, as the desktop sidebar draws it),
/// the name as the page's heading, and the demo's box. The box is a picture
/// for anyone reading the page another way.
const MAIN: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>\
<div id=\"gpui-mount\" role=\"img\" aria-label=\"A demo chat wired to GitHub, Gmail, Google Drive, \
Google Calendar, web search, a relay, a Pylon, and a Bitcoin wallet. The agent reads new issues, \
pays 21 demo sats for a decision, and drafts an email for you to approve.\"></div>";

/// The loader: after the background's first full frame and an idle moment,
/// the build for this browser, fetched at low priority and compiled as it
/// streams in. It is started, then mounted, each in an idle moment of its
/// own, so no one task holds the background's frames for long. The box
/// fades in when GPUI reports its first frame at the box's size
/// (`gpui-first-frame`, bubbling from its canvas). GPUI focuses its text
/// field when it starts; focus goes back where it was, after GPUI's frame
/// callback returns (the event fires inside it), since moving focus
/// re-enters GPUI. The field stays reachable with Tab and is named for the
/// composer it types into.
const LOADER: &str = "\
const box=document.getElementById(\"gpui-mount\");\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const fail=()=>{box.remove()};\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
let started=false;\
const load=async()=>{try{\
performance.mark(\"card-start\");\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/lander7/graph/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
performance.mark(\"card-requested\");\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(dir+\"openagents_web_graph_card_bg.wasm\",{priority:\"low\"})),\
import(dir+\"openagents_web_graph_card.js\")]);\
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
if(now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}})},{once:true});\
glue.mount(still)}catch(e){fail()}};\
const handoff=()=>new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"lander7-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.lander7GridWorker=worker}).catch(done)});\
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
        .route(LANDER7_PATH, get(page))
        .route("/lander7/graph/{variant}/{file}", get(graph_file))
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

/// One of the demo's files, brotli or gzip when accepted. Only the two
/// builds' glue and module are served.
async fn graph_file(
    State(app): State<App>,
    UrlPath((variant, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        LANDER7_GLUE => "text/javascript; charset=utf-8",
        LANDER7_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.lander7.as_deref() else {
        return crate::not_found().await;
    };
    if !LANDER7_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_the_corner_name_and_no_download_link() {
        let document = assemble_with(None, Some(&ADDITION));
        assert!(document.html.contains("<main><h1><svg"));
        assert!(
            document
                .html
                .contains("</svg>OpenAgents</h1><div id=\"gpui-mount\"")
        );
        assert!(!document.html.contains("Download</a>"));
        assert!(!document.html.contains("href=\"/download\""));
        assert!(document.html.contains(LOADER));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("'self' 'wasm-unsafe-eval'"));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
    }

    #[test]
    fn the_loader_compiles_first_and_starts_in_idle_moments() {
        assert!(LOADER.contains("lander5-presented"));
        assert!(LOADER.contains("requestIdleCallback"));
        assert!(LOADER.contains("priority:\"low\""));
        assert!(LOADER.contains("WebAssembly.compileStreaming"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("runner.pause()"));
        assert!(LOADER.contains("navigator.gpu"));
        assert!(LOADER.contains("glue.mount(still)"));
        assert!(LOADER.contains("Message OpenAgents"));
        assert!(STYLE.contains("prefers-reduced-motion:reduce"));
        assert!(STYLE.contains("safe-area-inset-bottom"));
        assert!(!LOADER.contains("</script"));
        // Load, start, and mount are separate tasks.
        assert_eq!(LOADER.matches("await idle(").count(), 2);
    }
}
