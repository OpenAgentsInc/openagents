// Owned fixtures only. Run: openagents browser run -- node tools/verify-water-splashes.mjs
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
  res.end("<!doctype html><title>Owned fountain foam qualification</title>");
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
    const { SourceWaterSplashes } =
      await import("/packages/renderer/source-water-splashes.ts");
    const { halfToFloat } =
      await import("/packages/renderer/environment-lighting.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [],
      cases = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      background = [0.2, 0.4, 0.6, 1];
    let maximumError = 0;
    const hash = async (bytes) =>
      [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
        .map((v) => v.toString(16).padStart(2, "0"))
        .join("");
    const keys = (v) => [
      { time: 0, value: v },
      { time: 1, value: v },
    ];
    async function render({
      exposure = 1,
      depth = 1,
      time = 1.5,
      noise = 128,
      noiseIndex = 0,
      alpha = 0.4,
      red = 2,
      fog,
      farther = false,
      width = 32,
      diameter = 1,
      minimumFootprintPixels,
      backgroundColor = background,
    } = {}) {
      const bytes = new ArrayBuffer(20),
        header = new DataView(bytes);
      header.setUint32(0, 0x41465238, false);
      header.setUint32(4, 1, true);
      header.setUint32(8, 1, true);
      header.setUint32(12, 1, true);
      new Uint8Array(bytes, 16).set([noise, 0, 0, 255]);
      const second = bytes.slice(0);
      new Uint8Array(second, 16)[0] = 192;
      const blobs = new Map([
        ["/private-assets/a.bin", bytes],
        ["/private-assets/b.bin", second],
      ]);
      const textures = await Promise.all(
        [...blobs].map(async ([url, data], i) => ({
          id: "noise" + i,
          url,
          sha256: await hash(data),
          byteLength: data.byteLength,
          width: 1,
          height: 1,
          colorSpace: "linear",
        })),
      );
      const profile = {
        id: "foam",
        noise: "noise" + noiseIndex,
        spawnRate: 1,
        lifetime: [2, 2],
        diameter: [diameter, diameter],
        velocityStart: [0, 0, 0],
        velocityEnd: [0, 0, 0],
        acceleration: [0, 0, 0],
        drag: 0,
        offset: [0, 0, 0],
        sphereRadius: 0,
        shapeScale: [1, 1, 1],
        sizeCurve: keys(1),
        alphaCurve: keys(alpha),
        redCurve: keys(red),
        uvScale: [1, 1],
        uvSpeed: [0, 0],
        uvFlip: "none",
      };
      const model = [...identity];
      model[14] = 0.5;
      const recipe = {
        schema: "aethyrforge.source-water-splashes.v1",
        textures,
        profiles: [profile],
        material: {
          radialRadius: 0.5,
          radialDensity: 2.333,
          depthFadeMeters: 0.25,
        },
        lifecycle: {
          startSeconds: 0,
          warmupSeconds: 0,
          delaySeconds: 0,
          loop: "continuous",
        },
        scenarios: { day: [{ id: "near", profile: "foam", model }], night: [] },
      };
      if (minimumFootprintPixels !== undefined) {
        recipe.appearanceProfile = "owned-subtle-spray-v1";
        recipe.material.minimumFootprintPixels = minimumFootprintPixels;
      }
      if (farther) {
        const m = [...model];
        m[14] = 0.7;
        recipe.profiles.push({ ...profile, id: "far", redCurve: keys(4) });
        recipe.scenarios.day.push({ id: "far", profile: "far", model: m });
      }
      const effect = await SourceWaterSplashes.create(
        device,
        recipe,
        async (url) => blobs.get(url),
      );
      recipe.profiles[0].redCurve = keys(100); // The installed recipe must be independent.
      const color = device.createTexture({
        size: [width, width],
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const depthTexture = device.createTexture({
        size: [width, width],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      const row = Math.ceil((width * 8) / 256) * 256,
        buffer = device.createBuffer({
          size: row * width,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
      const encoder = device.createCommandEncoder(),
        clear = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: color.createView(),
              clearValue: backgroundColor,
              loadOp: "clear",
              storeOp: "store",
            },
          ],
        });
      clear.end();
      const depthClear = encoder.beginRenderPass({
        colorAttachments: [],
        depthStencilAttachment: {
          view: depthTexture.createView(),
          depthClearValue: depth,
          depthLoadOp: "clear",
          depthStoreOp: "store",
        },
      });
      depthClear.end();
      const stats = effect.render(encoder, {
        mode: "day",
        time,
        width,
        height: width,
        colorView: color.createView(),
        depthView: depthTexture.createView(),
        viewProjection: identity,
        inverseViewProjection: identity,
        eye: [0, 0, -1],
        effectiveExposure: exposure,
        fog,
      });
      encoder.copyTextureToBuffer(
        { texture: color },
        { buffer, bytesPerRow: row },
        [width, width],
      );
      device.queue.submit([encoder.finish()]);
      await buffer.mapAsync(GPUMapMode.READ);
      const at = (Math.floor(width / 2) * row + Math.floor(width / 2) * 8) / 2,
        data = new Uint16Array(buffer.getMappedRange()),
        pixel = [...data.slice(at, at + 4)].map(halfToFloat),
        pixels = [];
      for (let y = 0; y < width; y++)
        for (let x = 0; x < width; x++)
          pixels.push(halfToFloat(data[(y * row + x * 8) / 2]));
      buffer.unmap();
      effect.dispose();
      effect.dispose();
      if (effect.bytes !== 0) throw Error("Disposed foam retains allocations");
      for (const r of [color, depthTexture, buffer]) r.destroy();
      return { pixel, pixels, stats };
    }
    const radial = (width) =>
      1 -
      Math.exp(-((2.333 * (1 - Math.hypot(1 / width, 1 / width) / 0.5)) ** 2));
    function expected({
      width = 32,
      alpha = 0.4,
      noise = 128,
      exposure = 1,
      red = 2,
      fade = 1,
    } = {}) {
      const a = ((alpha * noise) / 255) * radial(width) * fade;
      return background.map((v, i) =>
        i < 3 ? v * (1 - a) + ((red * 0.01) / exposure) * a : 1,
      );
    }
    function close(actual, expected, label, tolerance = 0.0008) {
      const error = Math.max(
        ...actual.map((v, i) => Math.abs(v - expected[i])),
      );
      maximumError = Math.max(maximumError, error);
      if (!actual.every(Number.isFinite) || error > tolerance)
        throw Error(`${label}: ${actual} != ${expected}`);
      cases.push(label);
    }
    for (const [label, options] of [
      ["source linear noise, radial opacity and unlit red/exposure", {}],
      ["exposure compensation applied once", { exposure: 2 }],
      ["linear mask red is not sRGB-decoded", { noise: 16 }],
      ["second source noise texture selected", { noiseIndex: 1 }],
      ["source alpha appears once", { alpha: 0.2 }],
      ["source25cm view-depth fade", { depth: 0.6 }],
      ["resized target", { width: 64 }],
    ]) {
      const { pixel, stats } = await render(options);
      if (stats.drawn !== 1)
        throw Error("Expected one positive-birth particle");
      close(
        pixel,
        expected({
          ...options,
          noise: options.noiseIndex === 1 ? 192 : (options.noise ?? 128),
          fade: options.depth === 0.6 ? 0.4 : 1,
        }),
        label,
      );
    }
    close(
      (await render({ time: 0 })).pixel,
      background,
      "zero warmup has no pre-start particles",
    );
    close(
      (await render({ depth: 0.4 })).pixel,
      background,
      "opaque foreground rejects foam",
    );
    const sorted = await render({ farther: true }),
      a = ((0.4 * 128) / 255) * radial(32),
      far = background.map((v, i) => (i < 3 ? v * (1 - a) + 0.04 * a : 1)),
      layered = far.map((v, i) => (i < 3 ? v * (1 - a) + 0.02 * a : 1));
    close(
      sorted.pixel,
      layered,
      "source foam sorts complete colors back to front",
    );
    const fogged = await render({
      fog: { density: 1, color: [0.003, 0.004, 0.005] },
    });
    if (!fogged.pixel.every(Number.isFinite) || fogged.stats.drawn !== 1)
      throw Error("Fogged foam is nonfinite");
    cases.push("shared analytic fog remains finite");
    const baseline = await render({ time: 0, backgroundColor: [0, 0, 0, 1] }),
      tiny = await render({
        diameter: 0.02,
        minimumFootprintPixels: 2,
        backgroundColor: [0, 0, 0, 1],
      });
    const opacityArea = tiny.pixels.reduce(
      (sum, value, i) => sum + (value - baseline.pixels[i]) / 0.02,
      0,
    );
    // Independent area integral of the continuous radial disk. Physical width
    // is .32pixel; the two-pixel raster expansion must not create a bright disc.
    let integral = 0;
    const steps = 10000;
    for (let i = 0; i < steps; i++) {
      const r = ((i + 0.5) * 0.5) / steps;
      integral +=
        (2 *
          Math.PI *
          r *
          (1 - Math.exp(-((2.333 * (1 - r / 0.5)) ** 2))) *
          0.5) /
        steps;
    }
    const expectedArea = 0.4 * (128 / 255) * integral * ((0.02 * 32) / 2) ** 2,
      areaRelativeError = Math.abs(opacityArea / expectedArea - 1);
    if (
      !Number.isFinite(opacityArea) ||
      opacityArea <= 0 ||
      areaRelativeError > 0.06
    )
      throw Error(
        `Owned subpixel spray changes physical opacity area: ${opacityArea} != ${expectedArea}`,
      );
    cases.push(
      "owned subpixel spray preserves physical opacity area within raster quadrature bounds",
    );
    if (
      Math.max(
        ...tiny.pixels.map((value, i) => Math.abs(value - baseline.pixels[i])),
      ) > 0.005
    )
      throw Error("Owned subpixel spray becomes a large opaque particle");
    cases.push(
      "owned small spray remains translucent after footprint expansion",
    );
    await device.queue.onSubmittedWorkDone();
    if (errors.length) throw Error(errors.join("\n"));
    device.destroy();
    return { cases, maximumError, areaRelativeError, errors };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify(result, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
