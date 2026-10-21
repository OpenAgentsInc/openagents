//! `/lander10`: one page with the real chat and the live pylons.
//!
//! It is `/lander5`'s one response with the Grid held still and drawn in
//! black and white (the camera `/pylons` uses, facing straight down the
//! Grid), the mark and name in the corner, and three things that load
//! after the Grid's first full frame:
//!
//! - `/lander8`'s chat (`apps/openagents/web/lander-chat`, its builds under
//!   `/lander8/build/`): the composer at the bottom and the conversation in
//!   a card that grows as it goes. Before the first message the page shows
//!   only the composer. A chat's address is `/lander10/chat/{id}`.
//! - `/pylons`' Grid layer (`crates/pylons-web`, under `/pylons/layer/`): a
//!   glowing pillar per pylon online now, fed by `/pylons/live` (the
//!   server's one relay subscription, every event checked by
//!   `nostr::pylon`), rising and fading as pylons come and go and pulsing on
//!   receipts. The chat publishes where its card and composer are (the
//!   `#lander-chat-keep` box, marked `data-pylons-keep`), and the pillars
//!   stand clear of them, moving up and to the sides as the card grows.
//! - The pylon card (`apps/openagents/web/pylon-card`, under
//!   `/pylons/card/`), loaded on the first click on a pillar and shown as
//!   an overlay above the composer. Escape or a click outside closes it.
//!
//! The list of online pylons is HTML in the response, visually hidden: it
//! is what screen readers read, and the layer keeps it current.
//!
//! Both GPUI modules draw into the element named `gpui-mount` when they
//! start: the loader renames the chat's box once the chat is drawn, and
//! names the card's box only when the card starts.

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use super::lander5::{Addition, BuildKey, Document, assemble_page, build, build_key, send};
use super::pylons_field::{field, list_html};
use crate::App;

/// The page.
pub(crate) const LANDER10_PATH: &str = "/lander10";

const TITLE: &str = "OpenAgents";
const DESCRIPTION: &str = "Chat with OpenAgents, with every pylon online now around you.";

/// The Grid's stand-in, the corner name, the chat's box, the pillars, and
/// the pylon card's box. The pillars' look is `/pylons`'.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0c0c0c 22%,#151515 36%,#262626 41.6%,#0f0f0f 42.2%,#0b0b0b 60%,#0a0a0a 100%);\
color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
h1{position:fixed;z-index:4;top:calc(18px + env(safe-area-inset-top));left:calc(18px + env(safe-area-inset-left));\
margin:0;display:flex;align-items:center;gap:10px;font-size:15px;font-weight:600;line-height:20px;letter-spacing:0;\
pointer-events:none;text-shadow:0 1px 12px rgba(0,0,0,.5)}\
h1 svg{width:18px;height:18px;flex:none}\
.sr{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
#pylons-panel button{all:unset}\
#lander-chat-keep{position:fixed;left:0;top:100%;width:0;height:0;pointer-events:none;visibility:hidden}\
.chat-mount{position:fixed;z-index:1;top:env(safe-area-inset-top);right:env(safe-area-inset-right);\
bottom:env(safe-area-inset-bottom);left:env(safe-area-inset-left);\
opacity:0;visibility:hidden;transform:translateY(8px);\
transition:opacity .5s cubic-bezier(.22,.8,.3,1),transform .5s cubic-bezier(.22,.8,.3,1),visibility 0s .5s}\
.chat-mount canvas{position:absolute}\
.chat-mount.ready{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
#lander10-fallback{position:fixed;z-index:2;left:50%;bottom:calc(32px + env(safe-area-inset-bottom));transform:translateX(-50%);\
padding:13px 26px;border-radius:999px;background:#ededed;color:#0a0a0a;font-size:15px;font-weight:600;text-decoration:none}\
#lander10-fallback[hidden]{display:none}\
#pylons-field{position:fixed;inset:0;z-index:3;pointer-events:none;overflow:hidden}\
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
#pylon-card{position:fixed;z-index:5;top:calc(60px + env(safe-area-inset-top));right:calc(16px + env(safe-area-inset-right));\
bottom:calc(112px + env(safe-area-inset-bottom));width:min(400px,calc(100vw - 32px));max-height:560px;\
border-radius:16px;overflow:hidden;background:rgba(8,8,8,.82);border:1px solid rgba(255,255,255,.12);\
-webkit-backdrop-filter:blur(18px);backdrop-filter:blur(18px);box-shadow:0 24px 90px rgba(0,0,0,.65);\
opacity:0;visibility:hidden;transform:translateX(14px);\
transition:opacity .3s cubic-bezier(.22,.8,.3,1),transform .3s cubic-bezier(.22,.8,.3,1),visibility 0s .3s}\
#pylon-card.open{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
.card-mount{position:absolute;inset:0;opacity:0;transition:opacity .25s}.card-mount canvas{position:absolute}\
#pylon-card.ready .card-mount{opacity:1}\
#pylon-facts{position:absolute;inset:0;overflow:auto;padding:18px 20px;box-sizing:border-box;font-size:14px;line-height:20px}\
#pylon-card.ready #pylon-facts{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
#pylon-facts h2{margin:0 40px 2px 0;font-size:17px;font-weight:600}\
#pylon-facts dl{display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:14px 0 0}\
#pylon-facts dt{color:#8f8f8f}#pylon-facts dd{margin:0;color:#ededed;word-break:break-all}\
#pylon-close{all:unset;position:absolute;z-index:1;top:10px;right:10px;width:32px;height:32px;border-radius:8px;\
display:grid;place-items:center;cursor:pointer;color:#bdbdbd;font-size:20px;line-height:1}\
#pylon-close:hover{background:rgba(255,255,255,.08);color:#fff}#pylon-close:focus-visible{outline:2px solid #fff}\
@media (max-width:640px){.pylon .tag{font-size:11px;line-height:14px}.pylon .tag small{font-size:10px}\
.pylon .tag b{max-width:110px}.pylon .slots i{width:5px;height:5px}\
#pylon-card{top:calc(52px + env(safe-area-inset-top));left:calc(12px + env(safe-area-inset-left));right:calc(12px + env(safe-area-inset-right));\
bottom:auto;width:auto;height:min(52vh,460px)}}\
@media (prefers-reduced-motion:reduce){.pylon,.pylon .beam,#pylon-card,.chat-mount{transition-duration:.15s}\
.pylon .base{animation:none}.pylon.pa .ring,.pylon.pb .ring{animation-duration:.6s}#pylon-card,.chat-mount{transform:none}}";

/// The mark and the name, as `/lander8` draws them.
const HEADING: &str = "<h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>";

/// After the list: the pillars, the live announcements, the box the chat
/// says it covers, the chat's box, the pylon card, and the link shown when
/// the chat can't start.
const AFTER_LIST: &str = "<div id=\"pylons-field\" aria-hidden=\"true\"></div>\
<p id=\"pylons-said\" class=\"sr\" aria-live=\"polite\"></p>\
<div id=\"lander-chat-keep\" data-pylons-keep></div>\
<div id=\"gpui-mount\" class=\"chat-mount\" role=\"region\" aria-label=\"Chat with OpenAgents\"></div>\
<div id=\"pylon-card\" role=\"dialog\" aria-labelledby=\"pylon-facts-name\">\
<div id=\"pylon-card-mount\" class=\"card-mount\" aria-hidden=\"true\"></div>\
<div id=\"pylon-facts\"><h2 id=\"pylon-facts-name\"></h2><dl></dl></div>\
<button id=\"pylon-close\" type=\"button\" aria-label=\"Close\">&times;</button></div>\
<a id=\"lander10-fallback\" href=\"/\" hidden>Chat with OpenAgents</a>";

/// The loader. After the Grid's first full frame it hands the Grid to a
/// worker (as `/lander6` does), then in idle moments starts the pillars'
/// layer (small) and the chat: the chat's build is compiled as it streams,
/// instantiated, and mounted in separate idle slices, and fades in on its
/// first frame, after which its box gives up the `gpui-mount` name. The
/// layer calls `pylonsCard.show(json)` and `pylonsCard.hide()`; the first
/// `show` loads the card's build, names the card's box `gpui-mount`, and
/// mounts it. GPUI focuses a text field of its own when it starts: on a
/// touch screen focus goes back where it was after the chat starts, and
/// the card's field is hidden from the keyboard.
const LOADER: &str = "\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const touch=matchMedia(\"(pointer: coarse)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
const chatBox=document.getElementById(\"gpui-mount\");\
const box=document.getElementById(\"pylon-card\"),cardMount=document.getElementById(\"pylon-card-mount\");\
let handed=null;\
const handoff=()=>handed||(handed=new Promise(done=>{\
const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{\
const grid=document.createElement(\"canvas\");grid.id=\"lander10-grid\";grid.setAttribute(\"aria-hidden\",\"true\");main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");runner.pause();done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,\
grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.lander10GridWorker=worker}).catch(done)}));\
const variant=(async()=>{try{return navigator.gpu&&await navigator.gpu.requestAdapter()?\"webgpu\":\"webgl\"}catch(e){return \"webgl\"}})();\
let chatDone;const chatSettled=new Promise(r=>chatDone=r);\
const chatFail=()=>{chatBox.remove();document.getElementById(\"lander10-fallback\").hidden=false;chatDone()};\
const loadChat=async()=>{try{\
performance.mark(\"card-start\");\
const dir=\"/lander8/build/\"+await variant+\"/\";\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(dir+\"openagents_web_lander_chat_bg.wasm\",{priority:\"low\"})),\
import(dir+\"openagents_web_lander_chat.js\")]);\
performance.mark(\"card-compiled\");\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"card-instantiated\");\
await idle(600);\
const before=document.activeElement;\
chatBox.addEventListener(\"gpui-failed\",chatFail,{once:true});\
chatBox.addEventListener(\"gpui-first-frame\",()=>{\
performance.mark(\"card-first-frame\");\
chatBox.classList.add(\"ready\");chatBox.id=\"lander10-chat\";\
setTimeout(()=>{\
for(const field of document.querySelectorAll(\"body>input\"))field.setAttribute(\"aria-label\",\"Message OpenAgents\");\
const now=document.activeElement;\
if(touch&&now&&now.tagName===\"INPUT\"){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}\
chatDone()})},{once:true});\
glue.mount(still)}catch(e){chatFail()}};\
const loadLayer=async()=>{try{\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(\"/pylons/layer/pylons_web_bg.wasm\")),\
import(\"/pylons/layer/pylons_web.js\")]);\
await glue.default({module_or_path:module});glue.start(still);performance.mark(\"pylons-layer\")}catch(e){}};\
let card=null,latest=\"\";\
const loadCard=()=>card||(card=(async()=>{\
await chatSettled;\
const dir=\"/pylons/card/\"+await variant+\"/\";\
const [module,glue]=await Promise.all([\
WebAssembly.compileStreaming(fetch(dir+\"openagents_web_pylon_card_bg.wasm\")),\
import(dir+\"openagents_web_pylon_card.js\")]);\
await idle(300);\
await glue.default({module_or_path:module});\
glue.show(latest);\
const fields=new Set(document.querySelectorAll(\"body>input\"));\
const before=document.activeElement;\
await new Promise((ok,no)=>{\
cardMount.id=\"gpui-mount\";\
cardMount.addEventListener(\"gpui-failed\",no,{once:true});\
cardMount.addEventListener(\"gpui-first-frame\",()=>{box.classList.add(\"ready\");cardMount.id=\"pylon-card-mount\";\
performance.mark(\"pylon-card-first-frame\");\
setTimeout(()=>{for(const field of document.querySelectorAll(\"body>input\"))if(!fields.has(field)){field.tabIndex=-1;field.setAttribute(\"aria-hidden\",\"true\")}\
const now=document.activeElement;if(now&&now.tagName===\"INPUT\"&&!fields.has(now)){if(before&&before!==document.body)before.focus({preventScroll:true});else now.blur()}});ok()},{once:true});\
glue.mount(still)});\
return glue})());\
globalThis.pylonsCard={\
show(json){latest=json;box.classList.add(\"open\");loadCard().then(glue=>glue.show(latest)).catch(()=>{box.classList.remove(\"ready\")})},\
hide(){box.classList.remove(\"open\")}};\
let started=false;\
const go=async()=>{if(started)return;started=true;await handoff();await idle(400);loadLayer();loadChat()};\
if(performance.getEntriesByName(\"lander5-presented\").length)go();\
else{new PerformanceObserver((list,observer)=>{\
if(list.getEntriesByName(\"lander5-presented\").length){observer.disconnect();go()}}).observe({type:\"mark\"});\
addEventListener(\"load\",()=>setTimeout(go,1500))}";

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: "",
    script: LOADER,
    runner_tail: super::lander6::RUNNER_TAIL,
    main: None,
    grid: Some(super::pylons::still_gray_grid),
    connect: "",
};

/// The last document made: the runner build and the list it came from.
static KEPT: Mutex<Option<(BuildKey, String, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(LANDER10_PATH, get(page))
        .route("/lander10/chat/{id}", get(chat_page))
}

/// `<main>` for this response: the heading, the list as it stands
/// (visually hidden), and the rest.
fn main_html(list: &str) -> String {
    format!(
        "{HEADING}<section id=\"pylons-panel\" class=\"sr\" aria-labelledby=\"pylons-count\">{list}</section>{AFTER_LIST}"
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
const UNAVAILABLE: &str = "<h2 id=\"pylons-count\">Pylons</h2><ul id=\"pylons-list\"></ul>";

async fn page(State(app): State<App>, request: HeaderMap) -> Response {
    let list = match app.config.pylons_relay.as_deref() {
        Some(relay) => list_html(&field(relay).current()),
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
    fn the_page_has_the_chat_and_the_pylons_and_no_welcome() {
        let list = "<h2 id=\"pylons-count\">1 pylon online</h2><ul id=\"pylons-list\"></ul>";
        let document = assemble_page(
            None,
            Some(&ADDITION),
            Some(&main_html(list)),
            (TITLE, DESCRIPTION),
        );
        assert!(document.html.contains("</svg>OpenAgents</h1>"));
        assert!(document.html.contains(list));
        assert!(
            document
                .html
                .contains("class=\"sr\" aria-labelledby=\"pylons-count\"")
        );
        assert!(document.html.contains("data-pylons-keep"));
        assert!(!document.html.contains("What can we help with"));
        assert!(!document.html.contains("href=\"/download\""));
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
        for copy in [AFTER_LIST, HEADING, TITLE, DESCRIPTION, UNAVAILABLE] {
            assert!(oa_copy::violations(copy, &[]).is_empty(), "{copy}");
        }
    }

    #[test]
    fn the_loader_starts_the_layer_and_chat_and_the_card_on_first_open() {
        assert!(LOADER.contains("lander5-presented"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("/pylons/layer/pylons_web_bg.wasm"));
        assert!(LOADER.contains("openagents_web_lander_chat_bg.wasm"));
        assert!(LOADER.contains("globalThis.pylonsCard"));
        assert!(LOADER.contains(super::super::pylons::CARD_WASM));
        // The card waits for the chat, then takes the mount's name.
        assert!(LOADER.contains("await chatSettled"));
        assert!(LOADER.contains("cardMount.id=\\\"gpui-mount\\\"".replace('\\', "").as_str()));
        assert!(!LOADER.contains("</script"));
        assert!(STYLE.contains("prefers-reduced-motion:reduce"));
    }
}
