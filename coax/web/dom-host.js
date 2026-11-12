// Firm reason: 1 (browser glue: applies the browser backend's commands to the page and reports presses, typing and sizes)
// The host for a text surface the browser draws (components/backend/dom.coax;
// docs/audits/2026-11-11-chat-zero-base-audit.md, sections 10 and 11). It
// loads the page's Wasm module, keeps its state, and each frame the module
// asks for applies the commands it wrote: make a node, place it, set its
// text, class or an attribute, draw its frame as SVG paths, take it away.
// It holds no logic: what is on the page, and what a press does, is the
// module's. It needs no WebGPU.
//   <script type="module" src="/coaxial/dom-host.js" data-app="kit" data-root="coaxial-root"></script>
const script = document.querySelector("script[data-app]");
const app = script.dataset.app;
const root = document.getElementById(script.dataset.root || "coaxial-root");
const NONE = 0xffff, SVG = "http://www.w3.org/2000/svg", LAYERS = ["fr-bg", "fr-line", "fr-deco"];
const text = new TextDecoder(), bytes = new TextEncoder();
const reduced = matchMedia("(prefers-reduced-motion: reduce)");
let memory, x, state = 0, els = [], frames = new Map(), asked = false, last = 0;

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
    frames.set(el, svg); sizes.observe(el);
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
      const slot = u16(), el = document.createElement(str());
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
    else if (op === 5) { els[u16()].className = str(); }
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
    } else if (op === 10) { els[u16()].value = str(); }
    else throw new Error(`${app}: unknown command ${op}`);
  }
}

function clear() { root.replaceChildren(); els = []; for (const el of [...frames.keys()]) unframe(el); }

// A frame is drawn only when one was asked for: by a press, typing, a size, or the module still moving.
function ask() { if (!asked) { asked = true; requestAnimationFrame(frame); } }
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

async function start() {
  const response = await fetch(`/build/${app}.wasm`, { cache: location.hostname === "127.0.0.1" || location.hostname === "localhost" ? "no-store" : "default" });
  const module = await WebAssembly.compile(await response.arrayBuffer());
  memory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
  const imports = { env: { memory } };
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
  root.addEventListener("input", (e) => { const slot = slotOf(e.target, "textarea[data-slot]"); if (slot >= 0) send(2, slot, e.target.value); });
  root.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
    const slot = slotOf(e.target, "textarea[data-slot]");
    if (slot >= 0) { e.preventDefault(); send(2, slot, e.target.value); send(3, slot); }
  });
  reduced.addEventListener("change", ask);
  globalThis.coaxialDom = { ask, state: () => state, exports: () => x };
  ask();
  // development: the dev server says when a module was rebuilt
  if (location.hostname === "127.0.0.1" || location.hostname === "localhost") {
    try { new WebSocket(`ws://${location.host}/coaxial/live`).onmessage = () => location.reload(); } catch { /* no dev server */ }
  }
}
start().catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
