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
const script = document.querySelector("script[data-app]");
const app = script.dataset.app;
const root = document.getElementById(script.dataset.root || "coaxial-root");
const NONE = 0xffff, SVG = "http://www.w3.org/2000/svg", LAYERS = ["fr-bg", "fr-line", "fr-deco"];
const text = new TextDecoder(), bytes = new TextEncoder();
const reduced = matchMedia("(prefers-reduced-motion: reduce)");
let memory, x, state = 0, els = [], frames = new Map(), asked = false, last = 0;
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
  const v = new DataView(memory.buffer, at, length);
  let i = 0;
  const u8 = () => v.getUint8(i++);
  const u16 = () => { const n = v.getUint16(i, true); i += 2; return n; };
  const u32 = () => { const n = v.getUint32(i, true); i += 4; return n; };
  const i32 = () => { const n = v.getInt32(i, true); i += 4; return n; };
  const str = () => { const n = u32(), s = text.decode(new Uint8Array(memory.buffer, at + i, n)); i += n; return s; };
  while (i < length) {
    const op = u8();
    if (op === 1) {                                  // make
      const slot = u16(), tag = str(), el = tag === "svg" || tag === "use" ? document.createElementNS(SVG, tag) : document.createElement(tag);
      el.id = str(); el.dataset.slot = slot; els[slot] = el;
    } else if (op === 2) {                           // remove
      const slot = u16(), el = els[slot];
      if (el) { for (const f of el.querySelectorAll("[data-frame]")) unframe(f); unframe(el); el.remove(); }
      els[slot] = null;
    } else if (op === 3) {                           // place: after its sibling, or first in its parent (past the frame)
      const el = els[u16()], p = u16(), a = u16(), parent = p === NONE ? root : els[p];
      let before = a === NONE ? parent.firstChild : els[a].nextSibling;
      if (a === NONE && before && before === frames.get(parent)) before = before.nextSibling;
      parent.insertBefore(el, before);
    } else if (op === 4) { els[u16()].textContent = str(); }
    else if (op === 5) { els[u16()].setAttribute("class", str()); }
    else if (op === 6) {                             // attr
      const el = els[u16()], key = str(), value = str();
      el.setAttribute(key, value);
      if (key === "data-frame") frameOf(el);
      if (key === "autofocus") queueMicrotask(() => el.focus());
    } else if (op === 7) {                           // unattr
      const el = els[u16()], key = str();
      el.removeAttribute(key);
      if (key === "data-frame") unframe(el);
    } else if (op === 8) {                           // frame: its runs as paths, reused in order
      const el = els[u16()], svg = el && frameOf(el), runs = u8();
      for (let r = 0; r < runs; r++) {
        const layer = u8(), fill = u8(), alpha = u16() / 1000, stroke = u16() / 64, points = u16();
        let d = "";
        for (let k = 0; k < points; k++) d += (k ? "L" : "M") + i32() / 64 + " " + i32() / 64;
        if (fill) d += "Z";
        if (!svg) continue;
        let path = svg.children[r];
        if (!path) { path = document.createElementNS(SVG, "path"); svg.appendChild(path); }
        path.setAttribute("class", LAYERS[layer] + (fill && layer === 2 ? " fr-fill" : ""));
        path.setAttribute("d", d);
        path.style.opacity = alpha; path.style.strokeWidth = stroke || "";
      }
      if (svg) while (svg.children.length > runs) svg.lastChild.remove();
    } else if (op === 9) {                           // style
      let style = document.getElementById("coaxial-style");
      if (!style) { style = document.createElement("style"); style.id = "coaxial-style"; document.head.appendChild(style); }
      style.textContent = str();
    } else if (op === 10) { const el = els[u16()]; el.value = str(); fit(el); }
    else if (op === 11) { els[u16()].focus(); }
    else throw new Error(`${app}: unknown command ${op}`);
  }
}

// A field as tall as its text, where the browser can't do it itself (CSS field-sizing). The most is the stylesheet's.
const grows = CSS.supports("field-sizing", "content");
function fit(el) { if (!grows && el.tagName === "TEXTAREA") { el.style.height = "auto"; el.style.height = el.scrollHeight + "px"; } }

function clear() { root.replaceChildren(); els = []; for (const el of [...frames.keys()]) unframe(el); }

// A frame is drawn only when one was asked for: by a press, typing, a size, or the module still moving.
function ask() { if (!asked && !clipping) { asked = true; requestAnimationFrame(frame); } }
function frame(now) {
  asked = false;
  const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
  last = now;
  const n = x.dom_frame(state, dt, reduced.matches ? 1 : 0);
  if (n < 0) clear();
  else if (n > 0) apply(x.dom_out(state), n);
  const copy = x.dom_clip_len(state);
  if (copy) { navigator.clipboard?.writeText(text.decode(new Uint8Array(memory.buffer, x.dom_clip(state), copy))).catch(() => {}); x.dom_clip_done(state); }
  if (x.dom_busy(state)) ask(); else last = 0;
}
// Clip mode. One step of the module's time, applied. Then the size of every framed node as the page
// lays it out now (reading it lays the page out) and, with no time passing, the frames for those sizes.
function clipFrame(dt) {
  const n = x.dom_frame(state, dt, reduced.matches ? 1 : 0);
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
  x.dom_event(state, kind, slot, n); ask();
}

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
        ask();
      }
    } catch { if (live()) status = 0; }
    if (live()) { stream = null; x.dom_event(state, 5, status, 0); ask(); }
  })();
}

async function start() {
  const response = await fetch(`/build/${app}.wasm`, { cache: location.hostname === "127.0.0.1" || location.hostname === "localhost" ? "no-store" : "default" });
  const module = await WebAssembly.compile(await response.arrayBuffer());
  memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
  const imports = { env: { memory, dom_stream: streamOpen, dom_stream_stop: streamStop } };
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
  root.addEventListener("click", (e) => {
    const slot = slotOf(e.target, "button[data-slot]");
    if (slot >= 0) send(1, slot);
  });
  root.addEventListener("input", (e) => { const slot = slotOf(e.target, "textarea[data-slot],input[data-slot]"); if (slot >= 0) { fit(e.target); send(2, slot, e.target.value); } });
  root.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
    const slot = slotOf(e.target, "textarea[data-slot],input[data-slot]");
    if (slot >= 0) { e.preventDefault(); send(2, slot, e.target.value); send(3, slot); }
  });
  reduced.addEventListener("change", ask);
  globalThis.coaxialDom = { ask, state: () => state, exports: () => x };
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
  if (location.hostname === "127.0.0.1" || location.hostname === "localhost") {
    try { new WebSocket(`ws://${location.host}/coaxial/live`).onmessage = () => location.reload(); } catch { /* no dev server */ }
  }
}
start().catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
