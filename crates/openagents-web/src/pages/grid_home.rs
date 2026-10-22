//! `/`: the Grid homepage. `/home`, where it started, redirects here.
//!
//! The header and accessible pylon list are HTML. The live pylons and Verse
//! load in separate chunks; only Verse draws the world. Once it paints,
//! the page prepares the GPUI chat so approaching the shared terminal
//! does not wait for its download and first frame.
//!
//! Nothing over the world is HTML once Verse and the chat are up (#12459).
//! Verse draws the walking hint, the machines' labels, and the prompts at
//! the help desk and under Game Lab's arch. The GPUI chat draws the top
//! bar, the chat, and Game Lab's and Kiln's panels; the GPUI pylon card
//! draws a pylon's details. The HTML they replace stays in the page for
//! screen readers, keyboards, crawlers, and browsers without them.
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
.reveal,.gpui-ready header{clip-path:inset(50%)}\
.reveal:focus-within,.gpui-ready header:focus-within{clip-path:none}\
.gpui-ready header nav{pointer-events:none}\
.gpui-ready header:focus-within nav{pointer-events:auto}\
#pylons-panel{position:fixed;z-index:3;left:calc(16px + env(safe-area-inset-left));bottom:calc(16px + env(safe-area-inset-bottom));box-sizing:border-box;width:min(340px,calc(100vw - 32px));max-height:40vh;overflow:auto;padding:12px 14px;border-radius:14px;background:rgba(10,10,10,.62);border:1px solid rgba(255,255,255,.1)}\
#pylons-panel h2{margin:0 0 6px;font-size:13px;font-weight:600}\
#pylons-panel ul{list-style:none;margin:0;padding:0}\
#pylons-panel li{padding:4px 0;font-size:13px}\
#pylons-panel .d{display:block;color:#9a9a9a;font-size:12px}\
#pylons-panel button{all:unset}\
#pylons-field{visibility:hidden;position:fixed;inset:0;z-index:2;pointer-events:none;overflow:hidden}\
.world-ready #pylons-field{visibility:visible}\
html:is([data-verse-place=zone],[data-verse-place=loading]) #pylons-field{visibility:hidden!important}\
html:has(#game-lab-play:not([hidden])) :is(#pylons-field,#pylon-card){visibility:hidden!important}\
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
#pylons-field.bay .pylon .tag{display:none}\
#pylons-field.bay .pylon:hover{z-index:20000!important}\
#pylon-card{position:fixed;z-index:5;top:calc(60px + env(safe-area-inset-top));right:calc(16px + env(safe-area-inset-right));bottom:calc(16px + env(safe-area-inset-bottom));width:min(400px,calc(100vw - 32px));max-height:600px;border-radius:16px;overflow:hidden;opacity:0;visibility:hidden;transform:translateX(14px);transition:opacity .3s cubic-bezier(.22,.8,.3,1),transform .3s cubic-bezier(.22,.8,.3,1),visibility 0s .3s}\
#pylon-card:not(.ready){background:rgba(8,8,8,.82);border:1px solid rgba(255,255,255,.12)}\
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
#pylon-card.ready #pylon-close{color:transparent;background:none}\
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
#pylon-composer.ready{opacity:1;visibility:visible}\
.game-playing #pylon-composer{z-index:21}\
.home-chat-mount{position:absolute;inset:0;pointer-events:none;clip-path:inset(50%)}\
.home-frame{position:fixed;z-index:7;border:0;background:#0b0a08}\
#kiln-frame{border-radius:0 0 15px 15px}\
html:has(#kiln-frame:not([hidden])) :is(#pylons-field,#pylon-card){visibility:hidden!important}\
.home-chat-mount canvas{position:absolute;pointer-events:auto}\
#lander-chat-keep{position:fixed;pointer-events:none;display:none}\
@media(prefers-reduced-motion:reduce){#pylon-composer{transition:none}\
}\
[hidden]{display:none!important}\
#game-lab-panel{position:fixed;z-index:9;top:70px;right:20px;display:flex;flex-direction:column;gap:8px;padding:14px;border:1px solid #333;border-radius:14px;background:#101010}\
#game-lab-panel h2{margin:0;font-size:15px}\
#game-lab-panel button,#game-lab-back{border:1px solid #555;border-radius:8px;background:#222;color:#eee;padding:8px 12px;cursor:pointer}\
#game-lab-play{position:fixed;inset:0;z-index:20;background:#000}\
#game-lab-play-frame{width:100%;height:100%;border:0}\
#game-lab-back{position:absolute;z-index:22;top:16px;left:16px}\
#kiln-frame{background:transparent;color-scheme:dark}\
";

/// The mark and the name as the page's heading, and the account links
/// when `account` is on ([`crate::features::Feature::HomeAccount`]). The
/// empty `<nav>` stays either way: the loader fills it for a signed-in
/// visitor. Once the GPUI chat draws its top bar, the header is
/// transparent and repeated there, and shows again only while it holds the
/// keyboard's focus (#12459).
fn heading(account: bool) -> String {
    let links = if account {
        format!(
            "<a href=\"/login?return_to={BACK}\">Log in</a><a class=\"solid\" href=\"/signup?return_to={BACK}\">Sign up</a>"
        )
    } else {
        String::new()
    };
    // Game Lab is entered in the world, under its arch (#12459).
    format!(
        "<header data-pylons-keep><h1><svg viewBox=\"0 0 386 386\" fill=\"currentColor\" aria-hidden=\"true\">\
<path d=\"M292.64 54.39A181 181 0 1 1 93.36 54.39A21 21 0 0 1 116.48 89.46A139 139 0 1 0 269.52 89.46A21 21 0 0 1 292.64 54.39ZM172 21A21 21 0 0 1 214 21V165.5A21 21 0 0 1 172 165.5Z\"/>\
</svg>OpenAgents</h1><nav id=\"home-account\" aria-label=\"Account\">\
{links}</nav></header>"
    )
}

/// After the list: Verse's canvas, its status line and walking hint for
/// screen readers, the pillars, the live announcements, the pylon card,
/// and Game Lab's section. The GPUI chat draws Game Lab's panel and Kiln's
/// (#12459); Game Lab's section repeats the panel's words and controls for
/// screen readers and keyboards, shown only while it holds the focus.
const AFTER_LIST: &str = concat!(
    "\
<canvas id=\"everglade-canvas\" data-ambient tabindex=\"-1\" aria-label=\"The Grid, where people, agents, and pylons meet\" aria-describedby=\"grid-controls\">\
</canvas>\
<p id=\"grid-controls\" class=\"sr\">\
Click or WASD to walk. Drag to look. Scroll to zoom. Press E at the help desk or under the Game Lab arch.</p>\
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
    r#"<section id="game-lab-panel" class="reveal" aria-labelledby="game-lab-name" hidden><h2 id="game-lab-name">Game Lab</h2><p id="game-lab-status" role="status"></p><a id="game-lab-signin" href="/login?return_to=%2F%3Flab%3Dgame" hidden>Sign in</a><button id="game-lab-close" type="button">Back to the Grid</button></section><section id="game-lab-play" aria-label="Play your game" hidden><button id="game-lab-back" class="reveal" type="button">Back to Game Lab</button><iframe id="game-lab-play-frame" title="Play your game" allow="fullscreen"></iframe></section>"#
);

/// Loads the pylons, Verse, and GPUI chat as separate modules. The pylon
/// card loads on click. Only the focused GPUI chat input pauses world
/// controls: the card has nothing to type into, so a click on it hands the
/// keyboard straight back to the world (#12415).
///
/// Once the chat's canvas draws, it stays up over the world: it draws the
/// top bar, Game Lab's and Kiln's panels, and the chat while it is open
/// (#12459). Each frame it hands `homeOverlay` the boxes it drew in, and
/// the canvas is clipped to them so the world under the rest still takes
/// clicks, and the pages it frames (Kiln, the games) are stood in their
/// boxes. A game in Game Lab's panel that takes the keyboard as it loads
/// hands it back to the chat.
///
/// First, unless the browser sends Do Not Track or Global Privacy Control,
/// it starts the visit's events (#12439, [`crate::analytics::visits`]): a
/// random visit id in this tab's `sessionStorage`, each event numbered
/// and queued, and the queue sent to `/a/v` every 20 seconds, when the tab
/// is hidden, and when the page is left. Zone trips come from Verse's
/// `verse-zone-load` events (#12438); `globalThis.oaTrack(name, detail)`
/// is how Verse, the Pylons layer, Game Lab, and the chat add the rest.
const LOADER: &str = concat!(
    "\
const track=(()=>{const nav=navigator;\n\
if(nav.doNotTrack===\"1\"||window.doNotTrack===\"1\"||nav.globalPrivacyControl===true||typeof nav.sendBeacon!==\"function\"||!window.crypto)return()=>{};\n\
const keep=(k,v)=>{try{if(v===undefined)return sessionStorage.getItem(k);sessionStorage.setItem(k,v)}catch(e){}return null};\n\
let id=keep(\"oa.visit\");\n\
if(!/^[0-9a-f]{16}$/.test(id||\"\")){id=Array.from(crypto.getRandomValues(new Uint8Array(8)),b=>b.toString(16).padStart(2,\"0\")).join(\"\");keep(\"oa.visit\",id);keep(\"oa.visit.n\",\"0\")}\n\
let n=+keep(\"oa.visit.n\")||0,queue=[],shown=0,since=document.hidden?null:performance.now(),next=0;\n\
const marks=[1e3,3e3,1e4,3e4,6e4,18e4,6e5,18e5];\n\
const seen=()=>Math.round(shown+(since===null?0:performance.now()-since));\n\
const flush=()=>{while(queue.length){try{nav.sendBeacon(\"/a/v\",JSON.stringify({v:id,r:\"/\",e:queue.splice(0,50)}))}catch(e){queue=[]}}};\n\
const send=(name,detail)=>{n++;keep(\"oa.visit.n\",String(n));queue.push(detail?[name,n,detail]:[name,n]);if(queue.length>=50)flush()};\n\
setInterval(()=>{if(next<marks.length&&seen()>=marks[next]){while(next<marks.length&&seen()>=marks[next])next++;send(\"heartbeat\",{time:seen()})}flush()},20000);\n\
document.addEventListener(\"visibilitychange\",()=>{if(document.hidden){if(since!==null)shown+=performance.now()-since;since=null;send(\"tab_hidden\",{time:seen()});flush()}else since=performance.now()});\n\
addEventListener(\"pagehide\",()=>{send(\"tab_left\",{time:seen()});flush()});\n\
let zoneFrom=\"grid\";\n\
const zoneWord=z=>{z=String(z||\"\").replace(/^verse-/,\"\").replace(/-v1$/,\"\");return [\"grid\",\"lagrange1\",\"lagrange2\",\"everglade\",\"hosted\",\"physics-lab\",\"robot-lab\",\"water-lab\",\"coast\",\"crypt\",\"grove\",\"arena\",\"duel\",\"hand-duel\",\"meteor-stress-test\",\"meteor-showcase\"].includes(z)?z:\"other\"};\n\
addEventListener(\"verse-zone-unavailable\",e=>send(\"portal_entered\",{zone:zoneWord((e.detail||{}).zone),opened:\"no\"}));\n\
addEventListener(\"verse-zone-load\",e=>{const d=e.detail||{},z=zoneWord(d.zone),time=Math.round(+d.ms||0);\n\
if(d.direction===\"return\"){if(d.phase===\"ready\")send(\"plaza_returned\",{from:zoneFrom});else if(d.phase===\"failed\")send(\"zone_failed\",{zone:zoneFrom,reason:\"return\"});return}\n\
if(d.phase===\"start\"){zoneFrom=z;send(\"portal_entered\",{zone:z,opened:\"yes\"});send(\"zone_load_start\",{zone:z})}\n\
else if(d.phase===\"ready\")send(\"zone_ready\",{zone:z,time});else if(d.phase===\"failed\")send(\"zone_failed\",{zone:z,reason:\"load\"})});\n\
globalThis.oaTrack=(name,detail)=>{try{send(String(name),typeof detail===\"string\"?JSON.parse(detail||\"{}\"):detail)}catch(e){}};\n\
document.getElementById(\"home-account\").addEventListener(\"click\",e=>{const a=e.target.closest&&e.target.closest(\"a[href]\");\n\
if(!a)return;const h=a.getAttribute(\"href\");\n\
if(h.startsWith(\"/login\")||h.startsWith(\"/signup\")){keep(\"oa.visit.signin\",\"1\");send(h.startsWith(\"/login\")?\"login_clicked\":\"signup_clicked\");flush()}});\n\
globalThis.oaSignedIn=()=>{if(keep(\"oa.visit.signin\")===\"1\"){keep(\"oa.visit.signin\",\"0\");send(\"signin_completed\")}};\n\
send(\"page_view\");\n\
return send})();\n\
const still=matchMedia(\"(prefers-reduced-motion: reduce)\").matches;\
const idle=t=>new Promise(r=>(window.requestIdleCallback||(f=>setTimeout(f,1)))(r,{timeout:t}));\
const account=()=>fetch(\"/home/me\").then(r=>r.ok?r.json():null).then(me=>{if(!me||!me.signed_in)return;\
globalThis.oaSignedIn&&oaSignedIn();\
if(me.admin===true)globalThis.openagentsAdmin=true;\
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
let terminalNear=false,chatReady=false,chatField=null,chatLoad=null,chatAssets=null,focusPending=false,chatShown=false;\n\
const held=new Set(),blocked=new Set();\n\
const world=document.getElementById(\"everglade-canvas\");\n\
const syncFocus=()=>world.toggleAttribute(\"data-composing\",chatReady&&terminalNear&&document.activeElement===chatField);\n\
const syncChat=()=>{const show=terminalNear&&chatReady;\n\
if(show&&!chatShown)track(globalThis.openagentsChatDesk?\"help_desk_opened\":globalThis.openagentsChatForeman?\"foreman_opened\":\"terminal_opened\");chatShown=show;\n\
globalThis.openagentsChatOpen=show;\n\
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
const frames=new Map();\n\
globalThis.homeOverlay=json=>{let o;try{o=JSON.parse(json)}catch(e){return}\n\
const box=r=>{const [x,y,w,h]=r.map(Math.round);return`M${x} ${y}h${w}v${h}h${-w}z`};\n\
chatBox.style.clipPath=o.regions.length?`path(\"${o.regions.map(box).join(\"\")}\")`:\"inset(50%)\";\n\
const live=new Set();\n\
for(const f of o.frames){live.add(f.id);let el=frames.get(f.id);const shown=!el||el.hidden;\n\
if(!el){el=document.createElement(\"iframe\");el.id=f.id+\"-frame\";el.className=\"home-frame\";el.title=f.title;el.allow=\"fullscreen\";\n\
if(f.keep)el.dataset.keep=\"1\";\n\
el.addEventListener(\"load\",()=>{try{el.contentWindow.addEventListener(\"keydown\",e=>{if(e.key===\"Escape\")dispatchEvent(new KeyboardEvent(\"keydown\",{key:\"Escape\"}))},true)}catch(e){}\n\
if(!f.keep){const back=()=>{if(document.activeElement===el&&chatField&&!chatField.disabled)chatField.focus({preventScroll:true})};setTimeout(back);setTimeout(back,800);setTimeout(back,2000)}});\n\
frames.set(f.id,el);document.body.append(el)}\n\
if(el.getAttribute(\"src\")!==f.src)el.src=f.src;el.hidden=false;\n\
Object.assign(el.style,{left:f.x+\"px\",top:f.y+\"px\",width:f.w+\"px\",height:f.h+\"px\"});\n\
if(shown&&f.keep)el.focus()}\n\
for(const [id,el] of frames)if(!live.has(id)){if(el.dataset.keep)el.hidden=true;else{el.remove();frames.delete(id)}}};\n\
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
composer.classList.add(\"ready\");document.documentElement.classList.add(\"gpui-ready\");\n\
chatStatus.textContent=\"\";\n\
performance.mark(\"home-chat-ready\");\n\
syncChat();ok()},{once:true});\n\
performance.mark(\"home-chat-mounting\");glue.mount(still)}));\n\
return glue})().catch(error=>{chatStatus.textContent=\"Chat couldn't open. Reload to try again.\";\n\
console.warn(error);track(\"terminal_failed\");focusPending=false;syncFocus();throw error}));\n\
globalThis.homeTerminal=(near,step)=>{const walked=near&&step!==false&&globalThis.openagentsLab!==\"game\"&&!globalThis.openagentsChatDesk&&!globalThis.openagentsChatForeman;near=(globalThis.openagentsLab===\"game\"||near)&&world.classList.contains(\"ready\");\n\
if(!near&&(globalThis.openagentsChatDesk||globalThis.openagentsChatForeman)){globalThis.openagentsChatDesk=false;globalThis.openagentsChatForeman=false;composer.setAttribute(\"aria-label\",\"Chat with the pylons\")}\n\
if(walked&&near&&!terminalNear)track(\"terminal_reached\",{time:Math.round(performance.now())});terminalNear=near;focusPending=near&&step!==false;\n\
syncChat();\n\
if(near)loadChat().catch(()=>{})};\n\
globalThis.homeDesk=open=>{open=!!open&&world.classList.contains(\"ready\");\n\
if(open){globalThis.openagentsChatForeman=false;globalThis.openagentsChatDesk=true;composer.setAttribute(\"aria-label\",\"Ask OpenAgents\")}\n\
homeTerminal(open)};\n\
globalThis.homeForeman=open=>{open=!!open&&world.classList.contains(\"ready\");\n\
if(open){globalThis.openagentsChatDesk=false;globalThis.openagentsChatForeman=true;composer.setAttribute(\"aria-label\",\"Ask the Foreman\")}\n\
homeTerminal(open)};\n\
world.addEventListener(\"pointerdown\",()=>{focusPending=false;\n\
if(chatField&&document.activeElement===chatField)chatField.blur();\n\
world.focus({preventScroll:true});syncFocus()},true);\n\
addEventListener(\"keydown\",e=>{if(e.key===\"Escape\"&&globalThis.openagentsKilnOpen){e.preventDefault();e.stopImmediatePropagation();homeKiln(false);return}\n\
if(e.key===\"Escape\"&&terminalNear){if(globalThis.openagentsLab===\"game\")return;e.preventDefault();\n\
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
const verse=async()=>{let stage=\"download\",gpu=\"webgl\",shown=false;try{await idle(800);\
gpu=await variant();\
performance.mark(\"verse-start\");\
track(\"verse_load_start\",{gpu});\
addEventListener(\"verse-stopped\",()=>track(\"verse_load_failed\",{gpu,reason:shown?\"stopped\":\"gpu\"}),{once:true});\
const canvas=document.getElementById(\"everglade-canvas\"),status=document.getElementById(\"everglade-status\");\
const drawn=new Promise(r=>new MutationObserver((list,observer)=>{if(status.style.display===\"none\"){observer.disconnect();\
r()}}).observe(status,{attributes:true,attributeFilter:[\"style\"]}));\
const [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch(\"/everglade/everglade_web_bg.wasm\",{priority:\"low\"})),import(\"/everglade/everglade_web.js\")]);\
performance.mark(\"verse-compiled\");\
stage=\"start\";\
prepareChat().catch(()=>{});\
await idle(600);\
await glue.default({module_or_path:module});\
performance.mark(\"verse-instantiated\");\
await drawn;\
track(\"verse_first_frame\",{gpu,time:Math.round(performance.now())});\
await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));\
performance.mark(\"verse-first-frame\");\
canvas.addEventListener(\"transitionend\",()=>performance.mark(\"verse-visible\"),{once:true});\
canvas.classList.add(\"ready\");\
document.documentElement.classList.add(\"world-ready\");\
shown=true;track(\"verse_ready\",{gpu,time:Math.round(performance.now())});\
idle(1200).then(()=>loadChat()).catch(()=>{});\
addEventListener(\"verse-stopped\",()=>{canvas.classList.remove(\"ready\");document.documentElement.classList.remove(\"world-ready\");homeGameLab(false);homeTerminal(false)},{once:true})}catch(e){track(\"verse_load_failed\",{gpu,reason:stage})}};\
account();\
loadLayer().then(verse);\
",
    include_str!("grid_kiln.js"),
    include_str!("../game_frame_watch.js"),
    include_str!("grid_game_lab.js")
);

/// The loader, for the analytics tests.
#[cfg(test)]
pub(crate) fn loader() -> &'static str {
    LOADER
}

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
    format!(
        "{}<section id=\"pylons-panel\" aria-labelledby=\"pylons-count\">{list}</section>{AFTER_LIST}",
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

/// `{signed_in, name, profile, chat, verse_name, csrf, admin}` for the top bar
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
            // A site admin's terminal offers Kiln (#12440).
            "admin": viewer.admin,
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
    fn production_hides_the_account_links_and_game_lab_is_in_the_world() {
        let html = main_html("<ul></ul>", false);
        assert!(html.contains("<nav id=\"home-account\" aria-label=\"Account\"></nav>"));
        assert!(!html.contains(">Log in</a>") && !html.contains(">Sign up</a>"));
        // Game Lab has no button over the world: it opens under its arch
        // (#12459). Its section keeps the sign-in path for a keyboard.
        assert!(html.contains("id=\"game-lab-signin\" href=\"/login?return_to=%2F%3Flab%3Dgame\""));
        assert!(!html.contains("game-lab-open") && !html.contains("href=\"/?lab=game\""));
        assert!(!html.contains("data-off"));
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
        assert!(html.contains("aria-describedby=\"grid-controls\""));
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
    fn kiln_opens_in_a_panel_only_for_a_site_admin() {
        // The chat draws Kiln's panel and asks the page to stand Kiln's
        // page in it; only a `/home/me` that says admin lets it open, and
        // the server refuses `/admin/kiln/` to everyone else (#12440).
        for account in [true, false] {
            let html = main_html("<ul></ul>", account);
            assert!(!html.contains("kiln-panel") && !html.contains("/admin/kiln/"));
        }
        assert!(LOADER.contains("if(me.admin===true)globalThis.openagentsAdmin=true;"));
        assert!(LOADER.contains("globalThis.homeKiln"));
        assert!(LOADER.contains("open = !!open && globalThis.openagentsAdmin === true;"));
        assert!(LOADER.contains("globalThis.homeOverlay"));
        assert!(STYLE.contains(
            "html:has(#kiln-frame:not([hidden])) \
             :is(#pylons-field,#pylon-card){visibility:hidden!important}"
        ));
    }

    #[test]
    fn nothing_but_the_world_and_the_gpui_canvas_shows_once_they_are_up() {
        // #12459: the header shows only until the chat draws its top bar,
        // and again while it has the keyboard's focus.
        assert!(STYLE.contains(".reveal,.gpui-ready header{clip-path:inset(50%)}"));
        assert!(
            STYLE.contains(".reveal:focus-within,.gpui-ready header:focus-within{clip-path:none}")
        );
        assert!(LOADER.contains("document.documentElement.classList.add(\"gpui-ready\")"));
        // Verse draws the machines' labels; the HTML ones are hidden.
        assert!(STYLE.contains("#pylons-field.bay .pylon .tag{display:none}"));
        // The pylon card's panel is GPUI's; the HTML one is the fallback.
        assert!(STYLE.contains("#pylon-card:not(.ready){background:"));
        assert!(
            STYLE.contains("#pylon-card.ready #pylon-close{color:transparent;background:none}")
        );
        // The canvas takes no clicks until the chat says where it draws.
        assert!(STYLE.contains("clip-path:inset(50%)}"));
        assert!(LOADER.contains("chatBox.style.clipPath=o.regions.length?"));
        assert!(!LOADER.contains("pylons-keep\",()=>{const r=keep"));
        // No Game Lab card frame, walking hint, or Kiln chrome in HTML.
        let html = main_html("<ul></ul>", true);
        for gone in [
            "game-lab-card",
            "game-lab-heading",
            "kiln-heading",
            "kiln-close",
        ] {
            assert!(!html.contains(gone), "{gone}");
            assert!(!STYLE.contains(gone), "{gone}");
        }
        assert!(html.contains("<section id=\"game-lab-panel\" class=\"reveal\""));
        assert!(html.contains("<button id=\"game-lab-back\" class=\"reveal\""));
        assert!(html.contains("<p id=\"grid-controls\" class=\"sr\">"));
        // Verse opens Game Lab under its arch; the chat draws its panel.
        assert!(LOADER.contains("globalThis.homeGameLab = open =>"));
        assert!(LOADER.contains("globalThis.homeGamePlay = src =>"));
        assert!(LOADER.contains("globalThis.openagentsChatOpen=show;"));
    }

    #[test]
    fn the_pylon_overlay_hides_away_from_the_grid_plaza() {
        // Verse marks the root with where the player is (#12438): the
        // pillars and labels show on the plaza only.
        let rule = "html:is([data-verse-place=zone],[data-verse-place=loading]) \
                    #pylons-field{visibility:hidden!important}";
        assert!(STYLE.contains(rule));
        assert!(STYLE.contains(".world-ready #pylons-field{visibility:visible}"));
        // Game Lab's full-screen Play view hides them too.
        assert!(STYLE.contains(
            "html:has(#game-lab-play:not([hidden])) \
             :is(#pylons-field,#pylon-card){visibility:hidden!important}"
        ));
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
