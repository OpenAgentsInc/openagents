// Owned synthetic HDR maps only. No application state or licensed source assets.
// Run: openagents browser run -- node tools/verify-water-lighting.mjs
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
  response.end(
    "<!doctype html><title>Owned water environment qualification</title>",
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
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { WATER_PASS_SHADER, WaterPass } =
      await import("/packages/renderer/water-pass.ts");
    const { encodeRgba16fLE, halfToFloat } =
      await import("/packages/renderer/environment-lighting.ts");
    const { multiply, perspective, lookAt, inverse } =
      await import("/packages/renderer/math.ts");
    if (!WATER_PASS_SHADER.includes("fn waterEnvironmentReflection("))
      throw new Error(
        "The actual water shader must expose its environment helper.",
      );
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const realPass = await WaterPass.create(device);
    const module = device.createShaderModule({
      code:
        WATER_PASS_SHADER +
        `
@group(1) @binding(0) var<uniform> query:vec4<f32>;
@vertex fn qaVS(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{
 let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0,1);
}
@fragment fn qaFS()->@location(0)vec4<f32>{return vec4(waterEnvironmentReflection(query.xyz),1.0);}
`,
    });
    const shaderErrors = (await module.getCompilationInfo()).messages
      .filter((m) => m.type === "error")
      .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "qaVS" },
      fragment: {
        module,
        entryPoint: "qaFS",
        targets: [{ format: "rgba32float" }],
      },
    });
    const frame = device.createBuffer({
        size: 512 * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      query = device.createBuffer({
        size: 16,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      readback = device.createBuffer({
        size: 256,
        usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
      }),
      environment = device.createTexture({
        size: [4, 2, 2],
        mipLevelCount: 3,
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      }),
      output = device.createTexture({
        size: [1, 1],
        format: "rgba32float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      }),
      sampler = device.createSampler({
        magFilter: "nearest",
        minFilter: "nearest",
        mipmapFilter: "linear",
        addressModeU: "repeat",
        addressModeV: "clamp-to-edge",
      });
    const color = (layer, mip, x, y) =>
      mip === 0
        ? layer === 0
          ? [1 + x, 2 + y, 3 + x + y]
          : [10 + x, 20 + y, 30 + x + y]
        : mip === 1
          ? layer === 0
            ? [4 + x, 5, 6 + x]
            : [20 + x, 30, 40 + x]
          : layer === 0
            ? [8, 10, 12]
            : [40, 50, 60];
    for (let mip = 0; mip < 3; mip++)
      for (let layer = 0; layer < 2; layer++) {
        const width = Math.max(1, 4 >> mip),
          height = Math.max(1, 2 >> mip),
          values = [];
        for (let y = 0; y < height; y++)
          for (let x = 0; x < width; x++)
            values.push(...color(layer, mip, x, y), 1);
        device.queue.writeTexture(
          { texture: environment, mipLevel: mip, origin: [0, 0, layer] },
          encodeRgba16fLE(new Float32Array(values)),
          { bytesPerRow: width * 8, rowsPerImage: height },
          [width, height],
        );
      }
    const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          {
            binding: 4,
            resource: environment.createView({ dimension: "2d-array" }),
          },
          { binding: 5, resource: sampler },
        ],
      }),
      queryGroup = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(1),
        entries: [{ binding: 0, resource: { buffer: query } }],
      });
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      rotate = [0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1],
      at = (u, v) => [
        Math.sin(u * 2 * Math.PI) * Math.sin(v * Math.PI),
        Math.cos(v * Math.PI),
        Math.cos(u * 2 * Math.PI) * Math.sin(v * Math.PI),
      ];
    const fixtures = [];
    for (const solar of [0, 1, 2])
      for (const constant of [0, 1])
        fixtures.push({
          name: `solar${solar}-constant${constant}`,
          solar,
          constant,
        });
    for (const roughness of [0, 0.25, 0.5, 0.75, 1])
      fixtures.push({
        name: `mip${roughness * 2}`,
        roughness,
        solar: 2,
        constant: 1,
      });
    fixtures.push(
      { name: "gain-once", solar: 2, constant: 1, gain: [2, 0.5, 3] },
      { name: "gain-zero", solar: 2, constant: 1, gain: [0, 0, 0] },
      { name: "colored-gain", solar: 1, constant: 1, gain: [0, 1, 2] },
      { name: "rotation-both-layers", solar: 1, constant: 1, matrix: rotate },
      {
        name: "rotation-mip-blend",
        solar: 2,
        constant: 1,
        matrix: rotate,
        roughness: 0.25,
      },
      { name: "mipmax-zero", solar: 1, constant: 1, roughness: 1, mipMax: 0 },
      {
        name: "zero-direction-fallback",
        solar: 2,
        constant: 1,
        direction: [0, 0, 0],
        finiteOnly: true,
      },
      {
        name: "zero-transform-fallback",
        solar: 2,
        constant: 1,
        matrix: Array(16).fill(0),
        finiteOnly: true,
      },
    );
    const nearest = (layer, mip, uv) => {
      const width = Math.max(1, 4 >> mip),
        height = Math.max(1, 2 >> mip);
      return color(
        layer,
        mip,
        Math.min(width - 1, Math.floor(uv[0] * width)),
        Math.min(height - 1, Math.max(0, Math.floor(uv[1] * height))),
      );
    };
    let maxError = 0;
    const values = [];
    for (const f of fixtures) {
      const u = new Float32Array(512),
        direction = f.direction ?? at(0.125, 0.25),
        matrix = f.matrix ?? identity,
        gain = f.gain ?? [1, 1, 1],
        roughness = f.roughness ?? 0,
        mipMax = f.mipMax ?? 2;
      u.set([0, 0, roughness, 0], 56);
      u.set(matrix, 460);
      u.set([1, mipMax, f.solar, f.constant], 476);
      u.set([...gain, 0], 480);
      device.queue.writeBuffer(frame, 0, u);
      device.queue.writeBuffer(query, 0, new Float32Array([...direction, 0]));
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            { view: output.createView(), loadOp: "clear", storeOp: "store" },
          ],
        });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.setBindGroup(1, queryGroup);
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: output },
        { buffer: readback, bytesPerRow: 256 },
        [1, 1],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const actual = Array.from(
        new Float32Array(readback.getMappedRange(), 0, 4),
      );
      readback.unmap();
      let expected;
      if (!f.finiteOnly) {
        const d = [0, 1, 2].map(
            (i) =>
              matrix[i] * direction[0] +
              matrix[4 + i] * direction[1] +
              matrix[8 + i] * direction[2],
          ),
          len = Math.hypot(...d);
        const uv = [
            (((Math.atan2(d[0], d[2]) / (2 * Math.PI)) % 1) + 1) % 1,
            Math.acos(Math.max(-1, Math.min(1, d[1] / len))) / Math.PI,
          ],
          level = roughness * mipMax,
          low = Math.floor(level),
          high = Math.ceil(level),
          weight = level - low;
        const sampled = (layer) =>
            nearest(layer, low, uv).map(
              (v, c) => v * (1 - weight) + nearest(layer, high, uv)[c] * weight,
            ),
          solar = sampled(0),
          constant = sampled(1);
        expected = solar.map(
          (v, c) => (v * f.solar + constant[c] * f.constant) * gain[c],
        );
        actual
          .slice(0, 3)
          .forEach(
            (v, c) =>
              (maxError = Math.max(maxError, Math.abs(v - expected[c]))),
          );
      }
      values.push({
        name: f.name,
        actual,
        expected: expected ?? null,
        finiteOnly: !!f.finiteOnly,
      });
    }
    // Exercise actual binding creation and draw submission. A successful shader
    // helper/pipeline alone cannot validate fallback texture-view dimensions.
    const actualTextures = [];
    const texture = (description) => {
      const t = device.createTexture(description);
      actualTextures.push(t);
      return t;
    };
    const size = 8,
      actualColor = texture({
        size: [size, size],
        sampleCount: 4,
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      }),
      depth = texture({
        size: [size, size],
        sampleCount: 4,
        format: "depth24plus",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      }),
      resolved = texture({
        size: [size, size],
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      }),
      scene = texture({
        size: [size, size],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      }),
      actualReadback = device.createBuffer({
        size: 256 * size,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      }),
      opaque = [0.125, 0.25, 0.5, 1],
      pixels = new Float32Array(size * size * 4);
    for (let i = 0; i < size * size; i++) pixels.set(opaque, i * 4);
    device.queue.writeTexture(
      { texture: scene },
      encodeRgba16fLE(pixels),
      { bytesPerRow: size * 8 },
      [size, size],
    );
    realPass.setSources({
      schema: "aethyrforge.water.v1",
      basins: [
        {
          id: "owned-basin",
          polygonXZ: [
            [-1, -1],
            [1, -1],
            [1, 1],
            [-1, 1],
          ],
          surfaceY: 0,
          floorY: -0.2,
        },
      ],
      jets: [],
    });
    const eye = [0, 2, 3],
      view = multiply(perspective(1, 1, 0.1, 100), lookAt(eye, [0, 0, 0])),
      realRenders = [];
    for (const withEnvironment of [false, true]) {
      device.pushErrorScope("validation");
      const encoder = device.createCommandEncoder(),
        initial = encoder.beginRenderPass({
          colorAttachments: [
            {
            view: actualColor.createView(),
              loadOp: "clear",
              storeOp: "store",
              clearValue: opaque,
            },
          ],
          depthStencilAttachment: {
            view: depth.createView(),
            depthClearValue: 1,
            depthLoadOp: "clear",
            depthStoreOp: "store",
          },
        });
      initial.end();
      const drew = realPass.render(encoder, {
        width: size,
        height: size,
        colorView: actualColor.createView(),
        resolveTarget: resolved.createView(),
        depthView: depth.createView(),
        sceneColor: scene.createView(),
        viewProjection: view,
        inverseViewProjection: inverse(view),
        eye,
        time: 0,
        sunColor: [0, 0, 0],
        ambient: [0, 0, 0],
        skyZenith: [0.2, 0.3, 0.4],
        skyHorizon: [0.1, 0.2, 0.3],
        screenSpaceReflections: false,
        ...(withEnvironment
          ? {
              environment: {
                view: environment.createView({ dimension: "2d-array" }),
                worldToEnvironment: identity,
                mipMax: 2,
                gain: [0.1, 0.1, 0.1],
                solarRatio: 2,
                hasConstantFog: true,
              },
            }
          : {}),
      });
      encoder.copyTextureToBuffer(
        { texture: resolved },
        { buffer: actualReadback, bytesPerRow: 256 },
        [size, size],
      );
      device.queue.submit([encoder.finish()]);
      await device.queue.onSubmittedWorkDone();
      const validation = await device.popErrorScope();
      if (validation) throw new Error(validation.message);
      await actualReadback.mapAsync(GPUMapMode.READ);
      const bytes = new DataView(actualReadback.getMappedRange());
      let changed = 0,
        finite = true;
      for (let y = 0; y < size; y++)
        for (let x = 0; x < size; x++) {
          const pixel = [0, 1, 2, 3].map((c) =>
            halfToFloat(bytes.getUint16(y * 256 + x * 8 + c * 2, true)),
          );
          finite &&= pixel.every(Number.isFinite);
          if (
            pixel.slice(0, 3).some((v, c) => Math.abs(v - opaque[c]) > 0.0001)
          )
            changed++;
        }
      actualReadback.unmap();
      realRenders.push({
        withEnvironment,
        planar: false,
        drew,
        finite,
        changedPixels: changed,
      });
    }
    realPass.dispose();
    actualReadback.destroy();
    for (const t of actualTextures) t.destroy();
    await device.queue.onSubmittedWorkDone();
    for (const resource of [frame, query, readback, environment, output])
      resource.destroy();
    device.destroy();
    return {
      shaderErrors,
      errors,
      realPassPipeline: true,
      realRenders,
      maxError,
      values,
    };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.realPassPipeline, true);
  assert.equal(result.realRenders.length, 2);
  for (const frame of result.realRenders) {
    assert.equal(frame.drew, true);
    assert.equal(frame.finite, true);
    assert.ok(frame.changedPixels > 0);
  }
  for (const value of result.values) {
    assert.equal(value.actual[3], 1);
    assert.ok(value.actual.every(Number.isFinite), value.name);
    if (value.expected)
      value.actual
        .slice(0, 3)
        .forEach((v, c) =>
          assert.ok(
            Math.abs(v - value.expected[c]) < 0.0001,
            `${value.name}: ${value.actual} != ${value.expected}`,
          ),
        );
    else
      assert.ok(
        value.actual.slice(0, 3).every((v) => v >= 0 && v <= 200),
        `${value.name}: invalid fallback radiance`,
      );
  }
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
