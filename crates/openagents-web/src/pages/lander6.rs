//! `/lander6`: `/lander5` (the one-response twilight Grid, title, and
//! Download link), then an agent demo card that loads after it and fades in
//! once its first frame is drawn.
//!
//! The card is the desktop app's chat views drawn by GPUI
//! (`apps/openagents/web/card`), playing the desktop demo's scripted reply
//! in a loop inside the page's `#gpui-mount` box. Its builds
//! (`apps/openagents/scripts/build-web-card.sh`) are read from the directory
//! the server was started with (`--lander6 DIR`, default `/srv/lander6`):
//!
//! ```text
//! DIR/webgpu/openagents_web_card.js         glue, WebGPU-only build
//! DIR/webgpu/openagents_web_card_bg.wasm    module, WebGPU-only build
//! DIR/webgl/...                             the same with wgpu's WebGL2 fallback
//! DIR/*/*.br, DIR/*/*.gz                    brotli and gzip copies
//! ```
//!
//! and served at `/lander6/card/{webgpu,webgl}/{file}`.
//!
//! The background starts exactly as `/lander5`'s does, on the page's main
//! thread. Before the card loads, the loader hands the Grid to a dedicated
//! worker: the same runner (`--lander5 DIR`'s `worker/` build, with its
//! worker exports) started from the page's own runner script, drawing on a
//! second canvas transferred with `transferControlToOffscreen`, from the
//! scene time the page had reached. Once the worker has presented a frame
//! the page's loop stops, so GPUI's work on the main thread (instantiating,
//! starting, its first layouts) can't hold up a Grid frame. Without
//! `OffscreenCanvas`, or with reduced motion, the Grid stays on the page.
//!
//! The loader then waits for an idle moment, picks the WebGPU build when `navigator.gpu` gives an adapter and
//! the WebGL2 build otherwise, and fetches it at low priority with streaming
//! compilation. When the card's first frame is drawn, the box fades and
//! scales in (a short fade alone for reduced motion). If anything fails, the
//! box is removed and the page stays `/lander5`.

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
pub(crate) const LANDER6_PATH: &str = "/lander6";

/// The card's two builds, by the directory each sits in.
pub(crate) const LANDER6_VARIANTS: [&str; 2] = ["webgpu", "webgl"];
/// Each build's files, as `build-web-card.sh` names them.
pub(crate) const LANDER6_GLUE: &str = "openagents_web_card.js";
pub(crate) const LANDER6_WASM: &str = "openagents_web_card_bg.wasm";

/// `/lander5`'s look, with room for the card under the link: the stack
/// starts higher and the title is a little smaller. The card is a glass box
/// over the Grid, hidden until its first frame.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0d0a09 22%,#1c0f09 36%,#3b1a0b 41.6%,#120c0a 42.2%,#0b0a0a 60%,#0a0a0a 100%);\
color:#ededed;font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
main{position:relative;box-sizing:border-box;min-height:100%;display:flex;flex-direction:column;align-items:center;\
justify-content:center;gap:clamp(22px,3.6vh,36px);padding:4vh 16px;text-align:center}\
h1{margin:0;font-size:clamp(46px,8.4vw,112px);font-weight:600;letter-spacing:-.035em;line-height:1;\
text-shadow:0 2px 30px rgba(0,0,0,.55)}\
a{display:inline-block;padding:15px 34px;border-radius:999px;background:#ededed;color:#0a0a0a;\
font-size:17px;font-weight:600;text-decoration:none;box-shadow:0 0 0 1px rgba(255,255,255,.2),0 8px 40px rgba(255,140,40,.18)}\
a:hover{background:#fff}\
a:focus-visible{outline:2px solid #ffb35c;outline-offset:4px}\
#gpui-mount{position:relative;flex:none;width:min(680px,calc(100vw - 32px));height:min(440px,46vh);margin-top:4px;\
border-radius:18px;overflow:hidden;text-align:left;background:rgba(12,12,12,.58);\
-webkit-backdrop-filter:blur(18px) saturate(1.25);backdrop-filter:blur(18px) saturate(1.25);\
border:1px solid rgba(255,255,255,.1);box-shadow:0 24px 90px rgba(0,0,0,.6),0 0 0 1px rgba(0,0,0,.35);\
opacity:0;visibility:hidden;transform:translateY(10px) scale(.96);\
transition:opacity .4s cubic-bezier(.22,.8,.3,1),transform .4s cubic-bezier(.22,.8,.3,1),visibility 0s .4s}\
#gpui-mount canvas{position:absolute}\
#gpui-mount.ready{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
@media (prefers-reduced-motion:reduce){#gpui-mount{transform:none;transition:opacity .15s linear,visibility 0s .15s}}";

/// The card's box. It is a picture of a chat for anyone reading the page
/// another way.
const CARD: &str = "<div id=\"gpui-mount\" role=\"img\" aria-label=\"A demo chat: an agent \
answers a question, reads a file, and runs the tests.\"></div>";

/// The loader: after the background's first full frame and an idle moment,
/// the card's build for this browser, fetched at low priority and compiled
/// off the main thread as it streams in (`compileStreaming`). Instantiating
/// it (which runs its static constructors) and starting GPUI each take an
/// idle slice of their own, so neither lands in the same task as the other
/// or as a Grid frame. The box fades in when GPUI reports the card's first
/// frame at the box's size (`gpui-first-frame`, bubbling from its canvas). GPUI
/// focuses a text field of its own when it starts; the card takes no
/// typing, so that field is hidden from the keyboard and screen readers and
/// focus goes back where it was. That happens after GPUI's frame callback
/// returns (the event fires inside it), since moving focus re-enters GPUI.
const LOADER: &str = "\
const box=document.getElementById(\"gpui-mount\");\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const fail=()=>{box.remove()};\
const idle=()=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:400}));\
let started=false;\
const load=async()=>{try{\
performance.mark(\"card-start\");\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/lander6/card/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
performance.mark(\"card-requested\");\
const compiled=WebAssembly.compileStreaming(fetch(dir+\"openagents_web_card_bg.wasm\",{priority:\"low\"}));\
const glue=await import(dir+\"openagents_web_card.js\");\
const module=await compiled;\
performance.mark(\"card-compiled\");\
await idle();\
await glue.default({module_or_path:module});\
performance.mark(\"card-instantiated\");\
await idle();\
const before=document.activeElement;\
box.addEventListener(\"gpui-failed\",fail,{once:true});\
box.addEventListener(\"gpui-first-frame\",()=>{\
performance.mark(\"card-first-frame\");\
box.addEventListener(\"transitionend\",e=>{if(e.propertyName===\"opacity\")performance.mark(\"card-visible\")});\
box.classList.add(\"ready\");\
setTimeout(()=>{\
for(const field of document.querySelectorAll(\"body>input\")){field.tabIndex=-1;field.setAttribute(\"aria-hidden\",\"true\")}\
const now=document.activeElement;\
if(now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}})},{once:true});\
glue.mount(still)}catch(e){fail()}};\
const handoff=()=>new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"lander6-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.lander6GridWorker=worker}).catch(done)});\
const go=()=>{if(started)return;started=true;\
handoff().then(()=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(load,{timeout:400}))};\
if(performance.getEntriesByName(\"lander5-presented\").length)go();\
else{new PerformanceObserver((list,observer)=>{\
if(list.getEntriesByName(\"lander5-presented\").length){observer.disconnect();go()}}).observe({type:\"mark\"});\
addEventListener(\"load\",()=>setTimeout(go,1500))}";

/// The end of the runner's own script on this page. On the page it hands
/// the loader the runner (its start promise, `pause`, and `scene_time`). The
/// loader starts a worker from this same script text, where it runs the
/// Grid on the transferred canvas instead and answers the page's messages:
/// the canvas and shaders to start, a new view size, or a request for its
/// recent frame times.
const RUNNER_TAIL: &str = "\
if(typeof WorkerGlobalScope!==\"undefined\"){self.onmessage=async e=>{await ready;const d=e.data;\
if(d.canvas)start_worker(d.canvas,d.sky,d.grid,d.w,d.h,d.dpr,d.time);\
else if(d.resize)resize_worker(d.w,d.h,d.dpr);\
else if(d.frames)postMessage({frames:frame_times()})}}\
else globalThis.lander5Runner={ready,pause,scene_time};";

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: CARD,
    script: LOADER,
    runner_tail: RUNNER_TAIL,
};

/// The last document made, and the background build it came from.
static KEPT: Mutex<Option<(BuildKey, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(LANDER6_PATH, get(page))
        .route("/lander6/card/{variant}/{file}", get(card_file))
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
    // The runner built with its worker exports, beside /lander5's own.
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

/// One of the card's files, brotli or gzip when accepted. Only the two
/// builds' glue and module are served.
async fn card_file(
    State(app): State<App>,
    UrlPath((variant, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        LANDER6_GLUE => "text/javascript; charset=utf-8",
        LANDER6_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.lander6.as_deref() else {
        return crate::not_found().await;
    };
    if !LANDER6_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_adds_the_card_and_its_loader_to_lander5() {
        let document = assemble_with(None, Some(&ADDITION));
        assert!(document.html.contains(
            "<main><h1>OpenAgents</h1><a href=\"/download\">Download</a><div id=\"gpui-mount\""
        ));
        assert!(document.html.contains(LOADER));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("'self' 'wasm-unsafe-eval'"));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        // The runner keeps its start promise and hands itself to the loader,
        // or, in the worker, answers the loader's messages.
        assert!(RUNNER_TAIL.contains("start_worker(d.canvas"));
        assert!(RUNNER_TAIL.contains("globalThis.lander5Runner={ready,pause,scene_time}"));
        assert!(!policy.contains("unsafe-inline"));
    }

    #[test]
    fn the_loader_waits_for_the_background_and_respects_reduced_motion() {
        assert!(LOADER.contains("lander5-presented"));
        assert!(LOADER.contains("requestIdleCallback"));
        assert!(LOADER.contains("priority:\"low\""));
        assert!(LOADER.contains("navigator.gpu"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("runner.pause()"));
        assert!(LOADER.contains("WebAssembly.compileStreaming"));
        assert!(LOADER.contains("glue.mount(still)"));
        assert!(STYLE.contains("prefers-reduced-motion:reduce"));
        assert!(!LOADER.contains("</script"));
    }
}
