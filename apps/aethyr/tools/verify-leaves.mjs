// Asset-free actual renderer qualification: masked coverage, depth, raw SH and storage scale.
import assert from "node:assert/strict";
import { createServer } from "vite";
import { chromium } from "playwright";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((req, res, next) => {
  if (req.url !== "/") return next();
  res.setHeader("content-type", "text/html");
  res.end("<title>Owned petal qualification</title>");
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
    const { Leaves, WGSL_LEAVES } =
        await import("/packages/renderer/leaves.ts"),
      { leafFixture } = await import("/tests/fixtures/leaves.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({ code: WGSL_LEAVES }),
      shaderErrors = (await module.getCompilationInfo()).messages
        .filter((m) => m.type === "error")
        .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const vertices = new Float32Array([
        -0.8, -0.8, 0.4, 0, 0, 1, 0, 0, 1, 1, 1, 1, 0.8, -0.8, 0.4, 0, 0, 1, 1,
        0, 1, 1, 1, 1, 0, 0.8, 0.4, 0, 0, 1, 0.5, 1, 1, 1, 1, 1,
      ]),
      indices = new Uint32Array([0, 1, 2]),
      digest = async (bytes) =>
        [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
          .map((n) => n.toString(16).padStart(2, "0"))
          .join("");
    const maps = new Map(),
      realFetch = globalThis.fetch;
    globalThis.fetch = async (url, ...rest) => {
      const key = String(url);
      return maps.has(key)
        ? new Response(maps.get(key), {
            headers: {
              "content-type": "application/octet-stream",
              "content-length": String(maps.get(key).byteLength),
            },
          })
        : realFetch(url, ...rest);
    };
    const image = (rgba) => {
      const bytes = new ArrayBuffer(20),
        h = new DataView(bytes);
      h.setUint32(0, 0x41465238, false);
      h.setUint32(4, 1, true);
      h.setUint32(8, 1, true);
      h.setUint32(12, 1, true);
      new Uint8Array(bytes, 16).set(rgba);
      return bytes;
    };
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      half = (n) => {
        const s = n & 32768 ? -1 : 1,
          e = (n >> 10) & 31,
          m = n & 1023;
        return e === 0
          ? s * m * 2 ** -24
          : e === 31
            ? m
              ? NaN
              : s * Infinity
            : s * 2 ** (e - 15) * (1 + m / 1024);
      };
    const cases = [];
    for (const [label, time, sun, sh, alpha, depthClear] of [
      ["zero-warmup", 0, 1, 0, 255, 1],
      ["sun", 0.5, 1, 0, 255, 1],
      ["double-sun", 0.5, 2, 0, 255, 1],
      ["sh", 0.5, 0, 1, 255, 1],
      ["double-sh", 0.5, 0, 2, 255, 1],
      ["masked", 0.5, 1, 0, 0, 1],
      ["behind-opaque", 0.5, 1, 0, 255, 0.2],
    ]) {
      const m = leafFixture();
      m.profiles[0].gravity = [0, 0, 0];
      m.scenarios.day[0].model = [...identity];
      m.geometry.vertices.sha256 = await digest(vertices.buffer);
      m.geometry.indices.sha256 = await digest(indices.buffer);
      maps.set(m.geometry.vertices.url, vertices.buffer);
      maps.set(m.geometry.indices.url, indices.buffer);
      maps.set(
        m.material.vegetation.baseColorUrl,
        image([255, 255, 255, alpha]),
      );
      maps.set(m.material.vegetation.normalUrl, image([128, 128, 255, 255]));
      maps.set(m.material.vegetation.mraUrl, image([0, 255, 255, 255]));
      device.pushErrorScope("validation");
      const leaves = await Leaves.create(device, m),
        msaa = device.createTexture({
          size: [16, 16],
          sampleCount: 4,
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        }),
        resolved = device.createTexture({
          size: [16, 16],
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        }),
        depth = device.createTexture({
          size: [16, 16],
          sampleCount: 4,
          format: "depth24plus",
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        }),
        read = device.createBuffer({
          size: 256 * 16,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        }),
        encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: msaa.createView(),
              resolveTarget: resolved.createView(),
              loadOp: "clear",
              storeOp: "store",
              clearValue: [0.2, 0.2, 0.2, 1],
            },
          ],
          depthStencilAttachment: {
            view: depth.createView(),
            depthLoadOp: "clear",
            depthStoreOp: "store",
            depthClearValue: depthClear,
          },
        }),
        coeff = new Float32Array(36);
      coeff[0] = coeff[1] = coeff[2] = sh / 0.2820947918;
      const sample = leaves.render(pass, {
        mode: "day",
        time,
        viewProjection: identity,
        eye: [0, 0, 2],
        sunDirection: [0, 0, 1],
        sunIlluminance: [sun, sun, sun],
        diffuseRotation: identity,
        diffuseSH: coeff,
      });
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: resolved },
        { buffer: read, bytesPerRow: 256 },
        [16, 16],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const data = new Uint16Array(read.getMappedRange()),
        at = (8 * 256 + 8 * 8) / 2,
        rgb = [half(data[at]), half(data[at + 1]), half(data[at + 2])];
      read.unmap();
      const error = await device.popErrorScope();
      if (error) errors.push(error.message);
      cases.push({ label, count: sample.count, rgb });
      leaves.dispose();
      for (const t of [msaa, resolved, depth]) t.destroy();
      read.destroy();
    }
    globalThis.fetch = realFetch;
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { shaderErrors, errors, cases };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  const c = Object.fromEntries(result.cases.map((c) => [c.label, c]));
  assert.equal(c["zero-warmup"].count, 0);
  for (const key of ["zero-warmup", "masked", "behind-opaque"])
    assert.ok(c[key].rgb.every((v) => Math.abs(v - 0.2) < 0.0001));
  assert.ok(c.sun.rgb.every((v) => Math.abs(v - 0.01 / Math.PI) < 0.00001));
  assert.ok(c.sh.rgb.every((v) => Math.abs(v - 0.01) < 0.00001));
  for (let a = 0; a < 3; a++) {
    assert.ok(Math.abs(c["double-sun"].rgb[a] - c.sun.rgb[a] * 2) < 0.00001);
    assert.ok(Math.abs(c["double-sh"].rgb[a] - c.sh.rgb[a] * 2) < 0.00001);
  }
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
