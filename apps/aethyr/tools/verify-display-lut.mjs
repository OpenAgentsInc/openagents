// Owned synthetic HDR response cubes; no vendor shader or image data.
// Run: openagents browser run -- node tools/verify-display-lut.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
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
  res.end(
    "<!doctype html><title>Owned HDR display lookup qualification</title>",
  );
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
const sourceProfiles = [];
if (process.env.AETHYRFORGE_DISPLAY_LUT_QA_DIR) {
  const directory = resolve(process.env.AETHYRFORGE_DISPLAY_LUT_QA_DIR);
  for (const mode of ["day", "night"]) {
    const descriptor = JSON.parse(
      await readFile(resolve(directory, `source-${mode}-lut.json`), "utf8"),
    );
    if (!/^[a-zA-Z0-9._-]+$/.test(descriptor.url))
      throw Error("QA response must be a sibling file");
    const heldout = JSON.parse(
      await readFile(resolve(directory, `source-${mode}-heldout.json`), "utf8"),
    );
    sourceProfiles.push({
      name: `source-${mode}`,
      descriptor,
      heldout,
      base64: (await readFile(resolve(directory, descriptor.url))).toString(
        "base64",
      ),
    });
  }
}
try {
  const page = await context.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async (sourceProfiles) => {
    const {
      DisplayLut,
      WGSL_DISPLAY_LUT,
      decodeDisplayLut,
      sampleDisplayLut,
      displayLutGridInput,
    } = await import("/packages/renderer/display-lut.ts");
    const { fixtureBytes, fixtureDescriptor, fixtureHash } =
      await import("/tests/display-lut-fixtures.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({
      code:
        WGSL_DISPLAY_LUT +
        `
@group(0)@binding(0)var image:texture_3d<f32>;
@group(0)@binding(1)var filtering:sampler;
@group(0)@binding(2)var<uniform>params:vec4<f32>;
@group(0)@binding(3)var<storage,read>inputs:array<vec4<f32>>;
@group(0)@binding(4)var<storage,read_write>outputs:array<vec4<f32>>;
@compute @workgroup_size(64)fn main(@builtin(global_invocation_id)id:vec3<u32>){
 if id.x>=arrayLength(&inputs){return;}
 outputs[id.x]=vec4(sampleDisplayLut(image,filtering,params,inputs[id.x].rgb),1.0);
}`,
    });
    const info = await module.getCompilationInfo();
    const shaderErrors = info.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}:${m.linePos}: ${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "main" },
    });
    const layout = pipeline.getBindGroupLayout(0);
    const finiteColors = [
      [0, 0, 0],
      [64, 64, 64],
      [0.18, 0.18, 0.18],
      [-1, 100, 0.5],
    ];
    for (const r of [0, 64])
      for (const g of [0, 64])
        for (const b of [0, 64]) finiteColors.push([r, g, b]);
    for (const i of [1, 7, 15, 24, 31])
      finiteColors.push([
        displayLutGridInput(i, 33),
        displayLutGridInput(32 - i, 33),
        displayLutGridInput(16, 33),
      ]);
    let seed = 19;
    const random = () => {
      seed = (1664525 * seed + 1013904223) >>> 0;
      return seed / 4294967296;
    };
    for (let i = 0; i < 48; i++)
      finiteColors.push(
        Array.from({ length: 3 }, () => 2 ** (random() * 16 - 10)),
      );
    const cases = [],
      fixtures = [
        { name: "identity-33", edge: 33, transform: (c) => c },
        {
          name: "constant-unclipped",
          edge: 33,
          transform: () => [-0.25, 1.5, 2],
        },
        { name: "log-ramp", edge: 33, transform: (_c, u) => u },
        {
          name: "colored-cross-channel",
          edge: 33,
          transform: (_c, [r, g, b]) => [
            2 * r + 0.25 * g - 0.5 * b - 0.3,
            4 * b - 2 * g,
            (r + g + b) * 0.5,
          ],
        },
        { name: "identity-65", edge: 65, transform: (c) => c },
        {
          name: "parametric-native-63",
          edge: 63,
          transform: (_c, u) => u,
          descriptor: {
            schema: "aethyrforge.display-lut.v2",
            shaper: {
              kind: "log2-1p",
              a: 0.002667719095256729,
              max: 43.70524193759099,
            },
          },
        },
        {
          name: "parametric-unit-domain",
          edge: 33,
          transform: (c) => c,
          descriptor: {
            schema: "aethyrforge.display-lut.v2",
            shaper: { kind: "log2-1p", a: 1, max: 1 },
          },
        },
      ];
    for (const source of sourceProfiles) {
      if (
        source.heldout.actualRaw.length !== 1024 ||
        source.heldout.output.length !== 1024
      )
        throw Error("Source held-out fixture must have 1024 samples");
      fixtures.push({
        name: source.name,
        edge: source.descriptor.edge,
        descriptor: source.descriptor,
        bytes: Uint8Array.from(atob(source.base64), (c) => c.charCodeAt(0)),
        colors: source.heldout.actualRaw,
        target: source.heldout.output,
      });
    }
    const sourceErrors = [];
    const srgb = (v) => {
      const x = Math.min(1, Math.max(0, v));
      return x <= 0.0031308 ? x * 12.92 : 1.055 * x ** (1 / 2.4) - 0.055;
    };
    let maxRelativeError = 0,
      maxAbsoluteError = 0;
    for (const fixture of fixtures) {
      const descriptor = {
        ...fixtureDescriptor(fixture.edge),
        ...fixture.descriptor,
      };
      const bytes =
        fixture.bytes ??
        fixtureBytes(fixture.transform, fixture.edge, descriptor);
      if (!fixture.bytes) descriptor.sha256 = await fixtureHash(bytes);
      const cpu = decodeDisplayLut(descriptor, bytes);
      const owner = await DisplayLut.create(device, descriptor, bytes);
      if (
        owner.bytes !== bytes.byteLength ||
        owner.view !== owner.view ||
        owner.sampler !== owner.sampler
      )
        throw Error("LUT owner lost stable resources");
      for (const enabled of [true, false]) {
        const colors =
          fixture.colors ??
          (enabled
            ? [...finiteColors, [NaN, Infinity, -Infinity]]
            : finiteColors);
        const data = new Float32Array(colors.flatMap((c) => [...c, 1]));
        const input = device.createBuffer({
          size: data.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        const output = device.createBuffer({
          size: data.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });
        const read = device.createBuffer({
          size: data.byteLength,
          usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });
        const params = device.createBuffer({
          size: 16,
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        const config = owner.params.slice();
        config[3] = Number(enabled);
        device.queue.writeBuffer(input, 0, data);
        device.queue.writeBuffer(params, 0, config);
        const group = device.createBindGroup({
          layout,
          entries: [
            { binding: 0, resource: owner.view },
            { binding: 1, resource: owner.sampler },
            { binding: 2, resource: { buffer: params } },
            { binding: 3, resource: { buffer: input } },
            { binding: 4, resource: { buffer: output } },
          ],
        });
        const encoder = device.createCommandEncoder(),
          pass = encoder.beginComputePass();
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, group);
        pass.dispatchWorkgroups(Math.ceil(colors.length / 64));
        pass.end();
        encoder.copyBufferToBuffer(output, 0, read, 0, data.byteLength);
        device.queue.submit([encoder.finish()]);
        await read.mapAsync(GPUMapMode.READ);
        const values = new Float32Array(read.getMappedRange()).slice();
        read.unmap();
        const srgbErrors = [],
          linearErrors = [];
        for (let i = 0; i < colors.length; i++) {
          const color = Array.from(data.slice(i * 4, i * 4 + 3));
          const expected = enabled
            ? sampleDisplayLut(descriptor, cpu, color)
            : color;
          for (let c = 0; c < 3; c++) {
            const actual = values[i * 4 + c],
              absolute = Math.abs(actual - expected[c]),
              relative = absolute / Math.max(1, Math.abs(expected[c]));
            maxRelativeError = Math.max(maxRelativeError, relative);
            maxAbsoluteError = Math.max(maxAbsoluteError, absolute);
            if (!Number.isFinite(actual) || relative > 2 ** -9)
              throw Error(
                JSON.stringify({
                  fixture: fixture.name,
                  enabled,
                  color,
                  channel: c,
                  actual,
                  expected,
                  relative,
                }),
              );
            if (enabled && fixture.target) {
              linearErrors.push(Math.abs(actual - fixture.target[i][c]));
              srgbErrors.push(
                Math.abs(srgb(actual) - srgb(fixture.target[i][c])),
              );
            }
          }
          if (values[i * 4 + 3] !== 1) throw Error("Output alpha changed");
        }
        if (srgbErrors.length) {
          srgbErrors.sort((a, b) => a - b);
          linearErrors.sort((a, b) => a - b);
          const maximum = srgbErrors.at(-1);
          sourceErrors.push({
            name: fixture.name,
            samples: colors.length,
            maxLinear: linearErrors.at(-1),
            maxSrgb: maximum,
            max8bitCodes: maximum * 255,
            p95Srgb: srgbErrors[Math.floor(srgbErrors.length * 0.95)],
            p99Srgb: srgbErrors[Math.floor(srgbErrors.length * 0.99)],
          });
          if (maximum > 2 / 255)
            throw Error(
              `Source GPU response exceeds two 8-bit code values: ${fixture.name}`,
            );
        }
        cases.push({ fixture: fixture.name, enabled, samples: colors.length });
        for (const buffer of [input, output, read, params]) buffer.destroy();
      }
      owner.dispose();
      owner.dispose();
    }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return {
      cases: cases.length,
      samples: cases.reduce((sum, c) => sum + c.samples, 0),
      maxRelativeError,
      maxAbsoluteError,
      sourceErrors,
      shaderErrors,
      errors,
    };
  }, sourceProfiles);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases, 14 + sourceProfiles.length * 2);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
