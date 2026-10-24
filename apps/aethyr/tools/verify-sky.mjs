// Owned actual-SKY qualification. No licensed assets, private coordinates, or account access.
// Run: openagents browser run -- node tools/verify-sky.mjs
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
    "<!doctype html><title>Owned finite skydome transport qualification</title>",
  );
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
    const { SKY } = await import("/packages/renderer/shaders.ts"),
      { FRAME_FLOAT_COUNT, FRAME_OFFSETS } =
        await import("/packages/renderer/frame.ts"),
      { skyDomeDistance } = await import("/packages/renderer/sky.ts"),
      {
        heightFogComposition,
        FOG_HEIGHT_RATE,
        FOG_REFERENCE_HEIGHT,
        FOG_START_DISTANCE,
      } = await import("/packages/renderer/fog.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({ code: SKY }),
      info = await module.getCompilationInfo(),
      shaderErrors = info.messages
        .filter((m) => m.type === "error")
        .map((m) => `${m.lineNum}: ${m.message}`);
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
    const frame = device.createBuffer({
        size: FRAME_FLOAT_COUNT * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      domeBuffer = device.createBuffer({
        size: 16,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      }),
      image = device.createTexture({
        size: [1, 1],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      }),
      target = device.createTexture({
        size: [1, 1],
        format: "rgba32float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      }),
      read = device.createBuffer({
        size: 256,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      }),
      sampler = device.createSampler({ addressModeU: "repeat" });
    device.queue.writeTexture(
      { texture: image },
      new Uint16Array([0x4000, 0x3800, 0x3400, 0x3c00]),
      { bytesPerRow: 8 },
      [1, 1],
    );
    const group = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: frame } },
        { binding: 5, resource: image.createView() },
        { binding: 6, resource: sampler },
        { binding: 15, resource: { buffer: domeBuffer } },
      ],
    });
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      color = [2, 0.5, 0.25],
      fogColor = [0.1, 0.2, 0.3],
      cases = [];
    const fixtures = [
      { name: "zero density", rho: 0, direction: [0, 0, 1] },
      { name: "source horizon", rho: 0.002, direction: [0, 0, 1] },
      { name: "long ascent", rho: 0.002, direction: [0, 1, 0] },
      { name: "long descent", rho: 0.002, direction: [0, -1, 0] },
      {
        name: "volume replaces near fill",
        rho: 0.002,
        direction: [0, 0, 1],
        volume: [2, FOG_HEIGHT_RATE, 1, 0],
        range: [60, 1],
      },
      {
        name: "inside near volume",
        rho: 0.002,
        direction: [0, 0, 1],
        radius: 20,
        volume: [2, FOG_HEIGHT_RATE, 1, 0],
        range: [60, 1],
      },
      {
        name: "nonzero volume start",
        rho: 0.002,
        direction: [0, 0, 1],
        volume: [2, FOG_HEIGHT_RATE, 1, 10],
        range: [60, 1],
      },
      {
        name: "reflected eye",
        rho: 0.002,
        direction: [0, 1, 0],
        eye: [0, -2, 0],
      },
      {
        name: "outside dome toward surface",
        rho: 0.002,
        direction: [0, 0, -1],
        radius: 10,
        eye: [0, 1, 200],
      },
      {
        name: "outside dome misses",
        rho: 0.002,
        direction: [0, 0, 1],
        radius: 10,
        eye: [0, 1, 200],
      },
      { name: "unavailable dome", rho: 0.002, direction: [0, 0, 1], radius: 0 },
      {
        name: "generated no fog",
        rho: 0,
        direction: [0, 0, 1],
        generated: true,
      },
      {
        name: "generated unchanged",
        rho: 1,
        direction: [0, 0, 1],
        generated: true,
      },
    ];
    for (const fixture of fixtures) {
      const eye = fixture.eye ?? [0, 1, 0],
        direction = fixture.direction,
        radius = fixture.radius ?? 10000,
        dome = { center: [0, 1, 0], radiusMeters: radius },
        volume = fixture.volume ?? [1, FOG_HEIGHT_RATE, 1, 0],
        range = fixture.range ?? [60, 0];
      const u = new Float32Array(FRAME_FLOAT_COUNT);
      u.set(identity, 16);
      u.set(
        eye.map((v, i) => v + direction[i] * 100 - (i === 2 ? 1 : 0)),
        28,
      );
      u.set([...eye, 0], 48);
      u.set([...fogColor, fixture.rho], 64);
      u[74] = 1;
      u.set(identity, FRAME_OFFSETS.skyRotation);
      u.set([1, 0, 1, 0], FRAME_OFFSETS.skyUV);
      u.set([1, 1, 1, 1], FRAME_OFFSETS.skyTint);
      u[FRAME_OFFSETS.skyEnabled] = fixture.generated ? 0 : 1;
      u.set(volume, FRAME_OFFSETS.volumeMedium);
      u.set([...range, 0, 0], FRAME_OFFSETS.volumeRange);
      device.queue.writeBuffer(frame, 0, u);
      device.queue.writeBuffer(
        domeBuffer,
        0,
        new Float32Array([...dome.center, radius]),
      );
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            { view: target.createView(), loadOp: "clear", storeOp: "store" },
          ],
        });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.draw(3);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: target },
        { buffer: read, bytesPerRow: 256 },
        [1, 1],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const actual = [...new Float32Array(read.getMappedRange()).slice(0, 3)];
      read.unmap();
      if (!actual.every(Number.isFinite))
        throw Error(`${fixture.name} returned non-finite sky radiance`);
      let distance = 0,
        transport = [1, 0];
      if (!fixture.generated && radius > 0) {
        distance = skyDomeDistance(dome, eye, direction);
        if (distance > 0)
          transport = heightFogComposition(
            eye[1],
            direction[1],
            distance,
            fixture.rho,
            FOG_HEIGHT_RATE,
            FOG_REFERENCE_HEIGHT,
            FOG_START_DISTANCE,
            volume,
            range,
          );
      }
      if (!fixture.generated) {
        const expected = color.map(
          (c, i) => c * transport[0] + fogColor[i] * transport[1],
        );
        expected.forEach((v, i) => {
          if (Math.abs(v - actual[i]) > 2e-5)
            throw Error(`${fixture.name} channel${i}: ${actual[i]} != ${v}`);
        });
      }
      cases.push({ name: fixture.name, distance, transport, actual });
    }
    const generated = cases.slice(-2);
    generated[0].actual.forEach((v, i) => {
      if (v !== generated[1].actual[i])
        throw Error("Source dome fog changed the generated sky profile.");
    });
    const near = cases.find((c) => c.name === "inside near volume");
    if (near.transport[1] !== 0)
      throw Error("Near source volume received constant analytic fill.");
    await device.queue.onSubmittedWorkDone();
    for (const resource of [frame, domeBuffer, image, target, read])
      resource.destroy();
    device.destroy();
    return { cases, shaderErrors, errors };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases.length, 13);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
