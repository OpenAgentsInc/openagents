// Effects adapter for admitted Coax package/frame/binding plans. Numeric
// selectors below are renderer transport ABI1; no realm table lives here.
const number = (x) => Number(x);
class RenderRefusal extends Error {}
const blendComponent = (mode, alpha) => {
  const factors = [
    ["add", "one", "zero"], ["max", "one", "one"], ["add", "zero", alpha ? "src-alpha" : "src"],
    ["add", "one", "one"], ["add", "zero", "one"], ["add", "one", "one-minus-src-alpha"], ["add", "zero", "one-minus-src-alpha"],
  ][mode];
  if (!factors) throw Error(`unknown Coax blend ${mode}`);
  return { operation: factors[0], srcFactor: factors[1], dstFactor: factors[2] };
};

export class RenderHost {
  constructor({ device, context, format, memory, exports, arena, url, profile, coarse, capture = false, onMedia = null }) {
    Object.assign(this, { device, context, format, memory, x: exports, arena, url, profile, coarse, capture, onMedia });
    this.modules = new Map(); this.pipelines = new Map(); this.groups = new Map(); this.shaderMessages = [];this.ready = false;this.samplers = new Map();this.textureCaptures = [];this.layouts = new Map();this.requests = new Map();this.targets = new Map();this.generatedBuffers = new Map();this.buffers = [];this.pendingEffects = new Set();
  }
  info(field) { return number(this.x.coaxial_render_frame_info(field)); }
  op(index, field) { return number(this.x.coaxial_render_operation(index, field)); }
  pipeline(entry, field) { return number(this.x.coaxial_render_pipeline(entry, field)); }
  use(entry, slot, field) { return number(this.x.coaxial_render_use(entry, slot, field)); }
  binding(set, resource, field) { return number(this.x.coaxial_render_binding(set, resource, field)); }
  targetFormat(target) {
    const code = this.formatCodes[this.format] ?? this.ids.format.invalid;
    const status = this.x.coaxial_render_target(target, 1, 1, this.coarse, code, this.device.limits.maxTextureDimension2D);
    if (status !== 1) throw Error(`Coax target refused: ${target}/${status}`);
    return this.formats[number(this.x.coaxial_render_target_info(2))];
  }
  async initialize(previous = null) {
    this.previous = previous;this.reuses = previous?.x === this.x ? previous : null;
    if (previous) { previous.suspended = true;if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-await-frame";await previous.settled;if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-await-effects";await Promise.all(previous.pendingEffects); }
    try { if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-initialize";await this.initializeChecked(); }
    catch (error) {
      this.x.coaxial_render_abort?.();if (previous && previous.x === this.x) previous.suspended = false;
      if (!this.reuses) this.destroy();
      console.error("Coax renderer initialization", error);throw error;
    }
  }
  async initializeChecked() {
    const { x, device, memory, arena } = this;
    if (!x.coaxial_render_abi || x.coaxial_render_abi() !== 1) throw Error("engine has no compatible Coax renderer facade");
    const ids = (family, names) => Object.fromEntries(names.map((name, i) => [name, x.coaxial_render_enum(family, i)]));
    this.ids = {
      format: ids(0, ["rgba8", "bgra8", "rgba16f", "depth32", "invalid"]),
      kind: ids(1, ["uniform", "storage", "image2d", "image_array", "depth", "sampler", "comparison", "invalid"]),
      source: ids(2, ["invalid", "main_uniform", "media_uniform", "immutable_storage_asset", "dynamic_store", "particle_emitters", "mesh_asset", "layers", "sans_atlas", "render_target", "sampler", "media_image", "bloom_step", "pass_source"]),
      draw: ids(3, ["skip", "indexed", "nonindexed", "invalid"]),
      asset: ids(4, ["invalid", "data", "storage", "index", "layers", "sound", "loop", "particles", "mesh"]),
      readiness: ids(5, ["loading", "background", "ready", "invalid"]),
      media: ids(6, ["invalid", "image", "video"]),
    };
    const f = this.ids.format;
    this.formats = { [f.rgba8]: "rgba8unorm", [f.bgra8]: "bgra8unorm", [f.rgba16f]: "rgba16float", [f.depth32]: "depth32float" };
    this.formatCodes = { rgba8unorm: f.rgba8, bgra8unorm: f.bgra8, rgba16float: f.rgba16f };
    if (!x.coaxial_render_initialized()) x.coaxial_render_reset();
    x.coaxial_render_device(1, 1, 0); // WebGPU core has helper discard; its integer carrier is separately bounded.
    if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-fetch-package";
    const response = await fetch(this.url);
    if (!response.ok) throw Error(`renderer package: ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (x.coaxial_render_json_admit(bytes.length) !== 0) throw new RenderRefusal("Coax refused renderer package extent");
    new Uint8Array(memory.buffer, x.coaxial_render_json_ptr(), bytes.length).set(bytes);
    const admitted = x.coaxial_render_prepare(bytes.length, this.profile);
    if (admitted !== 0) throw new RenderRefusal(`Coax refused renderer package: ${admitted}`);
    // Only consume these observations after the compiled policy admitted all
    // fields against the actual catalog, shader interface and device profile.
    this.package = JSON.parse(new TextDecoder().decode(bytes));
    const effect = (field) => number(x.coaxial_render_media_effects(field));
    this.mediaEffects = { format: this.mediaText(effect(0), effect(1)), premultipliedAlpha: Boolean(effect(2)), resizeQuality: this.mediaText(effect(3), effect(4)) };
    if (this.reuses) {
      // The compiled catalog/profile is fixed. Keep initialized GPU resources,
      // retained upload prefixes and effectful producer retry state on reload.
      for (const key of ["assetRows","indexAssetRow","vertexBuffer","uniformBuffer","resourceRecords","indexRecord","layerTexture","layerRequest","sansTexture","textureCaptures","buffers","stageCursor","targets","generatedBuffers","samplers","mediaCache"]) this[key] = this.reuses[key];
    } else {
    this.readAssets();
    if (x.coaxial_render_buffer_sizes(arena.maxVertices, arena.floats, arena.uniformFloats, BigInt(device.limits.maxBufferSize)) !== 0) throw new RenderRefusal("Coax refused renderer buffer extents");
    this.vertexBuffer = device.createBuffer({ size: number(x.coaxial_render_buffer_size(0)), usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    this.uniformBuffer = device.createBuffer({ size: number(x.coaxial_render_buffer_size(1)), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.prepareBuffers();
    if (this.previous) this.retainEffects(this.previous);
    }
    for (const item of this.package.modules) {
      if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-fetch-module";
      const shaderResponse = await fetch(new URL(item.wgsl_file, new URL(this.url, location.href)));
      if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-module-response";
      if (!shaderResponse.ok) throw Error(`renderer module: ${shaderResponse.status}`);
      const shaderBytes = new Uint8Array(await shaderResponse.arrayBuffer());
      if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-module-bytes";
      if (!x.coaxial_render_shader_admit(shaderBytes.length)) throw new RenderRefusal("Coax refused shader extent");
      new Uint8Array(memory.buffer, x.coaxial_render_shader_ptr(), shaderBytes.length).set(shaderBytes);
      if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-module-admit";
      if (x.coaxial_render_candidate_module(item.binding_set, shaderBytes.length) !== 0) throw new RenderRefusal("Coax refused shader digest");
      const module = device.createShaderModule({ code: new TextDecoder().decode(shaderBytes) });
      if(this.capture)globalThis.coaxial.renderLifecyclePhase="renderer-module-info";
      const diagnostics = await module.getCompilationInfo();
      this.shaderMessages.push(...diagnostics.messages.map((m) => ({ type: m.type, message: m.message, line: m.lineNum, column: m.linePos })));
      if (diagnostics.messages.some((m) => m.type === "error")) throw Error(diagnostics.messages.map((m) => m.message).join("\n"));
      this.modules.set(item.binding_set, module);
    }
    for (const entry of this.package.entries) {
      if(this.capture)globalThis.coaxial.renderLifecyclePhase=`renderer-pipeline-${entry.entry_index}`;
      await this.makePipeline(entry);
      if (x.coaxial_render_candidate_pipeline(entry.entry_index, 1) !== 0) throw new RenderRefusal("Coax refused pipeline readiness");
    }
    if (x.coaxial_render_commit() !== 0) throw new RenderRefusal("Coax refused renderer transaction commit");
    this.ready = true;
  }
  retainEffects(previous) {
    this.retainedResources = new Set();
    for (let slot = 0; slot < this.assetRows.length; slot++) {
      const declared = this.assetRows[slot];
      const old = previous.assetRows.find((a) => a.i === declared.i && a.kindId === declared.kindId && JSON.stringify(a.urls) === JSON.stringify(declared.urls));
      if (!old) continue;
      // Async fetches retain this exact object. Carry the new declaration into
      // it instead of splitting completion/readiness across two generations.
      if (old.pack && declared.pack && old.pack.url === declared.pack.url) declared.pack = old.pack;
      Object.assign(old, declared);this.assetRows[slot] = old;
      if (old.bytes) this.acceptAsset(old);
      if (old.kindId === this.ids.asset.layers && old.texture) {
        this.layerTexture = old.texture;this.retainedResources.add(old.texture);
        this.textureCaptures.push(...previous.textureCaptures.filter((t) => t.name === this.sourceName(this.layerRequest.set, this.layerRequest.resource, true)));
      }
      if (old.done) this.assetComplete(old);
    }
    for (const record of this.resourceRecords.values()) {
      const old = previous.resourceRecords.get(record.name);
      if (!record.image || !old?.image || record.assetRow >= 0) continue;
      if (record.bytes !== old.bytes || record.sourceCapacity !== old.sourceCapacity) throw new RenderRefusal("producer storage layout changed during engine replacement");
      record.image.set(old.image);record.incoming = record.sourceCapacity;
      new Uint8Array(this.memory.buffer, record.address, record.bytes).set(record.image);
    }
  }
  readAssets() {
    const { x, memory } = this, count = x.coaxial_render_assets_begin(), packs = new Map();
    if (count < 0) throw new RenderRefusal("Coax refused asset profile");
    const text = (address) => new TextDecoder().decode(new Uint8Array(memory.buffer, address, x.coaxial_render_asset_text_bytes()));
    const kinds = Object.fromEntries(Object.entries(this.ids.asset).map(([name, value]) => [value, name]));
    this.assetRows = [];
    for (let row = 0; row < count; row++) {
      if (!x.coaxial_render_asset(row)) throw new RenderRefusal("Coax refused asset delivery row");
      const info = (field) => number(x.coaxial_render_asset_info(field));
      const asset = { i: info(0), kindId: info(1), kind: kinds[info(1)], packed: Boolean(info(2)), pack_part: info(3), pack_first: info(4), initial: Boolean(info(6)), gpu_required: Boolean(info(7)) };
      asset.name = text(x.coaxial_render_asset_name());asset.urls = [];
      for (let url = 0; url < info(5); url++) asset.urls.push(text(x.coaxial_render_asset_url(row, url)));
      asset.url = asset.urls[0];
      if (asset.packed) { if (!packs.has(asset.pack_first)) packs.set(asset.pack_first, { url: asset.url });asset.pack = packs.get(asset.pack_first); }
      this.assetRows.push(asset);
    }
    this.indexAssetRow = x.coaxial_render_index_asset();
  }
  assetComplete(asset) { this.x.coaxial_render_asset_complete(asset.i, asset.failed ? 0 : 1); }
  acceptAsset(asset) {
    const { x, device, memory } = this, data = asset.bytes;
    if (!data) return;
    for (const record of this.resourceRecords.values()) if (record.assetRow === asset.i) {
      if (x.coaxial_render_buffer_recipe(record.set, record.resource, BigInt(data.length), BigInt(data.length), BigInt(device.limits.maxBufferSize)) !== 1) throw new RenderRefusal("Coax refused immutable buffer extent");
      const bytes = number(x.coaxial_render_buffer_view_info(1)), address = this.stage(bytes), image = new Uint8Array(bytes);image.set(data);
      Object.assign(record, { bytes, address, image, sourceCapacity: data.length, logical: data.length, incoming: data.length,
        gpu: device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST }) });
      new Uint8Array(memory.buffer, address, bytes).set(image);
    }
    if (asset.i === this.indexAssetRow) {
      if (x.coaxial_render_index_recipe(BigInt(data.length), BigInt(device.limits.maxBufferSize)) !== 0) throw new RenderRefusal("Coax refused index allocation extent");
      const address = this.stage(data.length);new Uint8Array(memory.buffer, address, data.length).set(data);
      this.indexRecord = { address, bytes: data.length, incoming: data.length, image: data.slice(), gpu: device.createBuffer({ size: data.length, usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST }) };
    }
  }
  sourceName(set, resource, texture = false) {
    const address = this.x.coaxial_render_resource_name(set, resource, texture ? 1 : 0);
    const bytes = this.x.coaxial_render_resource_name_bytes();
    return new TextDecoder().decode(new Uint8Array(this.memory.buffer, address, bytes));
  }
  makeSampler(set, resource) {
    const key = `${set}/${resource}`;
    if (this.samplers.has(key)) return this.samplers.get(key);
    const { x } = this;
    if (x.coaxial_render_sampler(set, resource) !== 1) throw new RenderRefusal("Coax refused sampler descriptor");
    const info = (field) => number(x.coaxial_render_sampler_info(field));
    const descriptor = { addressModeU: ["clamp-to-edge", "repeat"][info(0)], addressModeV: ["clamp-to-edge", "repeat"][info(1)],
      minFilter: info(2) ? "linear" : "nearest", magFilter: info(3) ? "linear" : "nearest", mipmapFilter: info(4) ? "linear" : "nearest", maxAnisotropy: info(5) };
    if (info(6)) descriptor.compare = "less";
    const sampler = this.device.createSampler(descriptor);this.samplers.set(key, sampler);return sampler;
  }
  async makeSans(set, resource) {
    const { x, device, memory } = this;
    const info = (field) => number(x.coaxial_render_sans_info(field));
    const text = (field) => new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_render_sans_text(field), x.coaxial_render_sans_text_bytes(field)));
    const width = info(2), height = info(3), levelCount = info(1), format = text(1);
    if (x.coaxial_render_target(1, width, height, 0, this.ids.format.rgba8, device.limits.maxTextureDimension2D) !== 1) throw new RenderRefusal("Coax refused atlas device extent");
    const texture = device.createTexture({ size: [width, height], format, mipLevelCount: levelCount,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT | (this.capture ? GPUTextureUsage.COPY_SRC : 0) });
    const levels = [];
    for (let level = 0; level < levelCount; level++) {
      if (!x.coaxial_render_sans_level(level)) throw new RenderRefusal("Coax refused atlas mip");
      const w = number(x.coaxial_render_sans_level_info(0)), h = number(x.coaxial_render_sans_level_info(1)), fontPixels = number(x.coaxial_render_sans_level_info(2));
      const canvas = new OffscreenCanvas(w, h), context = canvas.getContext("2d");
      if (!context) throw Error("platform cannot rasterize the atlas");
      context.fillStyle = text(2);context.textBaseline = text(3);context.font = `${fontPixels}px ${text(0)}`;
      for (let glyph = 0; glyph < info(0); glyph++) {
        if (!x.coaxial_render_sans_glyph(level, glyph)) throw new RenderRefusal("Coax refused atlas glyph");
        context.fillText(String.fromCodePoint(x.coaxial_render_sans_glyph_code()), x.coaxial_render_sans_glyph_position(0), x.coaxial_render_sans_glyph_position(1));
      }
      device.queue.copyExternalImageToTexture({ source: canvas }, { texture, mipLevel: level, premultipliedAlpha: Boolean(info(4)) }, [w, h]);
      levels.push({ width: w, height: h });
    }
    if (this.capture) this.textureCaptures.push(await this.captureTexture(texture, this.sourceName(set, resource, true), format, levels));
    return texture;
  }
  async captureTexture(texture, name, format, levels, layers = 1) {
    this.device.pushErrorScope("validation");
    // Read back the exact uploaded GPU image. Padding is the API's physical
    // transfer pitch, recorded alongside each mip for both other backends.
    const encoder = this.device.createCommandEncoder(), reads = [];let total = 0;
    for (const [mip, level] of levels.entries()) {
      const row_bytes = Math.ceil(level.width * 4 / 256) * 256, layer_bytes = row_bytes * level.height;
      const buffer = this.device.createBuffer({ size: layer_bytes * layers, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
      encoder.copyTextureToBuffer({ texture, mipLevel: mip }, { buffer, bytesPerRow: row_bytes, rowsPerImage: level.height }, [level.width, level.height, layers]);
      reads.push({ buffer, width: level.width, height: level.height, offset: total, row_bytes, layer_bytes });total += layer_bytes * layers;
    }
    this.device.queue.submit([encoder.finish()]);const validation = this.device.popErrorScope();const bytes = new Uint8Array(total), metadata = [];
    for (const read of reads) {
      await read.buffer.mapAsync(GPUMapMode.READ);bytes.set(new Uint8Array(read.buffer.getMappedRange()), read.offset);read.buffer.unmap();read.buffer.destroy();
      const { buffer, ...level } = read;metadata.push(level);
    }
    const error = await validation;if (error) throw Error(error.message);
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
    const receipt = { name, format, layers, levels: metadata, sha256: [...digest].map((b) => b.toString(16).padStart(2, "0")).join("") };
    if (this.textureCaptureSink) { await this.textureCaptureSink(receipt, bytes);return receipt; }
    return { ...receipt, bytes: Array.from(bytes) };
  }
  makeLayers(images) {
    const effect = this.makeLayersEffect(images);this.pendingEffects.add(effect);
    effect.finally(() => this.pendingEffects.delete(effect)).catch(() => {});return effect;
  }
  async makeLayersEffect(images) {
    const { x, device, memory } = this, width = images[0]?.width ?? 0, height = images[0]?.height ?? 0;
    const mip = (level) => x.coaxial_render_mip(width, height, images.length, level, device.limits.maxTextureDimension2D, device.limits.maxTextureArrayLayers);
    if (!mip(0)) throw new RenderRefusal("Coax refused layer image/device extent");
    const count = number(x.coaxial_render_mip_info(2));x.coaxial_render_layers_begin();
    const text = (field) => new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_render_layers_text(field), x.coaxial_render_layers_text_bytes(field)));
    const format = text(0), resizeQuality = text(1), premultipliedAlpha = Boolean(x.coaxial_render_layers_premultiplied());
    const texture = device.createTexture({ size: [width, height, images.length], format, mipLevelCount: count,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT | (this.capture ? GPUTextureUsage.COPY_SRC : 0) });
    const levels = [];
    for (let level = 0; level < count; level++) {
      if (!mip(level)) throw new RenderRefusal("Coax refused layer mip");
      const w = number(x.coaxial_render_mip_info(0)), h = number(x.coaxial_render_mip_info(1));levels.push({ width: w, height: h });
      for (const [layer, image] of images.entries()) {
        const source = level === 0 && image.width === w && image.height === h ? image : await createImageBitmap(image, { resizeWidth: w, resizeHeight: h, resizeQuality });
        device.queue.copyExternalImageToTexture({ source }, { texture, mipLevel: level, origin: [0, 0, layer], premultipliedAlpha }, [w, h]);
      }
    }
    this.layerTexture = texture;
    if (this.capture) this.textureCaptures.push(await this.captureTexture(texture, this.sourceName(this.layerRequest.set, this.layerRequest.resource, true), format, levels, images.length));
    return texture;
  }
  async makePipeline(entry) {
    const { device, x } = this, index = entry.entry_index;
    if (!this.pipeline(index, 0)) throw new RenderRefusal(`Coax refused pipeline ${index}`);
    const groupCount = x.coaxial_render_group_count(index), layoutEntries = Array.from({ length: groupCount }, () => []), requests = [];
    const uses = x.coaxial_render_use_count(index);
    for (let slot = 0; slot < uses; slot++) {
      if (!this.use(index, slot, 0)) throw new RenderRefusal(`Coax refused binding ${index}/${slot}`);
      const set = this.use(index, slot, 1), resource = this.use(index, slot, 2), group = this.use(index, slot, 8), binding = this.use(index, slot, 9);
      const kind = this.binding(set, resource, 1), source = this.binding(set, resource, 6), visibility = this.use(index, slot, 7);
      const request = { set, resource, group, binding, kind, source };requests.push(request);
      if (kind === this.ids.kind.uniform) {
        if (x.coaxial_render_buffer_recipe(set, resource, 0n, 0n, BigInt(device.limits.maxBufferSize)) !== 1) throw new RenderRefusal("Coax refused uniform layout extent");
        layoutEntries[group].push({ binding, visibility, buffer: { type: "uniform", minBindingSize: number(x.coaxial_render_buffer_view_info(1)) } });
      } else if (kind === this.ids.kind.storage) {
        layoutEntries[group].push({ binding, visibility, buffer: { type: "read-only-storage", minBindingSize: this.binding(set, resource, 4) } });
      } else if (kind === this.ids.kind.sampler || kind === this.ids.kind.comparison) {
        this.makeSampler(set, resource);
        layoutEntries[group].push({ binding, visibility, sampler: { type: kind === this.ids.kind.comparison ? "comparison" : "filtering" } });
      } else {
        if (x.coaxial_render_texture_layout(set, resource) !== 1) throw new RenderRefusal("Coax refused texture layout");
        request.viewDimension = ["2d", "2d-array"][number(x.coaxial_render_texture_layout_info(1))];
        layoutEntries[group].push({ binding, visibility, texture: { sampleType: ["float", "unfilterable-float", "depth"][number(x.coaxial_render_texture_layout_info(0))], viewDimension: request.viewDimension } });
        if (source === this.ids.source.sans_atlas) this.sansTexture ||= await this.makeSans(set, resource);
        if (source === this.ids.source.layers) this.layerRequest ||= request;
      }
    }
    const layouts = layoutEntries.map((entries) => device.createBindGroupLayout({ entries }));
    this.layouts.set(index, layouts);this.requests.set(index, requests);
    const count = this.pipeline(index, 7), targets = [];
    for (let i = 0; i < count; i++) targets.push({ format: this.targetFormat(this.pipeline(index, 9 + i)),
      blend: { color: blendComponent(this.pipeline(index, 5), false), alpha: blendComponent(this.pipeline(index, 6), true) },
      writeMask: i === 1 && !this.pipeline(index, 8) ? 0 : GPUColorWrite.ALL });
    const module = this.modules.get(entry.binding_set);
    const buffers = entry.vertex_stride ? [{ arrayStride: entry.vertex_stride, attributes: entry.attributes.map((a) => ({ shaderLocation: a.location, offset: a.offset,
      format: ({ 2: "float32", 3: "sint32", 4: "uint32" })[a.scalar] + (a.width === 1 ? "" : `x${a.width}`) })) }] : [];
    const descriptor = { layout: device.createPipelineLayout({ bindGroupLayouts: layouts }), vertex: { module, entryPoint: entry.wgsl_vertex, buffers }, primitive: { topology: "triangle-list" } };
    if (entry.wgsl_fragment) descriptor.fragment = { module, entryPoint: entry.wgsl_fragment, targets };
    const depth = this.pipeline(index, 11);
    if (depth) descriptor.depthStencil = { format: this.targetFormat(depth), depthWriteEnabled: Boolean(this.pipeline(index, 2)),
      depthCompare: ({ 1: "always", 2: "less" })[this.pipeline(index, 1)], depthBias: this.pipeline(index, 3), depthBiasSlopeScale: this.pipeline(index, 4) };
    device.pushErrorScope("validation");
    let pipeline, error;
    try { if(this.capture)globalThis.coaxial.renderLifecyclePhase=`pipeline-${index}-create`;pipeline = await device.createRenderPipelineAsync(descriptor);if(this.capture)globalThis.coaxial.renderLifecyclePhase=`pipeline-${index}-created`; }
    finally { if(this.capture)globalThis.coaxial.renderLifecyclePhase=`pipeline-${index}-pop`;error = await device.popErrorScope();if(this.capture)globalThis.coaxial.renderLifecyclePhase=`pipeline-${index}-popped`; }
    if (error) throw Error(error.message);
    this.pipelines.set(index, pipeline);
  }
  stage(bytes) {
    const { x, memory } = this;
    this.stageCursor ??= BigInt(this.arena.uniforms) + BigInt(this.uniformBuffer.size);
    if (x.coaxial_render_stage(this.stageCursor, BigInt(bytes), x.coaxial_render_memory_limit()) !== 0) throw new RenderRefusal("Coax refused renderer staging allocation");
    const address = number(x.coaxial_render_stage_info(0)), pages = number(x.coaxial_render_stage_info(2));
    const have = memory.buffer.byteLength / 65536;
    if (pages > have) memory.grow(pages - have);
    this.stageCursor = x.coaxial_render_stage_info(1);return address;
  }
  mediaText(address, bytes) { return new TextDecoder().decode(new Uint8Array(this.memory.buffer, address, bytes)); }
  mediaKey(kind, url) { return `${kind} ${url}`; }
  mediaNotify() { this.onMedia?.([...this.mediaCache.values()].map(({ kind, url, ready, failed, width, height }) => ({ kind, url, ready, failed, width, height }))); }
  mediaAllocate(object, source, width, height) {
    const { x, device } = this;
    if (!x.coaxial_render_media_extent(BigInt(object.kind), BigInt(width), BigInt(height))) throw new RenderRefusal("Coax refused media image extent");
    const w = number(x.coaxial_render_media_extent_info(0)), h = number(x.coaxial_render_media_extent_info(1));
    if (!x.coaxial_render_mip(w, h, 1, 0, device.limits.maxTextureDimension2D, device.limits.maxTextureArrayLayers)) throw new RenderRefusal("Coax refused media device extent");
    object.texture?.destroy();object.texture = device.createTexture({ size: [w, h], format: this.mediaEffects.format,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT | (this.capture ? GPUTextureUsage.COPY_SRC : 0) });
    Object.assign(object, { width: w, height: h, source, ready: true });return [w, h];
  }
  mediaOpen(kind, url) {
    const object = { kind, url, ready: false, failed: false, width: 0, height: 0 }, key = this.mediaKey(kind, url);
    this.mediaCache.set(key, object);
    const current = () => this.mediaCache.get(key) === object;
    const fail = (error) => { if (current()) { object.failed = true;object.ready = false;object.error = String(error?.stack ?? error ?? "media decode failed");this.mediaNotify(); } };
    if (kind === this.ids.media.image) {
      fetch(url, { credentials: "same-origin" }).then((r) => { if (!r.ok) throw Error(`media: ${r.status}`);return r.blob(); })
        .then((b) => createImageBitmap(b)).then(async (image) => {
          if (!current()) { image.close();return; }
          const [w, h] = this.mediaAllocate(object, image, image.width, image.height);object.ready = false;
          if (w !== image.width || h !== image.height) {
            const resized = await createImageBitmap(image, { resizeWidth: w, resizeHeight: h, resizeQuality: this.mediaEffects.resizeQuality });image.close();object.source = resized;
          }
          if (current()) { object.ready = true;this.mediaNotify(); }else object.source?.close?.();
        }).catch(fail);
    } else if (kind === this.ids.media.video) {
      const video = document.createElement("video");Object.assign(video, { muted: true, defaultMuted: true, loop: true, playsInline: true, autoplay: true, preload: "auto" });
      video.setAttribute("muted", "");video.setAttribute("playsinline", "");object.element = video;
      video.addEventListener("loadeddata", () => { if (!current()) return;try { this.mediaAllocate(object, video, video.videoWidth, video.videoHeight);this.mediaNotify();video.play().catch(() => {}); } catch { fail(); } });
      video.addEventListener("error", fail);video.src = url;video.play().catch(() => {});
    } else throw new RenderRefusal("Coax returned an unsupported media kind");
    return object;
  }
  mediaClose(key, object) {
    this.mediaCache.delete(key);object.texture?.destroy();object.source?.close?.();
    if (object.element) { object.element.pause();object.element.removeAttribute("src");object.element.load(); }
  }
  prepareMedia(width, height) {
    const { x, memory, arena } = this;
    if (!this.pipelines.has(13)) return { text: "", images: [], draws: [] };
    this.mediaCache ||= new Map();
    const bytes = x.coaxial_media_len?.(arena.state) ?? 0, address = bytes ? x.coaxial_media(arena.state) : 0;
    if (x.coaxial_render_media_parse(address, bytes, memory.buffer.byteLength) !== 0) throw new RenderRefusal("Coax refused media command source");
    const text = this.mediaText(x.coaxial_render_media_text_ptr(), bytes), requests = [], wanted = new Set();
    for (let row = 0; row < x.coaxial_render_media_count(); row++) {
      const info = (field) => number(x.coaxial_render_media_request(row, field));
      const kind = info(0), line = info(1), url = this.mediaText(info(3), info(4)), key = this.mediaKey(kind, url);
      wanted.add(key);requests.push({ row, kind, line, url, object: this.mediaCache.get(key) || this.mediaOpen(kind, url) });
    }
    for (const [key, object] of this.mediaCache) if (!wanted.has(key)) this.mediaClose(key, object);
    const images = [];let slot = 0;
    for (const object of this.mediaCache.values()) {
      if (object.element && object.ready && (object.width !== object.element.videoWidth || object.height !== object.element.videoHeight)) this.mediaAllocate(object, object.element, object.element.videoWidth, object.element.videoHeight);
      if (object.urlAddress === undefined) {
        const encoded = new TextEncoder().encode(object.url);
        if (x.coaxial_render_allocation(BigInt(encoded.length), x.coaxial_render_memory_limit()) !== 1) throw new RenderRefusal("Coax refused media identity storage");
        object.urlAddress = this.stage(number(x.coaxial_render_buffer_view_info(1)));object.urlBytes = encoded.length;new Uint8Array(memory.buffer, object.urlAddress, encoded.length).set(encoded);
      }
      const observed = { kind: object.kind, url: object.url, ready: object.ready ? 1 : 0, width: object.width, height: object.height };
      if (x.coaxial_render_media_observation(slot++, BigInt(observed.kind), object.urlAddress, object.urlBytes, memory.buffer.byteLength, BigInt(observed.ready), BigInt(observed.width), BigInt(observed.height)) !== 0) throw new RenderRefusal("Coax refused actual media observation");
      images.push(observed);
    }
    let textureSlot = 0;
    for (const request of requests) if (request.object.ready) {
      const texture = request.object.texture;
      if (x.coaxial_render_media_texture(textureSlot, request.line, BigInt(texture.width), BigInt(texture.height), BigInt(texture.depthOrArrayLayers), texture.format === "depth32float" ? 1 : 0) !== 0) throw new RenderRefusal("Coax refused media texture transport");
      request.textureName = this.mediaText(number(x.coaxial_render_media_texture_name(textureSlot, 0)), number(x.coaxial_render_media_texture_name(textureSlot, 1)));textureSlot++;
    }
    if (x.coaxial_render_media_compose(width, height) !== 0) throw new RenderRefusal("Coax refused media physical resources");
    const draws = [];
    for (const request of requests) if (x.coaxial_render_media_rect(request.row, 0) === 1n) {
      const info = (field) => number(x.coaxial_render_media_rect(request.row, field));
      draws.push({ ...request, line: info(1), over: Boolean(info(2)), address: info(3), bytes: info(4) });
    }
    return { text, images, draws };
  }

  prepareBuffers() {
    const { x, device, arena } = this;this.resourceRecords = new Map();
    for (const module of this.package.modules) {
      const set = module.binding_set;
      for (let resource = 0; resource < x.coaxial_render_binding_count(set); resource++) {
        const source = this.binding(set, resource, 6), name = this.sourceName(set, resource);
        if (source === this.ids.source.layers) this.layerRequest ||= { set, resource };
        if (!name || this.resourceRecords.has(name)) continue;
        const record = { id: this.resourceRecords.size, set, resource, source, name, index: this.binding(set, resource, 7) };
        record.assetRow = x.coaxial_render_asset_for_source(set, resource);
        if (source === this.ids.source.main_uniform) {
          Object.assign(record, { gpu: this.uniformBuffer, address: arena.uniforms, sourceCapacity: this.uniformBuffer.size, logical: this.uniformBuffer.size, bytes: this.uniformBuffer.size });
        } else if (source === this.ids.source.mesh_asset) {
          if (x.coaxial_render_mesh_placeholder(set, resource, BigInt(device.limits.maxBufferSize)) !== 1) throw new RenderRefusal("Coax refused pending mesh placeholder");
          const bytes = number(x.coaxial_render_buffer_view_info(1)), address = this.stage(bytes), image = new Uint8Array(bytes);
          new Uint8Array(this.memory.buffer, address, bytes).set(image);
          Object.assign(record, { sourceCapacity: bytes, logical: bytes, bytes, address, image,
            gpu: device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST }) });
        } else if (source === this.ids.source.dynamic_store || source === this.ids.source.particle_emitters) {
          const capacity = source === this.ids.source.dynamic_store ? x.coaxial_store_cap(record.index) : x.coaxial_particle_cap();
          if (x.coaxial_render_buffer_recipe(set, resource, BigInt(capacity), 0n, BigInt(device.limits.maxBufferSize)) !== 1) throw new RenderRefusal("Coax refused producer buffer allocation");
          const bytes = number(x.coaxial_render_buffer_view_info(1)), address = this.stage(bytes);
          Object.assign(record, { sourceCapacity: capacity, logical: 0, bytes, address, image: new Uint8Array(bytes),
            gpu: device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST }) });
        }
        this.resourceRecords.set(name, record);
      }
    }
  }
  prepareFrameBuffers() {
    const { x, memory, arena } = this;const active = [];
    for (const record of this.resourceRecords.values()) {
      if (!record.gpu) continue;
      if (record.source === this.ids.source.dynamic_store || record.source === this.ids.source.particle_emitters) {
        const version = record.source === this.ids.source.dynamic_store ? x.coaxial_store_version?.(arena.state, record.index) : undefined;
        if (version === undefined || version !== record.version) {
          // The producer may consume an upload retry here. Read exactly once;
          // the admitted retained prefix remains queued through any refusal.
          const incoming = record.source === this.ids.source.dynamic_store ? x.coaxial_store_bytes(arena.state, record.index) : x.coaxial_particle_bytes(arena.state);
          let address;
          if (incoming) {
            address = record.source === this.ids.source.dynamic_store ? x.coaxial_store(arena.state, record.index) : x.coaxial_particles(arena.state);
            if (x.coaxial_render_source_words(address, incoming, memory.buffer.byteLength) !== 0) throw new RenderRefusal("Coax refused producer source span");
          }
          if (x.coaxial_render_upload_stage(record.id, BigInt(incoming), BigInt(record.sourceCapacity)) !== 0) throw new RenderRefusal("Coax refused producer upload extent");
          if (incoming) {
            record.image.set(new Uint8Array(memory.buffer, address, incoming));record.version = version;
          }
        }
        new Uint8Array(memory.buffer, record.address, record.bytes).set(record.image);
      }
      if (record.incoming !== undefined) {
        if (x.coaxial_render_upload_stage(record.id, BigInt(record.incoming), BigInt(record.sourceCapacity)) !== 0) throw new RenderRefusal("Coax refused immutable upload prefix");
        delete record.incoming;
      }
      active.push(record);
    }
    if (!x.coaxial_render_named_count(active.length)) throw Error("renderer transport has no room for admitted sources");
    for (const [slot, record] of active.entries()) {
      const upload = record.source === this.ids.source.main_uniform ? 0n : x.coaxial_render_upload_pending(record.id);
      if (!x.coaxial_render_named(slot, record.set, record.resource, record.address, record.bytes, memory.buffer.byteLength,
        BigInt(record.sourceCapacity), BigInt(record.logical), 0n, upload, BigInt(record.gpu.size), 0n, 4n, 1)) throw new RenderRefusal("Coax refused a named source span");
    }
    this.buffers = active;
    if (this.indexRecord?.incoming !== undefined) {
      if (x.coaxial_render_index_stage(BigInt(this.indexRecord.incoming), BigInt(this.indexRecord.bytes)) !== 0) throw new RenderRefusal("Coax refused index upload prefix");
      delete this.indexRecord.incoming;
    }
  }
  target(id) {
    if (!id) return null;
    if (id === 1) return this.lastTexture;
    const { x, device } = this;
    if (x.coaxial_render_target(id, this.width, this.height, this.coarse, this.formatCodes[this.format] ?? this.ids.format.invalid, device.limits.maxTextureDimension2D) !== 1) throw new RenderRefusal("Coax refused frame attachment extent");
    const width = number(x.coaxial_render_target_info(0)), height = number(x.coaxial_render_target_info(1)), format = this.formats[number(x.coaxial_render_target_info(2))];
    const previous = this.targets.get(id);
    if (previous?.width === width && previous?.height === height && previous?.format === format) return previous;
    const texture = device.createTexture({ size: [width, height], format, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST });
    previous?.destroy();this.targets.set(id, texture);return texture;
  }
  makeGroups(index, operationIndex, media = null) {
    const { x, device, memory } = this, layouts = this.layouts.get(index), entries = layouts.map(() => []), writes = [];
    for (const request of this.requests.get(index)) {
      const { set, resource, group, binding, kind, source } = request;let physical;
      if (kind === this.ids.kind.uniform || kind === this.ids.kind.storage) {
        let buffer;
        if (source === this.ids.source.bloom_step || source === this.ids.source.media_uniform) {
          const generatedKey = media ? `media-${media.line}` : `bloom-${operationIndex}`;
          buffer = this.generatedBuffers.get(generatedKey);
          if (!buffer) {
            if (x.coaxial_render_buffer_recipe(set, resource, 0n, 0n, BigInt(device.limits.maxBufferSize)) !== 1) throw new RenderRefusal("Coax refused generated uniform allocation");
            buffer = device.createBuffer({ size: number(x.coaxial_render_buffer_view_info(1)), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });this.generatedBuffers.set(generatedKey, buffer);
          }
          const address = media ? media.address : x.coaxial_render_bloom(operationIndex), bytes = media ? media.bytes : x.coaxial_render_bloom_bytes();
          if (x.coaxial_render_generated_buffer(set, resource, address, bytes, memory.buffer.byteLength, BigInt(buffer.size)) !== 1) throw new RenderRefusal("Coax refused bloom uniform view");
          writes.push({ buffer, address: number(x.coaxial_render_resolved_info(3)), bytes: number(x.coaxial_render_resolved_info(4)) });
        } else {
          if (x.coaxial_render_resolve_buffer(set, resource) !== 1) throw new RenderRefusal(`Coax refused buffer view ${set}/${resource}`);
          buffer = this.buffers[number(x.coaxial_render_resolved_info(0))]?.gpu;
        }
        if (!buffer) throw Error("admitted buffer has no platform allocation");
        physical = { buffer, offset: number(x.coaxial_render_resolved_info(1)), size: number(x.coaxial_render_resolved_info(2)) };
      } else if (kind === this.ids.kind.sampler || kind === this.ids.kind.comparison) physical = this.makeSampler(set, resource);
      else {
        const texture = source === this.ids.source.sans_atlas ? this.sansTexture
          : source === this.ids.source.render_target ? this.target(this.binding(set, resource, 8))
          : source === this.ids.source.pass_source ? this.target(this.op(operationIndex, 7))
          : source === this.ids.source.layers ? this.layerTexture
          : source === this.ids.source.media_image ? media?.object.texture : null;
        if (!texture) throw Error("admitted image has no platform allocation");
        if (x.coaxial_render_texture_shape(set, resource, texture.depthOrArrayLayers, texture.format === "depth32float" ? 1 : 0, media?.line ?? -1) !== 1) throw new RenderRefusal("Coax refused actual texture shape");
        physical = texture.createView({ dimension: request.viewDimension });
      }
      entries[group].push({ binding, resource: physical });
    }
    return { groups: layouts.map((layout, group) => device.createBindGroup({ layout, entries: entries[group] })), writes };
  }
  frame(count, width, height, observe) {
    try { return this.frameChecked(count, width, height, observe); }
    catch (error) { if (error instanceof RenderRefusal) { this.lastRefusal = { phase: "resources", message: error.message };return false; }throw error; }
  }
  frameChecked(count, width, height, observe) {
    this.observation = null;
    if (!this.ready || this.busy || this.suspended) return false;
    if (this.error) throw this.error;
    const { x, device, arena, memory } = this;
    const measured = (name) => x[name]?.(arena.state) ?? 0;
    this.lastRequested = count;this.width = width;this.height = height;
    const counts = { draws: measured("coaxial_draw_count"), water: measured("coaxial_water_draws"), grass: measured("coaxial_grass_instances"),
      particles: measured("coaxial_particle_instances"), surface: measured("coaxial_surface_vertices"), caster_vertices: measured("coaxial_caster_vertices"), caster_instances: measured("coaxial_caster_instances") };
    if (x.coaxial_render_counts(width, height, count, arena.floats, counts.draws, counts.water, counts.grass, counts.particles, counts.surface, counts.caster_vertices, counts.caster_instances) !== 1) {
      this.lastRefusal = { phase: "counts", requested: { count, width, height, ...counts } };return false;
    }
    this.prepareFrameBuffers();
    if (x.coaxial_render_asset_readiness(1, this.pipelines.has(9) ? 1 : 0) !== 1) { this.lastRefusal = { phase: "assets" };return false; }
    const readiness = number(x.coaxial_render_asset_readiness_info(0)), particlesReady = number(x.coaxial_render_asset_readiness_info(1));
    this.readiness = readiness;
    const status = x.coaxial_render_scene_frame(arena.vertices, arena.uniforms, arena.maxVertices, arena.uniformFloats,
      x.coaxial_draws?.(arena.state) ?? 0, this.indexRecord?.address ?? 0, this.indexRecord?.bytes ?? 0, memory.buffer.byteLength,
      this.vertexBuffer.size, this.uniformBuffer.size, this.indexRecord?.gpu.size ?? 0, readiness, particlesReady);
    if (status !== 1) { this.lastRefusal = { phase: "frame", status, frame: this.info(2), geometry: this.info(3), requested: counts };return false; }
    const media = this.prepareMedia(width, height);
    this.lastRefusal = null;
    // No GPU writes or commands precede the complete admitted receipt.
    const vertexBytes = this.info(7), uniformBytes = this.info(8), active = this.buffers.slice();
    const sourceSnapshot = observe ? {
      requested_hud: count, admitted_hud: this.info(11), admitted_draws: this.info(14), width, height, counts, coarse: this.coarse,
      vertex_stride: arena.floats, vertex_gpu_bytes: this.vertexBuffer.size, uniform_gpu_bytes: this.uniformBuffer.size,
      uniform_bytes: Array.from(new Uint8Array(memory.buffer, this.info(10), uniformBytes)),
      vertex_bytes: Array.from(new Uint8Array(memory.buffer, this.info(9), vertexBytes)),
      draw_bytes: Array.from(new Uint8Array(memory.buffer, this.info(16), this.info(17))),
      index_bytes: Array.from(new Uint8Array(memory.buffer, this.info(18), this.info(19))),
      buffers: active.map((record) => ({ name: record.name, bytes: Array.from(new Uint8Array(memory.buffer, record.address, record.bytes)),
        source_capacity: record.sourceCapacity, logical_payload: record.logical, initialized_highwater: 0,
        upload_bytes: record.source === this.ids.source.main_uniform ? 0 : number(x.coaxial_render_upload_pending(record.id)), gpu_bytes: record.gpu.size, zero_initialized: 1 })),
      package: this.package, shader_messages: this.shaderMessages.slice(), textures: this.textureCaptures.slice(), media_text: media.text, media_images: media.images,
    } : null;
    device.pushErrorScope("validation");this.busy = true;
    try {
      this.lastTexture = this.context.getCurrentTexture();
      const prepared = new Map();
      for (let i = 0; i < this.info(1); i++) if (this.op(i, 0) === 2) prepared.set(i, this.makeGroups(this.op(i, 1), i));
      const mediaGroups = new Map(media.draws.map((draw) => [draw.row, this.makeGroups(13, -1, draw)]));
      for (const object of this.mediaCache?.values() ?? []) if (object.ready && object.source) {
        device.queue.copyExternalImageToTexture({ source: object.source }, { texture: object.texture, premultipliedAlpha: this.mediaEffects.premultipliedAlpha }, [object.width, object.height]);
        if (!object.element) { object.source.close?.();object.source = null; }
      }
      if (vertexBytes) device.queue.writeBuffer(this.vertexBuffer, 0, new Uint8Array(memory.buffer, this.info(9), vertexBytes));
      if (uniformBytes) device.queue.writeBuffer(this.uniformBuffer, 0, new Uint8Array(memory.buffer, this.info(10), uniformBytes));
      for (const record of active) {
        const bytes = number(x.coaxial_render_upload_pending(record.id));
        if (bytes) device.queue.writeBuffer(record.gpu, 0, new Uint8Array(memory.buffer, record.address, bytes));
      }
      const indexBytes = number(x.coaxial_render_index_pending());
      if (indexBytes) device.queue.writeBuffer(this.indexRecord.gpu, 0, new Uint8Array(memory.buffer, this.indexRecord.address, indexBytes));
      const encoder = device.createCommandEncoder(), each = [];let pass;
      for (const binding of [...prepared.values(), ...mediaGroups.values()]) for (const write of binding.writes) device.queue.writeBuffer(write.buffer, 0, new Uint8Array(memory.buffer, write.address, write.bytes));
      const clear = { r: x.coaxial_render_clear(0), g: x.coaxial_render_clear(1), b: x.coaxial_render_clear(2), a: x.coaxial_render_clear(3) };
      const recordDraw = (record) => {
        const kind = x.coaxial_render_decode(record);
        if (kind === this.ids.draw.skip) return;
        const first = number(x.coaxial_render_decoded(0)), vertices = number(x.coaxial_render_decoded(1)), base = number(x.coaxial_render_decoded(2)), instances = number(x.coaxial_render_decoded(3));
        if (kind === this.ids.draw.nonindexed) pass.draw(vertices, instances, first, base);
        else if (kind === this.ids.draw.indexed) { pass.setIndexBuffer(this.indexRecord.gpu, "uint32");pass.drawIndexed(vertices, instances, first, 0, base); }
        else throw Error("admitted draw decoder returned invalid");
      };
      for (let i = 0; i < this.info(1); i++) {
        const kind = this.op(i, 0);
        if (kind === 0) {
          const colours = [this.op(i, 2), this.op(i, 3)].filter(Boolean).map((id) => ({ view: this.target(id).createView(), loadOp: this.op(i, 5) ? "load" : "clear", storeOp: "store", clearValue: clear }));
          const depth = this.target(this.op(i, 4));
          pass = encoder.beginRenderPass({ colorAttachments: colours, ...(depth ? { depthStencilAttachment: { view: depth.createView(), depthClearValue: x.coaxial_render_clear(4), depthLoadOp: this.op(i, 5) ? "load" : "clear", depthStoreOp: this.op(i, 6) ? "store" : "discard" } } : {}) });
        } else if (kind === 1) { pass.end();pass = null;
        } else if (kind === 2) {
          const entry = this.op(i, 1);pass.setPipeline(this.pipelines.get(entry));
          prepared.get(i).groups.forEach((group, slot) => pass.setBindGroup(slot, group));
          if (this.package.entries.find((p) => p.entry_index === entry).vertex_stride) pass.setVertexBuffer(0, this.vertexBuffer);
          const source = this.op(i, 9);
          if (source === 0) pass.draw(this.op(i, 11), this.op(i, 12), this.op(i, 10), 0);
          else if (source === 1) for (let record = this.op(i, 10); record < this.op(i, 10) + this.op(i, 11); record++) recordDraw(record);
          else if (source === 2) recordDraw(each[each.length - 1].record);
          else throw Error("admitted operation has unknown draw source");
        } else if (kind === 3) {
          const source = this.target(this.op(i, 7)), destination = this.target(this.op(i, 8));
          const aspect = source.format === "depth32float" ? "depth-only" : "all";
          encoder.copyTextureToTexture({ texture: source, aspect }, { texture: destination, aspect }, [destination.width, destination.height]);
        } else if (kind === 4) each.push({ begin: i, record: this.op(i, 10), end: this.op(i, 10) + this.op(i, 11) });
        else if (kind === 5) { const loop = each[each.length - 1];loop.record++;if (loop.record < loop.end) i = loop.begin;else each.pop(); }
        else if (kind === 6 || kind === 7) {
          for (const draw of media.draws) if (draw.over === (kind === 7)) {
            pass.setPipeline(this.pipelines.get(13));mediaGroups.get(draw.row).groups.forEach((group, slot) => pass.setBindGroup(slot, group));
            pass.draw(x.coaxial_render_media_vertices());
          }
        } else throw Error(`unknown admitted operation ${kind}`);
      }
      this.observation = observe?.(encoder, this.lastTexture);
      if (this.observation) this.observation.rendererSnapshot = sourceSnapshot;
      device.queue.submit([encoder.finish()]);
      this.settled = device.popErrorScope().then((error) => {
        if (error) this.error = error;
        else { for (const record of active) x.coaxial_render_upload_ack(record.id, 1);x.coaxial_render_index_ack(1); }
        return { ok: !error, error: error?.message ?? null };
      }).finally(() => { this.busy = false; });
      if (this.observation) {
        this.observation.validation = this.settled;
        this.observation.textureCapture = Promise.all(media.draws.map((draw) => this.captureTexture(draw.object.texture, draw.textureName, draw.object.texture.format, [{ width: draw.object.width, height: draw.object.height }])));
      }
      return true;
    } catch (error) {
      this.busy = false;device.popErrorScope().catch(() => {});throw error;
    }
  }
  destroy(keep = new Set()) {
    const resources = new Set([this.vertexBuffer,this.uniformBuffer,this.sansTexture,this.layerTexture,this.indexRecord?.gpu,
      ...[...(this.resourceRecords?.values() ?? [])].map((r) => r.gpu),...this.generatedBuffers.values(),...this.targets.values()]);
    for (const object of this.mediaCache?.values() ?? []) { object.element?.pause();resources.add(object.texture);object.source?.close?.(); }
    this.mediaCache?.clear();
    for (const resource of resources) if (resource && !keep.has(resource)) resource.destroy();
    this.ready = false;
  }
}
