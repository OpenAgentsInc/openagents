// Firm reason: 1 (browser WebGPU: the compute passes and indirect draws for Coax's cull entries; which pieces are drawn is Coax's, everglade/client/gpu/everglade2-cull.coax).
// Instance culling (GitHub #13050), for a page whose data-assets name a `cull:` module and whose
// face exports coaxial_cull_v1. The page still picks its pieces into runs of a store (one
// instanced draw a run); this pass leaves out the ones the frustum or the frame's own depth hide,
// in two phases (metaverse-engine/render/geometry/cull.coax, learned from Solstice's shaders/vg/cull.wgsl):
//   frame()   the runs' pieces against the frustum and last frame's depth pyramid; what passes is
//             copied to the run's first region and counted into its first indirect draw
//   draw()    a run's first indirect draw, in place of its instanced draw
//   late()    after the opaque draws: the pyramid of that depth, then the pieces the first phase
//             kept against it; what it no longer hides goes to the run's second region and draw
//   second()  every run's second indirect draw
// A piece is hidden only behind depth this frame really drew, so no piece in view is missed.
// coaxial_cull_v1(state, field): 0 the runs this frame, 1 the uniforms (100 floats: the matrix,
// the eye, the runs, the regions; the pyramid's words are filled here), 2 the runs' words (first
// piece, pieces, pieces before, kit piece), 3 each kit piece's first index and index count, 4 the
// kit pieces, 5 the store the pieces are in, 6 their base instance, 7 each run's draw-list index.
// The store is three regions on the GPU (the face's coaxial_store_cap): the page's, then the two
// phases'. The pass needs "indirect-first-instance"; without it the page draws everything itself.
const U_FLOATS = 100, LEVELS = 12, CHUNKS = 1024, ARGS = 10;
const groupsOf = (n) => Math.ceil(n / 64);

export async function instanceCull(device, code, say) {
  if (!device.features.has("indirect-first-instance")) return null;
  const module = device.createShaderModule({ code });
  const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
  if (errors.length) { say(errors.map((m) => `cull.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n")); return null; }
  const pipe = (entryPoint) => device.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint } });
  const [bounds, one, two, hizDepth, hizDown] = await Promise.all(["e2q_bounds", "e2q_cull_one", "e2q_cull_two", "cl_hiz_depth", "cl_hiz_down"].map(pipe));
  return new InstanceCull(device, { bounds, one, two, hizDepth, hizDown });
}

class InstanceCull {
  constructor(device, pipes) {
    const S = GPUBufferUsage;
    this.device = device; this.pipes = pipes; this.off = false;
    this.u = device.createBuffer({ size: U_FLOATS * 4, usage: S.UNIFORM | S.COPY_DST });
    this.runsBuf = null; this.args = null; this.flags = null; this.cap = 0;
    this.tally = device.createBuffer({ size: 16, usage: S.STORAGE | S.COPY_SRC | S.COPY_DST });
    this.levels = []; this.hiz = null; this.size = [0, 0]; this.ready = false; this.depthView = null;
    this.hz = Array.from({ length: LEVELS }, (_, l) => { const b = device.createBuffer({ size: 16, usage: S.UNIFORM | S.COPY_DST }); device.queue.writeBuffer(b, 0, new Float32Array([l, 0, 0, 0])); return b; });
    this.boxes = null; this.boundsDone = false;
    this.read = { buffer: null, busy: false, stats: null }; this.frames = 0;
    this.active = null;
  }
  group(pipeline, entries) {
    return this.device.createBindGroup({ layout: pipeline.getBindGroupLayout(0),
      entries: entries.map(([binding, r]) => ({ binding, resource: r instanceof GPUTextureView ? r : { buffer: r } })) });
  }
  // The pyramid's levels for a w x h depth: level 0 half of it, rounded up, each next half again.
  resize(w, h) {
    if (this.size[0] === w && this.size[1] === h) return;
    this.size = [w, h]; this.ready = false; this.levels = [];
    let lw = Math.ceil(w / 2), lh = Math.ceil(h / 2), at = 0;
    while (this.levels.length < LEVELS) { this.levels.push({ w: lw, h: lh, at }); at += lw * lh; if (lw === 1 && lh === 1) break; lw = Math.ceil(lw / 2); lh = Math.ceil(lh / 2); }
    this.hiz?.destroy();
    this.hiz = this.device.createBuffer({ size: Math.max(16, at * 4), usage: GPUBufferUsage.STORAGE });
    this.groups = null;
  }
  // The pyramid's words of the uniforms (cull.coax CL_*): the planes from the page's matrix.
  fill(u, hiz) {
    const M = u.subarray(0, 16), row = (r) => [M[r], M[4 + r], M[8 + r], M[12 + r]];
    const add = (a, b, s) => a.map((x, i) => x + s * b[i]);
    const planes = [add(row(3), row(0), 1), add(row(3), row(0), -1), add(row(3), row(1), 1), add(row(3), row(1), -1), row(2), add(row(3), row(2), -1)];
    planes.forEach((p, k) => { const l = Math.hypot(p[0], p[1], p[2]) || 1; u.set(p.map((x) => x / l), 28 + k * 4); });
    u[22] = hiz ? 1 : 0; u[23] = this.levels.length; u[24] = this.size[0]; u[25] = this.size[1]; u[26] = this.levels[0].w; u[27] = this.levels[0].h;
    this.levels.forEach((l, k) => { u[52 + k] = l.at; u[64 + k] = l.w; u[76 + k] = l.h; });
  }
  // Before the frame's passes: phase 1, encoded into its own compute pass. Returns each draw's run
  // (-1: drawn as the page drew it), or null when nothing is culled this frame.
  frame(encoder, x, state, memory, scene, depth, w, h) {
    this.active = null;
    if (this.off || !scene.index || !scene.vertices || !scene.store) return null;
    const runs = x.coaxial_cull_v1(state, 0);
    if (runs <= 0) return null;
    const d = this.device, q = d.queue, S = GPUBufferUsage;
    const u = new Float32Array(memory.buffer, x.coaxial_cull_v1(state, 1), U_FLOATS).slice();
    const words = new Uint32Array(memory.buffer, x.coaxial_cull_v1(state, 2), runs * 4).slice();
    const at = new Uint32Array(memory.buffer, x.coaxial_cull_v1(state, 7), runs).slice();
    const kits = x.coaxial_cull_v1(state, 4), table = new Uint32Array(memory.buffer, x.coaxial_cull_v1(state, 3), kits * 2).slice();
    const base = x.coaxial_cull_v1(state, 6), total = u[93];
    if (!(total > 0) || kits <= 0) return null;
    this.resize(w, h);
    if (this.depthView?.texture !== depth) { this.depthView = depth.createView(); this.depthView.texture = depth; this.groups = null; }
    if (runs > this.cap || total > this.flagCap) {
      this.cap = Math.max(runs, 64, this.cap); this.flagCap = Math.max(total, 4096, this.flagCap || 0);
      this.runsBuf?.destroy(); this.args?.destroy(); this.flags?.destroy();
      this.runsBuf = d.createBuffer({ size: this.cap * 16, usage: S.STORAGE | S.COPY_DST });
      this.args = d.createBuffer({ size: this.cap * ARGS * 4, usage: S.STORAGE | S.INDIRECT | S.COPY_SRC | S.COPY_DST });
      this.flags = d.createBuffer({ size: this.flagCap * 4, usage: S.STORAGE });
      this.read.buffer?.destroy(); this.read.buffer = null; this.groups = null;
    }
    if (this.groups?.store !== scene.store) this.groups = null;
    if (!this.boxes) {
      this.boxes = d.createBuffer({ size: kits * 16, usage: S.STORAGE | S.COPY_DST });
      this.bounds = d.createBuffer({ size: kits * 16, usage: S.STORAGE | S.COPY_SRC | S.COPY_DST });
      this.buckets = d.createBuffer({ size: kits * 8, usage: S.STORAGE | S.COPY_DST });
    }
    this.fill(u, this.ready);
    q.writeBuffer(this.u, 0, u);
    q.writeBuffer(this.runsBuf, 0, words);
    // each run's two indirect draws: its kit piece's indices, no pieces yet, from its regions
    const args = new Uint32Array(runs * ARGS);
    for (let r = 0; r < runs; r++) {
      const b = words[r * 4 + 3], first = table[b * 2], count = table[b * 2 + 1], start = words[r * 4];
      args.set([count, 0, first, 0, base + u[94] + start, count, 0, first, 0, base + u[95] + start], r * ARGS);
    }
    q.writeBuffer(this.args, 0, args);
    q.writeBuffer(this.tally, 0, new Uint32Array(4));
    if (!this.groups) {
      const set = [[0, this.u], [1, scene.store], [2, this.runsBuf], [3, this.boxes], [4, this.args], [5, this.flags], [6, this.tally], [7, this.hiz]];
      this.groups = { store: scene.store, one: this.group(this.pipes.one, set), two: this.group(this.pipes.two, set),
        depth: this.group(this.pipes.hizDepth, [[0, this.u], [7, this.hiz], [8, this.depthView]]),
        down: this.levels.map((_, l) => (l === 0 ? null : this.group(this.pipes.hizDown, [[0, this.u], [9, this.hz[l]], [7, this.hiz]]))) };
    }
    if (!this.boundsDone) {
      // once: each kit piece's box from the pack's own vertices
      q.writeBuffer(this.buckets, 0, table);
      const start = new Uint32Array(kits * 4); for (let b = 0; b < kits; b++) start[b * 4] = 0xffffffff;
      q.writeBuffer(this.bounds, 0, start);
      const p = encoder.beginComputePass();
      p.setPipeline(this.pipes.bounds);
      p.setBindGroup(0, this.group(this.pipes.bounds, [[0, this.u], [10, this.buckets], [11, scene.index], [12, scene.vertices], [13, this.bounds]]));
      p.dispatchWorkgroups(groupsOf(kits * CHUNKS)); p.end();
      encoder.copyBufferToBuffer(this.bounds, 0, this.boxes, 0, kits * 16);
      this.boundsDone = true;
    }
    const p = encoder.beginComputePass();
    p.setPipeline(this.pipes.one); p.setBindGroup(0, this.groups.one); p.dispatchWorkgroups(groupsOf(total)); p.end();
    const run = new Int32Array(Math.max(...at) + 1).fill(-1);
    for (let r = 0; r < runs; r++) run[at[r]] = r;
    let picked = 0; for (let r = 0; r < runs; r++) picked += words[r * 4 + 1];
    this.active = { runs, total, run, picked };
    return this.active;
  }
  // Draw i of the list: its run's first indirect draw, or false to draw it as the page did.
  draw(pass, i) {
    const r = this.active && i < this.active.run.length ? this.active.run[i] : -1;
    if (r < 0) return false;
    pass.drawIndexedIndirect(this.args, r * ARGS * 4);
    return true;
  }
  // After the opaque draws (their depth stored): the pyramid, then phase 2.
  late(encoder) {
    if (!this.active) return;
    const p = encoder.beginComputePass();
    p.setPipeline(this.pipes.hizDepth); p.setBindGroup(0, this.groups.depth); p.dispatchWorkgroups(groupsOf(this.levels[0].w * this.levels[0].h));
    for (let l = 1; l < this.levels.length; l++) { p.setPipeline(this.pipes.hizDown); p.setBindGroup(0, this.groups.down[l]); p.dispatchWorkgroups(groupsOf(this.levels[l].w * this.levels[l].h)); }
    p.setPipeline(this.pipes.two); p.setBindGroup(0, this.groups.two); p.dispatchWorkgroups(groupsOf(this.active.total)); p.end();
    this.ready = true;
    // a benchmark's counts (coaxial.cullStats), now and then, never waited for
    if (globalThis.coaxialPassTimer === true && !this.read.busy && this.frames++ % 15 === 0) {
      const bytes = 16 + this.active.runs * ARGS * 4;
      if (!this.read.buffer || this.read.buffer.size < bytes) { this.read.buffer?.destroy(); this.read.buffer = this.device.createBuffer({ size: 16 + this.cap * ARGS * 4, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }); }
      encoder.copyBufferToBuffer(this.tally, 0, this.read.buffer, 0, 16);
      encoder.copyBufferToBuffer(this.args, 0, this.read.buffer, 16, this.active.runs * ARGS * 4);
      this.read.pending = { runs: this.active.runs, total: this.active.total, picked: this.active.picked };
    }
  }
  // Every run's second indirect draw.
  second(pass) {
    if (!this.active) return;
    for (let r = 0; r < this.active.runs; r++) pass.drawIndexedIndirect(this.args, (r * ARGS + 5) * 4);
  }
  // After submit: read back the counts asked for in late().
  submitted() {
    const want = this.read.pending; if (!want) return;
    this.read.pending = null; this.read.busy = true;
    const b = this.read.buffer;
    b.mapAsync(GPUMapMode.READ).then(() => {
      const t = new Uint32Array(b.getMappedRange().slice(0)); b.unmap();
      let drawn1 = 0, drawn2 = 0;
      for (let r = 0; r < want.runs; r++) { drawn1 += t[4 + r * ARGS + 1]; drawn2 += t[4 + r * ARGS + 6]; }
      this.read.stats = { picked: want.picked, pieces: want.total, frustum: t[0], kept: t[1], hidden: t[2], drawn1, drawn2 };
    }, () => {}).finally(() => { this.read.busy = false; });
  }
  stats() { return this.read.stats; }
}
