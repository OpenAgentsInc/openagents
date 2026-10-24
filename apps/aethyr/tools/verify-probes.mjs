// Synthetic GPU check: no licensed inputs or screenshots.
// Run: openagents browser run -- node tools/verify-probes.mjs
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
  response.end("<!doctype html><title>Owned sky probe qualification</title>");
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
    const { COMMON } = await import("/packages/renderer/shaders.ts");
    const { WGSL_PROBES } = await import("/packages/renderer/probe-shading.ts");
    const { WGSL_IBL } = await import("/packages/renderer/ibl.ts");
    const { encodeRgba16fLE } =
      await import("/packages/renderer/environment-lighting.ts");
    const { FRAME_FLOAT_COUNT, FRAME_OFFSETS } =
      await import("/packages/renderer/frame.ts");
    const floatToHalf = (value) =>
      ({ 1: 0x3c00, 64: 0x5400, 4096: 0x6c00 })[value];
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const module = device.createShaderModule({
      code:
        COMMON +
        WGSL_PROBES +
        WGSL_IBL +
        `
@group(1)@binding(0)var<uniform>point:vec4<f32>;
@group(1)@binding(1)var texSampler:sampler;
@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.0,1.0);}
@fragment fn fs()->@location(0)vec4<f32>{
 if point.w>6.5{return vec4(environmentIrradiance(vec3(0.,1.,0.)),1.);}
 if point.w>5.5{return vec4(environmentSurface(point.xyz,vec3(0.,1.,0.),vec3(0.,1.,0.),vec3(0.,1.,0.),vec3(.5,.25,.75),.5,1.,.6,.04),1.);}
 if point.w>4.5{return vec4(environmentSurface(point.xyz,vec3(0.,1.,0.),vec3(0.,1.,0.),vec3(0.,1.,0.),vec3(.5,.25,.75),.5,0.,.6,.04),1.);}
 let value=localSkyLighting(point.xyz,vec3(0.,1.,0.),vec3(0.,1.,0.),vec3(0.,0.,1.));
 if point.w>3.5{return vec4(value.localBounce,value.confidence);}
 if point.w>2.5{return vec4(value.sunBounce,value.confidence);}
 if point.w>1.5{return vec4(value.skyBounce,value.confidence);}
 if point.w>0.5{return vec4(vec3(value.specularVisibility),value.confidence);}
 return vec4(value.irradiance,value.confidence);
}
`,
    });
    const compilation = await module.getCompilationInfo();
    const shaderErrors = compilation.messages
      .filter((m) => m.type === "error")
      .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [{ format: "rgba32float" }],
      },
    });
    const uniform = device.createBuffer({
      size: FRAME_FLOAT_COUNT * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const point = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const records = device.createBuffer({
      size: (10 + 9) * 8 * 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const bounce = device.createBuffer({
      size: 4 * 8 * 9 * 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const blackTexture = (layers = 1) => {
      const texture = device.createTexture({
        size: [1, 1, layers],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      device.queue.writeTexture(
        { texture },
        new Uint16Array(4 * layers),
        { bytesPerRow: 8, rowsPerImage: 1 },
        [1, 1, layers],
      );
      return texture;
    };
    const reflection = blackTexture(2),
      brdf = blackTexture(),
      sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
      });
    const moments = device.createTexture({
      size: [30, 30],
      format: "rgba16float",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    const output = device.createTexture({
      size: [1, 1],
      format: "rgba32float",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const readback = device.createBuffer({
      size: 256,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const group = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: uniform } },
        { binding: 6, resource: sampler },
        {
          binding: 8,
          resource: reflection.createView({ dimension: "2d-array" }),
        },
        { binding: 9, resource: brdf.createView() },
        { binding: 11, resource: moments.createView() },
        { binding: 12, resource: { buffer: records } },
        { binding: 14, resource: { buffer: bounce } },
      ],
    });
    const pointGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(1),
      entries: [
        { binding: 0, resource: { buffer: point } },
        { binding: 1, resource: sampler },
      ],
    });
    async function readPixel() {
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          { view: output.createView(), loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.setBindGroup(1, pointGroup);
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: output },
        { buffer: readback, bytesPerRow: 256 },
        [1, 1],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const pixel = [
        ...new Float32Array(readback.getMappedRange()).slice(0, 4),
      ];
      readback.unmap();
      return pixel;
    }
    const values = [];
    for (const name of [
      "constant-sky",
      "blocked-sky",
      "disabled",
      "outside",
      "invalid-probes",
      "open-visibility",
      "fixed-hit-visibility",
      "binary-visibility",
    ]) {
      const u = new Float32Array(FRAME_FLOAT_COUNT);
      u.set(
        [
          0,
          0,
          0,
          name === "disabled" ? 0 : 1,
          4,
          4,
          4,
          64,
          2,
          2,
          2,
          3,
          30,
          30,
          10,
          8,
        ],
        FRAME_OFFSETS.probes,
      );
      u.set([1, 0, 0, 0], FRAME_OFFSETS.skyLayers);
      device.queue.writeBuffer(uniform, 0, u);
      device.queue.writeBuffer(
        point,
        0,
        new Float32Array(
          name === "outside"
            ? [100, 2, 2, 0]
            : [2, 1.9, 1.95, Number(name.endsWith("visibility"))],
        ),
      );
      const data = new Float32Array(8 * 40);
      for (let i = 0; i < 8; i++) {
        data.set(
          [
            (i & 1) * 4,
            ((i >> 1) & 1) * 4,
            ((i >> 2) & 1) * 4,
            name === "invalid-probes" ? 0 : 1,
          ],
          i * 40,
        );
        data.set(
          [
            Math.sqrt(4 * Math.PI),
            Math.sqrt(4 * Math.PI) * 2,
            Math.sqrt(4 * Math.PI) * 3,
            0,
          ],
          i * 40 + 4,
        );
      }
      device.queue.writeBuffer(records, 0, data);
      const distances = new Uint16Array(30 * 30 * 4);
      for (let i = 0; i < 30 * 30; i++)
        distances.set(
          name === "binary-visibility"
            ? [0x5040, 0x6804, 0, 0x3c00]
            : name === "fixed-hit-visibility"
              ? [0x4400, 0x4c00, 0, 0x3c00]
              : name === "blocked-sky"
                ? [0, 0, 0, floatToHalf(1)]
                : [floatToHalf(64), floatToHalf(4096), 0, floatToHalf(1)],
          i * 4,
        );
      device.queue.writeTexture(
        { texture: moments },
        distances,
        { bytesPerRow: 30 * 8 },
        [30, 30],
      );
      values.push({
        name,
        pixel: await readPixel(),
      });
    }
    const bounceValues = [];
    const layerColors = [
      [2, 4, 8],
      [3, 5, 7],
      [11, 13, 17],
    ];
    const cases = [
      { name: "sky-layer", selector: 2 },
      { name: "sun-layer", selector: 3 },
      { name: "local-layer", selector: 4 },
      { name: "sky-disabled", selector: 2, flags: [0, 1, 1] },
      { name: "sun-disabled", selector: 3, flags: [1, 0, 1] },
      { name: "locals-disabled", selector: 4, flags: [1, 1, 0] },
      { name: "sky-survives-other-flags", selector: 2, flags: [1, 0, 0] },
      { name: "sun-survives-other-flags", selector: 3, flags: [0, 1, 0] },
      { name: "locals-survive-other-flags", selector: 4, flags: [0, 0, 1] },
      { name: "raw-sky-independent", selector: 0, flags: [0, 0, 0] },
      { name: "single-valid-normalized", selector: 4, singleValid: true },
      { name: "different-probes-normalized", selector: 3, vary: true },
      { name: "invalid-bounce-probes", selector: 4, invalid: true },
      { name: "outside-bounce-field", selector: 2, outside: true },
      { name: "disabled-base-field", selector: 4, disabled: true },
      { name: "surface-all", selector: 5 },
      { name: "surface-no-sky", selector: 5, skyGain: 0 },
      { name: "surface-no-sun", selector: 5, sunGain: 0 },
      { name: "surface-no-locals", selector: 5, localGain: 0 },
      { name: "surface-double-sky", selector: 5, skyGain: 3 },
      { name: "surface-double-sun", selector: 5, sunGain: 6 },
      { name: "surface-double-locals", selector: 5, localGain: 0.02 },
      { name: "surface-disabled-sun-layer", selector: 5, flags: [1, 0, 1] },
      { name: "surface-disabled-local-layer", selector: 5, flags: [1, 1, 0] },
    ];
    for (const fixture of cases) {
      const flags = fixture.flags ?? [1, 1, 1],
        u = new Float32Array(FRAME_FLOAT_COUNT),
        skyGain = fixture.skyGain ?? 1.5,
        sunGain = fixture.sunGain ?? 3,
        localGain = fixture.localGain ?? 0.01;
      u.set(
        [
          0,
          0,
          0,
          Number(!fixture.disabled),
          4,
          4,
          4,
          64,
          2,
          2,
          2,
          3,
          30,
          30,
          10,
          8,
        ],
        FRAME_OFFSETS.probes,
      );
      u.set([...flags, 8], FRAME_OFFSETS.bounceSettings);
      u.set(
        [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        FRAME_OFFSETS.iblRotation,
      );
      u.set([1, 0, 1.1, 0], FRAME_OFFSETS.iblSettings);
      u.set([0.5, 1.25, 2, skyGain], FRAME_OFFSETS.iblGain);
      // Shared frame header: three matrices, eye, sun, sunColor, ambient, fog, settings, cloud.
      u.set([0, 1, 0, sunGain], 52);
      u.set([0.2, 0.4, 0.8, 0], 56);
      u.set([0, 0, 0, localGain], 68);
      u.set([0, 0, 0.7, 0], 72);
      u.set([1, 0, 0, 0], FRAME_OFFSETS.skyLayers);
      device.queue.writeBuffer(uniform, 0, u);
      device.queue.writeBuffer(
        point,
        0,
        new Float32Array(
          fixture.outside
            ? [100, 2, 2, fixture.selector]
            : fixture.selector === 5
              ? [2, 1.85, 2, 5]
              : [2, 1.9, 1.95, fixture.selector],
        ),
      );
      const base = new Float32Array(8 * 40),
        bounced = new Float32Array(3 * 8 * 36);
      for (let i = 0; i < 8; i++) {
        const valid = !fixture.invalid && (!fixture.singleValid || i === 7);
        base.set(
          [(i & 1) * 4, ((i >> 1) & 1) * 4, ((i >> 2) & 1) * 4, Number(valid)],
          i * 40,
        );
        base.set(
          [
            Math.sqrt(4 * Math.PI),
            2 * Math.sqrt(4 * Math.PI),
            3 * Math.sqrt(4 * Math.PI),
            0,
          ],
          i * 40 + 4,
        );
        for (let layer = 0; layer < 3; layer++)
          bounced.set(
            [
              ...layerColors[layer].map(
                (v) => v * Math.sqrt(4 * Math.PI) * (fixture.vary ? i + 1 : 1),
              ),
              0,
            ],
            (layer * 8 + i) * 36,
          );
      }
      device.queue.writeBuffer(records, 0, base);
      device.queue.writeBuffer(bounce, 0, bounced);
      const distances = new Uint16Array(30 * 30 * 4);
      for (let i = 0; i < 30 * 30; i++)
        distances.set([0x5400, 0x6c00, 0, 0x3c00], i * 4);
      device.queue.writeTexture(
        { texture: moments },
        distances,
        { bytesPerRow: 30 * 8 },
        [30, 30],
      );
      let expected;
      if (fixture.selector === 5) {
        const sky = layerColors[0].map((v) => v * flags[0]),
          sun = layerColors[1].map((v) => v * flags[1]),
          local = layerColors[2].map((v) => v * flags[2]);
        expected = [0.5, 0.25, 0.75].map(
          (base, c) =>
            base *
            0.96 *
            0.6 *
            (([1, 2, 3][c] + sky[c]) * [0.5, 1.25, 2][c] * skyGain * 0.7 * 1.1 +
              sun[c] * [0.2, 0.4, 0.8][c] * sunGain +
              local[c] * localGain),
        );
      } else if (fixture.invalid || fixture.outside || fixture.disabled) {
        expected = [0, 0, 0];
      } else if (fixture.selector === 0) {
        expected = [1, 2, 3];
      } else {
        const layer = fixture.selector - 2;
        let factor = 1;
        if (fixture.vary) {
          // At the center, trilinear factors cancel. Upper/lower groups have
          // analytic normal-wrap weights; the shader's common cubic factor cancels too.
          const low = (((1 - 1 / Math.sqrt(3)) / 2) ** 2 + 0.2) ** 3,
            high = (((1 + 1 / Math.sqrt(3)) / 2) ** 2 + 0.2) ** 3;
          factor =
            (low * (1 + 2 + 5 + 6) + high * (3 + 4 + 7 + 8)) /
            (4 * (low + high));
        }
        expected = layerColors[layer].map((v) => v * flags[layer] * factor);
      }
      expected.push(
        fixture.invalid || fixture.outside || fixture.disabled ? 0 : 1,
      );
      bounceValues.push({
        name: fixture.name,
        pixel: await readPixel(),
        expected,
      });
    }
    const decompositionValues = [];
    const root4pi = Math.sqrt(4 * Math.PI),
      primary = [-2, 1, 3],
      constant = [3, 0.5, 2],
      skyPrimary = [-1, 2, 3],
      skyConstant = [2, 4, 1],
      reflectionPrimary = [1, 2, 3],
      reflectionConstant = [0.5, 1, 0.25],
      gain = [0.5, 1.25, 2],
      strength = 0.25 * 0.7 * 1.1;
    for (const [layer, color] of [
      reflectionPrimary,
      reflectionConstant,
    ].entries())
      device.queue.writeTexture(
        { texture: reflection, origin: [0, 0, layer] },
        encodeRgba16fLE(new Float32Array([...color, 1])),
        { bytesPerRow: 8 },
        [1, 1],
      );
    device.queue.writeTexture(
      { texture: brdf },
      encodeRgba16fLE(new Float32Array([1, 0, 0, 1])),
      { bytesPerRow: 8 },
      [1, 1],
    );
    for (const ratio of [0, 1, 2])
      for (const companions of [false, true])
        for (const selector of [0, 2, 3, 4, 6, 7]) {
          const u = new Float32Array(FRAME_FLOAT_COUNT);
          u.set(
            [0, 0, 0, 1, 4, 4, 4, 64, 2, 2, 2, 3, 30, 30, 10, 8],
            FRAME_OFFSETS.probes,
          );
          u.set([1, 1, 1, 8], FRAME_OFFSETS.bounceSettings);
          u.set(
            [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            FRAME_OFFSETS.iblRotation,
          );
          u.set([1, 0, 1.1, 0], FRAME_OFFSETS.iblSettings);
          u.set([...gain, 0.25], FRAME_OFFSETS.iblGain);
          u.set([0, 0, 0.7, 0], 72);
          u.set(
            [ratio, Number(companions), Number(companions), Number(companions)],
            FRAME_OFFSETS.skyLayers,
          );
          u.set([...primary.map((n) => n * root4pi), 0], FRAME_OFFSETS.iblSH);
          u.set(
            [...constant.map((n) => n * root4pi), 0],
            FRAME_OFFSETS.iblConstantSH,
          );
          const base = new Float32Array(8 * (40 + 36)),
            bounced = new Float32Array(4 * 8 * 36);
          for (let i = 0; i < 8; i++) {
            base.set(
              [(i & 1) * 4, ((i >> 1) & 1) * 4, ((i >> 2) & 1) * 4, 1],
              i * 40,
            );
            base.set([...primary.map((n) => n * root4pi), 0], i * 40 + 4);
            base.set([...constant.map((n) => n * root4pi), 0], 8 * 40 + i * 36);
            for (const [layer, color] of [
              skyPrimary,
              layerColors[1],
              layerColors[2],
              skyConstant,
            ].entries())
              bounced.set(
                [...color.map((n) => n * root4pi), 0],
                (layer * 8 + i) * 36,
              );
          }
          device.queue.writeBuffer(uniform, 0, u);
          device.queue.writeBuffer(records, 0, base);
          device.queue.writeBuffer(bounce, 0, bounced);
          device.queue.writeBuffer(
            point,
            0,
            new Float32Array(
              selector === 6
                ? [2, 1.85, 2, selector]
                : [2, 1.9, 1.95, selector],
            ),
          );
          const combined = (a, b) =>
            a.map((n, c) => Math.max(0, n * ratio + (companions ? b[c] : 0)));
          let expected;
          if (selector === 0 || selector === 7)
            expected = combined(primary, constant);
          else if (selector === 2) expected = combined(skyPrimary, skyConstant);
          else if (selector === 3 || selector === 4)
            expected = layerColors[selector - 2];
          else
            expected = combined(reflectionPrimary, reflectionConstant).map(
              (n, c) => n * [0.5, 0.25, 0.75][c] * 0.6 * gain[c] * strength,
            );
          decompositionValues.push({
            name: `solar-${ratio}-${companions ? "constant" : "legacy"}-${selector}`,
            pixel: await readPixel(),
            expected: [...expected, 1],
          });
        }
    // Companion data remains inert when the sky-bounce layer itself is disabled.
    const disabled = new Float32Array(FRAME_FLOAT_COUNT);
    disabled.set(
      [0, 0, 0, 1, 4, 4, 4, 64, 2, 2, 2, 3, 30, 30, 10, 8],
      FRAME_OFFSETS.probes,
    );
    disabled.set([0, 1, 1, 8], FRAME_OFFSETS.bounceSettings);
    disabled.set([2, 1, 1, 1], FRAME_OFFSETS.skyLayers);
    device.queue.writeBuffer(uniform, 0, disabled);
    device.queue.writeBuffer(point, 0, new Float32Array([2, 1.9, 1.95, 2]));
    decompositionValues.push({
      name: "disabled-constant-bounce",
      pixel: await readPixel(),
      expected: [0, 0, 0, 1],
    });
    await device.queue.onSubmittedWorkDone();
    for (const resource of [
      uniform,
      point,
      records,
      bounce,
      moments,
      reflection,
      brdf,
      output,
      readback,
    ])
      resource.destroy();
    device.destroy();
    return { shaderErrors, errors, values, bounceValues, decompositionValues };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  for (const value of result.values) {
    const expected =
      value.name === "open-visibility"
        ? [1, 1, 1, 1]
        : value.name === "fixed-hit-visibility"
          ? [0, 0, 0, 1]
          : value.name === "binary-visibility"
            ? [0.5, 0.5, 0.5, 1]
            : value.name === "constant-sky"
              ? [1, 2, 3, 1]
              : value.name === "blocked-sky"
                ? [0, 0, 0, 1]
                : [0, 0, 0, 0];
    value.pixel.forEach((n, i) =>
      assert.ok(
        Math.abs(n - expected[i]) < 0.0001,
        `${value.name} ${value.pixel} != ${expected}`,
      ),
    );
  }
  assert.equal(result.bounceValues.length, 24);
  for (const value of result.bounceValues)
    value.pixel.forEach((actual, c) =>
      assert.ok(
        Number.isFinite(actual) &&
          Math.abs(actual - value.expected[c]) <
            0.0001 + Math.abs(value.expected[c]) * 0.000003,
        `${value.name} channel${c}: ${value.pixel} != ${value.expected}`,
      ),
    );
  assert.equal(result.decompositionValues.length, 37);
  for (const value of result.decompositionValues)
    value.pixel.forEach((actual, c) =>
      assert.ok(
        Number.isFinite(actual) &&
          Math.abs(actual - value.expected[c]) <
            0.0001 + Math.abs(value.expected[c]) * 0.000003,
        `${value.name} channel${c}: ${value.pixel} != ${value.expected}`,
      ),
    );
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
