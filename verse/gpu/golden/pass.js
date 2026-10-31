// Firm reason: 1 (it runs in the browser and talks to WebGPU)
// One shader pass drawn by the browser's WebGPU, for the native port's
// per-pass check (docs/verse/native-gpu.md, G5): the WGSL, a vertex and a
// fragment entry, the uniform block's bytes at group 0 binding 0, and the
// pass's colour targets, cleared to zero and read back as float32 RGBA rows.
// verse/gpu/pass-check.coax calls window.runPass over the Chrome DevTools
// protocol and compares what it returns with the same pass natively.
//   runPass({wgsl, vs, fs, uniforms: base64, width, height, targets: ["rgba16float", ...], vertices: 3})
//   -> {ok: true, targets: [base64 of float32 RGBA, ...]} or {ok: false, error}

const b64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
function toB64(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(s);
}
// IEEE half to float.
function half(h) {
  const s = h & 0x8000 ? -1 : 1, e = (h >> 10) & 31, m = h & 1023;
  if (e === 0) return s * m * 2 ** -24;
  if (e === 31) return m ? NaN : s * Infinity;
  return s * (1 + m / 1024) * 2 ** (e - 15);
}

window.runPass = async function (o) {
  try {
    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (e) => errors.push(e.error.message));
    const module = device.createShaderModule({ code: o.wgsl });
    const info = await module.getCompilationInfo();
    const bad = info.messages.filter((m) => m.type === "error");
    if (bad.length) return { ok: false, error: bad.map((m) => `${m.lineNum}: ${m.message}`).join("\n") };
    const ubytes = b64(o.uniforms);
    const ubuf = device.createBuffer({ size: Math.ceil(ubytes.length / 16) * 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    device.queue.writeBuffer(ubuf, 0, ubytes);
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: o.vs },
      fragment: { module, entryPoint: o.fs, targets: o.targets.map((format) => ({ format })) },
      primitive: { topology: "triangle-list" },
    });
    const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: ubuf } }] });
    const textures = o.targets.map((format) => device.createTexture({ size: [o.width, o.height], format, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC }));
    const enc = device.createCommandEncoder();
    const pass = enc.beginRenderPass({ colorAttachments: textures.map((t) => ({ view: t.createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 0] })) });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, group);
    pass.draw(o.vertices ?? 3);
    pass.end();
    const row = Math.ceil(o.width * 8 / 256) * 256;
    const reads = textures.map(() => device.createBuffer({ size: row * o.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ }));
    textures.forEach((t, i) => enc.copyTextureToBuffer({ texture: t }, { buffer: reads[i], bytesPerRow: row }, [o.width, o.height]));
    device.queue.submit([enc.finish()]);
    const out = [];
    for (const r of reads) {
      await r.mapAsync(GPUMapMode.READ);
      const halves = new Uint16Array(r.getMappedRange());
      const f = new Float32Array(o.width * o.height * 4);
      for (let y = 0; y < o.height; y++)
        for (let x = 0; x < o.width * 4; x++) f[y * o.width * 4 + x] = half(halves[y * row / 2 + x]);
      out.push(toB64(new Uint8Array(f.buffer)));
    }
    if (errors.length) return { ok: false, error: errors.join("\n") };
    return { ok: true, targets: out };
  } catch (e) {
    return { ok: false, error: String(e && e.stack || e) };
  }
};
