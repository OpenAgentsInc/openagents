// Owned full-material GPU qualification; no licensed assets or account access.
// Run: openagents browser run -- node tools/verify-sidedness.mjs
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
  res.end(
    "<!doctype html><title>Owned material sidedness qualification</title>",
  );
});
await server.listen();
const browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  ),
  context = await browser.newContext();
try {
  const page = await context.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { MATERIAL } = await import("/packages/renderer/shaders.ts");
    const { FRAME_FLOAT_COUNT } = await import("/packages/renderer/frame.ts");
    const { LAYER_FLOAT_COUNT } = await import("/packages/renderer/layers.ts");
    const { GLASS_FLOAT_COUNT } = await import("/packages/renderer/glass.ts");
    const { PAINT_FLOAT_COUNT } = await import("/packages/renderer/paint.ts");
    const { TERRAIN_FLOAT_COUNT } =
      await import("/packages/renderer/terrain.ts");
    const { VEGETATION_FLOAT_COUNT } =
      await import("/packages/renderer/vegetation.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({ code: MATERIAL }),
      info = await module.getCompilationInfo();
    const shaderErrors = info.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}:${m.linePos}: ${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const stage = GPUShaderStage,
      uniform = device.createBuffer({
        size: FRAME_FLOAT_COUNT * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      material = device.createBuffer({
        size:
          (20 +
            LAYER_FLOAT_COUNT +
            GLASS_FLOAT_COUNT +
            PAINT_FLOAT_COUNT +
            TERRAIN_FLOAT_COUNT +
            VEGETATION_FLOAT_COUNT) *
          4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
    const frameLayout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: stage.VERTEX | stage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 1,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "depth" },
        },
        {
          binding: 3,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "depth", viewDimension: "2d-array" },
        },
        {
          binding: 2,
          visibility: stage.FRAGMENT,
          sampler: { type: "comparison" },
        },
        {
          binding: 4,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "depth", viewDimension: "2d-array" },
        },
        ...[5, 7, 9, 11].map((binding) => ({
          binding,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "float" },
        })),
        {
          binding: 8,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        { binding: 6, visibility: stage.FRAGMENT, sampler: {} },
        {
          binding: 10,
          visibility: stage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 16,
          visibility: stage.VERTEX,
          buffer: { type: "read-only-storage" },
        },
        ...[12, 13, 14].map((binding) => ({
          binding,
          visibility: stage.FRAGMENT,
          buffer: { type: "read-only-storage" },
        })),
      ],
    });
    const materialLayout = device.createBindGroupLayout({
        entries: [
          ...[0, 1, 2, 5].map((binding) => ({
            binding,
            visibility: stage.FRAGMENT,
            texture: { sampleType: "float" },
          })),
          {
            binding: 3,
            visibility: stage.VERTEX | stage.FRAGMENT,
            sampler: {},
          },
          {
            binding: 4,
            visibility: stage.VERTEX | stage.FRAGMENT,
            buffer: { type: "uniform" },
          },
          ...[6, 7, 8].map((binding) => ({
            binding,
            visibility: stage.FRAGMENT | (binding === 6 ? stage.VERTEX : 0),
            texture: { sampleType: "float", viewDimension: "2d-array" },
          })),
        ],
      }),
      layout = device.createPipelineLayout({
        bindGroupLayouts: [frameLayout, materialLayout],
      });
    const usage = GPUTextureUsage,
      white = device.createTexture({
        size: [1, 1],
        format: "rgba8unorm",
        usage: usage.COPY_DST | usage.TEXTURE_BINDING,
      });
    device.queue.writeTexture(
      { texture: white },
      new Uint8Array([255, 255, 255, 255]),
      { bytesPerRow: 4 },
      [1, 1],
    );
    const whiteView = white.createView(),
      arrayView = white.createView({ dimension: "2d-array" });
    const shadow = device.createTexture({
        size: [1, 1, 2],
        format: "depth32float",
        usage: usage.RENDER_ATTACHMENT | usage.TEXTURE_BINDING,
      }),
      shadowView = shadow.createView({
        dimension: "2d",
        baseArrayLayer: 0,
        arrayLayerCount: 1,
      }),
      shadowArray = shadow.createView({ dimension: "2d-array" }),
      sampler = device.createSampler(),
      comparison = device.createSampler({ compare: "less-equal" }),
      storage = device.createBuffer({
        size: 256,
        usage: GPUBufferUsage.STORAGE,
      });
    const frameGroup = device.createBindGroup({
        layout: frameLayout,
        entries: [
          { binding: 0, resource: { buffer: uniform } },
          { binding: 1, resource: shadowView },
          { binding: 3, resource: shadowArray },
          { binding: 2, resource: comparison },
          { binding: 4, resource: shadowArray },
          ...[5, 7, 9, 11].map((binding) => ({
            binding,
            resource: whiteView,
          })),
          { binding: 8, resource: arrayView },
          { binding: 6, resource: sampler },
          { binding: 10, resource: arrayView },
          { binding: 16, resource: { buffer: storage } },
          ...[12, 13, 14].map((binding) => ({
            binding,
            resource: { buffer: storage },
          })),
        ],
      }),
      materialGroup = device.createBindGroup({
        layout: materialLayout,
        entries: [
          ...[0, 1, 2, 5].map((binding) => ({ binding, resource: whiteView })),
          { binding: 3, resource: sampler },
          { binding: 4, resource: { buffer: material } },
          ...[6, 7, 8].map((binding) => ({ binding, resource: arrayView })),
        ],
      });
    const vertices = new Float32Array([
        -0.9, -0.9, 0.5, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0.9, -0.9, 0.5, 0, 0, 1, 1,
        0, 1, 0, 0, 1, 0, 0.9, 0.5, 0, 0, 1, 0.5, 1, 1, 0, 0, 1,
      ]),
      upload = (data, usage) => {
        const b = device.createBuffer({
          size: data.byteLength,
          usage: usage | GPUBufferUsage.COPY_DST,
        });
        device.queue.writeBuffer(b, 0, data);
        return b;
      },
      vertex = upload(vertices, GPUBufferUsage.VERTEX),
      models = device.createBuffer({
        size: 64,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      }),
      colors = device.createBuffer({
        size: 12,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      }),
      uv = upload(new Float32Array(6), GPUBufferUsage.VERTEX),
      wind = upload(new Float32Array(4), GPUBufferUsage.VERTEX);
    const buffers = [
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
    const pipelines = {};
    for (const reflection of [false, true])
      for (const kind of ["main", "depth", "shadow"])
        pipelines[`${kind}-${reflection}`] =
          await device.createRenderPipelineAsync({
            layout,
            vertex: {
              module,
              entryPoint: kind === "shadow" ? "shadowVS" : "vs",
              buffers,
            },
            fragment: {
              module,
              entryPoint:
                kind === "main"
                  ? "fs"
                  : kind === "depth"
                    ? "depthFS"
                    : "shadowFS",
              targets: kind === "main" ? [{ format: "rgba16float" }] : [],
            },
            primitive: {
              cullMode: "none",
              frontFace: reflection ? "cw" : "ccw",
            },
            depthStencil: {
              format: "depth24plus",
              depthWriteEnabled: true,
              depthCompare: "less",
            },
            multisample: { count: 4, alphaToCoverageEnabled: kind === "main" },
          });
    const depth = device.createTexture({
        size: [8, 8],
        format: "depth24plus",
        sampleCount: 4,
        usage: usage.RENDER_ATTACHMENT | usage.TEXTURE_BINDING,
      }),
      color = device.createTexture({
        size: [8, 8],
        format: "rgba16float",
        sampleCount: 4,
        usage: usage.RENDER_ATTACHMENT,
      }),
      resolved = device.createTexture({
        size: [8, 8],
        format: "rgba16float",
        usage: usage.RENDER_ATTACHMENT | usage.COPY_SRC,
      }),
      depthOutput = device.createTexture({
        size: [1, 1],
        format: "rgba32float",
        usage: usage.RENDER_ATTACHMENT | usage.COPY_SRC,
      });
    const readModule = device.createShaderModule({
        code: "@group(0)@binding(0)var z:texture_depth_multisampled_2d;@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.-1.,0.,1.);}@fragment fn fs()->@location(0)vec4<f32>{return vec4(textureLoad(z,vec2(4,4),0),textureLoad(z,vec2(4,4),1),textureLoad(z,vec2(4,4),2),textureLoad(z,vec2(4,4),3));}",
      }),
      readPipeline = await device.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module: readModule, entryPoint: "vs" },
        fragment: {
          module: readModule,
          entryPoint: "fs",
          targets: [{ format: "rgba32float" }],
        },
      }),
      readGroup = device.createBindGroup({
        layout: readPipeline.getBindGroupLayout(0),
        entries: [{ binding: 0, resource: depth.createView() }],
      });
    const identity = () =>
        new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
      half = (n) => {
        const e = (n >> 10) & 31,
          f = n & 1023;
        return e === 0 ? (2 ** -14 * f) / 1024 : 2 ** (e - 15) * (1 + f / 1024);
      },
      results = [];
    for (const reflection of [false, true])
      for (const negative of [false, true])
        for (const back of [false, true])
          for (const twoSided of [false, true])
            for (const sample of ["opaque", "mask", "vertex-mask", "lit"]) {
              const hole = sample === "mask" || sample === "vertex-mask",
                vertexHole = sample === "vertex-mask";
              const m = identity();
              if (negative) m[0] = -1;
              device.queue.writeBuffer(models, 0, m);
              device.queue.writeBuffer(
                colors,
                0,
                new Uint8Array([
                  255,
                  255,
                  255,
                  vertexHole ? 0 : 255,
                  255,
                  255,
                  255,
                  vertexHole ? 0 : 255,
                  255,
                  255,
                  255,
                  vertexHole ? 0 : 255,
                ]),
              );
              const f = new Float32Array(FRAME_FLOAT_COUNT),
                view = identity();
              if (back) {
                view[0] = -1;
                view[10] = -1;
                view[14] = 1;
              }
              if (reflection) view[0] *= -1;
              f.set(view, 0);
              f.set(view, 16);
              f.set(view, 32);
              f.set(view, 336);
              f[50] = back ? -1 : 1;
              f[70] = 1;
              f[71] = 1;
              if (sample === "lit") {
                f[54] = back ? -1 : 1;
                f[55] = 1;
                f.set([1, 1, 1], 56);
              }
              f[75] = 1;
              device.queue.writeBuffer(uniform, 0, f);
              const mat = new Float32Array(material.size / 4);
              mat.set([1, 1, 1, sample === "mask" ? 0 : 1]);
              mat[4] = sample === "lit" ? 0 : 1;
              mat[8] = 1;
              mat[11] = hole ? 1 : 0;
              mat.set([1, 1, 0.5, Number(twoSided)], 12);
              mat[18] = 1;
              device.queue.writeBuffer(material, 0, mat);
              const outcomes = {};
              for (const kind of ["main", "depth", "shadow"]) {
                const encoder = device.createCommandEncoder(),
                  clear = encoder.beginRenderPass({
                    colorAttachments: [],
                    depthStencilAttachment: {
                      view: shadowView,
                      depthClearValue: 1,
                      depthLoadOp: "clear",
                      depthStoreOp: "store",
                    },
                  });
                clear.end();
                const pass = encoder.beginRenderPass({
                  colorAttachments:
                    kind === "main"
                      ? [
                          {
                            view: color.createView(),
                            resolveTarget: resolved.createView(),
                            loadOp: "clear",
                            storeOp: "discard",
                            clearValue: [0, 0, 0, 0],
                          },
                        ]
                      : [],
                  depthStencilAttachment: {
                    view: depth.createView(),
                    depthClearValue: 1,
                    depthLoadOp: "clear",
                    depthStoreOp: "store",
                  },
                });
                pass.setPipeline(pipelines[`${kind}-${reflection}`]);
                pass.setBindGroup(0, frameGroup);
                pass.setBindGroup(1, materialGroup);
                for (const [i, b] of [
                  vertex,
                  models,
                  colors,
                  uv,
                  wind,
                ].entries())
                  pass.setVertexBuffer(i, b);
                pass.draw(3);
                pass.end();
                const readPass = encoder.beginRenderPass({
                  colorAttachments: [
                    {
                      view: depthOutput.createView(),
                      loadOp: "clear",
                      storeOp: "store",
                    },
                  ],
                });
                readPass.setPipeline(readPipeline);
                readPass.setBindGroup(0, readGroup);
                readPass.draw(3);
                readPass.end();
                const read = device.createBuffer({
                  size: 256,
                  usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
                });
                encoder.copyTextureToBuffer(
                  { texture: depthOutput },
                  { buffer: read, bytesPerRow: 256 },
                  [1, 1],
                );
                const colorRead =
                  kind === "main"
                    ? device.createBuffer({
                        size: 2048,
                        usage:
                          GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
                      })
                    : undefined;
                if (colorRead)
                  encoder.copyTextureToBuffer(
                    { texture: resolved },
                    { buffer: colorRead, bytesPerRow: 256 },
                    [8, 8],
                  );
                device.queue.submit([encoder.finish()]);
                await read.mapAsync(GPUMapMode.READ);
                const samples = Array.from(
                  new Float32Array(read.getMappedRange()).slice(0, 4),
                );
                read.unmap();
                read.destroy();
                let red;
                if (colorRead) {
                  await colorRead.mapAsync(GPUMapMode.READ);
                  red = half(
                    new Uint16Array(colorRead.getMappedRange())[
                      4 * 128 + 4 * 4
                    ],
                  );
                  colorRead.unmap();
                  colorRead.destroy();
                }
                outcomes[kind] = {
                  covered: samples.every((z) => z < 0.75),
                  empty: samples.every((z) => z === 1),
                  red,
                };
              }
              const visible = !hole && (twoSided || !back);
              for (const kind of ["main", "depth"])
                if (
                  outcomes[kind].covered !== visible ||
                  outcomes[kind].empty === visible
                )
                  throw Error(
                    JSON.stringify({
                      reflection,
                      negative,
                      back,
                      twoSided,
                      hole,
                      kind,
                      outcomes,
                    }),
                  );
              if (outcomes.shadow.covered !== (sample !== "mask"))
                throw Error("Shadow two-sided policy changed");
              if (
                sample !== "lit" &&
                Math.abs(outcomes.main.red - (visible ? 1 : 0)) > 0.001
              )
                throw Error("Main color does not match coverage");
              if (sample === "lit" && visible && outcomes.main.red < 0.2)
                throw Error("Source-facing normal is not lit");
              results.push({
                sample,
                reflection,
                negative,
                back,
                twoSided,
                hole,
                outcomes,
              });
            }
    for (const test of results.filter((r) => r.sample === "lit")) {
      const baseline = results.find(
        (r) =>
          r.sample === "lit" &&
          !r.reflection &&
          !r.negative &&
          r.back === test.back &&
          r.twoSided === test.twoSided,
      );
      if (Math.abs(test.outcomes.main.red - baseline.outcomes.main.red) > 0.001)
        throw Error("Mirrored/reflected normal changed lighting");
    }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return {
      cases: results.length,
      checks: results.length * 3,
      shaderErrors,
      errors,
    };
  });
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases, 64);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
