// Run: openagents browser run -- node tools/verify-texture-pool.mjs
// Original pixels qualify role color spaces, material-local resolutions, and cleanup.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const root = new URL("..", import.meta.url).pathname;
const server = await createServer({
  configFile: false,
  root,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
function fixture(size, pixel) {
  const bytes = Buffer.alloc(16 + size * size * 4);
  bytes.write("AFR8");
  bytes.writeUInt32LE(1, 4);
  bytes.writeUInt32LE(size, 8);
  bytes.writeUInt32LE(size, 12);
  for (let i = 16; i < bytes.length; i += 4) bytes.set(pixel, i);
  return bytes;
}
server.middlewares.use((request, response, next) => {
  if (request.url === "/") {
    response.setHeader("content-type", "text/html");
    response.end(
      "<!doctype html><title>Owned texture bucket qualification</title>",
    );
  } else if (request.url?.startsWith("/private-assets/owned-pool-")) {
    response.setHeader("content-type", "application/octet-stream");
    response.end(
      request.url.includes("shared")
        ? fixture(2, [64, 128, 192, 0])
        : request.url.includes("normal")
          ? fixture(4, [128, 200, 230, 255])
          : fixture(8, [0, 255, 0, 255]),
    );
  } else next();
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
    const { TexturePool, MaterialTexturePools } =
      await import("/packages/renderer/texture-pool.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required for this check.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const shared = "/private-assets/owned-pool-shared.bin",
      large = "/private-assets/owned-pool-large.bin",
      normal = "/private-assets/owned-pool-normal.bin";
    const requirements = new Map([
      ["small", [{ role: "data", url: shared }]],
      [
        "large",
        [
          { role: "data", url: shared },
          { role: "data", url: large },
        ],
      ],
      ["color", [{ role: "color", url: shared }]],
      ["normal", [{ role: "normal", url: normal }]],
      ["empty", []],
    ]);
    const fetchCounts = new Map(),
      originalFetch = globalThis.fetch.bind(globalThis);
    globalThis.fetch = async (input, init) => {
      const key = String(input);
      fetchCounts.set(key, (fetchCounts.get(key) ?? 0) + 1);
      return originalFetch(input, init);
    };
    function trackDevice(failPipeline = 0) {
      const records = [];
      let pipelines = 0;
      return {
        records,
        facade: {
          limits: device.limits,
          queue: device.queue,
          pushErrorScope: device.pushErrorScope.bind(device),
          popErrorScope: device.popErrorScope.bind(device),
          createSampler: device.createSampler.bind(device),
          createShaderModule: device.createShaderModule.bind(device),
          createCommandEncoder: device.createCommandEncoder.bind(device),
          createBindGroup: device.createBindGroup.bind(device),
          createRenderPipelineAsync: (...args) =>
            ++pipelines === failPipeline
              ? Promise.reject(new Error("Injected pipeline failure"))
              : device.createRenderPipelineAsync(...args),
          createTexture: (descriptor) => {
            const texture = device.createTexture(descriptor),
              destroy = texture.destroy.bind(texture),
              record = { destroyed: 0 };
            texture.destroy = () => {
              record.destroyed++;
              destroy();
            };
            records.push(record);
            return texture;
          },
        },
      };
    }
    const tracked = trackDevice(),
      collection = await MaterialTexturePools.create(
        tracked.facade,
        requirements,
      );
    const sourceLoads = Object.fromEntries(fetchCounts);
    const shader = device.createShaderModule({
      code: `@group(0)@binding(0)var image:texture_2d_array<f32>;@group(0)@binding(1)var<uniform>params:vec4<u32>;@vertex fn v(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.-1.,0.,1.);}@fragment fn f()->@location(0)vec4<f32>{return textureLoad(image,vec2<i32>(0),i32(params.x),i32(params.y));}`,
    });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: shader, entryPoint: "v" },
      fragment: {
        module: shader,
        entryPoint: "f",
        targets: [{ format: "rgba32float" }],
      },
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
    const params = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    async function read(pool, index, mip = 0) {
      device.queue.writeBuffer(params, 0, new Uint32Array([index, mip, 0, 0]));
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            { view: output.createView(), loadOp: "clear", storeOp: "store" },
          ],
        });
      pass.setPipeline(pipeline);
      pass.setBindGroup(
        0,
        device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [
            {
              binding: 0,
              resource: pool.texture.createView({ dimension: "2d-array" }),
            },
            { binding: 1, resource: { buffer: params } },
          ],
        }),
      );
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
        ...new Float32Array(readback.getMappedRange()).slice(0, 4),
      ];
      readback.unmap();
      return value;
    }
    const a = collection.forMaterial("small", requirements.get("small")),
      b = collection.forMaterial("large", requirements.get("large")),
      c = collection.forMaterial("color"),
      n = collection.forMaterial("normal"),
      defaults = collection.forMaterial("new-room", []);
    const pixels = {
      small: await read(a.data, a.data.index(shared)),
      largeSharedMip2: await read(b.data, b.data.index(shared), 2),
      large: await read(b.data, b.data.index(large)),
      color: await read(c.color, c.color.index(shared)),
      normal: await read(n.normal, n.normal.index(normal)),
      defaultColor: await read(defaults.color, 0),
      defaultNormal: await read(defaults.normal, 0),
      defaultData: await read(defaults.data, 0),
    };
    let unknownRejected = false,
      staleRejected = false;
    try {
      collection.forMaterial("unknown", [{ role: "data", url: shared }]);
    } catch {
      unknownRejected = true;
    }
    try {
      collection.forMaterial("small", [{ role: "normal", url: shared }]);
    } catch {
      staleRejected = true;
    }
    const sizes = {
        small: a.data.size,
        large: b.data.size,
        normal: n.normal.size,
        default: defaults.data.size,
      },
      bytes = collection.bytes;
    collection.dispose();
    collection.dispose();
    const disposedOnce = tracked.records.every((r) => r.destroyed === 1);
    let disposedRejected = false;
    try {
      collection.forMaterial("empty");
    } catch {
      disposedRejected = true;
    }
    const legacy = await TexturePool.create(device, "data", [shared, large]);
    pixels.legacyMip2 = await read(legacy, legacy.index(shared), 2);
    legacy.dispose();
    const failed = trackDevice(2);
    let preparationRejected = false;
    try {
      await MaterialTexturePools.create(failed.facade, requirements);
    } catch (error) {
      preparationRejected = String(error).includes("Injected pipeline failure");
    }
    await device.queue.onSubmittedWorkDone();
    const failedOnce =
      failed.records.length > 1 &&
      failed.records.every((r) => r.destroyed === 1);
    params.destroy();
    readback.destroy();
    output.destroy();
    device.destroy();
    return {
      pixels,
      sizes,
      bytes,
      sourceLoads,
      unknownRejected,
      staleRejected,
      disposedOnce,
      disposedRejected,
      preparationRejected,
      failedOnce,
      allocatedBeforeFailure: failed.records.length,
      errors,
    };
  });
  const close = (actual, expected) =>
    actual.forEach((value, c) =>
      assert.ok(
        Math.abs(value - expected[c]) < 0.002,
        `${value} != ${expected[c]}`,
      ),
    );
  const raw = [64 / 255, 128 / 255, 192 / 255, 0];
  close(result.pixels.small, raw);
  close(result.pixels.largeSharedMip2, raw);
  close(result.pixels.legacyMip2, raw);
  close(
    result.pixels.color,
    raw.map((v, c) =>
      c === 3 ? 0 : v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4,
    ),
  );
  close(result.pixels.large, [0, 1, 0, 1]);
  close(result.pixels.normal, [128 / 255, 200 / 255, 230 / 255, 1]);
  close(result.pixels.defaultColor, [1, 1, 1, 1]);
  close(result.pixels.defaultNormal, [128 / 255, 128 / 255, 1, 1]);
  close(result.pixels.defaultData, [0, 1, 1, 1]);
  assert.deepEqual(result.sizes, { small: 2, large: 8, normal: 4, default: 1 });
  assert.equal(result.bytes, 1280);
  assert.ok(Object.values(result.sourceLoads).every((count) => count === 1));
  for (const key of [
    "unknownRejected",
    "staleRejected",
    "disposedOnce",
    "disposedRejected",
    "preparationRejected",
    "failedOnce",
  ])
    assert.equal(result[key], true, key);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
