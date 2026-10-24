// Original fixtures qualify ordered blending and 4x depth without licensed assets.
// Run: openagents browser run -- node tools/verify-surface-decals.mjs
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const root = new URL("..", import.meta.url).pathname;
const server = await createServer({
  configFile: false,
  root,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((request, response, next) => {
  if (request.url === "/") {
    response.setHeader("content-type", "text/html");
    response.end(
      "<!doctype html><title>Owned surface decal qualification</title>",
    );
  } else if (request.url?.startsWith("/private-assets/owned-decal-")) {
    const rgba = request.url.includes("red")
      ? [255, 0, 0, 128]
      : [0, 0, 255, 128];
    const bytes = Buffer.alloc(20);
    bytes.write("AFR8");
    bytes.writeUInt32LE(1, 4);
    bytes.writeUInt32LE(1, 8);
    bytes.writeUInt32LE(1, 12);
    bytes.set(rgba, 16);
    response.setHeader("content-type", "application/octet-stream");
    response.end(bytes);
  } else next();
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
try {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { SurfaceDecalPass, SURFACE_DECAL_WGSL, SURFACE_DECAL_COMPOSE_WGSL } =
      await import("/packages/renderer/surface-decals.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required for this check.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const compilation = await device
      .createShaderModule({ code: SURFACE_DECAL_WGSL })
      .getCompilationInfo();
    const shaderErrors = compilation.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}:${m.linePos} ${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const material = (id, roughness, specular) => ({
      id,
      colorOpacityUrl: `/private-assets/owned-decal-${id}.bin`,
      uvTransform: [1, 1, 0, 0],
      normalFlatness: 0,
      normalConstant: [0, 0, 1],
      roughnessIntensity: roughness,
      sourceSpecularAmount: specular,
      metallic: 0,
    });
    const instance = (id, material, sourceOrder) => ({
      id,
      material,
      sourceOrder,
      sortOrder: 0,
      worldFromDecal: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0.1, 0, 0, 0, 0.5, 1],
      worldToDecal: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 10, 0, 0, 0, -5, 1],
      bounds: { min: [-1, -1, 0.4], max: [1, 1, 0.6] },
      fadeScreenSize: 0.01,
    });
    const decals = await SurfaceDecalPass.create(device, {
      schema: "aethyrforge.surface-decals.v1",
      materials: [material("red", 0.6, 0.25), material("blue", 0.2, 0.75)],
      instances: [instance("blue", 1, 1), instance("red", 0, 0)],
    });
    const depth = device.createTexture({
      size: [8, 8],
      sampleCount: 4,
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const values = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const writer = device.createShaderModule({
      code: `@group(0)@binding(0)var<uniform>depth:vec4<f32>;@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.0,1.0);}@fragment fn fs(@builtin(sample_index)s:u32)->@builtin(frag_depth)f32{return depth[s];}`,
    });
    const depthPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: writer, entryPoint: "vs" },
      fragment: { module: writer, entryPoint: "fs", targets: [] },
      multisample: { count: 4 },
      depthStencil: {
        format: "depth32float",
        depthCompare: "always",
        depthWriteEnabled: true,
      },
    });
    const depthGroup = device.createBindGroup({
      layout: depthPipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: values } }],
    });
    const debugModule = device.createShaderModule({
      code: `${SURFACE_DECAL_COMPOSE_WGSL}\n@group(0)@binding(0)var image:texture_2d_array<f32>;@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.0,1.0);}@fragment fn fs(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{let i=i32(p.x);if i<3{return textureLoad(image,vec2(4,4),i,0);}let result=composeSurfaceDecals(image,vec2(4,4),vec3(.2,.4,.6),vec3(0.,1.,0.),vec3(0.,.04,.8),7u);if i==3{return vec4(result.base,result.colorTransmittance);}if i==4{return vec4(result.normal,1.0);}return vec4(result.properties,1.0);}`,
    });
    const debugPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: debugModule, entryPoint: "vs" },
      fragment: {
        module: debugModule,
        entryPoint: "fs",
        targets: [{ format: "rgba32float" }],
      },
    });
    const output = device.createTexture({
      size: [6, 1],
      format: "rgba32float",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const readback = device.createBuffer({
      size: 256,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const cases = [];
    for (const [name, sampleDepths] of [
      ["ordered-source-over", [0.5, 0.5, 0.5, 0.5]],
      ["foreground-occludes", [0.2, 0.2, 0.2, 0.2]],
      ["msaa-one-valid-sample", [1, 1, 0.5, 1]],
      ["msaa-nearest-foreground-wins", [0.2, 0.5, 0.5, 0.5]],
      ["clear-depth", [1, 1, 1, 1]],
    ]) {
      device.queue.writeBuffer(values, 0, new Float32Array(sampleDepths));
      const encoder = device.createCommandEncoder();
      const prepass = encoder.beginRenderPass({
        colorAttachments: [],
        depthStencilAttachment: {
          view: depth.createView(),
          depthLoadOp: "clear",
          depthStoreOp: "store",
          depthClearValue: 1,
        },
      });
      prepass.setPipeline(depthPipeline);
      prepass.setBindGroup(0, depthGroup);
      prepass.draw(3);
      prepass.end();
      const stats = decals.render(encoder, {
        width: 8,
        height: 8,
        depthView: depth.createView(),
        viewProjection: identity,
        inverseViewProjection: identity,
      });
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          { view: output.createView(), loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(debugPipeline);
      pass.setBindGroup(
        0,
        device.createBindGroup({
          layout: debugPipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: decals.view }],
        }),
      );
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: output },
        { buffer: readback, bytesPerRow: 256 },
        [6, 1],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const pixels = [
        ...new Float32Array(readback.getMappedRange()).slice(0, 24),
      ];
      readback.unmap();
      cases.push({ name, sampleDepths, pixels, stats });
    }
    await device.queue.onSubmittedWorkDone();
    decals.destroy();
    depth.destroy();
    values.destroy();
    output.destroy();
    readback.destroy();
    device.destroy();
    return { cases, shaderErrors, errors };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(errors, []);
  const alpha = 128 / 255,
    transmittance = (1 - alpha) ** 2;
  const expected = [
    alpha * (1 - alpha),
    0,
    alpha,
    transmittance,
    -(1 - transmittance),
    0,
    0,
    transmittance,
    0,
    0.02 * alpha * (1 - alpha) + 0.06 * alpha,
    0.6 * alpha * (1 - alpha) + 0.2 * alpha,
    transmittance,
  ];
  for (const c of result.cases) {
    const admitted =
      c.name === "ordered-source-over" || c.name === "msaa-one-valid-sample";
    const want = admitted ? expected : [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1];
    for (let i = 0; i < 12; i++)
      assert.ok(
        Math.abs(c.pixels[i] - want[i]) < 0.003,
        `${c.name} channel${i}: ${c.pixels[i]} != ${want[i]}`,
      );
    if (admitted) {
      const base = expected
        .slice(0, 3)
        .map((v, i) => v + [0.2, 0.4, 0.6][i] * transmittance);
      for (let i = 0; i < 3; i++)
        assert.ok(Math.abs(c.pixels[12 + i] - base[i]) < 0.003);
      assert.ok(
        Math.abs(c.pixels[21] - (expected[9] + 0.04 * transmittance)) < 0.003,
      );
      assert.ok(
        Math.abs(c.pixels[22] - (expected[10] + 0.8 * transmittance)) < 0.003,
      );
    }
    assert.equal(c.stats.draws, 1);
    assert.equal(c.stats.visible, 2);
  }
  console.log(
    JSON.stringify(
      {
        ok: true,
        cases: result.cases.map(({ name, sampleDepths, pixels, stats }) => ({
          name,
          sampleDepths,
          accumulators: pixels.slice(0, 12),
          draws: stats.draws,
        })),
        errors: [],
      },
      null,
      2,
    ),
  );
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
