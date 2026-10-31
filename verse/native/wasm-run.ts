// wasm-run: realm-run.c's frames from the browser's wasm instead, the same
// host steps in the same order (coax/web/host.js loadEngine, unpack and
// deliver, without the GPU), so the two can be compared line for line.
//   bun verse/native/wasm-run.ts REALM.wasm [realm-run options]
// Network imports are refused as realm-host.c refuses them;
// VERSE_NATIVE_CLOCK_MS fixes the clock for both.
import { readFileSync } from "node:fs";

const [wasmPath, ...rest] = process.argv.slice(2);
const opt: Record<string, string> = {};
for (let i = 0; i < rest.length; i++) {
  const a = rest[i];
  if (a === "--golden") opt.golden = "1";
  else opt[a.slice(2)] = rest[++i];
}
const frames = Number(opt.frames ?? 120), every = Number(opt.every ?? 30);
const [w, h] = (opt.size ?? "1280x720").split("x").map(Number);
const scale = Number(opt.scale ?? 1);

// coax/web/wasm.sh links with --max-memory=67108864 (1024 pages); the module asks for at least its own data.
const memory = new WebAssembly.Memory({ initial: 512, maximum: 1024 });
const clock = () => BigInt(process.env.VERSE_NATIVE_CLOCK_MS ?? Math.floor(performance.now()));
const env: Record<string, unknown> = {
  memory,
  coaxial_now_ms: clock,
  coaxial_now_seconds: () => clock() / 1000n,
  coaxial_http: () => -1,
  coaxial_http_bounded: () => -1,
  coaxial_http_cancel: () => 0,
  host_line: () => {}, host_dot: () => {}, host_slot: () => {},
};
const module = await WebAssembly.compile(readFileSync(wasmPath));
const imports: Record<string, Record<string, unknown>> = { env };
for (const imp of WebAssembly.Module.imports(module)) {
  if (imp.kind !== "function") continue;
  const into = (imports[imp.module] ||= {});
  if (into[imp.name]) continue;
  if (imp.module === "wasi_snapshot_preview1" && ["fd_fdstat_get", "fd_seek", "fd_close", "clock_time_get"].includes(imp.name)) into[imp.name] = () => 52;
  else if (imp.module === "wasi_snapshot_preview1" && imp.name === "fd_write") into[imp.name] = () => 0;
  else into[imp.name] = () => { throw new Error(`wasm-run: ${imp.module}.${imp.name} is not provided`); };
}
const instance = await WebAssembly.instantiate(module, imports);
const x = instance.exports as Record<string, any>;
x._initialize?.();
const align = (n: number, a: number) => Math.ceil(n / a) * a;
const heapBase = x.__heap_base?.value ?? 0;
const floats = x.coaxial_vertex_floats(), maxVertices = x.coaxial_max_vertices(), uniformFloats = x.coaxial_uniform_floats();
const base = align(Math.max(heapBase, memory.buffer.byteLength), 65536);
const stateCap = align(Math.max(x.coaxial_state_size(), 1024), 16);
const total = base + stateCap + maxVertices * floats * 4 + uniformFloats * 4 + 64;
if (total > memory.buffer.byteLength) memory.grow(Math.ceil((total - memory.buffer.byteLength) / 65536));
const state = base, vertices = base + stateCap, uniforms = vertices + maxVertices * floats * 4;
x.coaxial_state_init?.(state); // struct defaults, as the browser host applies them
if (opt.golden) x.coaxial_golden?.(1);

const bytesAt = (p: number, n: number) => new Uint8Array(memory.buffer, p, n);
const after = () => { if (x.coaxial_action(state)) x.coaxial_action_done(state); };
const sayAfter = () => {
  const a = x.coaxial_action(state);
  if (!a) return;
  const len = x.coaxial_outbox_len(state);
  console.log(`realm-host: action ${a}: ${new TextDecoder().decode(bytesAt(x.coaxial_outbox(state), Math.min(len, 200)).slice())}`);
  x.coaxial_action_done(state);
};
const send = (kind: number, text: Uint8Array) => {
  const b = text.subarray(0, x.coaxial_inbox_cap());
  bytesAt(x.coaxial_inbox(state), b.length).set(b);
  x.coaxial_message(state, kind, b.length);
  after();
};

// The pack, as realm-host.c rh_pack reads it.
const pack = readFileSync(opt.pack);
const v = new DataView(pack.buffer, pack.byteOffset, pack.length);
const count = v.getUint32(4, true), table = v.getUint32(16, true);
const entries = Array.from({ length: count }, (_, k) => {
  const at = table + k * 48;
  return { kind: pack[at], bytes: pack.subarray(v.getUint32(at + 4, true), v.getUint32(at + 4, true) + v.getUint32(at + 8, true)) };
});
const of = (kind: number) => entries.find((e) => e.kind === kind);
const layers = entries.filter((e) => e.kind === 4).length;
// --fact K=V adds one fact to the file's (a realm's data-ruleset, say)
if (opt.facts) send(600, new TextEncoder().encode(readFileSync(opt.facts, "utf8").replace(/\n+$/, "") + (opt.fact ? `\n${opt.fact}` : "")));
send(606, new Uint8Array());
send(605, new TextEncoder().encode(`${pack.length} ${pack.length}`));
const catalog = of(1)!;
const at = x.coaxial_asset(state, 0, catalog.bytes.length);
if (at) bytesAt(at, catalog.bytes.length).set(catalog.bytes);
[catalog.bytes.length, of(2)?.bytes.length ?? 0, of(3)?.bytes.length ?? 0, layers].forEach((n, i) => x.coaxial_asset_ready(state, i, n));
(opt.extra ? opt.extra.split(",").map(Number) : []).forEach((n, i) => x.coaxial_asset_ready(state, 4 + i, n));
console.log(`realm-run: ${count} entries in the pack (${layers} layers), ${floats} floats a vertex, ${uniformFloats} uniforms`);
// --hash PLACE: the page's location hash, as host.js sendHash delivers it once the engine is up
if (opt.hash && x.coaxial_hash) {
  const b = new TextEncoder().encode(opt.hash).subarray(0, x.coaxial_inbox_cap());
  bytesAt(x.coaxial_inbox(state), b.length).set(b);
  x.coaxial_hash(state, b.length);
  after();
}

const fnv = (b: Uint8Array) => {
  let hh = 0xcbf29ce484222325n;
  for (const c of b) hh = BigInt.asUintN(64, (hh ^ BigInt(c)) * 0x100000001b3n);
  return hh.toString(16).padStart(16, "0");
};
const keys = (opt.keys ?? "").split(",").filter(Boolean).map((k) => k.split(":").map(Number));
// --pointer F:X:Y,...: a click (down, up) at device pixel X, Y before frame F, as host.js's test hook clicks
const clicks = (opt.pointer ?? "").split(",").filter(Boolean).map((k) => k.split(":").map(Number));
let hud = 0;
for (let f = 1; f <= frames; f++) {
  for (const [kf, key, down] of keys) if (kf === f) { (down ? x.coaxial_key : x.coaxial_key_up)(state, key, 0); sayAfter(); }
  for (const [cf, cx, cy] of clicks) if (cf === f) for (const ph of [0, 2]) { x.coaxial_pointer(state, cx, cy, ph, 0, 1); sayAfter(); }
  let n = x.coaxial_frame(state, vertices, uniforms, w, h, scale, 1 / 60, 0) >>> 0;
  n = Math.min(n, maxVertices);
  hud = n;
  sayAfter();
  if (f % every && f !== frames) continue;
  const draws = x.coaxial_draw_count(state) >>> 0;
  let line = `frame ${f}: ${n} vertices ${draws} draws (${x.coaxial_water_draws(state) >>> 0} water) ${x.coaxial_grass_instances(state) >>> 0} blades ${x.coaxial_particle_instances(state) >>> 0} sparks`;
  line += ` v ${fnv(bytesAt(vertices, n * floats * 4))} u ${fnv(bytesAt(uniforms, uniformFloats * 4))}`;
  line += ` d ${fnv(draws ? bytesAt(x.coaxial_draws(state), draws * 16) : new Uint8Array())}`;
  for (let k = 0; k < x.coaxial_stores(); k++) {
    const bytes = (x.coaxial_store_bytes(state, k) >>> 0) & ~3;
    line += ` s${k} ${bytes} ${fnv(bytesAt(x.coaxial_store(state, k), bytes))}`;
  }
  line += ` p ${fnv(bytesAt(x.coaxial_particles(state), (x.coaxial_particle_bytes(state) >>> 0) & ~3))}`;
  console.log(line);
}
if (opt.dump) {
  const { writeFileSync } = await import("node:fs");
  writeFileSync(`${opt.dump}/u.bin`, bytesAt(uniforms, uniformFloats * 4));
  for (let k = 0; k < Math.min(4, x.coaxial_stores()); k++)
    writeFileSync(`${opt.dump}/s${k}.bin`, bytesAt(x.coaxial_store(state, k), (x.coaxial_store_bytes(state, k) >>> 0) & ~3));
  // what a pass check draws the scene from (verse/gpu/everglade-pass.coax): the draw list, how many of its
  // last draws are water, the grass's blades, the particles' quads, the frame's vertices and their floats, the pack's storage (vertices) and index buffers, and its layer images in order
  const draws = x.coaxial_draw_count(state) >>> 0;
  writeFileSync(`${opt.dump}/draws.bin`, draws ? bytesAt(x.coaxial_draws(state), draws * 16) : new Uint8Array(16));
  writeFileSync(`${opt.dump}/draws.txt`, `${draws} ${x.coaxial_water_draws ? Math.min(x.coaxial_water_draws(state) >>> 0, draws) : 0} ${x.coaxial_grass_instances?.(state) >>> 0} ${x.coaxial_particle_instances?.(state) >>> 0} ${hud} ${floats}\n`);
  // the frame's own vertices (the HUD, host.js's fx pipeline after post), at least 16 bytes
  const hudBytes = new Uint8Array(Math.max(16, hud * floats * 4));
  hudBytes.set(bytesAt(vertices, hud * floats * 4));
  writeFileSync(`${opt.dump}/hud.bin`, hudBytes);
  // the particles' puffs (host.js writes coaxial_particle_bytes of them each frame)
  if (x.coaxial_particles) writeFileSync(`${opt.dump}/puffs.bin`, bytesAt(x.coaxial_particles(state), Math.max(16, (x.coaxial_particle_bytes(state) >>> 0) & ~3)));
  if (of(2)) writeFileSync(`${opt.dump}/vertices.bin`, of(2)!.bytes);
  if (of(3)) writeFileSync(`${opt.dump}/index.bin`, of(3)!.bytes);
  const images = entries.filter((e) => e.kind === 4);
  images.forEach((e, i) => writeFileSync(`${opt.dump}/layer-${i}.img`, e.bytes));
  writeFileSync(`${opt.dump}/layers.bin.list`, images.map((_, i) => `layer-${i}.img\n`).join(""));
}
