// Owned synthetic 3D fields only. Run through openagents browser run.
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
  res.end("<!doctype html><title>Owned ambient volume qualification</title>");
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
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const {
      FogAmbientVolume,
      fogAmbientProbeIncident,
      FOG_AMBIENT_VOLUME_WGSL,
    } = await import("/packages/renderer/fog-ambient-volume.ts");
    const { LocalFog } = await import("/packages/renderer/local-fog.ts");
    const { FRAME_FLOAT_COUNT, FRAME_OFFSETS } =
      await import("/packages/renderer/frame.ts");
    const { perspective, inverse } = await import("/packages/renderer/math.ts");
    const { encodeRgba16fLE, halfToFloat } =
      await import("/packages/renderer/environment-lighting.ts");
    const { buildClusteredLights } =
      await import("/packages/renderer/clustered-lights.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const hasTimestamps = adapter.features.has("timestamp-query");
    const device = await adapter.requestDevice({
        requiredFeatures: hasTimestamps ? ["timestamp-query"] : [],
      }),
      errors = [],
      resources = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const compilation = await device
        .createShaderModule({ code: FOG_AMBIENT_VOLUME_WGSL })
        .getCompilationInfo(),
      shaderErrors = compilation.messages
        .filter((m) => m.type === "error")
        .map((m) => m.message);
    if (shaderErrors.length) return { errors, shaderErrors };
    const volume = await FogAmbientVolume.create(device),
      fog = await LocalFog.create(device),
      width = 64,
      height = 36;
    const projection = perspective(Math.PI / 3, width / height, 0.1, 100),
      inv = inverse(projection),
      frame = new Float32Array(FRAME_FLOAT_COUNT);
    frame.set(projection);
    frame.set(inv, 16);
    frame.set([0, 0, 0, 0], 48);
    frame.set(
      [-64, -64, -128, 1, 64, 64, 64, 128, 3, 3, 3, 6, 60, 50, 10, 8],
      FRAME_OFFSETS.probes,
    );
    frame.set([1, 0, 3, 0], FRAME_OFFSETS.iblSettings);
    frame.set([1, 1, 1, 0.01], FRAME_OFFSETS.iblGain);
    frame.set([0, 0, 1, 0], 72);
    frame[FRAME_OFFSETS.skyEnabled] = 1;
    frame.set([1, 0, 1, 0], FRAME_OFFSETS.skyLayers);
    const makeBuffer = (size, usage) => {
      const value = device.createBuffer({ size, usage });
      resources.push(value);
      return value;
    };
    const frameBuffer = makeBuffer(
        frame.byteLength,
        GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      ),
      count = 27,
      records = new Float32Array(count * 76),
      probeRecords = makeBuffer(
        records.byteLength,
        GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      );
    const texture = (descriptor) => {
      const t = device.createTexture(descriptor);
      resources.push(t);
      return t;
    };
    const moments = texture({
        size: [60, 50],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      }),
      momentView = moments.createView(),
      momentPixels = new Float32Array(60 * 50 * 4);
    const read = makeBuffer(
      110592,
      GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    );
    function rayAt(x, y) {
      const xx = ((x + 0.5) / 32) * 2 - 1,
        yy = 1 - ((y + 0.5) / 18) * 2;
      const q = [0, 1, 2, 3].map(
        (row) =>
          inv[row] * xx + inv[4 + row] * yy + inv[8 + row] + inv[12 + row],
      );
      const v = q.slice(0, 3).map((n) => n / q[3]),
        length = Math.hypot(...v);
      return v.map((n) => n / length);
    }
    const queries = hasTimestamps
        ? device.createQuerySet({ type: "timestamp", count: 2 })
        : undefined,
      timingResolve = hasTimestamps
        ? makeBuffer(16, GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC)
        : undefined,
      timingRead = hasTimestamps
        ? makeBuffer(16, GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ)
        : undefined;
    const basePrimary = [-2, 1, 3],
      baseConstant = [3, 0.5, 2],
      root4pi = Math.sqrt(4 * Math.PI);
    let maxNormalizedError = 0,
      comparisons = 0;
    async function run({
      name,
      ratio = 1,
      phase = 0.2,
      enabled = true,
      sky = true,
      fieldEnabled = true,
      blocked = false,
      mixed = false,
      invalid = false,
      outside = false,
      strength = 1,
      anisotropic = false,
      gain = [1, 1, 1, 0.01],
      primary = basePrimary,
      constant = baseConstant,
    } = {}) {
      records.fill(0);
      for (let i = 0; i < count; i++) {
        records.set(
          [
            -64 + (i % 3) * 64,
            -64 + (Math.floor(i / 3) % 3) * 64,
            -128 + Math.floor(i / 9) * 64,
            Number(!invalid),
          ],
          i * 40,
        );
        records.set([...primary.map((n) => n * root4pi), 0], i * 40 + 4);
        records.set(
          [...constant.map((n) => n * root4pi), 0],
          count * 40 + i * 36,
        );
        if (anisotropic) {
          records.set([0.2, 0.3, -0.1, 0], i * 40 + 8);
          records.set([0.1, -0.1, 0.2, 0], count * 40 + i * 36 + 24);
        }
      }
      for (let y = 0; y < 50; y++)
        for (let x = 0; x < 60; x++) {
          const index = Math.floor(x / 10) + 6 * Math.floor(y / 10),
            hit = blocked || (mixed && index % 2);
          momentPixels.set(
            hit ? [0, 0, 0, 1] : [128, 16384, 0, 1],
            (y * 60 + x) * 4,
          );
        }
      device.queue.writeTexture(
        { texture: moments },
        encodeRgba16fLE(momentPixels),
        { bytesPerRow: 60 * 8 },
        [60, 50],
      );
      frame[FRAME_OFFSETS.skyEnabled] = Number(sky);
      frame[FRAME_OFFSETS.probes + 3] = Number(fieldEnabled);
      frame[FRAME_OFFSETS.probes] = outside ? 1000 : -64;
      frame[FRAME_OFFSETS.skyLayers] = ratio;
      frame.set(gain, FRAME_OFFSETS.iblGain);
      device.queue.writeBuffer(frameBuffer, 0, frame);
      device.queue.writeBuffer(probeRecords, 0, records);
      const encoder = device.createCommandEncoder(),
        started = performance.now();
      const measured = queries
        ? {
            beginComputePass: (descriptor) =>
              encoder.beginComputePass({
                ...descriptor,
                timestampWrites: {
                  querySet: queries,
                  beginningOfPassWriteIndex: 0,
                  endOfPassWriteIndex: 1,
                },
              }),
          }
        : encoder;
      volume.render(measured, {
        frameBuffer,
        probeRecords,
        probeMoments: momentView,
        enabled,
        phaseG: phase,
        scatteringIntensity: strength,
        startDistance: 0,
        maxDistance: 60,
      });
      encoder.copyTextureToBuffer(
        { texture: volume.texture },
        { buffer: read, bytesPerRow: 256, rowsPerImage: 18 },
        [32, 18, 24],
      );
      if (queries) {
        encoder.resolveQuerySet(queries, 0, 2, timingResolve, 0);
        encoder.copyBufferToBuffer(timingResolve, 0, timingRead, 0, 16);
      }
      device.queue.submit([encoder.finish()]);
      await Promise.all([
        read.mapAsync(GPUMapMode.READ),
        ...(timingRead ? [timingRead.mapAsync(GPUMapMode.READ)] : []),
      ]);
      let gpuMs;
      if (timingRead) {
        const stamps = new BigUint64Array(timingRead.getMappedRange());
        gpuMs = Number(stamps[1] - stamps[0]) / 1e6;
        timingRead.unmap();
      }
      const values = new Uint16Array(read.getMappedRange()).slice();
      read.unmap();
      const submittedAndReadMs = performance.now() - started;
      const cpuField = {
        origin: [outside ? 1000 : -64, -64, -128],
        spacing: [64, 64, 64],
        dimensions: [3, 3, 3],
        records,
        constantFog: true,
        moments: (index) =>
          blocked || (mixed && index % 2) ? [0, 0] : [128, 16384],
      };
      let maximum = 0,
        localError = 0;
      for (let z = 0; z < 24; z++)
        for (let y = 0; y < 18; y++)
          for (let x = 0; x < 32; x++) {
            const ray = rayAt(x, y),
              point = ray.map((n) => ((n * (z + 0.5)) / 24) * 60),
              incident =
                enabled && sky && fieldEnabled && strength > 0
                  ? fogAmbientProbeIncident(cpuField, point, ray, phase, ratio)
                  : [0, 0, 0];
            const offset = ((z * 18 + y) * 32 + x) * 4;
            for (let c = 0; c < 3; c++) {
              const expected = incident[c] * gain[c] * gain[3] * 3 * strength,
                actual = halfToFloat(values[offset + c]);
              maximum = Math.max(maximum, actual);
              const error =
                Math.abs(actual - expected) / Math.max(1, Math.abs(expected));
              localError = Math.max(localError, error);
              comparisons++;
            }
          }
      let interpolation;
      if (mixed || anisotropic) {
        let squares = 0,
          maxAbsolute = 0;
        const samples = 256;
        for (let k = 0; k < samples; k++) {
          const uv = [
              0.05 + 0.9 * ((k * 0.61803398875) % 1),
              0.05 + 0.9 * ((k * 0.41421356237) % 1),
              0.05 + 0.9 * ((k * 0.73205080757) % 1),
            ],
            ray = rayAt(uv[0] * 32 - 0.5, uv[1] * 18 - 0.5),
            distance = uv[2] * 60,
            world = ray.map((n) => n * distance),
            expected = fogAmbientProbeIncident(
              cpuField,
              world,
              ray,
              phase,
              ratio,
            ).map((n, c) => n * gain[c] * gain[3] * 3 * strength),
            pixel = uv.map((n, i) => n * [32, 18, 24][i] - 0.5),
            lo = pixel.map(Math.floor),
            fraction = pixel.map((n, i) => n - lo[i]),
            actual = [0, 0, 0];
          for (let corner = 0; corner < 8; corner++) {
            const bits = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1],
              p = lo.map((n, i) =>
                Math.min([31, 17, 23][i], Math.max(0, n + bits[i])),
              ),
              weight = fraction.reduce(
                (a, n, i) => a * (bits[i] ? n : 1 - n),
                1,
              ),
              offset = ((p[2] * 18 + p[1]) * 32 + p[0]) * 4;
            for (let c = 0; c < 3; c++)
              actual[c] += halfToFloat(values[offset + c]) * weight;
          }
          for (let c = 0; c < 3; c++) {
            const difference = Math.abs(actual[c] - expected[c]);
            maxAbsolute = Math.max(maxAbsolute, difference);
            squares += difference * difference;
          }
        }
        interpolation = {
          samples,
          rms: Math.sqrt(squares / (samples * 3)),
          maxAbsolute,
        };
      }
      maxNormalizedError = Math.max(maxNormalizedError, localError);
      return {
        name,
        maximum,
        maxNormalizedError: localError,
        submittedAndReadMs,
        gpuMs,
        interpolation,
      };
    }
    const cases = [];
    for (const ratio of [0, 1, 2])
      cases.push(await run({ name: `solar-${ratio}`, ratio }));
    for (const c of [
      {
        name: "isotropic-constant",
        phase: 0,
        primary: [1, 2, 3],
        constant: [0.5, 1, 2],
      },
      { name: "anisotropic", anisotropic: true },
      { name: "mixed-occlusion", mixed: true },
      { name: "closed-wall", blocked: true },
      { name: "no-sky", sky: false },
      { name: "invalid-field", invalid: true },
      { name: "disabled-field", fieldEnabled: false },
      { name: "outside-grid", outside: true },
      { name: "zero-strength", strength: 0 },
      { name: "gain-once", gain: [0.5, 1.25, 2, 0.02], strength: 2 },
      { name: "disabled-after-enabled", enabled: false },
    ])
      cases.push(await run(c));
    // A full LocalFog render with no explicit lights must still integrate ambient radiance.
    await run({
      name: "pipeline-setup",
      phase: 0,
      primary: [1, 2, 3],
      constant: [0.5, 1, 2],
    });
    const depth = texture({
        size: [width, height],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      }),
      depthView = depth.createView();
    const scene = texture({
        size: [width, height],
        format: "rgba16float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      }),
      sceneView = scene.createView();
    const shadow = texture({
        size: [1, 1, 1],
        format: "depth32float",
        usage: GPUTextureUsage.TEXTURE_BINDING,
      }),
      shadowView = shadow.createView({ dimension: "2d-array" }),
      sampler = device.createSampler({ compare: "less-equal" });
    const clusters = buildClusteredLights({
        width,
        height,
        viewProjection: projection,
        eye: [0, 0, 0],
        forward: [0, 0, -1],
        lights: [],
        near: 0.1,
        far: 100,
      }),
      clusterBuffer = makeBuffer(
        clusters.storage.byteLength,
        GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      );
    device.queue.writeBuffer(clusterBuffer, 0, clusters.storage);
    const depthAt = (d) => (projection[10] * -d + projection[14]) / d;
    const depthModule = device.createShaderModule({
      code: `@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.-1.,0.,1.);}@fragment fn fs(@builtin(position)p:vec4<f32>)->@builtin(frag_depth)f32{return select(${depthAt(10)},${depthAt(2)},p.x<32.);}`,
    });
    const depthPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: depthModule, entryPoint: "vs" },
      fragment: { module: depthModule, entryPoint: "fs", targets: [] },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "always",
      },
      multisample: { count: 4 },
    });
    const init = device.createCommandEncoder(),
      dp = init.beginRenderPass({
        colorAttachments: [],
        depthStencilAttachment: {
          view: depthView,
          depthClearValue: 1,
          depthLoadOp: "clear",
          depthStoreOp: "store",
        },
      });
    dp.setPipeline(depthPipeline);
    dp.draw(3);
    dp.end();
    const cp = init.beginRenderPass({
      colorAttachments: [
        {
          view: sceneView,
          loadOp: "clear",
          storeOp: "store",
          clearValue: [0, 0, 0, 1],
        },
      ],
    });
    cp.end();
    device.queue.submit([init.finish()]);
    const imageRead = makeBuffer(
        width * height * 8,
        GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      ),
      profile = {
        enabled: true,
        extinctionAtReference: 0.1,
        referenceHeight: 0,
        heightRate: 0,
        albedo: 0.8,
        phaseG: 0,
        startDistance: 0,
        maxDistance: 60,
      };
    const pipelineCases = [];
    for (const name of [
      "ambient",
      "volume-off",
      "zero-density",
      "without-option",
      "disabled-ambient",
    ]) {
      const e = device.createCommandEncoder();
      fog.render(e, {
        width,
        height,
        sceneColor: sceneView,
        depthView,
        frameBuffer,
        clusterBuffer,
        localShadowView: shadowView,
        shadowSampler: sampler,
        profile: { ...profile, enabled: name !== "volume-off" },
        extinctionAtReference: name === "zero-density" ? 0 : 0.1,
        ...(name === "without-option"
          ? {}
          : {
              ambient: {
                view: volume.view,
                sampler: volume.sampler,
                enabled: name !== "disabled-ambient",
              },
            }),
      });
      // LocalFog exposes its stable sampled view, so copy through an owned shader into a readback target.
      const copyModule = device.createShaderModule({
        code: `@group(0)@binding(0)var image:texture_2d<f32>;@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.-1.,0.,1.);}@fragment fn fs(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{return textureLoad(image,vec2<i32>(p.xy),0);}`,
      });
      const copyPipeline = await device.createRenderPipelineAsync({
          layout: "auto",
          vertex: { module: copyModule, entryPoint: "vs" },
          fragment: {
            module: copyModule,
            entryPoint: "fs",
            targets: [{ format: "rgba16float" }],
          },
        }),
        target = texture({
          size: [width, height],
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        }),
        copyGroup = device.createBindGroup({
          layout: copyPipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: fog.view }],
        });
      const pass = e.beginRenderPass({
        colorAttachments: [
          { view: target.createView(), loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(copyPipeline);
      pass.setBindGroup(0, copyGroup);
      pass.draw(3);
      pass.end();
      e.copyTextureToBuffer(
        { texture: target },
        { buffer: imageRead, bytesPerRow: width * 8 },
        [width, height],
      );
      device.queue.submit([e.finish()]);
      await imageRead.mapAsync(GPUMapMode.READ);
      const image = new Uint16Array(imageRead.getMappedRange()).slice();
      imageRead.unmap();
      const samples = [];
      for (const x of [16, 48]) {
        const y = 18,
          offset = (y * width + x) * 4,
          pixel = [0, 1, 2].map((c) => halfToFloat(image[offset + c]));
        const clip = [
            ((x + 0.5) / width) * 2 - 1,
            1 - ((y + 0.5) / height) * 2,
          ],
          far = [inv[0] * clip[0], inv[5] * clip[1], -1],
          distance = (x < 32 ? 2 : 10) * Math.hypot(...far);
        const expected =
          name === "ambient"
            ? [1.5, 3, 5].map(
                (n) => n * 0.03 * 0.8 * (1 - Math.exp(-0.1 * distance)),
              )
            : [0, 0, 0];
        samples.push({ pixel, expected });
      }
      pipelineCases.push({ name, samples });
    }
    await device.queue.onSubmittedWorkDone();
    fog.dispose();
    volume.dispose();
    volume.dispose();
    resources.forEach((r) => r.destroy());
    queries?.destroy();
    device.destroy();
    return {
      errors,
      shaderErrors,
      cases,
      pipelineCases,
      comparisons,
      maxNormalizedError,
    };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  assert.ok(result.maxNormalizedError < 0.001, JSON.stringify(result));
  for (const c of result.cases) {
    if (
      [
        "closed-wall",
        "no-sky",
        "invalid-field",
        "disabled-field",
        "outside-grid",
        "zero-strength",
        "disabled-after-enabled",
      ].includes(c.name)
    )
      assert.ok(c.maximum < 1e-8, JSON.stringify(c));
  }
  for (const c of result.pipelineCases)
    for (const sample of c.samples)
      sample.pixel.forEach((n, i) =>
        assert.ok(
          Math.abs(n - sample.expected[i]) < 0.0002,
          `${c.name} ${sample.pixel} != ${sample.expected}`,
        ),
      );
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
