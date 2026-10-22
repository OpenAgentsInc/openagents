//! `/pylons`: every pylon that is online now, standing on the Grid.
//!
//! The page is `/lander5`'s one response (the same runner and shaders,
//! assembled by the same code) with the grid pass in black and white and
//! its camera held still, so the pylons can stand on the floor. The same
//! response carries the list of online pylons as HTML (name, status,
//! class, and busy slots), made from the server's live field
//! ([`super::pylons_field`]) at the moment of the request: that list is
//! what screen readers, a browser without script, and one without WebGL
//! read.
//!
//! After the Grid's first full frame the loader fetches the Grid layer
//! (`crates/pylons-web`, `scripts/build-pylons-web.sh`), which opens the
//! page's event stream (`/pylons/live`) and keeps the list and a glowing
//! pillar per pylon current: a pillar rises when a beacon arrives, fades
//! when the pylon goes offline or its beacon expires, and pulses when a
//! receipt for one of its jobs arrives. Clicking a pillar or a list entry
//! opens the pylon's card: GPUI's chat views drawing its details
//! (`apps/openagents/web/pylon-card`,
//! `apps/openagents/scripts/build-web-pylon-card.sh`), loaded on the first
//! open after the loader hands the Grid to a worker as `/lander6` does. If
//! the card can't start, the same details show as HTML.
//!
//! Builds are read from `--pylons DIR` (default `/srv/pylons`):
//!
//! ```text
//! DIR/layer/pylons_web.js, pylons_web_bg.wasm        the Grid layer
//! DIR/card/{webgpu,webgl}/openagents_web_pylon_card.js, _bg.wasm
//! DIR/**/*.br, DIR/**/*.gz                           brotli and gzip copies
//! ```
//!
//! **Live updates.** The server holds one relay subscription and streams
//! snapshots to every page over server-sent events, rather than each
//! browser opening its own relay socket: the server verifies every event
//! with `crates/nostr` natively (no secp256k1 in the page's module), the
//! first response already lists the pylons online, a visitor's network
//! never needs to reach the relay, the policy keeps `connect-src 'self'`,
//! and the relay sees one reader per server instead of one per visitor.
//! `EventSource` reconnects by itself; the layer also drops a pylon whose
//! beacon has expired by its own clock if the stream goes quiet.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use super::lander5::{Addition, BuildKey, Document, assemble_page, build, build_key, send};
use super::pylons_field::{field, list_html};
use crate::App;

/// The page.
pub(crate) const PYLONS_PATH: &str = "/pylons";
/// The page's event stream of snapshots.
pub(crate) const PYLONS_LIVE_PATH: &str = "/pylons/live";

/// The Grid layer's files, as `scripts/build-pylons-web.sh` names them.
pub(crate) const LAYER_GLUE: &str = "pylons_web.js";
pub(crate) const LAYER_WASM: &str = "pylons_web_bg.wasm";
/// The card's two builds and their files, as `build-web-pylon-card.sh`
/// names them.
pub(crate) const CARD_VARIANTS: [&str; 2] = ["webgpu", "webgl"];
pub(crate) const CARD_GLUE: &str = "openagents_web_pylon_card.js";
pub(crate) const CARD_WASM: &str = "openagents_web_pylon_card_bg.wasm";

/// Where the camera is held: the grid pass's camera at this many seconds,
/// where the Grid layer projects the pylons from.
pub(crate) const CAMERA_TIME: f32 = pylons_web::camera::CAMERA_TIME;

const TITLE: &str = "Pylons online";
const DESCRIPTION: &str = "Every OpenAgents pylon that is online now, live.";

/// Black, white, and gray: the Grid's stand-in until its first frame, the
/// corner name, the list panel, the pillars the layer adds, and the card's
/// box, hidden until opened.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0c0c0c 22%,#151515 36%,#262626 41.6%,#0f0f0f 42.2%,#0b0b0b 60%,#0a0a0a 100%);\
color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
h1{position:fixed;z-index:3;top:calc(18px + env(safe-area-inset-top));left:calc(18px + env(safe-area-inset-left));\
margin:0;display:flex;align-items:center;gap:10px;font-size:15px;font-weight:600;line-height:20px;\
pointer-events:none;text-shadow:0 1px 12px rgba(0,0,0,.6)}\
h1 svg{width:18px;height:18px;flex:none}h1 span{color:#8f8f8f;font-weight:500}\
.sr{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
#pylons-panel{position:fixed;z-index:3;left:calc(16px + env(safe-area-inset-left));bottom:calc(16px + env(safe-area-inset-bottom));\
box-sizing:border-box;width:min(340px,calc(100vw - 32px));max-height:min(40vh,320px);overflow:auto;padding:12px 14px;\
border-radius:14px;background:rgba(10,10,10,.62);border:1px solid rgba(255,255,255,.1);\
-webkit-backdrop-filter:blur(14px);backdrop-filter:blur(14px)}\
#pylons-panel h2{margin:0 0 6px;font-size:13px;font-weight:600;color:#f2f2f2}\
#pylons-panel ul{list-style:none;margin:0;padding:0}\
#pylons-panel li{display:flex;flex-direction:column;padding:6px 0;border-top:1px solid rgba(255,255,255,.07);font-size:13px;line-height:18px}\
#pylons-panel li:first-child{border-top:0}\
#pylons-panel .n{color:#f2f2f2;font-weight:500}#pylons-panel .d{color:#9a9a9a;font-size:12px}\
#pylons-panel p{margin:6px 0 0;font-size:12px;color:#9a9a9a}\
#pylons-panel button{all:unset;cursor:pointer;color:#f2f2f2;font-weight:500;border-radius:4px}\
#pylons-panel button:hover{text-decoration:underline}\
#pylons-panel button:focus-visible{outline:2px solid #fff;outline-offset:2px}\
#pylons-field{position:fixed;inset:0;z-index:2;pointer-events:none;overflow:hidden}\
.pylon{all:unset;position:absolute;left:0;top:0;box-sizing:border-box;cursor:pointer;pointer-events:auto;\
width:var(--hw);height:var(--h);transform:translate3d(calc(var(--x) - var(--hw) / 2),calc(var(--y) - var(--h)),0);\
opacity:0;transition:opacity .7s ease,transform .6s cubic-bezier(.2,.8,.2,1)}\
.pylon.in{opacity:1}.pylon.out,.pylon.hid{opacity:0;pointer-events:none}\
.pylon .beam{position:absolute;left:50%;bottom:0;width:var(--bw);height:100%;margin-left:calc(var(--bw) / -2);\
transform:scaleY(0);transform-origin:50% 100%;border-radius:999px 999px 2px 2px;\
background:linear-gradient(to top,#fff 0%,rgba(255,255,255,.9) 35%,rgba(255,255,255,0) 100%);\
box-shadow:0 0 calc(var(--bw) * 3) calc(var(--bw) / 2) rgba(255,255,255,.28);\
transition:transform 1s cubic-bezier(.2,.8,.2,1)}\
.pylon.in .beam{transform:scaleY(1)}.pylon.out .beam{transform:scaleY(0);transition-duration:.7s}\
.pylon .base,.pylon .ring{position:absolute;left:50%;bottom:0;width:calc(var(--bw) * 9);height:calc(var(--bw) * 2.6);\
transform:translate(-50%,50%);border-radius:50%}\
.pylon .base{background:radial-gradient(closest-side,rgba(255,255,255,.95),rgba(255,255,255,.3) 55%,rgba(255,255,255,0));\
animation:breathe 4s ease-in-out infinite}\
.pylon .ring{border:1px solid rgba(255,255,255,.85);opacity:0}\
.pylon.pa .ring{animation:pulse-a 1.6s cubic-bezier(.2,.7,.3,1)}.pylon.pb .ring{animation:pulse-b 1.6s cubic-bezier(.2,.7,.3,1)}\
.pylon.pa .beam,.pylon.pb .beam{box-shadow:0 0 calc(var(--bw) * 6) var(--bw) rgba(255,255,255,.5)}\
@keyframes pulse-a{from{opacity:1;transform:translate(-50%,50%) scale(1)}to{opacity:0;transform:translate(-50%,50%) scale(4.5)}}\
@keyframes pulse-b{from{opacity:1;transform:translate(-50%,50%) scale(1)}to{opacity:0;transform:translate(-50%,50%) scale(4.5)}}\
@keyframes breathe{50%{opacity:.62}}\
.pylon.draining .beam{opacity:.42;background:repeating-linear-gradient(to top,#fff 0 6px,rgba(255,255,255,0) 6px 10px)}\
.pylon .tag{position:absolute;left:50%;bottom:calc(100% + 8px);transform:translateX(-50%) scale(var(--s));transform-origin:50% 100%;\
white-space:nowrap;text-align:center;font-size:13px;font-weight:600;line-height:17px;color:#f5f5f5;\
text-shadow:0 1px 10px rgba(0,0,0,.85),0 0 2px rgba(0,0,0,.9)}\
.pylon .tag b{display:block;max-width:156px;margin:0 auto;overflow:hidden;text-overflow:ellipsis;font-weight:600}\
.pylon .tag small{display:block;font-size:11px;font-weight:400;color:#b0b0b0}\
.pylon .slots{display:flex;justify-content:center;gap:3px;margin-top:3px}\
.pylon .slots i{width:7px;height:7px;border:1px solid rgba(255,255,255,.7);border-radius:1px}.pylon .slots i.b{background:#fff}\
.pylon:hover .tag,.pylon.sel .tag{color:#fff}.pylon.sel .base{filter:brightness(1.4)}\
.pylon:focus-visible{outline:2px solid #fff;outline-offset:4px}\
#pylon-card{position:fixed;z-index:4;top:calc(56px + env(safe-area-inset-top));right:calc(16px + env(safe-area-inset-right));\
bottom:calc(16px + env(safe-area-inset-bottom));width:min(400px,calc(100vw - 32px));max-height:620px;\
border-radius:16px;overflow:hidden;background:rgba(8,8,8,.78);border:1px solid rgba(255,255,255,.12);\
-webkit-backdrop-filter:blur(18px);backdrop-filter:blur(18px);box-shadow:0 24px 90px rgba(0,0,0,.65);\
opacity:0;visibility:hidden;transform:translateX(14px);\
transition:opacity .3s cubic-bezier(.22,.8,.3,1),transform .3s cubic-bezier(.22,.8,.3,1),visibility 0s .3s}\
#pylon-card.open{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
#gpui-mount{position:absolute;inset:0;opacity:0;transition:opacity .25s}#gpui-mount canvas{position:absolute}\
#pylon-card.ready #gpui-mount{opacity:1}\
#pylon-facts{position:absolute;inset:0;overflow:auto;padding:18px 20px;box-sizing:border-box;font-size:14px;line-height:20px}\
#pylon-work-facts{white-space:pre-wrap}\
#pylon-card.ready #pylon-facts{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
#pylon-facts h2{margin:0 40px 2px 0;font-size:17px;font-weight:600}\
#pylon-facts dl{display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:14px 0 0}\
#pylon-facts dt{color:#8f8f8f}#pylon-facts dd{margin:0;color:#ededed;word-break:break-all}\
#pylon-close{all:unset;position:absolute;z-index:1;top:10px;right:10px;width:32px;height:32px;border-radius:8px;\
display:grid;place-items:center;cursor:pointer;color:#bdbdbd;font-size:20px;line-height:1}\
#pylon-close:hover{background:rgba(255,255,255,.08);color:#fff}#pylon-close:focus-visible{outline:2px solid #fff}\
@media (max-width:640px){#pylons-panel{max-height:26vh}\
#pylon-card{top:auto;left:calc(16px + env(safe-area-inset-left));width:auto;height:min(64vh,560px)}}\
@media (prefers-reduced-motion:reduce){.pylon,.pylon .beam,#pylon-card{transition-duration:.15s}\
.pylon .base{animation:none}.pylon.pa .ring,.pylon.pb .ring{animation-duration:.6s}#pylon-card{transform:none}}";

/// The mark and the page's name in the corner, as `/lander7` draws them.
const HEADING: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents <span>Pylons</span></h1>";

/// The pillars' layer, the card's box (its HTML details for screen readers
/// and for when GPUI can't start), and the live announcements.
const AFTER_LIST: &str = "<div id=\"pylons-field\" aria-hidden=\"true\"></div>\
<p id=\"pylons-said\" class=\"sr\" aria-live=\"polite\"></p>\
<div id=\"pylon-card\" role=\"dialog\" aria-labelledby=\"pylon-facts-name\">\
<div id=\"gpui-mount\" aria-hidden=\"true\"></div>\
<div id=\"pylon-facts\"><h2 id=\"pylon-facts-name\"></h2><dl></dl></div>\
<button id=\"pylon-close\" type=\"button\" aria-label=\"Close\">&times;</button></div>";

/// The loader. After the Grid's first full frame and an idle moment it
/// loads the Grid layer and starts it; the layer then calls
/// `pylonsCard.show(json)` and `pylonsCard.hide()`. The first `show` hands
/// the Grid to a worker (as `/lander6` does, so GPUI's main-thread work
/// can't hold up its frames), compiles the card's build for this browser
/// as it streams, hands it the pylon, and mounts it; the box shows the
/// HTML details until GPUI's first frame, and keeps them if the card
/// fails. GPUI focuses a text field of its own when it starts; the card
/// takes no typing, so that field is hidden and focus goes back where it
/// was.
const LOADER: &str = "\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
const box=document.getElementById(\"pylon-card\"),mount=document.getElementById(\"gpui-mount\");\
const handoff=()=>new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"pylons-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.pylonsGridWorker=worker}).catch(done)});\
let card=null,latest=\"\";\
const loadCard=()=>card||(card=(async()=>{\
await handoff();\
let gpu=false;try{gpu=!!(navigator.gpu&&await navigator.gpu.requestAdapter())}catch(e){}\
const dir=\"/pylons/card/\"+(gpu?\"webgpu\":\"webgl\")+\"/\";\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(dir+\"openagents_web_pylon_card_bg.wasm\")),\
import(dir+\"openagents_web_pylon_card.js\")]);\
await idle(300);\
await glue.default({module_or_path:module});\
glue.show(latest);\
await idle(300);\
const before=document.activeElement;\
await new Promise((ok,no)=>{\
mount.addEventListener(\"gpui-failed\",no,{once:true});\
mount.addEventListener(\"gpui-first-frame\",()=>{box.classList.add(\"ready\");\
setTimeout(()=>{for(const field of document.querySelectorAll(\"body>input\")){field.tabIndex=-1;field.setAttribute(\"aria-hidden\",\"true\")}\
const now=document.activeElement;if(now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}});ok()},{once:true});\
glue.mount(still)});\
return glue})());\
globalThis.pylonsCard={\
show(json){latest=json;box.classList.add(\"open\");loadCard().then(glue=>glue.show(latest)).catch(()=>{box.classList.remove(\"ready\")})},\
hide(){box.classList.remove(\"open\")}};\
let started=false;\
const go=async()=>{if(started)return;started=true;await idle(400);try{\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(\"/pylons/layer/pylons_web_bg.wasm\")),\
import(\"/pylons/layer/pylons_web.js\")]);\
await glue.default({module_or_path:module});glue.start(still)}catch(e){}};\
if(performance.getEntriesByName(\"lander5-presented\").length)go();\
else{new PerformanceObserver((list,observer)=>{\
if(list.getEntriesByName(\"lander5-presented\").length){observer.disconnect();go()}}).observe({type:\"mark\"});\
addEventListener(\"load\",()=>setTimeout(go,1200))}";

pub(super) fn still_gray_grid() -> String {
    lander5_web::shaders::still_gray_grid_source(CAMERA_TIME)
}

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: "",
    script: LOADER,
    runner_tail: super::lander6::RUNNER_TAIL,
    main: None,
    grid: Some(still_gray_grid),
    connect: "",
    workers: "",
};

/// The last document made: the runner build and the list it came from.
static KEPT: Mutex<Option<(BuildKey, String, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PYLONS_PATH, get(page))
        .route(PYLONS_LIVE_PATH, get(live))
        .route("/pylons/layer/{file}", get(layer_file))
        .route("/pylons/card/{variant}/{file}", get(card_file))
}

/// `<main>` for this response: the heading, the list as it stands, and
/// the layer's elements.
fn main_html(list: &str) -> String {
    format!(
        "{HEADING}<section id=\"pylons-panel\" aria-labelledby=\"pylons-count\">{list}</section>{AFTER_LIST}"
    )
}

fn document(directory: Option<&Path>, list: String) -> Arc<Document> {
    let key = build_key(directory);
    if let Ok(kept) = KEPT.lock()
        && let Some((kept_key, kept_list, document)) = kept.as_ref()
        && *kept_key == key
        && *kept_list == list
    {
        return document.clone();
    }
    let document = Arc::new(assemble_page(
        build(directory),
        Some(&ADDITION),
        Some(&main_html(&list)),
        (TITLE, DESCRIPTION),
    ));
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, list, document.clone()));
    }
    document
}

/// The list without a live field: the server was started with no relay.
const UNAVAILABLE: &str = "<h2 id=\"pylons-count\">Pylons</h2><ul id=\"pylons-list\"></ul>\
<p>Live pylons can't be shown here right now.</p>";

async fn page(State(app): State<App>, request: HeaderMap) -> Response {
    let list = match app.config.pylons_relay.as_deref() {
        Some(relay) => {
            let snapshot = field(relay).current();
            // The list from the content manifest's handler component when
            // there is one (#12435), else rendered here.
            let shared = snapshot.clone();
            let data: livehost::site::DataSource = std::sync::Arc::new(move |name: &str| {
                (name == "pylons-snapshot")
                    .then(|| serde_json::to_string(&*shared).ok())
                    .flatten()
            });
            match crate::live_content::render("pylons-list", PYLONS_PATH, data).await {
                Some(list) => list,
                None => list_html(&snapshot),
            }
        }
        None => UNAVAILABLE.to_owned(),
    };
    // The runner built with its worker exports, as /lander6 serves it.
    let directory = app.config.lander5.as_deref().map(|directory| {
        let worker = directory.join("worker");
        if worker.join(super::lander5::LANDER5_WASM).is_file() {
            worker
        } else {
            directory.to_path_buf()
        }
    });
    let fallback = list.clone();
    let document = tokio::task::spawn_blocking(move || document(directory.as_deref(), list))
        .await
        .unwrap_or_else(|_| {
            Arc::new(assemble_page(
                None,
                Some(&ADDITION),
                Some(&main_html(&fallback)),
                (TITLE, DESCRIPTION),
            ))
        });
    let mut response = send(&document, &request);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

/// `GET /pylons/live`: the online pylons as `pylons` events, one at once
/// and one each time the snapshot changes. Public data only; no sign-in.
/// A stream ends after ten minutes, and `EventSource` reconnects.
async fn live(State(app): State<App>) -> Response {
    let Some(relay) = app.config.pylons_relay.clone() else {
        return crate::not_found().await;
    };
    let receiver = field(&relay).subscribe();
    let started = tokio::time::Instant::now();
    let updates = futures_util::stream::unfold(
        (receiver, true, started),
        |(mut receiver, first, started)| async move {
            if !first {
                let left = Duration::from_secs(600).saturating_sub(started.elapsed());
                match tokio::time::timeout(left, receiver.changed()).await {
                    Ok(Ok(())) => {}
                    _ => return None,
                }
            }
            let snapshot = receiver.borrow_and_update().clone();
            let data = serde_json::to_string(&*snapshot).unwrap_or_else(|_| "{}".to_owned());
            let event = axum::response::sse::Event::default()
                .event("pylons")
                .data(data);
            Some((
                Ok::<_, std::convert::Infallible>(event),
                (receiver, false, started),
            ))
        },
    );
    let updates = app.config.shutdown.until(updates);
    let mut response = axum::response::Sse::new(updates)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// One of the Grid layer's two files, brotli or gzip when accepted.
async fn layer_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        LAYER_GLUE => "text/javascript; charset=utf-8",
        LAYER_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.pylons.as_deref() else {
        return crate::not_found().await;
    };
    super::everglade::serve_build(&root.join("layer"), &file, content_type, &request).await
}

/// One of the card's files, brotli or gzip when accepted. Only the two
/// builds' glue and module are served.
async fn card_file(
    State(app): State<App>,
    UrlPath((variant, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        CARD_GLUE => "text/javascript; charset=utf-8",
        CARD_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.pylons.as_deref() else {
        return crate::not_found().await;
    };
    if !CARD_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join("card").join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_is_the_still_gray_grid_with_the_list_in_it() {
        let list = "<h2 id=\"pylons-count\">1 pylon online</h2><ul id=\"pylons-list\">\
<li data-id=\"a\"><span class=\"n\">Mac</span> <span class=\"d\">Online · GPU · 0 of 2 busy</span></li></ul>";
        let document = assemble_page(
            Some((
                "let wasm;\nasync function __wbg_init(m) { return m; }\nexport { __wbg_init as default };\n"
                    .to_owned(),
                b"\0asm\x01\0\0\0".to_vec(),
            )),
            Some(&ADDITION),
            Some(&main_html(list)),
            (TITLE, DESCRIPTION),
        );
        assert!(document.html.contains("<title>Pylons online</title>"));
        assert!(document.html.contains(list));
        assert!(
            document
                .html
                .contains("OpenAgents <span>Pylons</span></h1>")
        );
        assert!(
            document
                .html
                .contains("id=\"pylons-said\" class=\"sr\" aria-live=\"polite\"")
        );
        assert!(!document.html.contains("href=\"/download\""));
        // The grid pass is held still and drawn in black and white.
        assert!(document.html.contains("#define CAMERA_TIME 0.0"));
        assert!(document.html.contains("vec4(vec3(l), 1.0)"));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
        // Without a runner the list is still there, with no script but the
        // loader (which then finds no Grid and still starts the layer).
        let plain = assemble_page(
            None,
            Some(&ADDITION),
            Some(&main_html(list)),
            (TITLE, DESCRIPTION),
        );
        assert!(plain.html.contains(list));
        assert!(!plain.html.contains("x-shader"));
    }

    #[test]
    fn the_loader_waits_for_the_grid_and_loads_the_card_on_first_open() {
        assert!(LOADER.contains("lander5-presented"));
        assert!(LOADER.contains("/pylons/layer/pylons_web_bg.wasm"));
        assert!(LOADER.contains("glue.start(still)"));
        assert!(LOADER.contains("globalThis.pylonsCard"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("WebAssembly.compileStreaming"));
        assert!(LOADER.contains("glue.mount(still)"));
        assert!(LOADER.contains(CARD_WASM) && LOADER.contains(CARD_GLUE));
        assert!(!LOADER.contains("</script"));
        assert!(STYLE.contains("prefers-reduced-motion:reduce"));
        for copy in [UNAVAILABLE, AFTER_LIST, HEADING, TITLE, DESCRIPTION] {
            assert!(oa_copy::violations(copy, &[]).is_empty(), "{copy}");
        }
    }
}
