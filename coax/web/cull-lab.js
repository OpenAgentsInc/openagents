// Firm reason: 1 (WebGPU driver for Coax-written WGSL: buffers, passes, indirect draws, Range fetches and readback; Coax owns the draw rule, culling, the DAG, page admission and residency).
// The GPU cull lab (GitHub #13008, #13051, Valley Lab step 6). It loads the fixture that
// metaverse-engine/tools/cull-fixture.coax writes (cluster DAGs from metaverse-engine/render/geometry/dag.coax, each
// scene's geometry a VPK3 file of one page a DAG page), runs the cull of
// metaverse-engine/render/geometry/cull.coax as /cull.wgsl and draws with /cull-draw.wgsl (both written by
// coax wgsl), and checks, headless:
//   lod        the GPU's draw-rule choice is dag.coax's, cluster for cluster, on every view;
//   textures   each texture's wanted mip after the page entries is the finest its drawn clusters need;
//   coverage   from inside the closed sphere every pixel is covered, levels mixed;
//   residency  from nothing, the GPU's requests drive residency.coax (build/cullstream.wasm); each
//              page is fetched by HTTP Range, admitted by its SHA-256 in Coax and copied into a GPU
//              slot on install, every frame covered; with every page's slot, the full choice;
//   eviction   with fewer slots than the views need, pages leave and come back into other slots,
//              every frame covered and each view's image the image of the same pages all resident;
//   shadow     a shadow view's requests are its casters' pages, weighted, and draw nothing;
//   occlusion  two-phase Hi-Z skips clusters behind the wall, and the image is the image without it.
// The report is window.cullLab.report; coax/web/cull-check.coax reads it.
const W = 960, H = 540, MIPS = 10, SLOT_TRIS = 640, SLOT_VERTS = 768, WEIGHT = 0.25;
const lab = (window.cullLab = { done: false, ok: false, report: {}, errors: [] });
const fail = (m) => { lab.errors.push(String(m)); };

function parse(buffer) {
  const v = new DataView(buffer); let at = 0;
  const u32 = () => { const x = v.getUint32(at, true); at += 4; return x; };
  if (u32() !== 0x314c5543 || u32() !== 2) throw new Error("not a CUL1 fixture, version 2");
  const scenes = [];
  for (let s = u32(), k = 0; k < s; k++) {
    const N = u32(), P = u32(), L = u32(), K = u32(), X = u32();
    const take = (n, Kind) => { const a = new Kind(buffer.slice(at, at + n * 4)); at += n * 4; return a; };
    const scene = { N, P, X, pages: `/build/cull-${k}.vpk3`, data: take(N * 16, Float32Array), info: take(N * 8, Uint32Array), links: take(L * 2, Uint32Array), views: [] };
    const words = (N + 31) >> 5;
    for (let j = 0; j < K; j++) {
      const f = take(8, Float32Array);
      scene.views.push({ eye: [f[0], f[1], f[2]], target: [f[3], f[4], f[5]], fov: f[6], t: f[7], drawn: take(words, Uint32Array), near: take(words, Uint32Array) });
    }
    scenes.push(scene);
  }
  return scenes;
}

// Column-major matrices, WebGPU depth 0..1.
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => { const l = Math.hypot(a[0], a[1], a[2]); return [a[0] / l, a[1] / l, a[2] / l]; };
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const mul = (P, V) => { const M = new Array(16).fill(0); for (let c = 0; c < 4; c++) for (let row = 0; row < 4; row++) for (let k = 0; k < 4; k++) M[c * 4 + row] += P[k * 4 + row] * V[c * 4 + k]; return M; };
function lookAt(eye, target) {
  const f = norm(sub(target, eye)); let up = [0, 1, 0];
  if (Math.abs(dot(f, up)) > 0.99) up = [0, 0, 1];
  const r = norm(cross(f, up)), u = cross(r, f), e = eye;
  return [r[0], u[0], -f[0], 0, r[1], u[1], -f[1], 0, r[2], u[2], -f[2], 0, -dot(r, e), -dot(u, e), dot(f, e), 1];
}
function viewProjection(view) {
  const n = 0.02, far = 400, t = 1 / Math.tan(view.fov / 2), a = W / H;
  return mul([t / a, 0, 0, 0, 0, t, 0, 0, 0, 0, far / (n - far), -1, 0, 0, (n * far) / (n - far), 0], lookAt(view.eye, view.target));
}
// The light's view: orthographic, a box of half-width `half` round `centre`, looking along `dir`.
function lightProjection(light) {
  const eye = light.centre.map((x, i) => x - light.dir[i] * light.depth / 2), n = 0, far = light.depth, s = 1 / light.half;
  return mul([s, 0, 0, 0, 0, s, 0, 0, 0, 0, 1 / (n - far), 0, 0, 0, n / (n - far), 1], lookAt(eye, light.centre));
}
function uniforms(scene, M, eye, t, hiz, levels, extra = {}) {
  const u = new Float32Array(92);
  u.set(M, 0); u.set(eye, 16); u[19] = t; u[20] = scene.N; u[21] = scene.P; u[22] = hiz ? 1 : 0; u[23] = levels.length;
  u[24] = W; u[25] = H; u[26] = levels[0].w; u[27] = levels[0].h;
  const row = (r) => [M[r], M[4 + r], M[8 + r], M[12 + r]];
  const add = (a, b, s) => a.map((x, i) => x + s * b[i]);
  const planes = [add(row(3), row(0), 1), add(row(3), row(0), -1), add(row(3), row(1), 1), add(row(3), row(1), -1), row(2), add(row(3), row(2), -1)];
  planes.forEach((p, k) => { const l = Math.hypot(p[0], p[1], p[2]); u.set(p.map((x) => x / l), 28 + k * 4); });
  levels.forEach((l, k) => { u[52 + k] = l.at; u[64 + k] = l.w; u[76 + k] = l.h; });
  u[88] = extra.shadowD ?? 0; u[89] = extra.weight ?? 1; u[90] = extra.pixel ?? 0; u[91] = extra.mips ?? 0;
  return u;
}
const pixelOf = (view) => (2 * Math.tan(view.fov / 2)) / H;

class Cull {
  // slots: GPU slots of SLOT_TRIS triangles and SLOT_VERTS vertices; pages are copied in by place().
  constructor(device, cullModule, drawModule, scene, slots) {
    this.device = device; this.scene = scene; this.slots = slots; const d = device, S = GPUBufferUsage;
    const buffer = (data, usage) => { const b = d.createBuffer({ size: Math.max(16, data.byteLength), usage: usage | S.COPY_DST }); d.queue.writeBuffer(b, 0, data); return b; };
    this.levels = []; let w = Math.ceil(W / 2), h = Math.ceil(H / 2), at = 0;
    while (this.levels.length < 12) { this.levels.push({ w, h, at }); at += w * h; if (w === 1 && h === 1) break; w = Math.ceil(w / 2); h = Math.ceil(h / 2); }
    const N = scene.N, R = scene.P + scene.X;
    this.u = d.createBuffer({ size: 92 * 4, usage: S.UNIFORM | S.COPY_DST });
    this.us = d.createBuffer({ size: 92 * 4, usage: S.UNIFORM | S.COPY_DST });
    this.data = buffer(scene.data, S.STORAGE); this.info = buffer(scene.info, S.STORAGE);
    this.tris = d.createBuffer({ size: slots * SLOT_TRIS * 12, usage: S.STORAGE | S.COPY_DST });
    this.verts = d.createBuffer({ size: slots * SLOT_VERTS * 16, usage: S.STORAGE | S.COPY_DST });
    this.installed = d.createBuffer({ size: Math.max(16, scene.P * 4), usage: S.STORAGE | S.COPY_DST });
    this.requests = d.createBuffer({ size: Math.max(16, R * 4), usage: S.STORAGE | S.COPY_SRC | S.COPY_DST });
    this.draws = d.createBuffer({ size: 64, usage: S.STORAGE | S.INDIRECT | S.COPY_SRC | S.COPY_DST });
    this.lists = d.createBuffer({ size: N * 16, usage: S.STORAGE | S.COPY_SRC });
    this.hiz = d.createBuffer({ size: at * 4, usage: S.STORAGE });
    this.hz = this.levels.map((_, l) => buffer(new Float32Array([l, 0, 0, 0]), S.UNIFORM));
    this.du = [0, N].map(() => d.createBuffer({ size: 80, usage: S.UNIFORM | S.COPY_DST }));
    this.du.base = [0, N];
    this.color = d.createTexture({ size: [W, H], format: "rgba8unorm", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
    this.depth = d.createTexture({ size: [W, H], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
    const compute = (entryPoint) => d.createComputePipeline({ layout: "auto", compute: { module: cullModule, entryPoint } });
    this.one = compute("cull_one"); this.two = compute("cull_two"); this.hizDepth = compute("hiz_depth"); this.hizDown = compute("hiz_down"); this.shadow = compute("cull_shadow");
    this.draw = d.createRenderPipeline({ layout: "auto", vertex: { module: drawModule, entryPoint: "cluster_vs" },
      fragment: { module: drawModule, entryPoint: "cluster_fs", targets: [{ format: "rgba8unorm" }] },
      primitive: { topology: "triangle-list", cullMode: "none" }, depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" } });
    const group = (pipeline, entries) => d.createBindGroup({ layout: pipeline.getBindGroupLayout(0),
      entries: entries.map(([binding, r]) => ({ binding, resource: r instanceof GPUTextureView ? r : { buffer: r } })) });
    const cullSet = [[0, this.u], [1, this.data], [2, this.info], [4, this.requests], [5, this.draws], [6, this.lists], [7, this.hiz]];
    this.oneGroup = group(this.one, [...cullSet, [3, this.installed]]);
    this.twoGroup = group(this.two, cullSet);
    this.shadowGroup = group(this.shadow, [[0, this.us], [1, this.data], [2, this.info], [3, this.installed], [4, this.requests]]);
    this.depthGroup = group(this.hizDepth, [[0, this.u], [7, this.hiz], [8, this.depth.createView()]]);
    this.downGroups = this.levels.map((_, l) => (l === 0 ? null : group(this.hizDown, [[0, this.u], [9, this.hz[l]], [7, this.hiz]])));
    this.drawGroups = this.du.map((b) => group(this.draw, [[0, b], [1, this.lists], [2, this.info], [3, this.tris], [4, this.verts]]));
    this.read = { requests: d.createBuffer({ size: Math.max(16, R * 4), usage: S.MAP_READ | S.COPY_DST }),
      draws: d.createBuffer({ size: 64, usage: S.MAP_READ | S.COPY_DST }), flags: d.createBuffer({ size: N * 4, usage: S.MAP_READ | S.COPY_DST }),
      image: d.createBuffer({ size: W * H * 4, usage: S.MAP_READ | S.COPY_DST }) };
  }
  // A placed page (cullstream.coax's cs_place outputs) into its slot: triangles, vertices, and each
  // of its clusters' first triangle in its record.
  place(x, memory, state, slot) {
    if (slot < 0 || slot >= this.slots) throw new Error("a slot past the pool: " + slot);
    const q = this.device.queue, nt = x.cs_ntri(state), nv = x.cs_nvert(state), nc = x.cs_nfirst(state);
    q.writeBuffer(this.tris, slot * SLOT_TRIS * 12, new Uint32Array(memory.buffer, x.cs_tri_out(state), nt * 3).slice());
    q.writeBuffer(this.verts, slot * SLOT_VERTS * 16, new Float32Array(memory.buffer, x.cs_vert_out(state), nv * 4).slice());
    const pairs = new Uint32Array(memory.buffer, x.cs_first_out(state), nc * 2).slice();
    for (let k = 0; k < nc; k++) q.writeBuffer(this.info, pairs[2 * k] * 32 + 8, new Uint32Array([pairs[2 * k + 1]]));
  }
  // One frame of view with installed pages; occlusion tests last frame's pyramid when hiz is set.
  // A light adds a shadow view's requests; main: false runs only that. Resolves to the requests,
  // counters, flags and (when image) the RGBA pixels.
  async frame(view, installed, { hiz = false, occlusion = true, image = false, light = null, weight = WEIGHT, main = true } = {}) {
    const d = this.device, q = d.queue, N = this.scene.N, R = this.scene.P + this.scene.X, groups = (n) => Math.ceil(n / 64);
    const M = viewProjection(view), u = uniforms(this.scene, M, view.eye, view.t, hiz && occlusion, this.levels, { pixel: pixelOf(view), mips: MIPS });
    q.writeBuffer(this.u, 0, u);
    if (light) q.writeBuffer(this.us, 0, uniforms(this.scene, lightProjection(light), view.eye, light.texel, false, this.levels, { shadowD: 1, weight }));
    q.writeBuffer(this.installed, 0, installed);
    q.writeBuffer(this.draws, 0, new Uint32Array([384, 0, 0, 0, 384, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    this.du.forEach((b, k) => { const x = new Float32Array(20); x.set(M, 0); x[16] = this.du.base[k]; q.writeBuffer(b, 0, x); });
    const e = d.createCommandEncoder();
    e.clearBuffer(this.requests);
    let p = e.beginComputePass();
    if (light) { p.setPipeline(this.shadow); p.setBindGroup(0, this.shadowGroup); p.dispatchWorkgroups(groups(N)); }
    if (main) { p.setPipeline(this.one); p.setBindGroup(0, this.oneGroup); p.dispatchWorkgroups(groups(N)); }
    p.end();
    const pass = (load, k) => {
      const r = e.beginRenderPass({ colorAttachments: [{ view: this.color.createView(), loadOp: load, storeOp: "store", clearValue: { r: 1, g: 0, b: 1, a: 1 } }],
        depthStencilAttachment: { view: this.depth.createView(), depthLoadOp: load, depthStoreOp: "store", depthClearValue: 1 } });
      r.setPipeline(this.draw); r.setBindGroup(0, this.drawGroups[k]); r.drawIndirect(this.draws, k * 16); r.end();
    };
    pass("clear", 0);
    p = e.beginComputePass();
    p.setPipeline(this.hizDepth); p.setBindGroup(0, this.depthGroup); p.dispatchWorkgroups(groups(this.levels[0].w * this.levels[0].h));
    for (let l = 1; l < this.levels.length; l++) { p.setPipeline(this.hizDown); p.setBindGroup(0, this.downGroups[l]); p.dispatchWorkgroups(groups(this.levels[l].w * this.levels[l].h)); }
    if (occlusion && main) { p.setPipeline(this.two); p.setBindGroup(0, this.twoGroup); p.dispatchWorkgroups(groups(N)); }
    p.end();
    pass("load", 1);
    e.copyBufferToBuffer(this.requests, 0, this.read.requests, 0, Math.max(16, R * 4));
    e.copyBufferToBuffer(this.draws, 0, this.read.draws, 0, 64);
    e.copyBufferToBuffer(this.lists, N * 12, this.read.flags, 0, N * 4);
    if (image) e.copyTextureToBuffer({ texture: this.color }, { buffer: this.read.image, bytesPerRow: W * 4 }, [W, H]);
    q.submit([e.finish()]);
    const take = async (b, Kind) => { await b.mapAsync(GPUMapMode.READ); const a = new Kind(b.getMappedRange().slice(0)); b.unmap(); return a; };
    const out = { requests: await take(this.read.requests, Uint32Array), draws: await take(this.read.draws, Uint32Array), flags: await take(this.read.flags, Uint32Array) };
    if (image) out.image = await take(this.read.image, Uint8Array);
    const c = out.draws;
    out.counts = { lod: c[9], frustum: c[10], hidden1: c[11], drawn1: c[1], retest: c[8], hidden2: c[12], drawn2: c[5] };
    return out;
  }
}
const uncovered = (px) => { let n = 0; for (let i = 0; i < px.length; i += 4) if (px[i] === 255 && px[i + 1] === 0 && px[i + 2] === 255) n++; return n; };
const differing = (a, b) => { let n = 0; for (let i = 0; i < a.length; i += 4) if (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]) n++; return n; };
const bit = (set, c) => (set[c >> 5] >>> (c & 31)) & 1;
// The GPU's choice against dag.coax's: clusters whose CF_LOD flag differs, outside the near set.
function compare(scene, view, flags) {
  let chosen = 0, expected = 0, near = 0, mismatched = 0;
  for (let c = 0; c < scene.N; c++) {
    const g = flags[c] & 1, x = bit(view.drawn, c);
    chosen += g; expected += x;
    if (bit(view.near, c)) { near++; continue; }
    if (g !== x) mismatched++;
  }
  return { chosen, expected, near, mismatched };
}
// The texture demand cull.coax's cl_mip gives the drawn clusters, in f64: per texture MIPS - the
// finest mip, and the textures whose wanted mip sits within 0.01% of a boundary (f32 may differ).
function demand(scene, view, flags) {
  const want = new Array(scene.X).fill(0), edge = new Array(scene.X).fill(false);
  for (let c = 0; c < scene.N; c++) {
    if (!(flags[c] & (8 | 16))) continue;
    const d = scene.data, o = c * 16, tex = d[o + 15];
    if (tex < 0) continue;
    const dist = Math.max(Math.hypot(view.eye[0] - d[o], view.eye[1] - d[o + 1], view.eye[2] - d[o + 2]) - d[o + 3], 0.0001);
    const f = d[o + 11] * dist * pixelOf(view), l = Math.log2(f);
    const mip = f < 2 ? 0 : Math.min(MIPS - 1, Math.floor(l));
    if (f >= 1 && Math.abs(l - Math.round(l)) < 1e-4) edge[tex] = true;
    want[tex] = Math.max(want[tex], MIPS - mip);
  }
  return { want, edge };
}

// build/cullstream.wasm (apps/openagents-web/cullstream.coax): its memory is imported, as host.js
// gives an engine's, and its states sit above its own data and stack, one a stream.
async function streams(count) {
  const r = await fetch("/build/cullstream.wasm"); if (!r.ok) throw new Error("cullstream.wasm " + r.status);
  const module = await WebAssembly.compile(await r.arrayBuffer());
  const memory = new WebAssembly.Memory({ initial: 128, maximum: 1024 }), imports = { env: { memory } };
  for (const imp of WebAssembly.Module.imports(module)) {
    if (imp.kind !== "function") continue;
    (imports[imp.module] ||= {})[imp.name] ||= () => { throw new Error("cullstream.wasm called " + imp.module + "." + imp.name); };
  }
  let instance;
  for (let pages = 128; ; pages *= 2) {
    try { instance = await WebAssembly.instantiate(module, imports); break; }
    catch (e) { if (pages >= 1024) throw e; memory.grow(pages); }
  }
  const x = instance.exports; x._initialize?.();
  const base = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536, size = Math.ceil(x.cs_state_size() / 65536) * 65536;
  const need = base + size * count + 65536;
  if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
  const states = []; for (let k = 0; k < count; k++) { states.push(base + k * size); x.cs_init(base + k * size); }
  return { x, memory, states };
}
// HTTP Range: the bytes at..at + n - 1 of url, which must come back as 206 with exactly them.
const ranges = { fetches: 0, bytes: 0 };
async function range(url, at, n) {
  const r = await fetch(url, { headers: { Range: `bytes=${at}-${at + n - 1}` }, cache: "no-store" });
  if (r.status !== 206) throw new Error(`${url}: a Range request came back ${r.status}`);
  const b = new Uint8Array(await r.arrayBuffer());
  if (b.length !== n) throw new Error(`${url}: ${b.length} bytes for a range of ${n}`);
  ranges.fetches++; ranges.bytes += n;
  return b;
}
// A scene's VPK3 manifest into a stream: the header's first bytes, then the manifest it names.
async function openPack(w, state, scene) {
  const io = (b) => { if (b.length > w.x.cs_io_size()) throw new Error("past the stream's buffer"); new Uint8Array(w.memory.buffer, w.x.cs_io(state), b.length).set(b); };
  io(await range(scene.pages, 0, 96));
  const need = w.x.cs_need(state, 96); if (need < 96) throw new Error(scene.pages + ": not a VPK3 file");
  io(await range(scene.pages, 0, need));
  if (!w.x.cs_pack(state, need)) throw new Error(scene.pages + ": the manifest was refused");
}
// Page k's stored range into the stream, admitted there (track: the residency hears of it).
async function fetchPage(w, state, scene, k, track) {
  const n = w.x.cs_page_stored(state, k), b = await range(scene.pages, w.x.cs_page_at(state, k), n);
  new Uint8Array(w.memory.buffer, w.x.cs_io(state), n).set(b);
  return w.x.cs_arrive(state, k, n, track ? 1 : 0) === 1;
}
// Every page fetched, admitted and placed in the slot of its own number: the all-resident cull.
async function loadAll(w, state, scene, cull) {
  await openPack(w, state, scene);
  for (let k = 0; k < scene.P; k++) {
    if (!(await fetchPage(w, state, scene, k, false)) || !w.x.cs_place_page(state, k, k)) throw new Error(`${scene.pages}: page ${k} was refused`);
    cull.place(w.x, w.memory, state, k);
  }
}
// Streaming frames of view from a stream whose residency is open: the GPU's requests plan Range
// fetches, which land before the next frame (one frame's latency), and installs are placed.
async function streamFrames(w, state, scene, cull, view, frames, onFrame) {
  const { x, memory } = w;
  for (let f = 0; f < frames; f++) {
    const installed = new Uint32Array(new Uint32Array(memory.buffer, x.cs_installed(state), scene.P));
    const r = await cull.frame(view, installed, { hiz: f > 0, image: true });
    new BigInt64Array(memory.buffer, x.cs_requests(state), scene.P).set(Array.from(r.requests.subarray(0, scene.P), BigInt));  // Coax's int is 64-bit
    const n = x.cs_frame(state);
    for (let i = 0; i < n; i++) { if (!x.cs_place(state, i)) throw new Error(`install ${i} of page ${x.cs_done_page(state, i)} has no bytes`); cull.place(x, memory, state, x.cs_done_slot(state, i)); }
    const want = []; for (let i = 0; i < x.cs_fetches(state); i++) want.push(x.cs_fetch(state, i));
    for (const k of want) if (!(await fetchPage(w, state, scene, k, true))) fail(`residency: page ${k} was refused`);
    await onFrame(f, installed, r, n);
  }
}

async function run() {
  if (!navigator.gpu) throw new Error("no WebGPU");
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  const device = await adapter.requestDevice();
  device.addEventListener("uncapturederror", (e) => fail("GPU: " + e.error.message));
  const text = async (url) => { const r = await fetch(url); if (!r.ok) throw new Error(url + " " + r.status); return r.text(); };
  const shader = async (url) => {
    const m = device.createShaderModule({ code: await text(url) }); const info = await m.getCompilationInfo();
    const errors = info.messages.filter((x) => x.type === "error"); if (errors.length) throw new Error(url + ": " + errors.map((x) => x.lineNum + ": " + x.message).join("; "));
    return m;
  };
  const [cullModule, drawModule, fixture, w] = await Promise.all([shader("/cull.wgsl"), shader("/cull-draw.wgsl"),
    fetch("/build/cull-fixture.bin").then((r) => { if (!r.ok) throw new Error("fixture " + r.status); return r.arrayBuffer(); }),
    streams(4)]);
  const scenes = parse(fixture), report = lab.report;
  report.adapter = adapter.info ? `${adapter.info.vendor} ${adapter.info.architecture}` : "";
  // every page of both scenes by its range, admitted and placed: the all-resident culls
  const cull = scenes.map((s) => new Cull(device, cullModule, drawModule, s, s.P));
  for (let s = 0; s < scenes.length; s++) await loadAll(w, w.states[s], scenes[s], cull[s]);
  report.load = { fetches: ranges.fetches, bytes: ranges.bytes, refused: w.x.cs_refused(w.states[0]) + w.x.cs_refused(w.states[1]) };
  const all = (s) => new Uint32Array(Math.max(4, s.P)).fill(1);
  // lod and textures: every view of both scenes, every page in, no occlusion
  report.lod = []; report.textures = [];
  for (let s = 0; s < scenes.length; s++) for (let v = 0; v < scenes[s].views.length; v++) {
    const scene = scenes[s], view = scene.views[v], r = await cull[s].frame(view, all(scene), { occlusion: false });
    const m = compare(scene, view, r.flags); report.lod.push({ scene: s, view: v, ...m });
    if (m.mismatched) fail(`lod: scene ${s} view ${v}: ${m.mismatched} clusters differ from dag.coax`);
    if (m.chosen === 0) fail(`lod: scene ${s} view ${v} draws nothing`);
    const x = demand(scene, view, r.flags), got = Array.from(r.requests.subarray(scene.P, scene.P + scene.X));
    report.textures.push({ scene: s, view: v, wanted: got, expected: x.want });
    got.forEach((g, t) => { if (g !== x.want[t] && !(x.edge[t] && Math.abs(g - x.want[t]) === 1)) fail(`textures: scene ${s} view ${v} texture ${t}: the GPU wants ${g}, cl_mip gives ${x.want[t]}`); });
  }
  if (!report.textures.some((row) => new Set(row.wanted.filter((g) => g > 0)).size > 1)) fail("textures: no view wants two different mips");
  // coverage: inside the shell, every page in, occlusion on from the second frame
  report.coverage = [];
  for (const v of [0, 1, 2, 6, 7]) {
    const view = scenes[0].views[v];
    for (let f = 0; f < 3; f++) {
      const r = await cull[0].frame(view, all(scenes[0]), { hiz: f > 0, image: true });
      const levels = new Set(); for (let c = 0; c < scenes[0].N; c++) if (r.flags[c] & 1) levels.add(scenes[0].info[c * 8 + 6]);
      const row = { view: v, frame: f, uncovered: uncovered(r.image), levels: [...levels].sort(), ...r.counts };
      report.coverage.push(row);
      if (row.uncovered) fail(`coverage: view ${v} frame ${f}: ${row.uncovered} pixels uncovered`);
    }
  }
  if (!report.coverage.some((row) => row.levels.length > 1)) fail("coverage: no view from inside mixes levels");
  // residency and eviction: from nothing, streamed, with a slot for every page and then with 12
  const s0 = scenes[0], inside = new Set([0, 1, 2, 6, 7]);
  const residency = async (name, state, slots, plan, full) => {
    const { x, memory } = w;
    await openPack(w, state, s0);
    new BigInt64Array(memory.buffer, x.cs_links(state), s0.links.length).set(Array.from(s0.links, BigInt));
    if (!x.cs_open(state, s0.P, slots, s0.links.length / 2)) throw new Error("residency refused the DAG's pages");
    const streamed = new Cull(device, cullModule, drawModule, s0, slots), out = { pages: s0.P, slots, frames: [], views: [] };
    for (const [v, frames] of plan) {
      const view = s0.views[v]; let stable = -1;
      await streamFrames(w, state, s0, streamed, view, frames, async (f, installed, r, n) => {
        const held = installed.reduce((a, b) => a + b, 0), m = compare(s0, view, r.flags);
        const row = { view: v, frame: f, installed: held, installsNow: n, drawn: r.counts.drawn1 + r.counts.drawn2, lod: r.counts.lod, mismatched: m.mismatched, uncovered: held ? uncovered(r.image) : null, counts: [0, 1, 2, 3, 4, 5].map((k) => x.cs_count(state, k)), wants: Array.from(r.requests.subarray(0, s0.P)).filter((q) => q >= 2).length };
        out.frames.push(row);
        if (inside.has(v) && held && row.uncovered) fail(`${name}: view ${v} frame ${f}: ${row.uncovered} pixels uncovered with ${held} pages`);
        if (!x.cs_valid(state)) fail(`${name}: view ${v} frame ${f}: the residency broke its rules`);
        if (stable < 0 && m.mismatched === 0) stable = f;
      });
      // the streamed slots draw what the same pages draw all resident (the flags as the last frame's installs left them)
      const now = new Uint32Array(new Uint32Array(memory.buffer, x.cs_installed(state), s0.P));
      const a = await streamed.frame(view, now, { occlusion: false, image: true }), b = await cull[0].frame(view, now, { occlusion: false, image: true });
      const diff = differing(a.image, b.image);
      out.views.push({ view: v, stable, differing: diff });
      if (diff) fail(`${name}: view ${v}: ${diff} pixels differ from the same pages all resident`);
      if (full && stable < 0) fail(`${name}: view ${v} never reached dag.coax's full choice`);
    }
    out.installs = x.cs_installs(state); out.evictions = x.cs_evictions(state); out.refused = x.cs_refused(state); out.fetchedBytes = x.cs_fetched(state);
    if (out.refused) fail(`${name}: ${out.refused} pages refused`);
    return out;
  };
  report.residency = await residency("residency", w.states[2], s0.P, [[0, 24], [6, 24], [3, 24]], true);
  // the shell's 30 pages: 16 shared ancestors and 14 leaf pages. Views 5 and 7 each need 26 (each
  // leaves out leaves the other needs), 28 together; with 26 slots, every change of view evicts the
  // other's leaves into the slots the next needs, and each still reaches its full choice
  report.eviction = await residency("eviction", w.states[3], 26, [[5, 20], [7, 20], [4, 12], [5, 20], [7, 20]], true);
  if (report.eviction.evictions < 4) fail(`eviction: 26 slots evicted ${report.eviction.evictions} pages, fewer than the views make room for`);
  // shadow: a light over the whole shell, from inside it (where the camera's frustum leaves half out)
  {
    const view = s0.views[0], light = { centre: [0, 0, 0], dir: norm([0.4, -1, 0.3]), half: 3.5, depth: 12, texel: 7 / 1024 }, every = all(s0);
    const camera = await cull[0].frame(view, every, { occlusion: false });
    const lit = await cull[0].frame(view, every, { occlusion: false, light, main: false });
    const full = await cull[0].frame(view, every, { occlusion: false, light, main: false, weight: 1 });
    const both = await cull[0].frame(view, every, { occlusion: false, light });
    let only = 0, mixed = 0, weighed = 0;
    for (let p = 0; p < s0.P; p++) {
      const m = camera.requests[p], s = lit.requests[p], f = full.requests[p], b = both.requests[p];
      if (b !== Math.max(m, s)) mixed++;
      if (f >= 2 && Math.abs(s - (2 + Math.floor((f - 2) * WEIGHT))) > 1) weighed++;
      if (f < 2 && s !== f) weighed++;
      if (s >= 2 && m <= 1) only++;
    }
    report.shadow = { only, mixed, weighed, camera: camera.counts, both: both.counts, lit: lit.counts };
    if (mixed) fail(`shadow: ${mixed} pages' requests aren't the larger of the camera's and the light's`);
    if (weighed) fail(`shadow: ${weighed} pages' shadow requests aren't weighted by ${WEIGHT}`);
    if (!only) fail("shadow: no page is wanted by the light alone");
    if (JSON.stringify(both.counts) !== JSON.stringify(camera.counts)) fail("shadow: the shadow view changed what the camera draws");
    if (lit.counts.drawn1 || lit.counts.lod) fail("shadow: the shadow view alone drew something");
  }
  // occlusion: behind the wall, against the image without occlusion
  report.occlusion = [];
  for (let v = 0; v < scenes[1].views.length; v++) {
    const view = scenes[1].views[v], every = all(scenes[1]);
    const plain = await cull[1].frame(view, every, { occlusion: false, image: true });
    await cull[1].frame(view, every, { hiz: false, image: false });
    const r = await cull[1].frame(view, every, { hiz: true, image: true });
    const row = { view: v, ...r.counts, differing: differing(plain.image, r.image), plainDrawn: plain.counts.drawn1 };
    report.occlusion.push(row);
    if (row.differing) fail(`occlusion: view ${v}: ${row.differing} pixels differ from the image without occlusion`);
    if (row.hidden2 === 0) fail(`occlusion: view ${v} skipped nothing`);
    if (row.drawn1 + row.drawn2 + row.hidden2 + row.frustum !== row.lod) fail(`occlusion: view ${v}: counts don't add up`);
  }
  // timing: frames to the GPU's completion, with and without occlusion
  const time = async (occlusion) => {
    const view = scenes[1].views[0], every = all(scenes[1]); await cull[1].frame(view, every, { occlusion, hiz: false });
    const t0 = performance.now(); for (let k = 0; k < 30; k++) await cull[1].frame(view, every, { occlusion, hiz: true });
    return (performance.now() - t0) / 30;
  };
  report.timing = { occlusionMs: await time(true), plainMs: await time(false), note: "wall time a frame, readback included" };
  lab.ok = lab.errors.length === 0;
}
run().catch((e) => fail(e && e.stack ? e.stack : e)).finally(() => { lab.report.errors = lab.errors; lab.done = true; const el = document.getElementById("cull-report"); if (el) el.textContent = JSON.stringify(lab.report, null, 1); });
