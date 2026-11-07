// Firm reason: 1 (browser glue: fetch, WebCrypto, image decode and WebGPU for the character pass; admission, playback and lighting are Coax).
// The character pass (docs/verse/2026-11-06-native-hero-characters.md, Design 3; R1, #12985): a
// world that places a detailed character exports coaxial_character_v1(state), the address of its
// SceneLights block (aethyr/render/scene-lights.coax) or 0, and the name of the character's pack.
// Only then does the host load the character runtime (build/character-host.wasm, from
// apps/openagents-web/character-host.coax) and the pack, admitted by the runtime's Coax (its SHA-256,
// catalog and parts). Each frame the world's block goes through the runtime, which steps her idle
// body and face and fills her slots; her pipelines (character.wgsl, from aethyr/render/character.coax)
// then draw her into the world's own light, aux and depth targets after its opaque geometry, and her
// caster into the world's shadow map, so she and the world shadow and hide each other both ways.
// A world with no character placed never calls into this beyond coaxial_character_v1.
const wasmURL = new URL("../build/character-host.wasm", import.meta.url);
const wgslURL = new URL("../character.wgsl", import.meta.url);
const PACK_CATALOG = 1, PACK_VERTICES = 2, PACK_INDICES = 3, PACK_LAYER = 4;

// The imported memory's initial and maximum pages, from the module's import section.
function memoryPages(bytes) {
  const b = new Uint8Array(bytes);let i = 8;
  const leb = () => { let v = 0, shift = 0, c; do { c = b[i++]; v += (c & 0x7f) * 2 ** shift; shift += 7; } while (c & 0x80); return v; };
  const skip = () => { const n = leb(); i += n; };
  while (i < b.length) {
    const id = b[i++], size = leb(), end = i + size;
    if (id === 2) {
      for (let n = leb(); n > 0; n--) {
        skip(); skip();
        const kind = b[i++];
        if (kind === 0) leb();
        else if (kind === 1) { i++; const f = leb(); leb(); if (f & 1) leb(); }
        else if (kind === 2) { const f = leb(), initial = leb(), maximum = f & 1 ? leb() : 65536; return { initial, maximum }; }
        else if (kind === 3) i += 2;
        else break;
      }
      break;
    }
    i = end;
  }
  throw Error("character runtime: no imported memory");
}

// The runtime: its own memory, its state above its data and stack.
async function openRuntime() {
  const r = await fetch(wasmURL, { cache: "no-store" });
  if (!r.ok) throw Error(`character runtime: ${r.status}`);
  const bytes = await r.arrayBuffer(), pages = memoryPages(bytes);
  const memory = new WebAssembly.Memory({ initial: pages.initial, maximum: pages.maximum });
  const module = await WebAssembly.compile(bytes), imports = { env: { memory } };
  for (const imp of WebAssembly.Module.imports(module)) if (imp.kind === "function") (imports[imp.module] ||= {})[imp.name] ||= () => 8;
  const x = (await WebAssembly.instantiate(module, imports)).exports;
  x._initialize?.();
  const base = Math.ceil(Math.max(x.__heap_base?.value ?? 0, memory.buffer.byteLength) / 65536) * 65536;
  const need = base + x.character_state_size();
  if (need > memory.buffer.byteLength) memory.grow(Math.ceil((need - memory.buffer.byteLength) / 65536));
  x.character_state_init(base);
  return { x, memory, state: base };
}

// The pack's bytes: a local address reads /private/NAME.pack (serve.c), openagents.com the broker's
// session (privatePack opens it); anywhere else, and in a golden, none.
async function fetchPack(name, { dev, site, privatePack, progress }) {
  if (!/^[a-z0-9-]{1,58}$/.test(name) || (!dev && !site)) throw Error("character pack: not here");
  const url = dev ? `/private/${name}.pack` : `/api/verse/packs/${name}`;
  let session = null;
  if (!dev) {
    const s = await fetch(url, { method: "POST", cache: "no-store", credentials: "same-origin" });
    if (!s.ok) throw Error(`character pack: ${s.status}`);
    session = await s.json();
  }
  const r = await fetch(session ? session.url : url, { cache: "no-store" });
  if (!r.ok || !r.body) throw Error(`character pack: ${r.status}`);
  // one buffer, sized from the pack's header (or the session) once the first bytes are in
  const reader = r.body.getReader(), held = [];
  let bytes = null, loaded = 0, total = session ? session.size : 0;
  const put = (c) => { if (loaded + c.length > total) throw Error("character pack: longer than its header"); bytes.set(c, loaded); loaded += c.length; };
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (bytes) put(value);
    else {
      held.push(value);
      const head = held.reduce((n, c) => n + c.length, 0);
      if (!total && head >= 12) { const first = new Uint8Array(head); let at = 0; for (const c of held) { first.set(c, at); at += c.length; } total = new DataView(first.buffer).getUint32(8, true); }
      if (total) { bytes = new Uint8Array(total); for (const c of held) put(c); held.length = 0; }
    }
    progress?.(loaded, total);
  }
  if (!bytes || loaded !== total) throw Error("character pack: cut short");
  return session ? privatePack(session, bytes) : bytes;
}

async function layerTexture(device, images, decode) {
  const first = await decode(images[0]), [w, h] = [first.width, first.height];
  const levels = Math.floor(Math.log2(Math.max(w, h))) + 1;
  const texture = device.createTexture({ size: [w, h, images.length], format: "rgba8unorm-srgb", mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  try {
    for (let layer = 0; layer < images.length; layer++) {
      const image = layer === 0 ? first : await decode(images[layer], { resizeWidth: w, resizeHeight: h, resizeQuality: "high" });
      try {
        for (let level = 0; level < levels; level++) {
          const lw = Math.max(1, w >> level), lh = Math.max(1, h >> level);
          const source = level === 0 && image.width === w && image.height === h ? image : await createImageBitmap(image, { resizeWidth: lw, resizeHeight: lh, resizeQuality: "high" });
          try { device.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer] }, [lw, lh]); }
          finally { if (source !== image) source.close(); }
        }
        await device.queue.onSubmittedWorkDone();
      } finally { image.close(); }
    }
  } catch (error) { texture.destroy(); throw error; }
  return texture;
}

// The pass: want() each frame with the world's exports and state; while it answers true, cast()
// in the world's shadow pass and draw() after its opaque geometry. deps: { gpu: () => the host's
// gpu record, dev, site, golden, privatePack, unpack, decode, say }.
export function characterPass(deps) {
  let current = null;   // { owner, device, name, status, runtime, gpu resources, ... }
  const info = { status: "off", frames: 0, draws: 0, loadMs: 0, packBytes: 0, error: "" };

  function dispose() {
    if (!current) return;
    const c = current; current = null;
    for (const k of ["vertices", "index", "bones", "uniform"]) c[k]?.destroy?.();
    c.texture?.destroy();
    info.status = "off";
  }

  async function load(c) {
    const started = performance.now();
    try {
      const { device } = c;
      if (device.limits.maxStorageBufferBindingSize < c.storage) throw Error(`the device binds ${device.limits.maxStorageBufferBindingSize} storage bytes, her vertices need ${c.storage}`);
      const [runtime, code] = await Promise.all([openRuntime(), fetch(wgslURL, { cache: deps.dev ? "no-store" : "default" }).then((r) => { if (!r.ok) throw Error(`character.wgsl: ${r.status}`); return r.text(); })]);
      if (current !== c) return;
      c.runtime = runtime;
      const bytes = await fetchPack(c.name, { dev: deps.dev, site: deps.site, privatePack: deps.privatePack, progress: (n, t) => { info.status = `loading ${n}/${t}`; } });
      if (current !== c) return;
      info.packBytes = bytes.length;
      const { x, memory, state } = runtime;
      // the pack gate: SHA-256 is the browser's, acceptance the runtime's Coax
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      if (current !== c) return;
      new Uint8Array(memory.buffer, x.character_digest(state), 32).set(digest);
      if (!x.character_pack_admit(state, bytes.length)) throw Error("character pack: SHA-256 refused by Coax");
      const entries = deps.unpack(bytes), of = (kind) => entries.filter((e) => e.kind === kind);
      const catalog = of(PACK_CATALOG)[0], vertices = of(PACK_VERTICES)[0], indices = of(PACK_INDICES)[0], layers = of(PACK_LAYER);
      if (!catalog || !vertices || !indices || !layers.length) throw Error("character pack: a part is missing");
      const at = x.character_catalog_at(catalog.bytes.length);
      if (!at) throw Error("character pack: its catalog is over the runtime's capacity");
      new Uint8Array(memory.buffer, at, catalog.bytes.length).set(catalog.bytes);
      if (!x.character_catalog_admit(state, catalog.bytes.length)) throw Error("character pack: catalog refused by Coax");
      x.character_asset_size(state, 1, vertices.bytes.length); x.character_asset_size(state, 2, indices.bytes.length); x.character_asset_size(state, 3, layers.length);
      if (x.character_probe(state, 0) !== 1) throw Error("character pack: parts refused by Coax");
      groomOrder(x, state, memory, vertices.bytes, indices.bytes);
      // the GPU: her vertex words, indices, layers, palette and block
      const buffer = (data, usage) => { const b = device.createBuffer({ size: Math.ceil(Math.max(data.length, 4) / 4) * 4, usage: usage | GPUBufferUsage.COPY_DST, mappedAtCreation: true }); new Uint8Array(b.getMappedRange()).set(data); b.unmap(); return b; };
      c.vertices = buffer(vertices.bytes, GPUBufferUsage.STORAGE);
      c.index = buffer(indices.bytes, GPUBufferUsage.INDEX);
      c.texture = await layerTexture(device, layers.map((e) => e.bytes), deps.decode);
      if (current !== c) { c.vertices.destroy(); c.index.destroy(); c.texture.destroy(); return; }
      c.paletteBytes = x.character_palette_bytes();
      c.bones = device.createBuffer({ size: Math.ceil(c.paletteBytes / 16) * 16, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
      c.blockFloats = x.character_block_floats();
      c.uniform = device.createBuffer({ size: c.blockFloats * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      await pipelines(c, code);
      if (current !== c) return;
      c.status = "ready"; info.status = "ready"; info.loadMs = Math.round(performance.now() - started);
    } catch (error) {
      if (current !== c) return;
      c.status = "failed"; info.status = "failed"; info.error = error.message;
      if (deps.dev) console.error(`character pass: ${error.message}`);
    }
  }

  // Groom levels (#12988): her strands' index range ordered group by group in the runtime's order, so
  // a level draws a prefix. The runtime admits the groups' counts; refused, every strand draws.
  function groomOrder(x, state, memory, vertexBytes, indexBytes) {
    const groups = x.character_groom_layout(0), seedWord = x.character_groom_layout(1), stride = x.character_groom_layout(2);
    const first = x.character_groom_layout(3), count = x.character_groom_layout(4), segments = count / 6;
    const words = (b) => b.byteOffset % 4 === 0 ? new Uint32Array(b.buffer, b.byteOffset, b.length >> 2) : new Uint32Array(b.slice().buffer);
    const vw = words(vertexBytes), seeds = new Float32Array(vw.buffer, vw.byteOffset, vw.length), index = words(indexBytes);
    const group = new Uint16Array(segments), sizes = new Uint32Array(groups);
    for (let k = 0; k < segments; k++) {
      if (index[first + k * 6] !== 1000000 + k * 6) return;   // not in segment order: left whole
      const g = ((Math.round(seeds[seedWord + k * stride] * groups) % groups) + groups) % groups;
      group[k] = g; sizes[g]++;
    }
    const start = new Uint32Array(groups);
    for (let r = 0, at = 0; r < groups; r++) { const g = x.character_groom_group(r); start[g] = at; at += sizes[g]; }
    const out = new Uint32Array(count), corners = Array.from({ length: 6 }, (_, i) => x.character_groom_corner(i));
    for (let k = 0; k < segments; k++) { const at = start[group[k]]++ * 6; for (let v = 0; v < 6; v++) out[at + v] = 1000000 + k * 6 + corners[v]; }
    index.set(out, first);
    if (index.buffer !== indexBytes.buffer) indexBytes.set(new Uint8Array(index.buffer));
    new Uint32Array(memory.buffer, x.character_groom_groups(state), groups).set(sizes);
    x.character_groom_admit(state);
  }

  async function pipelines(c, code) {
    const { device } = c, g = deps.gpu(), both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
    const module = device.createShaderModule({ code });
    const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
    if (errors.length) throw Error(errors.map((m) => `character.wgsl:${m.lineNum}:${m.linePos}: ${m.message}`).join("\n"));
    const blockLayout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: both, buffer: {} }] });
    const ownLayout = device.createBindGroupLayout({ entries: [
      { binding: 0, visibility: both, buffer: { type: "read-only-storage" } },
      { binding: 1, visibility: both, buffer: { type: "read-only-storage" } },
      { binding: 2, visibility: GPUShaderStage.FRAGMENT, texture: { viewDimension: "2d-array" } },
      { binding: 3, visibility: GPUShaderStage.FRAGMENT, sampler: {} }] });
    c.blockGroup = device.createBindGroup({ layout: blockLayout, entries: [{ binding: 0, resource: { buffer: c.uniform } }] });
    c.ownGroup = device.createBindGroup({ layout: ownLayout, entries: [
      { binding: 0, resource: { buffer: c.vertices } }, { binding: 1, resource: { buffer: c.bones } },
      { binding: 2, resource: c.texture.createView({ dimension: "2d-array" }) },
      { binding: 3, resource: device.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear", addressModeU: "repeat", addressModeV: "repeat", maxAnisotropy: 8 }) }] });
    // the world's targets: its light and aux (rgba16float) with a post pass, else its screen format
    const lit = g.post ? [{ format: "rgba16float" }, { format: "rgba16float" }] : [{ format: g.format }];
    const layout = device.createPipelineLayout({ bindGroupLayouts: [blockLayout, ownLayout, g.shadowLayout] });
    const over = { color: { operation: "add", srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { operation: "add", srcFactor: "zero", dstFactor: "one-minus-src-alpha" } };
    const prim = { topology: "triangle-list", cullMode: "none" };
    [c.body, c.hair, c.caster] = await Promise.all([
      device.createRenderPipelineAsync({ layout, vertex: { module, entryPoint: "character_vs" }, fragment: { module, entryPoint: "character_fs", targets: lit }, primitive: prim,
        depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less" } }),
      device.createRenderPipelineAsync({ layout, vertex: { module, entryPoint: "character_vs" }, fragment: { module, entryPoint: "character_fs", targets: lit.map((t) => ({ ...t, blend: over })) }, primitive: prim,
        depthStencil: { format: "depth32float", depthWriteEnabled: false, depthCompare: "less" } }),
      device.createRenderPipelineAsync({ layout: device.createPipelineLayout({ bindGroupLayouts: [blockLayout, ownLayout] }), vertex: { module, entryPoint: "caster_vs" }, fragment: { module, entryPoint: "caster_fs", targets: [] }, primitive: prim,
        depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "less", depthBias: 2, depthBiasSlopeScale: 2 } }),
    ]);
  }

  // Each frame, before the world's passes: whether she draws this frame. Copies the world's block
  // through the runtime and up to the GPU, and her palette when it changed.
  function want(x, state, memory, now) {
    const block = x.coaxial_character_v1?.(state) || 0;
    const g = deps.gpu();
    if (!block || deps.golden || !g?.shadowLayout || !g.shadowGroup) { if (current && current.owner !== x) dispose(); return false; }
    if (current && (current.owner !== x || current.device !== g.device)) dispose();
    if (!current) {
      const name = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_character_pack_v1(state), x.coaxial_character_pack_len_v1(state)));
      current = { owner: x, device: g.device, name, storage: x.coaxial_character_storage_v1?.() ?? 0, status: "loading", last: now, version: -1 };
      info.status = "loading"; info.error = "";
      load(current);
    }
    const c = current;
    // the world learns whether she draws: only then is she a body there (coaxial_character_ready_v1)
    const shown = (on) => { x.coaxial_character_ready_v1?.(state, on ? 1 : 0); return on; };
    if (c.status !== "ready") return shown(false);
    const { x: r, memory: m, state: s } = c.runtime;
    const into = r.character_block(s);
    new Float32Array(m.buffer, into, c.blockFloats).set(new Float32Array(memory.buffer, block, c.blockFloats));
    const dt = Math.min(Math.max((now - c.last) / 1000, 0), 0.25); c.last = now;
    const t0 = performance.now();
    c.draws = r.character_step(s, dt);
    if (!c.draws) return shown(false);
    g.device.queue.writeBuffer(c.uniform, 0, m.buffer, into, c.blockFloats * 4);
    const version = r.character_palette_version(s);
    if (version !== c.version) { g.device.queue.writeBuffer(c.bones, 0, m.buffer, r.character_palette(s), c.paletteBytes & ~3); c.version = version; }
    c.list = new Uint32Array(m.buffer, r.character_draws(s), c.draws * 4).slice();
    info.frames++; info.draws = c.draws; info.stepMs = performance.now() - t0;
    return shown(true);
  }
  const drawEach = (pass, c, pick) => {
    pass.setBindGroup(0, c.blockGroup); pass.setBindGroup(1, c.ownGroup); pass.setIndexBuffer(c.index, "uint32");
    for (let i = 0; i < c.draws; i++) {
      const pipeline = pick(c.runtime.x.character_blend(i));
      if (!pipeline) continue;
      pass.setPipeline(pipeline);
      pass.drawIndexed(c.list[i * 4 + 1], c.list[i * 4 + 3], c.list[i * 4], 0, c.list[i * 4 + 2]);
    }
  };
  // In the world's shadow pass, after its casters (the caller sets its own groups again after).
  function cast(pass) { const c = current; if (c?.status === "ready") drawEach(pass, c, (blend) => blend === 2 ? null : c.caster); }
  // In the world's main pass, after its opaque geometry: her body, then her strands over it.
  function draw(pass) {
    const c = current; if (c?.status !== "ready") return;
    pass.setBindGroup(2, deps.gpu().shadowGroup);
    drawEach(pass, c, (blend) => blend === 0 ? c.body : null);
    drawEach(pass, c, (blend) => blend === 2 ? c.hair : null);
  }
  return { want, cast, draw, dispose, info: () => ({ ...info, probe: current?.runtime ? [0, 1, 2, 3, 4, 5, 6, 7, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35].map((f) => current.runtime.x.character_probe(current.runtime.state, f)) : null }) };
}
