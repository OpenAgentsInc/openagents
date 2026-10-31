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
//   coaxial_pixel_budget()    optional: the most pixels a frame draws (a page that traces every pixel);
//                             the canvas draws fewer and is scaled up, never past verse.quality's cap
// and, optionally, input and output, all drawn and decided by the app:
//   coaxial_key(state, key, mods) -> used     a code point, or a named key < 0 (KEYS)
//   coaxial_key_up(state, key, mods) -> used  the same key let go (a game holds keys)
//   coaxial_blur(state)       the page lost the keyboard: nothing is held
//   coaxial_pointer(state, x, y, phase, kind, id) device pixels; phase 0 down 1 move 2 up 3 cancel;
//                             kind 0 a mouse, 1 a finger or pen; id tells fingers apart
//   coaxial_wheel(state, dy)  the wheel, CSS pixels down (the page doesn't scroll)
//   coaxial_tilt(state, dyaw, dpitch) the phone turned, radians (the OpenAgents app's gyro: coaxial.tilt)
//   coaxial_inbox(state), coaxial_inbox_cap(), coaxial_message(state, kind, length)
//                             text in: kind 1 is typed or pasted text; kind 2 the page's data-text,
//                             once each time an engine loads (which project a page shows, say);
//                             others the page's own
//   coaxial_wants_text(state) focus the keyboard field (a phone's keyboard)
//   coaxial_field(state)      where the app draws that field: 4 floats, device pixels
//   coaxial_cursor(state)     0 the default, 1 a link, 2 a crosshair, 3 text
//   coaxial_hash(state, length) the location's hash (without '#') in the inbox, at load and on every change
//   coaxial_action(state), coaxial_outbox(state), coaxial_outbox_len(state), coaxial_action_done(state)
//                             out: 1 navigates to the outbox's URL (under data-root, if the page has one; or an https:// URL), 2 copies its text, 3 makes it the
//                             location's hash (empty: none), in place, 4 keeps it in this browser (one
//                             text per save slot, in IndexedDB, else localStorage; given back on the next visit as
//                             message 4; the slot is the page's data-slot, a realm's own, else the app's name), 5 keeps it as the browser's playtest player ({key, id, account},
//                             localStorage verse.player, which the landing's join writes too), 11 keeps it as the
//                             player's place in this realm (localStorage coaxial.<slot>.place, synchronous, so a page
//                             being left keeps it; given back as message 11 after the page facts; never in golden
//                             mode: apps/openagents-web/place.coax), 12 keeps it as the player's spell bar
//                             (localStorage coaxial.<slot>.bar, given back as message 12 after the place;
//                             never in golden mode: apps/openagents-web/everglade-arrange.coax); others go to the page's
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
//                             (X-Verse-Player); 16 the body's first line is its Authorization header (NIP-98);
//                             -1 when 8 are waiting or the URL isn't this site's
// and optional opaque byte storage: env.coaxial_storage_put/get/delete/cancel, replies at 768 + slot.
// The bounded ABI, raw inbox header and cancellation semantics are in STORAGE.md. No key, event,
// account or cache policy lives in this host; nothing is persisted without an explicit app request.
// and the page itself, through the inbox (what the browser knows; the app decides what to keep):
//   600 the page facts, key=value lines, once an engine loads: tab (a random id for this tab,
//       sessionStorage), fresh (1 when the tab id was just made), visitor (a random id for this
//       browser, localStorage), now (Date.now()), nav (ms since the navigation), url, ref, ua, lang,
//       tz, sw, sh, vw, vh, dpr, coarse, touch, gpu, hidden, webdriver, fcp, dcl, player and account (the
//       playtest player's public id and account, "" none; never its key), voice (the page's data-voice route, "" none), ruleset (data-ruleset: realms.json "ruleset", "" none),
//       style (the realm's art style, data-style: lowpoly, painterly, photoreal or flat; "" not a realm), zone.<id>
//       (a zone's style, data-zones "id:style ...") and clock (data-clock: cycle, real or fixed:T; the URL's
//       ?clock=T, T from 0 to 1, pins it to fixed:T for shots; ?clock=cycle or real sets it), day (the visitor's
//       local time of day, 0 midnight to 1)
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
//   font:URL, icons:URL, licence:URL  GPUI's atlas (gpui.js, a WGSL with gpui_atlas): Geist's four
//                 faces in weight order, Zeron's icons ({name: svg}), and the fonts' licence, served beside them
// and each one that arrives (or fails, bytes 0) is told: coaxial_asset_ready(state, i, bytes), i its
// place in the list. The app's scene, drawn between bg and fx with a depth buffer once every GPU asset is in:
//   coaxial_draws(state), coaxial_draw_count(state)   u32 [first, count, base instance, instances] each,
//                             indexed draws over the index buffer (a base instance with its top bit
//                             set: that many vertices from first, in order, no index buffer)
//   coaxial_stores(), coaxial_store_cap(k), coaxial_store(state, k), coaxial_store_bytes(state, k)
//                             memory copied each frame to storage buffers; an app with
//   coaxial_store_version(state, k)  has store k copied only when that number changes (a
//                             generation: bump it when the store is rewritten)
//   coaxial_sounds(state), coaxial_sounds_len(state)  sounds to play now, a line each: NAME GAIN PAN
//   coaxial_water_draws(state)  how many of the last draws are water (water_vs, waterPipelines)
//                               (a WGSL with water_scene gets the scene's colour and depth copied
//                               before them, at @group(3): waterCopyLayout)
// The WGSL's scene_vs/scene_fs read group 1: the storage assets, then the stores, then the texture
// and its sampler, in that order. A WGSL with shadow_vs/shadow_fs also gets a shadow map (shadowLayout).
// A traced scene (a realm whose bg_fs draws the world, the style fixture) puts its own casters in the
// same map: a WGSL with caster_vs gets the shadow map, caster_vs draws into it (group 0 only, no
// fragment), and bg_fs reads it as group 2 (group 1 empty):
//   coaxial_caster_vertices(state), coaxial_caster_instances(state)  what caster_vs draws (0: none)
// A traced bg_fs that writes @builtin(frag_depth) puts its depth in the scene's buffer. Grass, when the
// WGSL has grass_vs/grass_fs (bg's groups), draws right after bg_fs, depth-tested and written; in a
// mesh scene (no caster_vs) it takes the scene's groups instead, so its blades stand on the scene's
// ground and read its shadow map:
//   coaxial_grass_instances(state)  blades to draw, one triangle (3 vertices) each (0: none)
// A traced page's surface (the water lab's clipmap sea), when the page has no scene, its WGSL has
// surface_vs and surface_fs and the app exports coaxial_surface_vertices: the page gets a depth buffer,
// bg_fs writes its depth (@builtin(frag_depth)), and that many vertices (no buffer, bg's group) draw right
// after it, depth-tested and written:
//   coaxial_surface_vertices(state)  vertices to draw, three a triangle (0: none)
// Particles, optionally: particles:URL names a shader module of their own (particle_vs/particle_fs,
// group 0 the uniforms, group 1 binding 0 the emitters), drawn after the scene, added, depth-tested;
// a module with particle_over_vs/particle_over_fs draws them first too, premultiplied over (smoke), and
// one that reads particle_depth (group 3, binding 1) on a page that copies its scene for the water gets
// that copy's depth, for soft particles:
//   coaxial_particle_cap(), coaxial_particles(state), coaxial_particle_bytes(state)
//                             the emitters' memory, copied each frame to that storage buffer
//   coaxial_particle_instances(state)  quads to draw (6 vertices each)
// Pictures and videos from this site, optionally, drawn where the app says, over the scene and under
// its strokes (Hecate's Cemetery's memorials):
//   coaxial_media(state), coaxial_media_len(state)  a line each: "image URL X0 Y0 X1 Y1" or "video URL X0 Y0 X1 Y1",
//                             or "over URL X0 Y0 X1 Y1", a picture drawn over the strokes too (a HUD's card),
//                             URL this site's ("/..."), the box in the canvas's device pixels; the picture fits
//                             inside it, centred. A video plays muted and loops; one no longer listed is let go.
//                             At most 8. Message 607 says how each one came in, a line each: "URL W H" (0 0: it failed).

import { prepareRandomImport, unsupportedImport } from "./entropy-import.js";
import { startDrops } from "./drop.js";
import { startUploads } from "./uploads.js";
import { startVoice, ACT_VOICE, ACT_VOICE_TO, ACT_VOICE_SAY } from "./voice.js";
import { startGpui } from "./gpui.js";
import { startLoading, readBytes } from "./loader.js";

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

// Grown at load to whatever the app's module declares (importedMemoryPages).
const memory = new WebAssembly.Memory({ initial: 128, maximum: 1024 }); // 8 MiB, at most 64 MiB
let engine = null;   // { exports, layout }
let arena = null;    // { state, vertices, uniforms, floats, uniformFloats, maxVertices }
let gpu = null;      // { device, context, format, bindGroup, uniformBuffer, vertexBuffer, bg, fx }
let wanted = 0;      // the newest requested engine; an older finished build never replaces it
const PACK_PARTS = ["data", "storage", "index", "layers"], PACK_KIND = 605;
const PACK_CATALOG = 1, PACK_VERTICES = 2, PACK_INDICES = 3, PACK_LAYER = 4, PACK_SOUND = 5;
const assets = parseAssets((script && script.dataset.assets) || "");
// What main() loads once the GPU is up (sounds wait to be played; fonts and icons are gpui.js's).
const LATER = ["sound", "loop", "particles", "font", "icons", "licence"];
const firstAssets = assets.filter((a) => !a.pack && !LATER.includes(a.kind)), firstPacks = new Set(assets.filter((a) => a.pack).map((a) => a.pack));
// A realm's loading screen (loader.js): told what really loads, gone on the world's first frame.
const loading = startLoading({ realm: realmId, assets: firstAssets.length + firstPacks.size,
  textured: firstPacks.size > 0 || firstAssets.some((a) => a.kind === "layers"),
  scene: firstPacks.size > 0 || firstAssets.some((a) => a.kind === "index"), golden });
let last = 0;
// This is a completed indexed scene frame in the current engine generation,
// after all outstanding shader loads have settled. Pixel/error checks still
// decide whether the frame actually looks correct.
let sceneEpoch = 0, pendingSceneEpoch = -1, installedGeneration = 0, shaderLoads = 0;
function invalidateScene() {
  sceneEpoch += 1; pendingSceneEpoch = -1;
  globalThis.coaxial.sceneReady = false;
}
function beginEngine() {
  const generation = ++wanted;
  invalidateScene();
  return generation;
}
const randomScratch = {}; // lazily reserved once, shared by synchronous engine calls

// ---- engine: compile, check its layout, keep or start over its state
// The initial page count an app's module declares for its imported memory (0 if none), read from
// the import section: an app with large static buffers declares more than the shared memory starts with.
function importedMemoryPages(bytes) {
  const b = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let i = 8;
  const leb = () => { let v = 0, shift = 0, c; do { c = b[i++]; v += (c & 0x7f) * 2 ** shift; shift += 7; } while (c & 0x80); return v; };
  const skipName = () => { const n = leb(); i += n; };
  while (i < b.length) {
    const id = b[i++], size = leb(), end = i + size;
    if (id === 2) {
      for (let n = leb(); n > 0; n--) {
        skipName(); skipName();
        const kind = b[i++];
        if (kind === 0) leb();                                   // function: type index
        else if (kind === 1) { i++; const f = leb(); leb(); if (f & 1) leb(); } // table
        else if (kind === 2) { const f = leb(); return leb(); }   // memory: flags, then the initial pages
        else if (kind === 3) { i += 2; }                         // global: type, mutability
        else return 0;
      }
      return 0;
    }
    i = end;
  }
  return 0;
}

async function loadEngine(bytes, generation) {
  const module = await WebAssembly.compile(bytes);
  const pages = importedMemoryPages(bytes), have = memory.buffer.byteLength / 65536;
  if (pages > have) memory.grow(pages - have); // instantiate refuses an imported memory smaller than declared
  const random = prepareRandomImport(memory, module, randomScratch);
  const imports = Object.create(null);
  imports.env = Object.assign(Object.create(null), { memory, coaxial_now_seconds: () => BigInt(Math.floor(Date.now() / 1000)), ...socketImports, ...httpImports, ...random.imports, ...storageImports });
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
  installedGeneration = generation;
  invalidateScene();
  if (script?.dataset.text) send(2, script.dataset.text);
  // data-lines="KIND:URL": a text file the app reads as message KIND, fetched fresh each load
  // (/projects: 43:/projects/published.txt, the projects published from a drop since the build).
  if (script?.dataset.lines && location.protocol !== "file:") {
    const [kind, url] = script.dataset.lines.split(/:(.*)/);
    fetch(url, { cache: "no-store" }).then((r) => (r.ok ? r.text() : ""), () => "").then((t) => { if (t) send(Number(kind), t); });
  }
  for (const a of assets) if (a.done) deliver(a); // a new build gets what the last one had
  sendHash();
  // The page's scripts may talk to the app from now (action 0: ready).
  if (kept) send(4, kept);
  globalThis.coaxial.ready = true;
  if (x.coaxial_message) send(PAGE_KIND, pageFacts());
  // Where the player was in this realm (action 11), before the first frame: a reload never waits on the network.
  const place = golden ? "" : local(placeKey);
  if (place) send(PLACE_KIND, place);
  const bar = golden ? "" : local(barKey);
  if (bar) send(BAR_KIND, bar);
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
// Socket events wait in arrival order and reach the app inside the frame, a few milliseconds a frame
// (a relay catching up sends thousands of signed events at once, and the app checks each one's
// signature: handled as they arrive they took half the main thread for the first ~17 s and Chrome
// held frames back up to 100 ms). A hidden tab, which draws no frames, is drained on a timer.
const SOCKET_BUDGET_MS = 12, socketQueue = [];
let socketHead = 0, socketTimer = 0, socketDrained = 0;
function socketQueued(ws, id, event, text = "") {
  socketQueue.push([ws, id, event, text]);
  if (!socketTimer) socketTimer = setTimeout(socketIdle, 100);
}
function socketIdle() {
  socketTimer = 0;
  if (performance.now() - socketDrained > 90) drainSockets();
  if (socketHead < socketQueue.length) socketTimer = setTimeout(socketIdle, 100);
}
function drainSockets(budget = SOCKET_BUDGET_MS) {
  const t0 = socketDrained = performance.now();
  while (socketHead < socketQueue.length) {
    const [ws, id, event, text] = socketQueue[socketHead];
    socketQueue[socketHead++] = null;
    if (sockets[id] !== ws) continue;   // the app closed it: what it sent after says nothing
    if (event === SOCKET_CLOSED) sockets[id] = null;
    try { socketEvent(id, event, text); } catch (e) { say(`${app} socket: ${e.message}`, true); }
    if (performance.now() - t0 > budget) break;
  }
  if (socketHead === socketQueue.length) { socketQueue.length = 0; socketHead = 0; }
  else if (socketHead > 4096) { socketQueue.splice(0, socketHead); socketHead = 0; }
}
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
    ws.onopen = () => mine() && socketQueued(ws, id, SOCKET_OPENED);
    ws.onmessage = (m) => mine() && typeof m.data === "string" && socketQueued(ws, id, SOCKET_FRAME, m.data);
    ws.onerror = () => mine() && socketQueued(ws, id, SOCKET_ERROR);
    ws.onclose = () => mine() && socketQueued(ws, id, SOCKET_CLOSED);   // the slot frees when the app hears it
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
    let body = bodyLen ? new Uint8Array(memory.buffer, bodyPtr, bodyLen).slice() : null;
    const post = (flags & 1) !== 0, wanted = (flags & 4) !== 0;
    // 16: the body's first line is the request's Authorization (a NIP-98 login), the rest its body
    let auth = "";
    if ((flags & 16) && body) { const nl = body.indexOf(10); if (nl < 0) return -1; auth = new TextDecoder().decode(body.subarray(0, nl)); body = body.slice(nl + 1); }
    if ((flags & 2) && navigator.sendBeacon && navigator.sendBeacon(url, new Blob([body || ""], { type: "application/json" }))) return 0;
    let id = 0;
    if (wanted) { id = waiting.indexOf(false); if (id < 0) return -1; waiting[id] = true; }
    const headers = post ? { "Content-Type": "application/json" } : {};
    const who = flags & 8 ? player() : null;
    if (who) headers["X-Verse-Player"] = who.key;
    if (auth) headers.Authorization = auth;
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
// Who is logged in, the same on every page: /api/auth/viewer's lines (/api/auth/me's: K| npub, H| hex key,
// M| a member key it speaks for, J| a private project), or "" when nobody is (a 200, so no console error).
// Apps that care read VIEWER_KIND.
function viewer() {
  if (!engine?.exports.coaxial_message || location.protocol === "file:") return;
  fetch("/api/auth/viewer", { credentials: "same-origin" })
    // read the body either way: an unread one never finishes loading
    .then((r) => r.text().then((t) => (r.ok ? t : "")), () => "")
    .then((t) => send(VIEWER_KIND, t), () => send(VIEWER_KIND, ""));
}
const randomId = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
function kept(store, key) {
  try { const v = store.getItem(key); if (v) return [v, false]; const n = randomId(); store.setItem(key, n); return [n, true]; }
  catch { return [randomId(), true]; }   // storage refused (a private window): this page only
}
// The realm's style, zones and clock (docs/verse/art-styles.md, The style field).
function styleFacts() {
  const d = script?.dataset || {}, out = { style: d.style || "" };
  for (const z of (d.zones || "").split(/\s+/)) { const [id, st] = z.split(":"); if (id && st) out[`zone.${id}`] = st; }
  const pin = new URLSearchParams(location.search).get("clock");
  out.clock = pin !== null && /^(0(\.\d{1,4})?|1(\.0{1,4})?)$/.test(pin) ? `fixed:${pin}` : pin === "cycle" || pin === "real" ? pin : d.clock || "";
  // the visitor's local time of day, 0 midnight to 1, for a clock that is real
  const t = new Date();
  out.day = ((t.getHours() * 3600 + t.getMinutes() * 60 + t.getSeconds()) / 86400).toFixed(5);
  return out;
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
    player: player()?.id || "", account: player()?.account || "", realm: realmId, slot, voice: script?.dataset.voice || "", ruleset: script?.dataset.ruleset || "", ...styleFacts() })
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
  loading.watch(device);
  loading.step("shaders");
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
  if (gpu.gpuiOn) useGpui();
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

// GPUI's atlas (coax/web/gpui.js): a page whose shader declares gpui_atlas
// (binding 1, r8 glyphs and icons), gpui_color (2, color glyphs) and
// gpui_sampler (3) draws the shared Zeron components (apps/shared/zeron/,
// /chat); its vertices blend source-over, as GPUI's do. The atlas is built
// for the device pixel ratio once the app's engine and the GPU are up, and
// again when the ratio changes; the glyph cache is served after each frame.
function useGpui() {
  const { device } = gpu;
  gpu.gpuiOn = true;
  gpu.gpui ||= startGpui({ device, memory, assets, say });
  const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, f = GPUShaderStage.FRAGMENT;
  gpu.layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: both, buffer: {} },
    { binding: 1, visibility: f, texture: {} }, { binding: 2, visibility: f, texture: {} }, { binding: 3, visibility: f, sampler: {} }] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [
    { binding: 0, resource: { buffer: gpu.uniformBuffer } },
    { binding: 1, resource: gpu.gpui.g.mono.createView() }, { binding: 2, resource: gpu.gpui.g.color.createView() },
    { binding: 3, resource: device.createSampler({ magFilter: "linear", minFilter: "linear" }) }] });
}
function gpuiFrame(dpr) {
  const x = engine?.exports, k = gpu?.gpui;
  if (!x?.coaxial_gpui_set_atlas || !k) return;
  if (k.engine !== engine || k.dpr !== dpr) {
    if (k.building) return;
    k.building = true; k.engine = engine; k.dpr = dpr;
    const at = engine;
    k.build(x, arena.state, dpr)
      .then(() => { if (engine === at) useGpui(); else k.engine = null; })
      .catch((e) => { say(`${app}: atlas: ${e.message}`, true); })
      .finally(() => { k.building = false; });
    return;
  }
  if (!k.building) k.serve(x, arena.state);
}

async function loadShaders() {
  shaderLoads += 1;
  invalidateScene();
  try {
  const { device, format } = gpu;
  const code = await (await fetch(wgslUrl, { cache: dev ? "no-store" : "default" })).text();
  if (/var\s+sans_atlas\b/.test(code)) useSans();
  if (/var\s+gpui_atlas\b/.test(code)) useGpui();
  const module = device.createShaderModule({ code });
  const info = await module.getCompilationInfo();
  const errors = info.messages.filter((m) => m.type === "error");
  if (errors.length) { say(errors.map((m) => `${app}.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return false; }
  const layout = device.createPipelineLayout({ bindGroupLayouts: [gpu.layout] });
  const stride = arena.floats * 4;
  // With a scene, every pipeline in the pass shares its depth buffer; only the scene tests and writes it.
  const scene = !!engine?.exports.coaxial_draws;
  gpu.scene3d = scene;
  const surfaced = !scene && /fn\s+surface_vs\b/.test(code) && /fn\s+surface_fs\b/.test(code) && !!engine?.exports.coaxial_surface_vertices;
  const depthed = scene || surfaced;
  const flat = depthed ? { depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "always" } } : {};
  const shadows = scene && /fn\s+shadow_vs\b/.test(code);
  const casters = scene && /fn\s+caster_vs\b/.test(code);
  const post = scene && /fn\s+post_fs\b/.test(code);
  const lit = post ? [{ format: "rgba16float" }, { format: "rgba16float" }] : [{ format }];
  const sceneGroups = scene ? [gpu.layout, sceneLayout(), ...(shadows ? [shadowLayout()] : [])] : [];
  const bgLayout = casters ? device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, emptyLayout(), shadowLayout()] }) : layout;
  // A traced scene whose bg_fs writes @builtin(frag_depth) puts its depth in the buffer, so what
  // draws after it (grass) hides behind what it traced.
  const bgDepth = depthed && /@builtin\(frag_depth\)/.test(code) ? { depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "always" } } : flat;
  // Grass (docs/verse/art-styles.md 8), when the WGSL has grass_vs and grass_fs: coaxial_grass_instances(state)
  // blades of one triangle each, instanced from their cells' hashes (no buffer), drawn after bg_fs with bg's groups.
  const grassy = scene && /fn\s+grass_vs\b/.test(code) && !!engine?.exports.coaxial_grass_instances;
  const [bg, fx, sc, sh, bl, po, ca, gr, su] = await Promise.all([
    device.createRenderPipelineAsync({ layout: bgLayout, ...bgDepth, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: lit } }),
    device.createRenderPipelineAsync({
      layout, ...(post ? {} : flat),
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double (GPUI's straight alpha, source over)
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: gpu.gpuiOn ? {
        color: { operation: "add", srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" } } : {
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
    casters ? device.createRenderPipelineAsync({
      layout, vertex: { module, entryPoint: "caster_vs" },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less", depthBias: 2, depthBiasSlopeScale: 2 },
    }) : null,
    grassy ? device.createRenderPipelineAsync({
      layout: casters ? bgLayout : device.createPipelineLayout({ bindGroupLayouts: sceneGroups }), vertex: { module, entryPoint: "grass_vs" }, fragment: { module, entryPoint: "grass_fs", targets: lit },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" },
    }) : null,
    surfaced ? device.createRenderPipelineAsync({
      layout, vertex: { module, entryPoint: "surface_vs" }, fragment: { module, entryPoint: "surface_fs", targets: lit },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" },
    }) : null,
  ]);
  gpu.surface = su;
  gpu.bg = bg; gpu.fx = fx; gpu.scene = sc; gpu.shadow = sh; gpu.bloom = bl; gpu.post = po; gpu.caster = ca; gpu.grass = gr; // swapped whole, at the next frame
  gpu.waterCopy = post && /var\s+water_scene\b/.test(code);
  gpu.particles = sc ? await particlePipeline(post) : null;
  const waterGroups = gpu.waterCopy ? [...sceneGroups, ...(sceneGroups.length < 3 ? [emptyLayout()] : []), waterCopyLayout()] : sceneGroups;
  [gpu.waterTransmit, gpu.waterEmit] = sc && /fn\s+water_vs\b/.test(code) ? await waterPipelines(module, waterGroups, post) : [null, null];
  return true;
  } finally {
    shaderLoads -= 1;
    invalidateScene();
  }
}

// Water, when the WGSL has water_vs, water_transmit_fs and water_emit_fs: the app's last
// coaxial_water_draws(state) draws are water, drawn after the scene in two halves each, depth-tested
// and not written: the first multiplies what lies behind by its colour (per-channel transmittance),
// the second adds its own (docs/verse/water.md).
async function waterPipelines(module, groups, post) {
  const { device, format } = gpu;
  const keep = { operation: "add", srcFactor: "zero", dstFactor: "one" };
  const half = (entryPoint, color) => device.createRenderPipelineAsync({
    layout: device.createPipelineLayout({ bindGroupLayouts: groups }),
    vertex: { module, entryPoint: "water_vs" },
    fragment: { module, entryPoint, targets: post
      ? [{ format: "rgba16float", blend: { color, alpha: keep } }, { format: "rgba16float", writeMask: 0 }]
      : [{ format, blend: { color, alpha: keep } }] },
    primitive: { topology: "triangle-list", cullMode: "none" },
    depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" },
  });
  return Promise.all([half("water_transmit_fs", { operation: "add", srcFactor: "zero", dstFactor: "src" }),
    half("water_emit_fs", { operation: "add", srcFactor: "one", dstFactor: "one" })]);
}

// W5 (docs/verse/water.md): a page whose WGSL declares water_scene (and draws through the post pass)
// has the scene's light and depth copied after the scene and before the water, read at @group(3):
// binding 0 the light (rgba16float, linear), binding 1 the depth. Refraction, contact foam and soft
// edges read them; the water's own triangles are not in them.
function waterCopyLayout() {
  gpu.waterCopyLayout ||= gpu.device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "unfilterable-float" } },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "depth" } }] });
  return gpu.waterCopyLayout;
}

// The particles' own module and pipeline; a page without one, or one that doesn't compile, draws none.
async function particlePipeline(post) {
  const a = assets.find((s) => s.kind === "particles"), x = engine?.exports;
  gpu.particlesOver = null; gpu.particleDepth = false;
  if (!a || !x?.coaxial_particles) return null;
  const { device, format } = gpu;
  try {
    const code = await (await fetch(a.url, { cache: dev ? "no-store" : "default" })).text();
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
    if (errors.length) { say(errors.map((m) => `particles.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return null; }
    const group = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } }] });
    gpu.particleBuffer ||= device.createBuffer({ size: align(Math.max(x.coaxial_particle_cap(), 16), 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    gpu.particleGroup = device.createBindGroup({ layout: group, entries: [{ binding: 0, resource: { buffer: gpu.particleBuffer } }] });
    const add = { operation: "add", srcFactor: "one", dstFactor: "one" };
    const over = { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" };
    const under = { operation: "add", srcFactor: "zero", dstFactor: "one-minus-src-alpha" };
    // soft particles, when the module reads particle_depth and the page copies its scene for the water
    // (W5): the copy's light and depth at group 3, so a puff fades where it meets what stands behind it
    gpu.particleDepth = gpu.waterCopy && /\bparticle_depth\b/.test(code);
    const layout = device.createPipelineLayout({ bindGroupLayouts: gpu.particleDepth ? [gpu.layout, group, emptyLayout(), waterCopyLayout()] : [gpu.layout, group] });
    const pipe = (vs, fs, color, alpha) => device.createRenderPipelineAsync({
      layout, vertex: { module, entryPoint: vs }, fragment: { module, entryPoint: fs, targets: post
        ? [{ format: "rgba16float", blend: { color, alpha } }, { format: "rgba16float", writeMask: 0 }]
        : [{ format, blend: { color, alpha } }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" },
    });
    // smoke, when the module has particle_over_vs: premultiplied over what is behind, its alpha
    // taking that much of the light's bloom share, drawn before the added particles
    const [lit, shade] = await Promise.all([pipe("particle_vs", "particle_fs", add, add),
      /fn\s+particle_over_vs\b/.test(code) ? pipe("particle_over_vs", "particle_over_fs", over, under) : null]);
    gpu.particlesOver = shade;
    return lit;
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
      loading.bytes(pack.url, loaded, total);
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
    loading.textures(0, 2 * of(PACK_LAYER).length);   // each layer decoded, then on the GPU
    const images = await Promise.all(of(PACK_LAYER).map((e) => createImageBitmap(new Blob([e.bytes])).then((b) => (loading.textures(1, 0), b))));
    if (images.length) { layers.texture = await layerTexture(images); layers.size = images.length; }
    else layers.failed = true;
    for (const e of of(PACK_SOUND)) assets.push({ i: assets.length, kind: "sound", name: e.name, bytes: e.bytes, done: true });
  } catch (e) { for (const a of parts) a.failed = true; say(`${app}: ${e.message}`, true); }
  for (const a of parts) { a.done = true; deliver(a); }
  bindScene();
  loading.assetDone(parts[0].failed && `WORLD FAILED TO LOAD: ${pack.url.split("/").pop()}`);
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
      loading.textures(0, 2 * a.urls.length);
      const images = await Promise.all(a.urls.map(async (u) => {
        const r = await fetch(u);
        if (!r.ok) throw new Error(`${u}: ${r.status}`);
        const b = await createImageBitmap(new Blob([await readBytes(r, (l, t) => loading.bytes(u, l, t))]));
        loading.textures(1, 0);
        return b;
      }));
      a.texture = await layerTexture(images); a.size = images.length;
    } else {
      const bytes = await fetchedOnce(a.url);
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
  loading.assetDone(a.failed && `ASSET FAILED TO LOAD: ${(a.url || "").split("/").pop()}`);
}
// An asset's bytes, fetched once however many of the page's assets name the same URL (the Grid's
// Cast.bin is both data, for its clips, and storage, for its vertices).
const fetches = new Map();
function fetchedOnce(url) {
  if (!fetches.has(url)) fetches.set(url, (async () => {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`${url}: ${r.status}`);
    return readBytes(r, (l, t) => loading.bytes(url, l, t));
  })());
  return fetches.get(url);
}
// A 2D-array texture, an image a layer, sRGB, mipmapped (every image at the first one's size).
async function layerTexture(images) {   // the loading screen counts each layer as it goes up
  const { device } = gpu, [w, h] = [images[0].width, images[0].height];
  const levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
  const texture = device.createTexture({ size: [w, h, images.length], format: "rgba8unorm-srgb", mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  for (const [layer, image] of images.entries()) {
    for (let level = 0; level < levels; level++) {
      const lw = Math.max(1, w >> level), lh = Math.max(1, h >> level);
      const source = level === 0 && image.width === w && image.height === h ? image
        : await createImageBitmap(image, { resizeWidth: lw, resizeHeight: lh, resizeQuality: "high" });
      device.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer] }, [lw, lh]);
    }
    loading.textures(1, 0);
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
// An empty group, for a pipeline that skips group 1 (bg_fs reading the shadow map as group 2).
function emptyLayout() {
  if (!gpu.emptyLayout) {
    gpu.emptyLayout = gpu.device.createBindGroupLayout({ entries: [] });
    gpu.emptyGroup = gpu.device.createBindGroup({ layout: gpu.emptyLayout, entries: [] });
  }
  return gpu.emptyLayout;
}
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
  // the water's copies of the scene (waterCopyLayout): the light and depth are their sources
  const copied = gpu.waterCopy ? GPUTextureUsage.COPY_SRC : 0;
  const make = (tw, th, format) => device.createTexture({ size: [Math.max(1, tw), Math.max(1, th)], format, usage: use });
  const light = device.createTexture({ size: [w, h], format: "rgba16float", usage: use | copied }), aux = make(w, h, "rgba16float");
  const q0 = make(Math.ceil(w / 4), Math.ceil(h / 4), "rgba16float"), q1 = make(q0.width, q0.height, "rgba16float");
  const e0 = make(Math.ceil(w / 8), Math.ceil(h / 8), "rgba16float"), e1 = make(e0.width, e0.height, "rgba16float");
  gpu.depth?.destroy();
  gpu.depth = device.createTexture({ size: [w, h], format: "depth32float", usage: use | copied });
  gpu.postTextures = [light, aux, q0, q1, e0, e1];
  gpu.waterCopyGroup = null;
  if (gpu.waterCopy) {
    const into = (format) => device.createTexture({ size: [w, h], format, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
    gpu.waterLight = into("rgba16float"); gpu.waterDepth = into("depth32float");
    gpu.postTextures.push(gpu.waterLight, gpu.waterDepth);
    gpu.waterCopyGroup = device.createBindGroup({ layout: waterCopyLayout(), entries: [
      { binding: 0, resource: gpu.waterLight.createView() }, { binding: 1, resource: gpu.waterDepth.createView() }] });
  }
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
  gpu.stores = []; gpu.storeVersions = [];
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

// ---- media: pictures and videos the app names (coaxial_media), each a texture drawn into its box
const MEDIA_KIND = 607, MEDIA_MAX = 112;   // Everglade's spell bar names up to 100 (four rows of slots, each moved one's key tab, a phone row's switcher, a card and the slot in hand)
const media = new Map();   // "kind url" -> { kind, url, el, texture, w, h, ready, failed, unseen, buffer, group }
const MEDIA_WGSL = `
struct M { rect: vec4f, look: vec4f };
@group(0) @binding(0) var<uniform> m: M;
@group(0) @binding(1) var t: texture_2d<f32>;
@group(0) @binding(2) var s: sampler;
struct O { @builtin(position) p: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> O {
  var c = array(vec2f(0, 0), vec2f(1, 0), vec2f(0, 1), vec2f(0, 1), vec2f(1, 0), vec2f(1, 1));
  let k = c[i];
  var o: O;
  o.p = vec4f(mix(m.rect.x, m.rect.z, k.x), mix(m.rect.y, m.rect.w, k.y), 0, 1);
  o.uv = k;
  return o;
}
@fragment fn fs(o: O) -> @location(0) vec4f { return vec4f(textureSample(t, s, o.uv).rgb * m.look.x, 1); }`;
function mediaPipeline(depth) {
  const key = depth ? "mediaDepth" : "media";
  if (gpu[key] !== undefined) return gpu[key];
  const { device, format } = gpu;
  gpu.mediaLayout ||= device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
    { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: {} }] });
  gpu.mediaSampler ||= device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
  const module = device.createShaderModule({ code: MEDIA_WGSL });
  gpu[key] = device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [gpu.mediaLayout] }),
    vertex: { module, entryPoint: "vs" }, fragment: { module, entryPoint: "fs", targets: [{ format }] },
    ...(depth ? { depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "always" } } : {}),
  });
  return gpu[key];
}
function mediaTold() {
  send(MEDIA_KIND, [...media.values()].filter((m) => m.ready || m.failed).map((m) => `${m.url} ${m.failed ? 0 : m.w} ${m.failed ? 0 : m.h}`).join("\n"));
}
function mediaTexture(m, w, h) {
  m.w = w; m.h = h;
  m.texture = gpu.device.createTexture({ size: [w, h], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  m.buffer = gpu.device.createBuffer({ size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  mediaPipeline(false);
  m.group = gpu.device.createBindGroup({ layout: gpu.mediaLayout, entries: [
    { binding: 0, resource: { buffer: m.buffer } }, { binding: 1, resource: m.texture.createView() }, { binding: 2, resource: gpu.mediaSampler }] });
}
function mediaOpen(kind, url) {
  const m = { kind, url, el: null, texture: null, w: 0, h: 0, ready: false, failed: false, unseen: 0, buffer: null, group: null };
  media.set(`${kind} ${url}`, m);
  const fail = () => { if (media.get(`${kind} ${url}`) !== m) return; m.failed = true; mediaTold(); };
  if (kind === "image") {
    fetch(url, { credentials: "same-origin" }).then((r) => (r.ok ? r.blob() : Promise.reject(new Error(String(r.status)))))
      .then((b) => createImageBitmap(b)).then(async (img) => {
        if (media.get(`${kind} ${url}`) !== m) return;
        const f = Math.min(1, 2048 / Math.max(img.width, img.height));   // a texture at most 2048 a side
        const w = Math.max(1, Math.round(img.width * f)), h = Math.max(1, Math.round(img.height * f));
        const src = f < 1 ? await createImageBitmap(img, { resizeWidth: w, resizeHeight: h, resizeQuality: "high" }) : img;
        mediaTexture(m, w, h);
        gpu.device.queue.copyExternalImageToTexture({ source: src }, { texture: m.texture }, [w, h]);
        m.ready = true; mediaTold();
      }).catch(fail);
  } else {
    const v = document.createElement("video");
    v.muted = true; v.defaultMuted = true; v.loop = true; v.playsInline = true; v.autoplay = true; v.preload = "auto";
    v.setAttribute("muted", ""); v.setAttribute("playsinline", "");   // never a sound: it plays silently, inline
    v.addEventListener("loadeddata", () => {
      if (media.get(`${kind} ${url}`) !== m || m.ready) return;
      mediaTexture(m, Math.max(1, v.videoWidth), Math.max(1, v.videoHeight));
      m.ready = true; mediaTold(); v.play().catch(() => {});
    });
    v.addEventListener("error", fail);
    v.src = url; m.el = v;
    v.play().catch(() => {});
  }
}
function mediaClose(key, m) {
  media.delete(key);
  if (m.el) { m.el.pause(); m.el.removeAttribute("src"); m.el.load(); }
  m.texture?.destroy(); m.buffer?.destroy();
}
// The media this frame wants, opened as they're first named; the ready ones drawn into pass: the
// pictures and videos under the strokes, or (over) the ones drawn over them.
function drawMedia(x, pass, w, h, depth, over = false) {
  const n = x.coaxial_media && x.coaxial_media_len ? x.coaxial_media_len(arena.state) : 0;
  const wanted = new Set();
  if (n > 0) {
    const text = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_media(arena.state), n).slice());
    for (const line of text.split("\n").slice(0, MEDIA_MAX)) {
      const [named, url, ...b] = line.split(" ");
      const box = b.map(Number);
      const kind = named === "over" ? "image" : named;
      if ((kind !== "image" && kind !== "video") || !url?.startsWith("/") || url.startsWith("//") || box.length !== 4 || box.some((v) => !Number.isFinite(v))) continue;
      const key = `${kind} ${url}`;
      if ((named === "over") !== over) { wanted.add(key); continue; }
      wanted.add(key);
      if (!media.has(key)) mediaOpen(kind, url);
      const m = media.get(key);
      if (!m.ready || !m.group) continue;
      if (m.el && m.el.readyState >= 2) { try { gpu.device.queue.copyExternalImageToTexture({ source: m.el }, { texture: m.texture }, [m.w, m.h]); } catch {} }
      // fit inside the box, centred
      const [x0, y0, x1, y1] = box, bw = x1 - x0, bh = y1 - y0;
      if (bw <= 1 || bh <= 1) continue;
      const f = Math.min(bw / m.w, bh / m.h), dw = m.w * f, dh = m.h * f, cx = x0 + (bw - dw) / 2, cy = y0 + (bh - dh) / 2;
      gpu.device.queue.writeBuffer(m.buffer, 0, new Float32Array([cx / w * 2 - 1, 1 - cy / h * 2, (cx + dw) / w * 2 - 1, 1 - (cy + dh) / h * 2, 1, 0, 0, 0]));
      pass.setPipeline(mediaPipeline(depth)); pass.setBindGroup(0, m.group); pass.draw(6);
    }
  }
  if (!over) for (const [key, m] of media) if (!wanted.has(key) && ++m.unseen > 60) mediaClose(key, m); else if (wanted.has(key)) m.unseen = 0;
  if (n > 0 || media.size) pass.setBindGroup(0, gpu.bindGroup);
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
  if (golden || !canvas.clientWidth) return 1;
  const budget = engine?.exports.coaxial_pixel_budget?.() || 0;
  if (budget > 0) {
    const per = deviceWidth / canvas.clientWidth;
    return Math.min(1, Math.sqrt(budget / (canvas.clientWidth * Math.max(1, canvas.clientHeight))) / per, QUALITY_CAP[quality] / per);
  }
  if (!gpu?.scene3d) return 1;
  return Math.min(1, QUALITY_CAP[quality] / (deviceWidth / canvas.clientWidth));
};
// Device pixels per CSS pixel, as the canvas is actually drawn.
const scale = () => (canvas.width && canvas.clientWidth ? canvas.width / canvas.clientWidth : devicePixelRatio || 1);

// ---- the frame
// Clip mode (clip.ts sets coaxialClip before the page loads): the page draws only when
// coaxialClipStep says so, each frame one fixed step of game time, so a recording has
// every frame at its exact moment however slowly the machine draws or captures.
const clipping = globalThis.coaxialClip === true;
let clipNow = 0;
if (clipping) globalThis.coaxialClipStep = async (frames = 1, fps = 30) => {
  for (let k = 0; k < frames; k++) { clipNow += 1000 / fps; draw(clipNow); }
  if (gpu?.device) await gpu.device.queue.onSubmittedWorkDone();
  return engine && gpu?.bg ? 1 : 0;
};
function frame(now) {
  requestAnimationFrame(frame);
  if (clipping) return;
  if (engine && socketHead < socketQueue.length) drainSockets(Math.max(2, SOCKET_BUDGET_MS - frameCost));
  const t0 = performance.now();
  try { draw(now); } finally { frameCost = 0.9 * frameCost + 0.1 * (performance.now() - t0); }
}
let frameCost = 0;   // ms of main thread the drawing itself takes, smoothed: socket events get what's left
function draw(now) {
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
    if (gpu.gpuiOn) gpuiFrame(dpr);
  } catch (e) { say(`${app} trapped: ${e.message}`, true); loading.fail(`ENGINE STOPPED: ${e.message}`); engine = null; return; }
  n = Math.min(n, arena.maxVertices);
  const { device, context } = gpu;
  const x = engine.exports;
  device.queue.writeBuffer(gpu.uniformBuffer, 0, new Float32Array(memory.buffer, arena.uniforms, arena.uniformFloats));
  if (n) device.queue.writeBuffer(gpu.vertexBuffer, 0, new Float32Array(memory.buffer, arena.vertices, n * arena.floats));
  const draws = gpu.scene && gpu.sceneGroup ? x.coaxial_draw_count(arena.state) : 0;
  if (draws) gpu.stores.forEach((buffer, k) => {
    const version = x.coaxial_store_version?.(arena.state, k);
    if (version !== undefined && gpu.storeVersions[k] === version) return;
    const bytes = Math.min(x.coaxial_store_bytes(arena.state, k), buffer.size) & ~3;
    if (bytes) { device.queue.writeBuffer(buffer, 0, memory.buffer, x.coaxial_store(arena.state, k), bytes); gpu.storeVersions[k] = version; }
  });
  if ((gpu.scene || gpu.surface) && (!gpu.depth || gpu.depth.width !== w || gpu.depth.height !== h || !!gpu.post !== !!gpu.postGroup
      || (gpu.post && !!gpu.waterCopy !== !!gpu.waterCopyGroup))) {
    if (gpu.post) postTargets(w, h);
    else {
      gpu.depth?.destroy(); gpu.postGroup = null;
      gpu.depth = device.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT });
    }
  }
  const post = gpu.post && gpu.postGroup;
  const encoder = device.createCommandEncoder();
  const list = draws ? new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), draws * 4) : null;
  const wet = draws && gpu.waterTransmit && x.coaxial_water_draws ? Math.min(x.coaxial_water_draws(arena.state), draws) : 0;
  // a draw whose base instance has its top bit set draws its vertices in order, with no index buffer
  const drawOne = (pass, i) => {
    const [first, count, base, instances] = [list[i * 4], list[i * 4 + 1], list[i * 4 + 2], list[i * 4 + 3]];
    if (!count || !instances) return;
    if (base & 0x80000000) pass.draw(count, instances, first, base & 0x7fffffff);
    else pass.drawIndexed(count, instances, first, 0, base);
  };
  const drawScene = (pass) => {
    pass.setBindGroup(1, gpu.sceneGroup); pass.setIndexBuffer(gpu.index, "uint32");
    for (let i = 0; i < draws - wet; i++) drawOne(pass, i);
  };
  const cast = gpu.caster && x.coaxial_caster_instances ? [x.coaxial_caster_vertices(arena.state), x.coaxial_caster_instances(arena.state)] : [0, 0];
  if ((draws && gpu.shadow) || gpu.caster) {
    // cleared every frame even with nothing to cast, so a traced scene never reads a stale map
    const shade = encoder.beginRenderPass({ colorAttachments: [],
      depthStencilAttachment: { view: gpu.shadowMap.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" } });
    shade.setBindGroup(0, gpu.bindGroup);
    if (draws && gpu.shadow) { shade.setPipeline(gpu.shadow); drawScene(shade); }
    if (cast[0] && cast[1]) { shade.setPipeline(gpu.caster); shade.draw(cast[0], cast[1]); }
    shade.end();
  }
  const target = (view) => ({ view, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } });
  const screen = () => target(context.getCurrentTexture().createView());
  let pass = encoder.beginRenderPass({ colorAttachments: post ? [target(gpu.light.createView()), target(gpu.aux.createView())] : [screen()],
    ...(gpu.scene || gpu.surface ? { depthStencilAttachment: { view: gpu.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: post ? "store" : "discard" } } : {}) });
  pass.setBindGroup(0, gpu.bindGroup);
  if (gpu.caster) { pass.setBindGroup(1, gpu.emptyGroup); pass.setBindGroup(2, gpu.shadowGroup); }
  pass.setPipeline(gpu.bg); pass.draw(3);
  const blades = gpu.grass && (gpu.caster || gpu.sceneGroup) ? x.coaxial_grass_instances(arena.state) : 0;
  if (blades) {
    if (!gpu.caster) { pass.setBindGroup(1, gpu.sceneGroup); if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup); }
    pass.setPipeline(gpu.grass); pass.draw(3, blades);
  }
  const surface = gpu.surface ? x.coaxial_surface_vertices(arena.state) : 0;
  if (surface) { pass.setPipeline(gpu.surface); pass.draw(surface); }
  if (draws) {
    pass.setPipeline(gpu.scene);
    if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
    drawScene(pass);
    const sparks = gpu.particles ? x.coaxial_particle_instances(arena.state) : 0;
    if ((wet || (sparks && gpu.particleDepth)) && post && gpu.waterCopyGroup) {
      // W5: the scene's light and depth as they stand before the water (and the soft particles), then on
      // in a pass that keeps them
      pass.end();
      encoder.copyTextureToTexture({ texture: gpu.light }, { texture: gpu.waterLight }, [w, h]);
      encoder.copyTextureToTexture({ texture: gpu.depth }, { texture: gpu.waterDepth }, [w, h]);
      const keep = (view) => ({ view, loadOp: "load", storeOp: "store" });
      pass = encoder.beginRenderPass({ colorAttachments: [keep(gpu.light.createView()), keep(gpu.aux.createView())],
        depthStencilAttachment: { view: gpu.depth.createView(), depthLoadOp: "load", depthStoreOp: "store" } });
      pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.sceneGroup);
      pass.setBindGroup(2, gpu.shadow ? gpu.shadowGroup : (emptyLayout(), gpu.emptyGroup));
      pass.setBindGroup(3, gpu.waterCopyGroup);
      pass.setIndexBuffer(gpu.index, "uint32");
    }
    for (let i = draws - wet; i < draws; i++) {
      pass.setPipeline(gpu.waterTransmit); drawOne(pass, i);
      pass.setPipeline(gpu.waterEmit); drawOne(pass, i);
    }
    const bytes = sparks && (!gpu.particleDepth || gpu.waterCopyGroup) ? Math.min(x.coaxial_particle_bytes(arena.state), gpu.particleBuffer.size) & ~3 : 0;
    if (bytes) {
      device.queue.writeBuffer(gpu.particleBuffer, 0, memory.buffer, x.coaxial_particles(arena.state), bytes);
      pass.setBindGroup(1, gpu.particleGroup);
      if (gpu.particleDepth) { pass.setBindGroup(2, (emptyLayout(), gpu.emptyGroup)); pass.setBindGroup(3, gpu.waterCopyGroup); }
      if (gpu.particlesOver) { pass.setPipeline(gpu.particlesOver); pass.draw(6, sparks); }
      pass.setPipeline(gpu.particles); pass.draw(6, sparks);
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
  if (x.coaxial_media) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post);
  if (n) { pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(n); }
  if (x.coaxial_media) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post, true);
  pass.end();
  const texture = context.getCurrentTexture();
  const readback = probe && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;
  device.queue.submit([encoder.finish()]);
  if (draws && shaderLoads === 0 && installedGeneration === wanted && !globalThis.coaxial.sceneReady && pendingSceneEpoch !== sceneEpoch) {
    let nonempty = false;
    for (let i = 0; i < draws; i++) if (list[i * 4 + 1] && list[i * 4 + 3]) { nonempty = true; break; }
    if (nonempty) {
      const epoch = sceneEpoch, generation = installedGeneration, frameEngine = engine, frameGpu = gpu, frameScene = gpu.scene;
      pendingSceneEpoch = epoch;
      device.queue.onSubmittedWorkDone().then(() => {
        if (epoch === sceneEpoch && generation === wanted && shaderLoads === 0 && engine === frameEngine && gpu === frameGpu && gpu.scene === frameScene) globalThis.coaxial.sceneReady = true;
      }, (error) => { if (epoch === sceneEpoch) say(`scene frame: ${error?.message || error}`, true); });
    }
  }
  document.documentElement.classList.add("gpu");
  if (loading.on) loading.frame(globalThis.coaxial.sceneReady);
  if (readback) report(readback, texture);
  if (x.coaxial_sounds) playSounds(x);
  after(false);
}

// ---- input: keys, the pointer and typed text go to the app; its actions come out
const KEYS = { Escape: -1, Enter: -2, Backspace: -3, ArrowLeft: -4, ArrowRight: -5, ArrowUp: -6, ArrowDown: -7,
  Home: -8, End: -9, PageUp: -10, PageDown: -11, Delete: -12, Shift: -13, Control: -14, Tab: -15 };
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
// The player's place in this realm (action 11 keeps it, message 11 gives it back): small, so
// localStorage, whose write is done before a page being left goes.
const PLACE_KIND = 11, placeKey = `coaxial.${slot}.place`;
// The player's spell bar as they arranged it (action 12 keeps it, message 12 gives it back).
const BAR_KIND = 12, barKey = `coaxial.${slot}.bar`;
function local(key) { try { return localStorage.getItem(key) || ""; } catch { return ""; } }
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
globalThis.coaxial = { send, ready: false, sceneReady: false, facts: () => pageFacts(), probe: () => engine?.exports.coaxial_probe?.(arena.state) ?? 0 };
// The OpenAgents app turns the camera with the phone (its gyro), radians since the last call.
globalThis.coaxial.tilt = (dyaw, dpitch) => { engine?.exports.coaxial_tilt?.(arena.state, dyaw, dpitch); };
// A page that takes dropped files (data-drops: /projects, NEW PROJECT): drop.js uploads them, the app draws it.
if (script?.dataset.drops) startDrops({ send });
// A page that sends files to this site (data-uploads: what its picker takes): uploads.js, on the app's action 30.
if (script?.dataset.uploads) startUploads({ send, accept: script.dataset.uploads });
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
    else if (code === PLACE_KIND) { if (!golden) try { localStorage.setItem(placeKey, text); } catch {} }
    else if (code === BAR_KIND) { if (!golden) try { localStorage.setItem(barKey, text); } catch {} }
    else if (code === 6) setQuality(text);
    else if (code === ACT_VOICE && voice) voice.toggle(text === "end" ? "end" : text === "on");
    else if (code === ACT_VOICE_TO && voice) voice.face(text);
    else if (code === ACT_VOICE_SAY && voice) voice.say(text);
    else if (code === 5) { try { const p = JSON.parse(text); if (p && typeof p.key === "string") localStorage.setItem("verse.player", text); } catch {} }
    else dispatchEvent(new CustomEvent("coaxial", { detail: { code, text } }));
  }
  const cursor = ["", "pointer", "crosshair", "text"][call("coaxial_cursor")] || "";
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
// Touches on a phone, iOS WebKit (the OpenAgents app's WKWebView) above all: the canvas takes every
// touch (touch-action none in the page's CSS, and its touchmove here), so no pan or zoom starts a
// gesture that turns a tap's pointerup into a pointercancel; a pointercancel that ends a short, still touch is that tap; and the mouse events
// a phone makes up after a touch (a hover over what was tapped) are not the app's.
const TAP_MS = 600, TAP_MOVE = 24, AFTER_TOUCH_MS = 800;
const touches = new Map();   // pointerId -> { t, x0, y0, x, y }
let touchAt = -1e9;
canvas?.addEventListener("touchmove", (e) => { if (e.cancelable) e.preventDefault(); }, { passive: false });
canvas?.addEventListener("contextmenu", (e) => e.preventDefault());
for (const [type, phase] of [["pointerdown", 0], ["pointermove", 1], ["pointerup", 2], ["pointercancel", 3]])
  addEventListener(type, (e) => {
    if (!engine) return;
    const touch = e.pointerType !== "mouse", now = performance.now();
    if (touch) touchAt = now;
    else if (now - touchAt < AFTER_TOUCH_MS) return;
    let x = e.clientX, y = e.clientY, ph = phase;
    if (touch) {
      if (phase === 0) touches.set(e.pointerId, { t: now, x0: x, y0: y, x, y });
      const p = touches.get(e.pointerId);
      if (p && phase === 1) { p.x = x; p.y = y; }
      if (p && phase === 3) {
        // a cancel's coordinates can be stale or zero: where the finger last was
        x = p.x; y = p.y;
        if (now - p.t < TAP_MS && Math.hypot(p.x - p.x0, p.y - p.y0) < TAP_MOVE) ph = 2;
      }
      if (phase >= 2) touches.delete(e.pointerId);
    }
    const dpr = scale();
    if (ph !== 1) touched = touch;
    if (ph === 0) startAudio();
    call("coaxial_pointer", x * dpr, y * dpr, ph, touch ? 1 : 0, e.pointerId);
    after(ph === 2, ph === 2);
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
  if (!response.ok) throw new Error(`engine: ${response.status}`);
  const bytes = await readBytes(response, (l, t) => loading.engine(l, t));
  loading.step("compile");
  return loadEngine(bytes, generation);
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
      const generation = beginEngine();
      try {
        if (await fetchEngine(generation)) say(`${app} rebuilt in ${m.ms} ms, swapped in ${Math.round(performance.now() - t0)} ms`);
      } catch (e) { say(`${app}: ${e.message}`, true); }
    }
  };
  socket.onclose = () => setTimeout(live, 1000);
}

async function main() {
  try {
    await fetchEngine(beginEngine());
    // No usable WebGPU after all: show the page's text, as written (a realm's loader says so).
    loading.step("gpu");
    const started = await startGpu();
    if (!started) { document.documentElement.classList.remove("gpu"); loading.fail("SHADERS FAILED TO COMPILE", !gpu); return; }
    observeSize();
    requestAnimationFrame(frame);
    // The heavy ones first; sounds are fetched when first played.
    loading.step("assets");
    for (const a of firstAssets) loadAsset(a);
    for (const pack of firstPacks) loadPack(pack);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    loading.fail(`${gpu || /gpu|adapter|device/i.test(e.message) ? "GPU" : "ENGINE"} FAILED: ${e.message}`);
    say(`${app}: ${e.message}`, true);
  }
  if (dev) live();
}
main();
