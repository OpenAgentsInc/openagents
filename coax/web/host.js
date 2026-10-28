// Coaxial's browser host. Platform effects only, as Swift is on the phone:
// the WebGPU device and canvas, the frame clock, one WebAssembly.Memory that
// holds every byte of the app's state, and (in development) the live channel
// to `coax serve` that swaps in new code, shaders and pages while the app
// runs. Every decision about what to draw is the app's Coax code.
//
// An app is `/build/APP.wasm` (Coax compiled through C to wasm32) and
// `/APP.wgsl`. The wasm exports a scalar C face:
//   coaxial_layout()          state layout fingerprint
//   coaxial_state_size()      bytes of state the host keeps
//   coaxial_max_vertices(), coaxial_vertex_floats(), coaxial_uniform_floats()
//   coaxial_frame(state, vertices, uniforms, width, height, dpr, dt, still)
//                             -> vertex count
//   coaxial_page(state, page) optional: which page to draw (data-page, e.g. a 404)
// and, optionally, input and output, all drawn and decided by the app:
//   coaxial_key(state, key, mods) -> used     a code point, or a named key < 0 (KEYS)
//   coaxial_pointer(state, x, y, phase, kind) device pixels; phase 0 down 1 move 2 up 3 cancel
//   coaxial_inbox(state), coaxial_inbox_cap(), coaxial_message(state, kind, length)
//                             text in: kind 1 is typed or pasted text; others the page's own
//   coaxial_wants_text(state) focus the keyboard field (a phone's keyboard)
//   coaxial_field(state)      where the app draws that field: 4 floats, device pixels
//   coaxial_cursor(state)     0 the default, 1 a link
//   coaxial_action(state), coaxial_outbox(state), coaxial_outbox_len(state), coaxial_action_done(state)
//                             out: 1 navigates to the outbox's URL; others go to the page's
//                             scripts as a "coaxial" event (0 when an engine loads), which
//                             answer with coaxial.send(kind, text)
// The WGSL has bg_vs/bg_fs (a full-screen pass) and fx_vs/fx_fs (the
// vertices: one vec4f attribute per four floats), sharing one uniform buffer.

const script = document.currentScript || document.querySelector("script[data-app]");
const app = (script && script.dataset.app) || "app";
// A deploy names the files by their content (data-wasm, data-wgsl), so a CDN
// can keep them for good; in development they are /build/APP.wasm and /APP.wgsl.
const wasmUrl = (script && script.dataset.wasm) || `/build/${app}.wasm`;
const wgslUrl = (script && script.dataset.wgsl) || `/${app}.wgsl`;
const canvas = document.getElementById("coaxial");
const dev = ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname);
const probe = dev && new URLSearchParams(location.search).has("probe"); // tests: read the frame back
// ?probe&at=SECONDS: run the app's clock that far in 0.1 s steps before the
// probed frame, so a headless browser can see a moment of the animation.
const probeAt = probe ? Number(new URLSearchParams(location.search).get("at")) || 0 : 0;
// Reduced motion draws the finished frame at once (probes use it too: headless browsers barely tick).
const still = (probe && !probeAt) || matchMedia("(prefers-reduced-motion: reduce)").matches;
let frames = 0;
// Development tests only: ?probe&do=STEPS runs input before the probed frame,
// steps split by ";": "kTEXT" types keys, "K-2" a named key, "m16:TEXT" a
// message, "t1.5" runs the clock that many seconds.
const probeDo = probe ? new URLSearchParams(location.search).get("do") || "" : "";

const memory = new WebAssembly.Memory({ initial: 64, maximum: 1024 }); // 4 MiB, at most 64 MiB
let engine = null;   // { exports, layout }
let arena = null;    // { state, vertices, uniforms, floats, uniformFloats, maxVertices }
let gpu = null;      // { device, context, format, bindGroup, uniformBuffer, vertexBuffer, bg, fx }
let wanted = 0;      // the newest requested engine; an older finished build never replaces it
let last = 0;

// ---- engine: compile, check its layout, keep or start over its state
async function loadEngine(bytes, generation) {
  const module = await WebAssembly.compile(bytes);
  const imports = { env: { memory } };
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= {})[imp.name] ||= wasiStub(imp.name);
  }
  const instance = await WebAssembly.instantiate(module, imports);
  if (generation !== wanted) return false; // a newer build was asked for meanwhile
  const x = instance.exports;
  x._initialize?.();
  const layout = x.coaxial_layout();
  const heapBase = x.__heap_base?.value ?? 0;
  const floats = x.coaxial_vertex_floats(), maxVertices = x.coaxial_max_vertices();
  const uniformFloats = x.coaxial_uniform_floats(), stateSize = x.coaxial_state_size();
  if (!arena || heapBase > arena.base || floats !== arena.floats || maxVertices !== arena.maxVertices || uniformFloats !== arena.uniformFloats || stateSize > arena.stateCap) {
    // A fresh arena above the module's own data and stack, so a new
    // module's data segments never land on live state.
    const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
    const stateCap = align(Math.max(stateSize, 1024), 16);
    const need = base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64;
    if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
    arena = { base, stateCap, state: base, vertices: base + stateCap, uniforms: base + stateCap + maxVertices * floats * 4, floats, maxVertices, uniformFloats, layout: -1 };
    if (gpu) makeBuffers();
  }
  if (arena.layout !== layout) {
    new Uint8Array(memory.buffer, arena.state, arena.stateCap).fill(0); // the state's shape changed: start it over
    if (engine) say(`state layout changed: ${app} starts over`);
    arena.layout = layout;
  }
  x.coaxial_page?.(arena.state, Number((script && script.dataset.page) || 0));
  engine = { exports: x, layout };
  // The page's scripts may talk to the app from now (action 0: ready).
  globalThis.coaxial.ready = true;
  dispatchEvent(new CustomEvent("coaxial", { detail: { code: 0, text: "" } }));
  return true;
}

function wasiStub(name) {
  if (name === "fd_write") return (fd, iovs, n, written) => {
    const v = new DataView(memory.buffer); let text = "", total = 0;
    for (let i = 0; i < n; i++) {
      const p = v.getUint32(iovs + i * 8, true), len = v.getUint32(iovs + i * 8 + 4, true);
      text += new TextDecoder().decode(new Uint8Array(memory.buffer, p, len)); total += len;
    }
    v.setUint32(written, total, true); (fd === 2 ? console.error : console.log)(text.trimEnd()); return 0;
  };
  if (name === "proc_exit") return (code) => { throw new Error(`${app} exited (${code})`); };
  return () => 0;
}

const align = (n, a) => Math.ceil(n / a) * a;

// ---- GPU: device, the two pipelines from the app's WGSL, buffers
async function startGpu() {
  if (!navigator.gpu) return false;
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return false;
  const device = await adapter.requestDevice();
  const context = canvas.getContext("webgpu");
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | (probe ? GPUTextureUsage.COPY_SRC : 0) });
  gpu = { device, context, format };
  makeBuffers();
  return loadShaders();
}

function makeBuffers() {
  const { device } = gpu;
  gpu.uniformBuffer = device.createBuffer({ size: align(arena.uniformFloats * 4, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  gpu.vertexBuffer = device.createBuffer({ size: arena.maxVertices * arena.floats * 4, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
  gpu.layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} }] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [{ binding: 0, resource: { buffer: gpu.uniformBuffer } }] });
}

async function loadShaders() {
  const { device, format } = gpu;
  const code = await (await fetch(wgslUrl, { cache: dev ? "no-store" : "default" })).text();
  const module = device.createShaderModule({ code });
  const info = await module.getCompilationInfo();
  const errors = info.messages.filter((m) => m.type === "error");
  if (errors.length) { say(errors.map((m) => `${app}.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return false; }
  const layout = device.createPipelineLayout({ bindGroupLayouts: [gpu.layout] });
  const stride = arena.floats * 4;
  const [bg, fx] = await Promise.all([
    device.createRenderPipelineAsync({ layout, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: [{ format }] } }),
    device.createRenderPipelineAsync({
      layout,
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: {
        color: { operation: "max", srcFactor: "one", dstFactor: "one" }, alpha: { operation: "max", srcFactor: "one", dstFactor: "one" } } }] },
    }),
  ]);
  gpu.bg = bg; gpu.fx = fx; // swapped whole, at the next frame
  return true;
}

// ---- the canvas's size in device pixels: exactly its CSS box times the
// display's scale, from ResizeObserver's device-pixel box where the browser
// has one, so the GPU draws every device pixel and nothing is stretched (a
// buffer a little smaller than the screen, scaled up, blurs thin strokes).
let devicePixels = null;   // [width, height], or null until observed
function observeSize() {
  const take = ([e]) => {
    const s = e.devicePixelContentBoxSize?.[0];
    devicePixels = s ? [s.inlineSize, s.blockSize]
      : [Math.round(e.contentRect.width * devicePixelRatio), Math.round(e.contentRect.height * devicePixelRatio)];
  };
  try { new ResizeObserver(take).observe(canvas, { box: "device-pixel-content-box" }); }
  catch { new ResizeObserver(take).observe(canvas); }
}
// Device pixels per CSS pixel, as the canvas is actually drawn.
const scale = () => (canvas.width && canvas.clientWidth ? canvas.width / canvas.clientWidth : devicePixelRatio || 1);

// ---- the frame
function frame(now) {
  requestAnimationFrame(frame);
  if (!engine || !gpu?.bg) return;
  const dt = last ? (now - last) / 1000 : 0; last = now;
  const [w, h] = devicePixels || [Math.max(1, Math.round(canvas.clientWidth * devicePixelRatio)), Math.max(1, Math.round(canvas.clientHeight * devicePixelRatio))];
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  const dpr = scale();
  let n = 0;
  try {
    for (let k = 0; probeAt && !gpu.probed && k < probeAt * 10; k++)
      engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, 0.1, 0);
    if (probeDo && !gpu.probed) runProbe(w, h, dpr);
    n = engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, dt, still ? 1 : 0);
  } catch (e) { say(`${app} trapped: ${e.message}`, true); engine = null; return; }
  n = Math.min(n, arena.maxVertices);
  const { device, context } = gpu;
  device.queue.writeBuffer(gpu.uniformBuffer, 0, new Float32Array(memory.buffer, arena.uniforms, arena.uniformFloats));
  if (n) device.queue.writeBuffer(gpu.vertexBuffer, 0, new Float32Array(memory.buffer, arena.vertices, n * arena.floats));
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }] });
  pass.setBindGroup(0, gpu.bindGroup);
  pass.setPipeline(gpu.bg); pass.draw(3);
  if (n) { pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(n); }
  pass.end();
  const texture = context.getCurrentTexture();
  const readback = probe && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;
  device.queue.submit([encoder.finish()]);
  document.documentElement.classList.add("gpu");
  if (readback) report(readback, texture);
  after(false);
}

// ---- input: keys, the pointer and typed text go to the app; its actions come out
const KEYS = { Escape: -1, Enter: -2, Backspace: -3, ArrowLeft: -4, ArrowRight: -5, ArrowUp: -6, ArrowDown: -7,
  Home: -8, End: -9, PageUp: -10, PageDown: -11 };
const call = (name, ...args) => (engine && engine.exports[name] ? engine.exports[name](arena.state, ...args) : 0);
let field = null;   // INVARIANTS.md: the keyboard field, an invisible input over the app's drawn field (keyboard, paste, context menu)
let touched = false;

function send(kind, text = "") {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = new TextEncoder().encode(text).subarray(0, x.coaxial_inbox_cap());
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_message(arena.state, kind, bytes.length);
  after(false);
}
globalThis.coaxial = { send, ready: false };

// After input or a frame: focus or free the keyboard field, carry out an
// action, set the cursor. gesture: inside an input event, where a phone
// lets a field bring up its keyboard; tap: a touch just ended, which focuses
// the field again so a keyboard that was dismissed comes back.
function after(gesture, tap = false) {
  if (!engine) return;
  const x = engine.exports;
  const want = call("coaxial_wants_text") === 1;
  if (want && document.activeElement !== field ? gesture || !touched : want && tap && touched) focusField();
  else if (!want && field && document.activeElement === field) field.blur();
  if (field) placeField(want);
  const code = call("coaxial_action");
  if (code) {
    const text = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_outbox(arena.state), x.coaxial_outbox_len(arena.state)).slice());
    call("coaxial_action_done");
    if (code === 1) { if (text.startsWith("/")) location.assign(text); }
    else dispatchEvent(new CustomEvent("coaxial", { detail: { code, text } }));
  }
  const cursor = ["", "pointer"][call("coaxial_cursor")] || "";
  if ((canvas.dataset.cursor || "") !== cursor) canvas.dataset.cursor = cursor;
}

// Puts the field over the box the app draws it in, so a paste or a right
// click there lands in it; off to one side when nothing takes text.
function placeField(want) {
  const x = engine.exports;
  const r = want && x.coaxial_field ? new Float32Array(memory.buffer, x.coaxial_field(arena.state), 4) : [0, 0, 0, 0];
  const dpr = scale();
  const box = r[2] > r[0] ? [r[0] / dpr, r[1] / dpr, (r[2] - r[0]) / dpr, (r[3] - r[1]) / dpr] : [0, 0, 1, 1];
  const key = box.map(Math.round).join();
  if (field.dataset.at === key) return;
  field.dataset.at = key;
  field.style.left = `${box[0]}px`; field.style.top = `${box[1]}px`; // keyboard field
  field.style.width = `${box[2]}px`; field.style.height = `${box[3]}px`; // keyboard field
}

function focusField() {
  if (!field) {
    field = document.createElement("input"); // keyboard field
    for (const [k, v] of Object.entries({ "aria-label": "Typed text", autocomplete: "off", autocapitalize: "off",
      autocorrect: "off", spellcheck: "false", enterkeyhint: "go" })) field.setAttribute(k, v);
    field.style.cssText = "position:fixed;left:0;top:0;width:1px;height:1px;margin:0;padding:0;border:0;outline:0;opacity:0;background:transparent;color:transparent;caret-color:transparent;font-size:16px"; // keyboard field
    // The field holds one space, so a phone's delete always has something to delete.
    const flush = () => {
      const v = field.value;
      if (v === " ") return;
      if (v === "") call("coaxial_key", KEYS.Backspace, 0); else send(1, v.startsWith(" ") ? v.slice(1) : v);
      field.value = " "; field.setSelectionRange(1, 1);
      after(false);
    };
    field.addEventListener("input", (e) => { if (!e.isComposing) flush(); });
    field.addEventListener("compositionend", flush);
    document.body.append(field);
  }
  if (document.activeElement === field) field.blur();
  field.value = " ";
  field.focus({ preventScroll: true });
  field.setSelectionRange(1, 1);
}

addEventListener("keydown", (e) => {
  if (!engine || e.isComposing) return;
  const named = KEYS[e.key];
  // Paste is the browser's: it lands in the field, whose input event brings the text.
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "v") return;
  if (!named && e.target === field && !e.metaKey && !e.ctrlKey) return;   // typed text comes by the input event
  const key = named ?? ([...e.key].length === 1 ? e.key.codePointAt(0) : 0);
  if (!key) return;
  const mods = (e.shiftKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0);
  if (call("coaxial_key", key, mods)) e.preventDefault();
  after(true);
});
addEventListener("paste", (e) => {
  if (e.target === field || !engine) return;
  const text = e.clipboardData?.getData("text") || "";
  if (text) { e.preventDefault(); send(1, text); }
});
for (const [type, phase] of [["pointerdown", 0], ["pointermove", 1], ["pointerup", 2], ["pointercancel", 3]])
  addEventListener(type, (e) => {
    if (!engine) return;
    const dpr = scale();
    if (phase !== 1) touched = e.pointerType !== "mouse";
    call("coaxial_pointer", e.clientX * dpr, e.clientY * dpr, phase, e.pointerType === "mouse" ? 0 : 1);
    after(phase === 2, phase === 2);
  });

function runProbe(w, h, dpr) {
  for (const step of probeDo.split(";").filter(Boolean)) {
    const op = step[0], arg = step.slice(1);
    if (op === "k") for (const c of arg) call("coaxial_key", c.codePointAt(0), 0);
    if (op === "K") call("coaxial_key", Number(arg), 0);
    if (op === "m") { const i = arg.indexOf(":"); send(Number(i < 0 ? arg : arg.slice(0, i)), i < 0 ? "" : arg.slice(i + 1)); }
    if (op === "t") for (let k = 0; k < Number(arg) * 10; k++)
      engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, 0.1, 0);
  }
}

// Development tests only (?probe): copy one frame back and print its middle
// as text, so a headless browser can check what was drawn.
function readBack(encoder, texture) {
  gpu.probed = true;
  const row = align(texture.width * 4, 256);
  const buffer = gpu.device.createBuffer({ size: row * texture.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow: row }, [texture.width, texture.height]);
  return { buffer, row };
}
async function report({ buffer, row }, texture) {
  await buffer.mapAsync(GPUMapMode.READ);
  const px = new Uint8Array(buffer.getMappedRange());
  const w = texture.width, h = texture.height, cols = 110, rows = 14;
  const x0 = Math.floor(w * 0.3), x1 = Math.floor(w * 0.7), y0 = Math.floor(h * 0.47), y1 = Math.floor(h * 0.53);
  let art = "", peak = 0, sum = 0;
  for (let r = 0; r < rows; r++) {
    let line = "";
    for (let c = 0; c < cols; c++) {
      let m = 0;
      for (let y = y0 + Math.floor((y1 - y0) * r / rows); y < y0 + Math.floor((y1 - y0) * (r + 1) / rows); y++)
        for (let x = x0 + Math.floor((x1 - x0) * c / cols); x < x0 + Math.floor((x1 - x0) * (c + 1) / cols); x++)
          m = Math.max(m, px[y * row + x * 4 + 1]);
      line += " .:-=+*#%@"[Math.min(9, Math.floor(m / 26))];
    }
    art += line + "\n";
  }
  for (let i = 0; i < px.length; i += 4) { peak = Math.max(peak, px[i + 1]); sum += px[i + 1]; }
  console.log(`coaxial probe ${w}x${h} peak ${peak} mean ${(sum / (px.length / 4)).toFixed(2)}\n${art}`);
}

// ---- development: messages from `coax serve` (localhost only; INVARIANTS.md
// allows this note, the one piece of HTML a page draws, in development)
let note = null;
function say(text, error = false) {
  (error ? console.error : console.log)(`coaxial: ${text}`);
  if (!dev) return;
  note ||= Object.assign(document.body.appendChild(document.createElement("pre")), { role: "status" }); // dev only
  note.style.cssText = `position:fixed;left:12px;bottom:12px;margin:0;max-width:80vw;white-space:pre-wrap;font:12px ui-monospace,monospace;padding:6px 8px;border-radius:6px;background:${error ? "#3a0d0d" : "#111"};color:${error ? "#ffb4b4" : "#9a9a9a"};pointer-events:none`; // dev only
  note.textContent = text;
  clearTimeout(note.timer);
  if (!error) note.timer = setTimeout(() => (note.textContent = "", note.style.display = "none"), 2500); // dev only
  note.style.display = "block"; // dev only
}

async function fetchEngine(generation) {
  const response = await fetch(wasmUrl, { cache: dev ? "no-store" : "default" });
  return loadEngine(await response.arrayBuffer(), generation);
}

function live() {
  const socket = new WebSocket(`ws://${location.host}/coaxial/live`);
  socket.onmessage = async (event) => {
    const m = JSON.parse(event.data);
    const t0 = performance.now();
    if (m.kind === "page") { location.reload(); return; }
    if (m.kind === "error") { say(m.text, true); return; }
    if (m.kind === "wgsl" && gpu) { if (await loadShaders()) say(`shaders swapped in ${Math.round(performance.now() - t0)} ms`); return; }
    if (m.kind === "wasm") {
      const generation = ++wanted;
      try {
        if (await fetchEngine(generation)) say(`${app} rebuilt in ${m.ms} ms, swapped in ${Math.round(performance.now() - t0)} ms`);
      } catch (e) { say(`${app}: ${e.message}`, true); }
    }
  };
  socket.onclose = () => setTimeout(live, 1000);
}

async function main() {
  try {
    await fetchEngine(++wanted);
    // No usable WebGPU after all: show the page's text, as written.
    if (!(await startGpu())) { document.documentElement.classList.remove("gpu"); return; }
    observeSize();
    requestAnimationFrame(frame);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    say(`${app}: ${e.message}`, true);
  }
  if (dev) live();
}
main();
