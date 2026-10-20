//! `/home`: the Grid homepage. The production `/` route stays unchanged.
//!
//! One response paints the page: `/lander5`'s Grid shader inline (held
//! still, black and white, facing straight down the Grid, as `/pylons`
//! draws it), a top bar with the mark and the name and Log in and Sign up,
//! and the list of pylons online now as HTML, which is what screen readers
//! and a visitor without script read. Then, each in its own module and in
//! idle moments, so a slow computer keeps its frames:
//!
//! 1. the shader moves to a worker (as on `/lander6`);
//! 2. `/pylons`' layer (`/pylons/layer/`, about 60 KB) stands a pillar per
//!    pylon online, fed by `/pylons/live` (the server's one relay
//!    subscription, every event checked); a click opens the pylon's GPUI
//!    card (`/pylons/card/`, loaded on the first click);
//! 3. the Verse web build `/verse` serves (`/everglade/`), at low priority:
//!    on this page (its canvas carries `data-ambient`) it opens the Grid
//!    with its camera circling the plaza until WASD takes over, and joins no one;
//!    it cross-fades in behind the pillars and the shader stops.
//!
//! Approaching the shared terminal opens the GPUI lander chat. [`me`] (`GET /home/me`) tells the top
//! bar who is signed in, so the page itself stays one cached document:
//! signed in, the bar shows the person's name linking to
//! their profile, and Open chat.
//!
//! `/home` used to be the classic home page; that moved to
//! [`super::home::HOME`].

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use super::lander5::{Addition, BuildKey, Document, assemble_page, build, build_key, send};
use super::pylons_field::{field, list_html};
use crate::App;

/// The page.
pub(crate) const GRID_HOME_PATH: &str = "/home";
/// Who is signed in, for the top bar.
pub(crate) const GRID_HOME_ME: &str = "/home/me";

const TITLE: &str = "OpenAgents";
const DESCRIPTION: &str = "OpenAgents: agents, pylons, and the Grid where they meet.";

/// Where Log in and Sign up return to.
const BACK: &str = "%2Fhome";
/// The chat entry point respects the feature flags and saved UI choice.
const CHAT: &str = super::home::NEW;

/// The Grid's stand-in, the top bar, the pylons' list (visible without
/// script, read by screen readers with it), the pillars, the pylon card,
/// and Verse's canvas, hidden until its first frame. The pillars' look is
/// `/pylons`'.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0c0c0c 22%,#151515 36%,#262626 41.6%,#0f0f0f 42.2%,#0b0b0b 60%,#0a0a0a 100%);color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
#everglade-canvas{opacity:0;transition:opacity 1.2s ease;outline:none}\
#everglade-canvas.ready{opacity:1}\
.sr,.js #pylons-panel{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
header{position:fixed;z-index:4;top:0;left:0;right:0;display:flex;align-items:center;justify-content:space-between;padding:calc(14px + env(safe-area-inset-top)) calc(16px + env(safe-area-inset-right)) 0 calc(18px + env(safe-area-inset-left));pointer-events:none}\
h1{margin:0;display:flex;align-items:center;gap:10px;font-size:15px;font-weight:600;line-height:20px;letter-spacing:0;text-shadow:0 1px 12px rgba(0,0,0,.5)}\
h1 svg{width:18px;height:18px;flex:none}\
header nav{display:flex;align-items:center;gap:8px;pointer-events:auto}\
header nav a{display:flex;align-items:center;gap:8px;padding:6px 14px;border-radius:999px;color:#e5e5e5;font-size:13px;font-weight:500;line-height:18px;text-decoration:none;border:1px solid rgba(255,255,255,.16);background:rgba(10,10,10,.5);-webkit-backdrop-filter:blur(10px);backdrop-filter:blur(10px)}\
header nav a:hover{background:rgba(30,30,30,.7)}\
header nav a:focus-visible{outline:2px solid #fff;outline-offset:2px}\
header nav a.solid{background:#ededed;color:#0a0a0a;border-color:#ededed}\
header nav a.solid:hover{background:#fff}\
#pylons-panel{position:fixed;z-index:3;left:calc(16px + env(safe-area-inset-left));bottom:calc(16px + env(safe-area-inset-bottom));box-sizing:border-box;width:min(340px,calc(100vw - 32px));max-height:40vh;overflow:auto;padding:12px 14px;border-radius:14px;background:rgba(10,10,10,.62);border:1px solid rgba(255,255,255,.1)}\
#pylons-panel h2{margin:0 0 6px;font-size:13px;font-weight:600}\
#pylons-panel ul{list-style:none;margin:0;padding:0}\
#pylons-panel li{padding:4px 0;font-size:13px}\
#pylons-panel .d{display:block;color:#9a9a9a;font-size:12px}\
#pylons-panel button{all:unset}\
#pylons-field{position:fixed;inset:0;z-index:2;pointer-events:none;overflow:hidden}\
.pylon{all:unset;position:absolute;left:0;top:0;box-sizing:border-box;cursor:pointer;pointer-events:auto;width:var(--hw);height:var(--h);transform:translate3d(calc(var(--x) - var(--hw) / 2),calc(var(--y) - var(--h)),0);opacity:0;transition:opacity .7s ease,transform .6s cubic-bezier(.2,.8,.2,1)}\
.pylon.in{opacity:1}\
.pylon.out,.pylon.hid{opacity:0;pointer-events:none}\
.pylon .beam{position:absolute;left:50%;bottom:0;width:var(--bw);height:100%;margin-left:calc(var(--bw) / -2);transform:scaleY(0);transform-origin:50% 100%;border-radius:999px 999px 2px 2px;background:linear-gradient(to top,#fff 0%,rgba(255,255,255,.9) 35%,rgba(255,255,255,0) 100%);box-shadow:0 0 calc(var(--bw) * 3) calc(var(--bw) / 2) rgba(255,255,255,.28);transition:transform 1s cubic-bezier(.2,.8,.2,1)}\
.pylon.in .beam{transform:scaleY(1)}\
.pylon.out .beam{transform:scaleY(0);transition-duration:.7s}\
.pylon .base,.pylon .ring{position:absolute;left:50%;bottom:0;width:calc(var(--bw) * 9);height:calc(var(--bw) * 2.6);transform:translate(-50%,50%);border-radius:50%}\
.pylon .base{background:radial-gradient(closest-side,rgba(255,255,255,.95),rgba(255,255,255,.3) 55%,rgba(255,255,255,0));animation:breathe 4s ease-in-out infinite}\
.pylon .ring{border:1px solid rgba(255,255,255,.85);opacity:0}\
.pylon.pa .ring{animation:pulse-a 1.6s cubic-bezier(.2,.7,.3,1)}\
.pylon.pb .ring{animation:pulse-b 1.6s cubic-bezier(.2,.7,.3,1)}\
.pylon.pa .beam,.pylon.pb .beam{box-shadow:0 0 calc(var(--bw) * 6) var(--bw) rgba(255,255,255,.5)}\
@keyframes pulse-a{from{opacity:1;transform:translate(-50%,50%) scale(1)}\
to{opacity:0;transform:translate(-50%,50%) scale(4.5)}\
}\
@keyframes pulse-b{from{opacity:1;transform:translate(-50%,50%) scale(1)}\
to{opacity:0;transform:translate(-50%,50%) scale(4.5)}\
}\
@keyframes breathe{50%{opacity:.62}\
}\
.pylon.draining .beam{opacity:.42;background:repeating-linear-gradient(to top,#fff 0 6px,rgba(255,255,255,0) 6px 10px)}\
.pylon .tag{position:absolute;left:50%;bottom:calc(100% + 8px);transform:translateX(-50%) scale(var(--s));transform-origin:50% 100%;white-space:nowrap;text-align:center;font-size:13px;font-weight:600;line-height:17px;color:#f5f5f5;text-shadow:0 1px 10px rgba(0,0,0,.85),0 0 2px rgba(0,0,0,.9)}\
.pylon .tag b{display:block;max-width:156px;margin:0 auto;overflow:hidden;text-overflow:ellipsis;font-weight:600}\
.pylon .tag small{display:block;font-size:11px;font-weight:400;color:#b0b0b0}\
.pylon .slots{display:flex;justify-content:center;gap:3px;margin-top:3px}\
.pylon .slots i{width:7px;height:7px;border:1px solid rgba(255,255,255,.7);border-radius:1px}\
.pylon .slots i.b{background:#fff}\
.pylon:hover .tag,.pylon.sel .tag{color:#fff}\
.pylon.sel .base{filter:brightness(1.4)}\
.pylon:focus-visible{outline:2px solid #fff;outline-offset:4px}\
#pylon-card{position:fixed;z-index:5;top:calc(60px + env(safe-area-inset-top));right:calc(16px + env(safe-area-inset-right));bottom:calc(16px + env(safe-area-inset-bottom));width:min(400px,calc(100vw - 32px));max-height:600px;border-radius:16px;overflow:hidden;background:rgba(8,8,8,.82);border:1px solid rgba(255,255,255,.12);-webkit-backdrop-filter:blur(18px);backdrop-filter:blur(18px);box-shadow:0 24px 90px rgba(0,0,0,.65);opacity:0;visibility:hidden;transform:translateX(14px);transition:opacity .3s cubic-bezier(.22,.8,.3,1),transform .3s cubic-bezier(.22,.8,.3,1),visibility 0s .3s}\
#pylon-card.open{opacity:1;visibility:visible;transform:none;transition-delay:0s}\
.pylon-card-mount{position:absolute;inset:0;opacity:0;transition:opacity .25s}\
.pylon-card-mount canvas{position:absolute}\
#pylon-card.ready .pylon-card-mount{opacity:1}\
#pylon-facts{position:absolute;inset:0;overflow:auto;padding:18px 20px;box-sizing:border-box;font-size:14px;line-height:20px}\
#pylon-card.ready #pylon-facts{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0}\
#pylon-facts h2{margin:0 40px 2px 0;font-size:17px;font-weight:600}\
#pylon-facts dl{display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:14px 0 0}\
#pylon-facts dt{color:#8f8f8f}\
#pylon-facts dd{margin:0;color:#ededed;word-break:break-all}\
#pylon-close{all:unset;position:absolute;z-index:1;top:10px;right:10px;width:32px;height:32px;border-radius:8px;display:grid;place-items:center;cursor:pointer;color:#bdbdbd;font-size:20px;line-height:1}\
#pylon-close:hover{background:rgba(255,255,255,.08);color:#fff}\
#pylon-close:focus-visible{outline:2px solid #fff}\
@media (max-width:640px){.pylon .tag{font-size:11px;line-height:14px}\
.pylon .tag small{font-size:10px}\
.pylon .tag b{max-width:110px}\
.pylon .slots i{width:5px;height:5px}\
#pylon-card{top:auto;left:calc(12px + env(safe-area-inset-left));right:calc(12px + env(safe-area-inset-right));width:auto;height:min(60vh,520px)}\
}\
@media (prefers-reduced-motion:reduce){.pylon,.pylon .beam,#pylon-card{transition-duration:.15s}\
.pylon .base{animation:none}\
.pylon.pa .ring,.pylon.pb .ring{animation-duration:.6s}\
#pylon-card{transform:none}\
#everglade-canvas{transition-duration:.15s}\
}\
#pylon-composer{position:fixed;inset:0;z-index:6;opacity:0;visibility:hidden;pointer-events:none;transition:opacity .3s ease,visibility .3s}\
#pylon-composer.open{opacity:1;visibility:visible}\
.home-chat-mount{position:absolute;inset:0;pointer-events:none}\
.home-chat-mount canvas{position:absolute;pointer-events:auto}\
#lander-chat-keep{position:fixed;pointer-events:none;display:none}\
@media(prefers-reduced-motion:reduce){#pylon-composer{transition:none}\
}\
\
";

/// The mark and the name as the page's heading, and the account links.
fn heading() -> String {
    format!(
        "<header data-pylons-keep><h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1><nav id=\"home-account\" aria-label=\"Account\">\
<a href=\"/login?return_to={BACK}\">Log in</a><a class=\"solid\" href=\"/signup?return_to={BACK}\">Sign up</a></nav></header>"
    )
}

/// After the list: Verse's canvas and status line, the pillars, the live
/// announcements, and the pylon card.
const AFTER_LIST: &str = "\
<canvas id=\"everglade-canvas\" data-ambient tabindex=\"-1\" aria-label=\"The Grid, where people, agents, and pylons meet\">\
</canvas>\
<p id=\"everglade-status\" class=\"sr\" aria-live=\"polite\">\
</p>\
<div id=\"pylons-field\" aria-hidden=\"true\">\
</div>\
<p id=\"pylons-said\" class=\"sr\" aria-live=\"polite\">\
</p>\
<div id=\"lander-chat-keep\" data-pylons-keep>\
</div>\
<section id=\"pylon-composer\" role=\"region\" aria-label=\"Chat with the pylons\" aria-hidden=\"true\" inert>\
<div id=\"home-chat-mount\" class=\"home-chat-mount\">\
</div>\
</section>\
<p id=\"home-chat-status\" class=\"sr\" aria-live=\"polite\">\
</p>\
<div id=\"pylon-card\" role=\"dialog\" aria-labelledby=\"pylon-facts-name\">\
<div id=\"pylon-card-mount\" class=\"pylon-card-mount\" aria-hidden=\"true\">\
</div>\
<div id=\"pylon-facts\">\
<h2 id=\"pylon-facts-name\">\
</h2>\
<dl>\
</dl>\
</div>\
<button id=\"pylon-close\" type=\"button\" aria-label=\"Close\">\
&times;</button>\
</div>\
\
";

/// The loader asks who is signed in and after the Grid's first full
/// frame hands the Grid to a worker. Then, in idle moments: the pylons'
/// layer; once it runs, the Verse build at low priority, which fades in
/// behind the pillars on its first frame and stops the shader. The layer
/// calls `pylonsCard.show(json)` and `pylonsCard.hide()`; the first `show`
/// loads the card's build and mounts it. GPUI focuses a text field of its
/// own when the card starts; the card takes no typing, so that field is
/// hidden and focus goes back where it was.
const LOADER: &str = "\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
const account=()=>fetch(\"/home/me\").then(r=>r.ok?r.json():null).then(me=>{if(!me||!me.signed_in)return;\
const nav=document.getElementById(\"home-account\");\
nav.textContent=\"\";\
const who=document.createElement(me.profile?\"a\":\"span\");\
if(me.profile)who.href=me.profile;\
who.append(me.name);\
nav.append(who);\
const chat=document.createElement(\"a\");\
chat.className=\"solid\";\
chat.href=me.chat;\
chat.textContent=\"Open chat\";\
nav.append(chat)}).catch(()=>{});\
const box=document.getElementById(\"pylon-card\"),mount=document.getElementById(\"pylon-card-mount\");\
let handed=null;\
const handoff=()=>handed||(handed=new Promise(done=>{const runner=globalThis.lander5Runner,main=document.getElementById(\"lander5-canvas\");\
if(still||!runner||!main||!main.transferControlToOffscreen||!window.Worker)return done();\
setTimeout(done,2000);\
runner.ready.then(()=>{const grid=document.createElement(\"canvas\");\
grid.id=\"home-grid\";\
grid.setAttribute(\"aria-hidden\",\"true\");\
main.after(grid);\
const offscreen=grid.transferControlToOffscreen();\
const source=document.querySelector(\"script[type=module]\").textContent;\
const worker=new Worker(URL.createObjectURL(new Blob([source],{type:\"text/javascript\"})),{type:\"module\"});\
const view=()=>({w:innerWidth,h:innerHeight,dpr:devicePixelRatio});\
worker.addEventListener(\"message\",e=>{if(e.data===\"presented\"){performance.mark(\"grid-worker-presented\");\
runner.pause();\
done()}});\
worker.addEventListener(\"error\",()=>{grid.remove();\
done()});\
worker.postMessage({canvas:offscreen,sky:document.getElementById(\"lander5-sky\").textContent,grid:document.getElementById(\"lander5-grid\").textContent,...view(),time:runner.scene_time()},[offscreen]);\
addEventListener(\"resize\",()=>worker.postMessage({resize:1,...view()}));\
globalThis.homeGridWorker=worker}).catch(done)}));\
let chosen;\
const variant=()=>chosen||(chosen=(async()=>{try{return navigator.gpu&&await navigator.gpu.requestAdapter()?\"webgpu\":\"webgl\"}catch(e){return \"webgl\"}})());\
let mounting=Promise.resolve();\
const onMount=f=>{const next=mounting.then(f);\
mounting=next.catch(()=>{});\
return next};\
\nconst chatBox=document.getElementById(\"home-chat-mount\"),composer=document.getElementById(\"pylon-composer\"),keep=document.getElementById(\"lander-chat-keep\"),chatStatus=document.getElementById(\"home-chat-status\");\
\nlet terminalNear=false,chatReady=false,chatField=null,chatLoad=null;\
const held=new Set(),blocked=new Set();\
\nconst syncChat=()=>{const show=terminalNear&&chatReady;\
composer.classList.toggle(\"open\",show);\
composer.setAttribute(\"aria-hidden\",String(!show));\
composer.inert=!show;\
keep.style.display=show?\"block\":\"none\";\
\nif(chatField){chatField.disabled=!show;\
if(show){for(const code of held)blocked.add(code);\
requestAnimationFrame(()=>{if(terminalNear)chatField.focus({preventScroll:true})})}else if(document.activeElement===chatField)chatField.blur()}\nqueueMicrotask(()=>document.dispatchEvent(new Event(\"pylons-keep\")))};\
\ndocument.addEventListener(\"pylons-keep\",()=>{const r=keep.getBoundingClientRect();\
chatBox.style.clipPath=`inset(${Math.max(0,r.top)}px ${Math.max(0,innerWidth-r.right)}px ${Math.max(0,innerHeight-r.bottom)}px ${Math.max(0,r.left)}px)`});\
\nconst loadChat=()=>chatLoad||(chatLoad=(async()=>{chatStatus.textContent=\"Opening chat…\";\
const dir=\"/lander8/build/\"+await variant()+\"/\";\
\nconst [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch(dir+\"openagents_web_lander_chat_bg.wasm\",{priority:\"low\"})),import(dir+\"openagents_web_lander_chat.js\")]);\
await idle(300);\
await glue.default({module_or_path:module});\
\nawait onMount(()=>new Promise((ok,no)=>{const fields=new Set(document.querySelectorAll(\"body>input\"));\
chatBox.id=\"gpui-mount\";\
\nchatBox.addEventListener(\"gpui-failed\",()=>{chatBox.id=\"home-chat-mount\";\
no(new Error(\"Chat couldn't open\"))},{once:true});\
\nchatBox.addEventListener(\"gpui-first-frame\",()=>{chatBox.id=\"home-chat-mount\";\
chatReady=true;\
setTimeout(()=>{chatField=Array.from(document.querySelectorAll(\"body>input\")).find(f=>!fields.has(f));\
if(chatField){chatField.id=\"pylon-message\";\
chatField.setAttribute(\"aria-label\",\"Message the pylons\")}chatStatus.textContent=\"\";\
performance.mark(\"home-chat-ready\");\
syncChat();\
ok()})},{once:true});\
glue.mount(still)}));\
return glue})().catch(error=>{chatStatus.textContent=\"Chat couldn't open. Reload to try again.\";\
console.warn(error);\
const canvas=document.getElementById(\"everglade-canvas\");\
canvas.removeAttribute(\"data-composing\");\
throw error}));\
\nglobalThis.homeTerminal=near=>{terminalNear=near;if(near)for(const code of held)blocked.add(code);\
const canvas=document.getElementById(\"everglade-canvas\");\
canvas.toggleAttribute(\"data-composing\",near);\
syncChat();\
if(near)loadChat().catch(()=>{})};\
\naddEventListener(\"keydown\",e=>{if(e.key===\"Escape\"&&terminalNear){e.preventDefault();\
e.stopImmediatePropagation();\
homeTerminal(false);\
document.getElementById(\"everglade-canvas\").focus();\
return}if(blocked.has(e.code)&&terminalNear){e.preventDefault();\
e.stopImmediatePropagation();\
return}held.add(e.code)},true);\
\naddEventListener(\"keyup\",e=>{held.delete(e.code);\
blocked.delete(e.code)},true);\
addEventListener(\"blur\",()=>{held.clear();\
blocked.clear()});\
\nlet card=null,latest=\"\";\
const loadCard=()=>card||(card=(async()=>{const dir=\"/pylons/card/\"+await variant()+\"/\";\
const [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch(dir+\"openagents_web_pylon_card_bg.wasm\")),import(dir+\"openagents_web_pylon_card.js\")]);\
await idle(300);\
await glue.default({module_or_path:module});\
glue.show(latest);\
await onMount(()=>new Promise((ok,no)=>{const before=document.activeElement;const fields=new Set(document.querySelectorAll(\"body>input\"));mount.id=\"gpui-mount\";\
mount.addEventListener(\"gpui-failed\",no,{once:true});\
mount.addEventListener(\"gpui-first-frame\",()=>{mount.id=\"pylon-card-mount\";\
box.classList.add(\"ready\");\
performance.mark(\"pylon-card-first-frame\");\
setTimeout(()=>{for(const field of document.querySelectorAll(\"body>input\"))if(!fields.has(field)){field.tabIndex=-1;\
field.setAttribute(\"aria-hidden\",\"true\")}const now=document.activeElement;\
if(now&&now.tagName===\"INPUT\"&&!fields.has(now)){if(before&&before!==document.body)before.focus({preventScroll:true});\
else now.blur()}});\
ok()},{once:true});\
glue.mount(still)}));\
return glue})());\
globalThis.pylonsCard={show(json){latest=json;\
box.classList.add(\"open\");\
loadCard().then(glue=>glue.show(latest)).catch(()=>{box.classList.remove(\"ready\")})},hide(){box.classList.remove(\"open\")}};\
const loadLayer=async()=>{try{const [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch(\"/pylons/layer/pylons_web_bg.wasm\")),import(\"/pylons/layer/pylons_web.js\")]);\
await idle(300);\
await glue.default({module_or_path:module});\
glue.start(still);\
document.documentElement.classList.add(\"js\");\
performance.mark(\"pylons-layer\")}catch(e){}};\
const verse=async()=>{try{await idle(800);\
performance.mark(\"verse-start\");\
const canvas=document.getElementById(\"everglade-canvas\"),status=document.getElementById(\"everglade-status\");\
const drawn=new Promise(r=>new MutationObserver((list,observer)=>{if(status.style.display===\"none\"){observer.disconnect();\
r()}}).observe(status,{attributes:true,attributeFilter:[\"style\"]}));\
const [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch(\"/everglade/everglade_web_bg.wasm\",{priority:\"low\"})),import(\"/everglade/everglade_web.js\")]);\
performance.mark(\"verse-compiled\");\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"verse-instantiated\");\
await drawn;\
await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));\
performance.mark(\"verse-first-frame\");\
canvas.addEventListener(\"transitionend\",()=>{performance.mark(\"verse-visible\");\
const worker=globalThis.homeGridWorker;\
if(worker)worker.terminate();\
const runner=globalThis.lander5Runner;\
if(runner)runner.pause();\
const grid=document.getElementById(\"home-grid\");\
if(grid)grid.remove()},{once:true});\
canvas.classList.add(\"ready\");\
addEventListener(\"verse-stopped\",()=>canvas.classList.remove(\"ready\"),{once:true})}catch(e){}};\
let started=false;\
const go=async()=>{if(started)return;\
started=true;\
account();\
await handoff();\
await idle(400);\
await loadLayer();\
verse()};\
if(performance.getEntriesByName(\"lander5-presented\").length)go();\
else{new PerformanceObserver((list,observer)=>{if(list.getEntriesByName(\"lander5-presented\").length){observer.disconnect();\
go()}}).observe({type:\"mark\"});\
addEventListener(\"load\",()=>setTimeout(go,1500))}\
";

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: "",
    script: LOADER,
    runner_tail: super::lander6::RUNNER_TAIL,
    main: None,
    grid: Some(super::pylons::still_gray_grid),
};

/// The last document made: the runner build and the list it came from.
static KEPT: Mutex<Option<(BuildKey, String, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(GRID_HOME_PATH, get(page))
        .route(GRID_HOME_ME, get(me))
}

/// `<main>` for this response: the top bar, the list as it stands, and the
/// rest.
fn main_html(list: &str) -> String {
    format!(
        "{}<section id=\"pylons-panel\" aria-labelledby=\"pylons-count\">{list}</section>{AFTER_LIST}",
        heading()
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

/// `{signed_in, name, profile, chat}` for the top bar: who is
/// signed in, where their profile is (their key's page, or their GitHub
/// login's, which moves to the key's), and the chat. Signed out (or on a
/// server without sign-in),
/// `{signed_in: false}`.
async fn me(State(app): State<App>, headers: HeaderMap) -> Response {
    let mut body = json!({"signed_in": false});
    if let Some(service) = app.config.cloud.as_deref()
        && crate::chat_owner::session(&headers).is_some()
        && let Ok(viewer) = service.authenticate(&headers).await
    {
        let name = match &viewer.nostr {
            Some(key) => crate::profiles::name_of_key(&app, key).await,
            None => None,
        };
        let profile = match (&viewer.nostr, &viewer.github) {
            (Some(key), _) => Some(crate::profiles::href(key, name.as_deref())),
            (None, Some((_, login))) => Some(format!("/u/{login}")),
            (None, None) => None,
        };
        body = json!({
            "signed_in": true,
            "name": name.unwrap_or(viewer.account_label),
            "profile": profile,
            "chat": CHAT,
        });
    }
    let mut response = (
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_a_heading_account_links_list_and_lazy_gpui_chat() {
        let list = "<h2 id=\"pylons-count\">1 pylon online</h2><ul id=\"pylons-list\"></ul>";
        let document = assemble_page(
            None,
            Some(&ADDITION),
            Some(&main_html(list)),
            (TITLE, DESCRIPTION),
        );
        let html = &document.html;
        assert!(
            html.contains("</svg>OpenAgents</h1><nav id=\"home-account\" aria-label=\"Account\">")
        );
        assert!(html.contains("href=\"/login?return_to=%2Fhome\">Log in</a>"));
        assert!(html.contains("href=\"/signup?return_to=%2Fhome\">Sign up</a>"));
        assert!(html.contains(list));
        assert!(html.contains("<canvas id=\"everglade-canvas\" data-ambient"));
        // The GPUI chat is lazy; there is no HTML composer.
        assert!(html.contains("openagents_web_lander_chat") && html.contains("pylon-composer"));
        for hidden in ["wallet", "Wallet", "sats", "hand-toggle"] {
            assert!(!html.contains(hidden), "{hidden}");
        }
        let policy = document.policy.to_str().unwrap();
        assert!(policy.contains(&super::super::lander5::source_hash(LOADER)));
        assert!(policy.contains(&super::super::lander5::source_hash(STYLE)));
        assert!(policy.contains("connect-src 'self'; worker-src blob:;"));
        assert!(!policy.contains("unsafe-inline"));
        for copy in [
            heading().as_str(),
            AFTER_LIST,
            TITLE,
            DESCRIPTION,
            UNAVAILABLE,
        ] {
            assert!(oa_copy::violations(copy, &[]).is_empty(), "{copy}");
        }
    }

    #[test]
    fn the_loader_loads_the_pylons_then_verse_and_the_card_on_first_open() {
        let pylons = LOADER.find("/pylons/layer/pylons_web_bg.wasm").unwrap();
        let verse = LOADER.find("/everglade/everglade_web_bg.wasm").unwrap();
        assert!(pylons < verse);
        assert!(LOADER.contains("await loadLayer();verse()"));
        assert!(LOADER.contains("transferControlToOffscreen"));
        assert!(LOADER.contains("worker.terminate()"));
        assert!(LOADER.contains("globalThis.pylonsCard"));
        assert!(LOADER.contains(super::super::pylons::CARD_WASM));
        assert!(LOADER.contains("/home/me"));
        assert!(!LOADER.contains("</script"));
    }
}
