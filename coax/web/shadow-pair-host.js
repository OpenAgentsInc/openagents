// Firm reason: 1 (WebGPU effects and observation for the paired shadow experiment).
// Loaded only by shadow-pair.coax in fresh muted headless profiles. Diagnostic
// interventions deliberately change shadows and never qualify visual parity.
(() => {
  const config = globalThis.shadowPairConfig;
  globalThis.coaxialClip = true;
  globalThis.coaxialPassTimer = true;
  globalThis.coaxialFrameTrace = true;
  Date.now = () => 1794009600000;
  localStorage.setItem('verse.quality', 'high');
  const witness = globalThis.shadowPairWitness = {hashes: [], writes: 0, binds: 0, clears: 0, overrides: 0, reads: 0, gpuDeltaNs: 0};
  // Observe the host's 32-slot timestamp readbacks before its mapped range is
  // copied/unmapped. Stale bytes cannot pass as another completed frame.
  const readbacks = new WeakSet(); let previousEnd = null;
  const makeBuffer = GPUDevice.prototype.createBuffer;
  GPUDevice.prototype.createBuffer = function(d) {
    const b = makeBuffer.call(this, d);
    if (d.size === 256 && d.usage === (GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ)) readbacks.add(b);
    return b;
  };
  const mapped = GPUBuffer.prototype.getMappedRange;
  GPUBuffer.prototype.getMappedRange = function(...args) {
    const bytes = mapped.apply(this, args);
    if (readbacks.has(this)) {
      const ticks = new BigUint64Array(bytes); let end = 0n;
      for (let i = 1; i < ticks.length; i += 2) if (ticks[i] > end) end = ticks[i];
      witness.gpuDeltaNs = previousEnd === null ? 0 : Number(end - previousEnd);
      previousEnd = end; witness.reads++;
    }
    return bytes;
  };
  const module = GPUDevice.prototype.createShaderModule;
  GPUDevice.prototype.createShaderModule = function(d) {
    if (d.code.includes('fn coax_p_eg_casc_lit(')) {
      witness.hashes.push(crypto.subtle.digest('SHA-256', new TextEncoder().encode(d.code))
        .then(bytes => Array.from(new Uint8Array(bytes), x => x.toString(16).padStart(2, '0')).join('')));
    }
    return module.call(this, d);
  };
  if (config.shader) {
    const fetchOriginal = globalThis.fetch.bind(globalThis);
    globalThis.fetch = async function(input, ...rest) {
      const url = new URL(typeof input === 'string' ? input : input.url, location.href);
      if (url.pathname === '/everglade5e.wgsl') {
        witness.overrides++;
        return fetchOriginal(config.shader, ...rest);
      }
      return fetchOriginal(input, ...rest);
    };
  }
  const receiver = config.legacy || config.one;
  if (receiver) {
    const uniforms = new WeakMap(), descriptions = new WeakMap(), groups = new WeakMap(), passes = new WeakSet();
    const create = GPUDevice.prototype.createBuffer;
    GPUDevice.prototype.createBuffer = function(d) {
      const buffer = create.call(this, d);
      if (d.size === config.uniformFloats * 4 && d.usage === (GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST)) uniforms.set(buffer, create.call(this, d));
      return buffer;
    };
    const write = GPUQueue.prototype.writeBuffer;
    GPUQueue.prototype.writeBuffer = function(buffer, offset, data, ...rest) {
      if (uniforms.has(buffer) && data instanceof Float32Array && data.length === config.uniformFloats) {
        const copy = data.slice();
        if (config.legacy) copy[config.cascadeOffset] = 0;
        if (config.one) copy[config.tapsOffset] = 1;
        write.call(this, uniforms.get(buffer), offset, copy, ...rest);
        witness.writes++;
      }
      return write.call(this, buffer, offset, data, ...rest);
    };
    const group = GPUDevice.prototype.createBindGroup;
    GPUDevice.prototype.createBindGroup = function(d) {const g = group.call(this, d); descriptions.set(g, {d, device: this}); return g;};
    const begin = GPUCommandEncoder.prototype.beginRenderPass;
    GPUCommandEncoder.prototype.beginRenderPass = function(d) {const p = begin.call(this, d); if (d.colorAttachments?.length) passes.add(p); return p;};
    const bind = GPURenderPassEncoder.prototype.setBindGroup;
    GPURenderPassEncoder.prototype.setBindGroup = function(index, g, ...rest) {
      const description = descriptions.get(g);
      if (passes.has(this) && description && description.d.entries.some(e => uniforms.has(e.resource?.buffer))) {
        if (!groups.has(g)) {
          const {d, device} = description;
          const entries = d.entries.map(e => uniforms.has(e.resource?.buffer) ? {...e, resource: {...e.resource, buffer: uniforms.get(e.resource.buffer)}} : e);
          groups.set(g, group.call(device, {...d, entries}));
        }
        g = groups.get(g); witness.binds++;
      }
      return bind.call(this, index, g, ...rest);
    };
  }
  if (config.clear) {
    const begin = GPUCommandEncoder.prototype.beginRenderPass;
    GPUCommandEncoder.prototype.beginRenderPass = function(d) {
      if (!d.colorAttachments?.length && d.depthStencilAttachment?.depthLoadOp === 'load') {
        d = {...d, depthStencilAttachment: {...d.depthStencilAttachment, depthLoadOp: 'clear', depthClearValue: 1}};
        witness.clears++;
      }
      return begin.call(this, d);
    };
  }
})();
