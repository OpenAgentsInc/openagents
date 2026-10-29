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
//   coaxial_golden(on)        optional: golden mode, its animated effects held at one moment
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
//                             out: 1 navigates to the outbox's URL (under data-root, if the page has one; or an https:// URL), 2 copies its text, 3 makes it the
//                             location's hash (empty: none), in place, 4 keeps it in this browser (one
//                             text per save slot, in IndexedDB, else localStorage; given back on the next visit as
//                             message 4; the slot is the page's data-slot, a realm's own, else the app's name), 5 keeps it as the browser's playtest player ({key, id, account},
//                             localStorage verse.player, which the landing's join writes too); others go to the page's
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
// and, optionally, entropy: env.coaxial_random(ptr, len) -> Coax ENTROPY_* error
// (0 success). The module exports the shared entropy policy face. The host keeps
// bounded scratch outside app storage; bytes commit only after full CSPRNG success.
// and, optionally, HTTP to this site: one request's bytes out, its answer's bytes back. The app imports:
//   coaxial_http(url, url_len, body, body_len, flags) -> id   flags 1 POST (else GET), 2 a beacon (it
//                             outlives a page being left; nothing comes back), 4 the answer is wanted:
//                             id 0..7, its answer comes as message 512 + id, "STATUS\n" then the body
//                             (status 0: no answer); 8 sends the browser's playtest player key
//                             (X-Verse-Player); -1 when 8 are waiting or the URL isn't this site's
// and optional opaque byte storage: env.coaxial_storage_put/get/delete/cancel, replies at 768 + slot.
// The bounded ABI, raw inbox header and cancellation semantics are in STORAGE.md. No key, event,
// account or cache policy lives in this host; nothing is persisted without an explicit app request.
// and the page itself, through the inbox (what the browser knows; the app decides what to keep):
//   600 the page facts, key=value lines, once an engine loads: tab (a random id for this tab,
//       sessionStorage), fresh (1 when the tab id was just made), visitor (a random id for this
//       browser, localStorage), now (Date.now()), nav (ms since the navigation), url, ref, ua, lang,
//       tz, sw, sh, vw, vh, dpr, coarse, touch, gpu, hidden, webdriver, fcp, dcl, player and account (the
//       playtest player's public id and account, "" none; never its key)
//   601 hidden, 602 shown again, 603 the page is being left (the app's beacon goes in that turn),
//   604 an error the page raised: message, file and line, a line each
// and, on a page with data-drops (drop.js, /projects): files dropped on the page or picked go to
// /api/drops, private to who dropped them. Message 40 is their state, a line each: "S|signed|signer|
// dragging|note" (signer: the browser has a Nostr signer), then "F|name|kind|bytes|permille|status|note"
// per file (status queued, starting, uploading, scanning, kept, refused); message 3 the signed-in pubkey.
// Action 20 opens the file picker; 22 signs in with the browser's Nostr signer (NIP-07, then NIP-98);
// 23 asks for the draft ("D|..." in message 40). The review (rc-review): message 41 is the draft as rows,
// "V|state|edits|note|name" then "R|column|kind|label|preview"; action 24 (a row) brings its whole value
// as message 42 "row|value"; 25 "row|value" saves it; 26 PUBLISH; 27 DISCARD.
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
//   pack:URL      a realm's asset pack (verse/tools/pack_realm.py, docs/verse/realm-host.md): one file,
//                 streamed, that stands for four assets in its place, its catalog (data), vertices
//                 (storage), indices (index) and texture layers (layers), and adds its sounds by name.
//                 While it streams the app hears message 605, "LOADED TOTAL" bytes. One pack a page.
// and each one that arrives (or fails, bytes 0) is told: coaxial_asset_ready(state, i, bytes), i its
// place in the list. The app's scene, drawn between bg and fx with a depth buffer once every GPU asset is in:
//   coaxial_draws(state), coaxial_draw_count(state)   u32 [first, count, base instance, instances] each,
//                             indexed draws over the index buffer
//   coaxial_stores(), coaxial_store_cap(k), coaxial_store(state, k), coaxial_store_bytes(state, k)
//                             memory copied each frame to storage buffers
//   coaxial_sounds(state), coaxial_sounds_len(state)  sounds to play now, a line each: NAME GAIN PAN
// The WGSL's scene_vs/scene_fs read group 1: the storage assets, then the stores, then the texture
// and its sampler, in that order. A WGSL with shadow_vs/shadow_fs also gets a shadow map (shadowLayout).
// Particles, optionally: particles:URL names a shader module of their own (particle_vs/particle_fs,
// group 0 the uniforms, group 1 binding 0 the emitters), drawn after the scene, added, depth-tested:
//   coaxial_particle_cap(), coaxial_particles(state), coaxial_particle_bytes(state)
//                             the emitters' memory, copied each frame to that storage buffer
//   coaxial_particle_instances(state)  quads to draw (6 vertices each)

import { prepareRandomImport, unsupportedImport } from "./entropy-import.js";
import { startDrops } from "./drop.js";
import { startVoice, ACT_VOICE } from "./voice.js";

const script = document.currentScript || document.querySelector("script[data-app]");
const app = (script && script.dataset.app) || "app";
// A deploy names the files by their content (data-wasm, data-wgsl), so a CDN
// can keep them for good; in development they are /build/APP.wasm and /APP.wgsl.
const wasmUrl = (script && script.dataset.wasm) || `/build/${app}.wasm`;
const wgslUrl = (script && script.dataset.wgsl) || `/${app}.wgsl`;
// A realm page (docs/verse/realm-host.md) names its realm (data-realm) and the
// save slot it keeps under (data-slot); data-slot-was is a slot it used
// before, read once when its own is empty, so a renamed slot keeps its save.
const realmId = (script && script.dataset.realm) || "";
const slot = (script && script.dataset.slot) || app;
const slotWas = (script && script.dataset.slotWas) || "";
// A pinned demo (pin.sh) serves its pages under a path of its own (data-root,
// e.g. /demo/playtest); the app's own links stay inside it.
const root = (script && script.dataset.root) || "";
const canvas = document.getElementById("coaxial");
const dev = ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname);
const probe = dev && new URLSearchParams(location.search).has("probe"); // tests: read the frame back
// ?probe&at=SECONDS: run the app's clock that far in 0.1 s steps before the
// probed frame, so a headless browser can see a moment of the animation.
const probeAt = probe ? Number(new URLSearchParams(location.search).get("at")) || 0 : 0;
// Reduced motion draws the finished frame at once (probes use it too: headless browsers barely tick).
const still = (probe && !probeAt) || matchMedia("(prefers-reduced-motion: reduce)").matches;
// Golden mode (goldens.ts sets coaxialGolden; ?golden by hand): an app that exports coaxial_golden holds its effects still.
const golden = globalThis.coaxialGolden === true || new URLSearchParams(location.search).has("golden");
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
const PACK_PARTS = ["data", "storage", "index", "layers"], PACK_KIND = 605;
const PACK_CATALOG = 1, PACK_VERTICES = 2, PACK_INDICES = 3, PACK_LAYER = 4, PACK_SOUND = 5;
const assets = parseAssets((script && script.dataset.assets) || "");
let last = 0;
const randomScratch = {}; // lazily reserved once, shared by synchronous engine calls

// ---- engine: compile, check its layout, keep or start over its state
async function loadEngine(bytes, generation) {
  const module = await WebAssembly.compile(bytes);
  const random = prepareRandomImport(memory, module, randomScratch);
  const imports = Object.create(null);
  imports.env = Object.assign(Object.create(null), { memory, ...socketImports, ...httpImports, ...random.imports, ...storageImports });
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= Object.create(null))[imp.name] ||= wasiStub(imp.module, imp.name);
  }
  const instance = await WebAssembly.instantiate(module, imports);
  if (generation !== wanted) return false; // a newer build was asked for meanwhile
  const kept = await keptRead;   // before the first frame, so a kept world opens with it
  if (generation !== wanted) return false;
  byteStorage.reset(); // pending effects belong to the old engine, even if its state layout survives
  const x = instance.exports;
  random.bind(x);
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
  if (golden) x.coaxial_golden?.(1);
  x.coaxial_quality?.(arena.state, golden ? 0 : quality);
  engine = { exports: x, layout };
  if (script?.dataset.text) send(2, script.dataset.text);
  for (const a of assets) if (a.done) deliver(a); // a new build gets what the last one had
  sendHash();
  // The page's scripts may talk to the app from now (action 0: ready).
  if (kept) send(4, kept);
  globalThis.coaxial.ready = true;
  if (x.coaxial_message) send(PAGE_KIND, pageFacts());
  viewer();
  dispatchEvent(new CustomEvent("coaxial", { detail: { code: 0, text: "" } }));
  return true;
}

function wasiStub(module, name) {
  if (module !== "wasi_snapshot_preview1") return unsupportedImport(module, name);
  if (name === "fd_write") return (fd, iovs, n, written) => {
    const v = new DataView(memory.buffer); let text = "", total = 0;
    for (let i = 0; i < n; i++) {
      const p = v.getUint32(iovs + i * 8, true), len = v.getUint32(iovs + i * 8 + 4, true);
      text += new TextDecoder().decode(new Uint8Array(memory.buffer, p, len)); total += len;
    }
    v.setUint32(written, total, true); (fd === 2 ? console.error : console.log)(text.trimEnd()); return 0;
  };
  if (name === "proc_exit") return (code) => { throw new Error(`${app} exited (${code})`); };
  // Existing libc optional probes may be unavailable. Return WASI ENOSYS,
  // without pretending to initialize their output buffers. All other imports,
  // especially random_get, refuse before the module can run.
  if (["fd_fdstat_get", "fd_seek", "fd_close", "clock_time_get"].includes(name)) return () => 52;
  return unsupportedImport(module, name);
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
    const headers = post ? { "Content-Type": "application/json" } : {};
    const who = flags & 8 ? player() : null;
    if (who) headers["X-Verse-Player"] = who.key;
    fetch(url, { method: post ? "POST" : "GET", body: post ? body : undefined, credentials: "same-origin", cache: "no-store",
      keepalive: !wanted && bodyLen < 60000, headers })
      .then(async (r) => { const t = await r.text(); if (wanted) { waiting[id] = false; answer(id, `${r.status}\n${t}`); } })
      .catch(() => { if (wanted) { waiting[id] = false; answer(id, "0\n"); } });
    return id;
  },
};

// ---- IndexedDB: bounded opaque bytes; namespaces and versions are chosen by Coax
// The factory's dependencies are platform effects. Tests exercise this same implementation
// with actual IndexedDB/Wasm, and inject refusal/timeout effects separately.
function makeByteStorage({ memory, capacity, emit, indexedDB, timeout = 10000 }) {
  const HEADER = 16, MAX_BYTES = 1048576, SLOTS = 8;
  const OK = 0, MISSING = 1, UNAVAILABLE = 2, QUOTA = 3, LIMIT = 4, INVALID = 5;
  const BUSY = 6, ERROR = 7, TIMEOUT = 8, CAPACITY = 9, CORRUPT = 10;
  const requests = new Array(SLOTS).fill(null), decoder = new TextDecoder("utf-8", { fatal: true });
  let db = null, opening = null, generation = 0;
  const validToken = (token) => Number.isInteger(token) && token !== 0 && token >= -2147483648 && token <= 0xffffffff;
  const errorCode = (e) => e?.name === "QuotaExceededError" ? QUOTA
    : ["SecurityError", "NotAllowedError", "InvalidStateError", "NotFoundError", "VersionError"].includes(e?.name) ? UNAVAILABLE : ERROR;
  const mine = (r) => requests[r.slot] === r && r.generation === generation;
  const abort = (r) => { try { r.tx?.abort(); } catch {} };
  function forget(r) {
    requests[r.slot] = null;
    clearTimeout(r.timer);
  }
  function finish(r, status, payload = null, needed = payload?.byteLength || 0) {
    if (!mine(r)) return;
    forget(r); // Coax may reuse the slot while handling its reply
    const cap = capacity();
    if (!Number.isInteger(cap) || cap < HEADER) return; // receiver disappeared; never truncate a header
    if (payload && payload.byteLength > cap - HEADER) { status = CAPACITY; payload = null; }
    const wire = new Uint8Array(HEADER + (payload?.byteLength || 0)), h = new DataView(wire.buffer);
    h.setUint32(0, r.token, true); h.setUint32(4, status, true);
    h.setUint32(8, wire.length - HEADER, true); h.setUint32(12, needed, true);
    if (payload) wire.set(new Uint8Array(payload), HEADER);
    emit(r.slot, wire);
  }
  function open() {
    if (db) return Promise.resolve([db, OK]);
    if (opening) return opening;
    const attempt = new Promise((resolve) => {
      let request, settled = false;
      const timer = setTimeout(() => done(null, TIMEOUT), timeout);
      function done(connection, status) {
        if (settled) { connection?.close(); return; }
        settled = true; clearTimeout(timer);
        if (connection) {
          db = connection;
          connection.onversionchange = () => {
            if (db === connection) db = null;
            connection.close();
            for (const r of requests) if (r?.tx?.db === connection) { abort(r); finish(r, UNAVAILABLE); }
          };
          connection.onclose = () => { if (db === connection) db = null; };
        }
        resolve([connection, status]);
      }
      try {
        const factory = indexedDB();
        if (!factory) { done(null, UNAVAILABLE); return; }
        request = factory.open("coaxial-bytes", 1);
        request.onupgradeneeded = () => {
          try { if (!request.result.objectStoreNames.contains("bytes")) request.result.createObjectStore("bytes"); }
          catch (e) { try { request.transaction?.abort(); } catch {} done(null, errorCode(e)); }
        };
        request.onsuccess = () => done(request.result, OK);
        request.onerror = () => done(null, errorCode(request.error));
        request.onblocked = () => done(null, UNAVAILABLE);
      } catch (e) { done(null, errorCode(e)); }
    });
    opening = attempt;
    attempt.then(() => { if (opening === attempt) opening = null; });
    return attempt;
  }
  function span(ptr, len) {
    const buffer = memory().buffer;
    if (!Number.isInteger(ptr) || !Number.isInteger(len) || ptr < 0 || len < 0 || ptr > buffer.byteLength || len > buffer.byteLength - ptr) throw INVALID;
    return new Uint8Array(buffer, ptr, len);
  }
  function text(ptr, len, max) {
    if (!Number.isInteger(len) || len < 1 || len > max) throw INVALID;
    return decoder.decode(span(ptr, len)); // reject malformed UTF-8; do not normalize keys
  }
  function start(op, slot, token, np, nn, version, kp, kn, vp = 0, vn = 0) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= SLOTS || !validToken(token)) return INVALID;
    if (requests[slot]) return BUSY;
    if (!Number.isInteger(version) || version < 1 || version > 0x7fffffff) return INVALID;
    const cap = capacity();
    if (!Number.isInteger(cap) || cap < HEADER) return CAPACITY;
    let key, bytes;
    try {
      // Bound allocations and copy bytes before the caller can mutate/grow its memory.
      if (op === "put" && (!Number.isInteger(vn) || vn < 0)) return INVALID;
      if (op === "put" && vn > MAX_BYTES) return LIMIT;
      key = [text(np, nn, 64), version, text(kp, kn, 128)];
      if (op === "put") bytes = span(vp, vn).slice().buffer;
    } catch { return INVALID; }
    const r = { slot, token: token >>> 0, generation, tx: null, timer: null };
    requests[slot] = r;
    r.timer = setTimeout(() => { if (mine(r)) { abort(r); finish(r, TIMEOUT); } }, timeout);
    open().then(([connection, status]) => {
      if (!mine(r)) return;
      if (!connection) { finish(r, status); return; }
      try {
        const tx = connection.transaction("bytes", op === "get" ? "readonly" : "readwrite");
        r.tx = tx;
        const store = tx.objectStore("bytes");
        const request = op === "put" ? store.put(bytes, key) : op === "get" ? store.get(key) : store.delete(key);
        let value, failure = OK;
        request.onsuccess = () => { if (op === "get") value = request.result; };
        request.onerror = () => { failure = errorCode(request.error); }; // default abort remains enabled
        tx.onabort = () => finish(r, failure || errorCode(tx.error));
        tx.oncomplete = () => {
          if (failure) { finish(r, failure); return; }
          if (op !== "get") { finish(r, OK); return; }
          if (value === undefined) { finish(r, MISSING); return; }
          if (!(value instanceof ArrayBuffer)) { finish(r, CORRUPT); return; }
          if (value.byteLength > MAX_BYTES) { finish(r, LIMIT, null, value.byteLength); return; }
          finish(r, OK, value);
        };
      } catch (e) { abort(r); finish(r, errorCode(e)); }
    }); // open resolves every refusal: no unhandled promise or exception crosses the ABI
    return OK;
  }
  function cancel(slot, token) {
    if (!Number.isInteger(slot) || slot < 0 || slot >= SLOTS || !validToken(token)) return INVALID;
    const r = requests[slot];
    if (!r || r.token !== (token >>> 0)) return MISSING;
    forget(r); abort(r); // a committed write may already have won; cancellation suppresses its reply
    return OK;
  }
  return {
    imports: {
      coaxial_storage_put: (slot, token, np, nn, version, kp, kn, vp, vn) => start("put", slot, token, np, nn, version, kp, kn, vp, vn),
      coaxial_storage_get: (slot, token, np, nn, version, kp, kn) => start("get", slot, token, np, nn, version, kp, kn),
      coaxial_storage_delete: (slot, token, np, nn, version, kp, kn) => start("delete", slot, token, np, nn, version, kp, kn),
      coaxial_storage_cancel: cancel,
    },
    reset() { generation++; for (const r of requests) if (r) { forget(r); abort(r); } },
  };
}
const byteStorage = makeByteStorage({
  memory: () => memory,
  indexedDB: () => globalThis.indexedDB,
  capacity: () => engine?.exports.coaxial_message && engine.exports.coaxial_inbox && arena ? engine.exports.coaxial_inbox_cap?.() || 0 : 0,
  emit: (slot, bytes) => {
    const x = engine?.exports;
    if (!x?.coaxial_message || !x.coaxial_inbox || !arena) return;
    const dest = x.coaxial_inbox(arena.state);
    if (dest < 0 || dest > memory.buffer.byteLength || bytes.length > memory.buffer.byteLength - dest) return;
    new Uint8Array(memory.buffer, dest, bytes.length).set(bytes);
    x.coaxial_message(arena.state, 768 + slot, bytes.length);
    after(false);
  },
});
const storageImports = byteStorage.imports;

// ---- the page: what the browser knows about this visit, for the app
const PAGE_KIND = 600, HIDDEN_KIND = 601, SHOWN_KIND = 602, LEAVE_KIND = 603, ERROR_KIND = 604, VIEWER_KIND = 606;
// Who is logged in, the same on every page: /api/auth/me's lines (K| npub, H| hex key, M| a member key it
// speaks for, J| a private project), or "" when nobody is. Apps that care read VIEWER_KIND.
function viewer() {
  if (!engine?.exports.coaxial_message || location.protocol === "file:") return;
  fetch("/api/auth/me", { credentials: "same-origin" })
    .then((r) => (r.ok ? r.text() : ""), () => "")
    .then((t) => send(VIEWER_KIND, t), () => send(VIEWER_KIND, ""));
}
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
    webdriver: navigator.webdriver ? 1 : 0, fcp: fcp ? Math.round(fcp.startTime) : 0, dcl: nav ? Math.round(nav.domContentLoadedEventEnd) : 0,
    player: player()?.id || "", account: player()?.account || "", realm: realmId, slot })
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
  if (gpu.sans) useSans();
}

// The sans atlas: a page whose shader declares sans_atlas (binding 1) and
// sans_sampler (binding 2) gets the system sans rasterized here, so small text
// reads like the rest of the web. Characters: ASCII 32..126, then SANS_EXTRA;
// 16 columns of 96-texel cells, the pen at x 16 and the baseline at y 72 of
// each, drawn at 64 px; every mip level is drawn at its own size rather than
// downsampled. Coax lays the text out with the same fonts' widths
// (apps/openagents-web/font.coax, "the system sans"), so this list, the cell
// and the font must match it.
const SANS_EXTRA = [183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800];
const SANS_FONT = 'Helvetica, Arial, "Liberation Sans", "Nimbus Sans", sans-serif';
function sansAtlas(device) {
  const codes = [...Array.from({ length: 95 }, (_, i) => 32 + i), ...SANS_EXTRA];
  const cols = 16, rows = Math.ceil(codes.length / cols), cell = 96, levels = 5;
  const texture = device.createTexture({ size: [cols * cell, rows * cell], format: "rgba8unorm", mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  for (let level = 0; level < levels; level++) {
    const c = cell >> level, w = cols * c, h = rows * c;
    const canvas = new OffscreenCanvas(w, h), g = canvas.getContext("2d");
    g.fillStyle = "#fff"; g.textBaseline = "alphabetic";
    g.font = `${64 / (1 << level)}px ${SANS_FONT}`;
    codes.forEach((code, i) => g.fillText(String.fromCodePoint(code), (i % cols) * c + 16 / (1 << level), Math.floor(i / cols) * c + 72 / (1 << level)));
    device.queue.copyExternalImageToTexture({ source: canvas }, { texture, mipLevel: level, premultipliedAlpha: true }, [w, h]);
  }
  return texture;
}
function useSans() {
  const { device } = gpu;
  gpu.sans ||= sansAtlas(device);
  const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
  gpu.layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: both, buffer: {} },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
    { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: {} }] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [
    { binding: 0, resource: { buffer: gpu.uniformBuffer } },
    { binding: 1, resource: gpu.sans.createView() },
    { binding: 2, resource: device.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear" }) }] });
}

async function loadShaders() {
  const { device, format } = gpu;
  const code = await (await fetch(wgslUrl, { cache: dev ? "no-store" : "default" })).text();
  if (/var\s+sans_atlas\b/.test(code)) useSans();
  const module = device.createShaderModule({ code });
  const info = await module.getCompilationInfo();
  const errors = info.messages.filter((m) => m.type === "error");
  if (errors.length) { say(errors.map((m) => `${app}.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return false; }
  const layout = device.createPipelineLayout({ bindGroupLayouts: [gpu.layout] });
  const stride = arena.floats * 4;
  // With a scene, every pipeline in the pass shares its depth buffer; only the scene tests and writes it.
  const scene = !!engine?.exports.coaxial_draws;
  gpu.scene3d = scene;
  const flat = scene ? { depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "always" } } : {};
  const shadows = scene && /fn\s+shadow_vs\b/.test(code);
  const post = scene && /fn\s+post_fs\b/.test(code);
  const lit = post ? [{ format: "rgba16float" }, { format: "rgba16float" }] : [{ format }];
  const sceneGroups = scene ? [gpu.layout, sceneLayout(), ...(shadows ? [shadowLayout()] : [])] : [];
  const [bg, fx, sc, sh, bl, po] = await Promise.all([
    device.createRenderPipelineAsync({ layout, ...flat, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: lit } }),
    device.createRenderPipelineAsync({
      layout, ...(post ? {} : flat),
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: {
        color: { operation: "max", srcFactor: "one", dstFactor: "one" }, alpha: { operation: "max", srcFactor: "one", dstFactor: "one" } } }] },
    }),
    scene ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: sceneGroups }),
      vertex: { module, entryPoint: "scene_vs" }, fragment: { module, entryPoint: "scene_fs", targets: lit },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" },
    }) : null,
    shadows ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, sceneLayout()] }),
      vertex: { module, entryPoint: "shadow_vs" }, fragment: { module, entryPoint: "shadow_fs", targets: [] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less", depthBias: 2, depthBiasSlopeScale: 2 },
    }) : null,
    post ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, postLayouts().bloom] }),
      vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bloom_fs", targets: [{ format: "rgba16float" }] },
    }) : null,
    post ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, postLayouts().post] }),
      vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "post_fs", targets: [{ format }] },
    }) : null,
  ]);
  gpu.bg = bg; gpu.fx = fx; gpu.scene = sc; gpu.shadow = sh; gpu.bloom = bl; gpu.post = po; // swapped whole, at the next frame
  gpu.particles = sc ? await particlePipeline(post) : null;
  return true;
}

// The particles' own module and pipeline; a page without one, or one that doesn't compile, draws none.
async function particlePipeline(post) {
  const a = assets.find((s) => s.kind === "particles"), x = engine?.exports;
  if (!a || !x?.coaxial_particles) return null;
  const { device, format } = gpu;
  try {
    const module = device.createShaderModule({ code: await (await fetch(a.url, { cache: dev ? "no-store" : "default" })).text() });
    const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
    if (errors.length) { say(errors.map((m) => `particles.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return null; }
    const group = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } }] });
    gpu.particleBuffer ||= device.createBuffer({ size: align(Math.max(x.coaxial_particle_cap(), 16), 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    gpu.particleGroup = device.createBindGroup({ layout: group, entries: [{ binding: 0, resource: { buffer: gpu.particleBuffer } }] });
    const add = { operation: "add", srcFactor: "one", dstFactor: "one" };
    return await device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, group] }),
      vertex: { module, entryPoint: "particle_vs" }, fragment: { module, entryPoint: "particle_fs", targets: post
        ? [{ format: "rgba16float", blend: { color: add, alpha: add } }, { format: "rgba16float", writeMask: 0 }]
        : [{ format, blend: { color: add, alpha: add } }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" },
    });
  } catch (e) { say(`${app}: particles: ${e.message}`, true); return null; }
}

// ---- the scene's assets and group 1
function parseAssets(text) {
  const list = [];
  for (const entry of text.split(/\s+/).filter(Boolean)) {
    const k = entry.indexOf(":"), kind = entry.slice(0, k), rest = entry.slice(k + 1), i = list.length;
    if (kind === "sound") { const e = rest.indexOf("="); list.push({ i, kind, name: rest.slice(0, e), url: rest.slice(e + 1) }); }
    else if (kind === "pack") { const pack = { url: rest }; for (const part of PACK_PARTS) list.push({ i: list.length, kind: part, pack }); }
    else list.push({ i, kind, url: rest, urls: rest.split("|") });
  }
  return list;
}
// ---- a realm's asset pack (verse/tools/pack_realm.py has the format)
async function loadPack(pack) {
  const parts = assets.filter((a) => a.pack === pack);
  let entries = null;
  try {
    const r = await fetch(pack.url);
    if (!r.ok || !r.body) throw new Error(`${pack.url}: ${r.status}`);
    const reader = r.body.getReader(), chunks = [];
    let loaded = 0, total = Number(r.headers.get("content-length")) || 0, told = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value); loaded += value.length;
      if (loaded >= 12 && chunks[0].length >= 12) total = new DataView(chunks[0].buffer, chunks[0].byteOffset).getUint32(8, true);
      if (performance.now() - told > 100) { told = performance.now(); send(PACK_KIND, `${loaded} ${total}`); }
    }
    const bytes = new Uint8Array(loaded);
    let at = 0; for (const c of chunks) { bytes.set(c, at); at += c.length; }
    send(PACK_KIND, `${loaded} ${loaded}`);
    entries = unpack(bytes);
    const { device } = gpu, of = (kind) => entries.filter((e) => e.kind === kind);
    const [catalog, vertices, indices, layers] = parts;
    catalog.bytes = of(PACK_CATALOG)[0].bytes; catalog.size = catalog.bytes.length;
    for (const [a, e] of [[vertices, of(PACK_VERTICES)[0]], [indices, of(PACK_INDICES)[0]]]) {
      const whole = new Uint8Array(align(Math.max(e.bytes.length, 4), 4)); whole.set(e.bytes);
      a.buffer = device.createBuffer({ size: whole.length, usage: (a.kind === "index" ? GPUBufferUsage.INDEX : GPUBufferUsage.STORAGE) | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(a.buffer, 0, whole); a.size = e.bytes.length;
    }
    const images = await Promise.all(of(PACK_LAYER).map((e) => createImageBitmap(new Blob([e.bytes]))));
    if (images.length) { layers.texture = await layerTexture(images); layers.size = images.length; }
    else layers.failed = true;
    for (const e of of(PACK_SOUND)) assets.push({ i: assets.length, kind: "sound", name: e.name, bytes: e.bytes, done: true });
  } catch (e) { for (const a of parts) a.failed = true; say(`${app}: ${e.message}`, true); }
  for (const a of parts) { a.done = true; deliver(a); }
  bindScene();
}
// The pack's entries: its header and table checked against its length; nothing outside it is read.
function unpack(bytes) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.length);
  if (bytes.length < 32 || String.fromCharCode(...bytes.subarray(0, 4)) !== "VPK1" || v.getUint32(8, true) !== bytes.length) throw new Error("not a whole asset pack");
  const n = v.getUint32(4, true), table = v.getUint32(16, true), out = [];
  if (table + n * 48 > bytes.length) throw new Error("asset pack: its table runs past its end");
  for (let k = 0; k < n; k++) {
    const at = table + k * 48, offset = v.getUint32(at + 4, true), length = v.getUint32(at + 8, true);
    if (offset + length > bytes.length) throw new Error("asset pack: an entry runs past its end");
    const name = new TextDecoder().decode(bytes.subarray(at + 16, at + 48)).replace(/\0.*$/s, "");
    out.push({ kind: bytes[at], licence: bytes[at + 1], name, bytes: bytes.subarray(offset, offset + length) });
  }
  return out;
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
      a.texture = await layerTexture(images); a.size = images.length;
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
// A 2D-array texture, an image a layer, sRGB, mipmapped (every image at the first one's size).
async function layerTexture(images) {
  const { device } = gpu, [w, h] = [images[0].width, images[0].height];
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
  return texture;
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
// The shadow map, when the WGSL has shadow_vs/shadow_fs: the scene drawn first from its key
// light into a depth texture (2048 square, 1024 on a phone), which scene_fs reads as group 2:
// the map, then a comparison sampler.
function shadowLayout() {
  if (gpu.shadowLayout) return gpu.shadowLayout;
  const { device } = gpu, size = matchMedia("(pointer: coarse)").matches ? 1024 : 2048;
  gpu.shadowLayout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "depth" } },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "comparison" } }] });
  gpu.shadowMap = device.createTexture({ size: [size, size], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
  gpu.shadowGroup = device.createBindGroup({ layout: gpu.shadowLayout, entries: [
    { binding: 0, resource: gpu.shadowMap.createView() },
    { binding: 1, resource: device.createSampler({ compare: "less", magFilter: "linear", minFilter: "linear" }) }] });
  return gpu.shadowLayout;
}
// The post pass, when the WGSL has post_fs: bg_fs and scene_fs draw light (rgba16float, its alpha
// the share of it that blooms) and a second target, aux (rgba16float), instead of the screen;
// bloom_fs runs five times, each pass's group 1 a source, a sampler and its vec4f (x, y the
// blur's step in source texels, z 0 to take the bright parts, 1 to blur, w how many source
// texels a target texel spans): the bright parts to a quarter of the size, blurred across and
// down, then to an eighth, blurred across and down. post_fs then draws the screen from group 1: the
// light, the quarter, the eighth, aux, the depth, a sampler; the HUD (fx) goes over it.
function postLayouts() {
  if (gpu.postLayouts) return gpu.postLayouts;
  const f = GPUShaderStage.FRAGMENT, device = gpu.device;
  const tex = (binding) => ({ binding, visibility: f, texture: {} });
  gpu.postLayouts = {
    bloom: device.createBindGroupLayout({ entries: [tex(0), { binding: 1, visibility: f, sampler: {} }, { binding: 2, visibility: f, buffer: {} }] }),
    post: device.createBindGroupLayout({ entries: [tex(0), tex(1), tex(2), tex(3),
      { binding: 4, visibility: f, texture: { sampleType: "depth" } }, { binding: 5, visibility: f, sampler: {} }] }),
  };
  gpu.linear = device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
  gpu.steps = [[0, 0, 0, 4], [1, 0, 1, 1], [0, 1, 1, 1], [1, 0, 1, 2], [0, 1, 1, 1]].map((v) => {
    const b = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    device.queue.writeBuffer(b, 0, new Float32Array(v));
    return b;
  });
  return gpu.postLayouts;
}
// The post pass's textures at the canvas's size: [view, source, step] for each bloom pass, and post_fs's group.
function postTargets(w, h) {
  const { device } = gpu, use = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING;
  for (const t of gpu.postTextures || []) t.destroy();
  const make = (tw, th, format) => device.createTexture({ size: [Math.max(1, tw), Math.max(1, th)], format, usage: use });
  const light = make(w, h, "rgba16float"), aux = make(w, h, "rgba16float");
  const q0 = make(Math.ceil(w / 4), Math.ceil(h / 4), "rgba16float"), q1 = make(q0.width, q0.height, "rgba16float");
  const e0 = make(Math.ceil(w / 8), Math.ceil(h / 8), "rgba16float"), e1 = make(e0.width, e0.height, "rgba16float");
  gpu.depth?.destroy();
  gpu.depth = make(w, h, "depth32float");
  gpu.postTextures = [light, aux, q0, q1, e0, e1];
  const { bloom, post } = postLayouts();
  const group = (src, k) => device.createBindGroup({ layout: bloom, entries: [
    { binding: 0, resource: src.createView() }, { binding: 1, resource: gpu.linear }, { binding: 2, resource: { buffer: gpu.steps[k] } }] });
  gpu.light = light; gpu.aux = aux;
  gpu.blooms = [[q0, group(light, 0)], [q1, group(q0, 1)], [q0, group(q1, 2)], [e1, group(q0, 3)], [e0, group(e1, 4)]];
  gpu.postGroup = device.createBindGroup({ layout: post, entries: [light, q0, e0, aux, gpu.depth]
    .map((t, binding) => ({ binding, resource: t.createView() })).concat([{ binding: 5, resource: gpu.linear }]) });
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
  if (loop) decoded(loop).then((b) => {
    const s = audio.createBufferSource(), g = audio.createGain();
    s.buffer = b; s.loop = true; g.gain.value = 0.35; s.connect(g).connect(audio.destination); s.start();
  }).catch(() => {});
}
function decoded(a) {
  const key = a.url || `pack:${a.name}`;
  if (!sounds.has(key)) sounds.set(key, (a.bytes ? Promise.resolve(a.bytes.slice().buffer) : fetch(a.url).then((r) => r.arrayBuffer())).then((b) => audio.decodeAudioData(b)));
  return sounds.get(key);
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
    decoded(a).then((b) => {
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
// Graphics on a page with a 3D scene: HIGH draws every device pixel, MEDIUM at most 1.5 a CSS
// pixel, LOW 1 (the browser scales the canvas up). Phones start at MEDIUM: their screens have up to
// 9 times the pixels of the page, and a scene's shaders run on each. The app's action 6 names a new
// one and coaxial_quality(state, 0|1|2) tells the app the one in force; kept in localStorage
// verse.quality. Goldens always draw HIGH.
const QUALITY = ["high", "medium", "low"], QUALITY_CAP = [Infinity, 1.5, 1];
let quality = (() => {
  try { const q = QUALITY.indexOf(localStorage.getItem("verse.quality")); if (q >= 0) return q; } catch {}
  return matchMedia("(pointer: coarse)").matches ? 1 : 0;
})();
function setQuality(name) {
  const q = QUALITY.indexOf(name);
  if (q < 0) return;
  quality = q;
  try { localStorage.setItem("verse.quality", name); } catch {}
}
// The share of the device pixels a frame draws.
const drawn = (deviceWidth) => {
  if (!gpu?.scene3d || golden || !canvas.clientWidth) return 1;
  return Math.min(1, QUALITY_CAP[quality] / (deviceWidth / canvas.clientWidth));
};
// Device pixels per CSS pixel, as the canvas is actually drawn.
const scale = () => (canvas.width && canvas.clientWidth ? canvas.width / canvas.clientWidth : devicePixelRatio || 1);

// ---- the frame
function frame(now) {
  requestAnimationFrame(frame);
  if (!engine || !gpu?.bg) return;
  const dt = last ? (now - last) / 1000 : 0; last = now;
  const [dw, dh] = devicePixels || [Math.max(1, Math.round(canvas.clientWidth * devicePixelRatio)), Math.max(1, Math.round(canvas.clientHeight * devicePixelRatio))];
  const f = drawn(dw), w = Math.max(1, Math.round(dw * f)), h = Math.max(1, Math.round(dh * f));
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
  if (gpu.scene && (!gpu.depth || gpu.depth.width !== w || gpu.depth.height !== h || !!gpu.post !== !!gpu.postGroup)) {
    if (gpu.post) postTargets(w, h);
    else {
      gpu.depth?.destroy(); gpu.postGroup = null;
      gpu.depth = device.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT });
    }
  }
  const post = gpu.post && gpu.postGroup;
  const encoder = device.createCommandEncoder();
  const list = draws ? new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), draws * 4) : null;
  const drawScene = (pass) => {
    pass.setBindGroup(1, gpu.sceneGroup); pass.setIndexBuffer(gpu.index, "uint32");
    for (let i = 0; i < draws; i++) if (list[i * 4 + 1] && list[i * 4 + 3]) pass.drawIndexed(list[i * 4 + 1], list[i * 4 + 3], list[i * 4], 0, list[i * 4 + 2]);
  };
  if (draws && gpu.shadow) {
    const shade = encoder.beginRenderPass({ colorAttachments: [],
      depthStencilAttachment: { view: gpu.shadowMap.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" } });
    shade.setBindGroup(0, gpu.bindGroup); shade.setPipeline(gpu.shadow); drawScene(shade); shade.end();
  }
  const target = (view) => ({ view, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } });
  const screen = () => target(context.getCurrentTexture().createView());
  let pass = encoder.beginRenderPass({ colorAttachments: post ? [target(gpu.light.createView()), target(gpu.aux.createView())] : [screen()],
    ...(gpu.scene ? { depthStencilAttachment: { view: gpu.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: post ? "store" : "discard" } } : {}) });
  pass.setBindGroup(0, gpu.bindGroup);
  pass.setPipeline(gpu.bg); pass.draw(3);
  if (draws) {
    pass.setPipeline(gpu.scene);
    if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
    drawScene(pass);
    const sparks = gpu.particles ? x.coaxial_particle_instances(arena.state) : 0;
    const bytes = sparks ? Math.min(x.coaxial_particle_bytes(arena.state), gpu.particleBuffer.size) & ~3 : 0;
    if (bytes) {
      device.queue.writeBuffer(gpu.particleBuffer, 0, memory.buffer, x.coaxial_particles(arena.state), bytes);
      pass.setPipeline(gpu.particles); pass.setBindGroup(1, gpu.particleGroup); pass.draw(6, sparks);
    }
  }
  if (post) {
    pass.end();
    for (const [into, group] of gpu.blooms) {
      const b = encoder.beginRenderPass({ colorAttachments: [target(into.createView())] });
      b.setBindGroup(0, gpu.bindGroup); b.setBindGroup(1, group); b.setPipeline(gpu.bloom); b.draw(3); b.end();
    }
    pass = encoder.beginRenderPass({ colorAttachments: [screen()] });
    pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.postGroup); pass.setPipeline(gpu.post); pass.draw(3);
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
  Home: -8, End: -9, PageUp: -10, PageDown: -11, Delete: -12, Shift: -13, Control: -14 };
const call = (name, ...args) => (engine && engine.exports[name] ? engine.exports[name](arena.state, ...args) : 0);
let field = null;   // INVARIANTS.md: the keyboard field, an invisible input over the app's drawn field (keyboard, paste, context menu)
let touched = false;

// What the app kept in this browser (action 4): IndexedDB (database coaxial, store kept, the save
// slot), else localStorage. Storage can be refused, so it is only a convenience. A text kept in
// localStorage before IndexedDB is read once and moves over at the next keep.
const keptKey = `coaxial.${slot}.kept`;
let keptDb = null;
const keptRead = (async () => {
  keptDb = await new Promise((ok) => {
    try { const r = indexedDB.open("coaxial", 1); r.onupgradeneeded = () => r.result.createObjectStore("kept"); r.onsuccess = () => ok(r.result); r.onerror = r.onblocked = () => ok(null); }
    catch { ok(null); }
  });
  const read = (key) => !keptDb ? "" : new Promise((ok) => {
    try { const q = keptDb.transaction("kept").objectStore("kept").get(key); q.onsuccess = () => ok(typeof q.result === "string" ? q.result : ""); q.onerror = () => ok(""); }
    catch { ok(""); }
  });
  const local = (key) => { try { return localStorage.getItem(key) || ""; } catch { return ""; } };
  let text = (await read(slot)) || local(keptKey);
  if (!text && slotWas) text = (await read(slotWas)) || local(`coaxial.${slotWas}.kept`);
  return text;
})();
function keep(text) {
  if (keptDb) {
    try {
      const t = keptDb.transaction("kept", "readwrite");
      t.objectStore("kept").put(text, slot);
      t.oncomplete = () => { try { localStorage.removeItem(keptKey); } catch {} };
      return;
    } catch {}
  }
  try { localStorage.setItem(keptKey, text); } catch {}
}
// The browser's playtest player: {key, id, account} from the landing's join or a guest's key.
function player() {
  try { const p = JSON.parse(localStorage.getItem("verse.player") || "null"); return p && typeof p.key === "string" ? p : null; }
  catch { return null; }
}

function send(kind, text = "") {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = new TextEncoder().encode(text).subarray(0, x.coaxial_inbox_cap());
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_message(arena.state, kind, bytes.length);
  after(false);
}
globalThis.coaxial = { send, ready: false };
// A page that takes dropped files (data-drops: /projects, NEW PROJECT): drop.js uploads them, the app draws it.
if (script?.dataset.drops) startDrops({ send });
// A page that talks (data-voice: the route its voice host answers as): voice.js, on the app's VOICE.
const voice = script?.dataset.voice
  ? startVoice({ send, route: script.dataset.voice, wasmUrl: script.dataset.voiceWasm || "/build/voice.wasm" }) : null;

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
    if (code === 1) { if (text.startsWith("https://")) location.assign(text); else if (text.startsWith("/")) location.assign(root && !text.startsWith(root + "/") ? root + (text === "/" ? "/index.html" : text) : text); }
    // A copy or cut: the app chose the text (never a masked field's); this runs
    // inside the key event, so the browser lets it reach the clipboard.
    else if (code === 2) navigator.clipboard?.writeText(text).catch(() => {});
    // The app's own place in the page (the hash), replaced so Back still leaves the page.
    else if (code === 3) { if (location.hash.slice(1) !== text) history.replaceState(history.state, "", text ? `#${encodeURIComponent(text)}` : location.pathname + location.search); }
    else if (code === 4) keep(text);
    else if (code === 6) setQuality(text);
    else if (code === ACT_VOICE && voice) voice.toggle(text === "on");
    else if (code === 5) { try { const p = JSON.parse(text); if (p && typeof p.key === "string") localStorage.setItem("verse.player", text); } catch {} }
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
    for (const a of assets) if (!a.pack && !["sound", "loop", "particles"].includes(a.kind)) loadAsset(a);
    for (const pack of new Set(assets.filter((a) => a.pack).map((a) => a.pack))) loadPack(pack);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    say(`${app}: ${e.message}`, true);
  }
  if (dev) live();
}
main();
