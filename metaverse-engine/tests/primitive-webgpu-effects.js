// Firm reason: 1 (runs only inside headless Chrome; request-supplied typed WebGPU resources, uploads, draws and raw readbacks).
// Coax supplies all data/descriptors and owns fixture interpretation/verdicts.
globalThis.aethyrPrimitiveGPU = async request => {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw Error("no WebGPU adapter");
  const device = await adapter.requestDevice(), errors = [], messages = [];
  device.addEventListener("uncapturederror", event => errors.push(event.error.message));
  device.pushErrorScope("validation");
  const data = async upload => {
    if (upload.url) return new Uint8Array(await (await fetch(upload.url)).arrayBuffer());
    if (upload.type === "u16") return new Uint16Array(upload.values);
    if (upload.type === "f32") return new Float32Array(upload.values);
    return new Uint8Array(upload.values);
  };
  const buffers = [];
  for (const item of request.buffers) {
    const buffer = device.createBuffer(item.descriptor);
    if (item.upload) device.queue.writeBuffer(buffer, 0, await data(item.upload));
    buffers.push(buffer);
  }
  const images = [];
  for (const item of request.images) {
    const image = device.createTexture(item.descriptor), bytes = await data(item.upload);
    for (const level of item.levels) {
      device.queue.writeTexture({ texture: image, mipLevel: level.level }, bytes, level.layout, level.size);
    }
    images.push(image);
  }
  const samplers = request.samplers.map(descriptor => device.createSampler(descriptor));
  const pipelines = [];
  for (const item of request.programs) {
    const module = device.createShaderModule({ code: item.code });
    messages.push(...(await module.getCompilationInfo()).messages.map(message => ({
      type: message.type, message: message.message, line: message.lineNum, column: message.linePos
    })));
    const layout = item.layouts === "auto" ? "auto" : device.createPipelineLayout({
      bindGroupLayouts: item.layouts.map(entries => device.createBindGroupLayout({ entries }))
    });
    pipelines.push(await device.createRenderPipelineAsync({ layout,
      vertex: { module, entryPoint: item.vertex },
      fragment: { module, entryPoint: item.fragment, targets: [{ format: request.format }] },
      primitive: { topology: request.topology }
    }));
  }
  const groups = request.groups.map(item => device.createBindGroup({
    layout: pipelines[item.program].getBindGroupLayout(item.index),
    entries: item.entries.map(entry => {
      let resource;
      if (entry.kind === "buffer") resource = { buffer: buffers[entry.id], offset: entry.offset, size: entry.size };
      else if (entry.kind === "texture") resource = images[entry.id].createView(entry.view);
      else resource = samplers[entry.id];
      return { binding: entry.binding, resource };
    })
  }));
  const target = device.createTexture({ size: [request.width, request.height], format: request.format,
    usage: request.target_usage });
  const results = [];
  globalThis.aethyrPrimitiveProgress = 0;
  for (const item of request.cases) {
    for (const write of item.writes) device.queue.writeBuffer(buffers[write.buffer], write.offset, new Float32Array(write.values));
    const read = device.createBuffer({ size: request.readback_bytes, usage: request.readback_usage });
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store", clearValue: request.clear }] });
    pass.setPipeline(pipelines[item.program]);
    for (const binding of item.groups) pass.setBindGroup(binding.index, groups[binding.id]);
    pass.draw(request.vertices); pass.end();
    encoder.copyTextureToBuffer({ texture: target, origin: item.origin },
      { buffer: read, bytesPerRow: request.readback_row_bytes, rowsPerImage: request.readback_height },
      [request.readback_width, request.readback_height, 1]);
    device.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);
    const mapped = read.getMappedRange(), values = [], words = [];
    for (let y = 0; y < request.readback_height; y++) {
      values.push(...new Float32Array(mapped, y * request.readback_row_bytes, request.readback_width * 4));
      words.push(...new Uint32Array(mapped, y * request.readback_row_bytes, request.readback_width * 4));
    }
    results.push({ values, words });
    globalThis.aethyrPrimitiveProgress = results.length;
    read.unmap(); read.destroy();
  }
  const scoped = await device.popErrorScope(); if (scoped) errors.push(scoped.message);
  const info = adapter.info, result = { format: request.format, width: request.width, height: request.height,
    device: { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description },
    messages, errors, results };
  target.destroy(); buffers.forEach(buffer => buffer.destroy()); images.forEach(image => image.destroy()); device.destroy();
  return result;
};
globalThis.aethyrPrimitiveStart = request => {
  globalThis.aethyrPrimitiveJob = { done: false, value: null, error: null };
  globalThis.aethyrPrimitiveGPU(request).then(value => {
    globalThis.aethyrPrimitiveJob.value = value; globalThis.aethyrPrimitiveJob.done = true;
  }, error => {
    globalThis.aethyrPrimitiveJob.error = String(error?.stack ?? error); globalThis.aethyrPrimitiveJob.done = true;
  });
  return "started";
};
globalThis.aethyrPrimitivePoll = () => ({ ...globalThis.aethyrPrimitiveJob,
  completed_readbacks: globalThis.aethyrPrimitiveProgress });
"ready";
