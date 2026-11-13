// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Coaxial's browser host. Platform effects only, as Swift is on the phone:
// the WebGPU device and canvas, the frame clock, one WebAssembly.Memory that
// holds every byte of the app's state, and (in development) the live channel
// to `coax serve` that swaps in new code, shaders and pages while the app
// runs. Every decision about what to draw is the app's Coax code.
//
// An app is `/build/APP.wasm` (Coax compiled through C to wasm32) and
// `/APP.wgsl`. The wasm exports a scalar C face:
//   coaxial_layout()          state layout fingerprint (a development module also carries the state's
//                             field table, which a code reload uses to keep the state: state-keep.js)
//   coaxial_state_size()      bytes of state the host keeps
//   coaxial_state_init(state) a fresh state's struct defaults (the face's, from coax_default_T; every face has it)
//   coaxial_max_vertices(), coaxial_vertex_floats(), coaxial_uniform_floats()
//   coaxial_frame(state, vertices, uniforms, width, height, dpr, dt, still)
//                             -> vertex count
//   coaxial_page(state, page) optional: which page to draw (data-page, e.g. a 404)
//   coaxial_golden(on)        optional: golden mode, its animated effects held at one moment
//   coaxial_frame_plan_v1(state, features, out, cap)  optional: the frame's order, from the engine (GitHub #13303):
//                             metaverse-engine/render/passes.coax's operations as records of 8 words
//                             (render/frame-plan.coax) written to `out`; the words written, 0 refused. `features`
//                             is this host's word (FPH_*). draw() walks the records and decides no order, and a
//                             refused plan clears the frame; only a page
//                             without it is drawn in the built-in order. ?plan=check and ?plan=off on a
//                             development server; coaxial.framePlan() says what was walked
//   coaxial_bloom_passes_v1(state) optional: how many of the five bloom passes to draw this frame (the rest
//                             are left as they were; the app's post pass reads only those drawn)
//   coaxial_pixel_budget()    optional: the most pixels a frame draws (a page that traces every pixel);
//                             the canvas draws fewer and is scaled up, never past verse.quality's cap
// and, optionally, input and output, all drawn and decided by the app:
//   coaxial_key(state, key, mods) -> used     a code point, or a named key < 0 (KEYS)
//   coaxial_key_up(state, key, mods) -> used  the same key let go (a game holds keys)
//   coaxial_blur(state)       the page lost the keyboard: nothing is held
//   coaxial_pointer(state, x, y, phase, kind, id) device pixels; phase 0 down 1 move 2 up 3 cancel;
//                             kind 0 a mouse, 1 a finger or pen; id tells fingers apart
//   coaxial_pointer_buttons_v1(state, x, y, phase, kind, id, button) optional additive DOM button hook;
//                             button 0 left, 2 right, -1 on ordinary pointer moves
//   coaxial_wheel(state, dy)  the wheel, CSS pixels down (the page doesn't scroll)
//   coaxial_mouselook(state)  optional: 0 no mouselook here, 1 its cursor is free, 2 hold the pointer
//                             (Pointer Lock, inside a user gesture; metaverse-engine/cam/mouselook.coax)
//   coaxial_look_delta(state, dx, dy) the mouse moved while the pointer is held, CSS pixels
//   coaxial_pointer_lock(state, status) 1 the pointer is held, 0 let go (Escape too), 2 refused
//   coaxial_tilt(state, dyaw, dpitch) the phone turned, radians (the OpenAgents app's gyro: coaxial.tilt; a
//                             phone's browser, from devicemotion: yaw about the world's up, left positive;
//                             pitch about the screen's across, the top tilted back positive)
//   coaxial_gyro(state)       optional: 1 while the app wants the phone's turn (iOS asks for the motion
//                             permission at the next touch), 0 not (metaverse-engine/cam/gyro.coax)
//   coaxial_gyro_status(state, status) the motion sensor: 1 its turns come, 2 refused, 3 none
//   coaxial_inbox(state), coaxial_inbox_cap(), coaxial_message(state, kind, length)
//                             text in: kind 1 is typed or pasted text; kind 2 the page's data-text,
//                             once each time an engine loads (which project a page shows, say);
//                             others the page's own
//   coaxial_wants_text(state) focus the keyboard field (a phone's keyboard)
//   coaxial_field(state)      where the app draws that field: 4 floats, device pixels
//   coaxial_cursor(state)     0 the default, 1 a link, 2 a crosshair, 3 text, 4 none (the app draws its own crosshair)
//   coaxial_a11y(state) -> bytes, coaxial_a11y_buf(), coaxial_a11y_do(state, id)
//                             the app's controls, a line each ("ID ROLE STATE LABEL"), mirrored as DOM
//                             buttons and checkboxes beside the canvas, and the one pressed there (a11yMirror)
//   coaxial_pad(state, x, y, button) a controller's left stick (x right, y forward) and first button, once a frame
//   coaxial_hash(state, length) the location's hash (without '#') in the inbox, at load and on every change
//   coaxial_action(state), coaxial_outbox(state), coaxial_outbox_len(state), coaxial_action_done(state)
//                             out: 1 navigates to the outbox's URL (under data-root, if the page has one; or an https:// URL; a
//                             lightning: invoice is paid by this browser's WebLN wallet when it has one, whose
//                             preimage comes back as message 615, else it opens the device's wallet), 2 copies its text, 3 makes it the
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
//   coaxial_socket_open(url, url_len) -> id   0..3, or -1 when 4 are open or the URL is refused; a URL
//                             local:PATH opens the room server's local host instead (room-worker.js)
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
//   coaxial_http_bounded(url, url_len, body, body_len, flags, timeout_ms:i64) -> id
//                             answer wanted, no beacon; streamed bytes fit the module inbox, including
//                             STATUS\n. Deadline/overflow/network failure: one "0\n" reply. Same eight slots.
//   coaxial_http_cancel(id) -> 1 cancelled, 0 absent; silent, no later callback after cancel or engine replacement
//   coaxial_now_ms() -> i64   monotonic performance.now() milliseconds (not a frame delta or wall date)
// and optional opaque byte storage: env.coaxial_storage_put/get/delete/cancel, replies at 768 + slot.
// The bounded ABI, raw inbox header and cancellation semantics are in STORAGE.md. No key, event,
// account or cache policy lives in this host; nothing is persisted without an explicit app request.
// and the page itself, through the inbox (what the browser knows; the app decides what to keep):
//   600 the page facts, key=value lines, once an engine loads: tab (a random id for this tab,
//       sessionStorage), fresh (1 when the tab id was just made), visitor (a random id for this
//       browser, localStorage), now (Date.now()), nav (ms since the navigation), url, ref, ua, lang,
//       tz, sw, sh, vw, vh, dpr, coarse, touch, gpu, hidden, clip, webdriver, fcp, dcl, player and account (the
//       playtest player's public id and account, "" none; never its key), voice (the page's data-voice route, "" none), ruleset (data-ruleset: realms.json "ruleset", "" none),
//       fork (data-fork: a fork's slug on its page, docs/metaverse/realm-fork.md; "" none), rules (data-rules: its rule ids, a comma between),
//       style (the realm's art style, data-style: lowpoly, painterly, photoreal, flat or anime; "" not a realm),
//       variant (an anime realm's, data-variant: pastoral, action, dusk or neon; the URL's ?variant= sets it; no
//       line on a page without either), zone.<id> (a zone's style, data-zones "id:style ...") and zone.<id>.variant
//       (an anime zone's, "id:anime:variant") and clock (data-clock: cycle, real or fixed:T; the URL's
//       ?clock=T, T from 0 to 1, pins it to fixed:T for shots; ?clock=cycle or real sets it), day (the visitor's
//       local time of day, 0 midnight to 1)
//   601 hidden, 602 shown again, 603 the page is being left (the app's beacon goes in that turn),
//   604 an error the page raised: message, file and line, a line each,
//   612 the network: "1" back online (a socket from before is likely dead), "0" offline (#12856)
//   614 the dev server saved an edit to a world: its ID (coax serve's live channel; never in production, where
//       a page asks the API for a world's events on its own clock) (#13413)
//   700 a driver's command (coax drive, metaverse-engine/agent/drive.coax): the realm answers through
//       coaxial_drive_out(state) and coaxial_drive_out_len(state), read by coaxial.drive.send
//   613 the GPU, once the adapter opens: gpu_vendor, gpu_arch, gpu_fallback (also in every later 600), and
//       action 33 keeps the graphics rung the app settled at (verse.quality.settled, the next 600's settled=)
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
//   sounds:/DIR/  a directory of sounds the app plays by name, DIR/NAME.ogg each, fetched when first
//                 named (metaverse-engine/audio/mix.coax's lines; NAME lowercase letters, digits and dashes)
//   pack:URL      a realm's asset pack (metaverse/tools/pack_realm.py, docs/metaverse/realm-host.md): one file,
//                 streamed, that stands for four assets in its place, its catalog (data), vertices
//                 (storage), indices (index) and texture layers (layers), and adds its sounds by name.
//                 While it streams the app hears message 605, "LOADED TOTAL" bytes. One pack a page.
//   private:NAME  a private pack (a MetaHuman's, docs/metaverse/metahuman-support.md) that never ships as a
//                 file: on openagents.com (and its subdomains) always, a session from the broker
//                 (POST /api/verse/packs/NAME, coax/web/api/verse_packs.c) whose bytes are opened in
//                 privatePack; on a local address only when its URL asks (/private/NAME.pack from
//                 coax/web/serve.c's private builds). ?pack=OTHER asks for another, &outfit=NAME its NAME
//                 outfit, the pack OTHER-NAME. On a phone (or ?phone) it asks first for NAME-phone, the
//                 pack's phone build, then NAME. Never in a golden, nowhere else. Its four parts as pack:'s.
//                 Not asked for, absent, refused or broken, its parts fail quietly (bytes 0, no error,
//                 the world loads on), so the page draws what it has without it.
//   message 608     a claimed realm's maker's display name (oa-api /api/playtest/makers/<realm>), to draw "Made by <name>" and TIP
//   local:URL     data that only a local address loads: a lab's big licensed capture, kept in a cache
//                 outside the repo (coax/web/serve.c's /private/, Splat Lab's skull). Anywhere else,
//                 or absent, it arrives empty (bytes 0) and the page loads on without it.
//   mesh:URL      a traced page's mesh (Orbit League's roadster, metaverse/tools/mesh-bvh.coax): its bytes as
//                 a read-only storage buffer at group 0 binding 1, for a WGSL that declares
//                 `var<storage, read> mesh` there; until it arrives the shader reads 16 zero bytes
//   (a WGSL that declares `var<storage, read> frame` at group 0 binding 1 gets the frame store there
//   instead: memory the app rewrites each frame, too big for the uniform block's 64 KB, seen by
//   every stage; coaxial_frame_store_cap() bytes at most, coaxial_frame_store(state) and
//   coaxial_frame_store_bytes(state) copied each frame; 16 zero bytes until the engine says)
//   font:URL, icons:URL, licence:URL  GPUI's atlas (gpui.js, a WGSL with gpui_atlas): Geist's four
//                 faces in weight order, Zeron's icons ({name: svg}), and the fonts' licence, served beside them
// and each one that arrives (or fails, bytes 0) is told: coaxial_asset_ready(state, i, bytes), i its
// place in the list. The app's scene, drawn between bg and fx with a depth buffer once every GPU asset is in:
//   coaxial_draws(state), coaxial_draw_count(state)   u32 [first, count, base instance, instances] each,
//                             indexed draws over the index buffer (a base instance with its top bit
//                             set: that many vertices from first, in order, no index buffer)
//   coaxial_cull_v1(state, field)   optional, with a cull:URL asset: the GPU instance cull's block
//                             (instance-cull.js has its fields); its runs draw only what the
//                             frustum and the frame's own depth leave in view
//   coaxial_mesh_bounds_v1(state)   optional: a big static mesh's ask for its runs' boxes
//                             (metaverse-engine/render/mesh-cull.coax has its words), written once from the
//                             pack's vertex and index bytes (meshBounds); ?meshcull=0 or
//                             coaxial.meshCull(false) answers "draw it whole", to compare
//   coaxial_stores(), coaxial_store_cap(k), coaxial_store(state, k), coaxial_store_bytes(state, k)
//                             memory copied each frame to storage buffers; an app with
//   coaxial_store_version(state, k)  has store k copied only when that number changes (a
//                             generation: bump it when the store is rewritten)
//   coaxial_sounds(state), coaxial_sounds_len(state)  sounds to play now, a line each: NAME GAIN PAN
//                             [DELAY], or ~NAME GAIN PAN for a looping bed, or a mixed voice and the
//                             bus's lines (metaverse-engine/audio/mix.coax; playSoundText)
//   coaxial_sound_bank(state), coaxial_sound_bank_len(state)  sounds the realm rendered itself,
//                             a line each: NAME AT BYTES, WAV bytes in its memory (soundBank)
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
// WGSL has grass_vs/grass_fs (bg's groups), draws depth-tested and written; in an opaque mesh scene
// it follows scene geometry so hidden fragments can fail depth early. Blended scenes retain the old order. In a
// mesh scene (no caster_vs) it takes the scene's groups instead, so its blades stand on the scene's
// ground and read its shadow map:
//   coaxial_grass_instances(state)  original blade domain, one triangle (3 vertices) each (0: none)
// Optional grass ranges v1 retain IDs in that domain; absence retains the original full draw:
//   coaxial_grass_range_count_v1(state)  at most 128 ordered, nonoverlapping ranges
//   coaxial_grass_range_v1(state, range, field)  field 0 first instance, 1 instance count
// Coax bounds/selects the ranges; the SDK issues draw(3, count, 0, first).
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
//   coaxial_particle_version(state)  optional: the emitters are copied only when this number changes
// Sorting, optionally (Splat Lab's Gaussian splats, docs/metaverse/2026-11-03-splat-lab.md): a particles
// module with sort_vs, key_fs and sort_fs, on a page whose app exports coaxial_sort, gets its quads
// in an order the GPU sorts before they draw. coaxial_sort(state) points at f32s: the sort texture's
// width and height (texels: one a particle, a power of two in all), how many passes, whether to run
// them this frame (0: keep the last order), then four floats a pass, its step uniform. Pass 0 draws
// key_fs into the texture (each texel's key and index, rg32float); each pass after it draws sort_fs
// from the texture the pass before wrote into the other (ping-pong: one bitonic step each). Their
// groups: 0 the uniforms, 1 the emitters (binding 0), 2 the pass's step (binding 0, a vec4f) and the
// texture before it (binding 1). The particles' own group 1 then holds the sorted texture at binding
// 1, which particle_over_vs and particle_vs read to know which particle the n-th quad draws.
// A page with data-timing (realms.json "timing") asks for the GPU's timestamps, when it has them,
// and hears message 610, "SORT_US DRAW_US": the sort's passes and the main pass, in microseconds.
// A traced page (no scene: bg_fs draws its world, 5E Lab's glade) with a particles: module draws them
// too, right after bg_fs (and its grass and surface), the same two passes, not depth-tested: a page
// with no depth buffer gets pipelines without one, and its shader hides puffs itself from what its
// uniforms say (everglade/client/gpu/gpu-puff.coax fades them into the ground it traces). Pages with a scene
// draw theirs as before.
// Pictures and videos from this site, optionally, drawn where the app says, over the scene and under
// its strokes (Hecate's Cemetery's memorials):
//   coaxial_media(state), coaxial_media_len(state)  a line each: "image URL X0 Y0 X1 Y1" or "video URL X0 Y0 X1 Y1",
//                             or "over URL X0 Y0 X1 Y1", a picture drawn over the strokes too (a HUD's card),
//                             URL this site's ("/..."), the box in the canvas's device pixels; the picture fits
//                             inside it, centred. A video plays muted and loops; one no longer listed is let go.
//                             "over URL X0 Y0 X1 Y1 LOOK V0 V1": LOOK its brightness in hundredths (100 as it
//                             is) and V0 to V1 (thousandths) the band of the picture, top to bottom, stretched
//                             over the box (a hotbar's cooldown: the icon's top part, dimmed, over the icon).
//                             At most 8. Message 607 says how each one came in, a line each: "URL W H" (0 0: it failed).

import { prepareRandomImport, unsupportedImport } from "./entropy-import.js";
import { fastEditBoundary } from "./fast-edit.js";
import { stateTable, planState, applyState, stateNote } from "./state-keep.js";
import { startDrops } from "./drop.js";
import { startUploads } from "./uploads.js";
import { startDocs } from "./docs.js";
import { startCard } from "./card.js";
import { startVoice, ACT_VOICE, ACT_VOICE_TO, ACT_VOICE_SAY } from "./voice.js";
import { localRoomSocket } from "./room-worker.js";
import { wtSocket } from "./room-wt.js";
import { startGpui } from "./gpui.js";
import { startLoading, readBytes } from "./loader.js";
import { layerFeatures, layerPolicy, warmLayerPolicy, layerBuild, blockLayout, layerTier } from "./layers-host.js";
import { makeHttpRequests } from "./http-requests.js";
import { makeHostTransport } from "./host-transport.js";
import {openSocketRetention,makeSocketQueues} from "./socket-queue-host.js";
import {openPackHost,openPackTable} from "./pack-host.js";
import { layerBitmap, layerPoolWidth, poolLayerLevels, warmLayerPool } from "./layer-pool.js";
import { fetchPackInWorker, fetchPagedDemand, fetchPagedImage, fetchWholePack } from "./pack-fetch.js";
import { characterPass } from "./character-host.js";
import { instanceCull } from "./instance-cull.js";
import { makeHostLifecycle } from "./host-lifecycle.js";
import { openHostServices, openPcmMemory } from "./services-host.js";
import { openCoaxAudio, renderCoaxAudio, renderClipAudio } from "./audio-host.js";
import { openClipTimeline } from "./audio-timeline.js";
import { RenderHost } from "./render.js";
import {mediaSource} from "./media-source.js";
import {actionSource} from "./action-source.js";
import {warmPage, enterPage} from "./zone-host.js";

const script = document.currentScript || document.querySelector("script[data-app]");
const app = (script && script.dataset.app) || "app";
// A deploy names the files by their content (data-wasm, data-wgsl), so a CDN
// can keep them for good; in development they are /build/APP.wasm and /APP.wgsl.
const wasmUrl = (script && script.dataset.wasm) || `/build/${app}.wasm`;
const wgslUrl = (script && script.dataset.wgsl) || `/${app}.wgsl`;
const renderUrl = script?.dataset.render || "";
// A realm page (docs/metaverse/realm-host.md) names its realm (data-realm) and the
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
// The crash test (coaxial.trapNextFrame, #13298): a real trap in the next frame, in development and on staging only.
const crashTest = dev || location.hostname.startsWith("staging.");
const probe = dev && new URLSearchParams(location.search).has("probe"); // tests: read the frame back
// Agent mode (?agent, in development; coax drive opens every page so: docs/metaverse/agent-clock.md, #13096).
// Simulation time moves only when the agent's clock steps it (coaxial.agent.step): the page draws
// fixed frames as a clip does (clip mode below), and the local room (room-worker.js) ticks on the
// same clock. The session is pinned: the seed, epoch, locale and time zone of metaverse-engine/agent/pin.coax,
// which the driver hands over as globalThis.coaxialAgent before the page loads (?agent=SEED.EPOCH_MS
// by hand). Date, Math.random and crypto.getRandomValues answer from the pin, the date moving with
// the agent's clock. While the page loads, before the first step, it draws frames a microsecond apart.
const agentMode = dev && new URLSearchParams(location.search).has("agent");
let agentMs = 0, agentHeld = false, agentLoading = false, agentQueue = Promise.resolve();
const agentPin = agentMode ? agentPinned() : null;
function agentPinned() {
  const g = globalThis.coaxialAgent, q = (new URLSearchParams(location.search).get("agent") || "").split(".");
  const seed = Number(g?.seed ?? q[0]) >>> 0, epoch = Number(g?.epoch ?? q[1]);
  if (!seed || !(epoch > 0)) { console.warn("?agent without a pin (coax drive gives one): the clock is the agent's, but the seed and the date are this machine's"); return { pinned: false, locale: "", timeZone: "" }; }
  const pin = { pinned: true, seed, epoch, locale: String(g?.locale || ""), timeZone: String(g?.timeZone || "") };
  let r = seed;
  const byte = () => { r ^= r << 13; r ^= r >>> 17; r ^= r << 5; return r & 255; };
  Math.random = () => { let v = 0; for (let i = 0; i < 4; i++) v = v * 256 + byte(); return v / 4294967296; };
  try { crypto.getRandomValues = (a) => { const b = new Uint8Array(a.buffer, a.byteOffset, a.byteLength); for (let i = 0; i < b.length; i++) b[i] = byte(); return a; }; } catch {}
  const RealDate = Date, now = () => epoch + Math.floor(agentMs);
  globalThis.Date = class extends RealDate { constructor(...a) { if (a.length) super(...a); else super(now()); } static now() { return now(); } };
  globalThis.coaxialAgentSession = pin;   // room-worker.js: the room on the agent's clock, its seed and date the pin's
  return pin;
}
// Explicit local observation can retain normal animation and simulation.
const captureRequested = probe || (dev && globalThis.coaxialRenderCapture === true);
const mediaNoticeTrace = captureRequested ? [] : null;
let mediaNoticeDropped = 0;
let probeArmed = probe && !new URLSearchParams(location.search).has("probe-manual");
// ?probe&at=SECONDS: run the app's clock that far in 0.1 s steps before the
// probed frame, so a headless browser can see a moment of the animation.
const probeAt = probe ? Number(new URLSearchParams(location.search).get("at")) || 0 : 0;
// Reduced motion draws the finished frame at once (probes use it too: headless browsers barely tick).
const still = (probe && !probeAt) || matchMedia("(prefers-reduced-motion: reduce)").matches;
// Golden mode (goldens.ts sets coaxialGolden; ?golden by hand): an app that exports coaxial_golden holds its effects still.
const golden = globalThis.coaxialGolden === true || new URLSearchParams(location.search).has("golden");
// A golden page's clock is counted, not measured (#13303). A page with a scene (a pack or an index) gets no time until
// its scene is ready, or 600 frames have gone by; then every page gets GOLDEN_STEPS frames of a sixtieth of a second
// each, and after them the clock holds. So a golden is one moment of the build, whatever the machine's pace and
// however long the shot waits: before this, cloud shadows, walkers, water and fire moved between two shots of a build.
// The first key or pointer press ends it: a walk that plays the page (goldens.ts's journey) gets measured time from then.
const GOLDEN_STEPS = 120, GOLDEN_WAIT = 600;
let goldenSteps = 0, goldenWaited = 0, goldenPlayed = false;
if (golden) for (const type of ["keydown", "pointerdown"]) addEventListener(type, () => { goldenPlayed = true; }, { capture: true, passive: true });
function goldenStep(scene, measured) {
  if (goldenPlayed) return measured;
  if (scene && !globalThis.coaxial.sceneReady && goldenWaited < GOLDEN_WAIT) { goldenWaited++; return 0; }
  if (goldenSteps >= GOLDEN_STEPS) return 0;
  goldenSteps++; return 1 / 60;
}
// ?shadowmap: the one key-light map even where the realm lays out shadow tiles (A/B timing, #13016)
const shadowMapOnly = new URLSearchParams(location.search).has("shadowmap");
let frames = 0;
// Development tests only: ?probe&do=STEPS runs input before the probed frame,
// steps split by ";": "kTEXT" types keys, "K-2" a named key, "c40,120" clicks at CSS pixels, "m16:TEXT" a
// message, "t1.5" runs the clock that many seconds.
const probeDo = probe ? new URLSearchParams(location.search).get("do") || "" : "";

// Grown at load to whatever the app's module declares (importedMemoryPages).
// Coax's reviewed page declaration and wasm linker select the same ceiling.
const memoryPages = Number(script?.dataset.memoryPages || 1024);
if (!Number.isInteger(memoryPages) || memoryPages < 128 || memoryPages > 4096) throw new Error("invalid Coax memory declaration");
const memory = new WebAssembly.Memory({ initial: 128, maximum: memoryPages });
let engine = null;   // { exports, layout }
let hostTransport = null, hostLifecycle = null, hostServices = null, reconnectAfterEngine = false;
let socketQueues=null,lastSocketQueueRetired=null,socketDeliveryHeld=false,socketSendingHeld=false;
function hostLifeEvent(kind,nowUs=Math.floor(performance.now()*1000),arg=0){const actions=hostLifecycle?.event(kind,nowUs,arg)??0;if(actions&1)call("coaxial_blur");return actions;}
let engineBytes = null;   // the running engine's wasm: a rescue save names it (rescueIdentity)
let engineInstalling = false;
// The frame plan's words (coaxial_frame_plan_v1): the version record and at most 256 operations of 8 words.
const PLAN_WORDS = 257 * 8, PLAN_BYTES = PLAN_WORDS * 4;
let arena = null;    // { state, vertices, uniforms, floats, uniformFloats, maxVertices }
let gpu = null;      // { device, context, format, bindGroup, uniformBuffer, vertexBuffer, bg, fx }
let wanted = 0;      // the newest requested engine; an older finished build never replaces it
// Where a fresh load's time goes (docs/metaverse/fast-loads.md): [name, ms since the page began], in
// the order they happened. coaxial.loadMarks() reads them:
//   coax/web/tool.sh shot URL OUT.png 1280x720 --eval "JSON.stringify(coaxial.loadMarks())"
const loadMarks = [];
function loadMark(name) { if (loadMarks.length < 256) loadMarks.push([name, Math.round(performance.now() * 10) / 10]); }
const PACK_PARTS = ["data", "storage", "index", "layers"], PACK_KIND = 605;
const PACK_CATALOG = 1, PACK_VERTICES = 2, PACK_INDICES = 3, PACK_LAYER = 4, PACK_SOUND = 5;
// A pack's data layers (normal and ORM maps, metaverse/tools/humans/pack_material_layers.coax): their
// own rgba8unorm array, never sRGB-decoded, bound after the colour layers' sampler for a realm that
// asks for them (coaxial_data_layers_v1). The realm's catalog names them ("D i NAME lic").
const PACK_DATA_LAYER = 6;
const PACK_RIG = 7;   // a pack's rigs (VRG1, metaverse-engine/anim/rig-pack.coax, #13306): handed to the page before its catalog
function wantsDataLayers() { return !gpu?.renderer && engine?.exports.coaxial_data_layers_v1?.() === 1; }
let soundDir = "";   // a sounds:/DIR/ asset: sounds played by name from DIR/NAME.ogg
let packBlocks = [];   // a blocks:TAG,TAG asset: the block builds beside the page's pack (GitHub #13305)
const assets = parseAssets((script && script.dataset.assets) || "");
// What main() loads once the GPU is up (sounds wait to be played; fonts and icons are gpui.js's).
const LATER = ["sound", "loop", "particles", "cull", "font", "icons", "licence"];
let firstAssets = assets.filter((a) => !a.pack && !LATER.includes(a.kind)), firstPacks = new Set(assets.filter((a) => a.pack).map((a) => a.pack));
const goldenScene = firstPacks.size > 0 || firstAssets.some((a) => a.kind === "index");   // goldenStep waits for this scene
// A realm's loading screen (loader.js): told what really loads, gone on the world's first frame.
const loading = startLoading({ realm: realmId, assets: firstAssets.length + firstPacks.size,
  textured: firstPacks.size > 0 || firstAssets.some((a) => a.kind === "layers"),
  scene: firstPacks.size > 0 || firstAssets.some((a) => a.kind === "index"), golden });
// A claimed realm's maker (ev-claim-realm, docs/metaverse/makers.md): their name, from oa-api, as message
// MAKER_KIND for the realm to draw ("Made by <name>" and TIP, in Coax); nothing when unclaimed, offline or in a golden shot.
// Only a realm with a tip page asks (data-maker, coax/web/realms.ts): the rest have no maker to find.
const MAKER_KIND = 608;
let makerName = "";
if (realmId && !golden && script?.dataset.maker === "1" && /^[a-z0-9-]{1,64}$/.test(realmId)) {
  fetch(`/api/playtest/makers/${realmId}`, { credentials: "same-origin" }).then((r) => (r.ok ? r.json() : null)).then((m) => {
    if (!m || typeof m.name !== "string" || !m.name) return;
    makerName = m.name.slice(0, 40);
    if (globalThis.coaxial.ready) send(MAKER_KIND, makerName);
  }).catch(() => {});
}
let last = 0;
// This is a completed indexed scene frame in the current engine generation,
// after all outstanding shader loads have settled. Pixel/error checks still
// decide whether the frame actually looks correct.
let sceneEpoch = 0, pendingSceneEpoch = -1, installedGeneration = 0, shaderLoads = 0;
let renderReload = Promise.resolve();
let packHost=null,packRetirement=Promise.resolve();
// The character pass: only a world that places a character (coaxial_character_v1) loads anything.
const characters = characterPass({ gpu: () => gpu, dev, golden, site: /(^|\.)openagents\.com$/.test(location.hostname),
  privatePack: (session, cipher) => privatePack(session, cipher), unpack: (bytes) => unpack(bytes), decode: (bytes, options) => layerBitmap(bytes, options) });
// A pack's rig entry (kind 7) goes to the page that asks for it (coaxial_pack_rig), before the pack's
// catalog is delivered: the page's bake reads it. A page that does not ask is told nothing.
function deliverRig(rigs){
  const px=engine?.exports;if(!rigs||!px?.coaxial_pack_rig||!px.coaxial_pack_rig_ready)return;
  const n=rigs.bytes.length,at=Number(px.coaxial_pack_rig(arena.state,n))||0,capacity=Number(px.coaxial_pack_rig_capacity?.(arena.state)??0);
  const fits=at>0&&n<=capacity&&at+n<=memory.buffer.byteLength;
  if(fits)new Uint8Array(memory.buffer,at,n).set(rigs.bytes);
  px.coaxial_pack_rig_ready(arena.state,fits?n:0);loadMark("pack-rig");
}
const packGroups=new Map();
function retirePacks(test=()=>true){
  const selected=[...packGroups.values()].filter(test);
  for(const group of selected){group.retired=true;group.retiredResolve();group.result?.close();}
  const retired=Promise.all(selected.map(async group=>{
    await packHost?.retire(group);
    for(const row of [...group.parts,...group.sounds]){row.bytes=null;}
    if(packGroups.get(group.pack)===group)packGroups.delete(group.pack);
    group.pack.done=false;group.pack.fetching=false;
  }));
  packRetirement=Promise.all([packRetirement,retired]).then(()=>{});return retired;
}
function packSoundSource(asset){
  const group=packGroups.get(asset.sourcePack);if(!group?.result||group.retired)return null;
  return new Promise((resolve,reject)=>{
    group.result.use(async()=>{let release;const held=new Promise(done=>{release=done;});resolve({opus:asset.bytes,release});await held;}).catch(reject);
  });
}
function invalidateScene() {
  sceneEpoch += 1; pendingSceneEpoch = -1;
  globalThis.coaxial.sceneReady = false;
}
function beginEngine() {
  const generation = ++wanted;
  retirePacks(group=>group.loading);
  if(hostTransport){reconnectAfterEngine ||= sockets.some(Boolean);retireTransport();}
  else {
    httpRequests.reset();
    // A pinned/pre-contract face retains its existing live-swap semantics.
    for(const ws of sockets)if(ws)ws.__coaxialEngine=generation;
  }
  invalidateScene();
  return generation;
}
function retireTransport() {
  if(hostTransport){retireContractAudio();const retired=clipTimeline?.retire();clipTimeline=null;if(retired)audioRetirement=audioRetirement.then(()=>retired);}
  httpRequests.reset();resetSockets();hostTransport?.reset();hostTransport=null;hostLifecycle=null;hostServices=null;
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

let stateSaid = ""; // what the last engine swap did with the state, for the swap's dev note
async function loadEngine(bytes, generation) {
  await Promise.all([audioRetirement,packRetirement]);if(generation!==wanted)return false;
  if (dev) globalThis.coaxial.renderLifecyclePhase="engine-await-render";
  if (dev) globalThis.coaxial.renderLifecyclePhase="engine-compile";
  const module = await WebAssembly.compile(bytes);
  if (generation !== wanted) return false;
  const previousEngine = engine, previousRenderer = gpu?.renderer;
  if (previousRenderer) {
    previousRenderer.suspended = true;
    await previousRenderer.settled;
    await Promise.all(previousRenderer.pendingEffects);
  }
  if (generation !== wanted) { if (previousRenderer) previousRenderer.suspended = false;return false; }
  // The next operation can initialize imported memory. Retire every callback
  // sharing this media cache, including older same-instance shader owners.
  if (previousRenderer?.mediaCache) previousRenderer.mediaCache.retiring = true;
  engine = null;globalThis.coaxial.ready = false;
  const pages = importedMemoryPages(bytes), have = memory.buffer.byteLength / 65536;
  if (pages > have) memory.grow(pages - have); // instantiate refuses an imported memory smaller than declared
  const random = prepareRandomImport(memory, module, randomScratch);
  const fast = fastEditBoundary(message => say(message, true));
  const imports = Object.create(null);
  imports.env = Object.assign(Object.create(null), { memory, coaxial_measure_ms: () => performance.now(), coaxial_now_seconds: () => BigInt(Math.floor(Date.now() / 1000)),
    coaxial_now_ms: () => BigInt(Math.floor(clipRoom ? clipStart + clipNow : performance.now())), coax_dev_dispatch: value => fast.dispatch(value), ...socketImports, ...httpImports, ...random.imports, ...storageImports, ...nip07Imports });
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= Object.create(null))[imp.name] ||= wasiStub(imp.module, imp.name);
  }
  // The state's field table (a development module carries it: state-keep.js). When this module and
  // the running one both have one, the running state's bytes are copied out now: instantiating
  // writes the new module's data into the shared memory, and a larger module's can reach the state.
  const table = stateTable(module);
  stateSaid = "";
  const carried = previousEngine?.table && table && arena?.stateSize ? new Uint8Array(memory.buffer, arena.state, arena.stateSize).slice() : null;
  if (dev) globalThis.coaxial.renderLifecyclePhase="engine-instantiate";
  const instance = await WebAssembly.instantiate(module, imports);
  globalThis.coaxial.fastEdit?.stop();
  fast.bind(instance.exports);
  if (instance.exports.coax_dev_baseline) globalThis.coaxial.fastEdit = fast;
  else delete globalThis.coaxial.fastEdit;
  if (generation !== wanted) { stopEngine();return false; } // memory was already initialized
  const kept = await keptRead;   // before the first frame, so a kept world opens with it
  const rescue = await rescueRead, rescueId = rescue ? await rescueIdentity(bytes) : "";
  if (generation !== wanted) { stopEngine();return false; }
  byteStorage.reset(); // pending effects belong to the old engine, even if its state layout survives
  httpRequests.reset();
  const x = instance.exports;
  random.bind(x);
  x._initialize?.();
  hostTransport=makeHostTransport(x,memory);
  hostLifecycle=makeHostLifecycle(x);
  if(hostTransport){const services=await openHostServices();if(generation!==wanted)return false;hostServices=services;
    if(firstPacks.size){packHost??=await openPackHost();if(generation!==wanted)return false;}
    if(clipping&&globalThis.coaxialClipAudio){const timeline=await openClipTimeline();if(generation!==wanted){await timeline.retire();return false;}clipTimeline=timeline;}}
  if(hostTransport && sockets.some(Boolean)){reconnectAfterEngine=true;resetSockets();}
  if(hostTransport){
    const retention=await openSocketRetention(x.coaxial_inbox_cap());if(generation!==wanted)return false;
    socketQueues=makeSocketQueues(retention,{
      current:(ws,id)=>ws.__coaxialEngine===generation&&wanted===generation&&(sockets[id]===ws||sockets[id]===null&&ws.__coaxialLocallyClosed),
      closed:(ws,id)=>{if(sockets[id]===ws)sockets[id]=null;},
      deliver:(ws,id,event,bytes)=>socketEvent(id,event,"",ws,bytes,retention),
      schedule:()=>{if(!socketTimer)socketTimer=setTimeout(socketIdle,100);},
      canDeliver:()=>!waiting&&!!engine&&!socketDeliveryHeld,canSend:()=>!socketSendingHeld});
  }
  if(frameTrace)x.coaxial_frame_parts?.(0,-1);
  const layout = x.coaxial_layout();
  const heapBase = x.__heap_base?.value ?? 0;
  const floats = x.coaxial_vertex_floats(), maxVertices = x.coaxial_max_vertices();
  const uniformFloats = x.coaxial_uniform_floats(), stateSize = x.coaxial_state_size();
  if (!arena || heapBase > arena.base || floats !== arena.floats || maxVertices !== arena.maxVertices || uniformFloats !== arena.uniformFloats || stateSize > arena.stateCap) {
    // A fresh arena above the module's own data and stack, so a new
    // module's data segments never land on live state.
    const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
    const stateCap = align(Math.max(stateSize, 1024), 16);
    const need = base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64 + PLAN_BYTES;
    if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
    arena = { base, stateCap, state: base, vertices: base + stateCap, uniforms: base + stateCap + maxVertices * floats * 4,
      plan: base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64, floats, maxVertices, uniformFloats, layout: -1 };
    if (gpu) makeBuffers();
  }
  // A swap keeps the state. With both field tables: where the state's layout is unchanged it stays
  // as it is; else the new state starts from its defaults and takes every field both layouts have,
  // by name (state-keep.js has the rules), and the dev note names what was added and dropped.
  // Without them (a release module), the layout fingerprint decides: the same keeps, another starts over.
  let keep = null;
  try { if (carried && !(table === previousEngine.table && arena.layout === layout)) keep = planState(previousEngine.table, carried.length, table, stateSize); }
  catch (error) { console.error(`coaxial: ${app}: the state's field tables don't read (${error.message})`); }
  if (keep && !(keep.same && arena.layout === layout)) {
    new Uint8Array(memory.buffer, arena.state, arena.stateCap).fill(0);
    x.coaxial_state_init(arena.state);
    applyState(keep, carried, memory.buffer, arena.state);
    arena.layout = layout;
    stateSaid = stateNote(keep);
    globalThis.coaxial.stateKept = { note: stateSaid, root: keep.root, kept: keep.kept, added: keep.added, dropped: keep.dropped, reset: keep.reset, resized: keep.resized, pointers: keep.pointers };
    say(`${app}: ${stateSaid}`);
  } else if (arena.layout !== layout) {
    new Uint8Array(memory.buffer, arena.state, arena.stateCap).fill(0); // the state's shape changed: start it over
    x.coaxial_state_init(arena.state); // its struct defaults, as native (the face calls the compiler's coax_default_T)
    if (previousEngine) { stateSaid = "state layout changed: it starts over"; say(`state layout changed: ${app} starts over`); }
    arena.layout = layout;
  }
  x.coaxial_page?.(arena.state, Number((script && script.dataset.page) || 0));
  if (golden) x.coaxial_golden?.(1);
  x.coaxial_quality?.(arena.state, golden ? 0 : quality);
  engine = { exports: x, layout, table };
  if(reconnectAfterEngine){reconnectAfterEngine=false;send(612,"1");}
  engineBytes = bytes;
  arena.stateSize = stateSize;
  rescueFrom(rescue, rescueId, layout, stateSize);   // a save this realm left when it stopped: put back once its world is in
  installedGeneration = generation;
  invalidateScene();
  // Renderer metadata and scratch belong to this concrete Wasm instance.
  // Re-admit its package before any new producer frame reaches the GPU.
  if (renderUrl && gpu) {
    try { if (dev) globalThis.coaxial.renderLifecyclePhase="engine-renderer";await loadShadersOnce();if (generation !== wanted) { stopEngine();return false; }if (dev) globalThis.coaxial.renderLifecyclePhase="engine-renderer-done";for (const a of firstAssets) if (!a.done) loadAsset(a);for (const pack of firstPacks) if (!pack.done) loadPack(pack); }
    catch (error) {
      // Instantiation has already initialized shared linear memory. An old
      // facade cannot safely resume against those new data segments.
      stopEngine();
      loading.fail(`ENGINE REPLACEMENT STOPPED: ${error.message}`);throw error;
    }
  }
  if (script?.dataset.text) send(2, script.dataset.text);
  // data-lines="KIND:URL [KIND:URL...]": text files the app reads as message KIND, fetched fresh each
  // load (/projects: 43:/projects/published.txt, the projects published from a drop since the build).
  if (script?.dataset.lines && location.protocol !== "file:") {
    for (const entry of script.dataset.lines.split(/\s+/).filter(Boolean)) {
      const [kind, url] = entry.split(/:(.*)/);
      fetch(url, { cache: "no-store" }).then((r) => (r.ok ? r.text() : ""), () => "").then((t) => { if (t) send(Number(kind), t); });
    }
  }
  for (const a of assets) if (a.done) deliver(a); // a new build gets what the last one had
  if (dev) globalThis.coaxial.renderLifecyclePhase="engine-assets-delivered";
  sendHash();
  // The page's scripts may talk to the app from now (action 0: ready).
  if (kept) send(4, kept);
  if (x.coaxial_player_appearance_v1?.() === 1) {const appearance=local("aethyr.player.appearance.v1");if (appearance) send(16,appearance);}
  globalThis.coaxial.ready = true;loadMark("engine-ready");
  if (x.coaxial_message) send(PAGE_KIND, pageFacts());
  editorOpen();
  // A page opening in place under another (zone-host.js) is out of sight until it is shown: its app
  // hears so (message 601), then 602 with the wall clock (now=) when the page is handed over.
  if (x.coaxial_message && (() => { try { return window.top !== window && window.frameElement?.dataset.zone === "1"; } catch { return false; } })()) send(HIDDEN_KIND);
  if (makerName) send(MAKER_KIND, makerName);
  // Where the player was in this realm (action 11), before the first frame: a reload never waits on the network.
  const place = golden ? "" : local(placeKey);
  if (place) send(PLACE_KIND, place);
  const bar = golden ? "" : local(barKey);
  if (bar) send(BAR_KIND, bar);
  viewer();
  dispatchEvent(new CustomEvent("coaxial", { detail: { code: 0, text: "" } }));
  if (dev) globalThis.coaxial.renderLifecyclePhase="engine-ready";
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
    if (fd === 2 && text.trim()) lastStderr = { text: text.trimEnd().split("\n").pop(), at: performance.now() };   // a Coax trap prints PATH:LINE: WHAT, then stops (crashProps)
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
// Opt-in measurements do not delay connection, gameplay or rendering. Each
// concrete socket/engine receives a fresh independent Coax observer.
const roomTraceEnabled=new URLSearchParams(location.search).get('roomtrace')==='1';
const roomTraceModule=roomTraceEnabled?import('./latency-host.js'):null;
const roomTraceRetired=[];
function retireRoomTrace(ws){if(!ws?.__roomTrace)return;ws.__roomTrace.retire();roomTraceRetired.push(ws.__roomTrace);if(roomTraceRetired.length>4)roomTraceRetired.shift();ws.__roomTrace=null;}
function roomTraceApplied(ws){const x=engine?.exports;if(ws?.__roomTrace&&x?.coaxial_net_probe)ws.__roomTrace.applied(x.coaxial_net_probe(arena.state,12),x.coaxial_net_probe(arena.state,51),x.coaxial_net_probe(arena.state,11)*1000);}
function roomTraceSubmitted(queue){const mine=engine,device=gpu?.device,x=mine?.exports;if(!x?.coaxial_net_probe)return;for(const ws of sockets)if(ws?.__coaxialEngine===wanted)ws.__roomTrace?.submitted(queue,x.coaxial_net_probe(arena.state,11)*1000,()=>engine===mine&&gpu?.device===device&&sockets.includes(ws)&&ws.__coaxialEngine===wanted);}
function resetSockets() {
  for(let id=0;id<sockets.length;id++){
    const ws=sockets[id];sockets[id]=null;
    if(ws){retireRoomTrace(ws);try{ws.close();}catch{}}
  }
  if(socketQueues){socketQueues.reset();lastSocketQueueRetired=socketQueues.info();socketQueues=null;}
  socketQueue.length=0;socketHead=0;
  if(socketTimer){clearTimeout(socketTimer);socketTimer=0;}
}
const utf8 = (ptr, len) => new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len).slice());
// Socket events wait in arrival order and reach the app inside the frame, a few milliseconds a frame
// (a relay catching up sends thousands of signed events at once, and the app checks each one's
// signature: handled as they arrive they took half the main thread for the first ~17 s and Chrome
// held frames back up to 100 ms). A hidden tab, which draws no frames, is drained on a timer.
const SOCKET_BUDGET_MS = 12, socketQueue = [];
let socketHead = 0, socketTimer = 0, socketDrained = 0, socketFrames = 0;
function socketQueued(ws, id, event, text = "") {
  if(event===SOCKET_FRAME)ws.__roomTrace?.receive(text);
  if(event===SOCKET_CLOSED||event===SOCKET_ERROR)retireRoomTrace(ws);
  if (event === SOCKET_FRAME) socketFrames++;
  if(ws.__coaxialPolicy&&socketQueues){socketQueues.receive(ws,id,event,text);return;}
  socketQueue.push([ws, id, event, text]);
  if (!socketTimer) socketTimer = setTimeout(socketIdle, 100);
}
function socketIdle() {
  socketTimer = 0;
  if (performance.now() - socketDrained > 90) drainSockets();
  if (socketQueues?.hasInput() || socketHead < socketQueue.length) socketTimer = setTimeout(socketIdle, 100);
}
function drainSockets(budget = SOCKET_BUDGET_MS) {
  const t0 = socketDrained = performance.now();
  if(socketQueues){socketQueues.drain();return;}
  while (socketHead < socketQueue.length) {
    const [ws, id, event, text] = socketQueue[socketHead];
    socketQueue[socketHead++] = null;
    if (sockets[id] !== ws) continue;   // the app closed it: what it sent after says nothing
    if (event === SOCKET_CLOSED) sockets[id] = null;
    try { socketEvent(id, event, text, ws); } catch (e) { say(`${app} socket: ${e.message}`, true); }
    if (performance.now() - t0 > budget) break;
  }
  if (socketHead === socketQueue.length) { socketQueue.length = 0; socketHead = 0; }
  else if (socketHead > 4096) { socketQueue.splice(0, socketHead); socketHead = 0; }
}
// An event for socket id: its bytes go in the inbox, as typed text does.
function socketEvent(id, event, text = "", ws = null, ownedBytes = null, retention = null) {
  if(ws && (ws.__coaxialEngine!==wanted || (sockets[id] && sockets[id]!==ws)))return;
  if (waiting) {if(ownedBytes)throw Error('leased socket callback attempted inside a realm call');waiting.push(() => socketEvent(id, event, text, ws));return; }
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  let bytes = ownedBytes ?? new TextEncoder().encode(text);
  let kind=SOCKET_KIND+4*id+event;
  if(ws?.__coaxialPolicy){
    const receipt=ws.__coaxialPolicy.socketEvent(ws.__coaxialOwner,event,bytes.length,x.coaxial_inbox_cap());
    if(!receipt)return;
    kind=receipt.kind;if(receipt.failure)bytes=new Uint8Array();
  } else if (bytes.length > x.coaxial_inbox_cap()) { socketEvent(id, SOCKET_ERROR, "",ws); return; }
  const address=x.coaxial_inbox(arena.state);
  if(retention&&!retention.span(address,x.coaxial_inbox_cap(),bytes.length,memory.buffer.byteLength))throw Error('Coax refused socket inbox span');
  if(bytes.length)new Uint8Array(memory.buffer,address,bytes.length).set(bytes);
  x.coaxial_message(arena.state, kind, bytes.length);
  roomTraceApplied(ws);
  after(false);
}
const socketImports = {
  coaxial_socket_open(ptr, len) {
    const policy=hostTransport,owner=policy?.socketReserve();
    const id = policy ? owner?.id ?? -1 : sockets.indexOf(null);
    if (id < 0) return -1;
    let ws;
    // local:PATH is the room server's local host, a loopback over postMessage (room-worker.js, #12784)
    let url;
    try {
      url=policy ? new TextDecoder('utf-8',{fatal:true}).decode(new Uint8Array(memory.buffer,ptr,len)) : utf8(ptr,len);
      if(policy && url.startsWith("local:") && !policy.localUri(url))throw Error("local route refused");
      ws = url.startsWith("local:") ? localRoomSocket(url, script?.dataset.roomWasm || "/build/everglade-room.wasm") : url.startsWith("wt+") ? wtSocket(url) : new WebSocket(url);   // wt+: WebTransport first (room-wt.js, #13176)
    } catch { if(owner)policy.socketClose(owner);return -1; }
    ws.__coaxialPolicy=policy;ws.__coaxialOwner=owner;ws.__coaxialEngine=wanted;
    sockets[id] = ws;
    if(roomTraceModule)roomTraceModule.then(m=>m.openLatencyObserver()).then(observer=>{if(sockets[id]===ws&&ws.__coaxialEngine===wanted&&ws.readyState<2)ws.__roomTrace=observer;else observer.retire();}).catch(error=>{globalThis.coaxial.roomTraceError=String(error);});
    const mine = () => sockets[id] === ws && ws.__coaxialEngine===wanted;   // a closed socket says nothing more
    ws.onopen = () => mine() && socketQueued(ws, id, SOCKET_OPENED, ws.__wt ? "wt" : "");   // "wt": the room runs on WebTransport
    ws.onmessage = (m) => mine() && typeof m.data === "string" && socketQueued(ws, id, SOCKET_FRAME, m.data);
    ws.onerror = () => mine() && socketQueued(ws, id, SOCKET_ERROR);
    ws.onclose = () => mine() && socketQueued(ws, id, SOCKET_CLOSED);   // the slot frees when the app hears it
    return id;
  },
  coaxial_socket_send(id, ptr, len) {
    const ws = sockets[id];
    if(ws?.__coaxialPolicy&&socketQueues){if(ws.readyState===1&&ws.__coaxialPolicy.socketSend(ws.__coaxialOwner))socketQueues.send(ws,id,memory,ptr,len,text=>ws.__roomTrace?.before(text));return;}
    if (ws && ws.readyState === 1 && (!ws.__coaxialPolicy || ws.__coaxialPolicy.socketSend(ws.__coaxialOwner))) {const text=utf8(ptr,len),sample=ws.__roomTrace?.before(text);if(sample)ws.send(sample);ws.send(text);}   // WebSocket.OPEN, or a local socket's
  },
  coaxial_socket_close(id) {
    const ws = sockets[id];
    if (!ws) return;
    if(ws.__coaxialPolicy && !ws.__coaxialPolicy.socketClose(ws.__coaxialOwner))return;
    sockets[id] = null;
    retireRoomTrace(ws);
    ws.close();
    // The app hears it a turn later, never inside its own call.
    if(ws.__coaxialPolicy&&socketQueues){ws.__coaxialLocallyClosed=true;socketQueues.receive(ws,id,SOCKET_CLOSED);return;}
    setTimeout(() => socketEvent(id, SOCKET_CLOSED,"",ws));
  },
};

// ---- HTTP: bytes to this site and back, nothing else
const HTTP_KIND = 512;
function answer(id, text) {
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  if(text instanceof Uint8Array){send(HTTP_KIND+id,text);return;}
  const bytes = new TextEncoder().encode(text);
  send(HTTP_KIND + id, bytes.length > x.coaxial_inbox_cap() ? text.slice(0, text.indexOf("\n") + 1) : text);
}
const httpRequests = makeHttpRequests({emit: answer, capacity: () => engine?.exports.coaxial_inbox_cap() ?? 0,
  followRedirects:()=>hostServices?.httpFollow(engine?.exports.coaxial_host_contract_version?.()??0)??true,
  responseAllowed:status=>hostServices?.httpResponse(engine?.exports.coaxial_host_contract_version?.()??0,status)??true,
  policy:()=>hostTransport,player, origin: location.origin, frame: () => frameJpeg(), sendBeacon: navigator.sendBeacon ? (url, body) => navigator.sendBeacon(url, body) : undefined});
function httpRequest(urlPtr, urlLen, bodyPtr, bodyLen, flags, timeoutMs) {
  let url;
  try {url=hostTransport ? new TextDecoder('utf-8',{fatal:true}).decode(new Uint8Array(memory.buffer,urlPtr,urlLen)) : utf8(urlPtr,urlLen);}
  catch {return -1;}
  return httpRequests.request(url, bodyLen ? new Uint8Array(memory.buffer, bodyPtr, bodyLen) : null, flags, timeoutMs);
}
const httpImports = {
  coaxial_http: (urlPtr, urlLen, bodyPtr, bodyLen, flags) => httpRequest(urlPtr, urlLen, bodyPtr, bodyLen, flags),
  // A deadline failure is one ordinary "0\n" response. Cancellation is silent;
  // it invalidates ownership before aborting, so an old promise cannot answer a reused slot.
  coaxial_http_bounded: (urlPtr, urlLen, bodyPtr, bodyLen, flags, timeoutMs) => httpRequest(urlPtr, urlLen, bodyPtr, bodyLen, flags, Number(timeoutMs)),
  coaxial_http_cancel: id => httpRequests.cancel(id),
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
  emit: function emit(slot, bytes) {
    if (waiting) { waiting.push(() => emit(slot, bytes));return; }
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

// ---- NIP-07: the browser's Nostr signer, for coax/lib/nostr/keys.coax's signer requests.
// The page calls this only for a request the person approved, with signer_template's JSON
// ({kind, created_at, tags, content}). The answer comes back as message SIGNER_NIP07_KIND:
// "id\n" and the extension's signed event, or "id\n" alone when there is no signer or it
// refused. Nothing here judges the event; Coax's signer_reply checks it.
const SIGNER_NIP07_KIND = 609;
const nip07Imports = {
  coaxial_nip07_sign: (id, ptr, len) => {
    if (!Number.isInteger(id) || id <= 0 || len <= 0 || len > 65536 || ptr < 0 || ptr + len > memory.buffer.byteLength) return -1;
    let template;
    try { template = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len))); } catch { return -1; }
    const signer = globalThis.nostr;
    if (!signer || typeof signer.signEvent !== "function") { queueMicrotask(() => send(SIGNER_NIP07_KIND, `${id}\n`)); return 0; }
    Promise.resolve().then(() => signer.signEvent(template))
      .then((ev) => send(SIGNER_NIP07_KIND, `${id}\n${ev ? JSON.stringify(ev) : ""}`), () => send(SIGNER_NIP07_KIND, `${id}\n`));
    return 0;
  },
};

// ---- the page: what the browser knows about this visit, for the app
const PAGE_KIND = 600, HIDDEN_KIND = 601, SHOWN_KIND = 602, LEAVE_KIND = 603, ERROR_KIND = 604, VIEWER_KIND = 606, DRIVE_KIND = 700;
// A realm in the site's sandboxed frame (realm-frame.js, #13432): 720 is a message the site posted
// (metaverse-engine/host/realm-frame.coax reads it), 721 what the frame is, "ISOLATED OPAQUE", said before it.
const FRAME_KIND = 720, FRAME_ENV_KIND = 721;
// Who is logged in, the same on every page: /api/auth/viewer's lines (/api/auth/me's: K| npub, H| hex key,
// M| a member key it speaks for, J| a private project), or "" when nobody is (a 200, so no console error).
// Apps that care read VIEWER_KIND.
function viewer() {
  if (!engine?.exports.coaxial_message || location.protocol === "file:") return;
  fetch("/api/auth/viewer", { credentials: "same-origin" })
    // read the body either way: an unread one never finishes loading
    .then((r) => r.text().then((t) => (r.ok ? t : "")), () => "")
    .then((t) => { viewerAdmin = /(^|\n)A\|1(\n|$)/.test(t); viewerIn = t.trim().length > 0; send(VIEWER_KIND, t); editorOpen(); }, () => send(VIEWER_KIND, ""));
}
let viewerIn = false;      // the viewer's lines were not empty: someone is logged in
let viewerAdmin = false;   // the viewer's lines said A|1: an admin, who may edit a world (the server decides each write)
const randomId = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
// `store` gives the storage: naming it can throw too, in a sandboxed page with no origin (a realm on
// the realm origin, #13432), so it is asked for inside the try.
function kept(store, key) {
  try { const s = store(), v = s.getItem(key); if (v) return [v, false]; const n = randomId(); s.setItem(key, n); return [n, true]; }
  catch { return [randomId(), true]; }   // storage refused (a private window, a sandboxed frame): this page only
}
// The realm's style, zones and clock (docs/metaverse/art-styles.md, The style field).
function styleFacts() {
  const d = script?.dataset || {}, out = { style: d.style || "" };
  for (const z of (d.zones || "").split(/\s+/)) { const [id, st, v] = z.split(":"); if (id && st) out[`zone.${id}`] = st; if (id && st && v) out[`zone.${id}.variant`] = v; }
  // an anime realm's variant (data-variant); the URL's ?variant= picks another for shots
  const pick = new URLSearchParams(location.search).get("variant");
  if (d.variant || pick) out.variant = pick && /^[a-z]{1,16}$/.test(pick) ? pick : d.variant || "";
  const pin = new URLSearchParams(location.search).get("clock");
  out.clock = pin !== null && /^(0(\.\d{1,4})?|1(\.0{1,4})?)$/.test(pin) ? `fixed:${pin}` : pin === "cycle" || pin === "real" ? pin : d.clock || "";
  // the visitor's local time of day, 0 midnight to 1, for a clock that is real
  const t = new Date();
  out.day = ((t.getHours() * 3600 + t.getMinutes() * 60 + t.getSeconds()) / 86400).toFixed(5);
  return out;
}
const GPU_KIND = 613;
let gpuFacts = {};
function pageFacts() {
  const [tab, fresh] = kept(() => sessionStorage, "coaxial.tab"), [visitor] = kept(() => localStorage, "coaxial.visitor");
  let tz = ""; try { tz = Intl.DateTimeFormat().resolvedOptions().timeZone || ""; } catch {}
  const nav = performance.getEntriesByType?.("navigation")?.[0], fcp = performance.getEntriesByName?.("first-contentful-paint")?.[0];
  const clean = (v) => String(v ?? "").replace(/[\r\n]/g, " ");
  return Object.entries({ tab, fresh: fresh ? 1 : 0, visitor, now: Date.now(), nav: performance.now().toFixed(1), url: location.href,
    ref: document.referrer, ua: navigator.userAgent, lang: agentPin?.locale || navigator.language, tz: agentPin?.timeZone || tz, sw: screen.width, sh: screen.height,
    vw: innerWidth, vh: innerHeight, dpr: devicePixelRatio || 1, coarse: matchMedia("(pointer: coarse)").matches ? 1 : 0,
    touch: navigator.maxTouchPoints || 0, gpu: navigator.gpu ? 1 : 0, hidden: document.hidden ? 1 : 0,
    cores: navigator.hardwareConcurrency || 0, memory: navigator.deviceMemory || 0, quality: storedQuality(),
    settled: (() => { try { return localStorage.getItem("verse.quality.settled") || ""; } catch { return ""; } })(), ...gpuFacts,
    frame_hz: globalThis.coaxialPreferredFrameRate || 0, minimum_hz: globalThis.coaxialMinimumFrameRate || 0,
    sound: (() => { try { return localStorage.getItem("verse.sound") || ""; } catch { return ""; } })(),
    settings: (() => { try { return localStorage.getItem("verse.settings") || ""; } catch { return ""; } })(),
    gyro: (() => { try { return localStorage.getItem("verse.gyro") || ""; } catch { return ""; } })(),
    focused: document.hasFocus() ? 1 : 0,
    clip: globalThis.coaxialClip === true ? 1 : 0,
    webdriver: navigator.webdriver ? 1 : 0, fcp: fcp ? Math.round(fcp.startTime) : 0, dcl: nav ? Math.round(nav.domContentLoadedEventEnd) : 0,
    player: player()?.id || "", account: player()?.account || "", realm: realmId, slot, voice: script?.dataset.voice || "", ruleset: script?.dataset.ruleset || "",
    fork: script?.dataset.fork || "", rules: script?.dataset.rules || "", ...styleFacts() })
    .map(([k, v]) => `${k}=${clean(v)}`).join("\n");
}
addEventListener("visibilitychange", () => {hostLifeEvent(document.hidden?4:5);send(document.hidden ? HIDDEN_KIND : SHOWN_KIND);});
const NET_KIND = 612;
addEventListener("online", () => {hostLifeEvent(6);send(NET_KIND, "1");});
addEventListener("offline", () => {hostLifeEvent(6);send(NET_KIND, "0");});
let leaving = false;   // the page is going: a device lost now is not replaced
addEventListener("pagehide", () => { leaving = true;send(LEAVE_KIND); httpRequests.reset(); });
addEventListener("pageshow", () => { leaving = false; });
addEventListener("error", (e) => send(ERROR_KIND, `${String(e.message || "").slice(0, 200)}\n${String(e.filename || "").split("/").pop()}\n${e.lineno || 0}`));
addEventListener("unhandledrejection", (e) => send(ERROR_KIND, `${String(e.reason?.message || e.reason || "").slice(0, 200)}\npromise\n0`));
// What stopped the page, which the page's own analytics (analytics.coax) can't say because its engine
// may be gone or never came: a load that failed (load-fail), a lost GPU device (gpu-lost) and an engine
// or renderer that stopped (crash). One event in analytics.coax's batch shape, sent as a beacon to the
// same ingest, in this tab's session ("c" and the tab id, as analytics.coax names it) under the realm's
// page name (data-realm, else the app). The message only, no page data.
function lifeBeacon(ev, msg, more = null) {
  if (location.protocol === "file:" || !navigator.sendBeacon) return;
  const [tab] = kept(() => sessionStorage, "coaxial.tab");
  if (!/^[0-9a-f]{8,64}$/.test(tab)) return;
  const body = { sid: `c${tab}`, events: [{ ev, page: realmId || app, ct: Date.now(), p: { msg: String(msg || "").slice(0, 80), webgpu: !!navigator.gpu, ...(more || {}) } }] };
  if (navigator.webdriver) body.auto = "webdriver";
  try { navigator.sendBeacon("/api/analytics/events", new Blob([JSON.stringify(body)], { type: "text/plain" })); } catch {}
}

// ---- GPU: device, the two pipelines from the app's WGSL, buffers
async function startGpu() {
  const opened = await openDevice((device) => { loading.watch(device);loading.step("shaders"); });
  if (!opened) return false;
  gpu = opened;
  makeBuffers();
  return loadShaders();
}
// A device and the canvas configured for it, watched for loss (deviceLost). The first start and a
// lost device's replacement both take theirs here.
async function openDevice(made = () => {}) {
  if (!navigator.gpu) return null;
  let adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return null;
  // the GPU's facts for the graphics quality's device class (metaverse-engine/core/quality.coax quality_gpu):
  // in every later pageFacts, and told to the app now (message 613)
  gpuFacts = { gpu_vendor: String(adapter.info?.vendor || "").toLowerCase().slice(0, 32), gpu_arch: String(adapter.info?.architecture || "").toLowerCase().slice(0, 32),
    gpu_fallback: (adapter.info?.isFallbackAdapter ?? adapter.isFallbackAdapter) ? 1 : 0 };
  if (engine?.exports.coaxial_message) send(GPU_KIND, Object.entries(gpuFacts).map(([k, v]) => `${k}=${v}`).join("\n"));
  if(roomTraceEnabled)globalThis.coaxial.gpuAdapterInfo={vendor:adapter.info?.vendor,architecture:adapter.info?.architecture,device:adapter.info?.device,description:adapter.info?.description,fallback:adapter.info?.isFallbackAdapter??adapter.isFallbackAdapter??null};
  // a page with data-timing asks for the GPU's timestamps (message 610), when the adapter has them
  let timing = !!script?.dataset.timing && adapter.features.has("timestamp-query");
  // a world that may place a detailed character (the character pass) asks for her vertex words'
  // binding, as far as the adapter has it; she stays off on one that has less
  const character = Math.min(engine?.exports.coaxial_character_storage_v1?.() ?? 0, adapter.limits.maxStorageBufferBindingSize);
  const storage = Math.max(engine?.exports.coaxial_storage_limit_v1?.() ?? 0, character);
  // a headless benchmark (coax/web/flyover.coax) asks for each pass's GPU time (globalThis.coaxialPassTimer)
  const passTiming = globalThis.coaxialPassTimer === true && adapter.features.has("timestamp-query");
  // every block-compression format the adapter has, so Coax can pick one for the layers (GitHub #13014)
  // a page with a cull: module culls its pieces on the GPU (instance-cull.js), drawn by indirect draws
  // that start at their run's first piece
  const culling = assets.some((a) => a.kind === "cull") && adapter.features.has("indirect-first-instance");
  const features = [...(timing || passTiming ? ["timestamp-query"] : []), ...(culling ? ["indirect-first-instance"] : []),
    ...["texture-compression-bc", "texture-compression-astc", "texture-compression-etc2"].filter((f) => adapter.features.has(f))];
  warmLayerPolicy();
  // Some drivers (Linux Vulkan) list a feature but refuse a device that asks for it, with
  // VK_ERROR_INITIALIZATION_FAILED. Ask again with less: first without the block-compression
  // formats (the layers read device.features, so they fall back to plain textures), then without
  // GPU timing too, then the same on the system's other adapters. Only the last refusal stops the page.
  const compression = (f) => f.startsWith("texture-compression-");
  const ladder = [features, features.filter((f) => !compression(f)), features.filter((f) => !compression(f) && f !== "timestamp-query")]
    .filter((f, i, all) => i === 0 || f.length !== all[i - 1].length);
  let device = null, refused = null;
  // An adapter makes one device at most, refused or not ("consumed"), so every attempt after the
  // first asks for a fresh adapter.
  const attempt = async (a, asked) => {
    if (!a || !asked.every((f) => a.features.has(f))) return null;
    const options = asked.length ? { requiredFeatures: asked } : {};
    if (storage) options.requiredLimits = { maxStorageBufferBindingSize: Math.min(storage, a.limits.maxStorageBufferBindingSize) };
    try { return { d: await a.requestDevice(options), asked, a }; }
    catch (e) { refused = e; console.warn(`coaxial: the GPU refused a device (${asked.join(", ") || "no features"}): ${e.message}`); return null; }
  };
  let got = await attempt(adapter, ladder[0]);
  for (const preference of [undefined, "low-power", "high-performance"]) {
    for (const asked of preference === undefined ? ladder.slice(1) : ladder) {
      if (got) break;
      got = await attempt(await navigator.gpu.requestAdapter(preference ? { powerPreference: preference } : undefined).catch(() => null), asked);
    }
  }
  if (got) adapter = got.a;
  if (!got) throw refused ?? new Error("the GPU refused every device request");
  device = got.d;
  const granted = got.asked;
  if (!granted.includes("timestamp-query")) { timing = false; }
  device.lost.then((info) => deviceLost(device, info));
  made(device);
  if (dev) device.addEventListener("uncapturederror", (event) => { console.error("RENDER DEVICE",event.error?.message); });
  const context = canvas.getContext("webgpu");
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | (captureRequested ? GPUTextureUsage.COPY_SRC : 0) });
  // A headless numerical measurement can substitute an offscreen target.
  // The same renderer admits its format and executes the same graph/shaders.
  const measuredFormat = captureRequested && globalThis.coaxialRenderCaptureFormat;
  let measuredTexture;
  const renderContext = measuredFormat ? { getCurrentTexture() {
    if (!measuredTexture || measuredTexture.width !== canvas.width || measuredTexture.height !== canvas.height) {
      measuredTexture?.destroy();measuredTexture = device.createTexture({ size: [canvas.width, canvas.height], format: measuredFormat,
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
    }
    return measuredTexture;
  } } : context;
  return { adapter, device, context: renderContext, format: measuredFormat || format, timing };   // the adapter held: a collected one can drop its device
}

// ---- a lost GPU device (metaverse-engine/book ch084). The engine and every byte of its state are in
// memory, not on the GPU, so the host takes a new device, builds the pipelines again, uploads the
// assets again, and puts the state back as the loss found it (the assets' deliveries during the
// rebuild do not change it). Frames and input wait meanwhile; the first frame after draws the
// current step (no time passes in it). A new device lost while it is being built is built again.
// A device that can't be replaced within RECOVER_MS, or a fourth loss in a minute, saves the realm
// and offers a reload that restores it (stopped).
const RECOVER_MS = 10000, LOSSES_MAX = 3, LOSSES_WINDOW_MS = 60000;
let recovering = null, losses = [];   // recovering: { n } while a device is rebuilt, n its attempt
// What the world tells the realm while its device is replaced (a socket's event, a fetch's answer,
// a stored value) waits here, in order, and reaches it after the first frame that follows: a write
// during the rebuild would be lost when the state is put back, and one before that frame would
// change the state the loss found. null when nothing waits.
let waiting = null;
function releaseWaiting() {
  if (!waiting || recovering) return;
  queueMicrotask(() => { const w = waiting;waiting = null;for (const write of w || []) { if (!engine) return;write(); } });
}
async function deviceLost(device, info) {
  if (dev) globalThis.coaxial.renderDeviceLost = { reason: info.reason, message: info.message };
  if (!gpu || gpu.device !== device || leaving || !engine) return;
  const t0 = performance.now();
  gpu.retired = true;
  const retainedOptional = new Set(firstAssets.filter(a => engine.exports.coaxial_asset_optional_v1?.(a.i) && a.done && !a.failed));
  hostLifeEvent(8,Math.floor(t0*1000));
  lifeBeacon("gpu-lost", `${info.reason || "unknown"}${info.message ? `: ${info.message}` : ""}`);
  losses = losses.filter((t) => t0 - t < LOSSES_WINDOW_MS);losses.push(t0);
  say(`GPU device lost (${info.reason}${info.message ? `: ${info.message}` : ""}); taking a new one`, true);
  gpu.bg = null;
  try { gpu.renderer?.destroy(); } catch {}
  const retiredPacks=retirePacks();
  if (recovering) { recovering.n += 1;return; }   // the device being built went too: its rebuild starts again
  waiting ||= [];
  // Optional v1 scalar ABI: the Coax realm declares its recovery window in milliseconds.
  // Existing realms retain RECOVER_MS; the platform only applies the selected deadline.
  const realmRecoveryMs = engine.exports.coaxial_gpu_recover_ms_v1?.() ?? RECOVER_MS;
  const attempt = recovering = { n: 0 }, held = stateBytes(), until = t0 + (dev && globalThis.coaxialRecoverMs || realmRecoveryMs);
  globalThis.coaxial.gpuRecovery = { state: "recovering", reason: info.reason, losses: losses.length };
  let error = null;
  await retiredPacks;
  for (;;) {
    const n = attempt.n;
    try {
      if (losses.length > LOSSES_MAX) throw new Error(`the GPU was lost ${losses.length} times in a minute`);
      let timer;
      const late = new Promise((_, no) => { timer = setTimeout(() => no(new Error("no new GPU device in time")), Math.max(0, until - performance.now())); });
      await Promise.race([late, rebuildGpu(() => {
        if (attempt.n !== n) throw new Error("the new device was lost too");
        if (performance.now() > until) throw new Error("no new GPU device in time");
      }, until, retainedOptional)]).finally(() => clearTimeout(timer));
      if (attempt.n === n) { error = null;break; }
    } catch (e) { error = e; }
    if (attempt.n === n || losses.length > LOSSES_MAX || performance.now() > until) break;
  }
  recovering = null;
  if (error || !engine) {
    if (gpu) gpu.bg = null;
    globalThis.coaxial.gpuRecovery = { state: "failed", reason: info.reason, losses: losses.length, error: error?.message };
    stopped(`THE GPU STOPPED: ${error?.message || "the engine went"}`, held);
    engine = null;globalThis.coaxial.ready = false;waiting = null;
    return;
  }
  new Uint8Array(memory.buffer, arena.state, held.length).set(held);
  hostLifeEvent(9);
  last = 0;
  const ms = Math.round(performance.now() - t0);
  globalThis.coaxial.gpuRecovery = { state: "recovered", reason: info.reason, losses: losses.length, ms, waited: waiting?.length ?? 0 };
  say(`GPU device replaced in ${ms} ms`);
}
// A new device, the pipelines and every asset the world opens with, uploaded to it; `check` throws
// once this attempt is lost or late.
async function rebuildGpu(check, until, retainedOptional = new Set()) {
  // A GPU process that crashed is started again by the browser; until it is, there is no adapter.
  let opened = null;
  for (let wait = 100; !(opened = await openDevice()) && performance.now() + wait < until; wait *= 2) await new Promise((ok) => setTimeout(ok, wait));
  if (!opened) throw new Error("no GPU device");
  check();
  if (!renderUrl) {
    // the legacy path keeps its assets' GPU objects on the rows: fetched again (the browser's cache)
    for (const a of assets) if (firstAssets.includes(a) || firstPacks.has(a.pack))
      Object.assign(a, { done: false, fetching: false, failed: false, buffer: null, texture: null });
    for (const pack of firstPacks) Object.assign(pack, { done: false, fetching: false });
  } else engine?.exports.coaxial_render_reset?.();   // the renderer's bookkeeping starts over with its resources
  gpu = opened;
  makeBuffers();
  if (!(await loadShaders())) throw new Error("the shaders did not build");
  check();
  // the renderer path's rows are new ones (loadShadersOnce): none done yet
  await Promise.all([...firstAssets.map(a => {const task = loadAsset(a);return engine.exports.coaxial_asset_optional_v1?.(a.i) && !retainedOptional.has(a) ? Promise.resolve() : task;}), ...[...firstPacks].map(loadPack)]);
  check();
  if (firstAssets.some((a) => a.failed && (!engine.exports.coaxial_asset_optional_v1?.(a.i) || retainedOptional.has(a))) || assets.some((a) => a.pack && a.failed)) throw new Error("the world's assets did not load again");
}

function makeBuffers() {
  if (renderUrl) { gpu.renderer?.destroy();gpu.renderer = null;return; }
  const { device } = gpu;
  gpu.uniformBuffer = device.createBuffer({ size: align(arena.uniformFloats * 4, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  gpu.vertexBuffer = device.createBuffer({ size: arena.maxVertices * arena.floats * 4, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
  gpu.layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} }] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [{ binding: 0, resource: { buffer: gpu.uniformBuffer } }] });
  if (gpu.sans) useSans();
  if (gpu.gpuiOn) useGpui();
  if (gpu.meshOn) useMesh();
  if (gpu.frameOn) useFrame();
}
// The frame store (a WGSL with `var<storage, read> frame`): group 0 is the uniforms and a storage
// buffer the app's memory is copied into each frame (frameUpload), sized by coaxial_frame_store_cap.
function useFrame() {
  const { device } = gpu;
  gpu.frameOn = true;
  const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
  const cap = engine?.exports.coaxial_frame_store_cap?.() || 16;
  if (!gpu.frameBuffer || gpu.frameBuffer.size !== align(cap, 16)) {
    gpu.frameBuffer?.destroy();
    gpu.frameBuffer = device.createBuffer({ size: align(cap, 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  }
  gpu.layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: both, buffer: {} },
    { binding: 1, visibility: both, buffer: { type: "read-only-storage" } },
    ...(gpu.sans ? [{ binding: 2, visibility: GPUShaderStage.FRAGMENT, texture: {} },
      { binding: 3, visibility: GPUShaderStage.FRAGMENT, sampler: {} }] : [])] });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [
    { binding: 0, resource: { buffer: gpu.uniformBuffer } },
    { binding: 1, resource: { buffer: gpu.frameBuffer } },
    ...(gpu.sans ? [{ binding: 2, resource: gpu.sans.createView() },
      { binding: 3, resource: device.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear" }) }] : [])] });
}
function frameUpload(x) {
  if (!gpu.frameOn || !gpu.frameBuffer || !x.coaxial_frame_store) return;
  const bytes = Math.min(x.coaxial_frame_store_bytes(arena.state), gpu.frameBuffer.size) & ~3;
  if (bytes) gpu.device.queue.writeBuffer(gpu.frameBuffer, 0, memory.buffer, x.coaxial_frame_store(arena.state), bytes);
}

// The sans atlas: a page whose shader declares sans_atlas (binding 1) and
// sans_sampler (binding 2) gets the system sans rasterized here, so small text
// reads like the rest of the web. Characters: ASCII 32..126, then SANS_EXTRA;
// 16 columns of 96-texel cells, the pen at x 16 and the baseline at y 72 of
// each, drawn at 64 px and cut at its cell; every mip level is drawn at its own size rather than
// downsampled. Coax lays the text out with the same fonts' widths
// (metaverse-engine/ui/font.coax, "the system sans"), so this list, the cell
// and the font must match it.
const SANS_EXTRA = [183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800];
const SANS_FONT = 'Helvetica, Arial, "Liberation Sans", "Nimbus Sans", sans-serif';
let pageFontFamily = null, pageTitleFont = null;
async function loadPageFont() {
  const source = script?.dataset.uiFontSrc, family = script?.dataset.uiFontFamily;
  if (!source || !family) return;
  const response = await fetch(new URL(source, location.href));
  if (!response.ok) throw Error(`UI font HTTP ${response.status}`);
  const font = new FontFace(family, await response.arrayBuffer());
  await font.load();document.fonts.add(font);pageFontFamily = JSON.stringify(family);
  // A HUD style's title face (metaverse-engine/ui/hud-style.coax, docs/metaverse/hud-styles.md): data-ui-title-font-src,
  // -family and -weight. The atlas then holds two faces in one set of cells: the reading face in red, the
  // title face in green; only a realm whose strokes ask for a channel (everglade/scene/gpu/fivelab.coax fivelab_hud_fs) has one.
  const titleSource = script?.dataset.uiTitleFontSrc, titleFamily = script?.dataset.uiTitleFontFamily;
  if (!titleSource || !titleFamily) return;
  const titleResponse = await fetch(new URL(titleSource, location.href));
  if (!titleResponse.ok) throw Error(`UI title font HTTP ${titleResponse.status}`);
  const title = new FontFace(titleFamily, await titleResponse.arrayBuffer(), { weight: "100 900" });
  await title.load();document.fonts.add(title);
  // data-ui-title-font-scale (0.4 to 1): a wide title face is drawn that much smaller so its widest
  // glyphs fit their cells; the realm's layout scales its quads back up (metaverse/splice/splice-game.coax).
  const titleScale = Math.min(1, Math.max(0.4, Number(script?.dataset.uiTitleFontScale) || 1));
  pageTitleFont = { family: JSON.stringify(titleFamily), weight: script?.dataset.uiTitleFontWeight || "400", scale: titleScale };
}
function sansAtlas(device) {
  const codes = [...Array.from({ length: 95 }, (_, i) => 32 + i), ...SANS_EXTRA];
  const cols = 16, rows = Math.ceil(codes.length / cols), cell = 96, levels = 5;
  const texture = device.createTexture({ size: [cols * cell, rows * cell], format: "rgba8unorm", mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  for (let level = 0; level < levels; level++) {
    const c = cell >> level, w = cols * c, h = rows * c;
    const canvas = new OffscreenCanvas(w, h), g = canvas.getContext("2d");
    g.fillStyle = pageTitleFont ? "#f00" : "#fff"; g.textBaseline = "alphabetic";
    // Each glyph is cut at its own cell: a glyph wider than the room after its pen (Cinzel's Q, whose
    // tail ends 1.33 em out) otherwise leaves its end in the next glyph's cell, a mark before every R (#13116).
    const glyph = (code, i) => {
      const x = (i % cols) * c, y = Math.floor(i / cols) * c;
      g.save(); g.beginPath(); g.rect(x, y, c, c); g.clip();
      g.fillText(String.fromCodePoint(code), x + 16 / (1 << level), y + 72 / (1 << level));
      g.restore();
    };
    g.font = `${64 / (1 << level)}px ${pageFontFamily || SANS_FONT}`;
    codes.forEach(glyph);
    if (pageTitleFont) {
      g.globalCompositeOperation = "lighter"; g.fillStyle = "#0f0";
      g.font = `${pageTitleFont.weight} ${64 * pageTitleFont.scale / (1 << level)}px ${pageTitleFont.family}`;
      codes.forEach(glyph);
    }
    device.queue.copyExternalImageToTexture({ source: canvas }, { texture, mipLevel: level, premultipliedAlpha: true }, [w, h]);
  }
  return texture;
}
// The mesh (mesh:URL): group 0 is the uniforms and the mesh's storage buffer, 16 zero bytes (a
// mesh of no triangles) until the asset has arrived.
function useMesh() {
  const { device } = gpu;
  gpu.meshOn = true;
  gpu.layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } }] });
  bindMesh();
}
function bindMesh() {
  if (gpu?.renderer) return;
  if (!gpu.meshOn) return;
  const { device } = gpu, a = assets.find((x) => x.kind === "mesh" && x.done && !x.failed && x.buffer);
  gpu.meshEmpty ||= device.createBuffer({ size: 16, usage: GPUBufferUsage.STORAGE });
  gpu.bindGroup = device.createBindGroup({ layout: gpu.layout, entries: [
    { binding: 0, resource: { buffer: gpu.uniformBuffer } },
    { binding: 1, resource: { buffer: a ? a.buffer : gpu.meshEmpty } }] });
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
// gpui_sampler (3) draws the shared Zeron components (components/zeron/,
// /chat); its vertices blend source-over, as GPUI's do. The atlas is built
// for the device pixel ratio once the app's engine and the GPU are up, and
// again when the ratio changes; the glyph cache is served after each frame.
// Optional generic FX transport: the producer supplies textured canvas vertices;
// the host binds the already-uploaded scene atlas and selects straight alpha.
function fxSceneOptions(x, scene, gpui) {
  return { scene: !!scene && x?.coaxial_fx_scene_v1?.() === 1,
    alpha: !!gpui || x?.coaxial_fx_alpha_v1?.() === 1 };
}

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
  const next = renderReload.then(() => loadShadersOnce());
  renderReload = next.catch(() => {});return next;
}
async function loadShadersOnce() {
  loadMark("shaders-start");
  if (!engine) return false;
  shaderLoads += 1;
  invalidateScene();
  try {
  const { device, format } = gpu;
  if (renderUrl) {
    const previous = gpu.renderer;
    const candidate = new RenderHost({ device, context: gpu.context, format, memory, exports: engine.exports, arena,
      url: renderUrl, profile: Number(script.dataset.renderProfile), coarse: matchMedia("(pointer: coarse)").matches, capture: captureRequested,
      presentation: hostServices?.presentation(engine.exports.coaxial_host_contract_version?.()??0,Number(script.dataset.renderProfile)) ? "unorm8-v1" :
        captureRequested && ["float","unorm8-v1"].includes(globalThis.coaxialRenderPresentation) ? globalThis.coaxialRenderPresentation : false,
      canonicalLayers: Boolean(hostTransport) || (captureRequested && globalThis.coaxialCanonicalLayers === true),
      canonicalMedia: Boolean(hostTransport) || (captureRequested && globalThis.coaxialCanonicalMedia === true),
      mediaSpan: hostTransport ? hostServices.mediaSpan : null,
      onMedia: (rows) => {
        if(candidate.canonicalMedia){
          if(engine?.exports!==candidate.x||candidate.mediaCache?.retiring)return;
          try{const notice=candidate.mediaNotice(rows,engine.exports.coaxial_inbox_cap());send(notice.kind,notice.text);candidate.mediaNoticeError=null;}
          catch(error){candidate.mediaNoticeError=String(error);console.error('Coax media notice refused',error);}
        }else send(MEDIA_KIND, rows.filter((m) => m.ready || m.failed).map((m) => `${m.url} ${m.failed ? 0 : m.width} ${m.failed ? 0 : m.height}`).join("\n"));
      } });
    await candidate.initialize(previous);
    for (const asset of candidate.assetRows) asset.renderOwner = candidate;
    assets.splice(0, assets.length, ...candidate.assetRows);
    firstAssets = assets.filter((a) => a.initial && !a.pack);firstPacks = new Set(assets.filter((a) => a.initial && a.pack).map((a) => a.pack));
    gpu.renderer = candidate;
    gpu.bg = candidate.pipelines.get(0);gpu.fx = candidate.pipelines.get(12);gpu.scene3d = Boolean(candidate.pipeline(5, 0));
    if (previous) { if (!candidate.reuses) previous.destroy(candidate.retainedResources);else previous.ready = false; }
    await retirePacks(group=>!assets.some(row=>row.pack===group.pack));
    for(const group of packGroups.values())for(const sound of group.sounds)if(!assets.some(row=>row.kind==="sound"&&row.name===sound.name)){sound.i=assets.length;assets.push(sound);}
    if(hostTransport&&firstPacks.size)packHost??=await openPackHost();
    return true;
  }
  const code = await (await fetch(wgslUrl, { cache: dev ? "no-store" : "default" })).text();
  if (/var\s+sans_atlas\b/.test(code)) useSans();
  if (/var\s+gpui_atlas\b/.test(code)) useGpui();
  if (/var<storage,\s*read>\s+mesh\b/.test(code)) useMesh();
  if (/var<storage,\s*read>\s+frame\b/.test(code)) useFrame();
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
  const fxOptions = fxSceneOptions(engine?.exports, scene, gpu.gpuiOn);
  gpu.fxScene = fxOptions.scene;
  const fxLayout = fxOptions.scene ? device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, sceneLayout()] }) : layout;
  const sceneGroups = scene ? [gpu.layout, sceneLayout(), ...(shadows ? [shadowLayout()] : [])] : [];
  const bgLayout = casters ? device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, emptyLayout(), shadowLayout()] }) : layout;
  // A traced scene whose bg_fs writes @builtin(frag_depth) puts its depth in the buffer, so what
  // draws after it (grass) hides behind what it traced.
  const bgDepth = depthed && /@builtin\(frag_depth\)/.test(code) ? { depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "always" } } : flat;
  // Grass (docs/metaverse/art-styles.md 8), when the WGSL has grass_vs and grass_fs: coaxial_grass_instances(state)
  // blades of one triangle each, instanced from their cells' hashes (no buffer), drawn after bg_fs with bg's groups.
  const grassy = scene && /fn\s+grass_vs\b/.test(code) && !!engine?.exports.coaxial_grass_instances;
  const [bg, fx, sc, sh, bl, po, ca, gr, su, sa, st] = await Promise.all([
    device.createRenderPipelineAsync({ layout: bgLayout, ...bgDepth, vertex: { module, entryPoint: "bg_vs" }, fragment: { module, entryPoint: "bg_fs", targets: lit } }),
    device.createRenderPipelineAsync({
      layout: fxLayout, ...(post ? {} : flat),
      // one vec4f attribute per four floats of the app's vertex, in order
      vertex: { module, entryPoint: "fx_vs", buffers: [{ arrayStride: stride, attributes:
        Array.from({ length: Math.ceil(arena.floats / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] },
      // max, not add: where strokes meet, their glow doesn't double (GPUI's straight alpha, source over)
      fragment: { module, entryPoint: "fx_fs", targets: [{ format, blend: fxOptions.alpha ? {
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
    scene && engine.exports.coaxial_draw_blend_v1 ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: sceneGroups }),
      vertex: { module, entryPoint: "scene_vs" },
      fragment: { module, entryPoint: "scene_fs", targets: lit.map((target) => ({ ...target, blend: {
        color: { operation: "add", srcFactor: "src-alpha", dstFactor: "one" },
        alpha: { operation: "add", srcFactor: "zero", dstFactor: "one" } } })) },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" },
    }) : null,
    scene && engine.exports.coaxial_draw_blend_v1 ? device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: sceneGroups }),
      vertex: { module, entryPoint: "scene_vs" },
      fragment: { module, entryPoint: "scene_fs", targets: lit.map((target) => ({ ...target, blend: {
        color: { operation: "add", srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" },
        alpha: { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" } } })) },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" },
    }) : null,
  ]);
  gpu.sceneAdditive = sa;gpu.sceneAlpha = st;
  gpu.surface = su;
  gpu.bg = bg; gpu.fx = fx; gpu.scene = sc; gpu.shadow = sh; gpu.bloom = bl; gpu.post = po; gpu.caster = ca; gpu.grass = gr; // swapped whole, at the next frame
  // A procedural scene may have stores and no fetched assets. Asset delivery
  // also calls this; pending assets still defer binding until they are ready.
  if (sc) bindScene();
  gpu.waterCopy = post && /var\s+water_scene\b/.test(code);
  gpu.particles = sc ? await particlePipeline(post, true) : await particlePipeline(post, !!surfaced);
  gpu.cull = sc ? await cullPipelines() : null;
  const waterGroups = gpu.waterCopy ? [...sceneGroups, ...(sceneGroups.length < 3 ? [emptyLayout()] : []), waterCopyLayout()] : sceneGroups;
  [gpu.waterTransmit, gpu.waterEmit] = sc && /fn\s+water_vs\b/.test(code) ? await waterPipelines(module, waterGroups, post) : [null, null];
  return true;
  } finally {
    shaderLoads -= 1;loadMark("shaders-done");
    invalidateScene();
  }
}

// Water, when the WGSL has water_vs, water_transmit_fs and water_emit_fs: the app's last
// coaxial_water_draws(state) draws are water, drawn after the scene in two halves each, depth-tested
// and not written: the first multiplies what lies behind by its colour (per-channel transmittance),
// the second adds its own (docs/metaverse/water.md).
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

// W5 (docs/metaverse/water.md): a page whose WGSL declares water_scene (and draws through the post pass)
// has the scene's light and depth copied after the scene and before the water, read at @group(3):
// binding 0 the light (rgba16float, linear), binding 1 the depth. Refraction, contact foam and soft
// edges read them; the water's own triangles are not in them.
function waterCopyLayout() {
  gpu.waterCopyLayout ||= gpu.device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "unfilterable-float" } },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "depth" } }] });
  return gpu.waterCopyLayout;
}

// The instance cull (instance-cull.js): a page with a cull: module and coaxial_cull_v1; ?cull=0 (or
// coaxial.cull(false)) draws every piece the page picked, to compare.
const wantsCull = () => assets.some((a) => a.kind === "cull") && !!engine?.exports.coaxial_cull_v1;
async function cullPipelines() {
  const a = assets.find((s) => s.kind === "cull");
  if (!a || !wantsCull()) return null;
  try {
    const code = await (await fetch(a.url, { cache: dev ? "no-store" : "default" })).text();
    const cull = await instanceCull(gpu.device, code, (text) => say(text, true));
    if (cull) cull.off = new URLSearchParams(location.search).get("cull") === "0";
    return cull;
  } catch (e) { say(`${app}: cull: ${e.message}`, true); return null; }
}
// The particles' own module and pipeline; a page without one, or one that doesn't compile, draws none.
async function particlePipeline(post, depth) {
  const a = assets.find((s) => s.kind === "particles"), x = engine?.exports;
  gpu.particlesOver = null; gpu.particleDepth = false; gpu.sort = null;
  if (!a || !x?.coaxial_particles) return null;
  const { device, format } = gpu;
  try {
    const code = await (await fetch(a.url, { cache: dev ? "no-store" : "default" })).text();
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
    if (errors.length) { say(errors.map((m) => `particles.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"), true); return null; }
    // sorting (the header's "Sorting"): the particles' group also holds the sorted texture
    const sorting = !!x.coaxial_sort && /fn\s+sort_vs\b/.test(code) && /fn\s+key_fs\b/.test(code) && /fn\s+sort_fs\b/.test(code);
    const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
    const group = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: sorting ? both : GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } },
      ...(sorting ? [{ binding: 1, visibility: both, texture: { sampleType: "unfilterable-float" } }] : [])] });
    gpu.particleBuffer ||= device.createBuffer({ size: align(Math.max(x.coaxial_particle_cap(), 16), 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    gpu.particleVersion = undefined;
    gpu.sort = sorting ? await sortPipelines(module, group) : null;
    gpu.particleGroup = sorting ? null : device.createBindGroup({ layout: group, entries: [{ binding: 0, resource: { buffer: gpu.particleBuffer } }] });
    const add = { operation: "add", srcFactor: "one", dstFactor: "one" };
    const over = { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" };
    const under = { operation: "add", srcFactor: "zero", dstFactor: "one-minus-src-alpha" };
    // soft particles, when the module reads particle_depth and the page copies its scene for the water
    // (W5): the copy's light and depth at group 3, so a puff fades where it meets what stands behind it
    gpu.particleDepth = depth && gpu.waterCopy && /\bparticle_depth\b/.test(code);
    const layout = device.createPipelineLayout({ bindGroupLayouts: gpu.particleDepth ? [gpu.layout, group, emptyLayout(), waterCopyLayout()] : [gpu.layout, group] });
    const pipe = (vs, fs, color, alpha) => device.createRenderPipelineAsync({
      layout, vertex: { module, entryPoint: vs }, fragment: { module, entryPoint: fs, targets: post
        ? [{ format: "rgba16float", blend: { color, alpha } }, { format: "rgba16float", writeMask: 0 }]
        : [{ format, blend: { color, alpha } }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      ...(depth ? { depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" } } : {}),
    });
    // smoke, when the module has particle_over_vs: premultiplied over what is behind, its alpha
    // taking that much of the light's bloom share, drawn before the added particles
    const [lit, shade] = await Promise.all([pipe("particle_vs", "particle_fs", add, add),
      /fn\s+particle_over_vs\b/.test(code) ? pipe("particle_over_vs", "particle_over_fs", over, under) : null]);
    gpu.particlesOver = shade;
    return lit;
  } catch (e) { say(`${app}: particles: ${e.message}`, true); return null; }
}

// Sorting's pipelines and the layouts its passes share (the header's "Sorting"): key_fs and sort_fs
// draw a full-screen triangle (sort_vs) into an rg32float texture, reading the uniforms (group 0),
// the emitters (group 1) and the pass's step and the texture before it (group 2).
async function sortPipelines(module, particleLayout) {
  const { device } = gpu, f = GPUShaderStage.FRAGMENT;
  const emitters = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: f, buffer: { type: "read-only-storage" } }] });
  const step = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: f, buffer: { type: "uniform", hasDynamicOffset: true, minBindingSize: 16 } },
    { binding: 1, visibility: f, texture: { sampleType: "unfilterable-float" } }] });
  const layout = device.createPipelineLayout({ bindGroupLayouts: [gpu.layout, emitters, step] });
  const [key, sort] = await Promise.all(["key_fs", "sort_fs"].map((entryPoint) => device.createRenderPipelineAsync({
    layout, vertex: { module, entryPoint: "sort_vs" }, fragment: { module, entryPoint, targets: [{ format: "rg32float" }] } })));
  return { key, sort, particleLayout, step, w: 0, h: 0, textures: null, views: null, orders: null, reads: null, steps: null, final: 0, ran: false,
    emitterGroup: device.createBindGroup({ layout: emitters, entries: [{ binding: 0, resource: { buffer: gpu.particleBuffer } }] }) };
}
// This frame's sort, encoded before the main pass; false when the app's header asks for none.
function sortFrame(encoder, x) {
  const s = gpu.sort, { device } = gpu, at = x.coaxial_sort(arena.state);
  const head = new Float32Array(memory.buffer, at, 4);
  const w = head[0] | 0, h = head[1] | 0, passes = head[2] | 0, due = head[3] > 0;
  if (w < 1 || h < 1 || passes < 1 || w > device.limits.maxTextureDimension2D || h > device.limits.maxTextureDimension2D) return false;
  if (s.w !== w || s.h !== h) {
    for (const t of s.textures || []) t.destroy();
    s.textures = [0, 1].map(() => device.createTexture({ size: [w, h], format: "rg32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING }));
    s.views = s.textures.map((t) => t.createView());
    s.orders = s.views.map((view) => device.createBindGroup({ layout: s.particleLayout, entries: [
      { binding: 0, resource: { buffer: gpu.particleBuffer } }, { binding: 1, resource: view }] }));
    s.w = w; s.h = h; s.ran = false; s.reads = null;
  }
  // each pass's step at its own 256-byte offset (a dynamic offset into one buffer)
  if (!s.steps || s.steps.size < passes * 256) {
    s.steps?.destroy();
    s.steps = device.createBuffer({ size: Math.max(passes, 64) * 256, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    s.reads = null;
  }
  s.reads ||= s.views.map((view) => device.createBindGroup({ layout: s.step, entries: [
    { binding: 0, resource: { buffer: s.steps, size: 16 } }, { binding: 1, resource: view }] }));
  if (!due && s.ran) return false;
  const params = new Float32Array(memory.buffer, at + 16, passes * 4), staged = new Float32Array(passes * 64);
  for (let p = 0; p < passes; p++) staged.set(params.subarray(p * 4, p * 4 + 4), p * 64);
  device.queue.writeBuffer(s.steps, 0, staged);
  const timer = gpu.timing ? gpuTimer() : null;
  for (let p = 0; p < passes; p++) {
    const into = p % 2, stamps = {};
    if (timer && p === 0) stamps.beginningOfPassWriteIndex = 0;
    if (timer && p === passes - 1) stamps.endOfPassWriteIndex = 1;
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: s.views[into], loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 0 } }],
      ...(timer && (p === 0 || p === passes - 1) ? { timestampWrites: { querySet: timer.set, ...stamps } } : {}) });
    pass.setPipeline(p === 0 ? s.key : s.sort);
    pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, s.emitterGroup);
    pass.setBindGroup(2, s.reads[1 - into], [p * 256]);   // the texture the pass before wrote (pass 0 reads nothing)
    pass.draw(3); pass.end();
  }
  s.final = (passes - 1) % 2; s.ran = true;
  return true;
}
// The group the particles draw with: with sorting, the one holding the latest order (none yet: none).
function particleGroupNow() {
  if (!gpu.sort) return gpu.particleGroup;
  return gpu.sort.ran ? gpu.sort.orders[gpu.sort.final] : null;
}
// GPU timestamps (data-timing): 0 and 1 round the sort's passes, 2 and 3 round the main pass.
const TIMING_KIND = 610;
function gpuTimer() {
  if (!gpu.timer) {
    const { device } = gpu;
    gpu.timer = { set: device.createQuerySet({ type: "timestamp", count: 4 }), busy: false,
      resolve: device.createBuffer({ size: 32, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
      read: device.createBuffer({ size: 32, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }) };
  }
  return gpu.timer;
}
// After the frame's submit: the four stamps read back, then told to the app (once at a time).
function timingRead(timer) {
  timer.busy = true;
  const at = gpu;
  timer.read.mapAsync(GPUMapMode.READ).then(() => {
    const t = new BigUint64Array(timer.read.getMappedRange().slice(0));
    timer.read.unmap(); timer.busy = false;
    if (gpu !== at || t[1] < t[0] || t[3] < t[2]) return;
    send(TIMING_KIND, `${Math.round(Number(t[1] - t[0]) / 1000)} ${Math.round(Number(t[3] - t[2]) / 1000)}`);
  }, () => { timer.busy = false; });
}

// ---- the scene's assets and group 1
function parseAssets(text) {
  const list = [];
  for (const entry of text.split(/\s+/).filter(Boolean)) {
    const k = entry.indexOf(":"), kind = entry.slice(0, k), rest = entry.slice(k + 1), i = list.length;
    if (kind === "sound") { const e = rest.indexOf("="); list.push({ i, kind, name: rest.slice(0, e), url: rest.slice(e + 1) }); }
    else if (kind === "sounds") { if (/^\/[a-z0-9/_-]+\/$/.test(rest)) soundDir = rest; }
    else if (kind === "blocks") packBlocks = rest.split(",").filter((tag) => /^[a-z0-9]{1,8}$/.test(tag));
    // A .vpk3 pack loads by ranges (coax/web/pack-host.js); in development ?ranges=N sets how many at once.
    else if (kind === "pack") { const pack = { url: rest, ranges: dev ? Number(new URLSearchParams(location.search).get("ranges")) || undefined : undefined }; for (const part of PACK_PARTS) list.push({ i: list.length, kind: part, pack }); }
    else if (kind === "private") {
      // ?pack&outfit=NAME: the pack's NAME outfit (pack_human.coax --outfit), its own pack, BASE-NAME.
      // On a local address only when asked (serve.c's /private/); on openagents.com always, through
      // the broker (privatePack), never in a golden.
      const q = new URLSearchParams(location.search), site = /(^|\.)openagents\.com$/.test(location.hostname);
      const pick = golden ? null : dev ? q.get("pack") : site ? q.get("pack") || rest : null;
      const outfit = pick !== null ? q.get("outfit") : null, name = (pick || rest) + (outfit ? `-${outfit}` : "");
      // a phone (a coarse pointer on a small screen, or ?phone; ?phone=0 never) asks first for the
      // pack's phone build, NAME-phone (pack_human.coax --phone: smaller layers and meshes), and the
      // full pack when there is none
      const asked = q.get("phone"), phone = asked !== null ? asked !== "0"
        : matchMedia("(pointer: coarse)").matches && Math.min(screen.width, screen.height) <= 820;
      const urls = (phone ? [`${name}-phone`, name] : [name]).map((n) => dev ? `/private/${n}.pack` : `/api/verse/packs/${n}`);
      const pack = { url: urls[0], urls, broker: !dev, quiet: true,
        away: pick === null || !/^[a-z0-9-]{1,58}$/.test(name) };
      for (const part of PACK_PARTS) list.push({ i: list.length, kind: part, pack });
    }
    else if (kind === "local") list.push({ i, kind: "data", url: rest, urls: [rest], quiet: true, away: !dev || golden });
    else list.push({ i, kind, url: rest, urls: rest.split("|") });
  }
  return list;
}
// ---- a realm's asset pack (metaverse/tools/pack_realm.py has the format)
// A public pack's bytes on their way (pack-fetch.js): fetched, unzipped or inflated in a worker,
// on this thread when there is none. main() starts each as soon as the engine is in, by its URL;
// loadPack takes it up once the GPU is ready for it.
const packIncoming = new Map();
function packBytes(pack) {
  const paged = /\.vpk3$/.test(pack.url), gzip = pack.url.endsWith(".pack.gz");
  let told = 0;
  const progress = (loaded, total) => { loading.bytes(pack.url, loaded, total); if (performance.now() - told > 100) { told = performance.now(); send(PACK_KIND, `${loaded} ${total}`); } };
  // A page that takes its world by cell (coaxial_cell_asks_v1, GitHub #13280) gets a paged pack's
  // roots now and each cell's pages when its cell controller asks (pumpCells): got.cells.
  const demand = paged && !!engine?.exports.coaxial_cell_asks_v1;
  const here = async () => demand ? fetchPagedDemand(packHost ??= await openPackHost(), pack, { owner: pack, progress })
    : paged ? fetchPagedImage(packHost ??= await openPackHost(), pack, { owner: pack, progress }) : fetchWholePack(liveUrl(pack.url), { gzip, progress });
  const job = fetchPackInWorker({ url: new URL(liveUrl(pack.url), location.href).href, gzip, paged, demand, ranges: pack.ranges }, progress);
  loadMark("pack-asked");
  const promise = job.promise.catch((error) => { if (!error.worker) throw error; return here(); }).then((got) => { loadMark("pack-bytes");return got; });
  promise.catch(() => {});   // a page that stops before loadPack never asks
  return { promise, cancel: job.cancel };
}
// The world by cell (GitHub #13280; docs/metaverse/one-world.md, "Paged by cell"). A page whose pack is
// paged by cell exports coaxial_cell_asks_v1: once a frame the host reads the cells its controller
// waits for, fetches each one's ranges from the pack (cellPack.cells, pack-fetch.js: the catalog's Z
// line, then the pages that cover it, each admitted by Coax), copies them joined into the page and
// says so. The page decides: an answer is admitted only for the cell's current request at the
// manifest's digest. A cell with no line, or one whose fetch failed, is a failed load after a pause,
// and the controller asks again. A range the page took with one cell is not sent again with another
// (cellShared: the ground's record, which every cell's line names so the first cell in brings it).
// In development ?celldelay=MS holds every answer back that long (a slow connection for a check;
// with ?cellfree=N the first N answers are not held, so the world round the spawn is in and only
// later cells are slow), and coaxial.cellLog() lists each answer: the cell, its key, the
// milliseconds from ask to answer, its bytes and what the page said (0: taken).
let cellPack = null;
const cellAsks = new Set(), cellLog = [], cellShared = new Map();   // asks in flight; the answers; each range the page took, and the cell it came with
const cellDelay = dev ? Math.max(0, Number(new URLSearchParams(location.search).get("celldelay")) || 0) : 0;
let cellFree = dev ? Math.max(0, Number(new URLSearchParams(location.search).get("cellfree")) || 0) : 0;
function pumpCells(x) {
  const cells = cellPack?.cells;
  if (!cells || !x.coaxial_cell_asks_v1 || !x.coaxial_cell_loaded_v1) return;
  const n = x.coaxial_cell_asks_v1(arena.state), owner = engine, source = cellPack;
  for (let i = 0; i < n; i++) {
    const cell = x.coaxial_cell_ask_v1(arena.state, i, 0), request = x.coaxial_cell_ask_v1(arena.state, i, 1), key = x.coaxial_cell_ask_v1(arena.state, i, 2) >>> 0, id = `${cell}:${request}`;
    if (cellAsks.has(id)) continue;
    cellAsks.add(id);
    const began = performance.now(), pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
    const answer = (got) => {
      cellAsks.delete(id);
      if (engine !== owner || cellPack !== source || engine.exports !== x) return;
      let bytes = 0;
      if (got) {
        const total = got.parts.reduce((sum, part) => sum + part.byteLength, 0), at = (x.coaxial_cell_buffer_v1?.(arena.state, total) || 0) >>> 0;
        if (at && at + total <= memory.buffer.byteLength) { let o = at; for (const part of got.parts) { new Uint8Array(memory.buffer, o, part.byteLength).set(part); o += part.byteLength; } bytes = total; }
      }
      const said = x.coaxial_cell_loaded_v1(arena.state, cell, request, bytes ? got.digest : 0, bytes, bytes ? 0 : 1);
      if (said === 0 && bytes) for (const [from, size] of got.ranges) if (!cellShared.has(`${from}:${size}`)) cellShared.set(`${from}:${size}`, cell);
      if (cellLog.length < 4096) cellLog.push([cell, key, Math.round(performance.now() - began), bytes, said]);
      if (cellLog.length === 1) loadMark("cell-first");
    };
    cells.cell(key, [...cellShared].filter(([, with_]) => with_ !== cell).map(([range]) => range)).then(async (got) => { await pause(!got ? 500 : cellFree > 0 ? (cellFree--, 0) : cellDelay); answer(got); }, async (error) => { if (dev) console.warn(`${app} cell ${cell}: ${error?.message || error}`); await pause(500); answer(null); });
  }
}
// A big static mesh's runs' boxes (metaverse-engine/render/mesh-cull.coax, #13168), once the pack's catalog
// is delivered: the face asks with coaxial_mesh_bounds_v1 for one index range, and its runs of
// triangles each get the box of their vertices' positions (a vertex's first three words), flagged
// when a vertex's keep word matches. Which runs are drawn is the realm's; this only measures.
let meshCullOff = new URLSearchParams(location.search).get("meshcull") === "0";
function meshBounds(vertices, indices) {
  const x = engine?.exports, at = x?.coaxial_mesh_bounds_v1?.(arena.state) ?? 0;
  if (!at || at % 4 || !vertices || !indices || at + 48 > memory.buffer.byteLength) return;
  const ask = new Uint32Array(memory.buffer, at, 12);
  if (ask[0] !== 1) return;
  const [, first, count, word, stride, tris, cap, keepAt, keepMask, keepValue] = ask, tri3 = tris * 3, runs = tri3 ? Math.ceil(count / tri3) : 0;
  if (vertices.byteOffset % 4 || indices.byteOffset % 4 || !runs || runs > cap || first + count > indices.byteLength / 4 || at + 48 + runs * 32 > memory.buffer.byteLength) { ask[0] = 3; return; }
  const vu = new Uint32Array(vertices.buffer, vertices.byteOffset, vertices.byteLength >> 2), vf = new Float32Array(vertices.buffer, vertices.byteOffset, vertices.byteLength >> 2);
  const iu = new Uint32Array(indices.buffer, indices.byteOffset, indices.byteLength >> 2), box = new Float32Array(memory.buffer, at + 48, runs * 8);
  const keep = keepAt !== 0xffffffff, words = vu.length;
  for (let r = 0; r < runs; r++) {
    let x0 = Infinity, y0 = Infinity, z0 = Infinity, x1 = -Infinity, y1 = -Infinity, z1 = -Infinity, kept = 0;
    const end = Math.min((r + 1) * tri3, count);
    for (let i = r * tri3; i < end; i++) {
      const w = word + iu[first + i] * stride;
      if (w + 3 > words || (keep && w + keepAt >= words)) { ask[0] = 3; return; }
      const px = vf[w], py = vf[w + 1], pz = vf[w + 2];
      if (px < x0) x0 = px; if (px > x1) x1 = px; if (py < y0) y0 = py; if (py > y1) y1 = py; if (pz < z0) z0 = pz; if (pz > z1) z1 = pz;
      if (keep && ((vu[w + keepAt] & keepMask) >>> 0) === keepValue) kept = 1;
    }
    const b = r * 8; box[b] = x0; box[b + 1] = y0; box[b + 2] = z0; box[b + 3] = kept; box[b + 4] = x1; box[b + 5] = y1; box[b + 6] = z1; box[b + 7] = 0;
  }
  ask[10] = runs; ask[0] = meshCullOff ? 4 : 2;
}
function meshCullSwitch(on) {
  meshCullOff = !on;
  const at = engine?.exports.coaxial_mesh_bounds_v1?.(arena.state) ?? 0;
  if (!at) return false;
  const ask = new Uint32Array(memory.buffer, at, 12);
  if (ask[0] === 2 || ask[0] === 4) ask[0] = on ? 2 : 4;
  return ask[0] === 2;
}
async function loadPack(pack) {
  if(hostTransport)return loadPackV1(pack);
  if (pack.fetching || pack.done) return;pack.fetching = true;loadMark("pack-start");
  if (!gpu?.renderer) warmLayerPool();
  const parts = assets.filter((a) => a.pack === pack);
  const owner = engine, generation = wanted;
  let entries = null;
  try {
    if (pack.away) throw new Error("private");
    // Catalog entry views already retain this complete encoded pack. Reuse that
    // same storage on device replacement, within its original engine only.
    // Broker sessions still perform their authorization and download each time.
    let bytes = !pack.broker && pack.retainedSource?.owner.deref() === owner && pack.retainedSource.generation === generation
      ? pack.retainedSource.bytes : null;
    if (!bytes && !pack.broker && !pack.urls) {
      // A public pack, whole or range-addressable (.vpk3, each page admitted by the reactor): its
      // bytes come from a worker, begun before the GPU was up (packBytes, pack-fetch.js).
      pack.retainedSource = null;
      const incoming = packIncoming.get(pack.url) || packBytes(pack);packIncoming.delete(pack.url);
      const got = await incoming.promise;bytes = got.bytes;
      pack.cells?.close();pack.cells = got.cells || null;
      loadMark("pack-decoded");send(PACK_KIND, `${got.loaded} ${got.loaded}`);
    }
    else if (!bytes) {
      pack.retainedSource = null;
      let session = null, r = null;
      // the first of its urls that answers (a phone's build, then the full pack)
      for (const url of pack.urls || [pack.url]) {
        session = null;
        if (pack.broker) {
          const s = await fetch(url, { method: "POST", cache: "no-store", credentials: "same-origin" });
          if (!s.ok) { r = s; continue; }
          session = await s.json();
        }
        r = await fetch(session ? session.url : url, pack.quiet ? { cache: "no-store" } : undefined);
        if (r.ok && r.body) { pack.url = url; break; }
      }
      if (!r || !r.ok || !r.body) throw new Error(`${pack.url}: ${r ? r.status : 0}`);
      loadMark("pack-response");
      const reader = r.body.getReader(), chunks = [];
      let loaded = 0, total = session ? session.size : Number(r.headers.get("content-length")) || 0, told = 0;
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value); loaded += value.length;
        if (!session && loaded >= 12 && chunks[0].length >= 12) total = new DataView(chunks[0].buffer, chunks[0].byteOffset).getUint32(8, true);
        loading.bytes(pack.url, loaded, total);
        if (performance.now() - told > 100) { told = performance.now(); send(PACK_KIND, `${loaded} ${total}`); }
      }
      bytes = new Uint8Array(loaded);
      let at = 0; for (const c of chunks) { bytes.set(c, at); at += c.length; }
      chunks.length = 0;loadMark("pack-downloaded");
      if (session) bytes = await privatePack(session, bytes);
      loadMark("pack-decoded");send(PACK_KIND, `${loaded} ${loaded}`);
    }
    if (renderUrl) await renderReload;if (!parts.every(currentAsset)) return;
    // Optional pack gate v1: SHA256 is a platform effect; acceptance stays in the realm's Coax.
    const packGate = engine.exports;
    if (packGate.coaxial_pack_gate_version) {
      if (packGate.coaxial_pack_gate_version() !== 1) throw new Error("unsupported asset pack gate version");
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      if (renderUrl) await renderReload;if (!parts.every(currentAsset) || engine.exports !== packGate) return;
      const address = packGate.coaxial_pack_digest(arena.state), capacity = packGate.coaxial_pack_digest_capacity();
      if (!Number.isInteger(address) || address <= 0 || capacity !== digest.length || address > memory.buffer.byteLength - capacity)
        throw new Error("asset pack digest destination refused");
      new Uint8Array(memory.buffer, address, capacity).set(digest);
      if (packGate.coaxial_pack_admit(arena.state, bytes.length) !== 1) throw new Error("asset pack SHA256 refused by Coax");
    }
    loadMark("pack-admitted");entries = await unpack(bytes);if (!parts.every(currentAsset) || engine?.exports !== packGate) return;
    const { device } = gpu, of = (kind) => entries.filter((e) => e.kind === kind);
    const [catalog, vertices, indices, layers] = parts;
    deliverRig(of(PACK_RIG)[0]);
    catalog.bytes = of(PACK_CATALOG)[0].bytes; catalog.size = catalog.bytes.length;
    for (const [a, e] of [[vertices, of(PACK_VERTICES)[0]], [indices, of(PACK_INDICES)[0]]]) {
      if (gpu.renderer) { a.bytes = e.bytes;a.size = e.bytes.length;gpu.renderer.acceptAsset(a);continue; }
      // Immutable aligned entry views already own the upload span. Only tail
      // padding needs staging; do not duplicate a full groom on device recovery.
      const whole = e.bytes.length > 0 && e.bytes.length % 4 === 0 ? e.bytes : new Uint8Array(align(Math.max(e.bytes.length, 4), 4));
      if (whole !== e.bytes) whole.set(e.bytes);
      a.buffer = device.createBuffer({ size: whole.length, usage: (a.kind === "index" ? GPUBufferUsage.INDEX | (wantsCull() ? GPUBufferUsage.STORAGE : 0) : GPUBufferUsage.STORAGE) | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(a.buffer, 0, whole); a.size = e.bytes.length;
    }
    loadMark("pack-buffers");loading.textures(0, 2 * of(PACK_LAYER).length);   // each layer decoded, then on the GPU
    if (renderUrl) await renderReload;if (!parts.every(currentAsset)) return;
    const images = !gpu.renderer || gpu.renderer.canonicalLayers ? of(PACK_LAYER).map(e=>e.bytes)
      : await Promise.all(of(PACK_LAYER).map((e) => createImageBitmap(new Blob([e.bytes])).then((b) => (loading.textures(1, 0), b))));
    if (images.length) { layers.texture = await layerTexture(images);if (renderUrl) await renderReload;if (!parts.every(currentAsset)) return;if (gpu.renderer) gpu.renderer.layerTexture = layers.texture;layers.size = images.length; }
    else layers.failed = true;
    loadMark("pack-layers");if (wantsDataLayers()) loading.textures(0, 2 * of(PACK_DATA_LAYER).length);
    layers.dataTexture = wantsDataLayers() && of(PACK_DATA_LAYER).length ? await layerTexture(of(PACK_DATA_LAYER).map((e) => e.bytes), "rgba8unorm") : null;
    if (!parts.every(currentAsset)) return;
    if (pack.cells) {
      // by cell: the sounds' pages were not fetched with the roots; they follow the first image
      cellPack = pack;
      const sounds = of(PACK_SOUND).filter((e) => e.bytes.length), source = pack.cells, owner = engine;
      if (sounds.length) source.ranges(sounds.map((e) => [e.bytes.byteOffset - bytes.byteOffset, e.bytes.length])).then((parts) => {
        if (engine !== owner || pack.cells !== source) return;
        sounds.forEach((e, k) => { if (!assets.some((a) => a.kind === "sound" && a.name === e.name)) assets.push({ i: assets.length, kind: "sound", name: e.name, bytes: parts[k], done: true }); });
      }).catch((error) => { if (dev) console.warn(`${app} pack sounds: ${error?.message || error}`); });
    }
    else for (const e of of(PACK_SOUND)) if (!assets.some((a) => a.kind === "sound" && a.name === e.name)) assets.push({ i: assets.length, kind: "sound", name: e.name, bytes: e.bytes, done: true });
    if (!pack.broker && typeof WeakRef === "function" && engine === owner && wanted === generation && parts.every(a => !a.failed))
      pack.retainedSource = { owner: new WeakRef(owner), generation, bytes };
  } catch (e) { pack.retainedSource = null;for (const a of parts) a.failed = true; if (dev) console.error(`${app} asset pack: ${e.message}`); if (pack.live) pack.live.reason = e.message; else if (!pack.quiet) say(`${app}: ${e.message}`, true); }
  pack.fetching = false;pack.done = true;loadMark("pack-done");
  if (!parts.every(currentAsset)) return;
  if (pack.live?.reason) return;   // a refused live swap: liveAsset puts the old copy back (#13441)
  for (const a of parts) { a.done = true; deliver(a); }
  if (entries) meshBounds(entries.find((e) => e.kind === PACK_VERTICES)?.bytes, entries.find((e) => e.kind === PACK_INDICES)?.bytes);
  bindScene();loadMark("pack-delivered");
  loading.assetDone(parts[0].failed && !pack.quiet && `WORLD FAILED TO LOAD: ${pack.url.split("/").pop()}`);
}
// The Coax companion admits complete bytes before entry views reach the renderer.
async function loadPackV1(pack){
  if(pack.fetching||pack.done)return;
  const parts=assets.filter(row=>row.pack===pack),generation=wanted,exports=engine?.exports;
  const group={pack,parts,sounds:[],generation,exports,loading:true,retired:false,result:null};let mesh=null;
  group.retiredPromise=new Promise(resolve=>{group.retiredResolve=resolve;});
  const current=()=>!group.retired&&wanted===generation&&engine?.exports===exports&&parts.every(currentAsset);
  const check=()=>{if(!current())throw new DOMException("pack renderer retired","AbortError");};
  const rendererReady=async()=>{if(renderUrl)await Promise.race([renderReload,group.retiredPromise]);check();};
  try{
    if(pack.away)throw Error("private");
    if(!packHost)throw Error("pack reader is unavailable");
    let told=0;
    // A block build of the pack (GitHub #13305): the realm's blocks: asset lists the builds beside
    // it, NAME.TAG.vpk3, each layer GPU blocks with its levels; Coax picks one this device's GPU
    // takes (metaverse-engine/render/texture-format.coax). Without one, or when it won't load, the pack itself.
    let build=null;
    if(packBlocks.length&&!pack.broker&&!gpu.renderer&&/\.pack$/.test(pack.url)){
      pack.fetching=true;
      try{build=await layerBuild(layerFeatures(gpu.device.features),packBlocks,matchMedia("(pointer: coarse)").matches);}catch(error){if(dev)console.warn("layer build unavailable",error);}
      check();
    }
    const options={owner:group,current,progress:(loaded,total)=>{
      if(!current())return;loading.bytes(pack.url,loaded,total);
      if(!waiting&&performance.now()-told>100){told=performance.now();send(PACK_KIND,`${loaded} ${total}`);}
    }};
    // Admission is synchronous, before a group or fetch promise can retain bytes.
    const incoming=build?packHost.load({...pack,url:pack.url.replace(/\.pack$/,`.${build.tag}.vpk3`)},options)
      .catch(error=>{if(!current())throw error;if(dev)console.warn(`${pack.url}: its ${build.tag} build`,error);return packHost.load(pack,options);})
      :packHost.load(pack,options);
    pack.fetching=true;packGroups.set(pack,group);loadMark("pack-start");
    group.result=await incoming;loadMark("pack-bytes");
    await group.result.use(async result=>{
      await rendererReady();const entries=result.entries,of=kind=>entries.filter(entry=>entry.kind===kind);
      const [catalog,vertices,indices,layers]=parts;
      loading.textures(0,2*of(PACK_LAYER).length);
      const images=!gpu.renderer||gpu.renderer.canonicalLayers?of(PACK_LAYER).map(entry=>entry.bytes):await Promise.all(of(PACK_LAYER).map(entry=>createImageBitmap(new Blob([entry.bytes])).then(image=>(loading.textures(1,0),image))));
      await rendererReady();
      const texture=images.length?await layerTexture(images):null;
      await rendererReady();loadMark("pack-layers");
      if(wantsDataLayers())loading.textures(0,2*of(PACK_DATA_LAYER).length);
      const dataTexture=wantsDataLayers()&&of(PACK_DATA_LAYER).length?await layerTexture(of(PACK_DATA_LAYER).map(entry=>entry.bytes),"rgba8unorm"):null;
      await rendererReady();
      deliverRig(of(PACK_RIG)[0]);
      catalog.bytes=of(PACK_CATALOG)[0].bytes;catalog.size=catalog.bytes.length;mesh=[of(PACK_VERTICES)[0]?.bytes,of(PACK_INDICES)[0]?.bytes];
      for(const [row,entry]of [[vertices,of(PACK_VERTICES)[0]],[indices,of(PACK_INDICES)[0]]]){
        if(gpu.renderer){row.bytes=entry.bytes;row.size=entry.bytes.length;gpu.renderer.acceptAsset(row);}
        else{const whole=new Uint8Array(align(Math.max(entry.bytes.length,4),4));whole.set(entry.bytes);row.buffer=gpu.device.createBuffer({size:whole.length,usage:(row.kind==="index"?GPUBufferUsage.INDEX|(wantsCull()?GPUBufferUsage.STORAGE:0):GPUBufferUsage.STORAGE)|GPUBufferUsage.COPY_DST});gpu.device.queue.writeBuffer(row.buffer,0,whole);row.size=entry.bytes.length;}
      }
      layers.texture=texture;layers.dataTexture=dataTexture;layers.size=images.length;layers.failed=!texture;if(gpu.renderer&&texture)gpu.renderer.layerTexture=texture;
      for(const entry of of(PACK_SOUND))if((!result.ready||result.ready(entry))&&!assets.some(row=>row.kind==="sound"&&row.name===entry.name)){
        const sound={i:assets.length,kind:"sound",name:entry.name,bytes:entry.bytes,done:true,sourcePack:pack};group.sounds.push(sound);assets.push(sound);
      }
      if(!waiting)send(PACK_KIND,`${result.stored??result.bytes} ${result.stored??result.bytes}`);
    });
    // A VPK3 pack's deferred pages (its sounds) follow the first image.
    const later=group.result;
    if(later.ready)later.complete.then(()=>later.use(async result=>{for(const entry of result.entries)if(entry.kind===PACK_SOUND&&!assets.some(row=>row.kind==="sound"&&row.name===entry.name)){
      const sound={i:assets.length,kind:"sound",name:entry.name,bytes:entry.bytes,done:true,sourcePack:pack};group.sounds.push(sound);assets.push(sound);}})).catch(()=>{});
  }catch(error){
    if(group.retired||wanted!==generation||engine?.exports!==exports)return;
    for(const row of parts)row.failed=true;
    if(pack.live)pack.live.reason=error.message;else if(!pack.quiet)say(`${app}: ${error.message}`,true);
    group.result?.close();await packHost?.retire(group);if(packGroups.get(pack)===group)packGroups.delete(pack);
    if(pack.live){pack.live.sounds=group.sounds;return;}   // a refused live swap: liveAsset puts the old copy back (#13441)
  }finally{group.loading=false;}
  if(!current())return;
  pack.fetching=false;pack.done=true;
  for(const row of parts){row.done=true;deliver(row);}if(mesh)meshBounds(mesh[0],mesh[1]);mesh=null;bindScene();
  loading.assetDone(parts[0].failed&&!pack.quiet&&`WORLD FAILED TO LOAD: ${pack.url.split("/").pop()}`);
}
// A private pack from the broker (coax/web/api/verse_packs.c): the session's bytes are the gzipped
// pack under its own key (AES-256-CTR, a 64-bit block counter), opened here, unzipped and checked
// against the digest before anything reads them. The key is the session's alone and used once; the
// plain pack lives only in this page's memory and the GPU, never in a cache or a file.
async function privatePack(session, cipher) {
  const hex = (h) => new Uint8Array(h.match(/../g).map((b) => parseInt(b, 16)));
  if (cipher.length !== session.size) throw new Error("private pack: cut short");
  const key = await crypto.subtle.importKey("raw", hex(session.key), "AES-CTR", false, ["decrypt"]);
  const gz = await crypto.subtle.decrypt({ name: "AES-CTR", counter: hex(session.iv), length: 64 }, key, cipher);
  const plain = new Uint8Array(await new Response(new Blob([gz]).stream().pipeThrough(new DecompressionStream("gzip"))).arrayBuffer());
  const digest = [...new Uint8Array(await crypto.subtle.digest("SHA-256", plain))].map((b) => b.toString(16).padStart(2, "0")).join("");
  if (plain.length !== session.bytes || digest !== session.sha256) throw new Error("private pack: not the pack its digest names");
  return plain;
}
// The pack's entries, from the one table read (metaverse-engine/host/pack-table.coax pack_table, in the pack
// reactor): its header and table checked against its length; nothing outside it is read.
async function unpack(bytes) { return (await openPackTable()).table(bytes); }
// Hands an asset that has arrived to the app (data into its memory) and tells it.
function currentAsset(a) { return !renderUrl || !a.renderOwner || a.renderOwner === gpu?.renderer; }
// Optional legacy page companion. SHA is a browser effect; Coax admits its exact identity and spans.
function assetGateV1(a, x) {
  const at = x.coaxial_asset_digest_v1?.(arena.state, a.i) || 0;
  if (!at) return true;
  if (!a.pageDigest || at + 32 > memory.buffer.byteLength) return false;
  new Uint8Array(memory.buffer, at, 32).set(a.pageDigest);
  return !!x.coaxial_asset_admit_v1?.(arena.state, a.i, a.size);
}
async function pageBytesV1(a, x, owner) {
  const owned = () => gpu === owner && !owner.retired && engine?.exports === x;
  if (a.pageBytes) {if (!x.coaxial_asset_extent_v1(a.i, 0, a.pageBytes.length)) throw Error("Coax refused retained asset span");return a.pageBytes;}
  const r = await fetch(liveUrl(a.url));if (!r.ok) throw Error(`${a.url}: ${r.status}`);
  const reader = r.body.getReader(), chunks = [];let held = 0, complete = false;
  try {
    for (;;) {const row = await reader.read();if (!owned()) return null;if (row.done) {complete = true;break;}
      if (!x.coaxial_asset_extent_v1(a.i, held, row.value.length)) throw Error("Coax refused asset stream extent");
      chunks.push(row.value);held += row.value.length;loading.bytes(a.url, held, 0);
    }
  } finally {if (!complete) await reader.cancel().catch(() => {});reader.releaseLock();}
  const bytes = new Uint8Array(held);let at = 0;for (const chunk of chunks) {bytes.set(chunk, at);at += chunk.length;}return bytes;
}
function deliver(a) {
  if (!currentAsset(a)) return;
  const x = engine?.exports;
  if (!x) return;
  if (waiting && x.coaxial_asset_digest_v1?.(arena.state, a.i)) {const owner = gpu;waiting.push(() => {if (gpu === owner && !owner.retired && engine?.exports === x) deliver(a);});return;}
  if (!assetGateV1(a, x)) a.failed = true;
  if (a.kind === "data" && a.bytes) {
    const at = x.coaxial_asset?.(arena.state, a.i, a.bytes.length) || 0;
    if (gpu?.renderer) {
      const capacity = x.coaxial_asset_capacity?.(arena.state, a.i) ?? 0;
      if (x.coaxial_render_asset_span(at, capacity, a.bytes.length, memory.buffer.byteLength) !== 0) throw Error("Coax refused measured asset destination");
      new Uint8Array(memory.buffer, at, a.bytes.length).set(a.bytes);
    } else if(x.coaxial_host_copy_span) {
      const capacity=x.coaxial_asset_capacity?.(arena.state,a.i) ?? 0;
      if(x.coaxial_host_copy_span(BigInt(at),BigInt(capacity),BigInt(a.bytes.length),BigInt(memory.buffer.byteLength)))new Uint8Array(memory.buffer,at,a.bytes.length).set(a.bytes);
      else a.failed=true;
    } else if (at) new Uint8Array(memory.buffer, at, a.bytes.length).set(a.bytes);
  }
  x.coaxial_asset_ready?.(arena.state, a.i, a.failed ? 0 : a.size);
  gpu?.renderer?.assetComplete(a);
}
async function loadAsset(a) {
  if (a.fetching) return;a.fetching = true;
  const { device } = gpu;
  const owner = gpu, x = engine.exports, gated = !!x.coaxial_asset_digest_v1?.(arena.state, a.i);
  if (x.coaxial_asset_optional_v1?.(a.i) && !a.loadingDetached) {a.loadingDetached = true;loading.assetDone();}
  try {
    if (a.kind === "layers") {
      loading.textures(0, 2 * a.urls.length);
      if (renderUrl) await renderReload;if (!currentAsset(a)) return;
      const canonical = Boolean(gpu.renderer?.canonicalLayers);
      const images = await Promise.all(a.urls.map(async (u) => {
        const r = await fetch(liveUrl(u));
        if (!r.ok) throw new Error(`${u}: ${r.status}`);
        const bytes = await readBytes(r, (l, t) => loading.bytes(u, l, t));
        if (canonical) return bytes;
        const b = await createImageBitmap(new Blob([bytes]));
        loading.textures(1, 0);
        return b;
      }));
      if (renderUrl) await renderReload;if (!currentAsset(a)) return;
      a.texture = await layerTexture(images);if (renderUrl) await renderReload;if (!currentAsset(a)) return;
      if (gpu.renderer) gpu.renderer.layerTexture = a.texture;a.size = images.length;
    } else {
      if (a.away) throw new Error("local only");
      const bytes = gated ? await pageBytesV1(a, x, owner) : await fetchedOnce(a.url);
      if (gated && (!bytes || gpu !== owner || owner.retired || engine?.exports !== x)) return;
      if (renderUrl) await renderReload;if (!currentAsset(a)) return;
      a.size = bytes.length;
      if (gated) {
        const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
        if (gpu !== owner || owner.retired || engine?.exports !== x) return;
        a.pageDigest = digest;if (!assetGateV1(a, x)) throw Error("Coax refused asset identity");a.pageBytes = bytes;
      }
      if (a.kind === "data") a.bytes = bytes;
      else if (gpu.renderer) { a.bytes = bytes;gpu.renderer.acceptAsset(a); }
      else {
        const usage = (a.kind === "index" ? GPUBufferUsage.INDEX | (wantsCull() ? GPUBufferUsage.STORAGE : 0) : GPUBufferUsage.STORAGE) | GPUBufferUsage.COPY_DST | (x.coaxial_asset_index_v1?.(a.i) ? GPUBufferUsage.INDEX : 0);
        const whole = gated && bytes.length % 4 === 0 ? bytes : new Uint8Array(align(bytes.length, 4));
        if (whole !== bytes) whole.set(bytes);   // v1 immutable aligned pages need no second upload copy
        const priorBuffer = a.buffer;a.buffer = device.createBuffer({ size: whole.length, usage });
        device.queue.writeBuffer(a.buffer, 0, whole);
        if (gated) {gpu.sceneGroup = null;if (priorBuffer) device.queue.onSubmittedWorkDone().then(() => priorBuffer.destroy()).catch(() => priorBuffer.destroy());}
      }
    }
  } catch (e) {
    if (gated && (gpu !== owner || owner.retired || engine?.exports !== x)) return;
    a.failed = true;
    if (a.live) { a.live.reason = e.message;a.fetching = false;return; }   // a refused live swap: liveAsset puts the old copy back (#13441)
    if (x.coaxial_asset_optional_v1?.(a.i)) {
      if (a.kind === "storage") a.buffer ||= device.createBuffer({size:16,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.INDEX|GPUBufferUsage.COPY_DST});a.size = 0;
    } else if (!a.quiet) say(`${app}: ${e.message}`, true);
  }
  a.fetching = false;if (!currentAsset(a)) return;
  a.done = true;
  deliver(a);
  if (a.kind === "mesh") bindMesh();
  bindScene();
  if (!a.loadingDetached) loading.assetDone(a.failed && !a.quiet && `ASSET FAILED TO LOAD: ${(a.url || "").split("/").pop()}`);
}
// An asset's bytes, fetched once however many of the page's assets name the same URL (the Grid's
// Cast.bin is both data, for its clips, and storage, for its vertices).
const fetches = new Map();
function fetchedOnce(url) {
  if (!fetches.has(url)) fetches.set(url, (async () => {
    const r = await fetch(liveUrl(url));
    if (!r.ok) throw new Error(`${url}: ${r.status}`);
    return readBytes(r, (l, t) => loading.bytes(url, l, t));
  })());
  return fetches.get(url);
}
// A 2D-array texture, an image a layer, sRGB, mipmapped (every image at the first one's size). Data
// layers (format "rgba8unorm") are linear and keep their own size, under the same phone policy.
async function layerTexture(images, dataFormat = "") {   // the loading screen counts each layer as it goes up
  if (gpu.renderer) { const renderer=gpu.renderer,texture = await renderer.makeLayers(images);loading.textures(images.length*(renderer.canonicalLayers?2:1), 0);return texture; }
  // Pipeline two original atlases at most, overlapping decode with upload/mips.
  // Never retain the complete decoded array or all resized mipmaps.
  const encoded = images[0] instanceof Uint8Array;
  // A block build's layers go up as they are (GitHub #13305).
  const blocks = encoded && !dataFormat ? await blockLayout(images[0]).catch(() => null) : null;
  if (blocks) return blockTexture(images, blocks);
  const raw = dataFormat ? { colorSpaceConversion: "none", premultiplyAlpha: "none" } : {};
  const first = encoded ? await layerBitmap(images[0], dataFormat ? raw : undefined) : images[0];
  if (encoded) loading.textures(1, 0);
  const { device } = gpu, declaredSide = dataFormat ? 0 : engine?.exports.coaxial_layers_side_v1?.(arena.state) || 0,
    [dw, dh] = declaredSide ? [declaredSide, declaredSide] : [first.width, first.height];
  // Coax picks the format and the size tier (metaverse-engine/render/texture-format.coax). Image layers go up
  // as RGBA8, at most 512 a side on a phone (a block build's layers took blockTexture above).
  // Without the reactor the layers go up as they always have.
  const format = dataFormat || "rgba8unorm-srgb"; let w = dw, h = dh;
  try {
    const policy = await layerPolicy(layerFeatures(device.features), 0, matchMedia("(pointer: coarse)").matches, 0, Math.max(dw, dh));
    if (policy.block === 1 && policy.name === "rgba8unorm-srgb" && policy.side > 0 && policy.side < Math.max(dw, dh)) {
      const scale = policy.side / Math.max(dw, dh); w = Math.max(1, Math.round(dw * scale)); h = Math.max(1, Math.round(dh * scale));
    }
  } catch (error) { if (dev) console.warn("layer policy unavailable", error); }
  const levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
  const texture = device.createTexture({ size: [w, h, images.length], format, mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  // Every level is resized from the layer's own image. The first layer's image is already here; the
  // rest are decoded and resized in workers (layer-pool.js), several at once.
  const sizes = Array.from({ length: levels }, (_, level) => [Math.max(1, w >> level), Math.max(1, h >> level)]);
  const width = layerPoolWidth(w, h, images.length);
  async function levelsOf(layer) {
    if (encoded && layer !== 0) return poolLayerLevels(images[layer], !!dataFormat, sizes, width);
    const image = layer === 0 ? first : images[layer], made = [];
    const rest = await Promise.allSettled(sizes.map(([lw, lh], level) => level === 0 && image.width === w && image.height === h ? image
      : createImageBitmap(image, { ...raw, resizeWidth: lw, resizeHeight: lh, resizeQuality: "high" })));
    for (const row of rest) if (row.status === "fulfilled") made.push(row.value);
    if (!made.includes(image)) image.close();
    for (const row of rest) if (row.status === "rejected") { for (const bitmap of made) bitmap.close(); throw row.reason; }
    return made;
  }
  try {
    let nextLayer = 0, failure = null;
    async function uploadLayers() {
      while (nextLayer < images.length && !failure) {
        const layer = nextLayer++;
        let made;
        try { made = await levelsOf(layer); }
        catch (error) { failure ||= new Error(`texture layer ${layer}: ${error.message}`);return; }
        if (encoded && layer !== 0) loading.textures(1, 0);
        try {
          if (!failure) made.forEach((source, level) => device.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer] }, sizes[level]));
        } catch (error) { failure ||= error; } finally { for (const bitmap of made) bitmap.close(); }
        // Each maker waits for its own layer to reach the GPU before it takes the next.
        if (!failure) await device.queue.onSubmittedWorkDone().catch((error) => { failure ||= error; });
        loading.textures(1, 0);
      }
    }
    // Join every maker even on refusal, before destroying their shared texture.
    await Promise.all(Array.from({ length: width }, uploadLayers));
    if (failure) throw failure;
  } catch (error) { texture.destroy();throw error; }
  return texture;
}
// A block build's layers (metaverse-engine/render/block-layer.coax): every layer's levels written as the pack
// has them, no decode and no resize. Each layer has the first's shape. A phone's tier for the format
// (texture-format.coax tf_side) drops the levels above it.
async function blockTexture(images, first) {
  const { device } = gpu, layouts = [first];
  for (let k = 1; k < images.length; k++) layouts.push(await blockLayout(images[k]));
  if (layouts.some((l) => !l || l.code !== first.code || l.width !== first.width || l.height !== first.height || l.levels.length !== first.levels.length))
    throw new Error("block layers of different shapes");
  const side = await layerTier(Math.max(first.width, first.height), matchMedia("(pointer: coarse)").matches, first.code);
  let skip = 0;
  while (skip + 1 < first.levels.length && Math.max(first.width, first.height) >> skip > side) skip++;
  const levels = first.levels.slice(skip);
  const texture = device.createTexture({ size: [levels[0].width, levels[0].height, images.length], format: first.name, mipLevelCount: levels.length,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
  try {
    images.forEach((bytes, layer) => {
      levels.forEach((lv, level) => device.queue.writeTexture({ texture, mipLevel: level, origin: [0, 0, layer] }, bytes.subarray(lv.offset, lv.offset + lv.bytes),
        { bytesPerRow: lv.bytesPerRow, rowsPerImage: lv.rows }, [lv.width, lv.height, 1]));
      loading.textures(2, 0);
    });
    await device.queue.onSubmittedWorkDone();
  } catch (error) { texture.destroy(); throw error; }
  return texture;
}
function flatDataTexture(device) {
  if (gpu.flatData) return gpu.flatData;
  const texture = device.createTexture({ size: [1, 1, 1], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
  device.queue.writeTexture({ texture }, new Uint8Array([128, 128, 255, 255]), { bytesPerRow: 4 }, [1, 1, 1]);
  return (gpu.flatData = texture);
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
    if (wantsDataLayers()) entries.push({ binding: entries.length, visibility: GPUShaderStage.FRAGMENT, texture: { viewDimension: "2d-array" } });
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
  gpu.shadowLayout = gpu.device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "depth" } },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "comparison" } }] });
  shadowTexture();
  return gpu.shadowLayout;
}
// The map itself: 2048 square (1024 on a phone), or, when the realm's Coax lays out shadow tiles
// (coaxial_shadow_atlas_v1: metaverse-engine/render/cascades.coax, #13016), the atlas it asks for. Made again
// when the engine wants another size; a new one is empty (shadowFresh: the tiles are opened again).
function shadowTexture() {
  const { device } = gpu, coarse = matchMedia("(pointer: coarse)").matches ? 1 : 0, x = engine?.exports;
  const st = arena?.state;
  const atlas = st && x?.coaxial_shadow_atlas_v1 && x.coaxial_shadow_v1 && x.coaxial_shadow_event_v1 ? [x.coaxial_shadow_atlas_v1(st, coarse, 0), x.coaxial_shadow_atlas_v1(st, coarse, 1)] : null;
  const tiles = !shadowMapOnly && !!atlas && atlas[0] > 0 && atlas[1] > 0 && atlas[0] <= 8192 && atlas[1] <= 8192;
  const [w, h] = tiles ? atlas : [coarse ? 1024 : 2048, coarse ? 1024 : 2048];
  if (gpu.shadowMap && gpu.shadowMap.width === w && gpu.shadowMap.height === h && gpu.shadowTiles === tiles && gpu.shadowOwner === x) return;
  gpu.shadowMap?.destroy();
  gpu.shadowMap = device.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
  gpu.shadowView = gpu.shadowMap.createView();
  gpu.shadowGroup = device.createBindGroup({ layout: gpu.shadowLayout, entries: [
    { binding: 0, resource: gpu.shadowView },
    { binding: 1, resource: device.createSampler({ compare: "less", magFilter: "linear", minFilter: "linear" }) }] });
  gpu.shadowTiles = tiles; gpu.shadowOwner = x; gpu.shadowCoarse = coarse; gpu.shadowFresh = tiles;
}
// A tile's clear: one triangle over the viewport (the tile) at depth 1, depth always written. Browser
// glue only, as MEDIA_WGSL is: a render pass clears its whole attachment, and the other tiles keep theirs.
const SHADOW_CLEAR_WGSL = "@vertex fn v(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f { return vec4f(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 1.0, 1.0); }";
function shadowClear() {
  if (gpu.shadowClear) return gpu.shadowClear;
  const module = gpu.device.createShaderModule({ code: SHADOW_CLEAR_WGSL });
  gpu.shadowClear = gpu.device.createRenderPipeline({ layout: "auto", vertex: { module, entryPoint: "v" },
    primitive: { topology: "triangle-list" }, depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "always" } });
  return gpu.shadowClear;
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
  if (gpu?.renderer) return;
  // an asset or pack installed: the shadow tiles are drawn again (rate-limited in Coax: metaverse-engine/render/cascades.coax)
  if (engine?.exports.coaxial_shadow_event_v1 && arena?.state) engine.exports.coaxial_shadow_event_v1(arena.state, 1, 0);
  const gpuAssets = assets.filter((a) => ["storage", "index", "layers"].includes(a.kind));
  if (!gpu || gpu.sceneGroup || !engine?.exports.coaxial_draws || gpuAssets.some((a) => (!a.done || a.failed) && !engine.exports.coaxial_asset_optional_v1?.(a.i))) return;
  const x = engine.exports, { device } = gpu, resources = [];
  for (const a of assets) if (a.kind === "storage") {
    if (!a.buffer && x.coaxial_asset_optional_v1?.(a.i)) a.buffer = device.createBuffer({size:16,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.INDEX|GPUBufferUsage.COPY_DST});
    resources.push({ buffer: a.buffer });
  }
  if (!gpu.stores) {gpu.stores = [];gpu.storeVersions = [];for (let k = 0; k < (x.coaxial_stores?.() || 0); k++) gpu.stores.push(device.createBuffer({size:align(Math.max(x.coaxial_store_cap(k),16),16),usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST}));}
  for (const buffer of gpu.stores) resources.push({buffer});
  const layers = assets.find((a) => a.kind === "layers");
  if (layers) {
    resources.push(layers.texture.createView({ dimension: "2d-array" }));
    resources.push(device.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear", addressModeU: "repeat", addressModeV: "repeat", maxAnisotropy: 8 }));
    // a pack without data layers binds one flat-normal texel; the realm's table then names no maps
    if (wantsDataLayers()) resources.push((layers.dataTexture || flatDataTexture(device)).createView({ dimension: "2d-array" }));
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
@fragment fn fs(o: O) -> @location(0) vec4f { return vec4f(textureSample(t, s, vec2f(o.uv.x, mix(m.look.y, m.look.z, o.uv.y))).rgb * m.look.x, 1); }`;
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
  m.texture?.destroy(); m.buffer?.destroy(); m.bandBuffer?.destroy();
}
// The media this frame wants, opened as they're first named; the ready ones drawn into pass: the
// pictures and videos under the strokes, or (over) the ones drawn over them.
function drawMedia(x, pass, w, h, depth, over = false) {
  const source = mediaSource(x, arena.state, memory, hostTransport ? hostServices.mediaSpan : null);
  const n = source?.bytes ?? 0;
  const wanted = new Set();
  if (n > 0) {
    const text = new TextDecoder().decode(source.view.slice());
    for (const line of text.split("\n").slice(0, MEDIA_MAX)) {
      const [named, url, ...b] = line.split(" ");
      const nums = b.map(Number), box = nums.slice(0, 4);
      const kind = named === "over" ? "image" : named;
      if ((kind !== "image" && kind !== "video") || !url?.startsWith("/") || url.startsWith("//") || (nums.length !== 4 && nums.length !== 7) || nums.some((v) => !Number.isFinite(v))) continue;
      const band = nums.length === 7 ? [nums[4] / 100, nums[5] / 1000, nums[6] / 1000] : null;
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
      const f = band ? 0 : Math.min(bw / m.w, bh / m.h), dw = band ? bw : m.w * f, dh = band ? bh : m.h * f, cx = x0 + (bw - dw) / 2, cy = y0 + (bh - dh) / 2;
      // a band has its own uniforms, so the same picture drawn whole under it keeps its own
      if (band && !m.bandGroup) {
        m.bandBuffer = gpu.device.createBuffer({ size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
        m.bandGroup = gpu.device.createBindGroup({ layout: gpu.mediaLayout, entries: [
          { binding: 0, resource: { buffer: m.bandBuffer } }, { binding: 1, resource: m.texture.createView() }, { binding: 2, resource: gpu.mediaSampler }] });
      }
      gpu.device.queue.writeBuffer(band ? m.bandBuffer : m.buffer, 0, new Float32Array([cx / w * 2 - 1, 1 - cy / h * 2, (cx + dw) / w * 2 - 1, 1 - (cy + dh) / h * 2, ...(band || [1, 0, 1]), 0]));
      pass.setPipeline(mediaPipeline(depth)); pass.setBindGroup(0, band ? m.bandGroup : m.group); pass.draw(6);
    }
  }
  if (!over) for (const [key, m] of media) if (!wanted.has(key) && ++m.unseen > 60) mediaClose(key, m); else if (wanted.has(key)) m.unseen = 0;
  if (n > 0 || media.size) pass.setBindGroup(0, gpu.bindGroup);
}

// ---- sounds: Web Audio, started by the first touch or key (browsers wait for one)
let audio = null, audioOut = null;
let clipTimeline = null;
let contractAudio = null, contractAudioReady = null, audioOwner = 0, audioStarted = false, audioFailure = null;
let contractAudioMemory = null, audioRetirement = Promise.resolve(), lastAudioRetirement = null;
let audioBankStatus = 0, audioPacketStatus = 0, audioOfferRefusals = 0;
const bankSounds = new Map();
function retireContractAudio() {
  ++audioOwner;const adapter=contractAudio,pending=contractAudioReady,context=audio,memory=contractAudioMemory;
  contractAudio=null;contractAudioReady=null;contractAudioMemory=null;
  audio=null;audioOut=null;banked=false;bankSounds.clear();sounds.clear();audioFailure=null;audioBankStatus=0;audioPacketStatus=0;audioOfferRefusals=0;
  if(!context)return;
  audioRetirement=audioRetirement.then(async()=>{
    let report;try{const owner=adapter??await pending;report=await owner?.close();}
    finally{if(context.state!=='closed')await context.close();}
    lastAudioRetirement=[report?.memoryBefore??0,report?.memoryAfter??memory?.total()??0,report?.nodeBefore??0,report?.nodeAfter??0,Number(context.state==='closed')];
  });
  audioRetirement.catch(error=>{audioFailure=String(error.message);say(`Audio retirement: ${audioFailure}`,true);});
}
function prepareContractAudio() {
  if(contractAudioReady || !audio)return;
  const owner=audioOwner,context=audio,services=hostServices;contractAudioMemory=openPcmMemory(services);
  contractAudioReady=openCoaxAudio(context,audioOut,async(name,canonical)=>{
    const asset=soundAsset(name);if(!asset)return null;
    if(asset.sourcePack)return packSoundSource(asset);
    if(asset.canonical)return canonical(asset.bytes);
    return asset.bytes?{opus:asset.bytes}:{url:asset.url};
  },{services,memory:contractAudioMemory}).then(adapter=>{if(owner===audioOwner)contractAudio=adapter;return adapter;}).catch(error=>{if(owner===audioOwner){audioFailure=String(error.message);say(`Audio: ${audioFailure}`);}return null;});
}
const sounds = new Map();   // name -> a decoding or decoded buffer
// A realm that renders its own sounds (metaverse-engine/audio/sfx.coax) lists them once, when audio
// starts: coaxial_sound_bank(state) and coaxial_sound_bank_len(state), a line each, NAME
// AT BYTES, a WAV file's place in the wasm's memory. They join the sounds played by name.
let banked = false;
function soundBank() {
  const x = engine?.exports;
  if (banked || !x?.coaxial_sound_bank_len) return;
  banked = true;
  const n = x.coaxial_sound_bank_len(arena.state)>>>0;
  if(!n)return;
  if(hostTransport){
    const p=x.coaxial_sound_bank?.(arena.state)>>>0,capacity=x.coaxial_sound_bank_capacity?.(arena.state)>>>0;
    const base=x.coaxial_sound_bank_storage?.(arena.state)>>>0,storage=x.coaxial_sound_bank_storage_bytes?.(arena.state)>>>0;
    if(!hostServices||!hostServices.span(p,capacity,n,memory.buffer.byteLength)||!hostServices.span(base,storage,storage,memory.buffer.byteLength)){audioBankStatus=-1;return;}
    const text=new TextDecoder().decode(new Uint8Array(memory.buffer,p,n));
    for(const line of text.split("\n")){
      const source=hostServices.bankLine(line,base,storage);
      if(!source){audioBankStatus=-1;continue;}
      const bytes=new Uint8Array(memory.buffer,source.address,source.bytes).slice();
      bankSounds.set(source.name,{i:assets.length,kind:"sound",name:source.name,bytes,done:true,canonical:true});
    }
    return;
  }
  const text = n ? new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_sound_bank(arena.state), n).slice()) : "";
  for (const line of text.split("\n")) {
    const [name, at, bytes] = line.split(" ");
    if (!name || !(+bytes > 0)) continue;
    const asset={ i: assets.length, kind: "sound", name, bytes: new Uint8Array(memory.buffer, +at, +bytes).slice(), done: true };
    if(hostTransport)bankSounds.set(name,{...asset,canonical:true});else assets.push(asset);
  }
}
function startAudio(gesture = true) {
  if(clipping&&hostTransport){soundBank();return;}
  if(gesture)audioStarted=true;
  if (audio) {if(hostTransport){prepareContractAudio();if(gesture&&!worldPaused)audio.resume().catch(()=>{});}return;}
  if (clipping && !globalThis.coaxialClipAudio) return;   // a clip is silent unless it records its sound
  soundBank();
  if (!soundDir && !bankSounds.size && !assets.some((a) => a.kind === "sound" || a.kind === "loop")) return;
  try { audio = new AudioContext(hostTransport?{sampleRate:48000}:undefined); } catch { return; }
  // checks (metaverse-engine/tests/live-audio.coax): the live graph ends in the check's tap, which
  // records the samples on the audio thread and passes them on to the (muted) speakers
  audioOut = dev && typeof globalThis.coaxialAudioTap === "function" ? globalThis.coaxialAudioTap(audio) : audio.destination;
  if(hostTransport){if(!audioStarted||worldPaused)audio.suspend().catch(()=>{});prepareContractAudio();return;}
  const loop = assets.find((a) => a.kind === "loop");
  if (loop) decoded(loop).then((b) => {
    const s = audio.createBufferSource(), g = audio.createGain();
    s.buffer = b; s.loop = true; g.gain.value = 0.35; s.connect(g).connect(audioOut); s.start();
  }).catch(() => {});
}
function decoded(a) {
  const key = a.url || `pack:${a.name}`;
  if (!sounds.has(key)) sounds.set(key, (a.bytes ? Promise.resolve(a.bytes.slice().buffer) : fetch(a.url).then((r) => r.arrayBuffer())).then((b) => audio.decodeAudioData(b)));
  return sounds.get(key);
}
// One voice: the decoded buffer, its gain, a stereo pan clamped to -1..1, into out; with a
// lowpass corner in hertz (a far sound, metaverse-engine/audio/space.coax), first a one-pole lowpass,
// y += c (x - y) with c = 1 - e^(-2 pi f / rate). The live host, a clip's mix and the
// offline checks build the same graph with this.
function soundVoice(ctx, b, gain, pan, out, lowpass) {
  const s = ctx.createBufferSource(), g = ctx.createGain(), p = ctx.createStereoPanner();
  s.buffer = b; g.gain.value = Number(gain) || 0; p.pan.value = Math.max(-1, Math.min(1, Number(pan) || 0));
  const f = Number(lowpass) || 0;
  if (f >= 1 && f < ctx.sampleRate / 2) {
    const c = 1 - Math.exp(-2 * Math.PI * f / ctx.sampleRate);
    s.connect(ctx.createIIRFilter([c], [1, c - 1])).connect(g);
  } else s.connect(g);
  g.connect(p).connect(out);
  return s;
}
// A sound by name: the page's, or DIR/NAME.ogg of its sounds:/DIR/, added when first named.
function soundAsset(name) {
  if(hostTransport&&bankSounds.has(name))return bankSounds.get(name);
  const a = assets.find((s) => s.kind === "sound" && s.name === name);
  if (a || !soundDir || !/^[a-z0-9-]{1,48}$/.test(name)) return a;
  const b = { i: assets.length, kind: "sound", name, url: `${soundDir}${name}.ogg` };
  if(!hostTransport)assets.push(b);
  return b;
}
// The mixed voices' bus (metaverse-engine/audio/mix.coax): the voices and the room's return into a gain the
// realm sets (!bus, its limiter at control rate), then a gentle compressor and a brick-wall limiter
// at -1 dBFS, into the output. The room is a convolver with the realm's impulse (!room NAME).
let bus = null;
function soundBus(ctx, out) {
  const input = ctx.createGain(), level = ctx.createGain(), comp = ctx.createDynamicsCompressor(), wall = ctx.createDynamicsCompressor();
  comp.threshold.value = -18; comp.knee.value = 12; comp.ratio.value = 3; comp.attack.value = 0.01; comp.release.value = 0.25;
  wall.threshold.value = -1; wall.knee.value = 0; wall.ratio.value = 20; wall.attack.value = 0.001; wall.release.value = 0.08;
  input.connect(level).connect(comp).connect(wall).connect(out);
  const room = ctx.createConvolver(); room.normalize = true; room.connect(input);
  return { input, level, room, roomed: false, voices: new Map() };
}
// One mixed voice: NAME GAIN PAN DELAY LOWPASS RATE WET ID, into the bus and WET of it to the room.
function busVoice(ctx, b, f, buffer, at) {
  const [, gain, pan, , lowpass, rate, wet] = f;
  const s = soundVoice(ctx, buffer, gain, pan, b.input, lowpass);
  s.playbackRate.value = Math.max(0.5, Math.min(2, Number(rate) || 1));
  const w = Math.max(0, Math.min(1, Number(wet) || 0));
  if (w > 0 && b.roomed) { const g = ctx.createGain(); g.gain.value = (Number(gain) || 0) * w; s.connect(g).connect(b.room); }
  s.start(at);
  return s;
}
// A bus line: !bus GAIN, !room NAME, !fetch NAME, or -ID (that voice fades out now).
function busLine(ctx, b, line, at, decode) {
  const [word, arg] = line.split(" ");
  if (word === "!bus") b.level.gain.setTargetAtTime(Math.max(0, Math.min(1, Number(arg) || 0)), at, 0.03);
  else if (word === "!room" || word === "!fetch") {
    const a = soundAsset(arg);
    if (a) decode(a).then((buffer) => { if (word === "!room" && !b.roomed) { b.room.buffer = buffer; b.roomed = true; } }).catch(() => {});
  } else if (word[0] === "-") {
    const v = b.voices.get(word.slice(1));
    if (v) { b.voices.delete(word.slice(1)); try { v.fade.gain.setTargetAtTime(0, at, 0.015); v.s.stop(at + 0.08); } catch {} }
  }
}
let playing = 0, mixed = 0;
function playSounds(x) {
  const n = x.coaxial_sounds_len(arena.state)>>>0;
  if(hostTransport){
    if(!n)return;
    const p=x.coaxial_sounds(arena.state)>>>0,capacity=x.coaxial_sounds_capacity?.(arena.state)>>>0;
    if(!hostServices?.span(p,capacity,n,memory.buffer.byteLength)){audioPacketStatus=-1;return;}
    audioPacketStatus=1;playSoundText(new TextDecoder().decode(new Uint8Array(memory.buffer,p,n)));return;
  }
  if (!n || !audio) return;
  playSoundText(new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_sounds(arena.state), n).slice()));
}
// The realm's lines, NAME GAIN PAN each, an optional DELAY in seconds (a far sound is
// heard after it is seen, metaverse-engine/audio/space.coax) and after it an optional LOWPASS corner
// in hertz (a far sound is dull): at most 16 voices at once. A voice holds its place from
// its line, while its sound still decodes, so a burst can't pass 16.
// A line ~NAME GAIN PAN sets a looping bed instead: started on its first line, then eased
// to each new gain and pan (gain 0 keeps it silent), outside the 16.
const beds = new Map();   // name -> { gain, pan } nodes of a looping sound
const heard = [];         // on this machine only: the last lines played, for the checks
// A mixed voice's line, NAME GAIN PAN DELAY LOWPASS RATE WET ID (metaverse-engine/audio/mix.coax), goes to the
// bus with its rate and room send, and at most 48 at once (the realm keeps far fewer); a line that
// starts ! or - is the bus's (busLine).
function deliverContractAudio(offer,owner,started){
  contractAudioReady.then(adapter=>owner===audioOwner&&adapter?.play(offer.bytes,started,true)).finally(()=>offer.release()).catch(error=>{if(owner===audioOwner)audioFailure=String(error.message);});
}
function playSoundText(text) {
  for (const line of text.split("\n")) {
    if (clipping) { clipHear(line); continue; }
    if(hostTransport){
      if(!line)continue;const offer=hostServices?.audioLine(line);if(!offer){audioOfferRefusals++;continue;}
      startAudio(false);const owner=audioOwner,started=audioStarted;
      if(dev){heard.push(new TextDecoder().decode(offer.bytes));if(heard.length>512)heard.shift();}
      if(!contractAudioReady){offer.release();continue;}
      deliverContractAudio(offer,owner,started);continue;
    }
    if (line[0] === "~") { bedLine(line.slice(1)); continue; }
    if (line[0] === "!" || line[0] === "-") { if (audio) { bus ??= soundBus(audio, audioOut); busLine(audio, bus, line, audio.currentTime, decoded); } continue; }
    const f = line.split(" ");
    if (f.length === 8) {
      const a = soundAsset(f[0]);
      if (!a || !audio || mixed >= 48) continue;
      bus ??= soundBus(audio, audioOut);
      mixed++;
      if (dev) { heard.push(line); if (heard.length > 512) heard.shift(); }
      decoded(a).then((b) => {
        const fade = audio.createGain(); fade.connect(bus.input);
        const s = busVoice(audio, { ...bus, input: fade }, f, b, audio.currentTime + Math.max(0, Math.min(2, Number(f[3]) || 0)));
        bus.voices.set(f[7], { s, fade });
        s.onended = () => { mixed--; bus.voices.delete(f[7]); };
      }).catch(() => mixed--);
      continue;
    }
    const [name, gain, pan, delay, lowpass] = line.split(" ");
    const a = assets.find((s) => s.kind === "sound" && s.name === name);
    if (!a || playing >= 16) continue;
    playing++;
    if (dev) { heard.push(line); if (heard.length > 512) heard.shift(); }
    decoded(a).then((b) => {
      const s = soundVoice(audio, b, gain, pan, audioOut, lowpass);
      s.onended = () => playing--; s.start(audio.currentTime + Math.max(0, Math.min(2, Number(delay) || 0)));
    }).catch(() => playing--);
  }
}
// A clip's sound (clip.coax --audio sets coaxialClipAudio with coaxialClip): nothing plays
// while the frames are drawn. Each line is kept with the game time of the frame that sent
// it, the 16 voices counted in that time, and coaxialClipMix renders the clip's stretch of
// them afterwards through soundVoice in an OfflineAudioContext, so the sound is in step
// with the frames however slowly they were drawn.
const clipHeard = [];   // [ms, line]
const clipVoices = [];  // when each counted voice ends, ms
let clipMark = 0;       // the game time the recording starts at, ms
function clipHear(line) {
  if (!line) return;
  if(hostTransport){clipTimeline?.offer(clipNow,line);return;}
  if (line[0] === "~" || line[0] === "!" || line[0] === "-") { clipHeard.push([clipNow, line]); return; }
  const f = line.split(" "), [name, , , delay] = f;
  const a = soundAsset(name);
  if (!a) return;
  if (f.length === 8) { clipHeard.push([clipNow, line]); return; }   // a mixed voice: the realm limits them
  for (let i = clipVoices.length - 1; i >= 0; i--) if (clipVoices[i] <= clipNow) clipVoices.splice(i, 1);
  if (clipVoices.length >= 16) return;
  clipVoices.push(clipNow + 1000 * (Math.max(0, Math.min(2, Number(delay) || 0)) + wavSeconds(a)));
  clipHeard.push([clipNow, line]);
}
// A sound's seconds from its WAV header (a realm's own: 16-bit PCM), 2 for any other.
function wavSeconds(a) {
  const b = a.bytes;
  if (!b || b.length < 44 || b[0] !== 82 || b[8] !== 87) return 2;
  const v = new DataView(b.buffer, b.byteOffset, b.byteLength), rate = v.getUint32(24, true), frame = v.getUint16(32, true);
  return rate > 0 && frame > 0 ? (b.length - 44) / frame / rate : 2;
}
async function renderSoundLines(lines, seconds, zero, rate, asset = soundAsset, decode = decoded) {
    const n = Math.round(seconds * rate), o = new OfflineAudioContext(2, n, rate);
    const out = o.createGain(); out.connect(o.destination);
    const bedsAt = new Map(), cb = soundBus(o, out);
    for (const [ms, line] of lines) {
      const at = (ms - zero) / 1000;
      if (line[0] === "!" || line[0] === "-") {
        const [word, arg] = line.split(" ");
        if (word === "!room" && !cb.roomed) { const a = asset(arg); if (a) { try { cb.room.buffer = await decode(a); cb.roomed = true; } catch {} } }
        else if (word !== "!room" && word !== "!fetch") busLine(o, cb, line, Math.max(0, at), decode);
        continue;
      }
      if (line[0] === "~") {
        const [name, gain, pan] = line.slice(1).split(" ");
        const a = asset(name);
        if (!a) continue;
        let b = bedsAt.get(name);
        const g = Math.max(0, Math.min(1, Number(gain) || 0)), p = Math.max(-1, Math.min(1, Number(pan) || 0));
        if (!b) {
          b = { gain: o.createGain(), pan: o.createStereoPanner() }; bedsAt.set(name, b);
          b.gain.gain.value = 0; b.pan.pan.value = p; b.gain.connect(b.pan).connect(out);
          const s = o.createBufferSource(); s.buffer = await decode(a); s.loop = true; s.connect(b.gain); s.start(0, Math.max(0, -at) % s.buffer.duration);
        }
        // as bedLine eases them: a first line over 0.5 s, the next over 0.25 s
        const tc = b.started ? 0.25 : 0.5; b.started = true;
        b.gain.gain.setTargetAtTime(g, Math.max(0, at), at < 0 ? 0.001 : tc); b.pan.pan.setTargetAtTime(p, Math.max(0, at), at < 0 ? 0.001 : 0.25);
        continue;
      }
      const [name, gain, pan, delay, lowpass] = line.split(" ");
      const a = asset(name);
      if (!a) continue;
      const b = await decode(a), start = at + Math.max(0, Math.min(2, Number(delay) || 0));
      if (start + b.duration <= 0 || start >= seconds) continue;
      const f = line.split(" ");
      if (f.length === 8) {
        if (start < 0) continue;
        const fade = o.createGain(); fade.connect(cb.input);
        cb.voices.set(f[7], { s: busVoice(o, { ...cb, input: fade }, f, b, start), fade });
        continue;
      }
      const s = soundVoice(o, b, gain, pan, out, lowpass);
      if (start >= 0) s.start(start); else s.start(0, -start);
    }
    return await o.startRendering();
}
if (globalThis.coaxialClip === true) {
  // the recording starts after the frame drawn now: its first frame is the next step's
  globalThis.coaxialClipMark = () => { clipMark = clipNow; return hostTransport?!!clipTimeline:!!audio; };
  // The same offline graph used by conformance, serialized as the clip's WAV.
  globalThis.coaxialClipMix = async (seconds, fps, rate = 48000) => {
    const timeline=clipTimeline,services=hostServices,contract=!!hostTransport;
    if (!contract&&!audio) return "";
    let capture,failure,cleanup;const offered=timeline?.info();
    try {
    if(contract&&rate!==48000)throw Error('host contract v1 clips require48kHz');
    const zero = clipMark + 1000 / fps;
    const plan=contract?timeline?.seal(zero,seconds):null;
    if(contract&&!plan)throw Error('clip owner unavailable');
    capture=contract?await renderClipAudio(timeline,plan,async(name,canonical)=>{const asset=soundAsset(name);if(!asset)return null;return asset.canonical?canonical(asset.bytes):asset.bytes?{opus:asset.bytes}:{url:asset.url};},services):null;
    const n = contract?plan.frames:Math.round(seconds * rate);
    const r = capture?capture.buffer:await renderSoundLines(clipHeard, seconds, zero, rate), l = r.getChannelData(0), ri = r.getChannelData(1);
    let peak = 0, sum = 0;
    const bytes = new Uint8Array(44 + n * 4), v = new DataView(bytes.buffer);
    const tag = (at, t) => { for (let i = 0; i < 4; i++) v.setUint8(at + i, t.charCodeAt(i)); };
    tag(0, "RIFF"); v.setUint32(4, 36 + n * 4, true); tag(8, "WAVE"); tag(12, "fmt "); v.setUint32(16, 16, true);
    v.setUint16(20, 1, true); v.setUint16(22, 2, true); v.setUint32(24, rate, true); v.setUint32(28, rate * 4, true);
    v.setUint16(32, 4, true); v.setUint16(34, 16, true); tag(36, "data"); v.setUint32(40, n * 4, true);
    for (let i = 0; i < n; i++) for (const [c, x] of [[0, l[i]], [1, ri[i]]]) {
      peak = Math.max(peak, Math.abs(x)); sum += x * x;
      v.setInt16(44 + i * 4 + c * 2, Math.round(Math.max(-1, Math.min(1, x)) * 32767), true);
    }
    const db = (x) => x > 0 ? (20 * Math.log10(x)).toFixed(1) : "-inf";
    globalThis.clipAudioFacts = `${capture?capture.receipt.offered.commands:clipHeard.filter(([ms]) => ms >= zero - 2000 && ms < zero + seconds * 1000).length} lines, peak ${db(peak)} dBFS, RMS ${db(Math.sqrt(sum / (2 * n)))} dBFS`;
    if(capture)globalThis.coaxialClipReceipt=capture.receipt;
    let text = "";
    for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(text);
    } catch(error){failure=String(error.message);cleanup=error.coaxClipMemory;throw error;}
    finally {if(contract){capture?.release();const beforeRetire=timeline?.info();await timeline?.retire();const receipt=capture?.receipt??{version:1,offered,failure,memoryAfter:cleanup?.assets??services.words(),outputMemoryAfter:cleanup?.output??0};receipt.beforeRetire=beforeRetire;receipt.timelineAfter=timeline?.info();globalThis.coaxialClipReceipt=receipt;}}
  };
}
function bedLine(line) {
  const [name, gain, pan] = line.split(" ");
  const g = Math.max(0, Math.min(1, Number(gain) || 0)), p = Math.max(-1, Math.min(1, Number(pan) || 0));
  const bed = beds.get(name);
  if (bed) { bed.gain.gain.setTargetAtTime(g, audio.currentTime, 0.25); bed.pan.pan.setTargetAtTime(p, audio.currentTime, 0.25); return; }
  const a = soundAsset(name);
  if (!a) return;
  const nodes = { gain: audio.createGain(), pan: audio.createStereoPanner() };
  nodes.gain.gain.value = 0; nodes.pan.pan.value = p;
  beds.set(name, nodes);
  nodes.gain.connect(nodes.pan).connect(audioOut);
  nodes.gain.gain.setTargetAtTime(g, audio.currentTime, 0.5);
  decoded(a).then((b) => { const s = audio.createBufferSource(); s.buffer = b; s.loop = true; s.connect(nodes.gain); s.start(); }).catch(() => {});
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
// one and coaxial_quality(state, 0|1|2|3) tells the app the one in force; kept in localStorage
// verse.quality. AUTO (3) caps nothing: the app's governor sets its pixels (coaxial_pixel_budget,
// metaverse-engine/core/quality.coax). Goldens always draw HIGH.
const QUALITY = ["high", "medium", "low", "auto"], QUALITY_CAP = [Infinity, 1.5, 1, Infinity];
let quality = (() => {
  try { const q = QUALITY.indexOf(localStorage.getItem("verse.quality")); if (q >= 0) return q; } catch {}
  return matchMedia("(pointer: coarse)").matches ? 1 : 0;
})();
// The graphics tier this browser kept (verse.quality), or "" when none.
function storedQuality() { try { const q = localStorage.getItem("verse.quality"); return QUALITY.includes(q) ? q : ""; } catch { return ""; } }
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
// A page may ask to be drawn at 2 or 3 times its device pixels (data-supersample="2"): the browser
// scales the canvas down, so every pixel shown is the average of 4 (or 9) drawn, and thin lines and
// edges stay smooth. For staged clips (demos/grid-crowd); no other page asks.
const SUPERSAMPLE = Math.min(3, Math.max(1, Math.round(Number(script?.dataset.supersample) || 1)));
// Device pixels per CSS pixel, as the canvas is actually drawn.
const scale = () => (canvas.width && canvas.clientWidth ? canvas.width / canvas.clientWidth : devicePixelRatio || 1);

// ---- the frame
// Clip mode (clip.coax sets coaxialClip before the page loads): the page draws only when
// coaxialClipStep says so, each frame one fixed step of game time, so a recording has
// every frame at its exact moment however slowly the machine draws or captures.
const clipping = globalThis.coaxialClip === true || agentMode;
// A clip that records a local room (room-worker.js) keeps the page's clock and the room's on the clip's.
const clipRoom = clipping && (globalThis.coaxialClipRoom === true || agentMode), clipStart = agentMode ? 1000 : performance.now();
// Optional platform timing observations for the desktop profiler. No work is
// collected in an ordinary page; Coax tools summarize these raw milliseconds.
// ?frametrace turns the trace on by hand on a development server (coaxial.takeFrameTrace() reads it: row[1] the engine's ms, row[2] the host's encode, row[4] both).
const frameTrace = globalThis.coaxialFrameTrace === true || (dev && new URLSearchParams(location.search).has("frametrace")) ? [] : null;
// Pass times (globalThis.coaxialPassTimer = true before the page loads; coax/web/flyover.coax): every
// render pass of a frame gets its own pair of GPU timestamps, read back after the submit, and
// coaxial.takePassTimes() gives [SPAN_US, SHADOW_US, PASSES, SUM_US] a frame: the first pass's start to
// the last pass's end, the shadow map's pass (no colour targets), and every pass's own time added up. At most 600 frames are kept.
const passTimes = globalThis.coaxialPassTimer === true ? [] : null;
const PASS_SLOTS = 32;
function passTimerFor(device) {
  if (!passTimes || !device.features.has("timestamp-query")) return null;
  if (!device.__coaxialPassTimer) device.__coaxialPassTimer = { set: device.createQuerySet({ type: "timestamp", count: PASS_SLOTS }),
    resolve: device.createBuffer({ size: PASS_SLOTS * 8, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
    reads: [0, 1, 2, 3].map(() => ({ buffer: device.createBuffer({ size: PASS_SLOTS * 8, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }), busy: false })) };
  const t = device.__coaxialPassTimer, read = t.reads.find((r) => !r.busy);
  return read ? { t, read, n: 0, shadow: [] } : null;
}
// A pass described by d, timed when this frame's timer has room (and d isn't timed already).
function timedPass(encoder, timer, d) {
  if (!timer || d.timestampWrites || timer.n + 2 > PASS_SLOTS) return encoder.beginRenderPass(d);
  if (!d.colorAttachments?.length) timer.shadow.push(timer.n);
  const at = timer.n; timer.n += 2;
  return encoder.beginRenderPass({ ...d, timestampWrites: { querySet: timer.t.set, beginningOfPassWriteIndex: at, endOfPassWriteIndex: at + 1 } });
}
function passTimerResolve(encoder, timer) {
  if (!timer || !timer.n) return;
  encoder.resolveQuerySet(timer.t.set, 0, timer.n, timer.t.resolve, 0);
  encoder.copyBufferToBuffer(timer.t.resolve, 0, timer.read.buffer, 0, timer.n * 8);
}
function passTimerRead(timer) {
  if (!timer || !timer.n) return;
  const { read, n, shadow } = timer; read.busy = true;
  read.buffer.mapAsync(GPUMapMode.READ, 0, n * 8).then(() => {
    const t = new BigUint64Array(read.buffer.getMappedRange(0, n * 8).slice(0)); read.buffer.unmap(); read.busy = false;
    let first = t[0], last = t[1], shade = 0n, sum = 0n;
    for (let i = 0; i < n; i += 2) { if (t[i + 1] < t[i]) return; if (t[i] < first) first = t[i]; if (t[i + 1] > last) last = t[i + 1]; sum += t[i + 1] - t[i]; }
    for (const i of shadow) shade += t[i + 1] - t[i];
    passTimes.push([Math.round(Number(last - first) / 1000), Math.round(Number(shade) / 1000), n / 2, Math.round(Number(sum) / 1000)]);
    if (passTimes.length > 600) passTimes.shift();
  }, () => { read.busy = false; });
}
let traceRow = null, traceLast = 0;
function finishFrameTrace(observedStart) {
  if (!traceRow) return;
  traceRow[14] = performance.now();
  traceRow[4] = traceRow[14] - observedStart;
  if (traceRow[5] && globalThis.coaxial.sceneReady) {
    frameTrace.push(traceRow);
    if (frameTrace.length > 600) frameTrace.shift();
  }
  traceRow = null;
}
let clipNow = 0;
let worldPaused = false, frameRequest = 0, worldFrames = 0, worldSubmissions = 0, shadowTilesDrawn = 0;
function setWorldPaused(paused) {
  worldPaused = !!paused;
  hostLifeEvent(worldPaused?4:5);
  if (worldPaused) {
    cancelAnimationFrame(frameRequest); frameRequest = 0;
    audio?.suspend().catch(() => {});
  } else {
    traceLast = 0;
    if (!frameRequest) frameRequest = requestAnimationFrame(frame);
    if(!hostTransport||audioStarted)audio?.resume().catch(() => {});
  }
  return worldPaused;
}
if (clipping) globalThis.coaxialClipStep = async (frames = 1, fps = 30) => {
  // a clip that records its sound starts the audio once the engine is up (no gesture needed: nothing plays)
  if (globalThis.coaxialClipAudio && engine) {if(hostTransport)soundBank();else if(!audio)startAudio();}
  for (let k = 0; k < frames; k++) {
    clipNow += 1000 / fps; agentMs = clipNow;
    // the room to the same moment, and every frame it sent then in the app before it draws
    if (clipRoom && globalThis.coaxial?.roomHost?.advance) { await globalThis.coaxial.roomHost.advance(clipNow); if (engine) drainSockets(1e9); }
    const observedStart = frameTrace ? performance.now() : 0;
    if (frameTrace) {
      traceRow = [traceLast ? observedStart-traceLast : 0,0,0,0,0,0,0,-1,0,0,0];
      traceLast = observedStart;
    }
    // Keep actual submission/world-draw facts in fixed-step diagnostics too.
    // The trace interval is wall time, never the synthetic 1000/fps step.
    try { draw(clipNow); a11yMirror(clipNow); } finally { finishFrameTrace(observedStart); }
  }
  if (gpu?.device) await gpu.device.queue.onSubmittedWorkDone();
  return engine && gpu?.bg ? 1 : 0;
};
function frame(now) {
  frameRequest = 0;
  if (worldPaused) return;
  frameRequest = requestAnimationFrame(frame);
  // an agent's page loads on frames a microsecond apart, until its clock first steps
  if (agentMode && !agentHeld && !agentLoading) { agentLoading = true; agentQueue = agentQueue.then(() => agentHeld ? 0 : globalThis.coaxialClipStep(1, 1e6)).catch(() => 0).finally(() => { agentLoading = false; }); }
  if (clipping) return;
  worldFrames++;
  const observedStart = frameTrace ? performance.now() : 0;
  if (frameTrace) { traceRow = [traceLast ? now - traceLast : 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0];traceLast = now; }
  if (engine && (socketQueues?.hasInput() || socketHead < socketQueue.length)) drainSockets(Math.max(2, SOCKET_BUDGET_MS - frameCost));
  padPoll();
  const t0 = performance.now();
  if (traceRow) traceRow[3] = t0 - observedStart;
  try { draw(now); } finally { frameCost = 0.9 * frameCost + 0.1 * (performance.now() - t0); }
  a11yMirror(now);
  finishFrameTrace(observedStart);
}

// A controller (a module that exports coaxial_pad): the first pad's left stick (x right, y
// forward), its first button, and every button as bits (bit b for button b), once a frame; the
// module decides what they do (myrealm: metaverse-engine/core/input.coax; Everglade: its settings record,
// everglade-access.coax). A pad gone sends one still frame.
let padLive = false;
function padPoll() {
  const x = engine?.exports;
  if (!x?.coaxial_pad || !navigator.getGamepads) return;
  const pad = [...navigator.getGamepads()].find((g) => g && g.connected);
  if (!pad && !padLive) return;
  padLive = !!pad;
  let bits = 0;
  if (pad) for (let i = 0; i < Math.min(31, pad.buttons.length); i++) if (pad.buttons[i]?.pressed) bits |= 1 << i;
  x.coaxial_pad(arena.state, pad ? Number(pad.axes[0]) || 0 : 0, pad ? -(Number(pad.axes[1]) || 0) : 0, pad?.buttons[0]?.pressed ? 1 : 0, bits);
}

// The accessible layer (Metaverse Engine book chapter 85): a module that describes its controls
// (coaxial_a11y: a line each, "ID ROLE STATE LABEL") gets them as real buttons and checkboxes
// beside the canvas, which stays aria-hidden, and its note as a polite live region. Tab reaches
// them; Enter or Space presses one, which sends its id back (coaxial_a11y_do): the module does
// what a tap on it does. Hidden until something in it has focus. The host only mirrors.
const a11y = { root: null, status: null, text: null, at: 0, nodes: new Map() };
function a11yMirror(now) {
  const x = engine?.exports;
  if (!x?.coaxial_a11y || now - a11y.at < 150) return;
  a11y.at = now;
  const n = x.coaxial_a11y(arena.state);
  const text = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_a11y_buf(), Math.max(0, n)).slice());
  if (text === a11y.text) return;
  a11y.text = text;
  if (!a11y.root) {
    // Screen-reader text clipped out of sight, like <main> (INVARIANTS.md "Coaxial pages draw
    // everything in Coax"): the canvas draws what a sighted visitor sees, focus included.
    const hide = "position:fixed;left:0;top:0;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap";
    a11y.root = Object.assign(document.createElement("div"), { id: "coaxial-a11y" });   // accessible layer
    a11y.root.setAttribute("style", hide);
    a11y.root.setAttribute("role", "toolbar"); a11y.root.setAttribute("aria-label", "Realm controls");
    a11y.root.addEventListener("focusin", (e) => {
      const b = e.target.closest?.("button[data-id]");
      if (b) engine?.exports.coaxial_a11y_focus?.(arena.state, Number(b.dataset.id));
    });
    a11y.root.addEventListener("focusout", () => engine?.exports.coaxial_a11y_focus?.(arena.state, -1));
    a11y.root.addEventListener("click", (e) => {
      const b = e.target.closest?.("button[data-id]");
      if (!b || !engine?.exports.coaxial_a11y_do) return;
      engine.exports.coaxial_a11y_do(arena.state, Number(b.dataset.id));
      a11y.at = 0; after(true);
    });
    a11y.status = Object.assign(document.createElement("div"), { id: "coaxial-a11y-status" });   // accessible layer
    a11y.status.setAttribute("style", hide);
    a11y.status.setAttribute("role", "status"); a11y.status.setAttribute("aria-live", "polite");
    document.body.prepend(a11y.root, a11y.status);   // first in the tab order
  }
  const seen = new Set(), order = [];
  let say = "";
  for (const line of text.split("\n")) {
    const m = /^(\d+) (button|checkbox|status) ([01-]) (.*)$/.exec(line);
    if (!m) continue;
    const [, id, role, state, label] = m;
    if (role === "status") { say = label; continue; }
    seen.add(id);
    let b = a11y.nodes.get(id);
    if (!b) { b = Object.assign(document.createElement("button"), { type: "button" }); b.dataset.id = id; a11y.nodes.set(id, b); }   // accessible layer
    if (role === "checkbox") { b.setAttribute("role", "checkbox"); b.setAttribute("aria-checked", state === "1" ? "true" : "false"); b.removeAttribute("aria-pressed"); }
    else { b.removeAttribute("role"); b.removeAttribute("aria-checked"); if (state === "-") b.removeAttribute("aria-pressed"); else b.setAttribute("aria-pressed", state === "1" ? "true" : "false"); }
    if (b.textContent !== label) b.textContent = label;
    order.push(b);
  }
  for (const [id, b] of a11y.nodes) if (!seen.has(id)) {
    const had = document.activeElement === b;
    b.remove(); a11y.nodes.delete(id);
    if (had) (order[0] ?? a11y.root.querySelector("button"))?.focus();   // focus stays in the layer
  }
  // In the module's order, moving only what is out of place: a control moved in the DOM loses
  // focus, so the one being pressed, which stays where it was, keeps it (editor-access checks).
  let at = a11y.root.firstChild;
  for (const b of order) { if (b === at) at = at.nextSibling; else a11y.root.insertBefore(b, at); }
  if (a11y.status.textContent !== say) a11y.status.textContent = say;
}
let frameCost = 0;   // ms of main thread the drawing itself takes, smoothed: socket events get what's left
// ---- the frame plan's numbers (metaverse-engine/render/passes.coax's enums, as metaverse-engine/host/frame-plan.h
// has them for C hosts; coax/web/frame-plan.test.ts holds these to that header).
const FP = { magic: 0x31504641, version: 1,
  op: { begin: 0, end: 1, draw: 2, copy: 3, each_begin: 4, each_end: 5, media_before: 6, media_after: 7, cull_early: 8, cull_late: 9, viewport: 10, scissor: 11, particle_sort: 12 },
  pass: { background: 0, shadow: 1, caster: 2, grass: 3, surface: 4, scene: 5, water_transmit: 6, water_emit: 7, particle_over: 8, particle: 9, bloom: 10, post: 11, hud: 12,
    shadow_clear: 14, character_cast: 15, character: 16, scene_late: 17 },
  target: { none: 0, screen: 1, light: 2, aux: 3, scene_depth: 4, shadow_map: 5, water_light: 6, water_depth: 7, q0: 8, q1: 9, e0: 10, e1: 11 },
  source: { vertices: 0, records: 1, current: 2, shadow_records: 3, late: 4 } };
// ?plan=off draws a page that has a plan in the built-in order; ?plan=check (or coaxialPlanCheck) records both
// orders each frame and counts the frames whose GPU calls differ (coaxial.framePlan()). Development and tests only.
const planAsk = new URLSearchParams(location.search).get("plan");
const planMode = globalThis.coaxialPlanCheck === true ? "check" : globalThis.coaxialPlan === "off" ? "off" : dev && (planAsk === "off" || planAsk === "check") ? planAsk : "on";
const planStats = { mode: planMode, frames: 0, ops: 0, refused: 0, checked: 0, differing: 0, first: null, calls: 0 };
const planIds = new WeakMap(); let planIdNext = 1;
const planId = (o) => { if (o === null || typeof o !== "object") return o; if (o.__planTexture) o = o.__planTexture; let id = planIds.get(o); if (!id) { id = planIdNext++; planIds.set(o, id); } return `#${id}`; };
if (planMode === "check" && globalThis.GPUTexture) {
  // a view is named by its texture, so the two recordings agree only when they draw into the same textures
  const make = GPUTexture.prototype.createView;
  GPUTexture.prototype.createView = function (...a) { const v = make.apply(this, a); try { v.__planTexture = this; } catch {} return v; };
}
// An encoder that records its calls and its passes' (and makes them on `real`, when there is one).
function planRecorder(real) {
  const log = [];
  const word = (v) => Array.isArray(v) ? `[${v.map(word)}]` : v && typeof v === "object" && Object.getPrototypeOf(v) === Object.prototype
    ? `{${Object.keys(v).filter((k) => k !== "timestampWrites" && k !== "clearValue").sort().map((k) => `${k}:${word(v[k])}`)}}` : String(planId(v ?? null));
  const wrap = (target, name) => new Proxy({}, { get: (_, key) => (...args) => {
    log.push(`${name}.${String(key)}(${args.map(word)})`);
    const made = target ? target[key](...args) : undefined;
    return key === "beginRenderPass" || key === "beginComputePass" ? wrap(made, key === "beginRenderPass" ? "pass" : "compute") : made;
  } });
  return { log, encoder: wrap(real, "encoder") };
}
function planCompare(a, b) {
  planStats.checked++; planStats.calls = b.length;
  let at = 0; while (at < a.length && at < b.length && a[at] === b[at]) at++;
  if (at === a.length && at === b.length) return;
  planStats.differing++;
  planStats.first ||= { frame: planStats.checked, at, builtin: a.slice(Math.max(0, at - 2), at + 3), plan: b.slice(Math.max(0, at - 2), at + 3), lengths: [a.length, b.length] };
}
// The bytes the scene's stores were sent: each store's on the last frame, and every frame's together.
const storeUploads = { frames: 0, total: 0, last: new Array(16).fill(0) };
function draw(now) {
  if (engineInstalling || !engine || !gpu?.bg || recovering) return;
  releaseWaiting();   // once this frame, which draws the state as the loss found it, is done
  const [dw, dh] = devicePixels || [Math.max(1, Math.round(canvas.clientWidth * devicePixelRatio)), Math.max(1, Math.round(canvas.clientHeight * devicePixelRatio))];
  // Optional sampling v1: the Coax realm declares its integer supersampling scale.
  // The existing pixel-budget path bounds the source raster before this SDK resize.
  const sampling = Math.min(3, Math.max(1, engine.exports.coaxial_sampling_v1?.(arena.state) || SUPERSAMPLE));
  const f = drawn(dw) * sampling, w = Math.max(1, Math.round(dw * f)), h = Math.max(1, Math.round(dh * f));
  const pending = gpu.device.__coaxialFrames || 0;
  if (traceRow) {traceRow[5]=w;traceRow[6]=h;traceRow[10]=pending;}
  if (gpu.renderer && !gpu.renderer.available()) return;
  if (!gpu.renderer && engine.exports.coaxial_frame_admit && !engine.exports.coaxial_frame_admit(pending)) {
    if(traceRow){traceRow[7]=0;traceRow[8]=gpu.device.__coaxialCompletionMs||0;}return;
  }
  const offer=hostLifecycle?.offer(Math.floor(now*1000),pending);
  if(hostLifecycle&&!offer){if(traceRow){traceRow[7]=0;traceRow[8]=gpu.device.__coaxialCompletionMs||0;}return;}
  const measured = offer?offer.dt:(last ? (now - last) / 1000 : 0); last = now;
  const dt = golden ? goldenStep(goldenScene, measured) : measured;
  if (canvas.width !== w || canvas.height !== h) {hostLifeEvent(1,Math.floor(now*1000));canvas.width = w; canvas.height = h; }
  const dpr = scale();
  if (traceRow) {traceRow[5]=w;traceRow[6]=h;}
  let n = 0;
  frameStart();   // a rescue save put back, then this frame's start kept (a trap saves it)
  const trap = crashTest && trapArmed;trapArmed = false;
  try {
    for (let k = 0; probeAt && !gpu.probed && k < probeAt * 10; k++)
      engine.exports.coaxial_frame(arena.state, arena.vertices, arena.uniforms, w, h, dpr, 0.1, 0);
    if (probeDo && !gpu.probed) runProbe(w, h, dpr);
    const engineStart = traceRow ? performance.now() : 0;
    n = engine.exports.coaxial_frame(trap ? -16 : arena.state, arena.vertices, arena.uniforms, w, h, dpr, dt, still ? 1 : 0);
    if (traceRow) {traceRow[1] = performance.now() - engineStart;for(let i=0;i<3;i++)traceRow[11+i]=engine.exports.coaxial_frame_parts?.(arena.state,i)||0;}
    editorStep(); paneStep(); scriptStep();
    if (gpu.gpuiOn) gpuiFrame(dpr);
  } catch (e) { trapped(e); return; }
  if (gpu.renderer) {
    try {
      const encodeStart = traceRow ? performance.now() : 0;
      const submitted = gpu.renderer.frame(n, w, h, captureRequested && probeArmed && ++frames >= 1 && !gpu.probed ? (encoder, texture) => readBack(encoder, texture) : null);
      if (submitted) {worldSubmissions++;roomTraceSubmitted(gpu.device.queue);}
      if (traceRow) {traceRow[2]=performance.now()-encodeStart;traceRow[7]=submitted?1:0;traceRow[9]=gpu.renderer.buffers?.reduce((n,r)=>n+(r.image?r.bytes:0),0)||0;}
      if (submitted && gpu.renderer.readiness === gpu.renderer.ids.readiness.ready) globalThis.coaxial.sceneReady = true;
      if (probe || dev) globalThis.coaxial.renderStatus = { submitted, readiness: gpu.renderer.readiness, refusal: gpu.renderer.lastRefusal, error: gpu.renderer.error?.message, captureArmed: probeArmed, observed: !!gpu.renderer.observation, probed: gpu.probed };
      if (gpu.snap) { const take = gpu.snap;gpu.snap = null;take(); }
      document.documentElement.classList.add("gpu");
      if (loading.on) loading.frame(globalThis.coaxial.sceneReady);
      if (gpu.renderer.observation) report(gpu.renderer.observation, gpu.renderer.lastTexture);
      if (!hostTransport&&engine.exports.coaxial_sounds) playSounds(engine.exports);
      after(false);
    } catch (e) { say(`${app} renderer: ${e.message}`, true);loading.fail(`RENDERER STOPPED: ${e.message}`);lifeBeacon("crash", `RENDERER STOPPED: ${e.message}`);gpu.bg = null; }
    return;
  }
  n = Math.min(n, arena.maxVertices);
  const legacyEncodeStart = traceRow ? performance.now() : 0;
  const { device, context } = gpu;
  const x = engine.exports;
  device.queue.writeBuffer(gpu.uniformBuffer, 0, new Float32Array(memory.buffer, arena.uniforms, arena.uniformFloats));
  frameUpload(x);
  if (n) device.queue.writeBuffer(gpu.vertexBuffer, 0, new Float32Array(memory.buffer, arena.vertices, n * arena.floats));
  const draws = gpu.scene && gpu.sceneGroup ? x.coaxial_draw_count(arena.state) : 0;
  // Optional shadow-list v1: Coax selects each pass; older producers retain their shared list.
  const separateShadow = !!(x.coaxial_shadow_draws_v1 && x.coaxial_shadow_draw_count_v1);
  const shadowDraws = gpu.scene && gpu.sceneGroup && separateShadow ? x.coaxial_shadow_draw_count_v1(arena.state) : draws;
  // what this frame sends the scene's stores (coaxial.storeUploads(): the retained scene's measure, #13303)
  storeUploads.frames++; storeUploads.last.fill(0);
  if (draws || shadowDraws) gpu.stores.forEach((buffer, k) => {
    const version = x.coaxial_store_version?.(arena.state, k);
    if (version !== undefined && gpu.storeVersions[k] === version) return;
    const bytes = Math.min(x.coaxial_store_bytes(arena.state, k), buffer.size) & ~3;
    if (bytes) { device.queue.writeBuffer(buffer, 0, memory.buffer, x.coaxial_store(arena.state, k), bytes); gpu.storeVersions[k] = version; storeUploads.last[k] = bytes; storeUploads.total += bytes; }
  });
  if ((gpu.scene || gpu.surface) && (!gpu.depth || gpu.depth.width !== w || gpu.depth.height !== h || !!gpu.post !== !!gpu.postGroup
      || (gpu.post && !!gpu.waterCopy !== !!gpu.waterCopyGroup))) {
    if (gpu.post) postTargets(w, h);
    else {
      gpu.depth?.destroy(); gpu.postGroup = null;
      gpu.depth = device.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | (gpu.cull ? GPUTextureUsage.TEXTURE_BINDING : 0) });
    }
  }
  const post = gpu.post && gpu.postGroup;
  const encoder = device.createCommandEncoder();
  const passTimer = passTimerFor(device);
  // the instance cull's first phase (instance-cull.js), staged here and encoded before every pass; null when it culls nothing
  const culled = gpu.cull && draws && gpu.depth && x.coaxial_cull_v1 ? gpu.cull.want(x, arena.state, memory,
    { index: gpu.index, vertices: assets.find((a) => a.kind === "storage")?.buffer, store: gpu.stores?.[x.coaxial_cull_v1(arena.state, 5)], items: gpu.stores?.[x.coaxial_cull_v1(arena.state, 8)] }, gpu.depth, w, h) : null;
  const list = draws ? new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), draws * 4) : null;
  const shadowList = separateShadow && shadowDraws ? new Uint32Array(memory.buffer, x.coaxial_shadow_draws_v1(arena.state), shadowDraws * 4) : list;
  const wet = draws && gpu.waterTransmit && x.coaxial_water_draws ? Math.min(x.coaxial_water_draws(arena.state), draws) : 0;
  // a draw whose base instance has its top bit set draws its vertices in order, with no index buffer
  let observedWorldDraws = 0;
  const drawOne = (pass, i, source = list, shadow = false) => {
    const [first, count, base, instances] = [source[i * 4], source[i * 4 + 1], source[i * 4 + 2], source[i * 4 + 3]];
    if (!count || !instances) return;
    if (!shadow) observedWorldDraws++;
    if (culled && !shadow && source === list && gpu.cull.draw(pass, i)) return;
    if (base & 0x80000000) pass.draw(count, instances, first, base & 0x7fffffff);
    else pass.drawIndexed(count, instances, x.coaxial_draw_index_v1 ? x.coaxial_draw_index_v1(arena.state, i, shadow ? 1 : 0, 1) : first, 0, base);
  };
  const drawScene = (pass, colours = false, source = list, count = draws - wet, shadow = false) => {
    pass.setBindGroup(1, gpu.sceneGroup); if (gpu.index) pass.setIndexBuffer(gpu.index, "uint32");   // none: in-order draws only
    let indexBank = -1;
    for (let i = 0; i < count; i++) {
      if (x.coaxial_draw_index_v1 && !(source[i * 4 + 2] & 0x80000000)) {
        const bank = x.coaxial_draw_index_v1(arena.state, i, shadow ? 1 : 0, 0);
        if (bank !== -1 && (bank < 0 || bank >= assets.length || !assets[bank].buffer || !x.coaxial_asset_index_v1?.(bank))) throw Error(`invalid Coax index asset v1: ${bank}`);
        if (bank !== indexBank) {pass.setIndexBuffer(bank === -1 ? gpu.index : assets[bank].buffer, "uint32");indexBank = bank;}
      }
      if (colours && x.coaxial_draw_blend_v1) {
        const profile = x.coaxial_draw_blend_v1(arena.state, i);
        if (profile !== 0 && profile !== 1 && profile !== 2) throw Error(`invalid Coax scene blend v1: ${profile}`);
        pass.setPipeline(profile === 1 ? gpu.sceneAdditive : profile === 2 ? gpu.sceneAlpha : gpu.scene);
      }
      drawOne(pass, i, source, shadow);
    }
  };
  const cast = gpu.caster && x.coaxial_caster_instances ? [x.coaxial_caster_vertices(arena.state), x.coaxial_caster_instances(arena.state)] : [0, 0];
  // the character pass (character-host.js): a detailed character the world places, in its passes
  const hero = draws && gpu.shadow ? characters.want(x, arena.state, memory, now) : false;
  const hudVertices = n;
  let timed = false;
  if (gpu.shadow && gpu.shadowLayout) shadowTexture();
  // The shadow tiles (metaverse-engine/render/cascades.coax): open them once the atlas is new or the engine
  // has none yet; then this frame draws at most the one tile the engine names, cleared, into its rect.
  let tile = -1;
  if (gpu.shadow && gpu.shadowTiles) {
    const st = arena.state;
    if (gpu.shadowFresh || x.coaxial_shadow_v1(st, 6) <= 0) { x.coaxial_shadow_event_v1(st, 0, gpu.shadowCoarse); gpu.shadowFresh = false; }
    tile = x.coaxial_shadow_v1(st, 0);
    if (tile >= 0 && !shadowDraws) { x.coaxial_shadow_event_v1(st, 2, 0); tile = -1; }   // nothing to draw it with: empty again
  }
  const target = (view) => ({ view, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } });
  const texture = context.getCurrentTexture();
  const screen = () => target(texture.createView());
  const blades = gpu.grass && (gpu.caster || gpu.sceneGroup) ? x.coaxial_grass_instances(arena.state) : 0;
  const grassInto = (pass) => {
    if (!blades) return;
    if (!gpu.caster) { pass.setBindGroup(1, gpu.sceneGroup); if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup); }
    pass.setPipeline(gpu.grass);
    if (x.coaxial_grass_range_count_v1 && x.coaxial_grass_range_v1) {
      const count = x.coaxial_grass_range_count_v1(arena.state);
      for (let i = 0; i < count; i++) pass.draw(3, x.coaxial_grass_range_v1(arena.state, i, 1), 0, x.coaxial_grass_range_v1(arena.state, i, 0));
    } else pass.draw(3, blades);
  };
  // The particles' rows, uploaded when they changed; false when there is nothing to draw them with.
  const sparksInto = (count) => {
    const group = count ? particleGroupNow() : null;
    const bytes = group && (!gpu.particleDepth || gpu.waterCopyGroup) ? Math.min(x.coaxial_particle_bytes(arena.state), gpu.particleBuffer.size) & ~3 : 0;
    if (!bytes) return null;
    const version = x.coaxial_particle_version?.(arena.state);
    if (version === undefined || version !== gpu.particleVersion) {
      device.queue.writeBuffer(gpu.particleBuffer, 0, memory.buffer, x.coaxial_particles(arena.state), bytes); gpu.particleVersion = version;
    }
    return group;
  };
  // ---- the frame's plan (metaverse-engine/render/passes.coax, #13303). A page whose engine exports
  // coaxial_frame_plan_v1 is drawn by walking the engine's operations: this host maps a pass to its
  // pipeline, bind groups and targets and decides no order. `real` false is a recording only.
  const walk = (encoder, passTimer, words, count, real) => {
    const views = { [FP.target.screen]: () => texture.createView(), [FP.target.light]: () => gpu.light.createView(), [FP.target.aux]: () => gpu.aux.createView(),
      [FP.target.q0]: () => gpu.postTextures[2].createView(), [FP.target.q1]: () => gpu.postTextures[3].createView(),
      [FP.target.e0]: () => gpu.postTextures[4].createView(), [FP.target.e1]: () => gpu.postTextures[5].createView() };
    const textures = { [FP.target.light]: () => gpu.light, [FP.target.scene_depth]: () => gpu.depth, [FP.target.water_light]: () => gpu.waterLight, [FP.target.water_depth]: () => gpu.waterDepth };
    const plain = { [FP.pass.background]: () => gpu.bg, [FP.pass.surface]: () => gpu.surface, [FP.pass.caster]: () => gpu.caster,
      [FP.pass.water_transmit]: () => gpu.waterTransmit, [FP.pass.water_emit]: () => gpu.waterEmit };
    let pass = null, sorted = false, timed = false, mains = 0, blooms = 0, before = -1, bound0 = false, sparkGroup = undefined, tiled = false;
    for (let i = 0; i < count; i++) {
      const at = (i + 1) * 8, kind = words[at] & 255, id = (words[at] >> 8) & 255, source = (words[at] >> 16) & 255;
      const colour0 = words[at + 1] & 255, colour1 = (words[at + 1] >> 8) & 255, depth = (words[at + 1] >> 16) & 255, from = words[at + 1] >>> 24;
      const flags = words[at + 3], keep = !!(flags & 1), store = !!(flags & 2), first = words[at + 4], n = words[at + 5], instances = words[at + 6];
      if (kind === FP.op.cull_early) gpu.cull.early(encoder, real);
      else if (kind === FP.op.cull_late) gpu.cull.late(encoder, real);
      else if (kind === FP.op.particle_sort) { sorted = sortFrame(encoder, x); timed = sorted && gpu.timing && !gpu.timer.busy; }
      else if (kind === FP.op.copy) encoder.copyTextureToTexture({ texture: textures[from]() }, { texture: textures[words[at + 2] & 255]() }, [w, h]);
      else if (kind === FP.op.begin) {
        const colours = [colour0, colour1].filter((t) => t !== FP.target.none).map((t) => keep ? { view: views[t](), loadOp: "load", storeOp: "store" } : target(views[t]()));
        bound0 = false; sparkGroup = undefined;
        if (depth === FP.target.shadow_map) {
          // the key light's map: a tile's pass keeps the atlas (its clear pipeline first), the whole map is cleared
          tiled = keep;
          pass = timedPass(encoder, passTimer, { colorAttachments: [], depthStencilAttachment: keep ? { view: gpu.shadowView, depthLoadOp: "load", depthStoreOp: "store" }
            : { view: gpu.shadowMap.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" } });
          if (keep) pass.setPipeline(shadowClear()); else { pass.setBindGroup(0, gpu.bindGroup); bound0 = true; }
        } else if (depth === FP.target.scene_depth || !mains) {
          // the scene's pass, and the passes that go on from it keeping the light and depth
          const stamps = !mains && timed ? { timestampWrites: { querySet: gpu.timer.set, beginningOfPassWriteIndex: 2, endOfPassWriteIndex: 3 } } : {};
          pass = timedPass(encoder, passTimer, { colorAttachments: colours,
            ...(depth === FP.target.scene_depth ? { depthStencilAttachment: keep ? { view: gpu.depth.createView(), depthLoadOp: "load", depthStoreOp: store ? "store" : "discard" }
              : { view: gpu.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: store ? "store" : "discard" } } : {}), ...stamps });
          pass.setBindGroup(0, gpu.bindGroup);
          if (!keep) { if (gpu.caster) { pass.setBindGroup(1, gpu.emptyGroup); pass.setBindGroup(2, gpu.shadowGroup); } }
          else if (before === FP.op.cull_late) {
            pass.setBindGroup(1, gpu.sceneGroup);
            if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
            if (gpu.index) pass.setIndexBuffer(gpu.index, "uint32");
          } else {
            pass.setBindGroup(1, gpu.sceneGroup);
            pass.setBindGroup(2, gpu.shadow ? gpu.shadowGroup : (emptyLayout(), gpu.emptyGroup));
            pass.setBindGroup(3, gpu.waterCopyGroup);
            pass.setIndexBuffer(gpu.index, "uint32");
          }
          mains++;
        } else pass = timedPass(encoder, passTimer, { colorAttachments: colours });   // a bloom stage's target, or the screen for the post pass
      } else if (kind === FP.op.end) {
        pass.end(); pass = null;
        if (tiled && real) shadowTilesDrawn++;
        tiled = false;
      } else if (kind === FP.op.viewport) {
        if (n >>> 16) pass.setViewport(first & 65535, first >>> 16, n & 65535, n >>> 16, 0, 1);
        else pass.setViewport(0, 0, gpu.shadowMap.width, gpu.shadowMap.height, 0, 1);
      } else if (kind === FP.op.scissor) pass.setScissorRect(first & 65535, first >>> 16, n & 65535, n >>> 16);
      else if (kind === FP.op.media_before) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post);
      else if (kind === FP.op.media_after) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post, true);
      else if (kind === FP.op.each_begin) {
        // each wet draw in turn through the operations up to each_end
        let end = i + 1; while (end < count && (words[(end + 1) * 8] & 255) !== FP.op.each_end) end++;
        for (let d = first; d < first + n; d++) for (let k = i + 1; k < end; k++) { pass.setPipeline(plain[(words[(k + 1) * 8] >> 8) & 255]()); drawOne(pass, d); }
        i = end;
      } else if (kind === FP.op.draw) {
        if (id === FP.pass.shadow_clear) pass.draw(n);
        else if (id === FP.pass.shadow) {
          if (!bound0) { pass.setBindGroup(0, gpu.bindGroup); bound0 = true; }
          pass.setPipeline(gpu.shadow); drawScene(pass, false, source === FP.source.shadow_records ? shadowList : list, n, true);
        } else if (id === FP.pass.character_cast) characters.cast(pass);
        else if (id === FP.pass.scene) {
          pass.setPipeline(gpu.scene);
          if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
          drawScene(pass, true, list, n);
        } else if (id === FP.pass.scene_late) { pass.setPipeline(gpu.scene); gpu.cull.second(pass); }
        else if (id === FP.pass.grass) grassInto(pass);
        else if (id === FP.pass.character) {
          // after the world's opaque geometry, before its water and particles; then the world's groups again
          characters.draw(pass);
          pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.sceneGroup); pass.setBindGroup(2, gpu.shadowGroup);
          if (gpu.index) pass.setIndexBuffer(gpu.index, "uint32");
        } else if (id === FP.pass.particle_over || id === FP.pass.particle) {
          if (sparkGroup === undefined) {
            sparkGroup = sparksInto(instances);
            if (sparkGroup) { pass.setBindGroup(1, sparkGroup); if (gpu.particleDepth) { pass.setBindGroup(2, (emptyLayout(), gpu.emptyGroup)); pass.setBindGroup(3, gpu.waterCopyGroup); } }
          }
          if (sparkGroup) { pass.setPipeline(id === FP.pass.particle ? gpu.particles : gpu.particlesOver); pass.draw(n, instances); }
        } else if (id === FP.pass.bloom) {
          pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.blooms[blooms++][1]); pass.setPipeline(gpu.bloom); pass.draw(n);
        } else if (id === FP.pass.post) { pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.postGroup); pass.setPipeline(gpu.post); pass.draw(n); }
        else if (id === FP.pass.hud) {
          const most = Math.min(n, hudVertices);
          if (most && (!gpu.fxScene || gpu.sceneGroup)) { if (gpu.fxScene) pass.setBindGroup(1, gpu.sceneGroup); pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(most); }
        } else if (id === FP.pass.caster) { pass.setPipeline(gpu.caster); pass.draw(n, instances); }
        else { pass.setPipeline(plain[id]()); pass.draw(n); }   // the background's triangle, a traced page's surface
      }
      before = kind;
    }
    return timed;
  };
  // ---- the built-in order, for a page whose engine has no plan yet (pages move one at a time, #13303)
  const builtin = (encoder, passTimer, real) => {
  if (culled) gpu.cull.early(encoder, real);
  if (gpu.shadow && gpu.shadowTiles) {
    const st = arena.state;
    if (tile >= 0) {
      const [rx, ry, rw, rh] = [1, 2, 3, 4].map((f) => x.coaxial_shadow_v1(st, f));
      const shade = timedPass(encoder, passTimer, { colorAttachments: [],
        depthStencilAttachment: { view: gpu.shadowView, depthLoadOp: "load", depthStoreOp: "store" } });
      const clears = x.coaxial_shadow_v1(st, 7), count = x.coaxial_shadow_v1(st, 6);
      shade.setPipeline(shadowClear());
      for (let k = 0; k < count; k++) if (clears & (1 << k)) {
        const f = 16 + 4 * k;
        shade.setViewport(x.coaxial_shadow_v1(st, f), x.coaxial_shadow_v1(st, f + 1), x.coaxial_shadow_v1(st, f + 2), x.coaxial_shadow_v1(st, f + 3), 0, 1); shade.draw(3);
      }
      shade.setViewport(0, 0, gpu.shadowMap.width, gpu.shadowMap.height, 0, 1); shade.setScissorRect(rx, ry, rw, rh);
      shade.setBindGroup(0, gpu.bindGroup); shade.setPipeline(gpu.shadow);
      drawScene(shade, false, shadowList, separateShadow ? shadowDraws : draws - wet, true);
      if (hero && x.coaxial_shadow_v1(st, 5) === 1) characters.cast(shade);
      shade.end();
      if (real) shadowTilesDrawn++;
    }
  } else if (((draws || shadowDraws) && gpu.shadow) || gpu.caster) {
    // cleared every frame even with nothing to cast, so a traced scene never reads a stale map
    const shade = timedPass(encoder, passTimer, { colorAttachments: [],
      depthStencilAttachment: { view: gpu.shadowMap.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" } });
    shade.setBindGroup(0, gpu.bindGroup);
    if (shadowDraws && gpu.shadow) { shade.setPipeline(gpu.shadow); drawScene(shade, false, shadowList, separateShadow ? shadowDraws : draws - wet, true); }
    if (cast[0] && cast[1]) { shade.setPipeline(gpu.caster); shade.draw(cast[0], cast[1]); }
    if (hero) characters.cast(shade);
    shade.end();
  }
  // the particles' sort (the header's "Sorting"), before the pass that draws them; timed when asked
  const sorted = gpu.sort && gpu.particles && draws ? sortFrame(encoder, x) : false;
  const timed = sorted && gpu.timing && !gpu.timer.busy;
  let pass = timedPass(encoder, passTimer, { colorAttachments: post ? [target(gpu.light.createView()), target(gpu.aux.createView())] : [screen()],
    ...(gpu.scene || gpu.surface ? { depthStencilAttachment: { view: gpu.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: post || culled ? "store" : "discard" } } : {}),
    ...(timed ? { timestampWrites: { querySet: gpu.timer.set, beginningOfPassWriteIndex: 2, endOfPassWriteIndex: 3 } } : {}) });
  pass.setBindGroup(0, gpu.bindGroup);
  if (gpu.caster) { pass.setBindGroup(1, gpu.emptyGroup); pass.setBindGroup(2, gpu.shadowGroup); }
  pass.setPipeline(gpu.bg); pass.draw(3);
  const drawGrass = () => grassInto(pass);
  // Without a blend ABI the scene is opaque: populate depth before shading hidden blades.
  // Blended producers retain their existing ordering relative to grass.
  const opaqueGrass = draws && !x.coaxial_draw_blend_v1;
  if (!opaqueGrass) drawGrass();
  const surface = gpu.surface ? x.coaxial_surface_vertices(arena.state) : 0;
  if (surface) { pass.setPipeline(gpu.surface); pass.draw(surface); }
  if (draws) {
    pass.setPipeline(gpu.scene);
    if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
    drawScene(pass, true);
    if (culled) {
      // the cull's second phase: the pyramid of what was just drawn, then the pieces it no longer
      // hides, drawn on in a pass that keeps the light and depth
      pass.end();
      gpu.cull.late(encoder, real);
      const keep = (view) => ({ view, loadOp: "load", storeOp: "store" });
      pass = timedPass(encoder, passTimer, { colorAttachments: post ? [keep(gpu.light.createView()), keep(gpu.aux.createView())] : [keep(texture.createView())],
        depthStencilAttachment: { view: gpu.depth.createView(), depthLoadOp: "load", depthStoreOp: post ? "store" : "discard" } });
      pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.sceneGroup);
      if (gpu.shadow) pass.setBindGroup(2, gpu.shadowGroup);
      if (gpu.index) pass.setIndexBuffer(gpu.index, "uint32");
      pass.setPipeline(gpu.scene);
      gpu.cull.second(pass);
    }
    if (opaqueGrass) drawGrass();
    if (hero) {
      // after the world's opaque geometry, before its water and particles; then the world's groups again
      characters.draw(pass);
      pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.sceneGroup); pass.setBindGroup(2, gpu.shadowGroup);
      if (gpu.index) pass.setIndexBuffer(gpu.index, "uint32");
    }
    const sparks = gpu.particles ? x.coaxial_particle_instances(arena.state) : 0;
    if ((wet || (sparks && gpu.particleDepth)) && post && gpu.waterCopyGroup) {
      // W5: the scene's light and depth as they stand before the water (and the soft particles), then on
      // in a pass that keeps them
      pass.end();
      encoder.copyTextureToTexture({ texture: gpu.light }, { texture: gpu.waterLight }, [w, h]);
      encoder.copyTextureToTexture({ texture: gpu.depth }, { texture: gpu.waterDepth }, [w, h]);
      const keep = (view) => ({ view, loadOp: "load", storeOp: "store" });
      pass = timedPass(encoder, passTimer, { colorAttachments: [keep(gpu.light.createView()), keep(gpu.aux.createView())],
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
    const sparkGroup = sparksInto(sparks);
    if (sparkGroup) {
      pass.setBindGroup(1, sparkGroup);
      if (gpu.particleDepth) { pass.setBindGroup(2, (emptyLayout(), gpu.emptyGroup)); pass.setBindGroup(3, gpu.waterCopyGroup); }
      if (gpu.particlesOver) { pass.setPipeline(gpu.particlesOver); pass.draw(6, sparks); }
      pass.setPipeline(gpu.particles); pass.draw(6, sparks);
    }
  }
  const puffs = !gpu.scene && gpu.particles ? x.coaxial_particle_instances(arena.state) : 0;
  if (puffs) {
    // a traced page's particles, over what bg_fs drew
    const puffGroup = particleGroupNow();
    const bytes = puffGroup ? Math.min(x.coaxial_particle_bytes(arena.state), gpu.particleBuffer.size) & ~3 : 0;
    if (bytes) {
      device.queue.writeBuffer(gpu.particleBuffer, 0, memory.buffer, x.coaxial_particles(arena.state), bytes);
      pass.setBindGroup(1, puffGroup);
      if (gpu.particlesOver) { pass.setPipeline(gpu.particlesOver); pass.draw(6, puffs); }
      pass.setPipeline(gpu.particles); pass.draw(6, puffs);
    }
  }
  if (post) {
    pass.end();
    const blooms = x.coaxial_bloom_passes_v1 ? gpu.blooms.slice(0, Math.max(0, Math.min(gpu.blooms.length, x.coaxial_bloom_passes_v1(arena.state)))) : gpu.blooms;
    for (const [into, group] of blooms) {
      const b = timedPass(encoder, passTimer, { colorAttachments: [target(into.createView())] });
      b.setBindGroup(0, gpu.bindGroup); b.setBindGroup(1, group); b.setPipeline(gpu.bloom); b.draw(3); b.end();
    }
    pass = timedPass(encoder, passTimer, { colorAttachments: [screen()] });
    pass.setBindGroup(0, gpu.bindGroup); pass.setBindGroup(1, gpu.postGroup); pass.setPipeline(gpu.post); pass.draw(3);
  }
  if (x.coaxial_media) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post);
  if (n && (!gpu.fxScene || gpu.sceneGroup)) { if (gpu.fxScene) pass.setBindGroup(1, gpu.sceneGroup); pass.setPipeline(gpu.fx); pass.setVertexBuffer(0, gpu.vertexBuffer); pass.draw(n); }
  if (x.coaxial_media) drawMedia(x, pass, w, h, !!(gpu.scene || gpu.surface) && !post, true);
  pass.end();
  return timed;
  };
  // Ask the page's engine for its plan: this host's word (frame-plan.coax FPH_*) in, the operations out.
  let planned = 0;
  if (x.coaxial_frame_plan_v1 && planMode !== "off") {
    const has = (on, bit) => (on ? 1 << bit : 0);
    const copies = !!(post && gpu.waterCopyGroup && gpu.waterTransmit);
    const bits = has(gpu.scene, 0) | has(gpu.scene && gpu.shadow, 1) | has(gpu.scene && gpu.caster, 2) | has(gpu.scene && gpu.grass, 3) | has(!gpu.scene && gpu.surface, 4)
      | has(gpu.scene && gpu.waterTransmit, 5) | has(copies, 6) | has(post, 7) | has(gpu.scene && gpu.particles, 10) | has(copies && gpu.particles && gpu.particleDepth, 11)
      | has(gpu.scene && gpu.particles && gpu.particlesOver, 12) | has(x.coaxial_media, 13) | has(gpu.shadow && gpu.shadowTiles, 14) | has(culled, 15) | has(hero, 16)
      | has(gpu.scene && gpu.sort && gpu.particles, 17) | has(gpu.scene && gpu.sceneGroup, 18) | has(gpu.scene && gpu.particles, 19);
    const wrote = x.coaxial_frame_plan_v1(arena.state, bits >>> 0, arena.plan, PLAN_WORDS);
    const words = new Uint32Array(memory.buffer, arena.plan, PLAN_WORDS);
    if (wrote >= 8 && words[0] === FP.magic && words[1] === FP.version && words[2] === 8 && (words[3] + 1) * 8 === wrote) planned = words[3];
    else planStats.refused++;
    if (planned) {
      if (planMode === "check") {
        // the proof of the swap (#13303): the built-in order and the plan's walk, recorded on the same frame
        const a = planRecorder(null), b = planRecorder(encoder);
        try { builtin(a.encoder, null, false); } catch (e) { a.log.push(`threw ${e.message}`); }
        timed = walk(b.encoder, passTimer, words, planned, true);
        planCompare(a.log, b.log);
      } else timed = walk(encoder, passTimer, words, planned, true);
      planStats.frames++; planStats.ops = planned;
    }
  }
  // A page with a plan is never drawn in the built-in order (#13303): when its engine refuses this
  // frame's plan the frame is cleared and nothing is drawn (coaxial.framePlan().refused counts them).
  // The built-in order is for a page whose engine has no plan yet, and for ?plan=off.
  if (!planned && x.coaxial_frame_plan_v1 && planMode !== "off") timedPass(encoder, passTimer, { colorAttachments: [screen()] }).end();
  else if (!planned) timed = builtin(encoder, passTimer, true);
  if (timed) {
    encoder.resolveQuerySet(gpu.timer.set, 0, 4, gpu.timer.resolve, 0);
    encoder.copyBufferToBuffer(gpu.timer.resolve, 0, gpu.timer.read, 0, 32);
  }
  passTimerResolve(encoder, passTimer);
  const readback = captureRequested && probeArmed && ++frames >= 1 && !gpu.probed ? readBack(encoder, texture) : null;
  device.queue.submit([encoder.finish()]);
  if (culled) gpu.cull.submitted();
  roomTraceSubmitted(device.queue);
  globalThis.__evergladeBenchmark?.submitted(device.queue);
  worldSubmissions++;
  if (x.coaxial_frame_admit || hostLifecycle) {
    device.__coaxialFrames=(device.__coaxialFrames||0)+1;
    const began=frameTrace?performance.now():0;
    device.queue.onSubmittedWorkDone().then(()=>{if(began)device.__coaxialCompletionMs=performance.now()-began;}).catch(()=>{}).finally(()=>{device.__coaxialFrames--;});
  }
  if (traceRow) {
    traceRow[2]=performance.now()-legacyEncodeStart;traceRow[7]=1;traceRow[8]=device.__coaxialCompletionMs||0;traceRow[10]=device.__coaxialFrames||0;traceRow[11]=observedWorldDraws;traceRow[12]=hero?1:0;
  }
  if (timed) timingRead(gpu.timer);
  passTimerRead(passTimer);
  if (gpu.snap) { const take = gpu.snap; gpu.snap = null; take(); }   // card.js: this frame, before it is shown
  if (shaderLoads === 0 && installedGeneration === wanted && !globalThis.coaxial.sceneReady && pendingSceneEpoch !== sceneEpoch) {
    // A stroke-only world declares readiness in Coax after writing its world
    // geometry. Mesh worlds still require a nonempty admitted scene draw.
    let nonempty = !gpu.scene && x.coaxial_scene_ready?.(arena.state) === 1;
    for (let i = 0; i < draws; i++) if (list[i * 4 + 1] && list[i * 4 + 3]) { nonempty = true; break; }
    if (nonempty) {
      const epoch = sceneEpoch, generation = installedGeneration, frameEngine = engine, frameGpu = gpu, frameScene = gpu.scene;
      pendingSceneEpoch = epoch;loadMark("scene-first-draw");
      device.queue.onSubmittedWorkDone().then(() => {
        if (epoch === sceneEpoch && generation === wanted && shaderLoads === 0 && engine === frameEngine && gpu === frameGpu && gpu.scene === frameScene) { globalThis.coaxial.sceneReady = true;loadMark("scene-ready"); }
      }, (error) => { if (epoch === sceneEpoch) say(`scene frame: ${error?.message || error}`, true); });
    }
  }
  document.documentElement.classList.add("gpu");
  if (loading.on) loading.frame(globalThis.coaxial.sceneReady);
  if (readback) report(readback, texture);
  if (!hostTransport&&x.coaxial_sounds) playSounds(x);
  if (cellPack) pumpCells(x);
  after(false);
}

// ---- input: keys, the pointer and typed text go to the app; its actions come out
const KEYS = { Escape: -1, Enter: -2, Backspace: -3, ArrowLeft: -4, ArrowRight: -5, ArrowUp: -6, ArrowDown: -7,
  Home: -8, End: -9, PageUp: -10, PageDown: -11, Delete: -12, Shift: -13, Control: -14, Tab: -15, Alt: -16 };
// Input waits while a lost device is replaced (its state is put back as the loss found it), and a
// trap here stops the engine as one in a frame does.
const call = (name, ...args) => {
  if (!engine || recovering || !engine.exports[name]) return 0;
  try { return engine.exports[name](arena.state, ...args); } catch (e) { trapped(e); return 0; }
};

// ---- a realm that stops (metaverse-engine/book ch084). A trap in the engine leaves its state as far as the
// trapping call got, so the host keeps a copy of the state at each frame's start (every frame, or
// every HOLD_EVERY-th for a state over HOLD_BYTES) and, when the engine traps, saves the last one
// as the realm's rescue save and shows the reload message. The next load of the same engine (its
// wasm's SHA-256, its layout and state size all match) puts the save back once its world is in,
// so the save gives the same state digest; a save from any other build is dropped. A save is put
// back once: a realm that traps again within RESCUE_SETTLE frames of one starts over. The save is
// the state region the host owns (coaxial_state_size), the contract every face keeps its state in.
// IndexedDB coaxial-rescue, store saves, keyed by the save slot. Not in a golden walk.
const HOLD_BYTES = 2 * 1048576, HOLD_EVERY = 30, RESCUE_SETTLE = 300;
let frameHeld = null, heldFrames = 0, restoredFrames = -1, pendingRescue = null, rescueTaken = false, trapArmed = false;
let framesStarted = 0, heldAt = -1;   // checks: frame starts so far, and the one the held copy is from
const rescueDb = realmId && !golden ? new Promise((ok) => {
  try { const r = indexedDB.open("coaxial-rescue", 1); r.onupgradeneeded = () => r.result.createObjectStore("saves"); r.onsuccess = () => ok(r.result); r.onerror = r.onblocked = () => ok(null); }
  catch { ok(null); }
}) : Promise.resolve(null);
const rescueRead = rescueDb.then((db) => !db ? null : new Promise((ok) => {
  try { const q = db.transaction("saves").objectStore("saves").get(slot); q.onsuccess = () => ok(q.result || null); q.onerror = () => ok(null); }
  catch { ok(null); }
}));
function rescueWrite(value) {
  return rescueDb.then((db) => !db ? false : new Promise((ok) => {
    try {
      const t = db.transaction("saves", "readwrite"), store = t.objectStore("saves");
      if (value) store.put(value, slot); else store.delete(slot);
      t.oncomplete = () => ok(true);t.onerror = t.onabort = () => ok(false);
    } catch { ok(false); }
  }));
}
async function rescueIdentity(bytes) {
  try { return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map((b) => b.toString(16).padStart(2, "0")).join(""); }
  catch { return ""; }   // no SubtleCrypto (an insecure origin): no save
}
function rescueFrom(record, id, layout, size) {
  if (!record || rescueTaken) return;
  rescueTaken = true;
  rescueWrite(null);
  if (id && record.wasm === id && record.layout === layout && record.size === size && record.bytes?.byteLength === size) pendingRescue = new Uint8Array(record.bytes);
  else say(`${app}: a save from another build of this world was not put back`);
}
const stateBytes = () => new Uint8Array(memory.buffer, arena.state, arena.stateSize).slice();
const worldIn = () => firstAssets.every((a) => a.done || engine?.exports.coaxial_asset_optional_v1?.(a.i)) && [...firstPacks].every((p) => p.done);
function frameStart() {
  if (pendingRescue && worldIn()) {
    new Uint8Array(memory.buffer, arena.state, pendingRescue.length).set(pendingRescue);
    pendingRescue = null;restoredFrames = 0;
    globalThis.coaxial.rescued = dev ? rescueIdentity(stateBytes()) : true;   // checks: the state as put back, before its frame
    say(`${app}: back where it stopped`);
  }
  if (restoredFrames >= 0) restoredFrames += 1;
  framesStarted += 1;
  if (!realmId || golden) return;
  const size = arena.stateSize;
  if (size > HOLD_BYTES && heldFrames++ % HOLD_EVERY) return;
  if (frameHeld?.length !== size) frameHeld = new Uint8Array(size);
  frameHeld.set(new Uint8Array(memory.buffer, arena.state, size));
  heldAt = framesStarted;
}
// A trap's crash report (#13298, docs/deployment/crash-reports.md). The module ships without its
// function names, so the stack names functions by index (wasm-function[123]); NAME.wasm.syms beside
// the module (wasm.sh) has one "INDEX NAME" line a function. Only after a trap are the names fetched.
// The `crash` event then carries the module's file (rel), the Coax trap's own line (trap: the last
// line the engine wrote to stderr, when it wrote it just before) and the 8 innermost frames (f0 to f7:
// each its function's name, then " +0x" and its byte offset in the module's file when the stack gave
// one), which the API folds into a crash report v2 (coax/web/api/service.c, #13379).
let lastStderr = null;
async function crashProps(e) {
  const frames = [];
  for (const line of String(e?.stack || "").split("\n")) {
    const m = /wasm-function\[(\d+)\]/.exec(line);
    if (!m) continue;
    const named = /^\s*at (?:[\w$-]+\.)?([\w$]+) \(/.exec(line) || /^([\w$]+)@/.exec(line);   // a module that kept its names (development)
    const off = /wasm-function\[\d+\]:0x([0-9a-fA-F]{1,16})/.exec(line);   // the frame's byte offset in the module's file (crash report v2, #13379)
    frames.push({ index: +m[1], name: named ? named[1] : "", off: off ? off[1].toLowerCase() : "" });
    if (frames.length >= 8) break;
  }
  const p = { rel: wasmUrl.split("/").pop().slice(0, 80) };
  if (lastStderr && performance.now() - lastStderr.at < 2000) {
    // the build's own folder goes: the path from the repository's root, the line and what
    const at = /(?:^|\/)((?:apps|coax|aethyr|verse|everglade|components|pay|psionic|os|nostr|rules)\/[^:]+:\d+: .*)$/.exec(lastStderr.text);
    p.trap = (at ? at[1] : lastStderr.text).slice(-80);
  }
  let names = null;
  if (frames.length) try {
    const r = await Promise.race([fetch(wasmUrl + ".syms"), new Promise((_, late) => setTimeout(late, 4000))]);
    if (r.ok) {
      names = new Map();
      for (const l of (await r.text()).split("\n")) { const k = l.indexOf(" "); if (k > 0) names.set(+l.slice(0, k), l.slice(k + 1)); }
    }
  } catch {}
  // "NAME" or "NAME +0xOFFSET": one word, then the offset (room-crash.coax crash_page_report)
  frames.forEach((f, i) => { p["f" + i] = (names?.get(f.index) || f.name || "#" + f.index).replace(/\s/g, "?").slice(0, 60) + (f.off ? " +0x" + f.off : ""); });
  return p;
}
function trapped(e) {
  if (!engine) return;
  say(`${app} trapped: ${e.message}`, true);
  const again = restoredFrames >= 0 && restoredFrames < RESCUE_SETTLE;
  retireTransport();engine = null;globalThis.coaxial.ready = false;
  const bytes = again ? null : frameHeld?.slice() ?? null;
  stopped(`THE WORLD STOPPED: ${e.message}`, bytes, again ? "START OVER" : "", crashProps(e));
  // checks: the saved state's digest, and how many frame starts before the trapping one it was held
  // (0: that frame's own start; a state over HOLD_BYTES is held every HOLD_EVERY-th frame)
  if (dev && bytes) { globalThis.coaxial.stopped.held = rescueIdentity(bytes); globalThis.coaxial.stopped.heldAge = framesStarted - heldAt; }
}
// The reload message: what stopped, and one action. With the state to save, the save goes first
// and the reload puts it back; without, the realm starts over.
function stopped(what, bytes, action = "", props = null) {
  // a trap's event waits for its frames' names (crashProps, at most 4 s); any other stop goes at once
  if (props) props.then((more) => lifeBeacon("crash", what, more), () => lifeBeacon("crash", what)); else lifeBeacon("crash", what);
  const saving = bytes && engineBytes && arena ? rescueIdentity(engineBytes).then((wasm) => wasm
    ? rescueWrite({ wasm, layout: arena.layout, size: bytes.length, bytes: bytes.buffer, at: Date.now() }) : false) : Promise.resolve(false);
  const told = globalThis.coaxial.stopped = { what, saved: null };
  saving.then((ok) => { told.saved = ok; });
  loading.stop(what, action || (bytes ? "RELOAD WHERE YOU WERE" : "RELOAD"), () => saving.finally(() => location.reload()));
}
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
// Faces opting into coaxial_player_appearance_v1 share browser-local CC2 appearance:
// action16 writes it, message17 acknowledges it, message16 loads it before page facts.
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

// The editor shell (?edit; metaverse-engine/editor/shell.coax, GitHub #13416): a module of its own
// that the browser draws over the canvas (dom-host.js), on a page whose app tells what its world
// holds (coaxial_edit_model, editor.model.v1). This host only carries bytes between the two: the
// model to the shell whenever the app's changes (coaxial_edit_rev), and the shell's actions to the
// app as messages of the kind the shell names (520 a batch of edits, 521 a selection), and on the dev server
// the word that the realm's code can be opened here (event 9, #13421). It decides nothing
// but whether to open: on a deployed page, only for an admin (the viewer's lines), since the server lets
// nobody else write a world; or for anyone, on a page that says its world is the visitor's own and kept
// in this browser (coaxial_edit_own: WorldBuilder, #13422). It tells such a page the part of the window
// the shell's panes leave free (message 525), so the page's own interface is not under them.
const EDITOR = "editor-shell";
// A press, a key or a paste on the shell is the shell's: the world under it hears nothing of it.
const inEditor = (e) => !!e.target?.closest?.("[data-coaxial-dock]");   // the editor shell, or a page's own pane (below)
let editorAsked = false, editorRev = -1, editorFor = null;
function editorOpen() {
  if (editorAsked || !engine?.exports.coaxial_edit_model || !new URLSearchParams(location.search).has("edit")) return;
  // the dev server's page opens it for whoever asks; a deployed page only for an admin (who else writes a world: nobody),
  // unless the world is the visitor's own
  if (!dev && !viewerAdmin && !engine.exports.coaxial_edit_own?.()) return;
  editorAsked = true;
  const root = document.createElement("div"); // editor shell
  root.id = "coaxial-editor"; root.dataset.coaxialDock = "";
  document.body.appendChild(root);
  import(`/coaxial/dom-host.js?app=${EDITOR}&root=${root.id}`).catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
}
let editorIn = false, editorRoomTold = "", editorTicks = 0;
function editorStep() {
  const shell = globalThis.coaxialDoms?.[EDITOR], x = engine?.exports;
  if (!shell || !x?.coaxial_edit_model || editorIn || !arena) return;
  editorIn = true;
  try { editorCarry(shell, x); } finally { editorIn = false; }
  if ((editorTicks & 15) === 0) editorStatus(shell);
  if ((editorTicks++ & 7) === 0) editorRoom();   // the panes may have changed size since the last look: a few times a second is enough
}
// The Scripts pane (#13467; metaverse-engine/editor/shell-scripts.coax). While it is open the shell asks for the
// page's script status (its word 528, "scripts on"; "scripts off" when it shuts), and this host hands it what the
// script module says (world.script-status.v1, the text coaxial.scripts() answers) a few times a second as event 13,
// with no bytes while no script module runs. It reads none of it.
let editorScripts = false, editorScriptsTold = null;
function editorStatus(shell) {
  if (!editorScripts) return;
  const text = scriptHost ? globalThis.coaxial.scripts() : "";
  if (text === editorScriptsTold) return;
  editorScriptsTold = text;
  const sx = shell.exports(), bytes = new TextEncoder().encode(text).subarray(0, sx.dom_inbox_cap());
  new Uint8Array(shell.memory().buffer, sx.dom_inbox(shell.state()), bytes.length).set(bytes);
  sx.dom_event(shell.state(), 13, 0, bytes.length);
  shell.ask();
}
// A realm's scripts (#13467; docs/protocol/realm-script-v1.md). A realm that exports coaxial_script_wants
// runs realm scripts: small modules its world's tables name by hash. They are not run by the browser's own
// WebAssembly engine: the page's script module (apps/openagents-web/script-host.coax) interprets them with an
// instruction budget and a rollback, as a room does. This host only carries bytes, once a frame after the
// realm's step: the realm's frame (script.frame.v1) to the script module, the result (script.result.v1)
// back; and when the realm's wants change, the fetches the script module asks for (the world's script list
// and each module by its hash). It judges nothing, and a realm with no script rows loads no module.
let scriptHost = null, scriptAsked = false, scriptWant = "", scriptPumping = false, scriptFor = null;
async function scriptOpen() {
  try {
    const response = await fetch("/build/script-host.wasm", { cache: location.hostname === "127.0.0.1" || location.hostname === "localhost" ? "no-store" : "default" });
    const module = await WebAssembly.compile(await response.arrayBuffer());
    const mem = new WebAssembly.Memory({ initial: 64, maximum: 1024 });
    const imports = { env: { memory: mem } };
    for (const imp of WebAssembly.Module.imports(module)) {
      if (imp.kind === "function") (imports[imp.module] ||= {})[imp.name] ||= imp.name === "proc_exit" ? (code) => { throw new Error(`script-host exited (${code})`); } : () => 0;
    }
    const x = (await WebAssembly.instantiate(module, imports)).exports;
    x._initialize?.();
    const size = x.coaxial_state_size();
    const state = Math.ceil(Math.max(x.__heap_base?.value ?? 0, mem.buffer.byteLength) / 65536) * 65536;
    mem.grow(Math.ceil((size + 64) / 65536));
    x.coaxial_state_init(state);
    scriptHost = { x, mem, state };
    (globalThis.coaxial ||= {}).scripts = () => new TextDecoder().decode(new Uint8Array(mem.buffer, x.scripts_out(), x.scripts_status(state)));
  } catch (e) { console.warn("scripts: the script module did not load; the realm's things walk their plans", e); }
}
// The fetches the script module asks for, one at a time, until it asks for nothing.
async function scriptPump() {
  if (scriptPumping || !scriptHost) return;
  scriptPumping = true;
  try {
    for (;;) {
      const { x, mem, state } = scriptHost, n = x.scripts_need(state);
      if (!n) break;
      const url = new TextDecoder().decode(new Uint8Array(mem.buffer, x.scripts_out(), n));
      let status = 0, body = new Uint8Array(0);
      try { const r = await fetch(url, { credentials: "same-origin" }); status = r.status; body = new Uint8Array(await r.arrayBuffer()); } catch { status = 0; }
      if (body.length > x.scripts_in_cap()) { status = 0; body = new Uint8Array(0); }
      new Uint8Array(mem.buffer, x.scripts_in(), body.length).set(body);
      x.scripts_got(state, status, body.length);
    }
  } finally { scriptPumping = false; }
}
function scriptStep() {
  const x = engine?.exports;
  if (!x?.coaxial_script_wants || !arena) return;
  const wn = x.coaxial_script_wants_len(arena.state);
  if (!scriptHost) { if (wn && !scriptAsked) { scriptAsked = true; scriptOpen(); } return; }
  const sx = scriptHost.x, st = scriptHost.state;
  const want = wn ? new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_script_wants(arena.state), wn)) : "";
  if (want !== scriptWant || scriptFor !== engine) {
    scriptWant = want; scriptFor = engine;
    const bytes = new TextEncoder().encode(want);
    if (bytes.length <= sx.scripts_in_cap()) { new Uint8Array(scriptHost.mem.buffer, sx.scripts_in(), bytes.length).set(bytes); sx.scripts_want(st, bytes.length); }
  }
  if (!scriptPumping && sx.scripts_need(st)) scriptPump();   // scripts_need only looks: the pump asks again before each fetch
  const fn = x.coaxial_script_frame_len(arena.state);
  if (!fn) return;
  new Uint8Array(scriptHost.mem.buffer, sx.scripts_in(), fn).set(new Uint8Array(memory.buffer, x.coaxial_script_frame(arena.state), fn));
  const rn = sx.scripts_tick(st, fn);
  if (!rn || rn > x.coaxial_inbox_cap()) return;
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), rn).set(new Uint8Array(scriptHost.mem.buffer, sx.scripts_out(), rn));
  x.coaxial_script_result(arena.state, rn);
}

// A page's own pane (Crew's creation steps, #13462): a module of the page's own that the browser draws beside the
// canvas (dom-host.js), for a page whose controls are the frames set and not its HUD. The page names the module
// (coaxial_pane, coaxial_pane_len) and tells it what it holds (coaxial_pane_rev, coaxial_pane_model, event 6); the
// pane's actions go to the page as messages of the kinds the pane names. As with the editor's shell, this host
// only carries bytes, and the canvas is the part of the window the pane leaves free (editorRoom).
let paneAsked = false, paneApp = "", paneRev = -1, paneFor = null, paneTicks = 0;
function paneOpen() {
  const x = engine?.exports;
  if (paneAsked || !x?.coaxial_pane || !arena) return;
  if (new URLSearchParams(location.search).has("edit")) return;   // the editor's shell has the window
  const name = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_pane(arena.state), x.coaxial_pane_len(arena.state)));
  if (!/^[a-z][a-z0-9-]{0,31}$/.test(name)) return;   // none yet (the page has not read its address), or none asked for
  paneApp = name; paneAsked = true;
  const root = document.createElement("div"); // page pane
  root.id = "coaxial-pane"; root.dataset.coaxialDock = "";
  document.body.appendChild(root);
  import(`/coaxial/dom-host.js?app=${paneApp}&root=${root.id}`).catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
}
function paneActs() {
  const pane = globalThis.coaxialDoms?.[paneApp];
  if (!pane) return;
  const px = pane.exports(), st = pane.state();
  for (let kind; (kind = px.dom_act_kind(st)); ) {
    const body = new Uint8Array(pane.memory().buffer, px.dom_act(st), px.dom_act_len(st)).slice();
    px.dom_act_done(st);
    send(kind, body);
  }
}
let paneIn = false;
function paneStep() {
  paneOpen();
  const pane = globalThis.coaxialDoms?.[paneApp], x = engine?.exports;
  if (!pane || !x?.coaxial_pane_model || !arena || paneIn) return;
  paneIn = true;
  try {
    pane.after ||= () => { paneActs(); editorRoom(); after(false); };
    pane.still = agentMode;
    const rev = x.coaxial_pane_rev(arena.state);
    if (rev !== paneRev || paneFor !== engine) {
      paneRev = rev; paneFor = engine;
      const n = x.coaxial_pane_model_len(arena.state);
      const px = pane.exports(), most = Math.min(n, px.dom_inbox_cap());
      new Uint8Array(pane.memory().buffer, px.dom_inbox(pane.state()), most).set(new Uint8Array(memory.buffer, x.coaxial_pane_model(arena.state), most));
      px.dom_event(pane.state(), 6, 0, most);
      if (agentMode) pane.now(); else pane.ask();
    }
    paneActs();
  } finally { paneIn = false; }
  if ((paneTicks++ & 7) === 0) editorRoom();
}
// The agent strip (E9, #13418): the shell's foot has a slot, and what fills it is the chat page's own
// module (chat-web, the same conversation as /chat), opened by address the first time the slot is there.
const AGENT = "chat-web", AGENT_SLOT = "agent.slot";
let agentAsked = false;
function agentOpen() {
  if (agentAsked || !document.getElementById(AGENT_SLOT)) return;
  agentAsked = true;
  import(`/coaxial/dom-host.js?app=${AGENT}&root=${AGENT_SLOT}`).catch((error) => { console.error(error); });
}
// The site's side down the editor's left (docs/app-shell.md): a second copy of the chat's module, drawing the
// side alone (path /side), in the shell's slot for it. Every place in it is another page.
const SIDE_SLOT = "places.slot";
let sideAsked = false;
function sideOpen() {
  if (sideAsked || !document.getElementById(SIDE_SLOT)) return;
  sideAsked = true;
  import(`/coaxial/dom-host.js?app=${AGENT}&root=${SIDE_SLOT}&path=/side&as=side`).catch((error) => { console.error(error); });
}
// What the shell last told the conversation of the world (world.turn.v1), and the two ways bytes go between
// them (E9, #13418): the shell's word of the world to the conversation, so each turn carries the selection and
// "here"; and a hosted turn's plan (world.plan.v1) from the conversation to the shell, which makes it a batch
// of edits the page sends as its own. This host reads neither.
let agentWorld = null, agentTold = null;
function agentTell() {
  const chat = globalThis.coaxialDoms?.[AGENT];
  if (!chat || !agentWorld || agentTold === agentWorld) return;
  chat.after ||= agentActs;
  chat.still = agentMode;   // on an agent's clock no frame comes by itself: each event is drawn at once
  const cx = chat.exports(), most = Math.min(agentWorld.length, cx.dom_inbox_cap());
  new Uint8Array(chat.memory().buffer, cx.dom_inbox(chat.state()), most).set(agentWorld.subarray(0, most));
  cx.dom_event(chat.state(), 8, 0, most);
  agentTold = agentWorld;
  chat.ask();
}
function agentActs() {
  const chat = globalThis.coaxialDoms?.[AGENT], shell = globalThis.coaxialDoms?.[EDITOR];
  if (!chat || !shell) return;
  const cx = chat.exports(), cs = chat.state();
  for (let kind; (kind = cx.dom_act_kind(cs)); ) {
    const body = new Uint8Array(chat.memory().buffer, cx.dom_act(cs), cx.dom_act_len(cs)).slice();
    cx.dom_act_done(cs);
    if (kind !== 8) continue;
    const sx = shell.exports(), most = Math.min(body.length, sx.dom_inbox_cap());
    new Uint8Array(shell.memory().buffer, sx.dom_inbox(shell.state()), most).set(body.subarray(0, most));
    sx.dom_event(shell.state(), 8, 0, most);
    shell.ask();
    editorActs();
  }
}
function editorCarry(shell, x) {
  sideOpen(); agentOpen(); agentTell();
  const side = globalThis.coaxialDoms?.side;
  if (side) side.still = agentMode;   // on an agent's clock no frame comes by itself, here too
  shell.after ||= () => { editorActs(); sideOpen(); agentOpen(); editorRoom(); after(false); };
  shell.still = agentMode;
  // ?edit=agent: the page was opened for the agent (a world's card on /chat), so the foot is open from the start
  if (!shell.told) { shell.told = true; if (new URLSearchParams(location.search).get("edit") === "agent") { shell.exports().dom_event(shell.state(), 7, 0, 0); shell.ask(); } }
  // Code (E12, #13421): on the dev server's page the shell is told the page's module (event 9), and asks that
  // server for the realm's own sources. A deployed page's shell is never told, so it has no Code pane.
  if (dev && !shell.codeTold) {
    shell.codeTold = true;
    const sx = shell.exports(), name = new TextEncoder().encode(app);
    new Uint8Array(shell.memory().buffer, sx.dom_inbox(shell.state()), name.length).set(name);
    sx.dom_event(shell.state(), 9, 0, name.length);
    shell.ask();
  }
  // Someone is logged in (event 12): the shell then offers what only they may do (a report of a script, #13467).
  if (viewerIn && !shell.viewerTold) { shell.viewerTold = true; shell.exports().dom_event(shell.state(), 12, 0, 0); shell.ask(); }
  const rev = x.coaxial_edit_rev(arena.state);
  if (rev !== editorRev || editorFor !== engine) {
    editorRev = rev; editorFor = engine;
    const n = x.coaxial_edit_model_len(arena.state);
    const sx = shell.exports(), most = Math.min(n, sx.dom_inbox_cap());
    new Uint8Array(shell.memory().buffer, sx.dom_inbox(shell.state()), most).set(new Uint8Array(memory.buffer, x.coaxial_edit_model(arena.state), most));
    sx.dom_event(shell.state(), 6, 0, most);
    if (agentMode) shell.now(); else shell.ask();   // on an agent's clock the shell shows it before the next operation
  }
  editorActs();
}
// The part of the window the shell's panes leave free, for a page that asks (coaxial_edit_room_v1): message
// 525, "X0 Y0 X1 Y1" in CSS pixels. A pane that lies across is a strip above or below it; a narrow one is a side.
function editorRoom() {
  if (!arena || !document.querySelector("[data-coaxial-dock]")) return;
  const w = innerWidth, h = innerHeight;
  let x0 = 0, y0 = 0, x1 = w, y1 = h;
  for (const pane of document.querySelectorAll("[data-coaxial-dock] .pane, [data-coaxial-dock] .dock-s")) {
    const r = pane.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    if (r.width > w * 0.4) { if (r.top + r.height / 2 < h / 2) y0 = Math.max(y0, r.bottom); else y1 = Math.min(y1, r.top); }
    else if (r.left + r.width / 2 < w / 2) x0 = Math.max(x0, r.right);
    else x1 = Math.min(x1, r.left);
  }
  // The scene stands in a frame the panes make: the canvas is the part of the window they leave free, so the
  // world's own interface is inside it too, and nothing of the world is under a pane.
  const gap = editorGap();
  x0 = Math.round(x0 > 0 ? x0 + gap : 0); y0 = Math.round(y0 > 0 ? y0 + gap : 0);
  x1 = Math.round(x1 < w ? x1 - gap : w); y1 = Math.round(y1 < h ? y1 - gap : h);
  // a side with no pane keeps the dock's own margin, once there is a frame at all
  if (x0 > 0 || y0 > 0 || x1 < w || y1 < h) { x0 = Math.max(x0, gap); y0 = Math.max(y0, gap); x1 = Math.min(x1, w - gap); y1 = Math.min(y1, h - gap); }
  if (x1 - x0 < 160 || y1 - y0 < 120) { x0 = 0; y0 = 0; x1 = w; y1 = h; }   // no room for a frame: the panes lie over the scene
  const frame = [x0, y0, x1, y1].join(" ");
  if (frame === editorRoomTold) return;
  editorRoomTold = frame;
  Object.assign(canvas.style, { left: x0 + "px", top: y0 + "px", width: (x1 - x0) + "px", height: (y1 - y0) + "px", right: "auto", bottom: "auto" });
  canvasAt = [x0, y0];
  document.documentElement.dataset.coaxialEditFrame = frame;
  // a page that asks is told its room: all of the canvas now, in the canvas's own pixels
  if (engine?.exports.coaxial_edit_room_v1) send(525, `0 0 ${x1 - x0} ${y1 - y0}`);
}
// The space between a pane and the scene's frame: the dock's own gap.
function editorGap() {
  const dock = document.querySelector("[data-coaxial-dock] .dock");
  return dock ? parseFloat(getComputedStyle(dock).rowGap) || 0 : 0;
}
// Where the canvas's corner is in the window (CSS pixels): the window's corner, but in the editor's frame.
let canvasAt = [0, 0];
addEventListener("resize", () => editorRoom());
// The shell's word that the world's import pack is the one of this SHA-256 (526, "URL HASH"; #13420). A deployed page
// has no dev server to tell its tabs, so the pack this page loaded from that address is fetched again by the hash and
// swapped in (liveAsset: the copy from before stays if the new one is refused).
function editorPack(line) {
  const [url, hash] = line.split(" ");
  if (!url?.startsWith("/api/worlds/") || !/^[0-9a-f]{64}$/.test(hash || "")) return;
  liveAsset(url, hash).then((did) => { if (did) swapped("asset"); }).catch((e) => { say(e.message, true); });
}
function editorActs() {
  const shell = globalThis.coaxialDoms?.[EDITOR];
  if (!shell) return;
  const sx = shell.exports(), st = shell.state();
  for (let kind; (kind = sx.dom_act_kind(st)); ) {
    const body = new Uint8Array(shell.memory().buffer, sx.dom_act(st), sx.dom_act_len(st)).slice();
    sx.dom_act_done(st);
    if (kind === 524) { agentWorld = body; agentTell(); }   // for the conversation in the foot, not the page
    else if (kind === 526) editorPack(new TextDecoder().decode(body));   // for this host: the world's pack by its hash
    else if (kind === 528) { editorScripts = new TextDecoder().decode(body) === "scripts on"; editorScriptsTold = null; }   // for this host: whether the script status is wanted
    else send(kind, body);
  }
}

function send(kind, text = "") {
  if (waiting) { waiting.push(() => send(kind, text));return; }
  const x = engine?.exports;
  if (!x?.coaxial_message) return;
  const bytes = (text instanceof Uint8Array?text:new TextEncoder().encode(text)).subarray(0, x.coaxial_inbox_cap());
  new Uint8Array(memory.buffer, x.coaxial_inbox(arena.state), bytes.length).set(bytes);
  x.coaxial_message(arena.state, kind, bytes.length);
  editorStep();   // an answer may have changed what the editor's shell shows, whether or not a frame follows
  paneStep();
  if(kind===607&&mediaNoticeTrace){
    if(mediaNoticeTrace.length<112)mediaNoticeTrace.push({kind,bytes:Array.from(bytes)});
    else mediaNoticeDropped++;
  }
  after(false);
}
globalThis.coaxial = { send, ready: false, sceneReady: false, loadMarks: () => loadMarks.map((row) => row.slice()), cellLog: () => cellLog.map((row) => row.slice()), facts: () => pageFacts(), probe: () => engine?.exports.coaxial_probe?.(arena.state) ?? 0,
  roomTrace:()=>({active:sockets.flatMap(ws=>ws?.__roomTrace?[ws.__roomTrace.snapshot()]:[]),retired:roomTraceRetired.map(x=>x.snapshot()),error:globalThis.coaxial.roomTraceError||null}),
  framePlan: () => ({ ...planStats }), storeUploads: () => ({ frames: storeUploads.frames, total: storeUploads.total, last: storeUploads.last.slice(0, gpu?.stores?.length || 0) }), setWorldPaused, worldLoop: () => ({paused:worldPaused,frames:worldFrames,submissions:worldSubmissions,shadowTiles:shadowTilesDrawn,socketFrames,pending:gpu?.device.__coaxialFrames||0}),
  takeFrameTrace: () => frameTrace?.splice(0) ?? [], takePassTimes: () => passTimes?.splice(0) ?? [],
  // the instance cull (instance-cull.js): on or off for this page, and its latest counts when the pass timer is on
  cull: (on) => { if (gpu?.cull) gpu.cull.off = !on; return !!gpu?.cull && !gpu.cull.off; },
  cullStats: () => gpu?.cull?.stats() ?? null,
  meshCull: (on) => meshCullSwitch(on !== false),
  // the frame's draw lists for headless checks (coax/web/phone-fps-check.coax): draws and indices drawn, the view's and the shadow map's, and the view's indices by record (a draw's base instance >> 16, or "0.N" for a draw of record N alone)
  drawStats: () => { const x = engine?.exports, sum = (at, n) => { const d = n ? new Uint32Array(memory.buffer, at, n * 4) : []; let k = 0; for (let i = 0; i < n; i++) k += d[i * 4 + 1] * d[i * 4 + 3]; return k; };
    const n = x?.coaxial_draw_count?.(arena.state) ?? 0, sn = x?.coaxial_shadow_draw_count_v1?.(arena.state) ?? -1;
    const by = {}; if (n) { const d = new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), n * 4); for (let i = 0; i < n; i++) { const r = d[i * 4 + 2] >>> 16 || `0.${d[i * 4 + 2] & 65535}`; by[r] = (by[r] || 0) + d[i * 4 + 1] * d[i * 4 + 3]; } }
    return { draws: n, indices: n ? sum(x.coaxial_draws(arena.state), n) : 0, shadowDraws: sn, shadowIndices: sn > 0 ? sum(x.coaxial_shadow_draws_v1(arena.state), sn) : -1, byRecord: by }; },
  meshCullStats: () => { const x = engine?.exports, at = x?.coaxial_mesh_bounds_v1?.(arena.state) ?? 0, n = x?.coaxial_draw_count?.(arena.state) ?? 0, d = n ? new Uint32Array(memory.buffer, x.coaxial_draws(arena.state), n * 4) : [];
    let indices = 0; for (let i = 0; i < n; i++) indices += d[i * 4 + 1] * d[i * 4 + 3];
    return at ? { state: new Uint32Array(memory.buffer, at, 12)[0], mesh: new Uint32Array(memory.buffer, at, 12)[2], runs: new Uint32Array(memory.buffer, at, 12)[10], draws: n, indices } : null; },
  frameQueue: () => ({pending:gpu?.device.__coaxialFrames||0,bounded:typeof engine?.exports.coaxial_frame_admit==="function"}),
  quality: (field = 0) => engine?.exports.coaxial_quality_probe?.(arena.state, field) ?? -1,
  captureFrame: () => { if (captureRequested) { probeArmed = true;if (gpu) gpu.probed = false; }return probeArmed; },
  guardProbe: (field = 0) => engine?.exports.coaxial_guard_probe?.(arena.state, field) ?? 0,
  // a page's read-only facts for headless checks, where its face exports them (project-face.c)
  fieldProbe: (field = 0) => engine?.exports.coaxial_field_probe?.(arena.state, field) ?? -1,
  // The room's state for a page opening this one in place (zone-host.js): -1 none, 0 off, 1 opening, 2 open, 3 down.
  room: () => engine?.exports.coaxial_net_probe?.(arena.state, 0) ?? -1,
  // The agent driver (coax drive: coax/web/drive.coax, metaverse-engine/agent/drive.coax): send gives a realm one
  // command as message 700 and returns its answer (coaxial_drive_out), or null when the engine can't
  // take it yet; peek returns `size` bytes of the state at `offset` as hex, for `state` (the driver
  // reads the field's place and type from the build's layout, build/APP.layout).
  // The agent's clock (agent mode): step(MS) draws fixed frames, AGENT_HZ a second, until MS more of
  // simulation time has passed (at least one frame) and answers the clock, ms; now() is the clock;
  // scheduled() is how much the local room still has scheduled (-1: no local room, or one that can't say).
  agent: agentMode ? {
    pin: agentPin, now: () => clipNow,
    step: (ms, hz = 60) => (agentQueue = agentQueue.then(async () => {
      agentHeld = true;
      await globalThis.coaxialClipStep(Math.max(1, Math.round(Number(ms) * hz / 1000)), hz);
      return clipNow;
    })),
    scheduled: () => globalThis.coaxial?.roomHost?.scheduled ?? -1,
  } : null,
  // This page in the site's sandboxed frame (realm-frame.js, #13432): one text from the page that
  // holds it, answered with what the realm's module writes (coaxial_drive_out), or null. The module
  // reads it with the codec and refuses what is not a message; nothing here judges it.
  framed: (text) => {
    const x = engine?.exports;
    if (!x?.coaxial_drive_out || waiting || !arena) return null;
    let opaque = false;
    try { opaque = self.origin === "null"; } catch {}
    send(FRAME_ENV_KIND, `${globalThis.crossOriginIsolated ? 1 : 0} ${opaque ? 1 : 0}`);
    send(FRAME_KIND, String(text));
    return new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_drive_out(arena.state), x.coaxial_drive_out_len(arena.state)));
  },
  drive: {
    send: (text) => {
      const x = engine?.exports;
      if (!x?.coaxial_drive_out || waiting || !arena) return null;
      send(DRIVE_KIND, String(text));
      return new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_drive_out(arena.state), x.coaxial_drive_out_len(arena.state)));
    },
    peek: (offset, size) => {
      const n = engine?.exports.coaxial_state_size?.() ?? 0;
      if (!arena || !(offset >= 0) || !(size > 0) || offset + size > n) return null;
      return Array.from(new Uint8Array(memory.buffer, arena.state + offset, size), (b) => b.toString(16).padStart(2, "0")).join("");
    },
    stateSize: () => engine?.exports.coaxial_state_size?.() ?? 0,
    app: () => script?.dataset.app ?? "",
  },
  // zone-host.js: this page, opened in place, is shown now
  shown: () => send(SHOWN_KIND, `now=${Date.now()}\n`) };
// Headless verification observes effects without exposing renderer policy.
if (dev) {
  globalThis.coaxial.character=()=>characters.info();
  globalThis.coaxial.packRetention=()=>({...(packHost?.info()??{}),groups:packGroups.size});
  globalThis.coaxial.socketRetention=()=>({active:socketQueues?.info()??null,retired:lastSocketQueueRetired});
  globalThis.coaxial.holdSocketDelivery=value=>{socketDeliveryHeld=!!value;};
  globalThis.coaxial.holdSocketSending=value=>{socketSendingHeld=!!value;};
  globalThis.coaxial.actionFixtureMode=mode=>engine?.exports.coaxial_action_fixture_mode?.(mode);
  globalThis.coaxial.actionFixtureProbe=field=>Number(engine?.exports.coaxial_action_fixture_probe?.(field)??-1n);
  globalThis.coaxial.hostProbe=(event,field)=>engine?.exports.coaxial_host_probe?.(arena.state,event,field)?.toString() ?? "";
  globalThis.coaxial.hostReset=()=>engine?.exports.coaxial_host_reset?.(arena.state);
  globalThis.coaxial.hotbarProbe = (slot, field = 0) => engine?.exports.coaxial_hotbar_probe?.(arena.state, slot, field) ?? -1;
  // Read-only transport from the real Grid face for headless journeys.
  // The exported Coax procedures define every observed field.
  globalThis.coaxial.netProbe = (field = 0) => engine?.exports.coaxial_net_probe?.(arena.state, field) ?? -1;   // everglade-net.coax net_probe
  globalThis.coaxial.gridProbe = (field) => Number(engine?.exports.coaxial_grid_probe?.(BigInt(field)) ?? -1n);
  globalThis.coaxial.gridPose = (field) => engine?.exports.coaxial_grid_pose?.(BigInt(field)) ?? NaN;
  globalThis.coaxial.gridMark = (step) => Number(engine?.exports.coaxial_grid_mark?.(BigInt(step)) ?? -1n);
  globalThis.coaxial.gridReplay = () => engine?.exports.coaxial_grid_replay?.() ?? 0;
  globalThis.coaxial.gridEntry = (axis) => engine?.exports.coaxial_land_pylon_control?.(arena.state, 4, axis) ?? 0;
  globalThis.coaxial.reloadRenderer = async () => { try { return await loadShaders(); } catch { return false; } };
  globalThis.coaxial.renderResources = () => gpu?.renderer ? [gpu.renderer.vertexBuffer, gpu.renderer.uniformBuffer,
    ...[...gpu.renderer.resourceRecords.values()].map((r) => r.gpu), gpu.renderer.indexRecord?.gpu, gpu.renderer.sansTexture, gpu.renderer.layerTexture] : [];
  // Live audio for metaverse-engine/tests/live-audio.coax: a sound added as the realm's own, its
  // lines played through the host's path, and the voice graph for an offline render.
  globalThis.coaxial.audio = { add: (name, bytes, canonical=false) => { const asset={ i: assets.length, kind: "sound", name, bytes, done: true };if(canonical)bankSounds.set(name,{...asset,canonical:true});else assets.push(asset); },
    start: () => { startAudio(); return audio; }, play: (text) => playSoundText(text), playing: () => playing,
    heard: () => heard.slice(), probe: (slot, right, ahead, up) => engine?.exports.coaxial_sound_probe?.(arena.state, slot, right, ahead, up) ?? -1, beds: () => [...beds].map(([name, b]) => `${name} ${b.gain.gain.value.toFixed(3)} ${b.pan.pan.value.toFixed(3)}`),
    decode: (name) => decoded(soundAsset(name)), voice: soundVoice, mixed: () => mixed,
    contract: async () => ({version:hostTransport?1:0,owner:audioOwner,started:audioStarted,state:audio?.state??"absent",failure:audioFailure,memory:contractAudioMemory?.total()??0,retirement:lastAudioRetirement,queue:hostServices?.audioQueueInfo()??[0,0],queueRefusals:audioOfferRefusals,worklet:await contractAudio?.info()}),
    bank: () => {soundBank();return {status:audioBankStatus,count:bankSounds.size,bytes:Object.fromEntries([...bankSounds].map(([name,asset])=>[name,asset.bytes.length])),packet:audioPacketStatus};},
    clip:()=>({timeline:clipTimeline?.info()??null,memory:hostServices?.words()??0}),
    fixtureMode: mode => engine?.exports.coaxial_audio_fixture_mode?.(mode),fixtureProbe: field => Number(engine?.exports.coaxial_audio_fixture_probe?.(field)??-1n),
    bus: soundBus, busVoice, busLine,
    offlineCoax: renderCoaxAudio,
    offline: (lines, frames, buffers) => renderSoundLines(lines.map(([frame,line])=>[frame*1000/48000,line]), frames / 48000, 0, 48000, name => ({name}), a => Promise.resolve(buffers[a.name])) };
  globalThis.coaxial.hostLifecycle = {event:(kind,nowUs,arg)=>hostLifeEvent(kind,nowUs,arg),offer:(nowUs,pending)=>hostLifecycle?.offer(nowUs,pending),info:()=>hostLifecycle?.info(),reset:()=>hostLifecycle?.reset()};
  globalThis.coaxial.hostFrameAt = async nowUs => {draw(nowUs/1000);await gpu?.device.queue.onSubmittedWorkDone();};
  globalThis.coaxial.renderLedger = () => gpu?.renderer?.ready ? gpu.renderer.ledger() : null;
  globalThis.coaxial.renderPresentation = () => ({version:engine?.exports.coaxial_host_contract_version?.()??0,
    mode:gpu?.renderer?.presentationRequested??false,drawFormat:gpu?.renderer?.format??null,displayFormat:gpu?.renderer?.displayFormat??null,
    canonicalLayers:gpu?.renderer?.canonicalLayers??false,canonicalMedia:gpu?.renderer?.canonicalMedia??false,override:globalThis.coaxialRenderPresentation??null});
  globalThis.coaxial.renderLedgerLive = (slot, generation) => engine?.exports.coaxial_render_live?.(slot, generation) ?? -1;
  globalThis.coaxial.renderLedgerRelease = (slot, generation) => engine?.exports.coaxial_render_release?.(slot, generation) ?? -1;
  globalThis.coaxial.renderFixture = (kind) => kind === "video" ? engine?.exports.coaxial_render_fixture_video?.(arena.state) : engine?.exports.coaxial_render_fixture?.(arena.state);
  globalThis.coaxial.renderAssets = () => gpu?.renderer?.assetRows ?? [];
  globalThis.coaxial.renderMediaObjects = () => [...(gpu?.renderer?.mediaCache?.values() ?? [])];
  globalThis.coaxial.renderUploadState = (stage = false) => [...(gpu?.renderer?.resourceRecords?.values() ?? [])].filter((r) => r.source === gpu.renderer.ids.source.dynamic_store).map((r) => {
    if (stage) engine.exports.coaxial_render_upload_stage(r.id, BigInt(r.sourceCapacity), BigInt(r.sourceCapacity));
    return { name: r.name, pending: Number(engine.exports.coaxial_render_upload_pending(r.id)) };
  });
  globalThis.coaxial.renderMediaState = () => [...(gpu?.renderer?.mediaCache?.values() ?? [])].map(({ kind, url, ready, failed, error, width, height }) => ({ kind, url, ready, failed, error, width, height }));
  globalThis.coaxial.takeMediaNotices = () => ({notices:mediaNoticeTrace?.splice(0)??[],dropped:mediaNoticeDropped});
  globalThis.coaxial.renderMediaNoticeError = () => gpu?.renderer?.mediaNoticeError??"";
  globalThis.coaxial.reloadEngine = () => fetchEngine(beginEngine());
  globalThis.coaxial.renderSettled = () => gpu?.renderer?.settled;
  globalThis.coaxial.retireRenderer = () => stopEngine();
  globalThis.coaxial.renderEngineMarker = () => engine?.exports.coaxial_reload_marker?.() ?? null;
  globalThis.coaxial.renderBindingState = () => ({ stopped: !engine || !gpu?.bg, coherent: !!engine && !!gpu?.renderer && engine.exports === gpu.renderer.x });
  globalThis.coaxial.renderBank = () => engine?.exports.coaxial_render_active_bank?.();
  // Recovery checks (coax/web/gpu-recovery.coax): the state region's SHA-256, a real trap in the
  // next frame (its state pointer out of bounds), and a lost device.
  globalThis.coaxial.stateDigest = () => (engine && arena ? rescueIdentity(stateBytes()) : Promise.resolve(""));
  globalThis.coaxial.trapNextFrame = () => { trapArmed = true;return !!engine; };   // staging has it too (below)
  globalThis.coaxial.loseDevice = () => { gpu?.device.destroy();return !!gpu; };
  globalThis.coaxial.waitingWrites = () => waiting?.length ?? 0;
}
if (crashTest && !dev) globalThis.coaxial.trapNextFrame = () => { trapArmed = true;return !!engine; };   // staging's crash test (#13298): the next frame traps
// The OpenAgents app turns the camera with the phone (its gyro), radians since the last call.
globalThis.coaxial.tilt = (dyaw, dpitch) => { engine?.exports.coaxial_tilt?.(arena.state, dyaw, dpitch); };
// A phone's browser turns the camera with the phone too, while the app asks (coaxial_gyro, metaverse-engine/cam/gyro.coax):
// devicemotion's rotation rate, in the screen's frame, summed between frames. iOS asks for the motion
// permission, which only a touch's end may do; the app hears what came of it (coaxial_gyro_status).
// Browser glue: the sign of up from gravity is taken toward the screen's top and face, since WebKit's
// and Chrome's accelerationIncludingGravity point opposite ways.
const gyro = { on: false, asked: false, last: 0, quiet: 0 };
function gyroStatus(v) { if (engine?.exports.coaxial_gyro_status) call("coaxial_gyro_status", v); }
function gyroMotion(e) {
  const r = e.rotationRate, g = e.accelerationIncludingGravity, now = performance.now();
  const dt = gyro.last ? Math.min(0.1, (now - gyro.last) / 1000) : 0; gyro.last = now;
  // a browser with no sensor says so with an empty event (desktop Chrome); a real turn later says otherwise
  if (!r || (r.alpha == null && r.beta == null && r.gamma == null)) { if (!gyro.quiet++) gyroStatus(3); return; }
  if (gyro.quiet) { gyro.quiet = 0; gyroStatus(1); }
  if (!dt || !engine?.exports.coaxial_tilt) return;
  const k = Math.PI / 180, rx = (r.beta || 0) * k, ry = (r.gamma || 0) * k, rz = (r.alpha || 0) * k;
  const a = ((screen.orientation?.angle ?? window.orientation ?? 0) % 360 + 360) % 360;
  const across = a === 90 ? [0, -1] : a === 270 ? [0, 1] : a === 180 ? [-1, 0] : [1, 0];
  const top = a === 90 ? [1, 0] : a === 270 ? [-1, 0] : a === 180 ? [0, -1] : [0, 1];
  let ux = g?.x || 0, uy = g?.y || 0, uz = g?.z || 0; const len = Math.hypot(ux, uy, uz);
  let dyaw = 0;
  if (len > 1) { ux /= len; uy /= len; uz /= len; if (ux * top[0] + uy * top[1] + uz < 0) { ux = -ux; uy = -uy; uz = -uz; } dyaw = (rx * ux + ry * uy + rz * uz) * dt; }
  call("coaxial_tilt", dyaw, -(rx * across[0] + ry * across[1]) * dt);
}
function gyroListen() { if (gyro.on) return; gyro.on = true; gyro.last = 0; addEventListener("devicemotion", gyroMotion); gyroStatus(1); }
// After a touch: start or stop as the app wants; iOS's permission inside the touch's end (touchend).
function gyroSync(ended) {
  if (!engine?.exports.coaxial_gyro) return;
  const want = call("coaxial_gyro") === 1;
  // a sensor that has only said it is not there is still heard: a real turn after it says it is (gyroMotion)
  if (!want) { if (gyro.on && !gyro.quiet) { removeEventListener("devicemotion", gyroMotion); gyro.on = false; } return; }
  if (gyro.on) return;
  if (typeof DeviceMotionEvent === "undefined") { gyroStatus(3); return; }
  if (typeof DeviceMotionEvent.requestPermission !== "function") { gyroListen(); return; }
  if (!ended || gyro.asked) return;
  gyro.asked = true;
  // "denied" is the player's answer; a throw (no gesture) is not, and the next touch asks again
  DeviceMotionEvent.requestPermission().then((r) => { gyro.asked = false; if (r === "granted") gyroListen(); else gyroStatus(2); }, () => { gyro.asked = false; });
}
canvas?.addEventListener("touchend", () => gyroSync(true));
// A page that takes dropped files (data-drops: /projects, NEW PROJECT): drop.js uploads them, the app draws it.
if (script?.dataset.drops) startDrops({ send });
// A page that sends files to this site (data-uploads: what its picker takes): uploads.js, on the app's action 30.
if (script?.dataset.uploads) startUploads({ send, accept: script.dataset.uploads });
// A design document for a realm's feature list or the builder (docs.js), on the app's action 31.
startDocs({ send });
// A request with flag 32 (http-requests.js): the next frame drawn, at most 1024 wide, as base64 JPEG.
async function frameJpeg() {
  try {
    const shot = await snapshot();
    if (!shot) return null;
    const k = Math.min(1, 1024 / shot.width), c = new OffscreenCanvas(Math.round(shot.width * k), Math.round(shot.height * k));
    c.getContext("2d").drawImage(shot, 0, 0, c.width, c.height);
    const b = new Uint8Array(await (await c.convertToBlob({ type: "image/jpeg", quality: 0.75 })).arrayBuffer());
    let s = "";
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
    return btoa(s);
  } catch { return null; }
}
// A page's own share card (card.js, docs/web/og-tags.md): the next frame drawn, copied in its own task.
function snapshot() {
  return new Promise((resolve) => {
    if (!gpu) { resolve(null); return; }
    const timer = setTimeout(() => { if (gpu) gpu.snap = null; resolve(null); }, 5000);
    gpu.snap = () => {
      clearTimeout(timer);
      try {
        const c = new OffscreenCanvas(canvas.width, canvas.height);   // never shown: card.js draws the card from it
        c.getContext("2d").drawImage(canvas, 0, 0);
        resolve(c);
      } catch { resolve(null); }
    };
  });
}
if (script?.dataset.card) startCard({ url: script.dataset.card, key: () => player()?.key || "", snapshot, ready: () => !!globalThis.coaxial.sceneReady });
// A page that talks (data-voice: the route its voice host answers as): voice.js, on the app's VOICE.
const voice = script?.dataset.voice
  ? startVoice({ send, route: script.dataset.voice, wasmUrl: script.dataset.voiceWasm || "/build/voice.wasm" }) : null;
globalThis.coaxial.voice = voice;

// The location's hash, for an app that opens something by it (a link straight to it).
// Action 1 to a same-page address that differs only after its # (/worlds/built#a to #b) reloads the
// page: assigning such an address only changes the hash, and the app would go on showing the old one.
// A page opened in place by another (zone-host.js, data-zone on its frame) goes on as the whole tab.
const zoned = (() => { try { return window.top !== window && window.frameElement?.dataset.zone === "1"; } catch { return false; } })();
const tabLocation = zoned ? window.top.location : location;
function navigateTo(path) {
  const to = new URL(path, location.href);
  const samePage = to.origin === location.origin && to.pathname === location.pathname && to.search === location.search;
  // A page warmed with action 18 is handed over in place (zone-host.js) rather than loaded again.
  if (!(samePage && to.hash) && to.origin === location.origin && enterPage(to.pathname + to.search + to.hash, zoneOut)) return;
  tabLocation.assign(to.href);
  if (samePage && to.hash) tabLocation.reload();
}
// Action 21: the page at path, its engine, shaders, UI font and first assets (its script's data-wasm or
// /build/APP.wasm, data-wgsl or /APP.wgsl, data-ui-font-src and data-assets' same-origin files, sounds
// aside: they load when first played) fetched once into the HTTP cache, in the background. Nothing
// is kept here, and a failure only means the page loads as it would have.
const warmed = new Set();
async function warmCache(path) {
  const to = new URL(path, location.href);
  if (to.origin !== location.origin || warmed.has(to.href)) return;
  warmed.add(to.href);
  try {
    const doc = new DOMParser().parseFromString(await (await fetch(to.href)).text(), "text/html");
    const d = doc.querySelector("script[data-app]")?.dataset;
    if (!d) return;
    const urls = [d.wasm || `/build/${d.app}.wasm`, d.wgsl || `/${d.app}.wgsl`];
    if (d.uiFontSrc) urls.push(d.uiFontSrc);
    if (d.uiTitleFontSrc) urls.push(d.uiTitleFontSrc);
    for (const entry of (d.assets || "").split(/\s+/).filter(Boolean)) {
      const k = entry.indexOf(":"), kind = entry.slice(0, k), rest = entry.slice(k + 1);
      if (kind !== "sound" && kind !== "sounds" && kind !== "private" && rest.startsWith("/") && !rest.startsWith("//")) urls.push(rest);
    }
    // read through and let go as it comes (a world's pack can be tens of MB: never held whole here)
    await Promise.all(urls.map((u) => fetch(new URL(u, to.href)).then((r) => r.body ? r.body.pipeTo(new WritableStream()) : null, () => null).catch(() => null)));
  } catch {}
}
// This page has handed the tab to the page it opened in place: it stops as if it were unloaded.
function zoneOut() {
  leaving = true;send(LEAVE_KIND);httpRequests.reset();
  setWorldPaused(true);
  const device = gpu?.device;
  stopEngine();
  try { device?.destroy(); } catch {}
  canvas.remove();a11y.root?.remove();a11y.status?.remove();field?.remove();
}

// The site's page speaks to a realm in its sandboxed frame (realm-frame.js): ?frame=ORIGIN names the
// page that holds it. Only that window and that origin are heard, only text, only when the origin is
// one this page lists (its script's data-frame-origins) or, on this computer's own addresses, another
// of them. The answer goes back to that origin alone.
{
  const holder = new URLSearchParams(location.search).get("frame") || "";
  const local = (h) => h === "127.0.0.1" || h === "localhost" || h === "[::1]";
  let allowed = false;
  try {
    const u = new URL(holder);
    allowed = u.origin === holder && ((document.querySelector("script[data-app]")?.dataset.frameOrigins || "").split(" ").includes(holder) || (local(location.hostname) && local(u.hostname)));
  } catch {}
  if (allowed && window.parent !== window) {
    addEventListener("message", (e) => {
      if (e.source !== window.parent || e.origin !== holder || typeof e.data !== "string" || e.data.length > 4096) return;
      const answer = globalThis.coaxial?.framed?.(e.data);
      if (answer) window.parent.postMessage(answer, holder);
    });
  }
}
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
  if(hostTransport&&x.coaxial_sounds)playSounds(x);
  const want = call("coaxial_wants_text") === 1;
  if (want && document.activeElement !== field ? gesture || !touched : want && tap && touched) focusField();
  else if (!want && field && document.activeElement === field) field.blur();
  if (field) placeField(want);
  const code = call("coaxial_action");
  if (code) {
    const packet=actionSource(x,arena.state,memory,hostTransport?hostServices.actionSpan:null);
    if(!packet){call("coaxial_action_done");send(604,"Action output unavailable");return;}
    const text = new TextDecoder().decode(packet.view.slice());
    call("coaxial_action_done");
    // A Lightning invoice goes to a wallet. One in this browser (WebLN: the app's own wallet, an
    // extension) pays it here and reports the preimage, which the page hears as message 615 and
    // sends on as proof (docs/metaverse/makers.md); with none, or when it declines, the device's wallet opens.
    if (code === 1 && /^lightning:ln[a-z0-9]+$/i.test(text) && typeof globalThis.webln?.sendPayment === "function") {
      Promise.resolve(globalThis.webln.enable?.()).then(() => globalThis.webln.sendPayment(text.slice(10)))
        .then((paid) => { const pre = String(paid?.preimage ?? "").toLowerCase(); if (/^[0-9a-f]{64}$/.test(pre)) send(615, pre); })
        .catch(() => tabLocation.assign(text));
    }
    else if (code === 1) { if (text.startsWith("https://") || /^lightning:ln[a-z0-9]+$/i.test(text)) tabLocation.assign(text); else if (text.startsWith("/")) navigateTo(root && !text.startsWith(root + "/") ? root + (text === "/" ? "/index.html" : text) : text); }
    // A copy or cut: the app chose the text (never a masked field's); this runs
    // inside the key event, so the browser lets it reach the clipboard.
    else if (code === 2) navigator.clipboard?.writeText(text).catch(() => {});
    // The app's own place in the page (the hash), replaced so Back still leaves the page.
    else if (code === 3) { if (location.hash.slice(1) !== text) history.replaceState(history.state, "", text ? `#${encodeURIComponent(text)}` : location.pathname + location.search); }
    else if (code === 4) keep(text);
    // Versioned, origin-wide player appearance. The Coax consumer validates the record.
    else if (code === 16 && x.coaxial_player_appearance_v1?.() === 1) {
      try { localStorage.setItem("aethyr.player.appearance.v1", text);send(17,text); }
      catch {send(17,"");}
    }
    // Open another page in place, out of sight until action 1 goes to it (zone-host.js); progress back as message 18.
    else if (code === 18 && text.startsWith("/")) warmPage(root && !text.startsWith(root + "/") ? root + text : text, (t) => send(18, t));
    // Fetch a same-origin page and what its engine loads first into the HTTP cache, so going there
    // next (action 1) loads from it: Everglade's portal on the homepage asks while its camera flies.
    else if (code === 21 && text.startsWith("/") && !text.startsWith("//")) warmCache(root && !text.startsWith(root + "/") ? root + text : text);
    // The app's place in the site, said in the address bar without loading anything: one world whose
    // places have their own addresses (Character Create walking into Everglade, #13018). Same origin
    // only; the page's query and hash stay unless the path brings its own.
    else if (code === 19 && text.startsWith("/") && !text.startsWith("//")) {
      const to = new URL(root && !text.startsWith(root + "/") ? root + text : text, location.href);
      if (to.origin === location.origin) history.replaceState(history.state, "", to.pathname + (to.search || location.search) + (to.hash || location.hash));
    }
    else if (code === PLACE_KIND) { if (!golden) try { localStorage.setItem(placeKey, text); } catch {} }
    else if (code === BAR_KIND) { if (!golden) try { localStorage.setItem(barKey, text); } catch {} }
    else if (code === 6) setQuality(text);
    else if (code === 33) { if (!golden) try { localStorage.setItem("verse.quality.settled", text.slice(0, 16)); } catch {} }
    // a realm's sound settings, "VOLUME MUTED" (0..100, 0 or 1), back in its facts as sound=
    else if (code === 13) { if (!golden) try { localStorage.setItem("verse.sound", text.slice(0, 16)); } catch {} }
    // a realm's input and reading settings (metaverse-engine/ui/settings.coax), back in its facts as settings=
    else if (code === 14) { if (!golden) try { localStorage.setItem("verse.settings", text.slice(0, 256)); } catch {} }
    // the player's gyro settings (metaverse-engine/cam/gyro.coax), back in its facts as gyro=
    else if (code === 15) { if (!golden) try { localStorage.setItem("verse.gyro", text.slice(0, 32)); } catch {} }
    else if (code === ACT_VOICE && voice) voice.toggle(text === "end" ? "end" : text === "hold" ? "hold" : text === "on");
    else if (code === ACT_VOICE_TO && voice) voice.face(text);
    else if (code === ACT_VOICE_SAY && voice) voice.say(text);
    else if (code === 5) { try { const p = JSON.parse(text); if (p && typeof p.key === "string") localStorage.setItem("verse.player", text); } catch {} }
    else dispatchEvent(new CustomEvent("coaxial", { detail: { code, text } }));
  }
  syncLook();
  const cursor = ["", "pointer", "crosshair", "text", "none"][call("coaxial_cursor")] || "";
  if ((canvas.dataset.cursor || "") !== cursor) canvas.dataset.cursor = cursor;   // the page's CSS turns data-cursor into the cursor (realms.ts)
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
  if (!engine || e.isComposing || inEditor(e)) return;
  // the accessible layer's own keys: Tab moves through it, Enter and Space press what has focus
  if (a11y.root && ((e.key === "Tab" && !(call("coaxial_mouselook") > 0)) || (a11y.root.contains(e.target) && (e.key === "Enter" || e.key === " ")))) return;
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
  if (!engine?.exports.coaxial_key_up || e.isComposing || inEditor(e)) return;
  const key = keyOf(e);
  if (key && call("coaxial_key_up", key, modsOf(e))) e.preventDefault();
});
addEventListener("blur", () => {if(hostLifecycle)hostLifeEvent(3);else call("coaxial_blur");});
addEventListener("focus", () => {hostLifeEvent(2);call("coaxial_focus");});
addEventListener("visibilitychange", () => { if (document.hidden&&!hostLifecycle) call("coaxial_blur"); });
canvas.addEventListener("wheel", (e) => {
  if (!engine?.exports.coaxial_wheel) return;
  e.preventDefault();
  call("coaxial_wheel", e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1));
}, { passive: false });
addEventListener("paste", (e) => {
  if (e.target === field || !engine || inEditor(e)) return;
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
    if (!engine || inEditor(e)) return;
    const touch = e.pointerType !== "mouse", now = performance.now();
    if (touch) touchAt = now;
    else if (now - touchAt < AFTER_TOUCH_MS) return;
    let x = e.clientX - canvasAt[0], y = e.clientY - canvasAt[1], ph = phase;
    // the pointer held (mouselook): a move only turns the camera, and a press is at the reticle
    if (!touch && looking()) {
      if (phase === 1) { call("coaxial_look_delta", e.movementX || 0, e.movementY || 0); return; }
      const r = canvas.getBoundingClientRect(); x = r.width / 2; y = r.height / 2;
    }
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
    if (engine.exports.coaxial_pointer_buttons_v1) call("coaxial_pointer_buttons_v1", x * dpr, y * dpr, ph, touch ? 1 : 0, e.pointerId, e.button);
    else call("coaxial_pointer", x * dpr, y * dpr, ph, touch ? 1 : 0, e.pointerId);
    after(ph === 2, ph === 2);
    if (touch) gyroSync(false);
  });

// Mouselook (metaverse-engine/cam/mouselook.coax): the app says when it wants the pointer held; the host asks the
// browser inside a user gesture (a press or a key; Chrome lets a lock let go by the page come back
// without one), lets it go when the app frees the cursor, and says what the browser did. A refusal
// is the app's to answer (its drag orbit), never an error here. Dev pages can stand in for the
// browser's lock (coaxial.pointerLock) so headless checks can drive it.
let lookHeld = false, lookAsked = false, lookFake = false;
const looking = () => lookHeld || lookFake;
function syncLook() {
  const want = call("coaxial_mouselook");
  if (!canvas || !engine?.exports.coaxial_mouselook) return;
  if (lookFake) { if (want !== 2) { lookFake = false; call("coaxial_pointer_lock", 0); } return; }
  if (want === 2 && !lookHeld && !lookAsked && navigator.userActivation?.isActive !== false && document.hasFocus()) {
    lookAsked = true;
    try {
      const p = canvas.requestPointerLock?.();
      if (p?.catch) p.catch(() => { lookAsked = false; if (!lookHeld) call("coaxial_pointer_lock", 2); });
    } catch { lookAsked = false; call("coaxial_pointer_lock", 2); }
  } else if (want !== 2 && lookHeld) document.exitPointerLock?.();
}
document.addEventListener("pointerlockchange", () => {
  const held = document.pointerLockElement === canvas;
  lookAsked = false;
  if (held === lookHeld) return;
  lookHeld = held; call("coaxial_pointer_lock", held ? 1 : 0);
});
document.addEventListener("pointerlockerror", () => { lookAsked = false; call("coaxial_pointer_lock", 2); });
// Development checks only: 1 holds the pointer as the browser would (moves then turn the camera), 0 lets it go.
if (dev) globalThis.coaxial.pointerLock = (status) => {
  lookFake = status === 1; if (lookHeld && status !== 1) document.exitPointerLock?.();
  call("coaxial_pointer_lock", status); return call("coaxial_mouselook");
};

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
  const pixelBytes = texture.format === "rgba16float" ? 8 : 4;
  const row = align(texture.width * pixelBytes, 256);
  const buffer = gpu.device.createBuffer({ size: row * texture.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow: row }, [texture.width, texture.height]);
  return { buffer, row, pixelBytes };
}
async function report({ buffer, row, pixelBytes = 4, rendererSnapshot, validation, textureCapture }, texture) {
  const validated = validation ? await validation : null;
  if (validated && !validated.ok) { buffer.destroy();throw Error(validated.error); }
  await buffer.mapAsync(GPUMapMode.READ);
  const mapped = new Uint8Array(buffer.getMappedRange());
  const w = texture.width, h = texture.height, cols = 110, rows = 14;
  if (textureCapture && rendererSnapshot) rendererSnapshot.textures.push(...await textureCapture);
  const px = new Uint8Array(w * h * pixelBytes);
  for (let y = 0; y < h; y++) px.set(mapped.subarray(y * row, y * row + w * pixelBytes), y * w * pixelBytes);
  buffer.unmap();buffer.destroy();
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", px));
  globalThis.coaxial.renderCapture = { schema: "aethyr.render.readback.v1", width: w, height: h, format: texture.format, frame_validation: validated,
    row_bytes: w * pixelBytes, sha256: [...digest].map((b) => b.toString(16).padStart(2, "0")).join(""), bytes: Array.from(px),
    ...rendererSnapshot };
  if (gpu.renderer || pixelBytes !== 4) return;
  const x0 = Math.floor(w * 0.3), x1 = Math.floor(w * 0.7), y0 = Math.floor(h * 0.47), y1 = Math.floor(h * 0.53);
  let art = "", peak = 0, sum = 0;
  for (let r = 0; r < rows; r++) {
    let line = "";
    for (let c = 0; c < cols; c++) {
      let m = 0;
      for (let y = y0 + Math.floor((y1 - y0) * r / rows); y < y0 + Math.floor((y1 - y0) * (r + 1) / rows); y++)
        for (let x = x0 + Math.floor((x1 - x0) * c / cols); x < x0 + Math.floor((x1 - x0) * (c + 1) / cols); x++)
          m = Math.max(m, px[(y * w + x) * pixelBytes + 1]);
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
  // One queue owns destructive engine and shader installation. Asset effects
  // await this same queue, so none can publish through a retired facade.
  const install = renderReload.then(async () => {
    if (generation !== wanted) return false;
    engineInstalling = true;
    try { return await loadEngine(bytes, generation); }
    catch (error) { stopEngine();throw error; }
    finally { engineInstalling = false; }
  });
  renderReload = install.catch(() => {});return install;
}
function stopEngine() {
  globalThis.coaxial.fastEdit?.stop();delete globalThis.coaxial.fastEdit;
  retireTransport();
  engine = null;
  if (cellPack) { cellPack.cells?.close();cellPack.cells = null;cellPack = null; }
  cellAsks.clear();cellShared.clear();
  if (gpu) { gpu.bg = null;gpu.renderer?.destroy();gpu.renderer = null; }
  retirePacks();
  globalThis.coaxial.ready = false;globalThis.coaxial.sceneReady = false;
}

// The packs and textures this tab loaded, by URL, for `coax serve` to watch (#13410). One that
// changes is told as "asset" with its URL and SHA-256, fetched again by that hash and swapped.
function liveUrls() {
  const urls = new Set();
  for (const a of assets) {
    if (a.pack) { if (!a.pack.broker && !a.pack.away) urls.add(a.pack.url); }
    else if (!a.away) for (const u of a.urls || (a.url ? [a.url] : [])) urls.add(u);
  }
  return [...urls].filter((u) => u.startsWith("/") && !/\s/.test(u));
}
const liveHashes = new Map();   // URL -> the SHA-256 of its bytes here, as coax serve told it
function liveUrl(url) { const h = liveHashes.get(url); return h ? `${url}${url.includes("?") ? "&" : "?"}live=${h}` : url; }
// The old copy stays until the new one is accepted (#13441). A swap keeps each row's fields and, for
// a pack the reader holds, its group (unretired: its bytes back the rows), loads the new bytes, and
// only then lets the old go. A refusal (the fetch, the table, a digest the realm's Coax won't admit,
// a decode, the GPU) puts every field back and hands the old copy to the page again, so the world
// never stands without it, and throws the reason for the dev overlay.
const LIVE_KEPT = ["bytes", "size", "buffer", "texture", "dataTexture", "pageBytes", "pageDigest", "done", "failed"];
const liveKeep = (rows) => rows.map((a) => Object.fromEntries(LIVE_KEPT.map((f) => [f, a[f]])));
function liveBack(rows, kept) {
  rows.forEach((a, i) => { Object.assign(a, kept[i], { fetching: false }); a.live = null; });
  const r = gpu?.renderer;
  for (const a of rows) {
    if (r && a.bytes && a.kind !== "data" && a.kind !== "layers") r.acceptAsset(a);
    if (r && a.kind === "layers" && a.texture) r.layerTexture = a.texture;
    if (a.done) deliver(a);
  }
  if (gpu) gpu.sceneGroup = null;
  if (rows.some((a) => a.kind === "mesh")) bindMesh();
  bindScene();
}
// The realm's pack gate, asked again for bytes it admitted before (the refused ones were its last word).
async function liveReadmit(bytes) {
  const x = engine?.exports;
  if (!bytes || !x?.coaxial_pack_gate_version || x.coaxial_pack_gate_version() !== 1) return;
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  if (engine?.exports !== x) return;
  const at = x.coaxial_pack_digest(arena.state), capacity = x.coaxial_pack_digest_capacity();
  if (!Number.isInteger(at) || at <= 0 || capacity !== digest.length || at > memory.buffer.byteLength - capacity) return;
  new Uint8Array(memory.buffer, at, capacity).set(digest);
  x.coaxial_pack_admit(arena.state, bytes.length);
}
const LIVE_HELD = "\u0000held:";   // an old group's sounds step aside by name while the new pack's are read
async function liveAsset(url, hash) {
  if (liveHashes.get(url) === hash || !engine || !gpu) return false;
  const packs = new Set(assets.filter((a) => a.pack?.url === url).map((a) => a.pack));
  const rows = assets.filter((a) => !a.pack && (a.url === url || a.urls?.includes(url)));
  if (!packs.size && !rows.length) return false;
  const before = liveHashes.get(url), x = engine.exports, owner = gpu;
  const mine = () => engine?.exports === x && gpu === owner;   // a new engine or device loads its own copy
  const refuse = (reason) => {
    if (before) liveHashes.set(url, before); else liveHashes.delete(url);
    fetches.delete(url); packIncoming.delete(url);
    return new Error(`${url.split("/").pop()} not swapped: ${reason}\nthe copy from before stays`);
  };
  liveHashes.set(url, hash);
  for (const pack of packs) {
    const parts = assets.filter((a) => a.pack === pack), kept = liveKeep(parts);
    const old = packGroups.get(pack), source = pack.retainedSource;
    if (old) { packGroups.delete(pack); for (const sound of old.sounds) sound.name = LIVE_HELD + sound.name; }
    packIncoming.delete(pack.url);
    Object.assign(pack, { done: false, fetching: false, retainedSource: null, live: { reason: "", sounds: null } });
    for (const a of parts) Object.assign(a, { done: false, fetching: false, failed: false });
    let reason = "";
    try { await loadPack(pack); } catch (e) { reason = e.message || "did not load"; }
    const live = pack.live; pack.live = null;
    reason ||= live.reason || (pack.done ? "" : "the load stopped before it ended");
    if (!mine()) return false;
    if (!reason) {
      // accepted: now the old copy goes
      if (old) { old.retired = true; old.retiredResolve(); old.result?.close(); await packHost?.retire(old); for (const row of [...old.sounds]) { row.bytes = null; } }
      continue;
    }
    const added = packGroups.get(pack);
    if (added && added !== old) await retirePacks((group) => group === added);
    for (const sound of [...(live.sounds || []), ...(added && added !== old ? added.sounds : [])]) { sound.bytes = null; sound.name = LIVE_HELD + sound.name; }
    if (old) { for (const sound of old.sounds) sound.name = sound.name.slice(LIVE_HELD.length); packGroups.set(pack, old); }
    Object.assign(pack, { done: true, fetching: false, retainedSource: source });
    await liveReadmit(source?.bytes);
    if (!mine()) return false;
    liveBack(parts, kept);
    throw refuse(reason);
  }
  for (const a of rows) {
    const kept = liveKeep([a]);
    fetches.delete(a.url);
    Object.assign(a, { done: false, fetching: false, failed: false, pageBytes: null, live: { reason: "" } });
    let reason = "";
    try { await loadAsset(a); } catch (e) { reason = e.message || "did not load"; }
    reason ||= a.live?.reason || (a.failed ? "the page refused it" : a.done ? "" : "the load stopped before it ended");
    a.live = null;
    if (!mine()) return false;
    if (reason) { liveBack([a], kept); throw refuse(reason); }
  }
  return true;
}
// coaxial.liveSwaps counts what this tab swapped in, each after the frame that drew it
// (coax/web/live-reload-bench.coax measures an edit to its swap with them).
const liveSwaps = { wasm: 0, wgsl: 0, asset: 0 };
// (on an agent's clock the frames are the clock's and none comes by itself, so there a swap counts at once)
const swapped = (kind) => (agentMode ? Promise.resolve() : new Promise(requestAnimationFrame)).then(() => { liveSwaps[kind] += 1; globalThis.coaxial.liveSwaps = { ...liveSwaps }; });
function live() {
  const socket = new WebSocket(`ws://${location.host}/coaxial/live`);
  const mine = (path, url) => !path || path === new URL(url, location.href).pathname;
  socket.onopen = () => { const urls = liveUrls(); if (urls.length) socket.send(`watch ${urls.join(" ")}`); };
  socket.onmessage = async (event) => {
    const m = JSON.parse(event.data);
    const t0 = performance.now();
    if (m.kind === "page") { location.reload(); return; }
    if (m.kind === "error") { say(m.text, true); return; }
    // An edit was saved to a world on this dev server (#13413): the app, if it shows that world, asks for its new events.
    if (m.kind === "world" && typeof m.id === "string" && /^[a-z][a-z0-9-]{0,31}$/.test(m.id)) { if (engine) send(614, m.id); return; }
    // A message naming another page's module or shaders is not this tab's.
    if (m.kind === "wgsl" && gpu && (mine(m.path, wgslUrl) || assets.some((a) => a.url === m.path))) { if (await loadShaders()) { say(`shaders swapped in ${Math.round(performance.now() - t0)} ms`); swapped("wgsl"); } return; }
    if (m.kind === "asset" && typeof m.url === "string" && /^[0-9a-f]{64}$/.test(m.hash)) {
      try { if (await liveAsset(m.url, m.hash)) { say(`${m.url.split("/").pop()} swapped in ${Math.round(performance.now() - t0)} ms`); swapped("asset"); } }
      catch (e) { say(e.message, true); liveSwaps.failed = (liveSwaps.failed || 0) + 1; globalThis.coaxial.liveSwaps = { ...liveSwaps }; globalThis.coaxial.liveRefused = { url: m.url, hash: m.hash, reason: e.message }; }
      return;
    }
    if (m.kind === "wasm" && mine(m.path, wasmUrl)) {
      const generation = beginEngine();
      try {
        if (await fetchEngine(generation)) { say(`${app} rebuilt in ${m.ms} ms, swapped in ${Math.round(performance.now() - t0)} ms${stateSaid && `\n${stateSaid}`}`); swapped("wasm"); }
      } catch (e) { say(`${app}: ${e.message}`, true); }
    }
  };
  socket.onclose = () => setTimeout(live, 1000);
}

// A browser with no WebGPU adapter gets the page's text (the <main> every page carries), and above
// it one plain line saying why the page isn't drawn and what to do (#13153). Before this the reader
// saw only a title and a link to the page's Markdown. With no GPU, Coax can't draw it: it is part
// of the text fallback INVARIANTS.md allows, and check-visuals passes its lines marked so (#13155).
function noGpuNotice() {
  if (document.getElementById("coaxial-no-gpu")) return;
  const note = document.createElement("p"); // text fallback
  note.id = "coaxial-no-gpu"; note.setAttribute("role", "alert");
  note.style.cssText = "margin:0 0 16px;padding:12px 14px;border:1px solid #ff8c1f;border-radius:6px;color:#ededed;background:#1a1a1a"; // text fallback
  note.textContent = navigator.gpu
    ? "This page is drawn with WebGPU, and this browser has no graphics adapter for it. Use a current Chrome, Edge or Safari with hardware acceleration turned on. In Chrome on Linux, start it with --enable-unsafe-webgpu."
    : "This page is drawn with WebGPU, which this browser doesn't have. Use a current Chrome, Edge or Safari.";
  const text = document.querySelector("main") || document.body;
  text.prepend(note);
}

async function main() {
  try {
    loadMark("main");
    await fetchEngine(beginEngine());
    loadMark("engine-installed");
    // The world's public packs start coming in now, beside the font, the GPU and the shaders.
    if (!hostTransport && navigator.gpu) for (const pack of firstPacks) if (!pack.broker && !pack.urls && !pack.away && !packIncoming.has(pack.url)) packIncoming.set(pack.url, packBytes(pack));
    await loadPageFont();
    // No usable WebGPU after all: show the page's text, as written (a realm's loader says so).
    loading.step("gpu");
    const started = await startGpu();
    loadMark("gpu-started");
    if (!started) {
      for (const incoming of packIncoming.values()) incoming.cancel();packIncoming.clear();
      document.documentElement.classList.remove("gpu"); loading.fail("SHADERS FAILED TO COMPILE", !gpu);
      if (!gpu) noGpuNotice();
      lifeBeacon("load-fail", !navigator.gpu ? "no WebGPU" : !gpu ? "no GPU adapter" : "shaders failed to compile");
      return;
    }
    observeSize();
    if (!worldPaused) frameRequest = requestAnimationFrame(frame);
    // The heavy ones first; sounds are fetched when first played.
    loading.step("assets");
    for (const a of firstAssets) loadAsset(a);
    for (const pack of firstPacks) loadPack(pack);
  } catch (e) {
    document.documentElement.classList.remove("gpu");
    loading.fail(`${gpu || /gpu|adapter|device/i.test(e.message) ? "GPU" : "ENGINE"} FAILED: ${e.message}`);
    lifeBeacon("load-fail", `${gpu || /gpu|adapter|device/i.test(e.message) ? "GPU" : "ENGINE"} FAILED: ${e.message}`);
    say(`${app}: ${e.message}`, true);
  }
  // a clip is one pass: no live swap (a changed file's "wasm", "wgsl" or "page") mid-recording, which
  // restarted the engine and the film with it (the night raid video cut back to its start at 19.5 s)
  // An agent's run against the dev server (coax drive --live, ?agent&live) takes them too: it edits a source,
  // watches the swap and checks the result in one session (#13423).
  if (dev && (!clipping || (agentMode && new URLSearchParams(location.search).has("live")))) live();
}
main();
