// Synthetic source-card graph and actual four-sample readonly-depth pass qualification.
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
  res.end("<title>Source fog-card qualification</title>");
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
    const {
        SourceFogCards,
        SOURCE_FOG_CARDS_WGSL,
        fogCardSample,
        fogCardEdgeFactor,
      } = await import("/packages/renderer/source-fog-cards.ts"),
      { fogManifestFixture } =
        await import("/tests/fixtures/source-fog-cards.ts"),
      { perspective, inverse } = await import("/packages/renderer/math.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const info = await device
        .createShaderModule({ code: SOURCE_FOG_CARDS_WGSL })
        .getCompilationInfo(),
      shaderErrors = info.messages
        .filter((m) => m.type === "error")
        .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const color = device.createTexture({
        size: [64, 64],
        format: "rgba16float",
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      }),
      resolved = device.createTexture({
        size: [64, 64],
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      }),
      depth = device.createTexture({
        size: [64, 64],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      }),
      colorView = color.createView(),
      resolvedView = resolved.createView(),
      depthView = depth.createView(),
      readback = device.createBuffer({
        size: 256,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
    const vp = perspective(Math.PI / 2, 1, 0.1, 10),
      inv = inverse(vp),
      base = [0.2, 0.1, 0.05],
      saved = globalThis.fetch,
      maps = new Map();
    globalThis.fetch = async (url) => {
      const b = maps.get(String(url));
      if (!b) throw Error("Unexpected synthetic fetch");
      return new Response(b.slice(0));
    };
    const hash = async (b) =>
        [...new Uint8Array(await crypto.subtle.digest("SHA-256", b))]
          .map((x) => x.toString(16).padStart(2, "0"))
          .join(""),
      half = (n) => {
        const sign = n & 32768 ? -1 : 1,
          e = (n >> 10) & 31,
          m = n & 1023;
        return e === 0
          ? sign * m * 2 ** -24
          : e === 31
            ? m
              ? NaN
              : sign * Infinity
            : sign * 2 ** (e - 15) * (1 + m / 1024);
      };
    let maxError = 0;
    const cases = [];
    try {
      for (const name of [
        "source",
        "double-brightness",
        "half-density",
        "zero-mask",
        "near-fade",
        "opaque-before",
        "depth-fade",
        "extinction",
        "backside",
        "no-night",
        "soft-boundary",
        "soft-midpoint",
        "soft-interior",
        "soft-double-brightness",
        "face-floor",
        "reverse0",
      ]) {
        const m = fogManifestFixture(),
          card = m.scenarios.day.cards[0];
        card.thickestDistance = 0;
        card.transitionDistance = 1;
        card.depthFadeDistance = 1;
        if (name === "double-brightness") card.brightness *= 2;
        if (name === "half-density") card.intensity *= 0.5;
        if (name === "near-fade") card.thickestDistance = 3;
        if (name === "backside") card.model[0] *= -1;
        const soft = name.startsWith("soft-"),
          edgeUV =
            name === "soft-boundary"
              ? 0
              : name === "soft-midpoint"
                ? 0.0025
                : 0.005,
          fov = name === "face-floor" ? 0.01 : Math.PI / 2,
          selectedVP =
            name === "face-floor" ? perspective(fov, 1, 0.1, 10) : vp;
        if (soft) {
          m.edgeTreatment = "owned-soft-border-v1";
          card.model[0] = 128;
          card.model[12] = (2 * Math.tan(fov / 2)) / 64 - (edgeUV - 0.5) * 128;
        }
        if (name === "soft-double-brightness") card.brightness *= 2;
        const mask = new Uint8Array([name === "zero-mask" ? 0 : 255]),
          detail = new Uint8Array([255]);
        maps.set(m.textures.mask.url, mask);
        maps.set(m.textures.detail.url, detail);
        m.textures.mask.sha256 = await hash(mask);
        m.textures.detail.sha256 = await hash(detail);
        const pass = await SourceFogCards.create(device, m),
          encoder = device.createCommandEncoder(),
          opaqueDistance =
            name === "opaque-before" ? 1.5 : name === "depth-fade" ? 2.5 : 1e6,
          clearDepth =
            opaqueDistance === 1e6 ? 1 : 10 / 9.9 - 1 / (9.9 * opaqueDistance),
          density = name === "extinction" ? 0.2 : 0,
          clear = encoder.beginRenderPass({
            colorAttachments: [
              {
                view: colorView,
                resolveTarget: resolvedView,
                clearValue: [...base, 1],
                loadOp: "clear",
                storeOp: "store",
              },
            ],
            depthStencilAttachment: {
              view: depthView,
              depthClearValue: clearDepth,
              depthLoadOp: "clear",
              depthStoreOp: "store",
            },
          });
        clear.end();
        const frame = {
          mode: name === "no-night" ? "night" : "day",
          time: 0,
          viewProjection: selectedVP,
          inverseViewProjection: selectedVP === vp ? inv : inverse(selectedVP),
          eye: [0, 0, 0],
          forward: [0, 0, -1],
          width: 64,
          height: 64,
          fogMedium: [density, 0, 0, 0],
          volumeMedium: [0, 0, 0, 0],
          volumeRange: [0, 0],
          color: colorView,
          resolve: resolvedView,
          depth: depthView,
        };
        const rendered = pass.render(encoder, frame);
        encoder.copyTextureToBuffer(
          { texture: resolved, origin: [32, 32] },
          { buffer: readback, bytesPerRow: 256 },
          [1, 1],
        );
        device.queue.submit([encoder.finish()]);
        await readback.mapAsync(GPUMapMode.READ);
        const rgba = [
          ...new Uint16Array(readback.getMappedRange().slice(0, 8)),
        ].map(half);
        readback.unmap();
        const offset = (2 * Math.tan(fov / 2)) / 64,
          distance = Math.hypot(2, offset, offset),
          edge = fogCardEdgeFactor([soft ? edgeUV : 0.5, 0.5], m.edgeTreatment),
          sample = fogCardSample(
            card,
            mask[0] / 255,
            1,
            2,
            opaqueDistance,
            2 / distance,
          ),
          expected = base.map(
            (n, i) =>
              n +
              (name === "no-night" || opaqueDistance < 2
                ? 0
                : sample.emission[i] *
                  sample.alpha *
                  edge *
                  0.01 *
                  Math.exp(-density * distance)),
          );
        expected.forEach(
          (n, i) => (maxError = Math.max(maxError, Math.abs(n - rgba[i]))),
        );
        if (!rgba.every(Number.isFinite)) throw Error("Nonfinite fog output");
        cases.push({ name, rendered, rgb: rgba.slice(0, 3), expected });
        pass.dispose();
        pass.dispose();
      }
      const empty = await SourceFogCards.create(device);
      if (empty.has("day") || empty.bytes !== 0)
        throw Error("Absent profile allocated fog resources");
      empty.dispose();
      const bad = fogManifestFixture();
      bad.textures.mask.sha256 = await hash(maps.get(bad.textures.mask.url));
      let rejected = false;
      try {
        await SourceFogCards.create(device, bad);
      } catch {
        rejected = true;
      }
      if (!rejected) throw Error("Changed fog texture was accepted");
      await device.queue.onSubmittedWorkDone();
      return { shaderErrors, errors, cases, maxError };
    } finally {
      globalThis.fetch = saved;
      readback.destroy();
      color.destroy();
      resolved.destroy();
      depth.destroy();
      device.destroy();
    }
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases.length, 16);
  assert.ok(result.maxError < 0.0005, JSON.stringify(result));
  assert.deepEqual(result.cases[0].rgb, result.cases.at(-1).rgb);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
