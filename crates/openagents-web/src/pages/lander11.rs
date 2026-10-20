//! `/lander11`: `/lander8`'s real chat over Verse's own Grid.
//!
//! The page is `/lander5`'s one response (the Grid shader inline, so the
//! first paint is instant), with the mark and name in the corner. After
//! the shader's first full frame the loader hands it to a worker (as
//! `/lander6` does) and starts `/lander8`'s chat (`/lander8/build/`) in
//! idle slices: the composer at the bottom and the conversation in a glass
//! card that grows as it goes. A chat's address is `/lander11/chat/{id}`.
//!
//! Once the chat is drawn, the loader fetches the Verse web build
//! (`everglade-web`, the same files `/verse` serves under `/everglade/`) at
//! low priority, compiles it as it streams, and starts it in an idle slice.
//! On this path the module opens the shared Grid zone with its camera
//! circling the plaza by itself, joins no one, and leaves the keyboard and
//! mouse to the chat. When its first frame is drawn its canvas cross-fades
//! in behind the chat and the shader's worker stops. A small Walk button
//! then hands the keyboard and mouse to Verse (`verse_walk`); Escape or the
//! button gives them back.
//!
//! Without the Verse build the page stays the shader Grid with the chat;
//! without the chat build, a link to the home page's chat shows.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;

use super::lander5::{Addition, BuildKey, Document, assemble_with, build, build_key, send};
use crate::App;

/// The page.
pub(crate) const LANDER11_PATH: &str = "/lander11";

/// `/lander8`'s look, with Verse's canvas over the shader's (hidden until
/// its first frame) and under the chat's, the Walk button, and the status
/// line the Verse build writes to, kept for screen readers.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0a0a0a 22%,#111111 36%,#202020 41.6%,#0d0d0d 42.2%,#0a0a0a 60%,#0a0a0a 100%);\
color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
#everglade-canvas{opacity:0;transition:opacity 1.2s ease;outline:none}\
#everglade-canvas.ready{opacity:1}\
.sr{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
h1{position:fixed;z-index:2;top:calc(18px + env(safe-area-inset-top));left:calc(18px + env(safe-area-inset-left));\
margin:0;display:flex;align-items:center;gap:10px;font-size:15px;font-weight:600;line-height:20px;letter-spacing:0;\
pointer-events:none;text-shadow:0 1px 12px rgba(0,0,0,.5)}\
h1 svg{width:18px;height:18px;flex:none}\
#gpui-mount{position:fixed;z-index:1;top:env(safe-area-inset-top);right:env(safe-area-inset-right);\
bottom:env(safe-area-inset-bottom);left:env(safe-area-inset-left);\
opacity:0;visibility:hidden;transform:translateY(8px);\
transition:opacity .5s cubic-bezier(.22,.8,.3,1),transform .5s cubic-bezier(.22,.8,.3,1),visibility 0s .5s}\
#gpui-mount canvas{position:absolute}\
#gpui-mount.ready{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
.walking #gpui-mount.ready{opacity:.18;pointer-events:none}\
#lander11-walk{position:fixed;z-index:3;top:calc(14px + env(safe-area-inset-top));right:calc(16px + env(safe-area-inset-right));\
padding:6px 14px;border-radius:999px;border:1px solid rgba(255,255,255,.18);background:rgba(10,10,10,.55);\
color:#e5e5e5;font-family:inherit;font-size:13px;font-weight:500;line-height:18px;cursor:pointer;-webkit-backdrop-filter:blur(10px);backdrop-filter:blur(10px)}\
#lander11-walk:hover{background:rgba(30,30,30,.7)}#lander11-walk:focus-visible{outline:2px solid #fff;outline-offset:2px}\
#lander11-walk[aria-pressed=true]{background:#ededed;color:#0a0a0a}\
#lander11-walk[hidden]{display:none}\
#lander11-fallback{position:fixed;z-index:2;left:50%;bottom:calc(32px + env(safe-area-inset-bottom));transform:translateX(-50%);\
padding:13px 26px;border-radius:999px;background:#ededed;color:#0a0a0a;font-size:15px;font-weight:600;text-decoration:none}\
#lander11-fallback[hidden]{display:none}\
@media (prefers-reduced-motion:reduce){#gpui-mount{transform:none;transition:opacity .15s linear,visibility 0s .15s}\
#everglade-canvas{transition-duration:.15s}}";

/// The mark and name, Verse's canvas and status line, the chat's box, the
/// Walk button, and the link shown when the chat can't start.
const MAIN: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>\
<canvas id=\"everglade-canvas\" tabindex=\"-1\" aria-label=\"The Grid, where people, agents, and pylons meet\"></canvas>\
<p id=\"everglade-status\" class=\"sr\" aria-live=\"polite\"></p>\
<div id=\"gpui-mount\" role=\"region\" aria-label=\"Chat with OpenAgents\"></div>\
<button id=\"lander11-walk\" type=\"button\" aria-pressed=\"false\" hidden>Walk</button>\
<a id=\"lander11-fallback\" href=\"/\" hidden>Chat with OpenAgents</a>";

/// The loader: `/lander8`'s for the chat, then Verse. After the shader's
/// first full frame it hands the Grid to a worker and starts the chat in
/// idle slices. Once the chat is drawn (or can't start), it fetches the
/// Verse build at low priority, compiling it as it streams, and starts it
/// in an idle slice. The build hides its status line when its first frame
/// is drawn; the loader then fades Verse's canvas in and, once it shows,
/// stops the shader. The Walk button hands Verse the keyboard and mouse.
const LOADER: &str = "\
const box=document.getElementById(\"gpui-mount\");\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const touch=matchMedia(\"(pointer: coarse)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
let chatDone;const chatSettled=new Promise(r=>chatDone=r);\
const fail=()=>{box.remove();document.getElementById(\"lander11-fallback\").hidden=false;chatDone()};\
let started=false;\
const load=async()=>{try{\
performance.mark(\"card-start\");\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/lander8/build/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
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
box.classList.add(\"ready\");\
setTimeout(()=>{\
for(const field of document.querySelectorAll(\"body>input\"))field.setAttribute(\"aria-label\",\"Message OpenAgents\");\
const now=document.activeElement;\
if(touch&&now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}\
chatDone()})},{once:true});\
glue.mount(still)}catch(e){fail()}};\
const verse=async()=>{try{\
await chatSettled;await idle(800);\
performance.mark(\"verse-start\");\
const canvas=document.getElementById(\"everglade-canvas\"),status=document.getElementById(\"everglade-status\");\
const drawn=new Promise(r=>new MutationObserver((list,observer)=>{\
if(status.style.display===\"none\"){observer.disconnect();r()}}).observe(status,{attributes:true,attributeFilter:[\"style\"]}));\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(\"/everglade/everglade_web_bg.wasm\",{priority:\"low\"})),\
import(\"/everglade/everglade_web.js\")]);\
performance.mark(\"verse-compiled\");\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"verse-instantiated\");\
await drawn;\
await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));\
performance.mark(\"verse-first-frame\");\
canvas.addEventListener(\"transitionend\",()=>{performance.mark(\"verse-visible\");\
const worker=globalThis.lander11GridWorker;if(worker)worker.terminate();\
const runner=globalThis.lander5Runner;if(runner)runner.pause();\
for(const id of [\"lander5-canvas\",\"lander11-grid\"]){const c=document.getElementById(id);if(c)c.remove()}},{once:true});\
canvas.classList.add(\"ready\");\
const walk=document.getElementById(\"lander11-walk\");\
const set=on=>{glue.verse_walk(on);walk.setAttribute(\"aria-pressed\",String(on));\
document.body.classList.toggle(\"walking\",on);\
if(on){const now=document.activeElement;if(now&&now!==walk)now.blur();canvas.focus({preventScroll:true})}};\
walk.addEventListener(\"click\",()=>set(walk.getAttribute(\"aria-pressed\")!==\"true\"));\
addEventListener(\"keydown\",e=>{if(e.key===\"Escape\"&&walk.getAttribute(\"aria-pressed\")===\"true\")set(false)});\
walk.hidden=false}catch(e){}};\
const handoff=()=>new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"lander11-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.lander11GridWorker=worker}).catch(done)});\
const go=()=>{if(started)return;started=true;handoff().then(()=>idle(400)).then(()=>{load();verse()})};\
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
        .route(LANDER11_PATH, get(page))
        .route("/lander11/chat/{id}", get(chat_page))
}

fn document(directory: Option<&std::path::Path>) -> Arc<Document> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_the_chat_and_verse_s_canvas_behind_it() {
        let document = assemble_with(None, Some(&ADDITION));
        assert!(
            document
                .html
                .contains("</svg>OpenAgents</h1><canvas id=\"everglade-canvas\"")
        );
        assert!(document.html.contains("<div id=\"gpui-mount\""));
        assert!(!document.html.contains("Download</a>"));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
        assert!(oa_copy::violations(MAIN, &[]).is_empty());
    }

    #[test]
    fn the_loader_starts_verse_after_the_chat() {
        assert!(LOADER.contains("await chatSettled"));
        assert!(LOADER.contains(&format!("/everglade/{}", super::super::everglade::WASM)));
        assert!(LOADER.contains(&format!("/everglade/{}", super::super::everglade::GLUE)));
        assert!(LOADER.contains("openagents_web_lander_chat_bg.wasm"));
        assert!(LOADER.contains("glue.verse_walk(on)"));
        assert!(LOADER.contains("worker.terminate()"));
        assert!(!LOADER.contains("</script"));
    }
}
