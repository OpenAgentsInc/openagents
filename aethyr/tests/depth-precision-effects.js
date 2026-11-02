// Firm reason: 1 (browser-only typed WebGPU uploads, D32/LESS draws and raw readbacks).
// Coax authors every fixture/descriptor and owns predictions and verdicts.
globalThis.depthPrecisionStart = request => {
  globalThis.depthPrecisionJob = { done: false, value: null, error: null };
  (async () => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("no WebGPU adapter");
    const device = await adapter.requestDevice(), errors = [];
    device.addEventListener("uncapturederror", e => errors.push(e.error.message));
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code: request.code });
    const messages = (await module.getCompilationInfo()).messages.map(m => ({ type: m.type, message: m.message, line: m.lineNum, column: m.linePos }));
    const layout = device.createBindGroupLayout({ entries: request.layout });
    const uniform = device.createBuffer(request.uniform);
    const group = device.createBindGroup({ layout, entries: [{ binding: 0, resource: { buffer: uniform } }] });
    const pipeline = await device.createRenderPipelineAsync({ layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
      vertex: { module, entryPoint: request.vertex }, fragment: { module, entryPoint: request.fragment, targets: [{ format: request.format }] },
      primitive: request.primitive, depthStencil: request.depthStencil });
    const rows = [];
    for (const row of request.cases) {
      device.queue.writeBuffer(uniform, 0, new Uint32Array(row.clip_words));
      const color = device.createTexture({ size: row.size, format: request.format, usage: request.target_usage });
      const depth = device.createTexture({ size: row.size, format: request.depthStencil.format, usage: request.target_usage });
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({ colorAttachments: [{ view: color.createView(), loadOp: "clear", storeOp: "store", clearValue: request.clear }],
        depthStencilAttachment: { view: depth.createView(), depthLoadOp: "clear", depthStoreOp: "store", depthClearValue: request.depth_clear } });
      pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.draw(row.vertices); pass.end();
      const reads = [], z = device.createBuffer(row.depth_readback);
      encoder.copyTextureToBuffer({ texture: depth, aspect: "depth-only" }, { buffer: z, bytesPerRow: request.readback_row_bytes }, row.extent);
      for (const sample of row.samples) {
        const rgba = device.createBuffer(request.readback);
        encoder.copyTextureToBuffer({ texture: color, origin: sample.origin }, { buffer: rgba, bytesPerRow: request.readback_row_bytes }, [1, 1, 1]);
        reads.push([rgba, sample.depth_byte_offset]);
      }
      device.queue.submit([encoder.finish()]);
      const samples = [];
      await z.mapAsync(GPUMapMode.READ); const mappedDepth = z.getMappedRange();
      for (const [rgba, offset] of reads) {
        await rgba.mapAsync(GPUMapMode.READ);
        samples.push({ rgba_words: Array.from(new Uint32Array(rgba.getMappedRange(), 0, 4)), depth_word: new Uint32Array(mappedDepth, offset, 1)[0] });
        rgba.unmap(); rgba.destroy();
      }
      z.unmap(); z.destroy();
      rows.push(samples); color.destroy(); depth.destroy();
    }
    const scoped = await device.popErrorScope(); if (scoped) errors.push(scoped.message);
    const i = adapter.info;
    const value = { format: request.format, depth_format: request.depthStencil.format,
      device: { vendor: i.vendor, architecture: i.architecture, device: i.device, description: i.description }, messages, errors, rows };
    uniform.destroy(); device.destroy(); return value;
  })().then(value => { globalThis.depthPrecisionJob = { done: true, value, error: null }; },
    error => { globalThis.depthPrecisionJob = { done: true, value: null, error: String(error?.stack ?? error) }; });
  return "started";
};
globalThis.depthPrecisionPoll = () => globalThis.depthPrecisionJob;
"ready";
