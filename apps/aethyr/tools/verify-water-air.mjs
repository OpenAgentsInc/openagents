// Original mathematical fixtures only. Run through openagents browser run.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((q, r, next) => {
  if (q.url !== "/") return next();
  r.setHeader("content-type", "text/html");
  r.end("<!doctype html><title>Owned water-air qualification</title>");
});
await server.listen();
const browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  ),
  context = await browser.newContext();
try {
  const page = await context.newPage(),
    pageErrors = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { WGSL_WATER_AIR_SCATTERING, waterAirPhase, waterNozzleCoverage } =
      await import("/packages/renderer/water-air-scattering.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const queries = [];
    for (const g of [-0.8, 0, 0.45, 0.8])
      for (const nozzle of [0, 0.2, 1])
        for (const progress of [0, 0.5, 1])
          for (const cosine of [-1, -0.5, 0, 0.5, 1])
            queries.push({ g, nozzle, progress, cosine });
    const values = new Float32Array(
      queries.flatMap((q) => [q.cosine, q.g, q.nozzle, q.progress]),
    );
    const input = device.createBuffer({
        size: values.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      }),
      output = device.createBuffer({
        size: values.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      }),
      readback = device.createBuffer({
        size: values.byteLength,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
    device.queue.writeBuffer(input, 0, values);
    const module = device.createShaderModule({
      code:
        WGSL_WATER_AIR_SCATTERING +
        `@group(0)@binding(0)var<storage,read>queries:array<vec4<f32>>;@group(0)@binding(1)var<storage,read_write>output:array<vec4<f32>>;@compute@workgroup_size(64)fn main(@builtin(global_invocation_id)i:vec3<u32>){if i.x>=arrayLength(&queries){return;}let q=queries[i.x];output[i.x]=vec4(waterAirPhase(q.x,q.y),waterNozzleCoverage(q.w,q.z),waterAirSHFactor(1u,q.y),waterAirSHFactor(4u,q.y));}`,
    });
    const info = await module.getCompilationInfo();
    if (info.messages.some((m) => m.type === "error"))
      throw Error(JSON.stringify(info.messages));
    const pipeline = await device.createComputePipelineAsync({
        layout: "auto",
        compute: { module, entryPoint: "main" },
      }),
      group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: input } },
          { binding: 1, resource: { buffer: output } },
        ],
      });
    const encoder = device.createCommandEncoder(),
      pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, group);
    pass.dispatchWorkgroups(Math.ceil(queries.length / 64));
    pass.end();
    encoder.copyBufferToBuffer(output, 0, readback, 0, values.byteLength);
    device.queue.submit([encoder.finish()]);
    await readback.mapAsync(GPUMapMode.READ);
    const data = new Float32Array(readback.getMappedRange());
    let maxError = 0;
    queries.forEach((q, i) => {
      const expected = [
        waterAirPhase(q.cosine, q.g),
        waterNozzleCoverage(q.progress, q.nozzle),
        1.5 * q.g,
        4 * q.g * q.g,
      ];
      for (let c = 0; c < 4; c++) {
        const error = Math.abs(expected[c] - data[i * 4 + c]);
        maxError = Math.max(maxError, error);
        if (!Number.isFinite(data[i * 4 + c]) || error > 0.00002)
          throw Error(`Water-air GPU mismatch ${i}/${c}: ${error}`);
      }
    });
    readback.unmap();
    await device.queue.onSubmittedWorkDone();
    input.destroy();
    output.destroy();
    readback.destroy();
    device.destroy();
    return { queries: queries.length, maxError, errors };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
