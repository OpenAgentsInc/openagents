// Synthetic scene-linear colors only; no licensed inputs or application state.
// Run: openagents browser run -- node tools/verify-filmic.mjs
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
  response.end("<!doctype html><title>Owned filmic ramp qualification</title>");
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
try {
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const {
      WGSL_FILMIC,
      DEFAULT_FILMIC_CONTROLS,
      prepareFilmic,
      evaluateFilmic,
      evaluateFilmicColor,
    } = await import("/packages/renderer/filmic.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const module = device.createShaderModule({
      code:
        WGSL_FILMIC +
        `
struct Settings { prefix:vec4<f32>, filmic:FilmicParams };
@group(0)@binding(0)var<uniform>settings:Settings;
@group(0)@binding(1)var<storage,read>inputs:array<vec4<f32>>;
@group(0)@binding(2)var<storage,read_write>outputs:array<vec4<f32>>;
@compute @workgroup_size(64)fn main(@builtin(global_invocation_id)i:vec3<u32>){
 if i.x>=arrayLength(&inputs){return;}
 let value=inputs[i.x];
 outputs[i.x]=vec4(filmicColor(value.rgb,settings.filmic),filmicCurve(value.w,settings.filmic));
}
`,
    });
    const messages = (await module.getCompilationInfo()).messages.filter(
      (v) => v.type === "error",
    );
    if (messages.length)
      throw new Error(messages.map((v) => v.message).join("\n"));
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "main" },
    });
    const samples = [
      [0, 0, 0, 0],
      [-1, -10, -0.1, -1],
      [0.18, 0.18, 0.18, 0.18],
    ];
    const rampStart = samples.length;
    for (let i = 0; i <= 1200; i++) {
      const v = 10 ** (-30 + i / 20);
      samples.push([v, v, v, v]);
    }
    for (const v of [0.01, 0.18, 0.5, 1, 10, 1000, 1e20]) {
      samples.push(
        [v, 0, 0, v],
        [0, v, 0, v],
        [0, 0, v, v],
        [v, v * 0.2, v * 0.01, v],
      );
    }
    const values = new Float32Array(samples.flat());
    const byteLength = values.byteLength;
    const input = device.createBuffer({
      size: byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const output = device.createBuffer({
      size: byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
    });
    const readback = device.createBuffer({
      size: byteLength,
      usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
    });
    // The real post uniform embeds this struct after a leading vec4.
    const uniform = device.createBuffer({
      size: 96,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const group = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: uniform } },
        { binding: 1, resource: { buffer: input } },
        { binding: 2, resource: { buffer: output } },
      ],
    });
    device.queue.writeBuffer(input, 0, values);
    const profiles = [
      {},
      { whiteClip: 1 },
      { whiteClip: 1, shadowContrast: [0.85, 0.85, 0.85] },
      { shadowContrast: [0, 4, 0.85] },
    ];
    const keys = ["slope", "toe", "shoulder", "blackClip", "whiteClip"];
    for (let bits = 0; bits < 32; bits++)
      profiles.push(
        Object.fromEntries(keys.map((k, i) => [k, (bits >> i) & 1])),
      );
    let seed = 40173;
    const random = () => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return seed / 2 ** 32;
    };
    for (let i = 0; i < 16; i++)
      profiles.push(Object.fromEntries(keys.map((key) => [key, random()])));
    let maxError = 0,
      comparisons = 0;
    for (const changes of profiles) {
      const prepared = prepareFilmic({
        ...DEFAULT_FILMIC_CONTROLS,
        ...changes,
      });
      const packed = new Float32Array(24);
      packed.set([37, 0, 0, 0]); // Nonzero sentinel catches an incorrect embed offset.
      packed.set(prepared.packed, 4);
      device.queue.writeBuffer(uniform, 0, packed);
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.dispatchWorkgroups(Math.ceil(samples.length / 64));
      pass.end();
      encoder.copyBufferToBuffer(output, 0, readback, 0, byteLength);
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const actual = new Float32Array(readback.getMappedRange()).slice();
      readback.unmap();
      let last = 0;
      for (let i = 0; i < samples.length; i++) {
        const rgb = [...values.slice(i * 4, i * 4 + 3)];
        const expected = [
          ...evaluateFilmicColor(rgb, prepared),
          evaluateFilmic(values[i * 4 + 3], prepared),
        ];
        for (let channel = 0; channel < 4; channel++) {
          const v = actual[i * 4 + channel],
            delta = Math.abs(v - expected[channel]);
          if (!Number.isFinite(v) || v < -1e-6 || v > 1.000001 || delta > 3e-5)
            throw new Error(
              `Profile ${JSON.stringify(changes)}, sample ${i}, channel ${channel}: ${v} != ${expected[channel]}`,
            );
          maxError = Math.max(maxError, delta);
          comparisons++;
        }
        if (i >= rampStart && i < rampStart + 1201) {
          if (actual[i * 4 + 3] < last - 1e-6)
            throw new Error("GPU neutral ramp decreased.");
          last = actual[i * 4 + 3];
        }
      }
    }
    await device.queue.onSubmittedWorkDone();
    input.destroy();
    output.destroy();
    readback.destroy();
    uniform.destroy();
    device.destroy();
    return {
      profiles: profiles.length,
      samples: samples.length,
      comparisons,
      maxError,
      errors,
    };
  });
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
