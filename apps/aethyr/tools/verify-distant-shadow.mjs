// Owned cascade coverage and blending fixtures; no source assets or accounts.
// Run: openagents browser run -- node tools/verify-distant-shadow.mjs
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((req, res, next) => {
  if (req.url !== "/") return next();
  res.setHeader("content-type", "text/html");
  res.end("<!doctype html><title>Owned distant shadow qualification</title>");
});
await server.listen();
const browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  ),
  context = await browser.newContext();
try {
  const page = await context.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { MATERIAL } = await import("/packages/renderer/shaders.ts"),
      { LOCAL_FOG_WGSL } = await import("/packages/renderer/local-fog.ts"),
      { FRAME_FLOAT_COUNT, FRAME_OFFSETS } =
        await import("/packages/renderer/frame.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const near = device.createTexture({
        size: [8, 8],
        format: "depth32float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      }),
      outer = device.createTexture({
        size: [8, 8, 2],
        format: "depth32float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
    const nearView = near.createView(),
      farView = outer.createView({
        dimension: "2d",
        baseArrayLayer: 0,
        arrayLayerCount: 1,
      }),
      distantView = outer.createView({
        dimension: "2d",
        baseArrayLayer: 1,
        arrayLayerCount: 1,
      }),
      arrayView = outer.createView({ dimension: "2d-array" }),
      sampler = device.createSampler({
        compare: "less-equal",
        minFilter: "linear",
        magFilter: "linear",
      });
    const buffer = (size, usage) =>
      device.createBuffer({ size, usage: usage | GPUBufferUsage.COPY_DST });
    const frameBuffer = buffer(FRAME_FLOAT_COUNT * 4, GPUBufferUsage.UNIFORM),
      input = buffer(16 * 16, GPUBufferUsage.STORAGE),
      output = buffer(
        16 * 16,
        GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      ),
      read = buffer(16 * 16, GPUBufferUsage.MAP_READ);
    const declarations =
      "@group(GROUP)@binding(30)var<storage,read> queries:array<vec4<f32>>;@group(GROUP)@binding(31)var<storage,read_write> results:array<vec4<f32>>;@compute@workgroup_size(1)fn qualify(@builtin(global_invocation_id)i:vec3<u32>){results[i.x]=vec4(CALL,0.0,0.0,1.0);}";
    const shaderErrors = [];
    const pipelines = [];
    for (const [kind, source, group, call] of [
      [
        "surface",
        MATERIAL,
        0,
        "shadowVisibility(queries[i.x].xyz,vec3(0.0,1.0,0.0))",
      ],
      ["volume", LOCAL_FOG_WGSL, 1, "fogSunVisibility(queries[i.x].xyz)"],
    ]) {
      const module = device.createShaderModule({
        code:
          source +
          "\n" +
          declarations.replaceAll("GROUP", String(group)).replace("CALL", call),
      });
      const info = await module.getCompilationInfo();
      shaderErrors.push(
        ...info.messages
          .filter((m) => m.type === "error")
          .map((m) => kind + ":" + m.lineNum + ":" + m.message),
      );
      if (shaderErrors.length) throw Error(shaderErrors.join("\n"));
      const pipeline = await device.createComputePipelineAsync({
        layout: "auto",
        compute: { module, entryPoint: "qualify" },
      });
      const groups =
        kind === "surface"
          ? [
              device.createBindGroup({
                layout: pipeline.getBindGroupLayout(0),
                entries: [
                  { binding: 0, resource: { buffer: frameBuffer } },
                  { binding: 1, resource: nearView },
                  { binding: 2, resource: sampler },
                  { binding: 3, resource: arrayView },
                  { binding: 30, resource: { buffer: input } },
                  { binding: 31, resource: { buffer: output } },
                ],
              }),
            ]
          : [
              device.createBindGroup({
                layout: pipeline.getBindGroupLayout(0),
                entries: [{ binding: 0, resource: { buffer: frameBuffer } }],
              }),
              device.createBindGroup({
                layout: pipeline.getBindGroupLayout(1),
                entries: [
                  { binding: 5, resource: sampler },
                  { binding: 8, resource: nearView },
                  { binding: 9, resource: farView },
                  { binding: 10, resource: distantView },
                  { binding: 30, resource: { buffer: input } },
                  { binding: 31, resource: { buffer: output } },
                ],
              }),
            ];
      pipelines.push({ kind, pipeline, groups });
    }
    const projection = (extent) =>
      new Float32Array([
        1 / extent,
        0,
        0,
        0,
        0,
        1 / extent,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0.5,
        1,
      ]);
    const points = [0, 18, 40, 81, 200, 901].map((x) => [x, 0, 0, 1]);
    device.queue.writeBuffer(input, 0, new Float32Array(points.flat()));
    const fixtures = [
      {
        name: "near only",
        depths: [0.25, 1, 1],
        expected: [0, 0.5, 1, 1, 1, 1],
      },
      {
        name: "far only",
        depths: [1, 0.25, 1],
        expected: [1, 0.5, 0, 0.5, 1, 1],
      },
      {
        name: "distant only",
        depths: [1, 1, 0.25],
        expected: [1, 1, 1, 0.5, 0, 1],
      },
      {
        name: "all blocked",
        depths: [0.25, 0.25, 0.25],
        expected: [0, 0, 0, 0, 0, 1],
      },
      {
        name: "negative near W",
        depths: [1, 0.25, 1],
        expected: [0, 0, 0, 0.5, 1, 1],
        mutate: (n) => (n[15] = -1),
      },
      {
        name: "near/far outside depth",
        depths: [1, 1, 0.25],
        expected: [0, 0, 0, 0, 0, 1],
        mutate: (n, f) => {
          n[14] = 1.1;
          f[14] = -0.1;
        },
      },
    ];
    let checks = 0,
      maxError = 0;
    for (const fixture of fixtures) {
      const n = projection(20),
        f = projection(90),
        d = projection(900);
      fixture.mutate?.(n, f, d);
      const frame = new Float32Array(FRAME_FLOAT_COUNT);
      frame.set(n, 32);
      frame.set([0, 1, 0, 1], 52);
      frame.set(f, 336);
      frame.set(d, FRAME_OFFSETS.distantShadow);
      device.queue.writeBuffer(frameBuffer, 0, frame);
      const clear = device.createCommandEncoder();
      for (const [i, view] of [nearView, farView, distantView].entries()) {
        const pass = clear.beginRenderPass({
          colorAttachments: [],
          depthStencilAttachment: {
            view,
            depthLoadOp: "clear",
            depthStoreOp: "store",
            depthClearValue: fixture.depths[i],
          },
        });
        pass.end();
      }
      device.queue.submit([clear.finish()]);
      for (const { kind, pipeline, groups } of pipelines) {
        const encoder = device.createCommandEncoder(),
          pass = encoder.beginComputePass();
        pass.setPipeline(pipeline);
        groups.forEach((g, i) => pass.setBindGroup(i, g));
        pass.dispatchWorkgroups(points.length);
        pass.end();
        encoder.copyBufferToBuffer(output, 0, read, 0, points.length * 16);
        device.queue.submit([encoder.finish()]);
        await read.mapAsync(GPUMapMode.READ);
        const values = new Float32Array(read.getMappedRange()).slice();
        read.unmap();
        for (let i = 0; i < points.length; i++) {
          const error = Math.abs(values[i * 4] - fixture.expected[i]);
          maxError = Math.max(error, maxError);
          if (error > 1e-5 || !Number.isFinite(values[i * 4]))
            throw Error(
              JSON.stringify({
                fixture: fixture.name,
                kind,
                point: points[i],
                observed: values[i * 4],
                expected: fixture.expected[i],
                error,
              }),
            );
          checks++;
        }
      }
    }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { checks, maxError, shaderErrors, errors };
  });
  assert.equal(result.checks, 72);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
