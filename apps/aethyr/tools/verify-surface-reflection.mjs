// Owned synthetic SH fields only. Run with openagents browser run -- node FILE.
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
  response.end(
    "<!doctype html><title>Owned rough reflection qualification</title>",
  );
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
try {
  const page = await context.newPage(),
    pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { WGSL_SURFACE_SKY_REFLECTION, sh9RoughReflection } =
      await import("/packages/renderer/surface-sky-reflection.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [],
      cases = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const directions = [
      [1, 0, 0],
      [0, 0, -1],
      [0.2, -0.4, 0.6],
      [-1, 0.2, 0.8],
      [0, 1, 0],
    ];
    function add(coefficients, direction, roughness, label) {
      cases.push({
        coefficients: Float32Array.from(coefficients),
        direction: Float32Array.from(direction),
        roughness: Math.fround(roughness),
        label,
      });
    }
    for (const rgb of [
      [0, 0, 0],
      [1, 2, 3],
      [20, 0.1, 4],
    ]) {
      const c = new Float32Array(27);
      c.set(rgb.map((x) => x / 0.2820947918));
      for (const r of [0.3999, 0.4, 0.43, 0.6, 0.8, 1, 1.001])
        for (const d of directions)
          add(c, d, r, "constant radiance and roughness admission");
    }
    for (let k = 1; k < 9; k++)
      for (const r of [0.4, 0.47, 0.8, 1])
        for (const d of directions) {
          const c = new Float32Array(27);
          c.set([10 / 0.2820947918, 8 / 0.2820947918, 6 / 0.2820947918]);
          c.set([0.3, -0.2, 0.1], k * 3);
          add(c, d, r, `individual signed band ${k}`);
        }
    let randomState = 827171;
    const random = () => {
      randomState = (Math.imul(randomState, 1664525) + 1013904223) >>> 0;
      return randomState / 4294967296;
    };
    for (let n = 0; n < 16; n++) {
      const c = Float32Array.from({ length: 27 }, () => (random() - 0.5) * 0.7);
      c.set([3, 2, 1]);
      for (const r of [0.4, 0.4314, 0.5, 0.5093, 1])
        for (const d of directions) add(c, d, r, "asymmetric RGB field");
    }
    const ringing = new Float32Array(27);
    ringing[6] = 1;
    add(
      ringing,
      [0, 0, -1],
      0.5,
      "negative RGB requests existing reflection fallback",
    );
    add(ringing, [0, 0, 1], 0.5, "positive reconstruction remains valid");
    for (const d of [
      [0, 0, 0],
      [NaN, 0, 1],
      [Infinity, 0, 1],
    ])
      add(ringing, d, 0.5, "invalid direction fallback");
    for (const value of [NaN, Infinity]) {
      const c = new Float32Array(27);
      c[0] = value;
      add(c, [0, 0, 1], 0.5, "nonfinite coefficients fallback");
    }
    const values = new Float32Array(cases.length * 40);
    cases.forEach((sample, n) => {
      for (let k = 0; k < 9; k++)
        values.set(
          sample.coefficients.subarray(k * 3, k * 3 + 3),
          n * 40 + k * 4,
        );
      values.set([...sample.direction, sample.roughness], n * 40 + 36);
    });
    const input = device.createBuffer({
      size: values.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(input, 0, values);
    const output = device.createBuffer({
      size: cases.length * 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
    });
    const readback = device.createBuffer({
      size: cases.length * 16,
      usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
    });
    device.pushErrorScope("validation");
    const module = device.createShaderModule({
      code:
        WGSL_SURFACE_SKY_REFLECTION +
        `
struct Sample{coefficients:array<vec4<f32>,9>,directionRoughness:vec4<f32>};
@group(0)@binding(0)var<storage,read>samples:array<Sample>;
@group(0)@binding(1)var<storage,read_write>results:array<vec4<f32>>;
@compute @workgroup_size(64)fn main(@builtin(global_invocation_id)id:vec3<u32>){
 if id.x>=arrayLength(&samples){return;}let sample=samples[id.x];var c:array<vec3<f32>,9>;
 for(var k=0u;k<9u;k++){c[k]=sample.coefficients[k].xyz;}
 results[id.x]=sh9RoughReflection(c,sample.directionRoughness.xyz,sample.directionRoughness.w);
}`,
    });
    const diagnostics = (await module.getCompilationInfo()).messages.filter(
      (message) => message.type === "error",
    );
    if (diagnostics.length)
      throw Error(diagnostics.map((message) => message.message).join("\n"));
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "main" },
    });
    const bind = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: input } },
        { binding: 1, resource: { buffer: output } },
      ],
    });
    const encoder = device.createCommandEncoder(),
      pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bind);
    pass.dispatchWorkgroups(Math.ceil(cases.length / 64));
    pass.end();
    encoder.copyBufferToBuffer(output, 0, readback, 0, cases.length * 16);
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
    const validation = await device.popErrorScope();
    if (validation) throw validation;
    await readback.mapAsync(GPUMapMode.READ);
    const actual = new Float32Array(readback.getMappedRange()),
      max = { componentError: 0 };
    let fallbacks = 0;
    cases.forEach((sample, n) => {
      const expected = sh9RoughReflection(
        sample.coefficients,
        sample.direction,
        sample.roughness,
      );
      if (actual[n * 4 + 3] === 0) fallbacks++;
      for (let c = 0; c < 4; c++) {
        const error = Math.abs(actual[n * 4 + c] - expected[c]);
        max.componentError = Math.max(max.componentError, error);
        if (!Number.isFinite(actual[n * 4 + c]) || error > 0.00002)
          throw Error(
            `${sample.label}: ${Array.from(actual.subarray(n * 4, n * 4 + 4))} != ${expected}`,
          );
      }
    });
    readback.unmap();
    input.destroy();
    output.destroy();
    readback.destroy();
    device.destroy();
    return { cases: cases.length, fallbacks, max, errors };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
