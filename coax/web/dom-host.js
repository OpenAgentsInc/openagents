// Firm reason: 1 (browser glue: applies the browser backend's commands to the page and reports presses, typing and sizes)
// The host for a text surface the browser draws (components/backend/dom.coax;
// docs/audits/2026-11-11-chat-zero-base-audit.md, sections 10 and 11). It
// loads the page's Wasm module, keeps its state, and each frame the module
// asks for applies the commands it wrote: make a node, place it, set its
// text, class or an attribute, draw its frame as SVG paths, take it away.
// It holds no logic: what is on the page, and what a press does, is the
// module's. It needs no WebGPU.
//   <script type="module" src="/coaxial/dom-host.js" data-app="kit" data-root="coaxial-root"></script>
// Clip mode, as host.js has it for pages the GPU draws (coax/web/clip.coax sets coaxialClip before the
// page loads; ?clip in the address does the same, for a picture at an exact moment): the page moves
// only when coaxialClipStep(frames, fps) says so, each frame exactly 1/fps of the module's time, its
// commands applied, its frames drawn for the sizes the page then has, and the stylesheet's own
// animations held at that same moment. A recording then has every frame however slowly it is captured.
// A page the GPU draws opens one over its canvas by importing this file with the module and the root
// in the address (host.js, the editor shell): /coaxial/dom-host.js?app=editor-shell&root=coaxial-editor.
// Each address is its own copy of this host, so a page may hold several. A module may also keep
// actions for the page that opened it (dom_act_kind, dom_act, dom_act_len, dom_act_done): the page
// reads them through coaxialDoms[app] and is told of each event by its `after`.
// A module that imports dom_keep(at, len) keeps one short text in this browser (localStorage
// coaxial.dom.<app>.kept, at most 4 KB: a setting, never a secret), and is given it back as event 12
// as it starts. Storage can be refused, so it is only a convenience.
// A module that imports dom_go(at, len, push) is an app shell (docs/app-shell.md): it is told the address as it
// starts and after Back or Forward (event 13, the path), and it may change it. A push keeps the page and puts
// the address in the bar (the module drew what is there); otherwise the browser goes to it. A press on a link
// in the module is reported as a press (event 1): when the module pushes in answer, the browser doesn't follow
// the link, so the shell stays; when it doesn't, the link is followed as usual.
// A page that holds a shell as one of its parts names the address the shell draws in the import's address
// (&path=/side, the world editor's left): the shell is told that address once and is not the page's, so every
// address it goes to is another page. Any other module opened with &path= is told it once too, as event 13 (a
// project's attachments, project-files-web, learn the project from /projects/SLUG).
// &as=NAME names the copy in coaxialDoms when a page holds two of one module.
// A module that exports dom_drive(state, len) and dom_drive_out(state) answers coax drive's operations itself
// (coaxialDom.drive(op)): coax/web/drive.coax sends a page's tree, state, clock and logs there, and reads what a
// real click or typing made of it with `input`.
// The app shell holds the pages it doesn't draw itself in a frame beside the side (shell.js): a page of the shell's
// (data-shell on its script) opened at the top of the window becomes the shell instead of starting its own module, and
// the shell opens itself by address with &top, the page's own shell. That shell is told the address with its query.
import "./tree-commands.js";
import { shellTakes, shellHost, shellHeld, shellGo, shellDrawn } from "./shell.js";
const named = new URL(import.meta.url).searchParams;
const script = named.has("app") ? null : document.querySelector("script[data-app]");
if (shellTakes(script)) await new Promise(() => {});   // this page is the shell's now: its own module never starts
const app = named.get("app") || script.dataset.app;
const top = !named.has("app") || named.has("top");   // the page's own module, not a part opened by another
const root = document.getElementById(named.get("root") || script?.dataset.root || "coaxial-root");
const fixed = named.get("path");
const NONE = 0xffff, SVG = "http://www.w3.org/2000/svg", LAYERS = ["fr-bg", "fr-line", "fr-deco"];
const text = new TextDecoder(), bytes = new TextEncoder();
const reduced = matchMedia("(prefers-reduced-motion: reduce)");
let memory, x, state = 0, els = [], frames = new Map(), asked = false, last = 0, pushed = false;
const clipping = globalThis.coaxialClip === true || new URLSearchParams(location.search).has("clip");
let clipNow = 0, clipReady = false;
const clipBorn = new WeakMap();

const sizes = new ResizeObserver((entries) => {
  for (const e of entries) {
    const el = e.target, box = e.borderBoxSize?.[0];
    x.dom_size(state, Number(el.dataset.slot), box ? box.inlineSize : el.offsetWidth, box ? box.blockSize : el.offsetHeight);
  }
  ask();
});

function frameOf(el) {
  let svg = frames.get(el);
  if (!svg) {
    svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("class", "fr"); svg.setAttribute("aria-hidden", "true");
    el.insertBefore(svg, el.firstChild);
    frames.set(el, svg); if (!clipping) sizes.observe(el);
  }
  return svg;
}
function unframe(el) {
  const svg = frames.get(el);
  if (svg) { svg.remove(); frames.delete(el); sizes.unobserve(el); }
}

function apply(at, length) {
  for (const c of coaxTreeCommands(memory.buffer, at, length, b => text.decode(b))) {
    const [op, slot] = c;
    if (op === 1) {
      const [, , tag, id] = c;
      const el = tag === "svg" || tag === "use" ? document.createElementNS(SVG, tag) : document.createElement(tag);
      el.id = id; el.dataset.slot = slot; els[slot] = el;
      if (tag === "iframe") el.allow = "fullscreen";
    } else if (op === 2) {
      const el = els[slot];
      if (el) { for (const f of el.querySelectorAll("[data-frame]")) unframe(f); unframe(el); el.remove(); }
      els[slot] = null;
    } else if (op === 3) {
      const el = els[slot], p = c[2], a = c[3], parent = p === NONE ? root : els[p];
      let before = a === NONE ? parent.firstChild : els[a].nextSibling;
      if (a === NONE && before && before === frames.get(parent)) before = before.nextSibling;
      if (before === el && el.parentNode === parent) continue;
      parent.insertBefore(el, before);
    } else if (op === 4) els[slot].textContent = c[2];
    else if (op === 5) els[slot].setAttribute("class", c[2]);
    else if (op === 6) {
      const el = els[slot], key = c[2], value = c[3];
      if (key === "src" && (!value.startsWith("/") || value.startsWith("//") || value.startsWith("/\\"))) continue;
      el.setAttribute(key, value);
      if (key === "data-frame") frameOf(el);
      if (key === "autofocus") queueMicrotask(() => el.focus());
      if (key === "data-lang" && el.tagName === "TEXTAREA") { el.spellcheck = false; el.wrap = "off"; el.autocapitalize = "off"; }
    } else if (op === 7) {
      const el = els[slot], key = c[2]; el.removeAttribute(key);
      if (key === "data-frame") unframe(el);
    } else if (op === 8) {
      const el = els[slot], svg = el ? frameOf(el) : null;
      for (const [r, run] of c[2].entries()) {
        const [layer, fill, alpha, stroke, xy] = run;
        let d = "";
        for (let k = 0; k < xy.length; k += 2) d += (k ? "L" : "M") + xy[k] / 64 + " " + xy[k + 1] / 64;
        if (fill) d += "Z";
        if (!svg) continue;
        let path = svg.children[r];
        if (!path) { path = document.createElementNS(SVG, "path"); svg.appendChild(path); }
        path.setAttribute("class", LAYERS[layer] + (fill && layer === 2 ? " fr-fill" : ""));
        path.setAttribute("d", d); path.style.opacity = alpha / 1000; path.style.strokeWidth = stroke / 64 || "";
      }
      if (svg) while (svg.children.length > c[2].length) svg.lastChild.remove();
    } else if (op === 9) {
      const sheet = named.has("app") ? `coaxial-style-${app}` : "coaxial-style";
      let style = document.getElementById(sheet);
      if (!style) { style = document.createElement("style"); style.id = sheet; document.head.appendChild(style); }
      style.textContent = c[1];
    } else if (op === 10) { els[slot].value = c[2]; fit(els[slot]); }
    else if (op === 11) els[slot].focus();
  }
}

// A field as tall as its text, where the browser can't do it itself (CSS field-sizing). The most is the stylesheet's.
const grows = CSS.supports("field-sizing", "content");
function fit(el) { if (!grows && el.tagName === "TEXTAREA" && !el.dataset.lang) { el.style.height = "auto"; el.style.height = el.scrollHeight + "px"; } }

function clear() { root.replaceChildren(); els = []; for (const el of [...frames.keys()]) unframe(el); }

// A frame is drawn only when one was asked for: by a press, typing, a size, or the module still moving.
function ask() { if (!asked && !clipping) { asked = true; requestAnimationFrame(frame); } }
function frame(now) {
  asked = false;
  const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
  last = now;
  const n = x.dom_frame(state, dt, calm());
  if (n < 0) clear();
  else if (n > 0) { apply(x.dom_out(state), n); if (top) shellDrawn(); }   // held in the shell's frame: drawn
  const copy = x.dom_clip_len?.(state) || 0;
  if (copy) { navigator.clipboard?.writeText(text.decode(new Uint8Array(memory.buffer, x.dom_clip(state), copy))).catch(() => {}); x.dom_clip_done(state); }
  if (x.dom_busy(state)) ask(); else last = 0;
}
// Clip mode. One step of the module's time, applied. Then the size of every framed node as the page
// lays it out now (reading it lays the page out) and, with no time passing, the frames for those sizes.
function clipFrame(dt) {
  const n = x.dom_frame(state, dt, calm());
  if (n < 0) { clear(); clipFrame(0); } else if (n > 0) apply(x.dom_out(state), n);
}
function clipSizes() {
  for (const el of frames.keys()) { const box = el.getBoundingClientRect(); x.dom_size(state, Number(el.dataset.slot), box.width, box.height); }
}
// The stylesheet's animations and transitions run on the clip's clock: each is held, at the time since it began.
function clipAnimations() {
  for (const a of document.getAnimations()) {
    if (!clipBorn.has(a)) { clipBorn.set(a, clipNow); a.pause(); }
    a.currentTime = clipNow - clipBorn.get(a);
  }
}
if (clipping) globalThis.coaxialClipStep = async (count = 1, fps = 30) => {
  if (!clipReady) return 0;
  for (let k = 0; k < count; k++) {
    clipNow += 1000 / fps;
    clipFrame(1 / fps);
    clipSizes(); clipFrame(0);
    clipAnimations();
  }
  return 1;
};
function slotOf(target, selector) { const el = target.closest?.(selector); return el && root.contains(el) ? Number(el.dataset.slot) : -1; }
function send(kind, slot, value) {
  let n = 0;
  if (value !== undefined) {
    const cap = x.dom_inbox_cap(), encoded = bytes.encode(value);
    n = Math.min(cap, encoded.length);
    new Uint8Array(memory.buffer, x.dom_inbox(state), n).set(encoded.subarray(0, n));
  }
  x.dom_event(state, kind, slot, n);
  shown();
  own.after?.();
}
// What an event changed is drawn: on an agent's clock at once, so the answer is on the page before the next
// operation (there no frame comes by itself); else with the next frame.
function shown() {
  if (!own.still || clipping) { ask(); return; }
  for (let k = 0; k < 8; k++) { frame(performance.now()); if (!x.dom_busy(state)) break; }   // a change that takes more than one frame (a pane, then its field's text)
}
// now(): the frame that would be asked for, drawn at once (a page on an agent's clock, which waits for nothing).
// still: everything is entered at once, as with reduced motion (the same page: its pictures show what is, not what is arriving).
const calm = () => (reduced.matches || own.still ? 1 : 0);
const own = { still: false, ask, send, now: () => { if (!clipping) frame(performance.now()); }, state: () => state, exports: () => x, memory: () => memory, after: null };

// A request to this site (a POST of the body, or a GET when there is none) whose answer comes back in pieces as it arrives (the module's dom_stream): each
// piece goes into the module's inbox as event 4 with the answer's status, and its end is event 5 (status 0
// when there was no answer). One at a time: a new one, or dom_stream_stop, ends the last, and an ended
// one says nothing more.
let stream = null;
function streamStop() { if (stream) { stream.abort(); stream = null; } }
function streamOpen(urlAt, urlLen, bodyAt, bodyLen) {
  streamStop();
  const url = text.decode(new Uint8Array(memory.buffer, urlAt, urlLen));
  if (!url.startsWith("/") || url.startsWith("//")) return;
  const body = new Uint8Array(memory.buffer, bodyAt, bodyLen).slice(), mine = new AbortController();
  stream = mine;
  const live = () => stream === mine;
  (async () => {
    let status = 0;
    try {
      const r = await fetch(url, bodyLen ? { method: "POST", body, credentials: "same-origin", headers: { "Content-Type": "application/json" }, signal: mine.signal }
        : { credentials: "same-origin", cache: "no-store", signal: mine.signal });
      status = r.status;
      const reader = r.body.getReader(), cap = x.dom_inbox_cap();
      for (;;) {
        const { done, value } = await reader.read();
        if (done || !live()) break;
        for (let at = 0; at < value.length && live(); at += cap) {
          const piece = value.subarray(at, at + cap);
          new Uint8Array(memory.buffer, x.dom_inbox(state), piece.length).set(piece);
          x.dom_event(state, 4, status, piece.length);
        }
        try { shown(); } catch (error) { console.error(error); }   // a frame's own fault is not the answer's: the stream goes on
      }
    } catch { if (live()) status = 0; }
    if (live()) { stream = null; x.dom_event(state, 5, status, 0); shown(); own.after?.(); }   // an answer may leave an action for the page that opened this module
  })();
}

// Files (the kit's drop target, k_drop: a node with data-drop). A file dropped on one, or picked from the picker a
// press on it opens, waits here, and the module is told its size and name (event 10 on the target's slot:
// "BYTES NAME"). The module then asks for it to be sent (dom_file_send: a POST of the module's own first bytes, then
// the file's as they are), or asks nothing and the next file takes its place. dom_file_send with no first bytes is a
// GET. A target whose data-drop is empty takes a dropped file and opens no picker (a button that does something else when
// pressed). The answer comes back whole as event 11, its status in `slot` (0: no answer), cut to the inbox's size. While
// the page has a drop target, a file dropped anywhere on the window goes to it when it is the only one, so a miss
// never opens the file in place of the page.
// Where the server takes a file by direct upload, the module asks for it in steps, one at a time, each answered as
// event 11 (dom_file_step):
//   1  the file is named by its content: a POST of the module's first bytes, then the file's SHA-256 in hex and a
//      line end. The file stays here.
//   2  the file alone is PUT to the address the server answered, which may be another origin's (an upload it opened
//      in storage): no credentials go with it, and no content type of the page's choosing. The file stays here.
//   3  a POST of the module's bytes alone, and the file is let go.
let offered = null, picker = null, over = null;
function dropOn(e) {
  if (!e.dataTransfer || ![...e.dataTransfer.types].includes("Files")) return undefined;
  const all = root.querySelectorAll("[data-drop]");
  if (!all.length) return undefined;
  const el = e.target?.closest?.("[data-drop]");
  return el && root.contains(el) ? el : all.length === 1 ? all[0] : null;
}
function overNow(el) { if (over === el) return; over?.removeAttribute("data-over"); over = el; el?.setAttribute("data-over", ""); }
function offer(el, file) { if (!file || !el?.isConnected) return; offered = file; send(10, Number(el.dataset.slot), `${file.size} ${file.name}`); }
function pick(el) {
  picker ||= Object.assign(document.createElement("input"), { type: "file", hidden: true });
  picker.accept = el.dataset.drop; picker.value = "";
  picker.onchange = () => offer(el, picker.files?.[0]);
  picker.click();
}
// A made realm knows its maker by their player key (X-Verse-Player, the key coax/web/host.js keeps in
// localStorage verse.player): an ask of this site's API carries it, so a realm's own world answers its owner (#13496).
function playerHeaders(extra) {
  let key = "";
  try { const p = JSON.parse(localStorage.getItem("verse.player") || "null"); if (p && typeof p.key === "string") key = p.key; } catch {}
  return key ? { ...extra, "X-Verse-Player": key } : extra;
}
function fileSend(urlAt, urlLen, headAt, headLen) {
  const url = text.decode(new Uint8Array(memory.buffer, urlAt, urlLen));
  if (!url.startsWith("/") || url.startsWith("//")) return;
  const head = new Uint8Array(memory.buffer, headAt, headLen).slice(), file = headLen ? offered : null;
  if (headLen) offered = null;
  (async () => {
    let status = 0, n = 0;
    try {
      const r = await fetch(url, headLen ? { method: "POST", body: new Blob(file ? [head, file] : [head]), credentials: "same-origin", headers: playerHeaders({ "Content-Type": "application/octet-stream" }) }
        : { credentials: "same-origin", cache: "no-store", headers: playerHeaders({}) });
      const body = new Uint8Array(await r.arrayBuffer());
      status = r.status; n = Math.min(body.length, x.dom_inbox_cap());
      new Uint8Array(memory.buffer, x.dom_inbox(state), n).set(body.subarray(0, n));
    } catch { status = 0; n = 0; }
    x.dom_event(state, 11, status, n);
    ask(); own.after?.();
  })();
}
function fileStep(urlAt, urlLen, headAt, headLen, step) {
  const url = text.decode(new Uint8Array(memory.buffer, urlAt, urlLen));
  const head = new Uint8Array(memory.buffer, headAt, headLen).slice(), file = offered;
  const here = url.startsWith("/") && !url.startsWith("//");
  if (step === 3) offered = null;
  (async () => {
    let status = 0, n = 0;
    try {
      let r;
      if (step === 2) {
        if (!file || !/^https?:\/\//.test(url)) throw new Error("no file or no address");
        r = await fetch(url, { method: "PUT", body: file.slice(0, file.size, ""), credentials: "omit" });
      } else {
        if (!here || (step === 1 && !file)) throw new Error("no file or no address");
        const parts = [head];
        if (step === 1) parts.push([...new Uint8Array(await crypto.subtle.digest("SHA-256", await file.arrayBuffer()))].map((b) => b.toString(16).padStart(2, "0")).join("") + "\n");
        r = await fetch(url, { method: "POST", body: new Blob(parts), credentials: "same-origin", headers: playerHeaders({ "Content-Type": "application/octet-stream" }) });
      }
      const body = new Uint8Array(await r.arrayBuffer());
      status = r.status; n = Math.min(body.length, x.dom_inbox_cap());
      new Uint8Array(memory.buffer, x.dom_inbox(state), n).set(body.subarray(0, n));
    } catch { status = 0; n = 0; }
    x.dom_event(state, 11, status, n);
    ask(); own.after?.();
  })();
}

async function start() {
  const response = await fetch(`/build/${app}.wasm`, { cache: location.hostname === "127.0.0.1" || location.hostname === "localhost" ? "no-store" : "default" });
  const module = await WebAssembly.compile(await response.arrayBuffer());
  memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
  const keptKey = `coaxial.dom.${app}.kept`;
  const keep = (at, len) => { if (len > 4096) return; try { localStorage.setItem(keptKey, text.decode(new Uint8Array(memory.buffer, at, len))); } catch { /* refused: kept for this visit only */ } };
  const go = (at, len, push) => {
    const url = text.decode(new Uint8Array(memory.buffer, at, len));
    if (!url.startsWith("/") || url.startsWith("//")) return;
    if (!push || fixed) { if (!(top && shellGo(url))) location.assign(url); return; }   // in the shell's frame, the shell's
    pushed = true;
    if (url !== location.pathname + location.search) history.pushState(null, "", url);
  };
  const imports = { env: { memory, dom_stream: streamOpen, dom_stream_stop: streamStop, dom_file_send: fileSend, dom_file_step: fileStep, dom_keep: keep, dom_go: go } };
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= {})[imp.name] ||= imp.name === "fd_write"
      ? (fd, iovs, count, written) => {
          const v = new DataView(memory.buffer); let out = "", total = 0;
          for (let k = 0; k < count; k++) { const p = v.getUint32(iovs + k * 8, true), len = v.getUint32(iovs + k * 8 + 4, true); out += text.decode(new Uint8Array(memory.buffer, p, len)); total += len; }
          v.setUint32(written, total, true); console.log(out.trimEnd()); return 0;
        }
      : imp.name === "proc_exit" ? (code) => { throw new Error(`${app} exited (${code})`); } : () => 0;
  }
  x = (await WebAssembly.instantiate(module, imports)).exports;
  x._initialize?.();
  // the state, above the module's own data and stack
  const size = x.coaxial_state_size();
  state = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536;
  memory.grow(Math.ceil((size + 64) / 65536));
  x.coaxial_state_init(state);
  root.replaceChildren();
  if (WebAssembly.Module.imports(module).some((i) => i.name === "dom_keep")) {
    let kept = ""; try { kept = localStorage.getItem(keptKey) || ""; } catch { /* refused */ }
    send(12, 0, kept);
  }
  const going = WebAssembly.Module.imports(module).some((i) => i.name === "dom_go"), shell = going && !fixed;
  if (fixed) send(13, 0, fixed);
  const here = () => (top ? location.pathname + location.search : location.pathname);
  if (shell && top && !shellHeld) shellHost(own, root);   // before the first address: the frame it draws talks to it
  if (shell) { send(13, 0, here()); addEventListener("popstate", () => send(13, 0, here())); }
  root.addEventListener("click", (e) => {
    const drop = e.target.closest?.('button[data-drop]:not([data-drop=""])');
    if (drop && root.contains(drop)) { pick(drop); return; }   // a drop target that names what it takes: a press opens the picker
    const slot = slotOf(e.target, "button[data-slot]");
    if (slot >= 0) { send(1, slot); return; }
    // a link in an app shell: a plain press is the module's to keep in the shell (it pushes) or let go
    const link = shell && !e.defaultPrevented && e.button === 0 && !(e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) ? slotOf(e.target, "a[data-slot]") : -1;
    if (link >= 0) { pushed = false; send(1, link); if (pushed) e.preventDefault(); }
  });
  addEventListener("dragover", (e) => { const el = dropOn(e); if (el === undefined) return; e.preventDefault(); overNow(el); });
  addEventListener("dragleave", (e) => { if (!e.relatedTarget) overNow(null); });
  addEventListener("drop", (e) => { const el = dropOn(e); if (el === undefined) return; e.preventDefault(); overNow(null); offer(el, e.dataTransfer.files?.[0]); });
  root.addEventListener("input", (e) => { const slot = slotOf(e.target, "textarea[data-slot],input[data-slot]"); if (slot >= 0) { fit(e.target); send(2, slot, e.target.value); } });
  root.addEventListener("keydown", (e) => {
    // a code field (k_code_field): Enter makes a line, as the browser has it, and Tab indents two spaces
    if (e.target.dataset?.lang && e.target.tagName === "TEXTAREA") {
      if (e.key !== "Tab" || e.shiftKey || e.altKey || e.ctrlKey || e.metaKey || e.isComposing) return;
      e.preventDefault(); e.target.setRangeText("  ", e.target.selectionStart, e.target.selectionEnd, "end");
      send(2, Number(e.target.dataset.slot), e.target.value);
      return;
    }
    if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
    const slot = slotOf(e.target, "textarea[data-slot],input[data-slot]");
    if (slot >= 0) { e.preventDefault(); send(2, slot, e.target.value); send(3, slot); }
  });
  reduced.addEventListener("change", ask);
  // coax drive through a module that answers it (dom_drive, the shared app's operations: milestone.coax): the
  // operation goes into its inbox and its answer comes from dom_drive_out; what the operation changed is drawn
  // at once, so the page shows it before the next operation
  if (x.dom_drive) own.drive = (op) => {
    const encoded = bytes.encode(op), n = Math.min(x.dom_inbox_cap(), encoded.length);
    new Uint8Array(memory.buffer, x.dom_inbox(state), n).set(encoded.subarray(0, n));
    const m = x.dom_drive(state, n), answer = text.decode(new Uint8Array(memory.buffer, x.dom_drive_out(state), m));
    for (let k = 0; k < 8; k++) { frame(performance.now()); if (!x.dom_busy(state)) break; }
    return answer;
  };
  globalThis.coaxialDom ||= own;
  (globalThis.coaxialDoms ||= {})[named.get("as") || app] = own;
  dispatchEvent(new CustomEvent("coaxial-dom-ready", { detail: { app, host: own } }));
  if (clipping) {
    // the first nodes and the stylesheet, then every face it names, so no text is measured in a fallback face
    clipFrame(0); clipSizes(); clipFrame(0);
    await Promise.all([...document.fonts].map((face) => face.load().catch(() => {})));
    await document.fonts.ready;
    clipSizes(); clipFrame(0);
    clipReady = true;
    return;
  }
  ask();
  // development: the dev server says when a module was rebuilt
  // (a page that opened this host over its canvas listens itself; a world's write is not a rebuild)
  // (a page held in the shell's frame listens too: a reload there is the frame's alone; the shell reloads the window
  // only for a page or its own module, so a world's import or another module's rebuild doesn't take the frame down)
  if (top && (location.hostname === "127.0.0.1" || location.hostname === "localhost")) {
    try {
      new WebSocket(`ws://${location.host}/coaxial/live`).onmessage = (m) => {
        let said = {}; try { said = JSON.parse(m.data) || {}; } catch { said = { kind: "wasm" }; /* not JSON: a rebuild */ }
        if (said.kind === "page" || (said.kind === "wasm" && (!said.path || said.path === `/build/${app}.wasm`))) location.reload();
      };
    } catch { /* no dev server */ }
  }
}
start().catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
