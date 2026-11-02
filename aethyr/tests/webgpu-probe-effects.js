// Firm reason: 1 (executed inside headless Chrome; typed WebGPU creation, uploads, command submission and readbacks only).
// Coax supplies every fixture, entry pair and descriptor and decides the verdict.
globalThis.aethyrWebGPUProbe = async request => {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw Error("no WebGPU adapter");
  const device = await adapter.requestDevice(), errors = [];
  device.addEventListener("uncapturederror", event => errors.push(event.error.message));
  device.pushErrorScope("validation");
  const module = device.createShaderModule({ code: request.code });
  const messages = (await module.getCompilationInfo()).messages.map(message => ({
    type: message.type, message: message.message, line: message.lineNum, column: message.linePos
  }));
  const layout = device.createBindGroupLayout({ entries: request.layout });
  const uniform = device.createBuffer({ size: request.uniform_bytes, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const entries = [{ binding: 0, resource: { buffer: uniform, offset: 0, size: request.uniform_bytes } }];
  let image;
  if (request.image) {
    image = device.createTexture({ size: request.image.size, format: request.image.format,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
    device.queue.writeTexture({ texture: image }, new Uint8Array(request.image.bytes),
      { bytesPerRow: request.image.bytes_per_row }, request.image.size);
    entries.push({ binding: 1, resource: image.createView() });
    entries.push({ binding: 2, resource: device.createSampler(request.image.sampler) });
  }
  const group = device.createBindGroup({ layout, entries });
  const pipeline = await device.createRenderPipelineAsync({
    layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
    vertex: { module, entryPoint: request.vertex },
    fragment: { module, entryPoint: request.fragment, targets: [{ format: request.format }] },
    primitive: { topology: request.topology }
  });
  const target = device.createTexture({ size: [request.width, request.height], format: request.format,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
  const results = [], raw_words = [];
  globalThis.aethyrProbeProgress = 0;
  for (const row of request.cases) {
    device.queue.writeBuffer(uniform, 0, new Float32Array(row.uniform));
    const read = device.createBuffer({ size: request.readback_bytes, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store", clearValue: request.clear }] });
    pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.draw(request.vertices); pass.end();
    encoder.copyTextureToBuffer({ texture: target, origin: [row.pixel[0], row.pixel[1], 0] },
      { buffer: read, bytesPerRow: request.readback_bytes }, [1, 1, 1]);
    device.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);
    const mapped = read.getMappedRange();
    results.push({ ...row.observation, rgba: Array.from(new Float32Array(mapped, 0, 4)) });
    raw_words.push(Array.from(new Uint32Array(mapped, 0, 4)));
    globalThis.aethyrProbeProgress = results.length;
    read.unmap(); read.destroy();
  }
  const error = await device.popErrorScope(); if (error) errors.push(error.message);
  const info = adapter.info;
  const result = { format: request.format, width: request.width, height: request.height,
    device: { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description },
    messages, errors, results, raw_words };
  target.destroy(); uniform.destroy(); image?.destroy(); device.destroy();
  return result;
};
globalThis.aethyrWebGPUProbeStart = request => {
  globalThis.aethyrProbeJob = { done: false, value: null, error: null };
  globalThis.aethyrProbeProgress = 0;
  globalThis.aethyrWebGPUProbe(request).then(value => {
    globalThis.aethyrProbeJob.value = value;
    globalThis.aethyrProbeJob.done = true;
  }, error => {
    globalThis.aethyrProbeJob.error = String(error?.stack ?? error);
    globalThis.aethyrProbeJob.done = true;
  });
  return "started";
};
globalThis.aethyrWebGPUProbePoll = () => ({ ...globalThis.aethyrProbeJob,
  completed_readbacks: globalThis.aethyrProbeProgress });
"ready";
