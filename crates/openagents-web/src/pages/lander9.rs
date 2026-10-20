//! `/lander9`: `/lander7` with hand tracking. The same twilight Grid, the
//! mark and name in the corner, and the AgentGraph demo around a demo chat,
//! plus a hand button in the top-right corner. Clicking it asks for the
//! camera (never before) and starts a hand landmarker in the browser; the
//! person's hands are drawn over the page, mirrored, and they can point at
//! a part to read it, pinch to select it or to type a message, pinch and
//! hold (or make a fist) to move a part or the chat, drop a tool on the
//! chat to start a demo turn with it, pinch with both hands to spread the
//! parts, and hold an open palm to let go. The mouse does all of it too.
//!
//! The landmarker is MediaPipe Tasks Vision's HandLandmarker (Google,
//! Apache-2.0), pinned by digest and served from this site
//! (`scripts/fetch-hand-landmarker.sh`); the owner approved running it in
//! the browser on 2026-10-20. Frames never leave the page: the camera feeds
//! a `<video>` element, the landmarker reads it in the page, and only its
//! 21 points a hand go on, into the demo's wasm module, where the One Euro
//! smoothing, the gestures (`apps/openagents/web/hands`, with
//! `crates/coder-hands`' pose rules), and the interactions
//! (`apps/openagents/web/graph-card`, `hands` feature) run in Rust. The
//! script here only moves frames and points between the two.
//!
//! The files are read from the directory the server was started with
//! (`--lander9 DIR`, default `/srv/lander9`):
//!
//! ```text
//! DIR/graph/{webgpu,webgl}/...   build-web-graph-card.sh OUTDIR --hands
//! DIR/hands/...                  fetch-hand-landmarker.sh OUTDIR
//! ```
//!
//! served at `/lander9/graph/{webgpu,webgl}/{file}`,
//! `/lander9/hands/{vision_bundle.mjs,hand_landmarker.task}`, and
//! `/lander9/hands/wasm/{vision_wasm_internal.js,vision_wasm_internal.wasm}`.
//! None of the hand files is fetched before the button is clicked.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;

use super::lander5::{Addition, BuildKey, Document, assemble_with, build, build_key, send};
use super::lander7::{LANDER7_GLUE, LANDER7_VARIANTS, LANDER7_WASM};
use crate::App;

/// The page.
pub(crate) const LANDER9_PATH: &str = "/lander9";

/// The landmarker's files the page may load, by where they are served.
pub(crate) const LANDER9_HAND_FILES: [(&str, &str); 2] = [
    ("vision_bundle.mjs", "text/javascript; charset=utf-8"),
    ("hand_landmarker.task", "application/octet-stream"),
];
pub(crate) const LANDER9_HAND_WASM: [(&str, &str); 2] = [
    ("vision_wasm_internal.js", "text/javascript; charset=utf-8"),
    ("vision_wasm_internal.wasm", "application/wasm"),
];

/// `/lander7`'s style, and the hand button, its note, and the camera's
/// small picture.
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
#hand-bar{position:fixed;z-index:3;top:calc(12px + env(safe-area-inset-top));right:calc(14px + env(safe-area-inset-right));\
display:flex;flex-direction:column;align-items:flex-end;gap:8px}\
#hand-bar div{display:flex;gap:8px}\
#hand-bar button{font:inherit;font-size:12.5px;color:#e5e5e5;background:rgba(18,18,18,.9);border:1px solid rgba(255,255,255,.16);\
border-radius:10px;height:34px;padding:0 11px}\
#hand-bar button:hover{border-color:rgba(255,255,255,.34)}\
#hand-bar button:focus-visible{outline:2px solid #ededed;outline-offset:2px}\
#hand-bar button[hidden]{display:none}\
#hand-toggle{width:34px;padding:0!important;display:flex;align-items:center;justify-content:center}\
#hand-toggle svg{width:18px;height:18px}\
#hand-toggle[aria-pressed=true]{background:#ededed;color:#0a0a0a;border-color:#ededed}\
#hand-note{margin:0;max-width:250px;font-size:12px;line-height:17px;color:#b5b5b5;background:rgba(10,10,10,.92);\
border:1px solid rgba(255,255,255,.12);border-radius:8px;padding:6px 9px;opacity:0;visibility:hidden;transition:opacity .2s}\
#hand-note.show,#hand-bar:hover #hand-note,#hand-bar:focus-within #hand-note{opacity:1;visibility:visible}\
#hand-video{position:fixed;z-index:3;left:calc(18px + env(safe-area-inset-left));bottom:calc(26px + env(safe-area-inset-bottom));\
width:160px;height:120px;object-fit:cover;transform:scaleX(-1);border-radius:10px;border:1px solid rgba(255,255,255,.16);\
filter:grayscale(1);display:none}\
#hand-video.show{display:block}\
@media (prefers-reduced-motion:reduce){#gpui-mount{transform:none;transition:opacity .15s linear,visibility 0s .15s}#hand-note{transition:none}}";

/// The note under the hand button.
pub(crate) const HAND_NOTE: &str =
    "Hand tracking runs on this device. Your camera's picture is never uploaded.";

/// The mark and name, the demo's box, and the hand controls: a real button
/// with a name, a second one for the camera's picture once tracking runs,
/// and the note, which is also where the button says how it is going.
const MAIN: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>\
<div id=\"gpui-mount\" role=\"img\" aria-label=\"A demo chat wired to GitHub, Gmail, Google Drive, \
Google Calendar, web search, a relay, a Pylon, and a Bitcoin wallet. Drag a tool onto the chat, \
with the mouse or your hand, to start a demo turn with it.\"></div>\
<div id=\"hand-bar\"><div>\
<button id=\"hand-preview\" type=\"button\" aria-pressed=\"false\" hidden>Show camera</button>\
<button id=\"hand-toggle\" type=\"button\" aria-pressed=\"false\" aria-label=\"Turn on hand tracking\" \
aria-describedby=\"hand-note\" title=\"Hand tracking\">\
<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" stroke-linecap=\"round\" \
stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M8 12V5.5a1.5 1.5 0 0 1 3 0V11\"/>\
<path d=\"M11 10.5V4a1.5 1.5 0 0 1 3 0v6.5\"/><path d=\"M14 10.5V5.5a1.5 1.5 0 0 1 3 0V12\"/>\
<path d=\"M17 9a1.5 1.5 0 0 1 3 0v5a7 7 0 0 1-7 7h-1a6 6 0 0 1-4.6-2.2L4.3 15a1.5 1.5 0 0 1 2.3-2L8 14.5\"/></svg>\
</button></div>\
<p id=\"hand-note\" role=\"status\" aria-live=\"polite\">Hand tracking runs on this device. Your camera's picture is never uploaded.</p></div>\
<video id=\"hand-video\" muted playsinline aria-hidden=\"true\"></video>";

/// `/lander7`'s loader for `/lander9`'s builds, and the hand bridge.
///
/// The bridge, on the button's click and not before: asks for the camera
/// while it imports the landmarker (both from this site), then on each new
/// camera frame runs the landmarker on the `<video>` element and passes the
/// points, packed as floats, to the module's `hand_frame`. Clicking again
/// stops the camera's tracks and closes the landmarker. A refused or
/// missing camera is said plainly in the note and the mouse keeps working.
/// `lander9Hands.synth(pose, x, y[, pose2, x2, y2])` feeds made-up hands to
/// the same entry, for checking without a camera, and
/// `lander9Hands.stats()` reads the rate and the landmarker's time.
const LOADER: &str = "\
const box=document.getElementById(\"gpui-mount\");\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
let glueReady;const glueP=new Promise(r=>glueReady=r);\
const fail=()=>{box.remove();document.getElementById(\"hand-bar\").remove()};\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
let started=false;\
const load=async()=>{try{\
performance.mark(\"card-start\");\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/lander9/graph/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
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
box.classList.add(\"ready\");\
setTimeout(()=>{\
for(const field of document.querySelectorAll(\"body>input\"))field.setAttribute(\"aria-label\",\"Message OpenAgents\");\
const now=document.activeElement;\
if(now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}})},{once:true});\
glue.mount(still);glueReady(glue)}catch(e){fail()}};\
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
addEventListener(\"load\",()=>setTimeout(go,1500))}\
const hb=document.getElementById(\"hand-toggle\"),hp=document.getElementById(\"hand-preview\"),\
hn=document.getElementById(\"hand-note\"),hv=document.getElementById(\"hand-video\");\
const NOTE=hn.textContent;let hs=null;\
const say=(t,show)=>{hn.textContent=t;hn.classList.toggle(\"show\",!!show)};\
const pressed=on=>{hb.setAttribute(\"aria-pressed\",String(on));hb.setAttribute(\"aria-label\",on?\"Turn off hand tracking\":\"Turn on hand tracking\")};\
const handStop=()=>{const s=hs;if(!s)return;hs=null;s.on=false;\
if(s.stream)for(const t of s.stream.getTracks())t.stop();if(s.lm)s.lm.close();\
hv.srcObject=null;hv.classList.remove(\"show\");hp.hidden=true;hp.setAttribute(\"aria-pressed\",\"false\");hp.textContent=\"Show camera\";\
pressed(false);glueP.then(g=>g.hands_on(false));say(NOTE,false)};\
const handStart=async()=>{const s={on:true};hs=s;pressed(true);say(\"Starting your camera. \"+NOTE,true);\
performance.mark(\"hands-click\");\
try{if(!navigator.mediaDevices||!navigator.mediaDevices.getUserMedia)throw new Error(\"none\");\
const model=(async()=>{const v=await import(\"/lander9/hands/vision_bundle.mjs\");\
const files=await v.FilesetResolver.forVisionTasks(\"/lander9/hands/wasm\");\
const make=d=>v.HandLandmarker.createFromOptions(files,{baseOptions:{modelAssetPath:\"/lander9/hands/hand_landmarker.task\",delegate:d},\
runningMode:\"VIDEO\",numHands:2,minHandDetectionConfidence:.6,minHandPresenceConfidence:.6,minTrackingConfidence:.5});\
try{return await make(\"GPU\")}catch(e){return await make(\"CPU\")}})();\
model.catch(()=>{});\
const stream=await navigator.mediaDevices.getUserMedia({video:{facingMode:\"user\",width:{ideal:640},height:{ideal:480}},audio:false});\
s.stream=stream;if(!s.on){for(const t of stream.getTracks())t.stop();return}\
hv.srcObject=stream;await hv.play();performance.mark(\"hands-camera\");say(\"Loading hand tracking. \"+NOTE,true);\
const lm=await model;if(!s.on){lm.close();return}s.lm=lm;performance.mark(\"hands-ready\");\
const g=await glueP;if(!s.on)return;g.hands_on(true);hp.hidden=false;say(\"Hold up a hand. \"+NOTE,true);\
setTimeout(()=>{if(hs===s)say(NOTE,false)},5000);\
const buf=new Float32Array(2+2*64);let last=-1,first=true;\
const step=()=>{if(!s.on)return;\
if(hv.readyState>=2&&hv.currentTime!==last){last=hv.currentTime;const t=performance.now();\
const r=lm.detectForVideo(hv,t),ms=performance.now()-t,n=Math.min(r.landmarks.length,2);\
buf[0]=hv.videoWidth/hv.videoHeight;buf[1]=n;\
for(let h=0;h<n;h++){const o=2+h*64,p=r.landmarks[h],c=r.handedness[h];buf[o]=c&&c[0]?c[0].score:1;\
for(let j=0;j<21;j++){buf[o+1+j*3]=p[j].x;buf[o+2+j*3]=p[j].y;buf[o+3+j*3]=p[j].z}}\
g.hand_frame(buf.subarray(0,2+n*64),t,ms);if(first&&n){first=false;performance.mark(\"hands-first-frame\")}}\
if(hv.requestVideoFrameCallback)hv.requestVideoFrameCallback(step);else requestAnimationFrame(step)};step()}\
catch(e){if(hs===s){handStop();say(e&&e.name===\"NotAllowedError\"?\"Camera access is off for this page, so hand tracking can't start. \
The mouse still works.\":e&&e.name===\"NotFoundError\"?\"We couldn't find a camera. The mouse still works.\":\
\"We couldn't start hand tracking here. The mouse still works.\",true)}}};\
hb.addEventListener(\"click\",()=>{if(hs)handStop();else handStart()});\
hp.addEventListener(\"click\",()=>{const on=hv.classList.toggle(\"show\");hp.setAttribute(\"aria-pressed\",String(on));\
hp.textContent=on?\"Hide camera\":\"Show camera\"});\
globalThis.lander9Hands={synth:(p,x,y,p2,x2,y2)=>glueP.then(g=>g.synth_hand(p,x,y,p2,x2||0,y2||0,performance.now())),\
stats:()=>glueP.then(g=>JSON.parse(g.hand_stats()))};";

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
        .route(LANDER9_PATH, get(page))
        .route("/lander9/graph/{variant}/{file}", get(graph_file))
        .route("/lander9/hands/{file}", get(hand_file))
        .route("/lander9/hands/wasm/{file}", get(hand_wasm))
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

/// One of the demo's files, as `/lander7` serves its own.
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
    let Some(root) = app.config.lander9.as_deref() else {
        return crate::not_found().await;
    };
    if !LANDER7_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join("graph").join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}

async fn hand_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    serve_hand(&app, &LANDER9_HAND_FILES, "", &file, &request).await
}

async fn hand_wasm(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    serve_hand(&app, &LANDER9_HAND_WASM, "wasm", &file, &request).await
}

/// One of the landmarker's files, by name from the list; nothing else.
async fn serve_hand(
    app: &App,
    files: &[(&str, &'static str)],
    sub: &str,
    file: &str,
    request: &HeaderMap,
) -> Response {
    let Some((_, content_type)) = files.iter().find(|(name, _)| *name == file) else {
        return crate::not_found().await;
    };
    let Some(root) = app.config.lander9.as_deref() else {
        return crate::not_found().await;
    };
    let directory = root.join("hands").join(sub);
    super::everglade::serve_build(&directory, file, content_type, request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_the_hand_button_and_loads_nothing_for_it_first() {
        let document = assemble_with(None, Some(&ADDITION));
        assert!(
            document
                .html
                .contains("<button id=\"hand-toggle\" type=\"button\"")
        );
        assert!(
            document
                .html
                .contains("aria-label=\"Turn on hand tracking\"")
        );
        assert!(document.html.contains(HAND_NOTE));
        assert!(
            document
                .html
                .contains("</svg>OpenAgents</h1><div id=\"gpui-mount\"")
        );
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("connect-src 'self';"));
        assert!(!policy.contains("unsafe-inline"));
        // The landmarker is imported, and the camera asked for, only in the
        // click's handler.
        let click = LOADER.find("const handStart=").unwrap();
        for late in [
            "getUserMedia(",
            "import(\"/lander9/hands/vision_bundle.mjs\")",
            "hand_landmarker.task",
        ] {
            let at = LOADER.find(late).unwrap();
            assert!(at > click, "{late}");
            assert_eq!(LOADER.matches(late).count(), 1, "{late}");
        }
        assert!(LOADER.contains("hb.addEventListener(\"click\""));
        // Stopping stops the camera.
        assert!(LOADER.contains("for(const t of s.stream.getTracks())t.stop()"));
        // The frames stay in the page: the landmarker's points go to the
        // module, and nothing is sent anywhere.
        assert!(!LOADER.contains("XMLHttpRequest"));
        assert!(!LOADER.contains("sendBeacon"));
        assert!(!LOADER.contains("WebSocket"));
        assert!(LOADER.contains("g.hand_frame("));
        assert!(LOADER.contains("/lander9/graph/"));
        assert!(!LOADER.contains("</script"));
    }

    #[test]
    fn the_words_people_see_are_plain() {
        for text in [
            HAND_NOTE,
            "Starting your camera.",
            "Loading hand tracking.",
            "Hold up a hand.",
            "Camera access is off for this page, so hand tracking can't start.",
            "We couldn't find a camera. The mouse still works.",
            "We couldn't start hand tracking here. The mouse still works.",
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
            assert!(LOADER.contains(text) || MAIN.contains(text), "{text}");
        }
    }
}
