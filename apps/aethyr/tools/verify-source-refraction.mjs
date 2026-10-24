// Original fixtures only. Run: openagents browser run -- node tools/verify-source-refraction.mjs
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
import { readFile } from "node:fs/promises";
const oracle = JSON.parse(
  await readFile(
    new URL("./reference/fixtures/ue58-owned-refraction.json", import.meta.url),
    "utf8",
  ),
);
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
    "<!doctype html><title>Owned source refraction qualification</title>",
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
  page.on("pageerror", (e) => pageErrors.push(e.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async (oracle) => {
    const {
      WGSL_SOURCE_WATER_STREAMS,
      SourceWaterStreams,
      SOURCE_WATER_STREAM_FRAME_FLOATS,
    } = await import("/packages/renderer/source-water-streams.ts");
    const {
      perspective,
      inverse,
      multiply,
      lookAt,
      orthographic: orthoMatrix,
    } = await import("/packages/renderer/math.ts");
    const { encodeRgba16fLE, halfToFloat } =
      await import("/packages/renderer/environment-lighting.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [],
      cases = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({
      code:
        WGSL_SOURCE_WATER_STREAMS +
        `
  struct Query{worldIOR:vec4<f32>,normalDepth:vec4<f32>,uv:vec4<f32>};
  @group(1)@binding(0)var<uniform>query:Query;
  @vertex fn qaVS(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let p=array<vec2<f32>,3>(vec2(-1,-1),vec2(3,-1),vec2(-1,3));return vec4(p[i],0,1);}
  @fragment fn qaFS()->@location(0)vec4<f32>{let uv=sourceScreenUV(query.worldIOR.xyz,query.normalDepth.xyz,query.worldIOR.w,query.uv.xy,query.normalDepth.w);return vec4(uv,0,1);}`,
    });
    const compilation = await module.getCompilationInfo();
    if (compilation.messages.some((x) => x.type === "error"))
      throw Error(JSON.stringify(compilation.messages));
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "qaVS" },
      fragment: {
        module,
        entryPoint: "qaFS",
        targets: [{ format: "rgba32float" }],
      },
      primitive: { topology: "triangle-list" },
    });
    const frame = device.createBuffer({
        size: SOURCE_WATER_STREAM_FRAME_FLOATS * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      query = device.createBuffer({
        size: 48,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
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
    const projectDepth = (matrix, z) =>
      (matrix[10] * z + matrix[14]) / (matrix[11] * z + matrix[15]);
    let maximumFormulaError = 0,
      maximumOraclePixelError = 0;
    async function run({
      width = 128,
      height = 72,
      ior = 2,
      normal = [0.5, 0.5, Math.SQRT1_2],
      gap = 1,
      uv = [0.5, 0.5],
      missing = false,
      orthographic = false,
      skew = false,
      fov = 60,
      rotated = false,
    } = {}) {
      const matrix = perspective(
        2 * Math.atan(Math.tan((fov * Math.PI) / 360) / (width / height)),
        width / height,
        0.1,
        100,
      );
      if (orthographic) matrix.set(orthoMatrix(-10, 10, -10, 10, 0.1, 100));
      if (skew) matrix[8] = 0.2;
      const depth = missing ? 1 : projectDepth(matrix, -9 - gap);
      if (rotated) matrix.set(multiply(matrix, lookAt([0, 0, 0], [1, 0, 0])));
      const inv = inverse(matrix),
        u = new Float32Array(SOURCE_WATER_STREAM_FRAME_FLOATS);
      u.set(matrix);
      u.set(inv, 16);
      u.set([0, 0, 0, 0], 32);
      u.set([width, height, 1, 0.01], 36);
      u.set(rotated ? [1, 0, 0, 0] : [0, 0, -1, 0], 40);
      device.queue.writeBuffer(frame, 0, u);
      device.queue.writeBuffer(
        query,
        0,
        new Float32Array([
          ...(rotated ? [9, 0, 0] : [0, 0, -9]),
          ior,
          ...normal,
          depth,
          ...uv,
          0,
          0,
        ]),
      );
      const depthTexture = device.createTexture({
        size: [width, height],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      const group0 = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 1, resource: depthTexture.createView() },
        ],
      });
      const group1 = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(1),
        entries: [{ binding: 0, resource: { buffer: query } }],
      });
      const encoder = device.createCommandEncoder(),
        clear = encoder.beginRenderPass({
          colorAttachments: [],
          depthStencilAttachment: {
            view: depthTexture.createView(),
            depthClearValue: depth,
            depthLoadOp: "clear",
            depthStoreOp: "store",
          },
        });
      clear.end();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: output.createView(),
            clearValue: [0, 0, 0, 0],
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group0);
      pass.setBindGroup(1, group1);
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: output },
        { buffer: readback, bytesPerRow: 256 },
        [1, 1],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const value = [
        ...new Float32Array(readback.getMappedRange()).slice(0, 2),
      ];
      readback.unmap();
      depthTexture.destroy();
      return value;
    }
    function close(actual, expected, label, tolerance = 2e-5) {
      const error = Math.max(
        ...actual.map((v, i) => Math.abs(v - expected[i])),
      );
      maximumFormulaError = Math.max(maximumFormulaError, error);
      if (!actual.every(Number.isFinite) || error > tolerance)
        throw Error(`${label}: ${actual} != ${expected}`);
      cases.push(label);
    }
    for (const width of [128, 256])
      for (const gap of [1, 0.005, 0.01])
        for (const [polarity, ior] of [
          [1, 1],
          [1, 1.5],
          [1, 2],
          [-1, 2],
        ]) {
          const height = (width * 9) / 16,
            normal = [0.5, 0.5, Math.SQRT1_2].map((v) => v * polarity),
            scale =
              (width / Math.tan(Math.PI / 6)) *
              0.00023 *
              (ior - 1) *
              Math.min(1, Math.max(0, gap / 0.01));
          const actual = await run({ width, height, gap, ior, normal });
          close(
            actual,
            [0.5 + normal[0] * scale, 0.5 - normal[1] * scale],
            `signed projection ${width} gap${gap} ior${ior} sign${polarity}`,
            gap <= 0.01 ? 3e-4 : 2e-5,
          );
          const source = oracle.cases.find(
            (c) =>
              c.dimensions[0] === width &&
              c.backgroundSeparationCentimeters === gap * 100 &&
              c.ior === ior &&
              Math.sign(c.worldNormal[0]) === polarity,
          );
          const base = oracle.cases.find(
            (c) =>
              c.dimensions[0] === width &&
              c.backgroundSeparationCentimeters === gap * 100 &&
              c.ior === 1,
          );
          const slopes = [
            ((base.rgba[2][0] - base.rgba[1][0]) * width) / 16,
            ((base.rgba[4][1] - base.rgba[3][1]) * height) / 16,
          ];
          for (let axis = 0; axis < 2; axis++) {
            const measured =
              (source.rgba[0][axis] - base.rgba[0][axis]) / slopes[axis];
            const error = Math.abs(
              (actual[axis] - 0.5 - measured) * [width, height][axis],
            );
            maximumOraclePixelError = Math.max(maximumOraclePixelError, error);
            if (error > 0.75)
              throw Error(`Unreal displacement mismatch ${error} pixels`);
          }
        }
    for (const [label, options] of [
      ["missing depth", { missing: true }],
      ["opaque foreground", { gap: -1 }],
      ["offscreen", { uv: [0.99, 0.5] }],
      ["orthographic", { orthographic: true }],
      ["off-axis projection", { skew: true }],
    ])
      close(await run(options), options.uv ?? [0.5, 0.5], label);
    for (const fov of [45, 90]) {
      const delta = (128 / Math.tan((fov * Math.PI) / 360)) * 0.00023 * 0.5;
      close(
        await run({ fov }),
        [0.5 + delta, 0.5 - delta],
        `perspective projection scale FOV${fov}`,
      );
    }
    const rotatedDelta = (128 / Math.tan(Math.PI / 6)) * 0.00023 * 0.5;
    close(
      await run({ rotated: true, normal: [Math.SQRT1_2, 0.5, 0.5] }),
      [0.5 + rotatedDelta, 0.5 - rotatedDelta],
      "world normal transformed through rotated view",
    );
    const hash = async (bytes) =>
      [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
        .map((v) => v.toString(16).padStart(2, "0"))
        .join("");
    // Qualify the actual nearest-interface pass and its correlated coverage composite.
    const width = 128,
      height = 72,
      depth = projectDepth(
        perspective(
          2 * Math.atan(Math.tan(Math.PI / 6) / (width / height)),
          width / height,
          0.1,
          100,
        ),
        -10,
      );
    const matrix = perspective(
        2 * Math.atan(Math.tan(Math.PI / 6) / (width / height)),
        width / height,
        0.1,
        100,
      ),
      inv = inverse(matrix),
      identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const vertex = new Float32Array(
      [
        [-100, -100],
        [100, -100],
        [-100, 100],
        [100, 100],
      ].flatMap(([x, y]) => [x, y, -9, 0, 0, 1, 0.25, 0.25, 1, 0, 0, 1]),
    ).buffer;
    const indices = new Uint32Array([0, 1, 2, 1, 3, 2]).buffer;
    function rgba8(value) {
      const b = new ArrayBuffer(20),
        d = new DataView(b);
      d.setUint32(0, 0x41465238, false);
      d.setUint32(4, 1, true);
      d.setUint32(8, 1, true);
      d.setUint32(12, 1, true);
      new Uint8Array(b, 16).set(value);
      return b;
    }
    const noise = rgba8([255, 0, 0, 255]),
      normal = rgba8([255, 128, 255, 64]),
      blobs = new Map([
        ["vertices", vertex],
        ["indices", indices],
        ["noise", noise],
        ["normal", normal],
      ]);
    const descriptor = async (name) => ({
      url: `/private-assets/${name}.bin`,
      sha256: await hash(blobs.get(name)),
      byteLength: blobs.get(name).byteLength,
    });
    const recipe = {
      schema: "aethyrforge.source-water-streams.v1",
      vertices: await descriptor("vertices"),
      indices: await descriptor("indices"),
      vertexStride: 48,
      vertexCount: 4,
      indexCount: 6,
      meshes: [
        { id: "owned", firstIndex: 0, indexCount: 6, center: [0, 0, -9] },
      ],
      noise: {
        ...(await descriptor("noise")),
        width: 1,
        height: 1,
        colorSpace: "linear",
      },
      normal: {
        ...(await descriptor("normal")),
        width: 1,
        height: 1,
        colorSpace: "linear",
        encoding: "unreal-normal-red-splat",
        flipGreenChannel: false,
        coverage: "bilinear-positive-area-8x8-v1",
      },
      material: {
        flow: [0, 0],
        powerExponent: 0.3,
        fresnelExponent: 1,
        baseReflectFraction: 0.04,
        depthFadeMeters: 0.25,
        refractionMinimum: 1,
        refractionMode: "source-screen-displacement-v1",
      },
      scenarios: {
        day: [
          {
            id: "owned",
            meshId: "owned",
            model: identity,
            parameters: [2, 0.35, 0, 0],
          },
        ],
        night: [],
      },
    };
    const effect = await SourceWaterStreams.create(
      device,
      recipe,
      async (url) => blobs.get(url.split("/").at(-1).slice(0, -4)),
    );
    const color = device.createTexture({
      size: [width, height],
      format: "rgba16float",
      sampleCount: 4,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    const resolved = device.createTexture({
      size: [width, height],
      format: "rgba16float",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const source = device.createTexture({
      size: [width, height],
      format: "rgba16float",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    const depths = device.createTexture({
      size: [width, height],
      format: "depth24plus",
      sampleCount: 4,
      usage:
        GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });
    const pixels = new Float32Array(width * height * 4);
    for (let y = 0; y < height; y++)
      for (let x = 0; x < width; x++)
        pixels.set(
          [(x + 0.5) / width, (y + 0.5) / height, 0.2, 1],
          (y * width + x) * 4,
        );
    device.queue.writeTexture(
      { texture: source },
      encodeRgba16fLE(pixels),
      { bytesPerRow: width * 8 },
      [width, height],
    );
    const buffer = device.createBuffer({
      size: width * 8 * height,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const encoder = device.createCommandEncoder(),
      clear = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: color.createView(),
            loadOp: "clear",
            storeOp: "store",
            clearValue: [0, 0, 0, 1],
          },
        ],
        depthStencilAttachment: {
          view: depths.createView(),
          depthClearValue: depth,
          depthLoadOp: "clear",
          depthStoreOp: "store",
        },
      });
    clear.end();
    effect.render(encoder, {
      mode: "day",
      time: 0,
      width,
      height,
      colorView: color.createView(),
      refraction: { source, resolve: resolved },
      depthView: depths.createView(),
      viewProjection: matrix,
      inverseViewProjection: inv,
      eye: [0, 0, 0],
      effectiveExposure: 1,
    });
    encoder.copyTextureToBuffer(
      { texture: resolved },
      { buffer, bytesPerRow: width * 8 },
      [width, height],
    );
    device.queue.submit([encoder.finish()]);
    await buffer.mapAsync(GPUMapMode.READ);
    const colors = new Uint16Array(buffer.getMappedRange()),
      x = 64,
      y = 36,
      actual = [
        ...colors.slice((y * width + x) * 4, (y * width + x) * 4 + 3),
      ].map(halfToFloat);
    buffer.unmap();
    const uv = [(x + 0.5) / width, (y + 0.5) / height],
      point = [
        ((uv[0] * 2 - 1) * 9) / matrix[0],
        ((1 - uv[1] * 2) * 9) / matrix[5],
        -9,
      ],
      eye = point.map((v) => -v / Math.hypot(...point)),
      n = [1, -1, 1].map((v) => v / Math.sqrt(3));
    const cosine = n.reduce((s, v, i) => s + v * eye[i], 0),
      f = (c) => 0.04 + 0.96 * (1 - Math.max(0, Math.min(1, c))),
      aPos = 0.35 * f(cosine),
      aNeg = 0.35 * f(-cosine),
      delta = width * matrix[0] * 0.00023,
      coverage = 64 / 255;
    const posUV = [uv[0] + n[0] * delta, uv[1] - n[1] * delta],
      negUV = [uv[0] - n[0] * delta, uv[1] + n[1] * delta];
    const expected = [0, 1, 2].map((i) => {
      const positive = (i < 2 ? posUV[i] : 0.2) * (1 - aPos) + 0.01 * aPos,
        negative = (i < 2 ? negUV[i] : 0.2) * (1 - aNeg) + 0.01 * aNeg;
      return negative * (1 - coverage) + positive * coverage;
    });
    close(
      actual,
      expected,
      "actual SourceWaterStreams.render coverage averages full signed composites",
      0.0012,
    );
    const wrong = [0, 1].map((i) => {
      const bg = negUV[i] * (1 - coverage) + posUV[i] * coverage,
        opacity = aNeg * (1 - coverage) + aPos * coverage;
      return bg * (1 - opacity) + 0.01 * opacity;
    });
    if (
      Math.max(...expected.slice(0, 2).map((v, i) => Math.abs(v - wrong[i]))) <
      0.001
    )
      throw Error(
        "Coverage fixture does not distinguish the uncorrelated wrong model",
      );
    effect.dispose();
    for (const resource of [
      color,
      resolved,
      source,
      depths,
      buffer,
      frame,
      query,
      output,
      readback,
    ])
      resource.destroy();
    await device.queue.onSubmittedWorkDone();
    if (errors.length) throw Error(errors.join("\n"));
    device.destroy();
    return { cases, maximumFormulaError, maximumOraclePixelError, errors };
  }, oracle);
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify(result, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
