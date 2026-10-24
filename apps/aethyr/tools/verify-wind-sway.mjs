// Owned synthetic math; no source assets or gallery-phase assertions.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((request, response, next) => {
  if (request.url !== "/") return next();
  response.setHeader("content-type", "text/html");
  response.end("<!doctype html><title>Owned wind sway qualification</title>");
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
    const { WIND_SWAY_WGSL, windSwayOffset, windSwayBound } =
      await import("/packages/renderer/wind-sway.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const count = 512,
      packed = new Float32Array(count * 20),
      expected = [],
      bounds = [];
    for (let i = 0; i < count; i++) {
      const point = [
          Math.cos(i * 0.4) * (i % 3 ? -1 : 2),
          Math.sin(i * 0.23) * 3,
          Math.sin(i * 0.7) * 0.5,
        ],
        pivot = [0, 0, i % 4 === 0 ? 0.0025 : 0],
        up = i % 11 === 0 ? [0, 0, 1] : i % 3 === 0 ? [0, -2, 0] : [0, 1, 0],
        wind = [0, 0, 1],
        green = i % 13 === 0 ? 0 : (i % 17) / 16,
        time = (i % 100) / 4,
        enabled = i % 19 !== 0,
        settings = {
          enabled,
          pivot,
          worldUp: up,
          windDirection: wind,
          radiusMeters: 2,
          windSpeed: 0.5,
          swaySpeed: 0.2,
          swayAmount: i % 2 ? 0.005 : -0.005,
        };
      packed.set(
        [
          ...point,
          green,
          ...pivot,
          settings.radiusMeters,
          ...up,
          settings.windSpeed,
          ...wind,
          settings.swaySpeed,
          time,
          settings.swayAmount,
          Number(enabled),
          0,
        ],
        i * 20,
      );
      // Compare against the same admitted float32 inputs, not higher-precision artist data.
      const c = packed.subarray(i * 20, i * 20 + 20),
        s = {
          enabled: c[18] > 0.5,
          pivot: [...c.slice(4, 7)],
          worldUp: [...c.slice(8, 11)],
          windDirection: [...c.slice(12, 15)],
          radiusMeters: c[7],
          windSpeed: c[11],
          swaySpeed: c[15],
          swayAmount: c[17],
        };
      expected.push(windSwayOffset([...c.slice(0, 3)], c[3], c[16], s));
      bounds.push(
        windSwayBound(
          Math.hypot(...c.slice(0, 3).map((n, axis) => n - c[4 + axis])),
          c[3],
          s,
        ),
      );
    }
    const input = device.createBuffer({
        size: packed.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      }),
      output = device.createBuffer({
        size: count * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      }),
      read = device.createBuffer({
        size: count * 16,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
    device.queue.writeBuffer(input, 0, packed);
    const module = device.createShaderModule({
      code:
        WIND_SWAY_WGSL +
        `struct Case{point:vec4<f32>,pivot:vec4<f32>,up:vec4<f32>,wind:vec4<f32>,control:vec4<f32>};@group(0)@binding(0)var<storage,read>cases:array<Case>;@group(0)@binding(1)var<storage,read_write>results:array<vec4<f32>>;@compute@workgroup_size(64)fn main(@builtin(global_invocation_id)id:vec3<u32>){if id.x>=${count}u{return;}let c=cases[id.x];let value=windSwayOffset(c.point.xyz,c.pivot.xyz,c.up.xyz,c.wind.xyz,vec4(c.up.w,c.wind.w,c.control.y,c.pivot.w),c.control.x,c.point.w,c.control.z>.5);results[id.x]=vec4(value,length(value));}`,
    });
    const shaderErrors = (await module.getCompilationInfo()).messages
      .filter((m) => m.type === "error")
      .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
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
    pass.dispatchWorkgroups(8);
    pass.end();
    encoder.copyBufferToBuffer(output, 0, read, 0, output.size);
    device.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);
    const values = new Float32Array(read.getMappedRange()).slice();
    read.unmap();
    let maxError = 0,
      boundExcess = 0,
      pinnedNonzero = 0,
      finite = true;
    for (let i = 0; i < count; i++) {
      for (let c = 0; c < 3; c++) {
        finite &&= Number.isFinite(values[i * 4 + c]);
        maxError = Math.max(
          maxError,
          Math.abs(values[i * 4 + c] - expected[i][c]),
        );
        if (packed[i * 20 + 3] === 0 || packed[i * 20 + 18] === 0)
          pinnedNonzero += Number(values[i * 4 + c] !== 0);
      }
      boundExcess = Math.max(boundExcess, values[i * 4 + 3] - bounds[i]);
    }
    input.destroy();
    output.destroy();
    read.destroy();
    device.destroy();
    return {
      cases: count,
      shaderErrors,
      errors,
      finite,
      maxError,
      boundExcess,
      pinnedNonzero,
    };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  assert.equal(result.finite, true);
  assert.equal(result.pinnedNonzero, 0);
  assert.ok(result.maxError < 1e-6, JSON.stringify(result));
  assert.ok(result.boundExcess < 1e-7, JSON.stringify(result));
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
