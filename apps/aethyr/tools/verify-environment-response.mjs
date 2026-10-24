// Synthetic GPU qualification; no licensed source inputs or captures.
import assert from "node:assert/strict";
import { writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
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
    "<!doctype html><title>Environment response qualification</title>",
  );
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
    const { ENVIRONMENT_RESPONSE_WGSL, environmentBrdfWeight, directFresnel } =
      await import("/packages/renderer/environment-response.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU is required.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const cases = [];
    for (const f0 of [
      [0, 0, 0],
      [0.0001, 0.0001, 0.0001],
      [0.00686862, 0.00686862, 0.00686862],
      [0.01, 0.002, 0.001],
      [0.001, 0.01, 0.002],
      [0.01999, 0, 0],
      [0.02, 0.02, 0.02],
      [0.04, 0.04, 0.04],
      [0.9, 0.004, 0.002],
      [0.001, 0.001, 0.7],
      [1, 1, 1],
    ])
      for (const split of [
        [0.743184, 0.206799],
        [0, 1],
        [1, 0],
      ])
        for (const gain of [0.5, 1, 2]) cases.push({ f0, split, gain });
    const input = new Float32Array(cases.length * 12);
    cases.forEach((c, i) => {
      input.set(c.f0, i * 12);
      input.set([...c.split, (i % 11) / 10], i * 12 + 4);
      input.set([c.gain, 0.01, 0.5], i * 12 + 8);
    });
    const resources = [];
    try {
      const buffer = (size, usage) => {
        const b = device.createBuffer({ size, usage });
        resources.push(b);
        return b;
      };
      const source = buffer(
          input.byteLength,
          GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        ),
        output = buffer(
          cases.length * 48,
          GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        ),
        read = buffer(
          cases.length * 48,
          GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        );
      device.queue.writeBuffer(source, 0, input);
      const shader = device.createShaderModule({
        code:
          ENVIRONMENT_RESPONSE_WGSL +
          `
struct Query{f0:vec4<f32>,split:vec4<f32>,gain:vec4<f32>};
struct Result{weight:vec4<f32>,lit:vec4<f32>,direct:vec4<f32>};
@group(0)@binding(0)var<storage,read>queries:array<Query>;
@group(0)@binding(1)var<storage,read_write>results:array<Result>;
@compute @workgroup_size(64)fn main(@builtin(global_invocation_id)i:vec3<u32>){
 if i.x>=arrayLength(&queries){return;}
 let q=queries[i.x];let weight=environmentBrdfWeight(q.f0.rgb,q.split.xy);
 let legacy=q.f0.rgb*q.split.x+vec3(q.split.y);
 let same=all(bitcast<vec3<u32>>(legacy)==bitcast<vec3<u32>>(weight));
 let incident=max(vec3(4.0,2.0,1.0)*q.gain.z+vec3(.5,.25,.125),vec3(0.0));
 let direct=directFresnel(q.f0.rgb,q.split.z);
 let oldDirect=q.f0.rgb+(vec3(1.0)-q.f0.rgb)*pow(1.0-q.split.z,5.0);
 let directDelta=abs(oldDirect-direct);
 results[i.x]=Result(vec4(weight,environmentGrazingReflectance(q.f0.rgb)),vec4(weight*incident*q.gain.x*q.gain.y,select(0.0,1.0,same)),vec4(direct,max(directDelta.r,max(directDelta.g,directDelta.b))));
}`,
      });
      const compilation = await shader.getCompilationInfo();
      const shaderErrors = compilation.messages
        .filter((message) => message.type === "error")
        .map((message) => message.message);
      if (shaderErrors.length) throw Error(shaderErrors.join("\n"));
      const pipeline = await device.createComputePipelineAsync({
        layout: "auto",
        compute: { module: shader, entryPoint: "main" },
      });
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: source } },
          { binding: 1, resource: { buffer: output } },
        ],
      });
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.dispatchWorkgroups(Math.ceil(cases.length / 64));
      pass.end();
      encoder.copyBufferToBuffer(output, 0, read, 0, read.size);
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const values = new Float32Array(read.getMappedRange()).slice();
      read.unmap();
      let maxError = 0,
        legacyIdentical = 0,
        directLegacyMaxError = 0;
      const results = cases.map((c, i) => {
        const f0 = [...input.slice(i * 12, i * 12 + 3)],
          split = [...input.slice(i * 12 + 4, i * 12 + 6)],
          expected = environmentBrdfWeight(f0, split),
          directExpected = directFresnel(f0, input[i * 12 + 6]),
          actual = [...values.slice(i * 12, i * 12 + 12)];
        if (!actual.every(Number.isFinite)) throw Error("Nonfinite response.");
        for (let k = 0; k < 3; k++) {
          maxError = Math.max(maxError, Math.abs(actual[k] - expected[k]));
          maxError = Math.max(
            maxError,
            Math.abs(actual[8 + k] - directExpected[k]),
          );
          const lit = expected[k] * [2.5, 1.25, 0.625][k] * c.gain * 0.01;
          maxError = Math.max(maxError, Math.abs(actual[4 + k] - lit));
          if (
            c.f0.every((v) => v === 0) &&
            (actual[k] !== 0 || actual[8 + k] !== 0)
          )
            throw Error("Zero reflectance still reflects.");
        }
        if (Math.max(...c.f0) >= 0.02) {
          if (actual[7] !== 1)
            throw Error("Legacy environment response changed.");
          // Separate function boundaries can change backend floating-point fusion.
          // Check the unchanged direct equation within a few float32 ULPs.
          directLegacyMaxError = Math.max(directLegacyMaxError, actual[11]);
          if (actual[11] > 2e-7)
            throw Error("Ordinary direct response changed.");
          legacyIdentical++;
        }
        return { ...c, actual };
      });
      if (maxError > 2e-6) throw Error(`Response error ${maxError}.`);
      await device.queue.onSubmittedWorkDone();
      return {
        cases: cases.length,
        legacyIdentical,
        directLegacyMaxError,
        maxError,
        shaderErrors,
        errors,
        results,
      };
    } finally {
      for (const resource of resources) resource.destroy();
      device.destroy();
    }
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  if (process.env.AF_CAPTURE_DIR) {
    await mkdir(process.env.AF_CAPTURE_DIR, { recursive: true });
    await writeFile(
      join(process.env.AF_CAPTURE_DIR, "environment-response.json"),
      JSON.stringify(result, null, 2) + "\n",
    );
  }
  console.log(JSON.stringify({ ...result, results: undefined, pageErrors }));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
