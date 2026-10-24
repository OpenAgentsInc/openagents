// Owned full-material normal qualification; no vendor data or account access.
// Run: openagents browser run -- node tools/verify-normals.mjs
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
  res.end("<!doctype html><title>Owned normal-frame qualification</title>");
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext();
try {
  const page = await context.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { MATERIAL } = await import("/packages/renderer/shaders.ts");
    const { FRAME_FLOAT_COUNT } = await import("/packages/renderer/frame.ts");
    const { LAYER_FLOAT_COUNT, packLayers } =
      await import("/packages/renderer/layers.ts");
    const { GLASS_FLOAT_COUNT } = await import("/packages/renderer/glass.ts");
    const { PAINT_FLOAT_COUNT } = await import("/packages/renderer/paint.ts");
    const { TERRAIN_FLOAT_COUNT } =
      await import("/packages/renderer/terrain.ts");
    const { VEGETATION_FLOAT_COUNT, packVegetation } =
      await import("/packages/renderer/vegetation.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    // Keep the production vertex, sample, profile and normal paths; replace lighting only.
    const end = MATERIAL.indexOf(" if frame.passSettings.y>0.5");
    if (end < 0)
      throw Error("Material normal diagnostic insertion point changed");
    const module = device.createShaderModule({
      code:
        MATERIAL.slice(0, end).replace(
          "  base=max(vegetation.base",
          "  if frame.extra.z>0.5{return vec4(vec3(vegetation.fuzz),1.0);}\n  base=max(vegetation.base",
        ) +
        `
 if clipped||(material.uv.w<0.5&&!physicalFront){discard;}
 return vec4(n*0.5+0.5,1.0);}
`,
    });
    const info = await module.getCompilationInfo();
    const shaderErrors = info.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}:${m.linePos}: ${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const buffer = (size, usage) =>
      device.createBuffer({ size, usage: usage | GPUBufferUsage.COPY_DST });
    const frame = buffer(FRAME_FLOAT_COUNT * 4, GPUBufferUsage.UNIFORM);
    const surfaceSkyZero = buffer(16, GPUBufferUsage.STORAGE);
    const material = buffer(
      (20 +
        LAYER_FLOAT_COUNT +
        GLASS_FLOAT_COUNT +
        PAINT_FLOAT_COUNT +
        TERRAIN_FLOAT_COUNT +
        VEGETATION_FLOAT_COUNT) *
        4,
      GPUBufferUsage.UNIFORM,
    );
    const vertex = buffer(3 * 48, GPUBufferUsage.VERTEX);
    const model = buffer(64, GPUBufferUsage.VERTEX);
    const color = buffer(12, GPUBufferUsage.VERTEX);
    const uv1 = buffer(24, GPUBufferUsage.VERTEX);
    const wind = buffer(16, GPUBufferUsage.VERTEX);
    device.queue.writeBuffer(color, 0, new Uint8Array(12).fill(255));
    device.queue.writeBuffer(uv1, 0, new Float32Array(6));
    const usage = GPUTextureUsage;
    const white = device.createTexture({
      size: [1, 1],
      format: "rgba8unorm",
      usage: usage.TEXTURE_BINDING | usage.COPY_DST,
    });
    const normal = device.createTexture({
      size: [1, 1],
      format: "rgba8unorm",
      usage: usage.TEXTURE_BINDING | usage.COPY_DST,
    });
    device.queue.writeTexture(
      { texture: white },
      new Uint8Array([255, 255, 255, 255]),
      { bytesPerRow: 4 },
      [1, 1],
    );
    const normalArray = device.createTexture({
      size: [1, 1, 3],
      format: "rgba8unorm",
      usage: usage.TEXTURE_BINDING | usage.COPY_DST,
    });
    const layerSamples = [
      [204, 128, 0],
      [128, 230, 255],
      [128, 128, 0],
    ];
    device.queue.writeTexture(
      { texture: normalArray },
      new Uint8Array(layerSamples.flatMap((v) => [...v, 255])),
      { bytesPerRow: 4, rowsPerImage: 1 },
      [1, 1, 3],
    );
    const sampler = device.createSampler();
    const vertexBuffers = [
      {
        arrayStride: 48,
        attributes: [
          { shaderLocation: 0, offset: 0, format: "float32x3" },
          { shaderLocation: 1, offset: 12, format: "float32x3" },
          { shaderLocation: 2, offset: 24, format: "float32x2" },
          { shaderLocation: 3, offset: 32, format: "float32x4" },
        ],
      },
      {
        arrayStride: 64,
        stepMode: "instance",
        attributes: [0, 1, 2, 3].map((i) => ({
          shaderLocation: 4 + i,
          offset: i * 16,
          format: "float32x4",
        })),
      },
      {
        arrayStride: 4,
        attributes: [{ shaderLocation: 8, offset: 0, format: "unorm8x4" }],
      },
      {
        arrayStride: 8,
        attributes: [{ shaderLocation: 9, offset: 0, format: "float32x2" }],
      },
      {
        arrayStride: 16,
        stepMode: "instance",
        attributes: [{ shaderLocation: 10, offset: 0, format: "float32x4" }],
      },
    ];
    const pipelines = await Promise.all(
      [false, true].map((reflection) =>
        device.createRenderPipelineAsync({
          layout: "auto",
          vertex: { module, entryPoint: "vs", buffers: vertexBuffers },
          fragment: {
            module,
            entryPoint: "fs",
            targets: [{ format: "rgba32float" }],
          },
          primitive: { frontFace: reflection ? "cw" : "ccw", cullMode: "none" },
        }),
      ),
    );
    const groups = pipelines.map((pipeline) => [
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
          ...[0, 2, 5].map((binding) => ({
            binding,
            resource: white.createView(),
          })),
          { binding: 1, resource: normal.createView() },
          { binding: 3, resource: sampler },
          { binding: 4, resource: { buffer: material } },
          ...[6, 7, 8].map((binding) => ({
            binding,
            resource: (binding === 7 ? normalArray : white).createView({
              dimension: "2d-array",
            }),
          })),
        ],
      }),
    ]);
    const target = device.createTexture({
      size: [8, 8],
      format: "rgba32float",
      usage: usage.RENDER_ATTACHMENT | usage.COPY_SRC,
    });
    const targetView = target.createView();
    const read = device.createBuffer({
      size: 2048,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    const normalize = (v) => {
      const length = Math.hypot(...v);
      return length > 1e-15 ? v.map((x) => x / length) : [0, 0, 1];
    };
    const cross = (a, b) => [
      a[1] * b[2] - a[2] * b[1],
      a[2] * b[0] - a[0] * b[2],
      a[0] * b[1] - a[1] * b[0],
    ];
    const dot = (a, b) => a.reduce((sum, x, i) => sum + x * b[i], 0);
    const identity = () =>
      new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    let cases = 0,
      maxError = 0;
    const charts = [
      "uv",
      "mirror-u",
      "mirror-v",
      "rotate",
      "skew",
      "degenerate",
    ];
    const samples = [
      [128, 128, 255],
      [255, 128, 255],
      [128, 255, 255],
      [215, 175, 242],
    ];
    for (const sourceProfile of [false, true])
      for (const slanted of [false, true])
        for (const negative of [false, true])
          for (const reflection of [false, true])
            for (const back of [false, true])
              for (const directX of [false, true])
                for (const positiveZ of [false, true])
                  for (const chart of charts)
                    for (const strength of [0, 1])
                      for (const sample of samples) {
                        const scale = [negative ? -2 : 2, 3, 0.5];
                        const u = slanted ? [0.8, 0, 0.6] : [1, 0, 0],
                          v = [0, 1, 0];
                        const sourceN = cross(u, v);
                        let n = normalize(sourceN.map((x, i) => x / scale[i]));
                        const worldU = normalize(u.map((x, i) => x * scale[i]));
                        const worldV = [0, 1, 0];
                        let t = worldU,
                          b = worldV;
                        if (chart === "mirror-u") t = t.map((x) => -x);
                        if (chart === "mirror-v") b = b.map((x) => -x);
                        if (chart === "rotate") {
                          t = worldV;
                          b = worldU.map((x) => -x);
                        }
                        let localT = u,
                          localB = v;
                        if (chart === "mirror-u") localT = u.map((x) => -x);
                        if (chart === "mirror-v") localB = v.map((x) => -x);
                        if (chart === "rotate") {
                          localT = v;
                          localB = u.map((x) => -x);
                        }
                        const nativeHand =
                          dot(cross(sourceN, localT), localB) < 0 ? -1 : 1;
                        if (sourceProfile) {
                          n = sourceN.map((x, i) => x * Math.sign(scale[i]));
                          t = localT.map((x, i) => x * Math.sign(scale[i]));
                          b = localB.map((x, i) => x * Math.sign(scale[i]));
                        }
                        const raw = sample.map((x) => (x / 255) * 2 - 1);
                        if (positiveZ)
                          raw[2] = Math.sqrt(
                            Math.max(1 - raw[0] ** 2 - raw[1] ** 2, 0),
                          );
                        const mixed = n.map(
                          (x, i) =>
                            x * raw[2] +
                            strength *
                              (t[i] * raw[0] +
                                b[i] * raw[1] * (directX ? -1 : 1)),
                        );
                        const expected = (
                          (!sourceProfile && chart === "degenerate") ||
                          Math.hypot(...mixed) < 1e-15
                            ? n
                            : normalize(mixed)
                        ).map((x) => x * (back ? -1 : 1));
                        const vertices = [];
                        for (const [a, c] of [
                          [-0.8, -0.8],
                          [0.8, -0.8],
                          [0, 0.9],
                        ]) {
                          let uv = [a, c];
                          if (chart === "mirror-u") uv = [-a, c];
                          if (chart === "mirror-v") uv = [a, -c];
                          if (chart === "rotate") uv = [c, -a];
                          if (chart === "skew") uv = [a + 0.3 * c, c];
                          if (chart === "degenerate") uv = [0, 0];
                          vertices.push(
                            ...u.map((x, i) => x * a + v[i] * c),
                            ...sourceN,
                            ...uv,
                            ...(sourceProfile
                              ? [...localT, nativeHand]
                              : [1, 0, 0, 1]),
                          );
                        }
                        const m = identity();
                        m[0] = scale[0];
                        m[5] = scale[1];
                        m[10] = scale[2];
                        device.queue.writeBuffer(model, 0, m);
                        device.queue.writeBuffer(
                          vertex,
                          0,
                          new Float32Array(vertices),
                        );
                        const f = new Float32Array(FRAME_FLOAT_COUNT),
                          vp = identity();
                        vp[0] = 0.5 * (back ? -1 : 1) * (reflection ? -1 : 1);
                        vp[5] = 1 / 3;
                        vp[10] = 0;
                        vp[14] = 0.5;
                        f.set(vp);
                        f[50] = back ? -10 : 10;
                        f[79] = strength;
                        device.queue.writeBuffer(frame, 0, f);
                        const mat = new Float32Array(material.size / 4);
                        mat.set([1, 1, 1, 1]);
                        mat[10] = 1;
                        mat.set([1, 1, 0.5, 1], 12);
                        mat[16] = 1;
                        mat[19] =
                          Number(directX) |
                          (Number(positiveZ) << 1) |
                          (Number(sourceProfile) << 2);
                        device.queue.writeBuffer(material, 0, mat);
                        device.queue.writeTexture(
                          { texture: normal },
                          new Uint8Array([...sample, 255]),
                          { bytesPerRow: 4 },
                          [1, 1],
                        );
                        const encoder = device.createCommandEncoder();
                        const pass = encoder.beginRenderPass({
                          colorAttachments: [
                            {
                              view: targetView,
                              loadOp: "clear",
                              storeOp: "store",
                              clearValue: [0, 0, 0, 0],
                            },
                          ],
                        });
                        const index = Number(reflection);
                        pass.setPipeline(pipelines[index]);
                        groups[index].forEach((g, i) =>
                          pass.setBindGroup(i, g),
                        );
                        [vertex, model, color, uv1, wind].forEach((buffer, i) =>
                          pass.setVertexBuffer(i, buffer),
                        );
                        pass.draw(3);
                        pass.end();
                        encoder.copyTextureToBuffer(
                          { texture: target },
                          { buffer: read, bytesPerRow: 256 },
                          [8, 8],
                        );
                        device.queue.submit([encoder.finish()]);
                        await read.mapAsync(GPUMapMode.READ);
                        const pixel = Array.from(
                          new Float32Array(read.getMappedRange()).slice(
                            4 * 64 + 4 * 4,
                            4 * 64 + 4 * 4 + 4,
                          ),
                        );
                        read.unmap();
                        const observed = pixel
                          .slice(0, 3)
                          .map((x) => x * 2 - 1);
                        const error = Math.max(
                          ...expected.map((x, i) => Math.abs(x - observed[i])),
                        );
                        maxError = Math.max(maxError, error);
                        if (
                          pixel[3] !== 1 ||
                          error > 0.00002 ||
                          !observed.every(Number.isFinite)
                        )
                          throw Error(
                            JSON.stringify({
                              sourceProfile,
                              slanted,
                              negative,
                              reflection,
                              back,
                              directX,
                              positiveZ,
                              chart,
                              strength,
                              sample,
                              observed,
                              expected,
                              error,
                              n,
                              t,
                              b,
                              dotTB: dot(t, b),
                            }),
                          );
                        cases++;
                      }
    // This witness executes the production two-layer profile. Opposing XY values
    // distinguish per-texture Z reconstruction from reconstructing after blending.
    let layeredCases = 0;
    const layerFixture = {
      layers: {
        model: "ue-mm-blend-2-v1",
        slots: [
          {
            baseColorUrl: "white",
            normalUrl: "n0",
            mraUrl: "white",
            colorFactor: [1, 1, 1, 1],
          },
          {
            baseColorUrl: "white",
            normalUrl: "n1",
            mraUrl: "white",
            colorFactor: [1, 1, 1, 1],
          },
        ],
        uv: { mode: "mesh", scale: [1, 1] },
        blend: {
          vertexChannel: "r",
          phase: 0.75,
          contrast: 0,
          height: {
            mode: "layer2Alpha",
            customIntensity: 1,
            sourceColorSpace: "linear",
          },
          worldSubtract: {
            url: "white",
            sourceColorSpace: "linear",
            sizeMeters: 8,
            intensity: 0,
            power: 1,
            channel: "r",
            projection: "ue-world-aligned-v1",
          },
        },
        detailNormal: { url: "detail", uvScale: [1, 1], flatness: 1 },
        colorIntensity: 1,
        source: { master: "owned", graphSha256: "owned", parameters: {} },
      },
    };
    const layerPayload = packLayers(layerFixture, (url, role) =>
      role === "normal" ? { n0: 0, n1: 1, detail: 2 }[url] : 0,
    );
    const plane = [];
    for (const [x, y] of [
      [-0.8, -0.8],
      [0.8, -0.8],
      [0, 0.9],
    ])
      plane.push(x, y, 0, 0, 0, 1, x, y, 1, 0, 0, 1);
    device.queue.writeBuffer(vertex, 0, new Float32Array(plane));
    device.queue.writeBuffer(model, 0, identity());
    const lf = new Float32Array(FRAME_FLOAT_COUNT),
      lvp = identity();
    lvp[10] = 0;
    lvp[14] = 0.5;
    lf.set(lvp);
    lf[50] = 10;
    lf[79] = 1;
    device.queue.writeBuffer(frame, 0, lf);
    for (const flags of [0, 1, 2, 3]) {
      const mat = new Float32Array(material.size / 4);
      mat.set([1, 1, 1, 1]);
      mat[10] = 1;
      mat.set([1, 1, 0.5, 1], 12);
      mat[16] = 1;
      mat[19] = flags;
      mat.set(layerPayload, 20);
      device.queue.writeBuffer(material, 0, mat);
      const decoded = layerSamples.slice(0, 2).map((v) => {
        const n = v.map((x) => (x / 255) * 2 - 1);
        if (flags & 2) n[2] = Math.sqrt(Math.max(1 - n[0] ** 2 - n[1] ** 2, 0));
        return n;
      });
      const mixed = decoded[0].map((x, i) => (x + decoded[1][i]) * 0.5);
      if (flags & 1) mixed[1] *= -1;
      const expected = normalize(mixed);
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: targetView,
              loadOp: "clear",
              storeOp: "store",
              clearValue: [0, 0, 0, 0],
            },
          ],
        });
      pass.setPipeline(pipelines[0]);
      groups[0].forEach((g, i) => pass.setBindGroup(i, g));
      [vertex, model, color, uv1, wind].forEach((b, i) =>
        pass.setVertexBuffer(i, b),
      );
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: target },
        { buffer: read, bytesPerRow: 256 },
        [8, 8],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const pixel = Array.from(
        new Float32Array(read.getMappedRange()).slice(
          4 * 64 + 4 * 4,
          4 * 64 + 4 * 4 + 4,
        ),
      );
      read.unmap();
      const observed = pixel.slice(0, 3).map((x) => x * 2 - 1);
      const error = Math.max(
        ...expected.map((x, i) => Math.abs(x - observed[i])),
      );
      maxError = Math.max(maxError, error);
      if (pixel[3] !== 1 || error > 2e-5 || !observed.every(Number.isFinite))
        throw Error(
          JSON.stringify({ layered: true, flags, expected, observed, error }),
        );
      layeredCases++;
    }
    // Native foliage uses one recovered frame for both mapped normal and fuzz.
    // Rotate its tangent 90 degrees away from the chart so a gradient-view fallback fails.
    const vegetationFixture = {
      vegetation: {
        model: "ue-mm-vegetation-1-v1",
        baseColorUrl: "white",
        normalUrl: "n0",
        mraUrl: "white",
        mask: {
          url: "white",
          sourceColorSpace: "linear",
          enabled: false,
          color: [1, 1, 1],
        },
        tint: [1, 1, 1],
        color: {
          luminance: [0.2, 0.7, 0.1],
          desaturation: 0,
          contrast: 1,
          intensity: 1,
          overlayColor: [0, 0, 0],
          overlayStrength: 0,
        },
        gradient: { axis: "v", intensity: 0 },
        aoAffectsColorIntensity: 1,
        normalFlatness: 0,
        roughnessIntensity: 1,
        specularAmount: 0.3,
        fuzz: { coreDarkness: 0.3, power: 3, edgeBrightness: 0.2 },
        subsurface: { color: [1, 1, 1], intensity: 0 },
        alphaCutoff: 0.3,
        twoSided: true,
        lightingModel: "two-sided-foliage",
        sourceWindEnabled: false,
        source: { master: "owned", graphSha256: "owned", parameters: {} },
      },
    };
    const vegetationPayload = packVegetation(vegetationFixture, () => 0);
    const vegetationOffset =
      20 +
      LAYER_FLOAT_COUNT +
      GLASS_FLOAT_COUNT +
      PAINT_FLOAT_COUNT +
      TERRAIN_FLOAT_COUNT;
    let vegetationCases = 0;
    for (const native of [false, true])
      for (const negative of [false, true])
        for (const reflection of [false, true])
          for (const back of [false, true])
            for (const outputFuzz of [false, true])
              for (const shortened of [false, true]) {
                const scale = [negative ? -2 : 2, 3, 0.5],
                  vp = identity();
                vp[0] = 0.5 * (back ? -1 : 1) * (reflection ? -1 : 1);
                vp[5] = 1 / 3;
                vp[10] = 0;
                vp[14] = 0.5;
                const modelValue = identity();
                modelValue[0] = scale[0];
                modelValue[5] = 3;
                modelValue[10] = 0.5;
                const vertices = [];
                for (const [x, y] of [
                  [-0.8, -0.8],
                  [0.8, -0.8],
                  [0, 0.9],
                ])
                  vertices.push(
                    x,
                    y,
                    0,
                    0,
                    0,
                    shortened ? 0.8 : 1,
                    x,
                    y,
                    0,
                    shortened ? 0.7 : 1,
                    shortened ? 0.01 : 0,
                    1,
                  );
                device.queue.writeBuffer(vertex, 0, new Float32Array(vertices));
                device.queue.writeBuffer(model, 0, modelValue);
                const f = new Float32Array(FRAME_FLOAT_COUNT);
                f.set(vp);
                f[48] = 3;
                f[49] = 2;
                f[50] = back ? -10 : 10;
                f[78] = Number(outputFuzz);
                f[79] = 1;
                device.queue.writeBuffer(frame, 0, f);
                const mat = new Float32Array(material.size / 4);
                mat.set([1, 1, 1, 1]);
                mat[10] = 1;
                mat.set([1, 1, 0.3, 1], 12);
                mat[16] = 1;
                mat[19] = 2 + Number(native) * 4;
                mat.set(vegetationPayload, vegetationOffset);
                device.queue.writeBuffer(material, 0, mat);
                const sign = negative ? -1 : 1,
                  n = [0, 0, native && shortened ? 0.8 : 1],
                  t = native
                    ? [0, shortened ? 0.7 : 1, shortened ? 0.01 : 0]
                    : [sign, 0, 0],
                  b = native
                    ? [-sign * (shortened ? 0.56 : 1), 0, 0]
                    : [0, 1, 0];
                const raw = layerSamples[0].map((x) => (x / 255) * 2 - 1);
                raw[2] = Math.sqrt(Math.max(1 - raw[0] ** 2 - raw[1] ** 2, 0));
                const mapped = normalize(
                  n.map((x, i) => x * raw[2] + t[i] * raw[0] + b[i] * raw[1]),
                ).map((x) => x * (back ? -1 : 1));
                const world = [0.125 / vp[0], -0.125 / vp[5], 0];
                const view = normalize([
                  3 - world[0],
                  2 - world[1],
                  back ? -10 : 10,
                ]);
                const projectedView = [
                  dot(view, t),
                  dot(view, b),
                  dot(view, n),
                ];
                const tangentView = native
                  ? projectedView
                  : normalize(projectedView);
                const ndv = Math.max(
                  0,
                  Math.min(
                    1,
                    dot(tangentView, [
                      raw[0],
                      raw[1],
                      raw[2] * (back ? -1 : 1),
                    ]),
                  ),
                );
                const fuzz = 1 - ndv * 0.3 + (1 - ndv) ** 3 * 0.2;
                const expected = outputFuzz
                  ? [fuzz, fuzz, fuzz]
                  : mapped.map((x) => x * 0.5 + 0.5);
                const encoder = device.createCommandEncoder(),
                  pass = encoder.beginRenderPass({
                    colorAttachments: [
                      {
                        view: targetView,
                        loadOp: "clear",
                        storeOp: "store",
                        clearValue: [0, 0, 0, 0],
                      },
                    ],
                  });
                pass.setPipeline(pipelines[Number(reflection)]);
                groups[Number(reflection)].forEach((g, i) =>
                  pass.setBindGroup(i, g),
                );
                [vertex, model, color, uv1, wind].forEach((buffer, i) =>
                  pass.setVertexBuffer(i, buffer),
                );
                pass.draw(3);
                pass.end();
                encoder.copyTextureToBuffer(
                  { texture: target },
                  { buffer: read, bytesPerRow: 256 },
                  [8, 8],
                );
                device.queue.submit([encoder.finish()]);
                await read.mapAsync(GPUMapMode.READ);
                const pixel = Array.from(
                  new Float32Array(read.getMappedRange()).slice(
                    4 * 64 + 4 * 4,
                    4 * 64 + 4 * 4 + 4,
                  ),
                );
                read.unmap();
                const error = Math.max(
                  ...expected.map((x, i) => Math.abs(x - pixel[i])),
                );
                maxError = Math.max(maxError, error);
                if (pixel[3] !== 1 || error > 2e-5)
                  throw Error(
                    JSON.stringify({
                      native,
                      negative,
                      reflection,
                      back,
                      outputFuzz,
                      shortened,
                      expected,
                      pixel,
                      error,
                    }),
                  );
                vegetationCases++;
              }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return {
      cases,
      layeredCases,
      vegetationCases,
      maxError,
      shaderErrors,
      errors,
    };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases, 6144);
  assert.equal(result.layeredCases, 4);
  assert.equal(result.vegetationCases, 64);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
