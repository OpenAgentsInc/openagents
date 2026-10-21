//! `/`: the Grid homepage. `/home`, where it started, redirects here.
//!
//! The header and accessible pylon list are HTML. The live pylons and Verse
//! load in separate chunks; only Verse draws the world. Once it paints,
//! the page prepares the GPUI chat so approaching the shared terminal
//! does not wait for its download and first frame.
//!
//! Approaching the shared terminal opens the GPUI lander chat. [`me`] (`GET /home/me`, kept at its
//! first address because the page and older clients call it) tells the top
//! bar who is signed in, so the page itself stays one cached document:
//! signed in, the bar shows the person's name linking to
//! their profile, and Open chat.
//!
//! `/` used to be the new-chat page; that moved to [`super::home::NEW`]
//! (`/chat`), and the classic home page to [`super::home::HOME`].

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use super::lander5::{Addition, Document, assemble_page_with_head, send};
use super::pylons_field::{field, list_html};
use crate::App;

/// The page.
pub(crate) const GRID_HOME_PATH: &str = "/";
/// The page's first address, which redirects to [`GRID_HOME_PATH`] with
/// its query (`?chat=` names the terminal's open chat).
pub(crate) const OLD_GRID_HOME_PATH: &str = "/home";
/// Who is signed in, for the top bar.
pub(crate) const GRID_HOME_ME: &str = "/home/me";

const TITLE: &str = "OpenAgents";
const DESCRIPTION: &str = "OpenAgents: agents, pylons, and the Grid where they meet.";

/// Where Log in and Sign up return to.
const BACK: &str = "%2F";
/// The chat entry point respects the feature flags and saved UI choice.
const CHAT: &str = super::home::NEW;

/// The top bar, the pylons' list (visible without
/// script, read by screen readers with it), the pillars, the pylon card,
/// and Verse's canvas, hidden until its first frame. The pillars' look is
/// `/pylons`'.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a;color:#e5e5e5;font-family:Geist,-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
#lander5-canvas{display:none}\
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
#pylons-field{visibility:hidden;position:fixed;inset:0;z-index:2;pointer-events:none;overflow:hidden}\
.world-ready #pylons-field{visibility:visible}\
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
.pylon.group-part .beam{opacity:.6}\
.pylon.group-part .tag b{max-width:120px}\
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
#pylons-field.bay .pylon .beam,#pylons-field.bay .pylon .base,#pylons-field.bay .pylon .ring{display:none}\
#pylons-field.bay .pylon .tag{left:calc(50% + var(--lean,0px));bottom:calc(100% + 4px);opacity:0;transition:opacity .2s ease;pointer-events:none}\
#pylons-field.bay .pylon.t1 .tag,#pylons-field.bay .pylon.t2 .tag,#pylons-field.bay .pylon:hover .tag{opacity:1}\
#pylons-field.bay .pylon .tag small,#pylons-field.bay .pylon .tag .slots{display:none}\
#pylons-field.bay .pylon.t1 .tag b{font-size:11px;font-weight:500;line-height:14px;color:#d4d4d4}\
#pylons-field.bay .pylon.t2 .tag,#pylons-field.bay .pylon:hover .tag{padding:6px 10px 7px;border-radius:8px;background:rgba(8,8,8,.8);border:1px solid rgba(255,255,255,.14);text-shadow:none}\
#pylons-field.bay .pylon.t2 .tag b,#pylons-field.bay .pylon:hover .tag b{font-size:12px;line-height:16px;color:#fff}\
#pylons-field.bay .pylon.t2 .tag small,#pylons-field.bay .pylon:hover .tag small{display:block;line-height:15px}\
#pylons-field.bay .pylon.t2 .tag .slots,#pylons-field.bay .pylon:hover .tag .slots{display:flex;margin-top:4px}\
#pylons-field.bay .pylon:hover{z-index:20000!important}\
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
[hidden]{display:none!important}#game-lab-panel{position:fixed;z-index:7;top:70px;right:20px;width:min(560px,45vw);max-height:calc(100vh - 230px);overflow:auto;padding:16px;border:1px solid #333;border-radius:16px;background:#101010;box-sizing:border-box}.game-lab-heading{display:flex;align-items:center;justify-content:space-between;gap:12px}#game-lab-panel button,#game-lab-back{border:1px solid #555;border-radius:8px;background:#222;color:#eee;padding:8px 12px;cursor:pointer}#game-lab-status{font-size:13px;color:#aaa}#game-lab-card{border:0;width:100%;height:460px;background:#101010}#game-lab-signin{color:#fff}#game-lab-play{position:fixed;inset:0;z-index:20;background:#000}#game-lab-play-frame{width:100%;height:100%;border:0}#game-lab-back{position:absolute;z-index:21;top:16px;left:16px}@media(max-width:760px){#game-lab-panel{left:12px;right:12px;width:auto;top:60px;max-height:48vh}#game-lab-card{height:320px}}
";

/// The mark and the name as the page's heading, and the account links
/// when `account` is on ([`crate::features::Feature::HomeAccount`]). The
/// empty `<nav>` stays either way: the loader fills it for a signed-in
/// visitor.
fn heading(account: bool) -> String {
    let links = if account {
        format!(
            "<a href=\"/login?return_to={BACK}\">Log in</a><a class=\"solid\" href=\"/signup?return_to={BACK}\">Sign up</a>"
        )
    } else {
        String::new()
    };
    // Game Lab needs a signed-in account, so it shows only where sign-in
    // does (the same switch as the account links).
    let game_lab = if account {
        "<nav aria-label=\"Game Lab\"><a id=\"game-lab-open\" href=\"/?lab=game\">Game Lab</a></nav>"
    } else {
        ""
    };
    format!(
        "<header data-pylons-keep><h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1>{game_lab}<nav id=\"home-account\" aria-label=\"Account\">\
{links}</nav></header>"
    )
}

/// After the list: Verse's canvas and status line, the pillars, the live
/// announcements, and the pylon card.
const AFTER_LIST: &str = concat!(
    "\
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
",
    r#"<section id="game-lab-panel" aria-label="Game Lab" hidden><div class="game-lab-heading"><strong>Game Lab</strong><button id="game-lab-close" type="button">Back to the Grid</button></div><p id="game-lab-status" role="status"></p><a id="game-lab-signin" href="/login?return_to=%2F%3Flab%3Dgame" hidden>Sign in</a><iframe id="game-lab-card" title="Your game" allow="fullscreen" hidden></iframe></section><section id="game-lab-play" aria-label="Play your game" hidden><button id="game-lab-back" type="button">Back to Game Lab</button><iframe id="game-lab-play-frame" title="Play your game" allow="fullscreen"></iframe></section>"#
);

/// Loads the pylons, Verse, and GPUI chat as separate modules. The pylon
/// card loads on click. Only the focused GPUI chat input pauses world
/// controls: the card has nothing to type into, so a click on it hands the
/// keyboard straight back to the world (#12415).
const LOADER: &str = concat!(
    "\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
const account=()=>fetch(\"/home/me\").then(r=>r.ok?r.json():null).then(me=>{if(!me||!me.signed_in)return;\
const grid=document.getElementById(\"everglade-canvas\");\
if(me.verse_name)grid.dataset.name=me.verse_name;\
if(me.csrf)grid.dataset.csrf=me.csrf;\
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
let chosen;\
const variant=()=>chosen||(chosen=(async()=>{try{return navigator.gpu&&await navigator.gpu.requestAdapter()?\"webgpu\":\"webgl\"}catch(e){return \"webgl\"}})());\
let mounting=Promise.resolve();\
const onMount=f=>{const next=mounting.then(f);\
mounting=next.catch(()=>{});\
return next};\
\nconst chatBox=document.getElementById(\"home-chat-mount\"),composer=document.getElementById(\"pylon-composer\"),keep=document.getElementById(\"lander-chat-keep\"),chatStatus=document.getElementById(\"home-chat-status\");\
let terminalNear=false,chatReady=false,chatField=null,chatLoad=null,chatAssets=null,focusPending=false;\n\
const held=new Set(),blocked=new Set();\n\
const world=document.getElementById(\"everglade-canvas\");\n\
const syncFocus=()=>world.toggleAttribute(\"data-composing\",chatReady&&terminalNear&&document.activeElement===chatField);\n\
const syncChat=()=>{const show=terminalNear&&chatReady;\n\
composer.classList.toggle(\"open\",show);\n\
composer.setAttribute(\"aria-hidden\",String(!show));\n\
composer.inert=!show;\n\
keep.style.display=show?\"block\":\"none\";\n\
if(chatField){chatField.disabled=!show;\n\
if(show&&focusPending)requestAnimationFrame(()=>{if(focusPending&&terminalNear&&chatReady){focusPending=false;\n\
for(const code of held)blocked.add(code);\n\
chatField.focus({preventScroll:true});syncFocus()}});\n\
else if(!show&&document.activeElement===chatField)chatField.blur()}\n\
syncFocus();\n\
queueMicrotask(()=>document.dispatchEvent(new Event(\"pylons-keep\")))};\n\
document.addEventListener(\"pylons-keep\",()=>{const r=keep.getBoundingClientRect();\n\
chatBox.style.clipPath=`inset(${Math.max(0,r.top)}px ${Math.max(0,innerWidth-r.right)}px ${Math.max(0,innerHeight-r.bottom)}px ${Math.max(0,r.left)}px)`});\n\
const prepareChat=()=>chatAssets||(chatAssets=(async()=>{const dir=\"/lander8/build/\"+await variant()+\"/\";\n\
return Promise.all([WebAssembly.compileStreaming(fetch(dir+\"openagents_web_lander_chat_bg.wasm\",{priority:\"low\"})),import(dir+\"openagents_web_lander_chat.js\")])})());\n\
const loadChat=()=>chatLoad||(chatLoad=(async()=>{performance.mark(\"home-chat-start\");\n\
if(terminalNear)chatStatus.textContent=\"Opening chat…\";\n\
const [module,glue]=await prepareChat();performance.mark(\"home-chat-compiled\");\n\
await idle(300);\n\
await glue.default({module_or_path:module});performance.mark(\"home-chat-instantiated\");\n\
await onMount(()=>new Promise((ok,no)=>{const fields=new Set(document.querySelectorAll(\"body>input\")),before=document.activeElement;\n\
const captureField=()=>{if(chatField)return;\n\
chatField=Array.from(document.querySelectorAll(\"body>input\")).find(f=>!fields.has(f));\n\
if(!chatField)return;\n\
const tookFocus=document.activeElement===chatField;\n\
chatField.id=\"pylon-message\";\n\
chatField.setAttribute(\"aria-label\",globalThis.openagentsLab===\"game\"?\"Describe your game change\":\"Message OpenAgents\");\n\
chatField.disabled=true;\n\
chatField.addEventListener(\"focus\",syncFocus);\n\
chatField.addEventListener(\"blur\",syncFocus);\n\
if(tookFocus)(before&&before!==document.body?before:world).focus({preventScroll:true});\n\
observer.disconnect()};\n\
const observer=new MutationObserver(captureField);\n\
observer.observe(document.body,{childList:true});\n\
chatBox.id=\"gpui-mount\";\n\
chatBox.addEventListener(\"gpui-failed\",()=>{observer.disconnect();chatBox.id=\"home-chat-mount\";\n\
no(new Error(\"Chat couldn't open\"))},{once:true});\n\
chatBox.addEventListener(\"gpui-first-frame\",()=>{chatBox.id=\"home-chat-mount\";\n\
captureField();observer.disconnect();chatReady=true;\n\
chatStatus.textContent=\"\";\n\
performance.mark(\"home-chat-ready\");\n\
syncChat();ok()},{once:true});\n\
performance.mark(\"home-chat-mounting\");glue.mount(still)}));\n\
return glue})().catch(error=>{chatStatus.textContent=\"Chat couldn't open. Reload to try again.\";\n\
console.warn(error);focusPending=false;syncFocus();throw error}));\n\
globalThis.homeTerminal=near=>{near=(globalThis.openagentsLab===\"game\"||near)&&world.classList.contains(\"ready\");terminalNear=near;focusPending=near;\n\
syncChat();\n\
if(near)loadChat().catch(()=>{})};\n\
world.addEventListener(\"pointerdown\",()=>{focusPending=false;\n\
if(chatField&&document.activeElement===chatField)chatField.blur();\n\
world.focus({preventScroll:true});syncFocus()},true);\n\
addEventListener(\"keydown\",e=>{if(e.key===\"Escape\"&&terminalNear){if(globalThis.openagentsLab===\"game\")return;e.preventDefault();\n\
e.stopImmediatePropagation();homeTerminal(false);world.focus();return}\n\
if(blocked.has(e.code)&&document.activeElement===chatField){e.preventDefault();e.stopImmediatePropagation();return}\n\
held.add(e.code)},true);\n\
addEventListener(\"keyup\",e=>{held.delete(e.code);blocked.delete(e.code)},true);\n\
addEventListener(\"blur\",()=>{held.clear();blocked.clear()});\n\
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
field.setAttribute(\"aria-hidden\",\"true\");\
field.addEventListener(\"focus\",()=>world.focus({preventScroll:true}))}const now=document.activeElement;\
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
prepareChat().catch(()=>{});\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"verse-instantiated\");\
await drawn;\
await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));\
performance.mark(\"verse-first-frame\");\
canvas.addEventListener(\"transitionend\",()=>performance.mark(\"verse-visible\"),{once:true});\
canvas.classList.add(\"ready\");\
document.documentElement.classList.add(\"world-ready\");\
idle(1200).then(()=>loadChat()).catch(()=>{});\
addEventListener(\"verse-stopped\",()=>{canvas.classList.remove(\"ready\");document.documentElement.classList.remove(\"world-ready\");homeTerminal(false)},{once:true})}catch(e){}};\
account();\
loadLayer().then(verse);\
",
    include_str!("grid_game_lab.js")
);

/// The relay the Grid's players meet on, which the page's policy admits
/// (#12406).
const HOME_CONNECT: &str = super::everglade::GRID_RELAY;

const ADDITION: Addition = Addition {
    style: STYLE,
    in_main: "",
    script: LOADER,
    runner_tail: "",
    main: None,
    grid: None,
    connect: HOME_CONNECT,
};

/// The last document and the live list it came from.
static KEPT: Mutex<Option<((String, bool), Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(GRID_HOME_PATH, get(page))
        .route(OLD_GRID_HOME_PATH, get(moved))
        .route(GRID_HOME_ME, get(me))
}

/// `/home`, permanently at `/` now, with the query kept.
async fn moved(uri: axum::http::Uri) -> axum::response::Redirect {
    let to = match uri.query() {
        Some(query) => format!("{GRID_HOME_PATH}?{query}"),
        None => GRID_HOME_PATH.to_owned(),
    };
    axum::response::Redirect::permanent(&to)
}

/// The page's metadata: description, link preview, canonical address,
/// Markdown twin, and the site's JSON-LD, as every page of the site has.
fn head() -> String {
    crate::agent_ready::head(TITLE, Some(DESCRIPTION), Some(GRID_HOME_PATH))
}

/// `<main>` for this response: the top bar, the list as it stands, and the
/// rest.
fn main_html(list: &str, account: bool) -> String {
    let after = if account {
        AFTER_LIST.to_owned()
    } else {
        // The Game Lab script finds its panel either way; `data-off` keeps
        // it from opening.
        AFTER_LIST.replacen(
            "<section id=\"game-lab-panel\"",
            "<section id=\"game-lab-panel\" data-off",
            1,
        )
    };
    format!(
        "{}<section id=\"pylons-panel\" aria-labelledby=\"pylons-count\">{list}</section>{after}",
        heading(account)
    )
}

fn document(list: String, account: bool) -> Arc<Document> {
    if let Ok(kept) = KEPT.lock()
        && let Some(((kept_list, kept_account), document)) = kept.as_ref()
        && *kept_list == list
        && *kept_account == account
    {
        return document.clone();
    }
    let document = Arc::new(assemble_page_with_head(
        None,
        Some(&ADDITION),
        Some(&main_html(&list, account)),
        (TITLE, DESCRIPTION),
        Some(&head()),
    ));
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some(((list, account), document.clone()));
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
    let account = app
        .config
        .features
        .on(crate::features::Feature::HomeAccount);
    let fallback = list.clone();
    let document = tokio::task::spawn_blocking(move || document(list, account))
        .await
        .unwrap_or_else(|_| {
            Arc::new(assemble_page_with_head(
                None,
                Some(&ADDITION),
                Some(&main_html(&fallback, account)),
                (TITLE, DESCRIPTION),
                Some(&head()),
            ))
        });
    let mut response = send(&document, &request);
    if let Some(policy) = response
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(policy) = HeaderValue::from_str(&format!("{policy}; frame-src 'self'")) {
            response
                .headers_mut()
                .insert(header::CONTENT_SECURITY_POLICY, policy);
        }
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

/// `{signed_in, name, profile, chat, verse_name, csrf}` for the top bar
/// and the Grid's avatar: who is
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
        // The Grid's avatar carries the profile name, never the account
        // label, and asks for its owner tag with the page's write value
        // (#12313, #12406).
        let csrf = match crate::chat_owner::who(&app, &headers).await {
            crate::chat_owner::Who::Account(owner) => Some(super::chat::csrf(&app, &owner)),
            _ => None,
        };
        body = json!({
            "signed_in": true,
            "verse_name": name,
            "name": name.clone().unwrap_or(viewer.account_label),
            "profile": profile,
            "chat": CHAT,
            "csrf": csrf,
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
    fn production_hides_the_account_links_but_keeps_the_nav() {
        let html = main_html("<ul></ul>", false);
        assert!(html.contains("<nav id=\"home-account\" aria-label=\"Account\"></nav>"));
        assert!(!html.contains(">Log in</a>") && !html.contains(">Sign up</a>"));
        assert!(html.contains("id=\"game-lab-signin\""));
        assert!(!html.contains("game-lab-open"));
        assert!(html.contains("<section id=\"game-lab-panel\" data-off"));
        assert!(!crate::features::Features::default().on(crate::features::Feature::HomeAccount));
    }

    #[test]
    fn the_page_has_a_heading_account_links_list_and_lazy_gpui_chat() {
        let list = "<h2 id=\"pylons-count\">1 pylon online</h2><ul id=\"pylons-list\"></ul>";
        let document = assemble_page_with_head(
            None,
            Some(&ADDITION),
            Some(&main_html(list, true)),
            (TITLE, DESCRIPTION),
            Some(&head()),
        );
        let html = &document.html;
        assert!(
            html.contains("</svg>OpenAgents</h1>")
                && html.contains("<nav id=\"home-account\" aria-label=\"Account\">")
        );
        assert!(html.contains("id=\"game-lab-open\" href=\"/?lab=game\""));
        assert!(html.contains("href=\"/login?return_to=%2F\">Log in</a>"));
        assert!(html.contains("href=\"/signup?return_to=%2F\">Sign up</a>"));
        // `/` carries the site's metadata: its preview, canonical address,
        // and Markdown twin.
        assert!(html.contains("<link rel=\"canonical\" href=\"https://openagents.com/\">"));
        assert!(html.contains("href=\"/index.md\""));
        assert!(html.contains("og:image"));
        assert_eq!(html.matches("name=\"description\"").count(), 1);
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
        assert!(
            policy.contains("connect-src 'self' wss://relay.openagents.com; worker-src blob:;")
        );
        assert!(!policy.contains("unsafe-inline"));
        for copy in [
            heading(true).as_str(),
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
        assert!(LOADER.contains("loadLayer().then(verse)"));
        assert!(!LOADER.contains("lander5Runner"));
        assert!(!LOADER.contains("transferControlToOffscreen"));
        assert!(
            !document(UNAVAILABLE.to_owned(), true)
                .html
                .contains("x-shader")
        );
        assert!(LOADER.contains("globalThis.pylonsCard"));
        assert!(LOADER.contains(super::super::pylons::CARD_WASM));
        assert!(LOADER.contains("/home/me"));
        // A click on the card hands the keyboard back to the world.
        assert!(LOADER.contains("addEventListener(\"focus\",()=>world.focus("));
        assert!(!LOADER.contains("</script"));
    }
}
