// Owned GPU fixtures and original flat-chart measurements; no vendor assets.
// Run: openagents browser run -- node tools/verify-vignette.mjs
import assert from "node:assert/strict";
import { readFile, realpath, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";
import { createServer } from "vite";

async function bounded(path, maximum) {
  if ((await stat(path)).size > maximum)
    throw Error("QA input exceeds its byte limit");
  const bytes = await readFile(path);
  if (bytes.length > maximum) throw Error("QA input exceeds its byte limit");
  return bytes;
}
const reference = JSON.parse(
  await bounded(
    new URL("reference/fixtures/ue58-owned-vignette.json", import.meta.url),
    64 * 1024,
  ),
);
assert.equal(reference.schema, "aethyrforge.owned-vignette-reference.v1");
assert.deepEqual(reference.dimensions, [256, 144]);
assert.equal(reference.samples.length, 35);
let sourceProfile;
if (process.env.AETHYRFORGE_DISPLAY_LUT_QA_DIR) {
  const directory = await realpath(
    resolve(process.env.AETHYRFORGE_DISPLAY_LUT_QA_DIR),
  );
  const descriptor = JSON.parse(
    await bounded(resolve(directory, "source-night-lut.json"), 64 * 1024),
  );
  if (
    !/^[A-Za-z0-9][A-Za-z0-9._-]{0,255}\.(?:rgba16f|bin)$/.test(descriptor.url)
  )
    throw Error("QA display response must name a sibling file");
  const path = await realpath(resolve(directory, descriptor.url));
  if (resolve(directory, descriptor.url) !== path)
    throw Error("QA display response must not escape through a symlink");
  sourceProfile = {
    descriptor,
    base64: (await bounded(path, 65 ** 3 * 8)).toString("base64"),
  };
}

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
    "<!doctype html><title>Owned vignette and post-order qualification</title>",
  );
});
await server.listen();
let browser, context;
try {
  browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  );
  context = await browser.newContext();
  const page = await context.newPage(),
    pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(
    async ({ reference, sourceProfile }) => {
      const { WGSL_VIGNETTE, vignetteWeight } =
        await import("/packages/renderer/vignette.ts");
      const { DisplayLut, decodeDisplayLut, sampleDisplayLut } =
        await import("/packages/renderer/display-lut.ts");
      const { fixtureBytes, fixtureDescriptor, fixtureHash, fixtureHalf } =
        await import("/tests/display-lut-fixtures.ts");
      const { prepareFilmic } = await import("/packages/renderer/filmic.ts");
      const { POST } = await import("/packages/renderer/shaders.ts");
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) throw Error("WebGPU is required");
      const device = await adapter.requestDevice(),
        errors = [];
      device.addEventListener("uncapturederror", (event) =>
        errors.push(event.error.message),
      );
      const compilation = async (code) => {
        const module = device.createShaderModule({ code });
        const failures = (await module.getCompilationInfo()).messages.filter(
          (v) => v.type === "error",
        );
        if (failures.length)
          throw Error(
            failures
              .map((v) => `${v.lineNum}:${v.linePos}: ${v.message}`)
              .join("\n"),
          );
        return module;
      };
      const mathModule = await compilation(
        WGSL_VIGNETTE +
          `
@group(0)@binding(0)var<storage,read>inputs:array<vec4<f32>>;
@group(0)@binding(1)var<storage,read_write>outputs:array<vec4<f32>>;
@compute @workgroup_size(64)fn main(@builtin(global_invocation_id)i:vec3<u32>){
 if i.x>=arrayLength(&inputs){return;}
 let p=inputs[i.x];outputs[i.x]=vec4(vignetteWeight(p.xy,p.z,p.w),1.0,0.0,0.0);
}`,
      );
      const math = await device.createComputePipelineAsync({
        layout: "auto",
        compute: { module: mathModule, entryPoint: "main" },
      });
      const cases = [],
        pairs = [];
      for (const aspect of [1, 0.25, 4, 16 / 9, 9 / 16])
        for (const intensity of [0, 0.25, 0.5, 0.55, 1])
          for (const uv of [
            [0.5, 0.5],
            [0, 0],
            [1, 0],
            [0, 1],
            [1, 1],
            [0, 0.5],
            [1, 0.5],
            [0.5, 0],
            [0.5, 1],
            [0.131, 0.873],
          ]) {
            pairs.push([cases.length, cases.length + 1]);
            cases.push(
              [...uv, aspect, intensity],
              [uv[1], uv[0], 1 / aspect, intensity],
            );
          }
      const values = new Float32Array(cases.flat()),
        length = values.byteLength;
      const input = device.createBuffer({
        size: length,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
      const output = device.createBuffer({
        size: length,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      });
      const read = device.createBuffer({
        size: length,
        usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
      });
      device.queue.writeBuffer(input, 0, values);
      const group = device.createBindGroup({
        layout: math.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: input } },
          { binding: 1, resource: { buffer: output } },
        ],
      });
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginComputePass();
      pass.setPipeline(math);
      pass.setBindGroup(0, group);
      pass.dispatchWorkgroups(Math.ceil(cases.length / 64));
      pass.end();
      encoder.copyBufferToBuffer(output, 0, read, 0, length);
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const actual = new Float32Array(read.getMappedRange()).slice();
      read.unmap();
      let maxMathError = 0,
        maxReciprocalError = 0;
      for (let i = 0; i < cases.length; i++) {
        const p = Array.from(values.subarray(i * 4, i * 4 + 4));
        const expected = vignetteWeight([p[0], p[1]], p[2], p[3]),
          got = actual[i * 4];
        const analytic =
          p[0] === 0.5 && p[1] === 0.5
            ? 1
            : (p[0] === 0 || p[0] === 1) && (p[1] === 0 || p[1] === 1)
              ? 1 / (1 + 2 * p[3] ** 2) ** 2
              : expected;
        const error = Math.max(
          Math.abs(got - expected),
          Math.abs(got - analytic),
        );
        if (!Number.isFinite(got) || got <= 0 || got > 1 || error > 2e-6)
          throw Error(
            `Vignette math mismatch: ${JSON.stringify({ p, got, expected, analytic })}`,
          );
        maxMathError = Math.max(maxMathError, error);
      }
      for (const [a, b] of pairs) {
        const delta = Math.abs(actual[a * 4] - actual[b * 4]);
        if (delta > 2e-6)
          throw Error(
            "Reciprocal aspect and transposed UV changed lens falloff",
          );
        maxReciprocalError = Math.max(maxReciprocalError, delta);
      }
      input.destroy();
      output.destroy();
      read.destroy();

      // Use the actual POST source and its real128-byte buffer, not a copied order.
      const attenuation =
        "c*=vignetteWeight(v.uv,display.vignette.y,display.vignette.x);";
      const outputMarker = "return vec4(c,1.0);";
      if (
        POST.split(attenuation).length !== 2 ||
        POST.split(outputMarker).length !== 2 ||
        !POST.includes("vignette:vec4<f32>")
      )
        throw Error(
          "Actual POST vignette layout/order changed; review this qualifier",
        );
      const wrongOrder = POST.replace(attenuation, "").replace(
        outputMarker,
        `${attenuation}\n ${outputMarker}`,
      );
      if (wrongOrder === POST || wrongOrder.split(attenuation).length !== 2)
        throw Error("The negative ordering control was not constructed");
      const modules = {
        actual: await compilation(POST),
        wrong: await compilation(wrongOrder),
      };
      const [width, height] = reference.dimensions,
        bytesPerRow = width * 16;
      const target = device.createTexture({
        size: [width, height],
        format: "rgba32float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const source = device.createTexture({
        size: [1, 1],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      const bloom = device.createTexture({
        size: [1, 1],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      const black = new Uint16Array([0, 0, 0, fixtureHalf(1)]);
      device.queue.writeTexture(
        { texture: bloom },
        black,
        { bytesPerRow: 8 },
        [1, 1],
      );
      const uniform = device.createBuffer({
        size: 128,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      const pixels = device.createBuffer({
        size: bytesPerRow * height,
        usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
      });
      const sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
      });
      const film = prepareFilmic({
        ...reference.filmControls,
        shadowContrast: reference.filmControls.shadowContrast.slice(0, 3),
      });
      const profiles = [];
      const syntheticBytes = fixtureBytes((color) => color.map(Math.sqrt));
      profiles.push({
        name: "owned-nonlinear-response",
        descriptor: {
          ...fixtureDescriptor(),
          sha256: await fixtureHash(syntheticBytes),
        },
        bytes: syntheticBytes,
      });
      if (sourceProfile)
        profiles.push({
          name: "measured-source-night",
          descriptor: sourceProfile.descriptor,
          bytes: Uint8Array.from(atob(sourceProfile.base64), (c) =>
            c.charCodeAt(0),
          ),
        });
      const postCases = [];
      for (const profile of profiles) {
        const owner = await DisplayLut.create(
            device,
            profile.descriptor,
            profile.bytes,
          ),
          decoded = decodeDisplayLut(profile.descriptor, profile.bytes);
        try {
          const pipelines = {},
            groups = {};
          for (const name of ["actual", "wrong"]) {
            pipelines[name] = await device.createRenderPipelineAsync({
              layout: "auto",
              vertex: { module: modules[name], entryPoint: "vs" },
              fragment: {
                module: modules[name],
                entryPoint: "fs",
                targets: [{ format: "rgba32float" }],
              },
            });
            groups[name] = device.createBindGroup({
              layout: pipelines[name].getBindGroupLayout(0),
              entries: [
                { binding: 0, resource: source.createView() },
                { binding: 1, resource: sampler },
                { binding: 2, resource: { buffer: uniform } },
                { binding: 3, resource: bloom.createView() },
                { binding: 4, resource: owner.view },
                { binding: 5, resource: owner.sampler },
              ],
            });
          }
          for (const storageScale of [1, 0.01]) {
            device.queue.writeTexture(
              { texture: source },
              new Uint16Array([
                ...reference.sceneLinearRGB.map((v) =>
                  fixtureHalf(v * storageScale),
                ),
                fixtureHalf(1),
              ]),
              { bytesPerRow: 8 },
              [1, 1],
            );
            let correctMaxLinear = 0,
              wrongMaxLinear = 0,
              outputSamples = 0;
            for (const intensity of [
              ...new Set(reference.samples.map((s) => s.intensity)),
            ]) {
              const packed = new Float32Array(32);
              packed.set([1 / storageScale, 1, 0, 1], 0);
              packed.set(film.packed, 4);
              packed.set(owner.params, 24);
              packed.set([intensity, width / height, 0, 0], 28);
              device.queue.writeBuffer(uniform, 0, packed);
              for (const name of ["actual", "wrong"]) {
                const encoder = device.createCommandEncoder(),
                  pass = encoder.beginRenderPass({
                    colorAttachments: [
                      {
                        view: target.createView(),
                        loadOp: "clear",
                        storeOp: "store",
                        clearValue: [0, 0, 0, 0],
                      },
                    ],
                  });
                pass.setPipeline(pipelines[name]);
                pass.setBindGroup(0, groups[name]);
                pass.draw(3);
                pass.end();
                encoder.copyTextureToBuffer(
                  { texture: target },
                  { buffer: pixels, bytesPerRow },
                  [width, height],
                );
                device.queue.submit([encoder.finish()]);
                await pixels.mapAsync(GPUMapMode.READ);
                const result = new Float32Array(
                  pixels.getMappedRange(),
                ).slice();
                pixels.unmap();
                for (const sample of reference.samples.filter(
                  (s) => s.intensity === intensity,
                )) {
                  const [x, y] = sample.pixel,
                    index = (x + y * width) * 4;
                  const weight = vignetteWeight(
                    [(x + 0.5) / width, (y + 0.5) / height],
                    width / height,
                    intensity,
                  );
                  const expected =
                    profile.name === "measured-source-night"
                      ? sample.toneLinearRGB
                      : sampleDisplayLut(
                          profile.descriptor,
                          decoded,
                          reference.sceneLinearRGB.map((v) => v * weight),
                        );
                  for (let channel = 0; channel < 3; channel++) {
                    const got = result[index + channel],
                      delta = Math.abs(got - expected[channel]);
                    if (!Number.isFinite(got))
                      throw Error("POST produced a nonfinite vignette pixel");
                    if (name === "actual")
                      correctMaxLinear = Math.max(correctMaxLinear, delta);
                    else wrongMaxLinear = Math.max(wrongMaxLinear, delta);
                  }
                  if (result[index + 3] !== 1)
                    throw Error("POST changed output alpha");
                  outputSamples++;
                }
              }
            }
            if (correctMaxLinear >= 0.001)
              throw Error(
                `Actual POST exceeds .001 linear error: ${JSON.stringify({ profile: profile.name, storageScale, correctMaxLinear })}`,
              );
            if (wrongMaxLinear <= 0.01)
              throw Error(
                `Wrong post-tone order was not rejected: ${wrongMaxLinear}`,
              );
            postCases.push({
              profile: profile.name,
              storageScale,
              samples: outputSamples,
              maxLinear: correctMaxLinear,
              wrongOrderMaxLinear: wrongMaxLinear,
            });
          }
        } finally {
          owner.dispose();
        }
      }
      await device.queue.onSubmittedWorkDone();
      for (const resource of [target, source, bloom, uniform, pixels])
        resource.destroy();
      device.destroy();
      return {
        mathCases: cases.length,
        reciprocalPairs: pairs.length,
        maxMathError,
        maxReciprocalError,
        postCases,
        measuredReference: !!sourceProfile,
        errors,
      };
    },
    { reference, sourceProfile },
  );
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  assert.equal(result.postCases.length, sourceProfile ? 4 : 2);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context?.close();
  await browser?.close();
  await server.close();
}
