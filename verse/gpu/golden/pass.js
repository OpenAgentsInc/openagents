// Firm reason: 1 (it runs in the browser and talks to WebGPU)
// Passes of Everglade's WGSL drawn by the browser's WebGPU, for the native
// port's per-pass check (docs/verse/native-gpu.md, G5). verse/gpu/pass-check.coax
// serves a directory with this page and the spec's files, and calls these over
// the Chrome DevTools protocol; verse/gpu/vk-passes.c draws the same spec natively
// (its header documents the directives). Here `wgsl FILE` names the WGSL, and
// a pass's bindings (SPV.bindings, by name) are found in it by their
// @group/@binding. A pass clears its colour targets (rgba16float) to 0 and its
// depth to 1, tests less and writes depth, unless its options say otherwise.
//   runSpec(SPEC_FILE) -> {ok: true, targets: [base64 of float32 RGBA rows, ...]} or {ok: false, error}
//   decodeLayers(LIST_FILE, W, H) -> {ok, levels}; then layerBytes(i) -> base64 of layer i's
//     rgba8 texels, level after level: the pack's images as host.js layerTexture uploads them
//     (createImageBitmap, each level resized with resizeQuality "high"), read back raw.

const toB64 = (bytes) => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(s);
};
const half = (h) => {
  const s = h & 0x8000 ? -1 : 1, e = (h >> 10) & 31, m = h & 1023;
  if (e === 0) return s * m * 2 ** -24;
  if (e === 31) return m ? NaN : s * Infinity;
  return s * (1 + m / 1024) * 2 ** (e - 15);
};
const bytesOf = async (name) => {
  const r = await fetch(name);
  if (!r.ok) throw new Error(`${name}: ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
};
const textOf = async (name) => new TextDecoder().decode(await bytesOf(name));
let device;
const gpu = async () => {
  if (!device) {
    const adapter = await navigator.gpu.requestAdapter();
    device = await adapter.requestDevice({ requiredLimits: { maxStorageBufferBindingSize: adapter.limits.maxStorageBufferBindingSize, maxBufferSize: adapter.limits.maxBufferSize } });
  }
  return device;
};
// The texels of a texture's level 0 layer `layer`, as bytes (rows tightly packed).
async function readLayer(d, texture, w, h, bpp, layer, level, aspect) {
  const row = Math.ceil(w * bpp / 256) * 256;
  const buf = d.createBuffer({ size: row * h, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const enc = d.createCommandEncoder();
  enc.copyTextureToBuffer({ texture, mipLevel: level, origin: [0, 0, layer], ...(aspect ? { aspect } : {}) }, { buffer: buf, bytesPerRow: row }, [w, h]);
  d.queue.submit([enc.finish()]);
  await buf.mapAsync(GPUMapMode.READ);
  const src = new Uint8Array(buf.getMappedRange()), out = new Uint8Array(w * h * bpp);
  for (let y = 0; y < h; y++) out.set(src.subarray(y * row, y * row + w * bpp), y * w * bpp);
  buf.destroy();
  return out;
}

let layers = null;
window.decodeLayers = async function (list, w, h) {
  try {
    const d = await gpu();
    const names = (await textOf(list)).split("\n").filter(Boolean);
    const images = await Promise.all(names.map(async (n) => createImageBitmap(new Blob([await bytesOf(n)]))));
    const levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
    const texture = d.createTexture({ size: [w, h, images.length], format: "rgba8unorm-srgb", mipLevelCount: levels,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC | GPUTextureUsage.RENDER_ATTACHMENT });
    for (const [layer, image] of images.entries())
      for (let level = 0; level < levels; level++) {
        const lw = Math.max(1, w >> level), lh = Math.max(1, h >> level);
        const source = level === 0 && image.width === w && image.height === h ? image
          : await createImageBitmap(image, { resizeWidth: lw, resizeHeight: lh, resizeQuality: "high" });
        d.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer] }, [lw, lh]);
      }
    layers = { texture, w, h, levels, n: images.length };
    return { ok: true, levels, n: images.length };
  } catch (e) { return { ok: false, error: String(e && e.stack || e) }; }
};
window.layerBytes = async function (layer) {
  const d = await gpu(), parts = [];
  for (let level = 0; level < layers.levels; level++)
    parts.push(await readLayer(d, layers.texture, Math.max(1, layers.w >> level), Math.max(1, layers.h >> level), 4, layer, level));
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return toB64(out);
};

// The spec with its counts in place (vk-spec.h): `let NAME V` lines gone, $NAME their numbers,
// and each `each draws:A:B` ... `end` block written out once a record, draws:each that one.
const expandSpec = (text) => {
  const lets = new Map(), out = [];
  const num = (t) => {
    if (!t.startsWith("$")) return +t;
    if (!lets.has(t.slice(1))) throw new Error(`the spec has no count ${t}`);
    return lets.get(t.slice(1));
  };
  let body = null, range = null;
  for (const raw of text.split("\n")) {
    const a = raw.trim().split(/\s+/);
    if (a[0] === "let") { lets.set(a[1], +a[2]); continue; }
    if (a[0] === "each") { range = a[1].slice(6).split(":").map(num); body = []; continue; }
    if (a[0] === "end") {
      for (let i = range[0]; i < range[1]; i++) for (const l of body) out.push(l.replace(/\bdraws:each\b/, `draws:${i}:${i + 1}`));
      body = null;
      continue;
    }
    (body ?? out).push(raw.replace(/\$(\w+)/g, (m) => String(num(m))));
  }
  return out;
};
window.runSpec = async function (specFile) {
  try {
    const d = await gpu();
    const errors = [];
    d.addEventListener("uncapturederror", (e) => errors.push(e.error.message));
    d.pushErrorScope("validation");
    const res = new Map();
    let wgsl = "", module = null, w = 0, h = 0, index = null, draws = null, ndraws = 0;
    const targets = [];
    let last = [];
    for (const raw of expandSpec(await textOf(specFile))) {
      const a = raw.trim().split(/\s+/);
      if (!a[0] || a[0].startsWith("#")) continue;
      if (a[0] === "wgsl") {
        wgsl = await textOf(a[1]);
        module = d.createShaderModule({ code: wgsl });
        const bad = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
        if (bad.length) return { ok: false, error: bad.map((m) => `${m.lineNum}: ${m.message}`).join("\n") };
      } else if (a[0] === "size") { w = +a[1]; h = +a[2]; }
      else if (a[0] === "buffer" || a[0] === "index") {
        const bytes = await bytesOf(a[0] === "index" ? a[1] : a[2]);
        const b = d.createBuffer({ size: Math.max(16, Math.ceil(bytes.length / 16) * 16),
          usage: (a[0] === "index" ? GPUBufferUsage.INDEX : GPUBufferUsage.UNIFORM | GPUBufferUsage.STORAGE | GPUBufferUsage.VERTEX) | GPUBufferUsage.COPY_DST });
        const whole = new Uint8Array(Math.ceil(bytes.length / 4) * 4); whole.set(bytes);
        d.queue.writeBuffer(b, 0, whole);
        if (a[0] === "index") index = b; else res.set(a[1], { buffer: b });
      } else if (a[0] === "draws") { draws = new Uint32Array((await bytesOf(a[1])).buffer); ndraws = +a[2]; }
      else if (a[0] === "layers") {
        const [lw, lh, n, levels] = a.slice(3, 7).map(Number), bytes = await bytesOf(a[2]);
        const t = d.createTexture({ size: [lw, lh, n], format: "rgba8unorm-srgb", mipLevelCount: levels, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
        let at = 0;
        for (let level = 0; level < levels; level++) {
          const xw = Math.max(1, lw >> level), xh = Math.max(1, lh >> level);
          d.queue.writeTexture({ texture: t, mipLevel: level }, bytes.subarray(at, at + xw * xh * 4 * n), { bytesPerRow: xw * 4, rowsPerImage: xh }, [xw, xh, n]);
          at += xw * xh * 4 * n;
        }
        res.set(a[1], t.createView({ dimension: "2d-array" }));
      } else if (a[0] === "sampler") {
        const f = a[2] === "linear" ? "linear" : "nearest", wrap = a[3] === "repeat" ? "repeat" : "clamp-to-edge";
        res.set(a[1], d.createSampler({ magFilter: f, minFilter: f, mipmapFilter: a[4] === "mip" ? "linear" : "nearest",
          addressModeU: wrap, addressModeV: wrap, addressModeW: wrap, ...(a[5] === "less" ? { compare: "less" } : {}) }));
      } else if (a[0] === "depth") {
        const t = d.createTexture({ size: a[2] === "frame" ? [w, h] : [+a[2], +a[3]], format: "depth32float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST });
        res.set(a[1], { texture: t, view: t.createView() });
      } else if (a[0] === "floats") {
        const f = new Float32Array(a.slice(2).map(Number)), b = d.createBuffer({ size: Math.max(16, f.byteLength), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
        d.queue.writeBuffer(b, 0, f);
        res.set(a[1], { buffer: b });
      } else if (a[0] === "target") {
        const k = a[2] && a[2].startsWith("/") ? +a[2].slice(1) : 0;
        const t = d.createTexture({ size: k ? [Math.ceil(w / k), Math.ceil(h / k)] : a[2] ? [+a[2], +a[3]] : [w, h], format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST });
        res.set(a[1], { texture: t, view: t.createView() });
      } else if (a[0] === "copy") {
        const enc = d.createCommandEncoder();
        enc.copyTextureToTexture({ texture: res.get(a[1]).texture }, { texture: res.get(a[2]).texture }, [w, h]);
        d.queue.submit([enc.finish()]);
      } else if (a[0] === "pass") {
        const [spv, vs, fs] = a.slice(1, 4), depthName = a[5], bias = +a[6], slope = +a[7], calls = a[8], opts = a.slice(9);
        let colour;
        if (/^\d+$/.test(a[4])) {
          const nt = +a[4];
          while (targets.length < nt) targets.push(d.createTexture({ size: [w, h], format: "rgba16float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC }));
          colour = targets.slice(0, nt);
        } else colour = a[4].split(",").map((n) => { const r = res.get(n); if (!r) throw new Error(`the spec has no ${n}`); return r.texture; });
        const keep = opts.includes("keep"), write = !opts.includes("nowrite"), always = opts.includes("always");
        const mode = (opts.find((o) => o.startsWith("blend=")) || "").slice(6), alphaMode = (opts.find((o) => o.startsWith("alpha=")) || "").slice(6);
        const masked = opts.filter((o) => o.startsWith("mask=")).map((o) => +o.slice(5));
        for (const o of opts) if (!/^(keep|nowrite|always|(blend|alpha)=(add|alpha|mul|over|under|keep|max)|mask=\d+|bind=\w+:\w+|vertices=\w+:\d+)$/.test(o)) throw new Error(`no pass option ${o}`);
        // a vertex buffer of FLOATS floats a vertex, one vec4f attribute per four (host.js's fx pipeline)
        const vtx = (opts.find((o) => o.startsWith("vertices=")) || "").slice(9).split(":");
        const vertexBuffers = vtx[0] ? [{ arrayStride: +vtx[1] * 4, attributes: Array.from({ length: Math.ceil(+vtx[1] / 4) }, (_, i) => ({ shaderLocation: i, offset: i * 16, format: "float32x4" })) }] : [];
        const rebound = new Map(opts.filter((o) => o.startsWith("bind=")).map((o) => o.slice(5).split(":")));
        // vks.h's modes, as WebGPU factors; the alpha follows the colour's unless alpha= says (alpha: over)
        const modes = { add: ["one", "one"], alpha: ["src-alpha", "one-minus-src-alpha"], mul: ["zero", "src"], over: ["one", "one-minus-src-alpha"],
          under: ["zero", "one-minus-src-alpha"], keep: ["zero", "one"], max: ["one", "one"] };
        const of = (m) => ({ operation: m === "max" ? "max" : "add", srcFactor: modes[m][0], dstFactor: modes[m][1] });
        const blend = mode ? { color: of(mode), alpha: of(alphaMode || (mode === "alpha" ? "over" : "add")) } : undefined;
        last = colour;
        let depth = null;
        if (depthName === "own") depth = d.createTexture({ size: [w, h], format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT }).createView();
        else if (depthName !== "-") depth = res.get(depthName).view;
        const pipeline = await d.createRenderPipelineAsync({
          layout: "auto",
          vertex: { module, entryPoint: vs, buffers: vertexBuffers },
          fragment: { module, entryPoint: fs, targets: colour.map((_, i) => masked.includes(i) ? { format: "rgba16float", writeMask: 0 }
            : { format: "rgba16float", ...(blend ? { blend } : {}) }) },
          primitive: { topology: "triangle-list", cullMode: "none" },
          ...(depth ? { depthStencil: { format: "depth32float", depthWriteEnabled: write, depthCompare: always ? "always" : "less", depthBias: bias, depthBiasSlopeScale: slope } } : {}),
        });
        // the bindings by name, at the group and binding the WGSL declares them
        const groups = new Map();
        for (const line of (await textOf(`${spv}.bindings`)).split("\n")) {
          const m = line.match(/^binding \d+ \S+ (\S+)/);
          if (!m) continue;
          const name = m[1], decl = wgsl.match(new RegExp(`@group\\((\\d+)\\)\\s*@binding\\((\\d+)\\)\\s*var(?:<[^>]*>)?\\s+${name}\\s*:`));
          if (!decl) throw new Error(`the WGSL does not declare ${name}`);
          const r = res.get(rebound.get(name) ?? name);
          if (!r) throw new Error(`the spec has no ${rebound.get(name) ?? name}`);
          const g = +decl[1];
          if (!groups.has(g)) groups.set(g, []);
          groups.get(g).push({ binding: +decl[2], resource: r.view ?? r });
        }
        const enc = d.createCommandEncoder();
        const pass = enc.beginRenderPass({
          colorAttachments: colour.map((t) => ({ view: t.createView(), loadOp: keep ? "load" : "clear", storeOp: "store", clearValue: [0, 0, 0, 0] })),
          ...(depth ? { depthStencilAttachment: { view: depth, depthClearValue: 1, depthLoadOp: keep ? "load" : "clear", depthStoreOp: "store" } } : {}),
        });
        pass.setPipeline(pipeline);
        if (vtx[0]) pass.setVertexBuffer(0, res.get(vtx[0]).buffer);
        const top = Math.max(-1, ...groups.keys());
        for (let g = 0; g <= top; g++) pass.setBindGroup(g, d.createBindGroup({ layout: pipeline.getBindGroupLayout(g), entries: groups.get(g) ?? [] }));
        if (calls.startsWith("draws")) {
          const [from, to] = calls === "draws" ? [0, ndraws] : calls.slice(6).split(":").map(Number);
          if (!(from <= to && to <= ndraws)) throw new Error(`draws:A:B within the list: ${calls}`);
          if (index) pass.setIndexBuffer(index, "uint32");
          for (let i = from; i < to; i++) {
            const [first, count, base, instances] = draws.subarray(i * 4, i * 4 + 4);
            if (!count || !instances) continue;
            if (base & 0x80000000) pass.draw(count, instances, first, base & 0x7fffffff);
            else pass.drawIndexed(count, instances, first, 0, base);
          }
        } else {
          const [v, n] = calls.split(":").map(Number);
          pass.draw(v, n || 1);
        }
        pass.end();
        d.queue.submit([enc.finish()]);
      } else throw new Error(`no directive ${a[0]}`);
    }
    const out = [];
    for (const t of last) {
      const halves = new Uint16Array((await readLayer(d, t, w, h, 8, 0, 0)).buffer);
      const f = new Float32Array(w * h * 4);
      for (let i = 0; i < f.length; i++) f[i] = half(halves[i]);
      out.push(toB64(new Uint8Array(f.buffer)));
    }
    const scoped = await d.popErrorScope();
    if (scoped) errors.push(scoped.message);
    if (errors.length) return { ok: false, error: errors.join("\n") };
    return { ok: true, targets: out };
  } catch (e) {
    return { ok: false, error: String(e && e.stack || e) };
  }
};
