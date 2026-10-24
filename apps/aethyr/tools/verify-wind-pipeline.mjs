// Exercise the actual shared production vertex function with synthetic inputs only.
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
  res.end("<title>Owned wind vertex qualification</title>");
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
    const { MATERIAL } = await import("/packages/renderer/shaders.ts"),
      { FRAME_FLOAT_COUNT, FRAME_OFFSETS } =
        await import("/packages/renderer/frame.ts"),
      { windSwayOffset } = await import("/packages/renderer/wind-sway.ts");
    const {
      boundedFlutterTime,
      windFlutterOffset,
      windFlutterBound,
      WIND_FLUTTER_CLOCK_SCALE,
    } = await import("/packages/renderer/wind-flutter.ts");
    const { MAX_WORLD_ANIMATION_TIME } =
      await import("/packages/runtime/world-clock.ts");
    const { LAYER_FLOAT_COUNT } = await import("/packages/renderer/layers.ts");
    const { GLASS_FLOAT_COUNT } = await import("/packages/renderer/glass.ts");
    const { PAINT_FLOAT_COUNT } = await import("/packages/renderer/paint.ts");
    const { TERRAIN_FLOAT_COUNT } =
      await import("/packages/renderer/terrain.ts");
    const vegetationOffset =
      20 +
      LAYER_FLOAT_COUNT +
      GLASS_FLOAT_COUNT +
      PAINT_FLOAT_COUNT +
      TERRAIN_FLOAT_COUNT;
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const code =
      MATERIAL +
      `\nstruct WindInput{point:vec4<f32>,m0:vec4<f32>,m1:vec4<f32>,m2:vec4<f32>,m3:vec4<f32>,parameters:vec4<f32>};struct WindOutput{world:vec4<f32>,original:vec4<f32>,clip:vec4<f32>,normal:vec4<f32>};@group(2)@binding(0)var<storage,read>windInputs:array<WindInput>;@group(2)@binding(1)var<storage,read_write>windOutputs:array<WindOutput>;@compute@workgroup_size(32)fn qualify(@builtin(global_invocation_id)i:vec3<u32>){if i.x>=arrayLength(&windInputs){return;}let q=windInputs[i.x];var v:In;v.pos=q.point.xyz;v.normal=vec3(0.0,1.0,0.0);v.uv=vec2(0.0);v.tangent=vec4(1.0,0.0,0.0,1.0);v.m0=q.m0;v.m1=q.m1;v.m2=q.m2;v.m3=q.m3;v.color=vec4(1.0,q.point.w,1.0,1.0);v.uv1=vec2(0.0);v.wind=q.parameters;let o=vertexValue(v,0u,false);windOutputs[i.x]=WindOutput(vec4(o.world,1.0),vec4(o.undeformedWorld,1.0),o.clip,vec4(o.normal,1.0));}`;
    const module = device.createShaderModule({ code }),
      info = await module.getCompilationInfo(),
      shaderErrors = info.messages
        .filter((m) => m.type === "error")
        .map((m) => `${m.lineNum}:${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const pipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module, entryPoint: "qualify" },
    });
    const create = (size, usage) =>
        device.createBuffer({ size, usage: usage | GPUBufferUsage.COPY_DST }),
      frame = create(FRAME_FLOAT_COUNT * 4, GPUBufferUsage.UNIFORM),
      material = create(16384, GPUBufferUsage.UNIFORM),
      surfaceSkyZero = create(16, GPUBufferUsage.STORAGE);
    const flutterColor = device.createTexture({
      size: [1, 1, 1],
      format: "rgba8unorm-srgb",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    device.queue.writeTexture(
      { texture: flutterColor },
      new Uint8Array([51, 102, 153, 255]),
      {},
      [1, 1, 1],
    );
    const flutterSampler = device.createSampler();
    const count = 256,
      source = new Float32Array(count * 24),
      records = [];
    for (let i = 0; i < count; i++) {
      const point = [Math.cos(i * 0.4) * 0.7, Math.sin(i * 0.3) * 0.7, 0.2],
        green = i % 7 === 0 ? 0 : (i % 5) / 4,
        sx = i % 3 === 0 ? -1 : 1.25,
        sy = i % 4 === 0 ? -1.5 : 1;
      const matrix = [
          sx,
          0,
          0,
          0,
          0.12,
          sy,
          0,
          0,
          0,
          0.1,
          0.8,
          0,
          0,
          0,
          i % 2 === 0 ? 0.0025 : 0,
          1,
        ],
        parameters = [0.5, 0.2, i % 11 === 0 ? 0 : 0.005, i % 13 === 0 ? 0 : 1];
      source.set([...point, green, ...matrix, ...parameters], i * 24);
      records.push({ point, green, matrix, parameters });
    }
    const input = create(source.byteLength, GPUBufferUsage.STORAGE),
      output = create(
        count * 64,
        GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      ),
      readback = create(
        count * 64,
        GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
      );
    device.queue.writeBuffer(input, 0, source);
    const groups = [
      device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 16, resource: { buffer: surfaceSkyZero } },
        ],
      }),
      device.createBindGroup({
        layout: pipeline.getBindGroupLayout(1),
        entries: [
          { binding: 3, resource: flutterSampler },
          { binding: 4, resource: { buffer: material } },
          {
            binding: 6,
            resource: flutterColor.createView({ dimension: "2d-array" }),
          },
        ],
      }),
      device.createBindGroup({
        layout: pipeline.getBindGroupLayout(2),
        entries: [
          { binding: 0, resource: { buffer: input } },
          { binding: 1, resource: { buffer: output } },
        ],
      }),
    ];
    let maxError = 0,
      pinnedError = 0,
      originalError = 0,
      normalTimeError = 0,
      previousNormals;
    const cases = [],
      replay = new Map();
    const linear = (v) =>
      v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    const additional = [0.2, 0.4, 0.6].map(linear);
    const runs = [
      ...[0, 2.5, 3].map((time) => ({ time, intensity: undefined })),
      ...[0, 3, 0, MAX_WORLD_ANIMATION_TIME].map((time) => ({
        time,
        intensity: 0.15,
      })),
      { time: 3, intensity: 0 },
    ];
    for (const { time, intensity } of runs) {
      const flutter =
        intensity === undefined
          ? undefined
          : {
              profile: "source-triangle-flutter-v1",
              intensity,
              weight: 0.3,
              clockScale: WIND_FLUTTER_CLOCK_SCALE,
              smallWaveCm: 1,
              largeWaveCm: 200,
              vertexLod: 0,
            };
      // Isolate the new one-year phase from unchanged legacy sway's float clock.
      const swayTime = time > 86400 ? 0 : time;
      const u = new Float32Array(FRAME_FLOAT_COUNT);
      for (const offset of [0, 32])
        u.set([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], offset);
      u[51] = swayTime;
      u[FRAME_OFFSETS.passSettings + 3] = boundedFlutterTime(time);
      const materialData = new Float32Array(4096);
      if (flutter) {
        materialData.set(
          [1, flutter.intensity, flutter.weight, flutter.clockScale],
          vegetationOffset + 40,
        );
        materialData.set([1, 200, 0, 0], vegetationOffset + 44);
      }
      device.queue.writeBuffer(material, 0, materialData);
      device.queue.writeBuffer(frame, 0, u);
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      groups.forEach((g, i) => pass.setBindGroup(i, g));
      pass.dispatchWorkgroups(count / 32);
      pass.end();
      encoder.copyBufferToBuffer(output, 0, readback, 0, readback.size);
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const values = new Float32Array(readback.getMappedRange()).slice();
      readback.unmap();
      const normals = [];
      for (let i = 0; i < count; i++) {
        const q = source.subarray(i * 24, i * 24 + 24),
          m = q.subarray(4, 20),
          p = q.subarray(0, 3),
          green = q[3],
          params = q.subarray(20, 24),
          world = [
            m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
            m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
            m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
          ],
          offset =
            params[3] === 0
              ? [0, 0, 0]
              : windSwayOffset(world, green, swayTime, {
                  enabled: true,
                  pivot: [...m.subarray(12, 15)],
                  worldUp: [...m.subarray(4, 7)],
                  windDirection: [0, 0, 1],
                  radiusMeters: params[3],
                  windSpeed: params[0],
                  swaySpeed: params[1],
                  swayAmount: params[2],
                });
        if (flutter && params[3] > 0) {
          const motion = windFlutterOffset(
            world,
            green,
            time,
            additional,
            flutter,
          );
          for (let axis = 0; axis < 3; axis++) offset[axis] += motion[axis];
          if (Math.hypot(...motion) > windFlutterBound(green, flutter) + 1e-7)
            throw Error("Flutter exceeds its conservative bound.");
        }
        for (let axis = 0; axis < 3; axis++) {
          const actual = values[i * 16 + axis],
            expected = world[axis] + offset[axis];
          if (!Number.isFinite(actual)) throw Error("Nonfinite vertex output");
          maxError = Math.max(
            maxError,
            Math.abs(actual - expected),
            Math.abs(values[i * 16 + 8 + axis] - expected),
          );
          originalError = Math.max(
            originalError,
            Math.abs(values[i * 16 + 4 + axis] - world[axis]),
          );
          if (!green || !params[3] || (!params[2] && !flutter))
            pinnedError = Math.max(
              pinnedError,
              Math.abs(actual - values[i * 16 + 4 + axis]),
            );
          normals.push(values[i * 16 + 12 + axis]);
        }
      }
      if (previousNormals)
        normals.forEach(
          (n, i) =>
            (normalTimeError = Math.max(
              normalTimeError,
              Math.abs(n - previousNormals[i]),
            )),
        );
      previousNormals = normals;
      if (time === 0 && flutter) {
        const bytes = new Uint8Array(values.buffer);
        const key = String(intensity);
        if (replay.has(key) && !bytes.every((n, i) => n === replay.get(key)[i]))
          throw Error("Reverse capture changed vertex output.");
        replay.set(key, bytes.slice());
      }
      cases.push({
        time,
        ordinarySwayTime: swayTime,
        flutterIntensity: intensity,
        vertices: count,
      });
    }
    await device.queue.onSubmittedWorkDone();
    for (const b of [frame, material, surfaceSkyZero, input, output, readback])
      b.destroy();
    flutterColor.destroy();
    device.destroy();
    return {
      cases,
      maxError,
      pinnedError,
      originalError,
      normalTimeError,
      shaderErrors,
      errors,
    };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.ok(result.maxError < 2e-6, JSON.stringify(result));
  assert.equal(result.pinnedError, 0);
  assert.ok(result.originalError < 2e-7);
  assert.equal(result.normalTimeError, 0);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
