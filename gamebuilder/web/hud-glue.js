// Firm reason: 1 (browser glue: starts the page's module and tells the HUD where the page's panels are; no logic)
// The page (page.coax, every address of the site) is drawn by the browser; the world and the HUD (hud.coax, a
// Metaverse Engine scene) are drawn by the GPU on the canvas under it. Each frame this tells the HUD the box of
// every element with the `hud` class and the frame kind its `kN` class names
// (message 100), the pointer's place (101), how far down the page the window
// is (102), which view is shown (103: 1 the login, 2 the world), the keys
// held (104), the world's things (105), whether a press began on one of the
// page's own things (107), the wheel (108), whether the caret is in a field (112: first person's held pointer is let go) and the hotbar's state (113: the tool, edit mode, the selection, gravity, the follow camera); and it puts where the HUD says the
// character is, and where it says things came to rest, into the page's hidden fields. The window's pointers go to the HUD from the page host
// itself (coax/web/host.js); what the camera does with any of it is the HUD's (hud.coax). The world opens with the caret in the chat's box; Escape takes it
// out (the keys are then the character's: W A S D, Space to jump; the arrows turn and tilt the camera,
// = and - zoom it) and Enter puts it back.
// It also says how the world comes up (117: settled, with no entrance, for a person who prefers reduced motion or a kept world
// come back) and tells the page whether this browser has WebGPU at all (the hidden field "gpu"), so the page can say so in words.
// And it gives the page the browser's clock for counting the core loop's steps, and keeps the tab's note of them (below: countTell).
// What is drawn is the HUD's.
// The page's own module (page.coax, drawn by coax/web/dom-host.js) starts beside this file: web/app.js
// loads it first, then this glue, then the GPU host, as one file (web/build.sh).
// Coax's allowlisted own-frame report; a getter with no mutation path into the game.
let ownObservation = null;
const observeRequested = new URL(location.href).searchParams.get("observe") === "1";
Object.defineProperty(globalThis, "gamebuilderObservation", { get: () => ownObservation });
// Whether this browser can run the builder: it is drawn with WebGPU. The glue only asks the browser and
// tells the page which way it is missing, as one digit in the page's hidden field "gpu" (web/gpu-words.coax:
// 0 fine or not yet known, 1 no WebGPU, 2 WebGPU and no adapter, 3 both and the device did not start: the
// host takes the page's "gpu" class off when its start fails, coaxial/host.js); what is said is the page's.
let adapter = -1, wayTold = -1;
if (navigator.gpu) navigator.gpu.requestAdapter().then((a) => { adapter = a ? 1 : 0; }, () => { adapter = 0; });
const gpuWay = () => !navigator.gpu ? 1 : adapter === 0 ? 2 : adapter === 1 && (!document.documentElement.classList.contains("gpu") || document.getElementById("coaxial-load")?.dataset.state === "fail") ? 3 : 0;
const tellGpu = () => {
  const way = gpuWay(), field = document.querySelector("input.gs-gpu[data-slot]");   // (the page's module's own node: index.html's first-paint copy has no data-slot and hears nothing)
  if (field && way !== wayTold) { field.tabIndex = -1; field.value = String(way); field.dispatchEvent(new Event("input", { bubbles: true })); wayTold = way; }
  // (the page says it in its own words: the host's own note and its loading line step aside for them)
  if (way && document.querySelector(".nogpu")) for (const id of ["coaxial-no-gpu", way === 3 ? "" : "coaxial-load"]) { const el = id && document.getElementById(id); if (el && !el.hidden) el.hidden = true; }
};
// The loading line (index.html, #coaxial-load: the host writes how far the world's code has come, in words)
// goes once the world is drawing. The host itself keeps it until every asset is in, and the village's pack
// is held back until that type is picked: that wait is not the page's.
let drawn = 0;
const loaded = () => {
  const el = document.getElementById("coaxial-load"), c = globalThis.coaxial;
  if (!el || el.dataset.state === "fail" || el.classList.contains("gone")) return;
  // (drawing: the scene has been drawn, or the host has been past its GPU and shaders, at its assets, for a second)
  if (c && c.ready && (c.sceneReady || /^(assets|textures|world)/i.test(el.querySelector(".lt")?.textContent || "") && ++drawn > 60)) { el.classList.add("gone"); el.setAttribute("aria-valuenow", "100"); }
};
// The core loop's steps, counted (issue 18). What is counted, when, and what is sent are the page's (web/count.coax,
// over the one list in shared/loop.coax); the request is the page's own, through its host. The page has no clock and
// does not outlive a load, so the glue gives it three things and decides nothing: the browser's clock and what the
// browser knows of this page, as numbers, when the page's note asks ("NOW DOC PAINT FLAGS MASK LAST": count.coax
// says what each is); the page's note of what this tab has counted, kept in the tab's sessionStorage (it goes with
// the tab, is sent nowhere, and names no one: there is no id in it or anywhere); and, when the page is left with a
// count unsent, the page's own body to the browser's beacon, at the page's own door.
let countNote = null, countAsked = null, countWoke = null;
const countKept = (key, value) => { try { if (value === undefined) return sessionStorage.getItem(key); sessionStorage.setItem(key, value); } catch {} return null; };
const countNodes = () => [document.querySelector(".gs-count[data-slot]"), document.querySelector("input.gs-count-in[data-slot]")];
const countSay = (field, text) => { field.tabIndex = -1; field.value = text; field.dispatchEvent(new Event("input", { bubbles: true })); };
// (the tab's clock starts where its first page was asked for)
const countStart = (() => { const mine = Math.round(performance.timeOrigin), kept = Number(countKept("gb-t0")); if (kept > 0 && kept <= mine) return kept; countKept("gb-t0", String(mine)); return mine; })();
const countFlags = () => {
  let fromHome = 0; try { const r = new URL(document.referrer); fromHome = r.origin === location.origin && r.pathname === "/" ? 1 : 0; } catch {}
  // (a browser a program drives, and any copy of the site that is not the site, count nothing: ?count=1 lets a copy be tried)
  return fromHome | (navigator.webdriver ? 2 : 0) | (location.hostname !== "gamebuilder.org" && new URL(location.href).searchParams.get("count") !== "1" ? 4 : 0);
};
const countTell = () => {
  const [out, field] = countNodes();
  if (!out || !field) return;
  const note = (out.textContent || "").split("\n", 1)[0], [mask, last, asks, wake] = note.split(" ");
  if (asks === undefined) return;
  // the note changed after the page was first answered: it is this tab's to keep
  if (note !== countNote) { if (countNote !== null && countAsked !== null) countKept("gb-count", `${mask} ${last}`); countNote = note; }
  const doc = Math.round(performance.now());
  // (asked anew, or the page said when to say the clock once more: the first load is over by then)
  const due = Number(wake) > 0 && doc >= Number(wake) && wake !== countWoke;
  if (asks === countAsked && !due) return;
  if (due) countWoke = wake;
  countAsked = asks;
  const paint = performance.getEntriesByName("first-contentful-paint")[0];
  countSay(field, `${Math.round(performance.timeOrigin) - countStart + doc} ${doc} ${paint ? Math.round(paint.startTime) : -1} ${countFlags()} ${countKept("gb-count") || "0 -1"}`);
};
addEventListener("pagehide", () => {
  const [out, field] = countNodes(), [, door, body] = (out?.textContent || "").split("\n");
  if (field && door && body && navigator.sendBeacon && navigator.sendBeacon(door, body)) countSay(field, "left");
});
// Whether the person asked the browser for less motion: the world then comes up settled, with no entrance.
const calmly = matchMedia("(prefers-reduced-motion: reduce)");
// The ring of portals (issue 30; web/ring.coax, hud.coax "the home page's ring of portals"; docs/web-world.md, "The
// ring"). The page's class says "ring" where the game types are picked (the home page, and the builder before a type
// is): the HUD is told to raise the ring in the meadow (120 "1"), and that it is left (120 "0 KIND": the type picked,
// whose portal the view goes through, or -1). The HUD says where each portal's name goes on the screen and which
// portal is lit (its action 123: "STATE LIT", then "X Y WIDE" a portal, CSS pixels, WIDE -1 out of view); the glue
// puts that on the page's root as numbers the stylesheet reads (--rIx, --rIy, data-lit) and says the world is up
// ("ringed": the names stand over their portals; look.coax). A portal taken in the world (124: its place) is a
// press on its name, as a press on the name is: what a press does is the page's. The pictures the portals show are
// the page's own (its hidden line, a path a portal): the glue has the browser decode them and gives the HUD their
// texels (122), some rows at a time. Which portal is which, and what is lit, is the HUD's.
let ringUp = false, ringPics = null, ringState = -1;
const ringNames = () => [...document.querySelectorAll(".names[data-slot] .kind")];
const ringRoot = document.documentElement;
const ringClear = () => {
  ringRoot.classList.remove("ringed"); ringRoot.removeAttribute("data-lit");
  for (let i = 0; i < 16; i++) { ringRoot.style.removeProperty(`--r${i}x`); ringRoot.style.removeProperty(`--r${i}y`); }
};
const ringReport = (cls) => {
  const asking = /\basking\b/.test(cls), kind = /\bg(\d)\b/.exec(cls)?.[1];
  const shown = /\bring\b/.test(cls) && !asking;
  if (shown && !ringUp) { if (say(120, "1")) { ringUp = true; ringState = -1; kindTold = null; } }
  // (left: once the page says what comes next, a type or another page; a moment of "pick" with no type is the press on its way)
  else if (!shown && ringUp && (kind !== undefined || !/\bpick\b/.test(cls) || asking)) { if (say(120, `0 ${kind ?? -1}`)) { ringUp = false; } }
  if (ringUp && !ringPics) ringPicsLoad();
  ringPicsSend();
};
// The pictures, as the HUD holds them (web/ring.coax RG_PIC_W by RG_PIC_H, filled): "PORTAL ROW ROWS" and six hex digits a texel.
const RING_W = 256, RING_H = 144, RING_ROWS = 40;
const ringPicsLoad = () => {
  const said = document.querySelector(".gs-ring[data-slot]")?.textContent || "";
  const paths = said.split("\n").filter((p) => /^\/img\/[a-z0-9-]+\.jpg$/.test(p));
  if (!paths.length) return;
  ringPics = { queue: [], paths };
  paths.forEach((path, i) => {
    const img = new Image();
    img.decoding = "async";
    img.onload = () => {
      const c = document.createElement("canvas"); c.width = RING_W; c.height = RING_H;
      const g = c.getContext("2d", { willReadFrequently: true });
      // (the picture is 640 by 360, two and a half texels to one: the browser's own filter for a cut that
      // far, not its plain bilinear one, which skips texels and leaves the grass sparkling)
      g.imageSmoothingEnabled = true; g.imageSmoothingQuality = "high";
      // (cover: the picture fills the portal, cut evenly at whichever sides it is longer, never stretched)
      const s = Math.max(RING_W / img.naturalWidth, RING_H / img.naturalHeight), w = img.naturalWidth * s, h = img.naturalHeight * s;
      g.drawImage(img, (RING_W - w) / 2, (RING_H - h) / 2, w, h);
      const px = g.getImageData(0, 0, RING_W, RING_H).data;
      for (let row = 0; row < RING_H; row += RING_ROWS) {
        const rows = Math.min(RING_ROWS, RING_H - row); let text = `${i} ${row} ${rows}\n`;
        for (let k = row * RING_W * 4, end = (row + rows) * RING_W * 4; k < end; k += 4) text += ((1 << 24) | (px[k] << 16) | (px[k + 1] << 8) | px[k + 2]).toString(16).slice(1);
        ringPics.queue.push(text);
      }
    };
    // beside the script, as the hosts find their modules (../img/: /img/ here, /b/NAME/img/ in a release, kept for good)
    img.src = new URL(`../img/${path.slice(5)}`, import.meta.url).href;
  });
};
const ringPicsSend = () => { if (!ringPics) return; for (let k = 0; k < 2 && ringPics.queue.length; k++) { if (!say(122, ringPics.queue[0])) return; ringPics.queue.shift(); } };
const ringSays = (text) => {
  const lines = text.split("\n"), [state, lit] = (lines[0] || "").split(" ").map(Number);
  ringState = state;
  // gone (2): the type picked is told again, now its world can come (the HUD held it while the view went through)
  if (state === 2) { kindTold = null; ringClear(); return; }
  if (state !== 0) { ringClear(); return; }
  let seen = 0;
  for (let i = 1; i < lines.length && i <= 16; i++) {
    const [x, y, wide] = lines[i].split(" ").map(Number);
    if (!(wide > 0)) { ringRoot.style.removeProperty(`--r${i - 1}x`); continue; }
    ringRoot.style.setProperty(`--r${i - 1}x`, `${x}px`); ringRoot.style.setProperty(`--r${i - 1}y`, `${y}px`); seen++;
  }
  ringRoot.setAttribute("data-lit", String(lit));
  if (seen && ringUp) ringRoot.classList.add("ringed");
};
addEventListener("coaxial", (e) => {
  const code = e.detail?.code;
  if (code === 123) ringSays(e.detail.text || "");
  else if (code === 124 && ringUp) ringNames()[Number(e.detail.text)]?.click();
});
const say = (kind, text) => { const c = globalThis.coaxial; if (c && c.ready && c.send) { c.send(kind, text); return true; } return false; };
const report = () => {
  tellGpu();
  countTell();
  loaded();
  if (observeRequested) say(116, "1");
  say(140, "1");   // the megalith stands in the third- and first-person world (hud.coax HUD_PEAK_MSG)
  const panels = document.querySelectorAll(".hud");
  let text = panels.length + "\n";
  for (const el of panels) {
    const r = el.getBoundingClientRect(), k = /\bk(\d)\b/.exec(el.className);
    text += `${k ? k[1] : 3} ${Math.round(r.left)} ${Math.round(r.top)} ${Math.round(r.width)} ${Math.round(r.height)}\n`;
  }
  say(100, text);
  const room = document.documentElement.scrollHeight - innerHeight;
  say(102, `${room > 0 ? Math.round(scrollY / room * 1000) : 0} ${Math.round(scrollY)}`);
  // which view the page shows is in its class (page.coax: "gs v0" home, "gs v1" login, "gs v2" the world)
  // and, logged in, the game type (g0 to g3: the HUD's camera kinds) or that one is being picked: the
  // picker is the login's pose to the HUD, with the keys the picker's own
  const cls = document.querySelector(".gs")?.className || "";
  ringReport(cls);
  picking = /\bpick\b/.test(cls) && !ringUp;
  view = ringUp ? 2 : picking ? 1 : Number(/\bv(\d)\b/.exec(cls)?.[1] || 0);
  if (view !== 2) ownObservation = null;
  // how the world comes up (117), said before the type and the view: settled, with no entrance, for a
  // person who prefers reduced motion (1) or when the page's class says the world is a kept one come back (2: "still")
  say(117, String((calmly.matches ? 1 : 0) | (/\bstill\b/.test(cls) ? 2 : 0)));
  const kind = /\bg(\d)\b/.exec(cls)?.[1];
  if (kind !== undefined && kind !== kindTold && say(111, kind)) { kindTold = kind; try { localStorage.setItem("gb-kind", kind); } catch {} }
  say(103, String(view === 3 ? 0 : view));   // the credits page is the home page to the HUD: the flat grid, no world
  // the picker, as it appears: the type this browser picked last goes into its hidden field (a
  // convenience: the page lights that one), and the lit button, else the first, takes the keys' focus
  // (at the ring the keys are the ring's, and nothing takes the focus: Return enters the portal lit)
  if (!(picking || ringUp) || /\basking\b/.test(cls)) shownPicker = false;
  else if (!shownPicker && document.querySelector(".kind")) {
    shownPicker = true;
    let last = null; try { last = localStorage.getItem("gb-kind"); } catch {}
    const field = document.querySelector("input.gs-last");
    if (field && last !== null) { field.tabIndex = -1; field.value = last; field.dispatchEvent(new Event("input", { bubbles: true })); }
    if (!ringUp) requestAnimationFrame(() => (document.querySelector(".kind.on") || document.querySelector(".kind"))?.focus());
  }
  // logged out, the choice is forgotten
  if (!/\bv[23]\b/.test(cls) && !/\basking\b/.test(cls) && cls) { kindTold = null; try { if (localStorage.getItem("gb-kind") !== null) localStorage.removeItem("gb-kind"); } catch {} }
  // the world opens with the keys at the character's feet: the chat's box takes the caret only on Enter
  // the keys held are the character's while the world is shown and nothing is being typed
  const typing = typed_in(document.activeElement);
  keys();
  // the caret in a field: the HUD lets a held pointer go (112; first person holds it: hud.coax)
  if (typing !== typed && say(112, typing ? "1" : "0")) typed = typing;
  // the hotbar's state, as the page wrote it in a hidden element (page.coax): told when it changes
  const state = view === 2 ? document.querySelector(".gs-state")?.textContent || "" : "";
  if (state && state !== stateTold && say(113, state)) stateTold = state;
  if (!state) stateTold = "";
  // the blocks that stand in the world: the page writes them in a hidden element (web/blocks.coax);
  // told when they change, and again now and then, so a HUD that was reloaded has them
  // (crates: which level of its game the room is, said first (118): another room's things are a new room to the HUD)
  const list = document.querySelector(".gs-blocks")?.textContent || "0\n";
  const roomNow = view === 2 ? document.querySelector(".gs-room")?.textContent || "" : "";
  if (list !== blocks || roomNow !== roomTold || ++since > 120) { if (roomNow) say(118, roomNow); if (say(105, list)) { blocks = list; roomTold = roomNow; since = 0; } }
  if (view === 2) keepBar();
};
// The hotbar's order, kept in this browser for each game type (a convenience: the page checks what it is
// given against its catalogue and drops what it does not know). The page writes "KIND\nids", or
// "KIND\ndefault" while the bar is the type's own; the first time a type is shown its kept order goes into
// the hidden field "bar" ("KIND ids": a field keeps no line break), and from then on what the page writes is kept (a reset forgets it).
const barLoaded = new Map();
const keepBar = () => {
  const text = document.querySelector(".gs-bar")?.textContent || "";
  const cut = text.indexOf("\n");
  if (cut < 1) return;
  const kind = text.slice(0, cut), ids = text.slice(cut + 1), name = "gb-bar-" + kind;
  const was = barLoaded.get(kind);
  if (was === undefined) {
    let kept = null; try { kept = localStorage.getItem(name); } catch {}
    const field = document.querySelector("input.gs-bar-in");
    if (kept !== null && field) { field.tabIndex = -1; field.value = kind + " " + kept; field.dispatchEvent(new Event("input", { bubbles: true })); }
    barLoaded.set(kind, { told: kept === null ? "default" : kept, wait: kept === null ? 0 : 30 });
    return;
  }
  // (a kept order just given: what the page writes is the old one for a frame or two, and is not kept)
  if (was.wait > 0 && ids !== was.told) { was.wait--; return; }
  was.wait = 0;
  if (ids === was.told) return;
  try { if (ids === "default") localStorage.removeItem(name); else localStorage.setItem(name, ids); } catch {}
  was.told = ids;
};
// Where the character is, from the HUD (its action 106, "X Z FX FZ"), where things came to rest
// (110) and what the HUD says the world is (115: a game type that is its own world): into the page's hidden fields, as if typed, so the page's module hears them. The fields
// never take the caret.
addEventListener("coaxial", (e) => {
  const code = e.detail?.code;
  if (code === 119) { try { ownObservation = Object.freeze(JSON.parse(e.detail.text)); } catch { ownObservation = null; } return; }
  // (and an edit a tool made, 114: a plan line for the page's block list; the same edit twice is two edits)
  if (code !== 106 && code !== 110 && code !== 114 && code !== 115) return;
  const field = document.querySelector(code === 106 ? "input.gs-where" : code === 110 ? "input.gs-things" : code === 114 ? "input.gs-edit" : "input.gs-note");
  if (!field || (code !== 114 && field.value === e.detail.text)) return;
  field.tabIndex = -1;
  field.value = e.detail.text;
  field.dispatchEvent(new Event("input", { bubbles: true }));
});
// Every frame, and at once on a scroll or a resize (those come before the frame's
// drawing), so the frames move with the page and not a frame behind it.
const tick = () => { report(); requestAnimationFrame(tick); };
requestAnimationFrame(tick);
// Typing in a field is the browser's own: paste, copy, cut, select all, undo. The GPU host
// (host.js) listens on the window for keys and paste to hand to its module, and would take a
// paste for itself; in a field of the page it never hears of them.
// The keys held, told to the HUD the moment they change (and each frame, should a word have been
// lost): a tap shorter than a frame is still a press and a release there, in order.
const keys = () => {
  // (Shift over the hotbar or its palette, or while a slot is dragged, is the drag's, never the character's descent)
  const bits = view === 2 && !typed_in(document.activeElement) ? held & ~(overBar || drag ? 2048 : 0) : 0;
  if (bits !== told && say(104, String(bits))) told = bits;
};
// A field that takes typing: the chat's box, the login's. A slider is not one: with it under the
// keys' focus the keys are still the character's and the hotbar's.
const typed_in = (el) => /^(INPUT|TEXTAREA)$/.test(el?.tagName || "") && el.type !== "range";
const inField = (e) => /^(INPUT|TEXTAREA)$/.test(e.target?.tagName || "") || e.target?.isContentEditable;
for (const kind of ["paste", "copy", "cut"]) {
  // (the window's own listener, before the host's: the event goes no further, and the browser still does it)
  addEventListener(kind, (e) => { if (inField(e)) e.stopImmediatePropagation(); }, true);
}
// The keys the world reads, as the bits hud.coax names: forward 1, back 2, left 4, right 8, jump 16
// (Space: taken here, so the page doesn't scroll), and the camera's, as Everglade's: turn left 32 and
// right 64, tilt up 128 and down 256, zoom in 512 and out 1024; Shift 2048 (descend, while flying); and E 4096
// (the dragon breathes fire: no longer the hotbar's next tool, which Q still steps back through).
let overBar = false, drag = null;
let held = 0, told = -1, view = 0, blocks = null, since = 0, stateTold = "", roomTold = "", picking = false, kindTold = null, shownPicker = false, typed = null;
// (the chat's own box, by its place in the chat: a guest has none, and the login's fields over the world are not it)
const chatBox = () => document.querySelector("#coaxial-root .saybar input");
const bit = (e) => ({ KeyW: 1, KeyS: 2, KeyA: 4, KeyD: 8, Space: 16, ArrowLeft: 32, ArrowRight: 64, ArrowUp: 128, ArrowDown: 256, Equal: 512, NumpadAdd: 512, Minus: 1024, NumpadSubtract: 1024, ShiftLeft: 2048, ShiftRight: 2048, KeyE: 4096 })[e.code] || 0;
addEventListener("keydown", (e) => {
  const field = typed_in(e.target);
  // the ring's keys: 1 to 9 take that portal (its name pressed), the arrows turn to the next portal left or right, and
  // Return enters the one lit (a name with the keys' focus keeps Return: the browser presses it). W A S D walk.
  if (ringUp && !field && !e.metaKey && !e.ctrlKey && !e.altKey) {
    const names = ringNames();
    if (/^[1-9]$/.test(e.key) && names[Number(e.key) - 1]) { names[Number(e.key) - 1].click(); e.preventDefault(); return; }
    const turn = { ArrowLeft: "1", ArrowRight: "2" }[e.key];
    if (turn) { say(121, turn); e.preventDefault(); return; }
    if (e.key === "Enter" && !e.target?.closest?.(".kind")) { say(121, "3"); e.preventDefault(); return; }
  }
  // the picker's keys: 1 to 4 press a type; the arrows move between them (Tab does too, and Enter presses: the browser's own)
  if (picking && !e.metaKey && !e.ctrlKey && !e.altKey) {
    const kinds = [...document.querySelectorAll(".kind")];
    if (/^[1-9]$/.test(e.key) && kinds[Number(e.key) - 1]) { kinds[Number(e.key) - 1].click(); e.preventDefault(); return; }
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
    if (step && kinds.length) { kinds[(kinds.indexOf(document.activeElement) + step + kinds.length) % kinds.length].focus(); e.preventDefault(); }
    return;
  }
  if (view !== 2) return;
  if (e.key === "Shift") cue(e.shiftKey);
  // the palette "more" opened, open: its arrows move through it and Enter uses the item (the page's, as a key)
  if (!field && !e.metaKey && !e.ctrlKey && !e.altKey && document.querySelector(".hb-p")) {
    const key = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "enter" }[e.key];
    const to = key && document.querySelector("input.gs-hotkey");
    if (to) { to.tabIndex = -1; to.value = key; to.dispatchEvent(new Event("input", { bubbles: true })); e.preventDefault(); return; }
  }
  // Enter puts the caret in the chat's box; Escape gives the keys back to the character
  if (e.key === "Enter" && !field) { const box = chatBox(); if (box) { box.focus(); say(112, "1"); typed = true; e.preventDefault(); } return; }
  if (e.key === "Escape" && field) { e.target.blur(); return; }
  // Enter in the chat's box sends (the page's own handler) and then gives the keys back to the character
  if (e.key === "Enter" && field && e.target === chatBox()) { const box = e.target; setTimeout(() => box.blur(), 0); return; }
  // (Escape with a slider under the keys lets it go too, and then clears the hotbar as ever)
  if (e.key === "Escape" && document.activeElement?.type === "range") document.activeElement.blur();
  // a slider keeps its own arrows
  if (e.target?.type === "range" && /^Arrow/.test(e.key)) return;
  // undo and redo, as everywhere: Ctrl+Z or ⌘Z, and Ctrl+Shift+Z, ⇧⌘Z or Ctrl+Y. The page's (page.coax, web/undo.coax); in a
  // field they stay the browser's own, for what is typed there
  if (!field && (e.metaKey || e.ctrlKey) && !e.altKey && (e.code === "KeyZ" || e.code === "KeyY")) {
    const to = document.querySelector("input.gs-hotkey");
    if (to) { to.tabIndex = -1; to.value = e.code === "KeyY" || e.shiftKey ? "redo" : "undo"; to.dispatchEvent(new Event("input", { bubbles: true })); e.preventDefault(); return; }
  }
  // the hotbar's keys go to the page as they are (its hidden field): a slot's number, Escape, Delete, and Q and E (the shapes tool's shape) and Z and X (its mode), and R (a world's own action: crates resets its room).
  // What each does is the page's (page.coax, through the shared hotbar)
  if (!field && !e.metaKey && !e.ctrlKey && !e.altKey) {
    const key = /^[0-9]$/.test(e.key) ? e.key : e.key === "Escape" ? "esc" : e.key === "Delete" || e.key === "Backspace" ? "del" : e.code === "KeyQ" ? "q" : e.code === "KeyZ" ? "z" : e.code === "KeyX" ? "x" : e.code === "KeyR" ? (e.shiftKey ? "R" : "r") : "";
    const to = key && document.querySelector("input.gs-hotkey");
    if (to) { to.tabIndex = -1; to.value = key; to.dispatchEvent(new Event("input", { bubbles: true })); e.preventDefault(); return; }
  }
  if (field || e.metaKey || e.ctrlKey || e.altKey) return;
  const b = bit(e);
  if (b) { held |= b; if (e.code !== "ShiftLeft" && e.code !== "ShiftRight") e.preventDefault(); keys(); }
});
addEventListener("keyup", (e) => { held &= ~bit(e); if (e.key === "Shift") cue(false); keys(); });
const releaseInput = () => { held = 0; cue(false); if (drag) dragEnd(null); keys(); };
addEventListener("blur", releaseInput);
document.addEventListener("visibilitychange", () => { if (document.hidden) releaseInput(); });
addEventListener("scroll", report, { passive: true });
addEventListener("resize", report, { passive: true });
addEventListener("pointermove", (e) => say(101, `${Math.round(e.clientX)} ${Math.round(e.clientY)}`), { passive: true });
document.addEventListener("pointerleave", () => say(101, "gone"));
// The world's camera. Whether a press or the wheel is over one of the page's own things (the chat,
// the account menu, a link, a button, a field) is the page's to know; the HUD is told (107 before the
// press reaches it, and with the wheel), and decides. Over the world the browser keeps its hands
// off: the wheel scrolls nothing, the other button opens no menu, and a mouse held there stays
// the HUD's until it is let go, wherever it goes.
const own = (e) => !!e.target?.closest?.(".chat,.nav,.hotdock,.sheet,.playbar,a,button,input,textarea,select");
// A published game's link, shared (page.coax: the publish sheet's share button, shown only where the browser has a
// share sheet): the glue asks the browser, with the link the page wrote beside the button. What is shared is the page's.
if (navigator.share) document.documentElement.classList.add("can-share");
addEventListener("click", (e) => {
  if (!e.target?.closest?.(".gs-share")) return;
  const url = document.querySelector(".gs-link")?.textContent || "";
  if (url && navigator.share) navigator.share({ title: "GAMEBUILDER", url }).catch(() => {});
});
// A press on the hotbar or its panels' buttons does not take the keys' focus: they stay the character's
// (a slider keeps the browser's own press, so it can be dragged)
addEventListener("mousedown", (e) => { if (e.target?.closest?.(".hotdock") && !e.target.closest("input")) e.preventDefault(); }, true);
// A slider that was dragged does not keep the keys' focus: they go back to the character.
addEventListener("pointerup", () => { if (document.activeElement?.type === "range") document.activeElement.blur(); }, true);
addEventListener("pointerdown", (e) => {
  say(107, own(e) ? "1" : "0");
  if (view === 2 && !own(e) && e.pointerType === "mouse") { try { document.documentElement.setPointerCapture(e.pointerId); } catch {} }
}, true);
addEventListener("wheel", (e) => {
  if (view !== 2) return;
  if (!own(e)) e.preventDefault();
  say(108, `${Math.round(e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1))} ${own(e) ? 1 : 0}`);
}, { passive: false });
addEventListener("contextmenu", (e) => { if (view === 2 && !own(e)) e.preventDefault(); });
// Rearranging the hotbar: Shift held and a slot dragged (a long press, on a touch screen). The slot
// lifts, the others make room, and where it is let go is where it stands; let go away from the bar it
// is taken off (it is still in "more"). An item of the palette dragged onto the bar is put there (on a
// full bar it takes the place of the one it is dropped on); a slot dragged onto the palette is taken
// off. "more" itself never moves. The glue only shows the drag and says the result to the page (its
// hidden field "drag": "move FROM TO", "off FROM" or "add ITEM TO"); what the bar holds is the page's.
const barEl = () => document.getElementById("hot");
const slotsOf = () => [...(barEl()?.querySelectorAll(".hb-s") || [])].filter((el) => /^hot\.s\.\d+$/.test(el.id) && !el.classList.contains("hb-fix"));
const cue = (on) => { document.documentElement.classList.toggle("hb-shift", !!on && overBar); };
const liftable = (t) => {
  const s = t?.closest?.(".hb-s");
  if (s && /^hot\.s\.\d+$/.test(s.id) && !s.classList.contains("hb-fix")) return { from: Number(s.id.slice(6)), el: s };
  const p = t?.closest?.(".hb-pi");
  if (p && /^pal\.p\.\d+$/.test(p.id)) return { item: Number(p.id.slice(6)), el: p };
  return null;
};
const inBar = (x, y) => { const r = barEl()?.getBoundingClientRect(); return !!r && x >= r.left - 24 && x <= r.right + 24 && y >= r.top - 28 && y <= r.bottom + 20; };
const dragStart = (e, what) => {
  const slots = slotsOf(), rects = slots.map((el) => el.getBoundingClientRect());
  // (the ghost stands in a layer of its own: the dock is moved by a transform, which a fixed child would follow)
  const layer = document.createElement("div");
  layer.className = "hb-scifi";
  layer.style.cssText = "position:fixed;inset:0;display:block;margin:0;padding:0;border:0;background:none;box-shadow:none;transform:none;pointer-events:none;z-index:60";
  const ghost = document.createElement("div");
  ghost.className = "hb-s hb-ghost";
  const src = what.from !== undefined ? what.el : null;
  if (src) ghost.innerHTML = src.innerHTML;
  else { const g = what.el.querySelector(".hb-g"); if (g) ghost.appendChild(g.cloneNode(true)); }
  const r = what.el.getBoundingClientRect(), w = src ? r.width : rects[0]?.width || 56, h = src ? r.height : rects[0]?.height || 56;
  ghost.style.width = w + "px"; ghost.style.height = h + "px";
  layer.appendChild(ghost); document.body.appendChild(layer);
  if (src) src.classList.add("hb-lift");
  drag = { ...what, slots, rects, layer, ghost, w, h, to: -2, id: e.pointerId };
  try { what.el.setPointerCapture(e.pointerId); } catch {}
  keys(); dragMove(e);
};
// Where a drop at x would put it: among the bar's other slots, the number of them whose middle is to its left.
const dragMove = (e) => {
  const d = drag; if (!d) return;
  d.ghost.style.transform = `translate(${Math.round(e.clientX - d.w / 2)}px,${Math.round(e.clientY - d.h / 2)}px)`;
  const inside = inBar(e.clientX, e.clientY);
  d.ghost.classList.toggle("hb-out", d.from !== undefined && !inside);
  let to = -1;
  if (inside) { to = 0; d.rects.forEach((r, i) => { if (i !== d.from && e.clientX > r.left + r.width / 2) to++; }); }
  if (to === d.to) return;
  d.to = to;
  // the others slide to make room: a moved slot's neighbours by a slot each, an added item's gap by half a slot each side
  d.slots.forEach((el, i) => {
    let dx = 0;
    if (to >= 0 && d.from !== undefined) {
      if (i === d.from) dx = (d.rects[to]?.left ?? d.rects[i].left) - d.rects[i].left;
      else if (d.from < i && i <= to) dx = d.rects[i - 1].left - d.rects[i].left;
      else if (to <= i && i < d.from) dx = d.rects[i + 1].left - d.rects[i].left;
    } else if (to >= 0) dx = (i < to ? -1 : 1) * (d.w / 2 + 3);
    el.style.transform = dx ? `translateX(${Math.round(dx)}px)` : "";
  });
};
const dragEnd = (e) => {
  const d = drag; if (!d) return;
  drag = null;
  d.slots.forEach((el) => { el.style.transform = ""; el.classList.remove("hb-lift"); });
  d.layer.remove();
  keys();
  if (!e) return;
  const inside = inBar(e.clientX, e.clientY);
  const said = d.from !== undefined ? (inside ? (d.to !== d.from && d.to >= 0 ? `move ${d.from} ${d.to}` : "") : `off ${d.from}`) : (inside && d.to >= 0 ? `add ${d.item} ${d.to}` : "");
  const field = document.querySelector("input.gs-drag");
  if (said && field) { field.tabIndex = -1; field.value = said; field.dispatchEvent(new Event("input", { bubbles: true })); }
  // (the press that was a drag is no click on the slot or the item)
  addEventListener("click", (c) => { c.stopImmediatePropagation(); c.preventDefault(); }, { capture: true, once: true });
  setTimeout(() => { held_touch = false; }, 0);
};
let press = null, held_touch = false;
addEventListener("pointerdown", (e) => {
  if (view !== 2 || drag) return;
  const what = liftable(e.target);
  if (!what) return;
  if (e.pointerType === "mouse" && e.shiftKey && e.button === 0) { e.preventDefault(); e.stopImmediatePropagation(); dragStart(e, what); return; }
  // a long press on a touch screen lifts it as Shift would
  if (e.pointerType !== "mouse") {
    const at = { x: e.clientX, y: e.clientY };
    press = { at, timer: setTimeout(() => { press = null; held_touch = true; dragStart(e, what); }, 450) };
  }
}, true);
addEventListener("pointermove", (e) => {
  overBar = !!e.target?.closest?.("#hot,.hb-p");
  if (!drag) { cue(e.shiftKey); keys(); }
  if (press && Math.hypot(e.clientX - press.at.x, e.clientY - press.at.y) > 10) { clearTimeout(press.timer); press = null; }
  if (drag && e.pointerId === drag.id) { e.preventDefault(); dragMove(e); }
}, { capture: true, passive: false });
addEventListener("pointerup", (e) => { if (press) { clearTimeout(press.timer); press = null; } if (drag && e.pointerId === drag.id) dragEnd(e); }, true);
addEventListener("pointercancel", () => { if (press) { clearTimeout(press.timer); press = null; } if (drag) dragEnd(null); }, true);
// (while a long press drags, the page does not scroll under it)
addEventListener("touchmove", (e) => { if (drag || held_touch) e.preventDefault(); }, { passive: false });
