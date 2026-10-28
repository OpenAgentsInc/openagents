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
//   coaxial_key_up(state, key, mods) -> used  the same key let go (a game holds keys)
//   coaxial_blur(state)       the page lost the keyboard: nothing is held
//   coaxial_pointer(state, x, y, phase, kind, id) device pixels; phase 0 down 1 move 2 up 3 cancel;
//                             kind 0 a mouse, 1 a finger or pen; id tells fingers apart
//   coaxial_wheel(state, dy)  the wheel, CSS pixels down (the page doesn't scroll)
//   coaxial_inbox(state), coaxial_inbox_cap(), coaxial_message(state, kind, length)
//                             text in: kind 1 is typed or pasted text; kind 2 the page's data-text,
//                             once each time an engine loads (which project a page shows, say);
//                             others the page's own
//   coaxial_wants_text(state) focus the keyboard field (a phone's keyboard)
//   coaxial_field(state)      where the app draws that field: 4 floats, device pixels
//   coaxial_cursor(state)     0 the default, 1 a link
//   coaxial_hash(state, length) the location's hash (without '#') in the inbox, at load and on every change
//   coaxial_action(state), coaxial_outbox(state), coaxial_outbox_len(state), coaxial_action_done(state)
//                             out: 1 navigates to the outbox's URL, 2 copies its text, 3 makes it the
//                             location's hash (empty: none), in place, 4 keeps it in this browser (one
//                             text per app, given back on the next visit as message 4); others go to the page's
//                             scripts as a "coaxial" event (0 when an engine loads), which
//                             answer with coaxial.send(kind, text)
// and, optionally, sockets: a WebSocket byte pipe, text frames only, at most 4 open. The app (its C face)
// imports from "env":
//   coaxial_socket_open(url, url_len) -> id   0..3, or -1 when 4 are open or the URL is refused
//   coaxial_socket_send(id, ptr, len)         one text frame (UTF-8); dropped unless that socket is open
//   coaxial_socket_close(id)
// and hears back through the inbox: coaxial_message(state, 256 + 4 * id + event, length), event
// 0 opened, 1 a frame (the inbox holds its bytes), 2 closed (the slot is free again), 3 an error
// (a frame over the inbox cap is dropped and arrives as an error). The host reads no frame: what
// they say is the app's.
// and, optionally, HTTP to this site: one request's bytes out, its answer's bytes back. The app imports:
//   coaxial_http(url, url_len, body, body_len, flags) -> id   flags 1 POST (else GET), 2 a beacon (it
//                             outlives a page being left; nothing comes back), 4 the answer is wanted:
//                             id 0..7, its answer comes as message 512 + id, "STATUS\n" then the body
//                             (status 0: no answer); -1 when 8 are waiting or the URL isn't this site's
// and the page itself, through the inbox (what the browser knows; the app decides what to keep):
//   600 the page facts, key=value lines, once an engine loads: tab (a random id for this tab,
//       sessionStorage), fresh (1 when the tab id was just made), visitor (a random id for this
//       browser, localStorage), now (Date.now()), nav (ms since the navigation), url, ref, ua, lang,
//       tz, sw, sh, vw, vh, dpr, coarse, touch, gpu, hidden, webdriver, fcp, dcl
//   601 hidden, 602 shown again, 603 the page is being left (the app's beacon goes in that turn),
//   604 an error the page raised: message, file and line, a line each
// The WGSL has bg_vs/bg_fs (a full-screen pass) and fx_vs/fx_fs (the
// vertices: one vec4f attribute per four floats), sharing one uniform buffer.
//
// A 3D scene, optionally: the page's script names its assets (data-assets,
// space-separated KIND:URL, fetched once; content-named by a deploy):
//   data:URL      bytes into the app's memory, where coaxial_asset(state, i, bytes) says (0: not wanted)
//   storage:URL   a GPU storage buffer
//   index:URL     the scene's index buffer (u32)
//   layers:URL|URL|...  a 2D-array texture, an image a layer, sRGB, mipmapped
//   sound:NAME=URL a sound the app plays by name;  loop:URL  one that loops from the first touch or key
// and each one that arrives (or fails, bytes 0) is told: coaxial_asset_ready(state, i, bytes), i its
// place in the list. The app's scene, drawn between bg and fx with a depth buffer once every GPU asset is in:
//   coaxial_draws(state), coaxial_draw_count(state)   u32 [first, count, base instance, instances] each,
//                             indexed draws over the index buffer
//   coaxial_stores(), coaxial_store_cap(k), coaxial_store(state, k), coaxial_store_bytes(state, k)
//                             memory copied each frame to storage buffers
//   coaxial_sounds(state), coaxial_sounds_len(state)  sounds to play now, a line each: NAME GAIN PAN
// The WGSL's scene_vs/scene_fs read group 1: the storage assets, then the stores, then the texture
// and its sampler, in that order.

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
// steps split by ";": "kTEXT" types keys, "K-2" a named key, "c40,120" clicks at CSS pixels, "m16:TEXT" a
// message, "t1.5" runs the clock that many seconds.
const probeDo = probe ? new URLSearchParams(location.search).get("do") || "" : "";

const memory = new WebAssembly.Memory({ initial: 64, maximum: 1024 }); // 4 MiB, at most 64 MiB
let engine = null;   // { exports, layout }
let arena = null;    // { state, vertices, uniforms, floats, uniformFloats, maxVertices }
let gpu = null;      // { device, context, format, bindGroup, uniformBuffer, vertexBuffer, bg, fx }
let wanted = 0;      // the newest requested engine; an older finished build never replaces it
const assets = parseAssets((script && script.dataset.assets) || "");
let last = 0;

// ---- engine: compile, check its layout, keep or start over its state
async function loadEngine(bytes, generation) {
  const module = await WebAssembly.compile(bytes);
  const imports = { env: { memory, ...socketImports, ...httpImports } };
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
  if (script?.dataset.text) send(2, script.dataset.text);
  for (const a of assets) if (a.done) deliver(a); // a new build gets what the last one had
  sendHash();
  // The page's scripts may talk to the app from now (action 0: ready).
  const kept = keptText();
  if (kept) send(4, kept);
  globalThis.coaxial.ready = true;
  if (x.coaxial_message) send(PAGE_KIND, pageFacts());
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

// ---- sockets: bytes between a WebSocket and the app, nothing else
const SOCKETS = 4, SOCKET_KIND = 256;
const SOCKET_OPENED = 0, SOCKET_FRAME = 1, SOCKET_CLOSED = 2, SOCKET_ERROR = 3;
const sockets = new Array(SOCKETS).fill(null);
const utf8 = (ptr, len) => new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len).slice());
// An event for socket id: its bytes go in the inbox, as typed text does.
function socketEvent(id, event, text = "") {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = new TextEncoder().encode(text);
  if (bytes.length > x.coaxial_inbox_cap()) { socketEvent(id, SOCKET_ERROR, "frame over the inbox cap"); return; }
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_message(arena.state, SOCKET_KIND + 4 * id + event, bytes.length);
  after(false);
}
const socketImports = {
  coaxial_socket_open(ptr, len) {
    const id = sockets.indexOf(null);
    if (id < 0) return -1;
    let ws;
    try { ws = new WebSocket(utf8(ptr, len)); } catch { return -1; }
    sockets[id] = ws;
    const mine = () => sockets[id] === ws;   // a closed socket says nothing more
    ws.onopen = () => mine() && socketEvent(id, SOCKET_OPENED);
    ws.onmessage = (m) => mine() && typeof m.data === "string" && socketEvent(id, SOCKET_FRAME, m.data);
    ws.onerror = () => mine() && socketEvent(id, SOCKET_ERROR);
    ws.onclose = () => { if (mine()) { sockets[id] = null; socketEvent(id, SOCKET_CLOSED); } };
    return id;
  },
  coaxial_socket_send(id, ptr, len) {
    const ws = sockets[id];
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(utf8(ptr, len));
  },
  coaxial_socket_close(id) {
    const ws = sockets[id];
    if (!ws) return;
    sockets[id] = null;
    ws.close();
    // The app hears it a turn later, never inside its own call.
    setTimeout(() => socketEvent(id, SOCKET_CLOSED));
  },
};

// ---- HTTP: bytes to this site and back, nothing else
const HTTPS = 8, HTTP_KIND = 512;
const waiting = new Array(HTTPS).fill(false);
function answer(id, text) {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = new TextEncoder().encode(text);
  send(HTTP_KIND + id, bytes.length > x.coaxial_inbox_cap() ? text.slice(0, text.indexOf("\n") + 1) : text);
}
const httpImports = {
  coaxial_http(urlPtr, urlLen, bodyPtr, bodyLen, flags) {
    const url = utf8(urlPtr, urlLen);
    if (!url.startsWith("/") || url.startsWith("//")) return -1;   // this site only
    const body = bodyLen ? new Uint8Array(memory.buffer, bodyPtr, bodyLen).slice() : null;
    const post = (flags & 1) !== 0, wanted = (flags & 4) !== 0;
    if ((flags & 2) && navigator.sendBeacon && navigator.sendBeacon(url, new Blob([body || ""], { type: "application/json" }))) return 0;
    let id = 0;
    if (wanted) { id = waiting.indexOf(false); if (id < 0) return -1; waiting[id] = true; }
    fetch(url, { method: post ? "POST" : "GET", body: post ? body : undefined, credentials: "same-origin", cache: "no-store",
      keepalive: !wanted && bodyLen < 60000, headers: post ? { "Content-Type": "application/json" } : {} })
      .then(async (r) => { const t = await r.text(); if (wanted) { waiting[id] = false; answer(id, `${r.status}\n${t}`); } })
      .catch(() => { if (wanted) { waiting[id] = false; answer(id, "0\n"); } });
    return id;
  },
};

// ---- the page: what the browser knows about this visit, for the app
const PAGE_KIND = 600, HIDDEN_KIND = 601, SHOWN_KIND = 602, LEAVE_KIND = 603, ERROR_KIND = 604;
const randomId = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
function kept(store, key) {
  try { const v = store.getItem(key); if (v) return [v, false]; const n = randomId(); store.setItem(key, n); return [n, true]; }
  catch { return [randomId(), true]; }   // storage refused (a private window): this page only
}
function pageFacts() {
  const [tab, fresh] = kept(sessionStorage, "coaxial.tab"), [visitor] = kept(localStorage, "coaxial.visitor");
  let tz = ""; try { tz = Intl.DateTimeFormat().resolvedOptions().timeZone || ""; } catch {}
  const nav = performance.getEntriesByType?.("navigation")?.[0], fcp = performance.getEntriesByName?.("first-contentful-paint")?.[0];
  const clean = (v) => String(v ?? "").replace(/[\r\n]/g, " ");
  return Object.entries({ tab, fresh: fresh ? 1 : 0, visitor, now: Date.now(), nav: performance.now().toFixed(1), url: location.href,
    ref: document.referrer, ua: navigator.userAgent, lang: navigator.language, tz, sw: screen.width, sh: screen.height,
    vw: innerWidth, vh: innerHeight, dpr: devicePixelRatio || 1, coarse: matchMedia("(pointer: coarse)").matches ? 1 : 0,
    touch: navigator.maxTouchPoints || 0, gpu: navigator.gpu ? 1 : 0, hidden: document.hidden ? 1 : 0,
    webdriver: navigator.webdriver ? 1 : 0, fcp: fcp ? Math.round(fcp.startTime) : 0, dcl: nav ? Math.round(nav.domContentLoadedEventEnd) : 0 })
    .map(([k, v]) => `${k}=${clean(v)}`).join("\n");
}
addEventListener("visibilitychange", () => send(document.hidden ? HIDDEN_KIND : SHOWN_KIND));
addEventListener("pagehide", () => send(LEAVE_KIND));
addEventListener("error", (e) => send(ERROR_KIND, `${String(e.message || "").slice(0, 200)}\n${String(e.filename || "").split("/").pop()}\n${e.lineno || 0}`));
addEventListener("unhandledrejection", (e) => send(ERROR_KIND, `${String(e.reason?.message || e.reason || "").slice(0, 200)}\npromise\n0`));

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
  // With a scene, every pipeline in the pass shares its depth buffer; only the scene tests and writes it.
  const scene = !!engine?.exports.coaxial_draws;
  const flat = scene ? { depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "always" } } : {};
  const [bg, fx, sc] = await Promise.all([
    device.createRenderPipelineAsync({ layout, ...flat, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: [{ format }] } }),
    device.createRenderPipelineAsync({
      layout, ...flat,
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: {
        color: { operation: "max", srcFactor: "one", dstFactor: "one" }, alpha: { operation: "max", srcFactor: "one", dstFactor: "one" } } }] },
    }),
    scene ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, sceneLayout()] }),
      vertex: { module, entryPoint: "scene_vs" }, fragment: { module, entryPoint: "scene_fs", targets: [{ format }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" },
    }) : null,
  ]);
  gpu.bg = bg; gpu.fx = fx; gpu.scene = sc; // swapped whole, at the next frame
  return true;
}

// ---- the scene's assets and group 1
function parseAssets(text) {
  return text.split(/\s+/).filter(Boolean).map((entry, i) => {
    const k = entry.indexOf(":"), kind = entry.slice(0, k), rest = entry.slice(k + 1);
    if (kind === "sound") { const e = rest.indexOf("="); return { i, kind, name: rest.slice(0, e), url: rest.slice(e + 1) }; }
    return { i, kind, url: rest, urls: rest.split("|") };
  });
}
// Hands an asset that has arrived to the app (data into its memory) and tells it.
function deliver(a) {
  const x = engine?.exports;
  if (!x) return;
  if (a.kind === "data" && a.bytes) {
    const at = x.coaxial_asset?.(arena.state, a.i, a.bytes.length) || 0;
    if (at) new Uint8Array(memory.buffer, at, a.bytes.length).set(a.bytes);
  }
  x.coaxial_asset_ready?.(arena.state, a.i, a.failed ? 0 : a.size);
}
async function loadAsset(a) {
  const { device } = gpu;
  try {
    if (a.kind === "layers") {
      const images = await Promise.all(a.urls.map(async (u) => createImageBitmap(await (await fetch(u)).blob())));
      const [w, h] = [images[0].width, images[0].height];
      const levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
      const texture = device.createTexture({ size: [w, h, images.length], format: "rgba8unorm-srgb", mipLevelCount: levels,
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
      for (const [layer, image] of images.entries())
        for (let level = 0; level < levels; level++) {
          const lw = Math.max(1, w >> level), lh = Math.max(1, h >> level);
          const source = level === 0 && image.width === w && image.height === h ? image
            : await createImageBitmap(image, { resizeWidth: lw, resizeHeight: lh, resizeQuality: "high" });
          device.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer] }, [lw, lh]);
        }
      a.texture = texture; a.size = images.length;
    } else {
      const r = await fetch(a.url);
      if (!r.ok) throw new Error(`${a.url}: ${r.status}`);
      const bytes = new Uint8Array(await r.arrayBuffer());
      a.size = bytes.length;
      if (a.kind === "data") a.bytes = bytes;
      else {
        const usage = (a.kind === "index" ? GPUBufferUsage.INDEX : GPUBufferUsage.STORAGE) | GPUBufferUsage.COPY_DST;
        const whole = new Uint8Array(align(bytes.length, 4)); whole.set(bytes);   // writes come in 4-byte words
        a.buffer = device.createBuffer({ size: whole.length, usage });
        device.queue.writeBuffer(a.buffer, 0, whole);
      }
    }
  } catch (e) { a.failed = true; say(`${app}: ${e.message}`, true); }
  a.done = true;
  deliver(a);
  bindScene();
}
function sceneLayout() {
  if (gpu.sceneLayout) return gpu.sceneLayout;
  const x = engine.exports, entries = [];
  const buffer = { visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } };
  for (const a of assets) if (a.kind === "storage") entries.push({ binding: entries.length, ...buffer });
  for (let k = 0; k < (x.coaxial_stores?.() || 0); k++) entries.push({ binding: entries.length, ...buffer });
  if (assets.some((a) => a.kind === "layers")) {
    entries.push({ binding: entries.length, visibility: GPUShaderStage.FRAGMENT, texture: { viewDimension: "2d-array" } });
    entries.push({ binding: entries.length, visibility: GPUShaderStage.FRAGMENT, sampler: {} });
  }
  return (gpu.sceneLayout = gpu.device.createBindGroupLayout({ entries }));
}
// Group 1, once every GPU asset is in: the storage assets, the stores, the texture, its sampler.
function bindScene() {
  const gpuAssets = assets.filter((a) => ["storage", "index", "layers"].includes(a.kind));
  if (!gpu || gpu.sceneGroup || !engine?.exports.coaxial_draws || gpuAssets.some((a) => !a.done || a.failed)) return;
  const x = engine.exports, { device } = gpu, resources = [];
  for (const a of assets) if (a.kind === "storage") resources.push({ buffer: a.buffer });
  gpu.stores = [];
  for (let k = 0; k < (x.coaxial_stores?.() || 0); k++) {
    const buffer = device.createBuffer({ size: align(Math.max(x.coaxial_store_cap(k), 16), 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    gpu.stores.push(buffer); resources.push({ buffer });
  }
  const layers = assets.find((a) => a.kind === "layers");
  if (layers) {
    resources.push(layers.texture.createView({ dimension: "2d-array" }));
    resources.push(device.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear", addressModeU: "repeat", addressModeV: "repeat", maxAnisotropy: 8 }));
  }
  gpu.index = assets.find((a) => a.kind === "index")?.buffer;
  gpu.sceneGroup = device.createBindGroup({ layout: sceneLayout(), entries: resources.map((resource, binding) => ({ binding, resource })) });
}

// ---- sounds: Web Audio, started by the first touch or key (browsers wait for one)
let audio = null;
const sounds = new Map();   // name -> a decoding or decoded buffer
function startAudio() {
  if (audio || !assets.some((a) => a.kind === "sound" || a.kind === "loop")) return;
  try { audio = new AudioContext(); } catch { return; }
  const loop = assets.find((a) => a.kind === "loop");
  if (loop) decoded(loop.url).then((b) => {
    const s = audio.createBufferSource(), g = audio.createGain();
    s.buffer = b; s.loop = true; g.gain.value = 0.35; s.connect(g).connect(audio.destination); s.start();
  }).catch(() => {});
}
function decoded(url) {
  if (!sounds.has(url)) sounds.set(url, fetch(url).then((r) => r.arrayBuffer()).then((b) => audio.decodeAudioData(b)));
  return sounds.get(url);
}
let playing = 0;
function playSounds(x) {
  const n = x.coaxial_sounds_len(arena.state);
  if (!n || !audio) return;
  const text = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_sounds(arena.state), n).slice());
  for (const line of text.split("\n")) {
    const [name, gain, pan] = line.split(" ");
    const a = assets.find((s) => s.kind === "sound" && s.name === name);
    if (!a || playing >= 16) continue;
    decoded(a.url).then((b) => {
      const s = audio.createBufferSource(), g = audio.createGain(), p = audio.createStereoPanner();
      s.buffer = b; g.gain.value = Number(gain) || 0; p.pan.value = Math.max(-1, Math.min(1, Number(pan) || 0));
      s.connect(g).connect(p).connect(audio.destination); playing++; s.onended = () => playing--; s.start();
    }).catch(() => {});
  }
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
  const x = engine.exports;
  device.queue.writeBuffer(gpu.uniformBuffer, 0, new Float32Array(memory.buffer, arena.uniforms, arena.uniformFloats));
  if (n) device.queue.writeBuffer(gpu.vertexBuffer, 0, new Float32Array(memory.buffer, arena.vertices, n * arena.floats));
  const draws = gpu.scene && gpu.sceneGroup ? x.coaxial_draw_count(arena.state) : 0;
  if (draws) gpu.stores.forEach((buffer, k) => {
    const bytes = Math.min(x.coaxial_store_bytes(arena.state, k), buffer.size) & ~3;
    if (bytes) device.queue.writeBuffer(buffer, 0, memory.buffer, x.coaxial_store(arena.state, k), bytes);
  });
  if (gpu.scene && (!gpu.depth || gpu.depth.width !== w || gpu.depth.height !== h)) {
    gpu.depth?.destroy();
    gpu.depth = device.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT });
  }
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
    ...(gpu.scene ? { depthStencilAttachment: { view: gpu.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "discard" } } : {}) });
  pass.setBindGroup(0, gpu.bindGroup);
  pass.setPipeline(gpu.bg); pass.draw(3);
  if (draws) {
    pass.setPipeline(gpu.scene); pass.setBindGroup(1, gpu.sceneGroup); pass.setIndexBuffer(gpu.index, "uint32");
    const list = new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), draws * 4);
    for (let i = 0; i < draws; i++) if (list[i * 4 + 1] && list[i * 4 + 3]) pass.drawIndexed(list[i * 4 + 1], list[i * 4 + 3], list[i * 4], 0, list[i * 4 + 2]);
  }
  if (n) { pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(n); }
  pass.end();
  const texture = context.getCurrentTexture();
  const readback = probe && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;
  device.queue.submit([encoder.finish()]);
  document.documentElement.classList.add("gpu");
  if (readback) report(readback, texture);
  if (x.coaxial_sounds) playSounds(x);
  after(false);
}

// ---- input: keys, the pointer and typed text go to the app; its actions come out
const KEYS = { Escape: -1, Enter: -2, Backspace: -3, ArrowLeft: -4, ArrowRight: -5, ArrowUp: -6, ArrowDown: -7,
  Home: -8, End: -9, PageUp: -10, PageDown: -11, Delete: -12, Shift: -13 };
const call = (name, ...args) => (engine && engine.exports[name] ? engine.exports[name](arena.state, ...args) : 0);
let field = null;   // INVARIANTS.md: the keyboard field, an invisible input over the app's drawn field (keyboard, paste, context menu)
let touched = false;

// What the app kept in this browser (action 4): storage can be refused, so it is only a convenience.
const keptKey = `coaxial.${app}.kept`;
function keptText() { try { return localStorage.getItem(keptKey) || ""; } catch { return ""; } }

function send(kind, text = "") {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = new TextEncoder().encode(text).subarray(0, x.coaxial_inbox_cap());
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_message(arena.state, kind, bytes.length);
  after(false);
}
globalThis.coaxial = { send, ready: false };

// The location's hash, for an app that opens something by it (a link straight to it).
function sendHash() {
  const x = engine?.exports;
  if (!x?.coaxial_hash || !x.coaxial_inbox) return;
  let hash = location.hash.slice(1);
  try { hash = decodeURIComponent(hash); } catch {}
  const bytes = new TextEncoder().encode(hash).subarray(0, x.coaxial_inbox_cap());
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_hash(arena.state, bytes.length);
  after(false);
}
addEventListener("hashchange", sendHash);

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
    // A copy or cut: the app chose the text (never a masked field's); this runs
    // inside the key event, so the browser lets it reach the clipboard.
    else if (code === 2) navigator.clipboard?.writeText(text).catch(() => {});
    // The app's own place in the page (the hash), replaced so Back still leaves the page.
    else if (code === 3) { if (location.hash.slice(1) !== text) history.replaceState(history.state, "", text ? `#${encodeURIComponent(text)}` : location.pathname + location.search); }
    else if (code === 4) { try { localStorage.setItem(keptKey, text); } catch {} }
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

const keyOf = (e) => KEYS[e.key] ?? ([...e.key].length === 1 ? e.key.codePointAt(0) : 0);
const modsOf = (e) => (e.shiftKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0);
addEventListener("keydown", (e) => {
  if (!engine || e.isComposing) return;
  startAudio();
  const named = KEYS[e.key];
  // Paste is the browser's: it lands in the field, whose input event brings the text.
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "v") return;
  if (!named && e.target === field && !e.metaKey && !e.ctrlKey) return;   // typed text comes by the input event
  const key = keyOf(e);
  if (!key) return;
  if (call("coaxial_key", key, modsOf(e))) e.preventDefault();
  after(true);
});
addEventListener("keyup", (e) => {
  if (!engine?.exports.coaxial_key_up || e.isComposing) return;
  const key = keyOf(e);
  if (key && call("coaxial_key_up", key, modsOf(e))) e.preventDefault();
});
addEventListener("blur", () => call("coaxial_blur"));
addEventListener("visibilitychange", () => { if (document.hidden) call("coaxial_blur"); });
canvas.addEventListener("wheel", (e) => {
  if (!engine?.exports.coaxial_wheel) return;
  e.preventDefault();
  call("coaxial_wheel", e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1));
}, { passive: false });
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
    if (phase === 0) startAudio();
    call("coaxial_pointer", e.clientX * dpr, e.clientY * dpr, phase, e.pointerType === "mouse" ? 0 : 1, e.pointerId);
    after(phase === 2, phase === 2);
  });

function runProbe(w, h, dpr) {
  for (const step of probeDo.split(";").filter(Boolean)) {
    const op = step[0], arg = step.slice(1);
    if (op === "k") for (const c of arg) call("coaxial_key", c.codePointAt(0), 0);
    if (op === "K") call("coaxial_key", Number(arg), 0);
    // "cX,Y": a click (pointer down and up) at CSS pixels X, Y
    if (op === "c") { const [cx, cy] = arg.split(",").map(Number); for (const ph of [0, 2]) call("coaxial_pointer", cx * dpr, cy * dpr, ph, 0, 1); }
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
    // The heavy ones first; sounds are fetched when first played.
    for (const a of assets) if (a.kind !== "sound" && a.kind !== "loop") loadAsset(a);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    say(`${app}: ${e.message}`, true);
  }
  if (dev) live();
}
main();
