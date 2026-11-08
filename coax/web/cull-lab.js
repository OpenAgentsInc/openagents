// Firm reason: 1 (WebGPU driver for Coax-written WGSL: buffers, passes, indirect draws and readback; Coax owns the draw rule, culling, the DAG and residency).
// The GPU cull lab (GitHub #13008, Valley Lab step 6). It loads the fixture that
// aethyr/tools/cull-fixture.coax writes (cluster DAGs from aethyr/render/geometry/dag.coax), runs
// the cull of aethyr/render/geometry/cull.coax as /cull.wgsl and draws with /cull-draw.wgsl
// (both written by coax wgsl), and checks, headless:
//   lod        the GPU's draw-rule choice is dag.coax's, cluster for cluster, on every view;
//   coverage   from inside the closed sphere every pixel is covered, levels mixed;
//   residency  starting from nothing, the GPU's requests drive residency.coax (build/cullstream.wasm)
//              until the view's full choice is drawn, every frame covered;
//   occlusion  two-phase Hi-Z skips clusters behind the wall, and the image is the image without it.
// The report is window.cullLab.report; coax/web/cull-check.coax reads it.
const W = 960, H = 540, MAGENTA = [255, 0, 255, 255];
const lab = (window.cullLab = { done: false, ok: false, report: {}, errors: [] });
const fail = (m) => { lab.errors.push(String(m)); };

function parse(buffer) {
  const v = new DataView(buffer); let at = 0;
  const u32 = () => { const x = v.getUint32(at, true); at += 4; return x; };
  if (u32() !== 0x314c5543 || u32() !== 1) throw new Error("not a CUL1 fixture");
  const scenes = [];
  for (let s = u32(); s > 0; s--) {
    const V = u32(), T = u32(), N = u32(), P = u32(), L = u32(), K = u32();
    const take = (n, Kind) => { const a = new Kind(buffer.slice(at, at + n * 4)); at += n * 4; return a; };
    const scene = { V, T, N, P, verts: take(V * 4, Float32Array), tris: take(T * 3, Uint32Array), data: take(N * 16, Float32Array),
      info: take(N * 8, Uint32Array), links: take(L * 2, Uint32Array), views: [] };
    const words = (N + 31) >> 5;
    for (let k = 0; k < K; k++) {
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
function viewProjection(view) {
  const f = norm(sub(view.target, view.eye)); let up = [0, 1, 0];
  if (Math.abs(dot(f, up)) > 0.99) up = [0, 0, 1];
  const r = norm(cross(f, up)), u = cross(r, f), e = view.eye;
  const V = [r[0], u[0], -f[0], 0, r[1], u[1], -f[1], 0, r[2], u[2], -f[2], 0, -dot(r, e), -dot(u, e), dot(f, e), 1];
  const n = 0.02, far = 400, t = 1 / Math.tan(view.fov / 2), a = W / H;
  const P = [t / a, 0, 0, 0, 0, t, 0, 0, 0, 0, far / (n - far), -1, 0, 0, (n * far) / (n - far), 0];
  const M = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let row = 0; row < 4; row++) for (let k = 0; k < 4; k++) M[c * 4 + row] += P[k * 4 + row] * V[c * 4 + k];
  return M;
}
function uniforms(scene, view, hiz, levels) {
  const u = new Float32Array(88), M = viewProjection(view);
  u.set(M, 0); u.set(view.eye, 16); u[19] = view.t; u[20] = scene.N; u[21] = scene.P; u[22] = hiz ? 1 : 0; u[23] = levels.length;
  u[24] = W; u[25] = H; u[26] = levels[0].w; u[27] = levels[0].h;
  const row = (r) => [M[r], M[4 + r], M[8 + r], M[12 + r]];
  const add = (a, b, s) => a.map((x, i) => x + s * b[i]);
  const planes = [add(row(3), row(0), 1), add(row(3), row(0), -1), add(row(3), row(1), 1), add(row(3), row(1), -1), row(2), add(row(3), row(2), -1)];
  planes.forEach((p, k) => { const l = Math.hypot(p[0], p[1], p[2]); u.set(p.map((x) => x / l), 28 + k * 4); });
  levels.forEach((l, k) => { u[52 + k] = l.at; u[64 + k] = l.w; u[76 + k] = l.h; });
  return u;
}

class Cull {
  constructor(device, cullModule, drawModule, scene) {
    this.device = device; this.scene = scene; const d = device, S = GPUBufferUsage;
    const buffer = (data, usage) => { const b = d.createBuffer({ size: Math.max(16, data.byteLength), usage: usage | S.COPY_DST }); d.queue.writeBuffer(b, 0, data); return b; };
    this.levels = []; let w = Math.ceil(W / 2), h = Math.ceil(H / 2), at = 0;
    while (this.levels.length < 12) { this.levels.push({ w, h, at }); at += w * h; if (w === 1 && h === 1) break; w = Math.ceil(w / 2); h = Math.ceil(h / 2); }
    const N = scene.N;
    this.u = d.createBuffer({ size: 88 * 4, usage: S.UNIFORM | S.COPY_DST });
    this.data = buffer(scene.data, S.STORAGE); this.info = buffer(scene.info, S.STORAGE);
    this.tris = buffer(scene.tris, S.STORAGE); this.verts = buffer(scene.verts, S.STORAGE);
    this.installed = d.createBuffer({ size: Math.max(16, scene.P * 4), usage: S.STORAGE | S.COPY_DST });
    this.requests = d.createBuffer({ size: Math.max(16, scene.P * 4), usage: S.STORAGE | S.COPY_SRC | S.COPY_DST });
    this.draws = d.createBuffer({ size: 64, usage: S.STORAGE | S.INDIRECT | S.COPY_SRC | S.COPY_DST });
    this.lists = d.createBuffer({ size: N * 16, usage: S.STORAGE | S.COPY_SRC });
    this.hiz = d.createBuffer({ size: at * 4, usage: S.STORAGE });
    this.hz = this.levels.map((_, l) => buffer(new Float32Array([l, 0, 0, 0]), S.UNIFORM));
    this.du = [0, N].map((base) => d.createBuffer({ size: 80, usage: S.UNIFORM | S.COPY_DST }));
    this.du.base = [0, N];
    this.color = d.createTexture({ size: [W, H], format: "rgba8unorm", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
    this.depth = d.createTexture({ size: [W, H], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
    const compute = (entryPoint) => d.createComputePipeline({ layout: "auto", compute: { module: cullModule, entryPoint } });
    this.one = compute("cull_one"); this.two = compute("cull_two"); this.hizDepth = compute("hiz_depth"); this.hizDown = compute("hiz_down");
    this.draw = d.createRenderPipeline({ layout: "auto", vertex: { module: drawModule, entryPoint: "cluster_vs" },
      fragment: { module: drawModule, entryPoint: "cluster_fs", targets: [{ format: "rgba8unorm" }] },
      primitive: { topology: "triangle-list", cullMode: "none" }, depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" } });
    const group = (pipeline, entries) => d.createBindGroup({ layout: pipeline.getBindGroupLayout(0),
      entries: entries.map(([binding, r]) => ({ binding, resource: r instanceof GPUTextureView ? r : { buffer: r } })) });
    const cullSet = [[0, this.u], [1, this.data], [2, this.info], [4, this.requests], [5, this.draws], [6, this.lists], [7, this.hiz]];
    this.oneGroup = group(this.one, [...cullSet, [3, this.installed]]);
    this.twoGroup = group(this.two, cullSet);
    this.depthGroup = group(this.hizDepth, [[0, this.u], [7, this.hiz], [8, this.depth.createView()]]);
    this.downGroups = this.levels.map((_, l) => (l === 0 ? null : group(this.hizDown, [[0, this.u], [9, this.hz[l]], [7, this.hiz]])));
    this.drawGroups = this.du.map((b) => group(this.draw, [[0, b], [1, this.lists], [2, this.info], [3, this.tris], [4, this.verts]]));
    this.read = { requests: d.createBuffer({ size: Math.max(16, scene.P * 4), usage: S.MAP_READ | S.COPY_DST }),
      draws: d.createBuffer({ size: 64, usage: S.MAP_READ | S.COPY_DST }), flags: d.createBuffer({ size: N * 4, usage: S.MAP_READ | S.COPY_DST }),
      image: d.createBuffer({ size: W * H * 4, usage: S.MAP_READ | S.COPY_DST }) };
  }
  // One frame of view with installed pages; occlusion tests last frame's pyramid when hiz is set.
  // Resolves to the requests, counters, flags and (when image) the RGBA pixels.
  async frame(view, installed, { hiz = false, occlusion = true, image = false } = {}) {
    const d = this.device, q = d.queue, N = this.scene.N, groups = (n) => Math.ceil(n / 64);
    const u = uniforms(this.scene, view, hiz && occlusion, this.levels);
    q.writeBuffer(this.u, 0, u);
    q.writeBuffer(this.installed, 0, installed);
    q.writeBuffer(this.draws, 0, new Uint32Array([384, 0, 0, 0, 384, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    const M = u.slice(0, 16);
    this.du.forEach((b, k) => { const x = new Float32Array(20); x.set(M, 0); x[16] = this.du.base[k]; q.writeBuffer(b, 0, x); });
    const e = d.createCommandEncoder();
    e.clearBuffer(this.requests);
    let p = e.beginComputePass(); p.setPipeline(this.one); p.setBindGroup(0, this.oneGroup); p.dispatchWorkgroups(groups(N)); p.end();
    const pass = (load, k) => {
      const r = e.beginRenderPass({ colorAttachments: [{ view: this.color.createView(), loadOp: load, storeOp: "store", clearValue: { r: 1, g: 0, b: 1, a: 1 } }],
        depthStencilAttachment: { view: this.depth.createView(), depthLoadOp: load, depthStoreOp: "store", depthClearValue: 1 } });
      r.setPipeline(this.draw); r.setBindGroup(0, this.drawGroups[k]); r.drawIndirect(this.draws, k * 16); r.end();
    };
    pass("clear", 0);
    p = e.beginComputePass();
    p.setPipeline(this.hizDepth); p.setBindGroup(0, this.depthGroup); p.dispatchWorkgroups(groups(this.levels[0].w * this.levels[0].h));
    for (let l = 1; l < this.levels.length; l++) { p.setPipeline(this.hizDown); p.setBindGroup(0, this.downGroups[l]); p.dispatchWorkgroups(groups(this.levels[l].w * this.levels[l].h)); }
    if (occlusion) { p.setPipeline(this.two); p.setBindGroup(0, this.twoGroup); p.dispatchWorkgroups(groups(N)); }
    p.end();
    pass("load", 1);
    e.copyBufferToBuffer(this.requests, 0, this.read.requests, 0, Math.max(16, this.scene.P * 4));
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

// build/cullstream.wasm (apps/openagents-web/cullstream.coax): its memory is imported, as
// host.js gives an engine's, and its state sits above its own data and stack.
async function residency() {
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
  const base = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536;
  const need = base + x.cs_state_size() + 65536;
  if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
  return { x, memory, state: base };
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
  const [cullModule, drawModule, fixture, wasm] = await Promise.all([shader("/cull.wgsl"), shader("/cull-draw.wgsl"),
    fetch("/build/cull-fixture.bin").then((r) => { if (!r.ok) throw new Error("fixture " + r.status); return r.arrayBuffer(); }),
    residency()]);
  const scenes = parse(fixture), report = lab.report;
  report.adapter = adapter.info ? `${adapter.info.vendor} ${adapter.info.architecture}` : "";
  const cull = scenes.map((s) => new Cull(device, cullModule, drawModule, s));
  const all = (s) => new Uint32Array(Math.max(4, s.P)).fill(1);
  // lod: every view of both scenes, every page in, no occlusion
  report.lod = [];
  for (let s = 0; s < scenes.length; s++) for (let v = 0; v < scenes[s].views.length; v++) {
    const r = await cull[s].frame(scenes[s].views[v], all(scenes[s]), { occlusion: false });
    const m = compare(scenes[s], scenes[s].views[v], r.flags); report.lod.push({ scene: s, view: v, ...m });
    if (m.mismatched) fail(`lod: scene ${s} view ${v}: ${m.mismatched} clusters differ from dag.coax`);
    if (m.chosen === 0) fail(`lod: scene ${s} view ${v} draws nothing`);
  }
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
  // residency: from nothing, the GPU's requests drive residency.coax
  const { x, memory, state } = wasm;
  x.cs_init(state);
  const s0 = scenes[0], links = s0.links;
  new BigInt64Array(memory.buffer, x.cs_links(state), links.length).set(Array.from(links, BigInt));  // Coax's int is 64-bit
  const slots = Math.min(32, s0.P);
  if (!x.cs_open(state, s0.P, slots, links.length / 2)) throw new Error("residency refused the DAG's pages");
  report.residency = { pages: s0.P, slots, frames: [] };
  let converged = {};
  for (const [v, frames] of [[0, 24], [6, 24], [3, 24]]) {
    const view = s0.views[v]; let stable = -1;
    for (let f = 0; f < frames; f++) {
      const installed = new Uint32Array(new Uint32Array(memory.buffer, x.cs_installed(state), s0.P));
      const r = await cull[0].frame(view, installed, { hiz: f > 0, image: true });
      new BigInt64Array(memory.buffer, x.cs_requests(state), s0.P).set(Array.from(r.requests.subarray(0, s0.P), BigInt));
      const n = x.cs_frame(state);
      const held = installed.reduce((a, b) => a + b, 0), m = compare(s0, view, r.flags);
      const row = { view: v, frame: f, installed: held, installsNow: n, drawn: r.counts.drawn1 + r.counts.drawn2, lod: r.counts.lod, mismatched: m.mismatched, uncovered: held ? uncovered(r.image) : null };
      report.residency.frames.push(row);
      if (v !== 3 && held && row.uncovered) fail(`residency: view ${v} frame ${f}: ${row.uncovered} pixels uncovered with ${held} pages`);
      if (!x.cs_valid(state)) fail(`residency: view ${v} frame ${f}: the residency broke its rules`);
      if (stable < 0 && m.mismatched === 0) stable = f;
    }
    converged[v] = stable;
    if (stable < 0) fail(`residency: view ${v} never reached dag.coax's full choice`);
  }
  report.residency.converged = converged; report.residency.installs = x.cs_installs(state); report.residency.evictions = x.cs_evictions(state);
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
