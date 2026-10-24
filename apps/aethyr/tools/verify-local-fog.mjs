// Synthetic owned single-scattering qualification. No licensed inputs or account access.
// Run: openagents browser run -- node tools/verify-local-fog.mjs
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
  response.end("<!doctype html><title>Owned local fog qualification</title>");
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
  page.on("pageerror", (e) => pageErrors.push(e.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { LocalFog } = await import("/packages/renderer/local-fog.ts");
    const { buildClusteredLights } =
      await import("/packages/renderer/clustered-lights.ts");
    const { FRAME_FLOAT_COUNT, LOCAL_SHADOW_VIEWS } =
      await import("/packages/renderer/frame.ts");
    const { perspective, inverse } = await import("/packages/renderer/math.ts");
    const { FOG_SEGMENT_WGSL, heightFogComposition, segmentFogOpticalDepth } =
      await import("/packages/renderer/fog.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice();
    const errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    // Independent pure transport matrix. This covers finite sky endpoints and
    // near/far replacement without relying on the render pass's image fixtures.
    const compositionCases = [];
    for (const distance of [0, 1, 59.999, 60, 60.001, 150, 13540, 100000])
      for (const direction of [-1, -0.01, 0, 0.01, 1])
        for (const enabled of [0, 1])
          for (const start of [0, 10])
            compositionCases.push([
              1,
              direction,
              distance,
              0.002,
              0.04 * Math.LN2,
              1,
              0,
              0,
              0.5 / Math.LN2 ** 2,
              0.04 * Math.LN2,
              1,
              start,
              60,
              enabled,
              0,
              0,
            ]);
    for (const values of [
      [0, 0, 100, 0.01, 0, 0, 80, 0, 2, 0, 0, 10, 60, 1, 0, 0],
      [0, 0, 100, 0.01, 0, 0, 0, 0, 2, 0, 0, 10, 60, 1, 0, 0],
      [0, 0, 100, 0, 0, 0, 0, 0, 2, 0, 0, 10, 60, 1, 0, 0],
      [
        1,
        0.000000000001,
        100,
        0.002,
        0.04 * Math.LN2,
        1,
        0,
        0,
        1,
        0.04 * Math.LN2,
        1,
        0,
        60,
        0,
        0,
        0,
      ],
    ])
      compositionCases.push(values);
    const packedCases = new Float32Array(compositionCases.flat()),
      caseBuffer = device.createBuffer({
        size: packedCases.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      }),
      resultBuffer = device.createBuffer({
        size: compositionCases.length * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      }),
      resultRead = device.createBuffer({
        size: resultBuffer.size,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
    device.queue.writeBuffer(caseBuffer, 0, packedCases);
    const compositionModule = device.createShaderModule({
      code:
        FOG_SEGMENT_WGSL +
        `
struct Case {ray:vec4<f32>,analytic:vec4<f32>,volume:vec4<f32>,range:vec4<f32>};
@group(0)@binding(0)var<storage,read>cases:array<Case>;
@group(0)@binding(1)var<storage,read_write>results:array<vec4<f32>>;
@compute@workgroup_size(64)fn main(@builtin(global_invocation_id)i:vec3<u32>){
 if i.x>=${compositionCases.length}u{return;}let c=cases[i.x];
 let terms=heightFogComposition(c.ray.x,c.ray.y,c.ray.z,c.ray.w,c.analytic.x,c.analytic.y,c.analytic.z,c.volume,c.range.xy);
 let tau=segmentFogOpticalDepth(c.ray.x,c.ray.y,0.0,c.ray.z,c.ray.w,c.analytic.x,c.analytic.y);
 results[i.x]=vec4(terms,tau,0.0);
}`,
    });
    const compositionPipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module: compositionModule, entryPoint: "main" },
    });
    const compositionGroup = device.createBindGroup({
      layout: compositionPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: caseBuffer } },
        { binding: 1, resource: { buffer: resultBuffer } },
      ],
    });
    const compositionEncoder = device.createCommandEncoder(),
      compositionPass = compositionEncoder.beginComputePass();
    compositionPass.setPipeline(compositionPipeline);
    compositionPass.setBindGroup(0, compositionGroup);
    compositionPass.dispatchWorkgroups(Math.ceil(compositionCases.length / 64));
    compositionPass.end();
    compositionEncoder.copyBufferToBuffer(
      resultBuffer,
      0,
      resultRead,
      0,
      resultBuffer.size,
    );
    device.queue.submit([compositionEncoder.finish()]);
    await resultRead.mapAsync(GPUMapMode.READ);
    const compositionValues = new Float32Array(
      resultRead.getMappedRange(),
    ).slice();
    resultRead.unmap();
    let compositionMaxError = 0,
      compositionFinite = true;
    for (let i = 0; i < compositionCases.length; i++) {
      const c = packedCases.slice(i * 16, (i + 1) * 16);
      const expected = heightFogComposition(
        c[0],
        c[1],
        c[2],
        c[3],
        c[4],
        c[5],
        c[6],
        Array.from(c.slice(8, 12)),
        Array.from(c.slice(12, 14)),
      );
      expected.push(
        segmentFogOpticalDepth(c[0], c[1], 0, c[2], c[3], c[4], c[5]),
      );
      for (let channel = 0; channel < 3; channel++) {
        const value = compositionValues[i * 4 + channel];
        compositionFinite &&= Number.isFinite(value);
        compositionMaxError = Math.max(
          compositionMaxError,
          Math.abs(value - expected[channel]) / Math.max(1, expected[channel]),
        );
      }
    }
    caseBuffer.destroy();
    resultBuffer.destroy();
    resultRead.destroy();
    const fog = await LocalFog.create(device),
      width = 64,
      height = 32;
    fog.resize(width, height);
    const projection = perspective(Math.PI / 3, width / height, 0.1, 100),
      frame = new Float32Array(FRAME_FLOAT_COUNT);
    frame.set(projection);
    frame.set(inverse(projection), 16);
    const frameBuffer = device.createBuffer({
      size: frame.byteLength,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const depth = device.createTexture({
      size: [width, height],
      format: "depth24plus",
      sampleCount: 4,
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const input = device.createTexture({
      size: [width, height],
      format: "rgba16float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const shadow = device.createTexture({
      size: [4, 4, LOCAL_SHADOW_VIEWS],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const sunNear = device.createTexture({
      size: [4, 4],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const sunFar = device.createTexture({
      size: [4, 4],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    const sunNearView = sunNear.createView(),
      sunFarView = sunFar.createView();
    const shadowSampler = device.createSampler({
      compare: "less-equal",
      minFilter: "linear",
      magFilter: "linear",
    });
    const planeDepth = (d) => (projection[10] * -d + projection[14]) / d;
    const module = device.createShaderModule({
      code: `@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.,1.);}@fragment fn fs(@builtin(position)p:vec4<f32>)->@builtin(frag_depth)f32{return select(${planeDepth(10)},${planeDepth(1)},p.x<32.);}`,
    });
    const depthPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [] },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "always",
      },
      multisample: { count: 4 },
    });
    const init = device.createCommandEncoder();
    const colorPass = init.beginRenderPass({
      colorAttachments: [
        {
          view: input.createView(),
          loadOp: "clear",
          storeOp: "store",
          clearValue: [0.03125, 0.03125, 0.03125, 1],
        },
      ],
    });
    colorPass.end();
    const depthPass = init.beginRenderPass({
      colorAttachments: [],
      depthStencilAttachment: {
        view: depth.createView(),
        depthClearValue: 1,
        depthLoadOp: "clear",
        depthStoreOp: "store",
      },
    });
    depthPass.setPipeline(depthPipeline);
    depthPass.draw(3);
    depthPass.end();
    device.queue.submit([init.finish()]);
    const profile = {
      enabled: true,
      extinctionAtReference: 0.1,
      referenceHeight: 0,
      heightRate: 0,
      albedo: 0.8,
      phaseG: 0,
      startDistance: 0,
      maxDistance: 60,
    };
    const light = {
      id: "owned-light",
      label: "Owned light",
      kind: "point",
      position: [0, 0, -5],
      direction: [0, 0, 1],
      range: 1.5,
      color: [1, 0.3, 0.1],
      intensity: 100,
      intensityUnits: "candelas",
      inverseSquaredFalloff: true,
      castShadows: true,
      enabled: true,
      temperatureKelvin: 6500,
      useTemperature: false,
      raw: { volumetric_scattering_intensity: 1 },
    };
    let revision = 0;
    const half = (n) => {
      const e = (n >> 10) & 31,
        f = n & 1023,
        s = n & 32768 ? -1 : 1;
      return e === 0
        ? s * 2 ** -14 * (f / 1024)
        : e === 31
          ? f
            ? NaN
            : s * Infinity
          : s * 2 ** (e - 15) * (1 + f / 1024);
    };
    const sample = (data, x, y = 16) => data[(y * width + x) * 4];
    async function run({
      density = 0.1,
      black = false,
      strength = 1,
      shadowed = false,
      steps = 24,
      homogeneous = false,
      enabled = true,
      noLocal = false,
      sun = 0,
      sunRadiance = 1,
      sunShadow = "lit",
    } = {}) {
      const source = {
        ...light,
        color: black ? [0, 0, 0] : light.color,
        raw: { volumetric_scattering_intensity: strength },
        ...(homogeneous
          ? {
              color: [1, 1, 1],
              range: 100,
              inverseSquaredFalloff: false,
              falloffExponent: 0,
            }
          : {}),
      };
      const clusters = buildClusteredLights({
        width,
        height,
        viewProjection: projection,
        eye: [0, 0, 0],
        forward: [0, 0, -1],
        lights: noLocal ? [] : [source],
        near: 0.1,
        far: 100,
        shadowIndices: shadowed ? new Map([[source.id, 0]]) : undefined,
      });
      const buffer = device.createBuffer({
        size: clusters.storage.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
      device.queue.writeBuffer(buffer, 0, clusters.storage);
      fog.setLightScattering([source], clusters.sourceIndices, ++revision);
      frame.set([0, 0, -5, 1.5], 80);
      frame[530] = 0;
      frame.set([0, 0, -1, sunRadiance], 52);
      frame.set([1, 0.5, 0.25, 0], 56);
      frame.fill(0, 32, 48);
      frame[46] = 0.5;
      frame[47] = 1;
      frame.fill(0, 336, 352);
      frame[350] = 0.5;
      frame[351] = 1;
      if (sunShadow === "outside-lit" || sunShadow === "outside-blocked")
        frame[44] = 2;
      if (sunShadow === "edge") frame[44] = 0.9;
      for (let layer = 0; layer < LOCAL_SHADOW_VIEWS; layer++) {
        frame[608 + layer * 16 + 14] = 0.5;
        frame[608 + layer * 16 + 15] = 1;
      }
      device.queue.writeBuffer(frameBuffer, 0, frame);
      const encoder = device.createCommandEncoder();
      for (let layer = 0; layer < LOCAL_SHADOW_VIEWS; layer++) {
        const pass = encoder.beginRenderPass({
          colorAttachments: [],
          depthStencilAttachment: {
            view: shadow.createView({
              dimension: "2d",
              baseArrayLayer: layer,
              arrayLayerCount: 1,
            }),
            depthClearValue: shadowed ? 0 : 1,
            depthLoadOp: "clear",
            depthStoreOp: "store",
          },
        });
        pass.end();
      }
      for (const [texture, blocked] of [
        [sunNear, sunShadow === "near-blocked" || sunShadow === "edge"],
        [sunFar, sunShadow === "outside-blocked"],
      ]) {
        const pass = encoder.beginRenderPass({
          colorAttachments: [],
          depthStencilAttachment: {
            view: texture.createView(),
            depthClearValue: blocked ? 0 : 1,
            depthLoadOp: "clear",
            depthStoreOp: "store",
          },
        });
        pass.end();
      }
      fog.render(encoder, {
        width,
        height,
        sceneColor: input.createView(),
        depthView: depth.createView(),
        frameBuffer,
        clusterBuffer: buffer,
        localShadowView: shadow.createView({ dimension: "2d-array" }),
        shadowSampler,
        profile: { ...profile, enabled },
        extinctionAtReference: density,
        steps,
        directional: {
          scatteringIntensity: sun,
          nearShadowView: sunNearView,
          farShadowView: sunFarView,
        },
      });
      const read = device.createBuffer({
        size: width * height * 8,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      encoder.copyTextureToBuffer(
        { texture: fog.output },
        { buffer: read, bytesPerRow: width * 8 },
        [width, height],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const values = Array.from(new Uint16Array(read.getMappedRange())).map(
        half,
      );
      read.unmap();
      read.destroy();
      buffer.destroy();
      return values;
    }
    const baseline = await run({ density: 0 }),
      disabled = await run({ enabled: false }),
      black = await run({ black: true }),
      zero = await run({ strength: 0 }),
      lit = await run(),
      blocked = await run({ shadowed: true }),
      homogeneous = await run({ homogeneous: true, steps: 32 }),
      sunlight = await run({ noLocal: true, sun: 1 }),
      sunGain = await run({ noLocal: true, sun: 2.2 }),
      sunBlack = await run({ noLocal: true, sun: 2.2, sunRadiance: 0 }),
      sunZero = await run({ noLocal: true, sun: 0 }),
      sunNearBlocked = await run({
        noLocal: true,
        sun: 1,
        sunShadow: "near-blocked",
      }),
      sunFarBlocked = await run({
        noLocal: true,
        sun: 1,
        sunShadow: "outside-blocked",
      }),
      sunFarFallback = await run({
        noLocal: true,
        sun: 1,
        sunShadow: "outside-lit",
      }),
      sunEdge = await run({ noLocal: true, sun: 1, sunShadow: "edge" });
    const near = sample(lit, 30),
      far = sample(lit, 33),
      occluded = sample(blocked, 33),
      uniform = sample(homogeneous, 33);
    const expected =
      0.03125 + (0.8 * (1 - Math.exp(-0.1 * 10)) * 1) / (4 * Math.PI);
    const checks = {
      allFinite: [
        baseline,
        disabled,
        black,
        zero,
        lit,
        blocked,
        homogeneous,
        sunlight,
        sunGain,
        sunBlack,
        sunZero,
        sunNearBlocked,
        sunFarBlocked,
        sunFarFallback,
        sunEdge,
      ].every((a) => a.every(Number.isFinite)),
      baselineMax: Math.max(...baseline.filter((_, i) => i % 4 < 3)),
      disabledEqualsBaseline: disabled.every((v, i) => v === baseline[i]),
      blackEqualsBaseline: black.every((v, i) => v === baseline[i]),
      zeroEqualsBaseline: zero.every((v, i) => v === baseline[i]),
      near,
      far,
      occluded,
      uniform,
      expected,
      homogeneousRelativeError: Math.abs(uniform - expected) / expected,
      sun: {
        lit: sample(sunlight, 33),
        gain: sample(sunGain, 33),
        gainRatio:
          (sample(sunGain, 33) - 0.03125) / (sample(sunlight, 33) - 0.03125),
        zeroEqualsBaseline: sunZero.every((v, i) => v === baseline[i]),
        blackEqualsBaseline: sunBlack.every((v, i) => v === baseline[i]),
        nearShadowEqualsBaseline: sunNearBlocked.every(
          (v, i) => v === baseline[i],
        ),
        farShadowEqualsBaseline: sunFarBlocked.every(
          (v, i) => v === baseline[i],
        ),
        farFallbackEqualsLit: sunFarFallback.every((v, i) => v === sunlight[i]),
        edgeRatio:
          (sample(sunEdge, 33) - 0.03125) / (sample(sunlight, 33) - 0.03125),
      },
      errors,
      composition: {
        cases: compositionCases.length,
        finite: compositionFinite,
        maxNormalizedError: compositionMaxError,
      },
    };
    fog.dispose();
    frameBuffer.destroy();
    depth.destroy();
    input.destroy();
    shadow.destroy();
    sunNear.destroy();
    sunFar.destroy();
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return checks;
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.allFinite, true);
  assert.equal(result.composition.finite, true);
  assert.ok(
    result.composition.maxNormalizedError < 0.00003,
    JSON.stringify(result.composition),
  );
  assert.equal(result.baselineMax, 0.03125);
  assert.equal(result.disabledEqualsBaseline, true);
  assert.equal(result.blackEqualsBaseline, true);
  assert.equal(result.zeroEqualsBaseline, true);
  assert.equal(result.near, 0.03125);
  assert.ok(result.far > 0.0315, JSON.stringify(result));
  assert.equal(result.occluded, 0.03125);
  assert.ok(result.homogeneousRelativeError < 0.01, JSON.stringify(result));
  assert.ok(result.sun.lit > 0.0315, JSON.stringify(result));
  assert.ok(
    Math.abs(result.sun.gainRatio - 2.2) < 0.01,
    JSON.stringify(result),
  );
  for (const key of [
    "zeroEqualsBaseline",
    "blackEqualsBaseline",
    "nearShadowEqualsBaseline",
    "farShadowEqualsBaseline",
    "farFallbackEqualsLit",
  ])
    assert.equal(result.sun[key], true, key);
  assert.ok(
    Math.abs(result.sun.edgeRatio - 0.5) < 0.01,
    JSON.stringify(result),
  );
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
