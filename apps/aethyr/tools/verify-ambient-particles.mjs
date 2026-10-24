// Original fixtures only. Run: openagents browser run -- node tools/verify-ambient-particles.mjs
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
    "<!doctype html><title>Owned ambient particle qualification</title>",
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
    const { AmbientParticles } =
      await import("/packages/renderer/ambient-particles.ts");
    const { POST } = await import("/packages/renderer/shaders.ts");
    const { Bloom } = await import("/packages/renderer/bloom.ts");
    const { DisplaySharpen } =
      await import("/packages/renderer/display-sharpen.ts");
    const { prepareFilmic, DEFAULT_FILMIC_CONTROLS } =
      await import("/packages/renderer/filmic.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const atlas = new ArrayBuffer(16 + 8 * 8 * 4),
      header = new DataView(atlas);
    header.setUint32(0, 0x41465238, false);
    header.setUint32(4, 1, true);
    header.setUint32(8, 8, true);
    header.setUint32(12, 8, true);
    const texels = new Uint8Array(atlas, 16);
    texels.fill(255);
    texels[63 * 4 + 3] = 0;
    const digest = [
      ...new Uint8Array(await crypto.subtle.digest("SHA-256", atlas)),
    ]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("");
    const fixture = (kind, alpha = 0.5, phase = 0) => ({
      schema: "aethyrforge.ambient-particles.v1",
      profiles: [
        {
          id: "owned",
          kind,
          spawnRate: 1,
          lifetime: [1, 1],
          size: [1, 1],
          velocityMin: [0, 0, 0],
          velocityMax: [0, 0, 0],
          acceleration: [0, 0, 0],
          drag: [0, 0],
          color: [1, 1, 1],
          emission: 2,
          sphereRadius: 0,
          boxSize: [0, 0, 0],
          shapeOffset: [0, 0, 0],
          noiseStrength: 0,
          noiseFrequency: 0,
          alphaCurve: [{ time: 0, value: alpha }],
          sizeCurve: [{ time: 0, value: 1 }],
          phaseCurve: [{ time: 0, value: phase }],
          depthFade: kind === "smoke" ? 0.5 : 0,
          tint: [1, 1, 1],
          colorPower: 0.5,
        },
      ],
      scenarios: {
        day: [
          {
            id: "original-fixture",
            profile: "owned",
            model: [...identity.slice(0, 12), 0, 0, 0.5, 1],
          },
        ],
        night: [],
      },
      atlas: {
        url: "/private-assets/owned.bin",
        sha256: digest,
        byteLength: atlas.byteLength,
        width: 8,
        height: 8,
        grid: [8, 8],
        colorSpace: "srgb",
      },
    });
    const half = (v) => {
      const s = v & 0x8000 ? -1 : 1,
        e = (v >> 10) & 31,
        m = v & 1023;
      return (
        s *
        (e === 0
          ? (2 ** -14 * m) / 1024
          : e === 31
            ? m
              ? NaN
              : Infinity
            : 2 ** (e - 15) * (1 + m / 1024))
      );
    };
    const background = [0.2, 0.4, 0.6, 1],
      cases = [];
    let checks = 0,
      maxError = 0;
    const postModule = device.createShaderModule({ code: POST });
    const inactiveLut = device.createTexture({
      dimension: "3d",
      size: [1, 1, 1],
      format: "rgba16float",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    device.queue.writeTexture(
      { texture: inactiveLut },
      new Uint16Array([0, 0, 0, 0x3c00]),
      { bytesPerRow: 8 },
      [1, 1, 1],
    );
    const lutSampler = device.createSampler({
      minFilter: "linear",
      magFilter: "linear",
    });
    const postPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: postModule, entryPoint: "vs" },
      fragment: {
        module: postModule,
        entryPoint: "fs",
        targets: [{ format: "rgba16float" }],
      },
    });
    const close = (actual, expected, name) => {
      for (let i = 0; i < 4; i++) {
        const delta = Math.abs(actual[i] - expected[i]);
        if (!Number.isFinite(actual[i]) || delta > 0.0015)
          throw new Error(
            `${name}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`,
          );
        maxError = Math.max(maxError, delta);
        checks++;
      }
    };
    async function render(
      recipe,
      {
        exposure = 1,
        depth = 1,
        time = 0,
        fog,
        enabled,
        width = 32,
        height = 32,
        clear = background,
        projection = identity,
        inverse = identity,
        post = false,
        bloom = 0,
      } = {},
    ) {
      const effect = await AmbientParticles.create(
        device,
        recipe,
        async () => atlas,
      );
      const target = device.createTexture({
        size: [width, height],
        format: "rgba16float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT |
          GPUTextureUsage.COPY_SRC |
          GPUTextureUsage.TEXTURE_BINDING,
      });
      const opaque = device.createTexture({
        size: [width, height],
        format: "depth24plus",
        sampleCount: 4,
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      const rowBytes = Math.ceil((width * 8) / 256) * 256;
      const readback = device.createBuffer({
        size: rowBytes * height,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      const encoder = device.createCommandEncoder();
      const clearDepth = encoder.beginRenderPass({
        colorAttachments: [],
        depthStencilAttachment: {
          view: opaque.createView(),
          depthLoadOp: "clear",
          depthStoreOp: "store",
          depthClearValue: depth,
        },
      });
      clearDepth.end();
      const clearColor = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: target.createView(),
            loadOp: "clear",
            storeOp: "store",
            clearValue: clear,
          },
        ],
      });
      clearColor.end();
      const stats = effect.render(encoder, {
        mode: "day",
        time,
        width,
        height,
        colorView: target.createView(),
        depthView: opaque.createView(),
        viewProjection: projection,
        inverseViewProjection: inverse,
        eye: [0, 0, -1],
        effectiveExposure: exposure,
        fog,
        enabled,
      });
      let display, sharpen, postSettings, displayReadback, bloomPass;
      const displayRowBytes = Math.ceil((width * 4) / 256) * 256;
      if (post) {
        bloomPass = await Bloom.create(device);
        bloomPass.resize(width, height);
        bloomPass.render(encoder, {
          width,
          height,
          source: target.createView(),
          enabled: bloom > 0,
        });
        sharpen = await DisplaySharpen.create(device, "rgba8unorm");
        sharpen.resize(width, height);
        postSettings = device.createBuffer({
          size: 128,
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        device.queue.writeBuffer(
          postSettings,
          0,
          new Float32Array([
            exposure / 0.01,
            1,
            bloom,
            1,
            ...prepareFilmic({ ...DEFAULT_FILMIC_CONTROLS, whiteClip: 1 })
              .packed,
            1,
            2 ** -10,
            64,
            0,
          ]),
        );
        const group = device.createBindGroup({
          layout: postPipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: target.createView() },
            {
              binding: 1,
              resource: device.createSampler({
                minFilter: "linear",
                magFilter: "linear",
              }),
            },
            { binding: 2, resource: { buffer: postSettings } },
            { binding: 3, resource: bloomPass.view },
            {
              binding: 4,
              resource: inactiveLut.createView({ dimension: "3d" }),
            },
            { binding: 5, resource: lutSampler },
          ],
        });
        const tone = encoder.beginRenderPass({
          colorAttachments: [
            { view: sharpen.view, loadOp: "clear", storeOp: "store" },
          ],
        });
        tone.setPipeline(postPipeline);
        tone.setBindGroup(0, group);
        tone.draw(3);
        tone.end();
        display = device.createTexture({
          size: [width, height],
          format: "rgba8unorm",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        });
        sharpen.render(encoder, display.createView(), 1);
        displayReadback = device.createBuffer({
          size: displayRowBytes * height,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
        encoder.copyTextureToBuffer(
          { texture: display },
          { buffer: displayReadback, bytesPerRow: displayRowBytes },
          [width, height],
        );
      }
      encoder.copyTextureToBuffer(
        { texture: target },
        { buffer: readback, bytesPerRow: rowBytes },
        [width, height],
      );
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const raw = new Uint16Array(readback.getMappedRange()),
        centerAt =
          (Math.floor(height / 2) * rowBytes + Math.floor(width / 2) * 8) / 2;
      const output = [...raw.slice(centerAt, centerAt + 4)].map(half);
      let hdr, displayPixels;
      if (post) {
        hdr = new Float32Array(width * height * 4);
        for (let y = 0; y < height; y++)
          for (let x = 0; x < width * 4; x++)
            hdr[y * width * 4 + x] = half(raw[(y * rowBytes) / 2 + x]);
      }
      readback.unmap();
      if (displayReadback) {
        await displayReadback.mapAsync(GPUMapMode.READ);
        const bytes = new Uint8Array(displayReadback.getMappedRange());
        displayPixels = new Uint8Array(width * height * 4);
        for (let y = 0; y < height; y++)
          displayPixels.set(
            bytes.subarray(
              y * displayRowBytes,
              y * displayRowBytes + width * 4,
            ),
            y * width * 4,
          );
        displayReadback.unmap();
      }
      effect.dispose();
      target.destroy();
      opaque.destroy();
      readback.destroy();
      display?.destroy();
      displayReadback?.destroy();
      postSettings?.destroy();
      sharpen?.dispose();
      bloomPass?.dispose();
      return { output, stats, hdr, display: displayPixels };
    }
    const radial =
      1 -
      Math.exp(
        -((2.333 * Math.max(0, 1 - Math.hypot(0.5 / 16, 0.5 / 16) / 0.5)) ** 2),
      );
    const dust = await render(fixture("dust"));
    if (dust.stats.count !== 1 || dust.stats.dust !== 1)
      throw new Error(
        "The actual sampler did not produce one original fixture particle.",
      );
    close(
      dust.output,
      background.map((v, i) =>
        i < 3 ? v * (1 - 0.5 * radial) + 0.02 * 0.5 * radial * radial : 1,
      ),
      "dust translucent opacity and independent emissive mask",
    );
    cases.push("translucent dust");
    const dustBright = await render(fixture("dust"), { exposure: 2 });
    close(dustBright.output, dust.output, "dust is scene-linear");
    cases.push("dust exposure");
    const smoke = await render(fixture("smoke"));
    close(
      smoke.output,
      background.map((v, i) => (i < 3 ? v * 0.5 + 0.02 * 0.5 * 0.5 : 1)),
      "source smoke masks emissive before translucent composition",
    );
    cases.push("premultiplied smoke");
    const smokeBright = await render(fixture("smoke"), { exposure: 2 });
    close(
      smokeBright.output,
      background.map((v, i) => (i < 3 ? v * 0.5 + 0.01 * 0.5 * 0.5 : 1)),
      "smoke exposure compensation",
    );
    cases.push("smoke exposure");
    for (const kind of ["dust", "smoke"]) {
      const hidden = await render(fixture(kind), { depth: 0.4 });
      close(hidden.output, background, kind + " opaque occlusion");
      cases.push(kind + " occlusion");
    }
    const fade = await render(fixture("smoke"), { depth: 0.75 });
    const distance = Math.hypot(0.03125, 0.03125, 1.5),
      opaqueDistance = Math.hypot(0.03125, 0.03125, 1.75),
      alpha = (0.5 * (opaqueDistance - distance)) / 0.5;
    close(
      fade.output,
      background.map((v, i) =>
        i < 3 ? v * (1 - alpha) + 0.02 * alpha * alpha : 1,
      ),
      "half-meter soft depth fade",
    );
    cases.push("soft intersection");
    const lastFrame = await render(fixture("smoke", 0.5, 1));
    close(lastFrame.output, background, "last atlas frame alpha");
    cases.push("atlas phase");
    const replay = await render(fixture("smoke"), { time: 86400 });
    close(replay.output, smoke.output, "bounded deterministic replay");
    cases.push("replay");
    const off = await render(fixture("dust"), { enabled: false });
    close(off.output, background, "disabled effects");
    cases.push("disable");
    const fogged = await render(fixture("dust"), {
      fog: { density: 1, heightRate: 0, color: [0, 0, 0] },
    });
    close(
      fogged.output,
      background.map((v, i) =>
        i < 3
          ? v * (1 - 0.5 * radial) +
            0.02 * 0.5 * radial * radial * Math.exp(-distance)
          : 1,
      ),
      "fog attenuates dust without adding a light",
    );
    cases.push("fog attenuation");
    const volume = {
      density: 1,
      heightRate: 0,
      volumeMedium: [2, 0, 1, 0],
      volumeRange: [60, 1],
    };
    const noNearFill = await render(fixture("dust"), {
      fog: { ...volume, color: [0, 0, 0] },
    });
    const brightNearFill = await render(fixture("dust"), {
      fog: { ...volume, color: [5, 10, 20] },
    });
    close(
      brightNearFill.output,
      noNearFill.output,
      "near volume replaces constant particle fog color",
    );
    cases.push("volume replacement has no duplicate analytic fill");
    const invalid = fixture("smoke");
    invalid.atlas.sha256 = "0".repeat(64);
    let rejected = false;
    try {
      await AmbientParticles.create(device, invalid, async () => atlas);
    } catch {
      rejected = true;
    }
    if (!rejected) throw new Error("A changed private atlas was accepted.");
    cases.push("atlas integrity");
    // Real source intensity, original synthetic geometry, actual tone-map and
    // after-tone sharpen modules. Low-gain center samples miss HDR edge leaks.
    const width = 1280,
      height = 128,
      projection = [...identity],
      inverse = [...identity];
    projection[0] = height / width;
    inverse[0] = width / height;
    const sourceRGB = [
      0.5052083134651184, 0.47644615173339844, 0.2188926488161087,
    ];
    const hdrFixture = (diameter, phaseX = 0, phaseY = 0, alpha = 0.65) => {
      const m = fixture("dust", alpha);
      m.profiles[0].emission = 50000;
      m.profiles[0].color = sourceRGB;
      m.profiles[0].size = [(diameter * 2) / height, (diameter * 2) / height];
      m.scenarios.day[0].model[12] = (2 * (0.5 + phaseX)) / height;
      m.scenarios.day[0].model[13] = (-2 * (0.5 + phaseY)) / height;
      return m;
    };
    const sourceOptions = {
      width,
      height,
      projection,
      inverse,
      exposure: 0.125,
      clear: [0, 0, 0, 1],
      post: true,
    };
    const big = await render(hdrFixture(64), sourceOptions);
    const energyAt = (image, x, y) => image[(y * width + x) * 4];
    for (const dx of [-27, 27])
      for (const dy of [-27, 27]) {
        if (
          energyAt(big.hdr, 640 + dx, 64 + dy) !== 0 ||
          energyAt(big.display, 640 + dx, 64 + dy) !== 0
        )
          throw new Error(
            "A 50000-emission dust quad has a visible corner after tone mapping and sharpening.",
          );
        checks += 2;
      }
    let previous = Infinity;
    for (let x = 640; x <= 672; x++) {
      const v = energyAt(big.hdr, x, 64);
      if (v > previous + 0.001)
        throw new Error(
          "Resolved HDR radial energy increased toward its boundary.",
        );
      previous = v;
      checks++;
    }
    if (previous !== 0)
      throw new Error("Resolved HDR radial support did not reach zero.");
    cases.push("50000 HDR finite support through filmic and sharpen");
    // Independent midpoint integration of the recovered expression graph.
    let integrated = 0;
    for (let i = 0; i < 20000; i++) {
      const r = ((i + 0.5) * 0.5) / 20000;
      const mask = 1 - Math.exp(-((2.333 * (1 - r / 0.5)) ** 2));
      integrated += (2 * Math.PI * r * mask * mask * 0.5) / 20000;
    }
    const flux = [];
    for (const phase of [
      [0, 0],
      [0.25, 0.4],
      [0.5, 0.75],
      [0.9, 0.1],
    ]) {
      const small = await render(hdrFixture(0.8, ...phase), {
        ...sourceOptions,
        bloom: 0.08,
      });
      let energy = 0;
      for (let i = 0; i < small.hdr.length; i += 4) {
        const v = small.hdr[i];
        if (!Number.isFinite(v) || v < 0)
          throw new Error("Subpixel dust produced invalid HDR energy.");
        energy += v;
      }
      const expected =
        sourceRGB[0] * 50000 * 0.01 * 0.65 * integrated * 0.8 ** 2;
      if (Math.abs(energy - expected) / expected > 0.003)
        throw new Error(
          `Subpixel dust flux ${energy} differs from integrated source mask energy ${expected}.`,
        );
      if (!small.display.some((v, i) => i % 4 < 3 && v > 0))
        throw new Error(
          "Subpixel source-intensity dust disappeared after the actual display pipeline.",
        );
      flux.push(energy);
      checks++;
    }
    const halfAlpha = await render(hdrFixture(0.8, 0, 0, 0.325), sourceOptions);
    let halfEnergy = 0;
    for (let i = 0; i < halfAlpha.hdr.length; i += 4)
      halfEnergy += halfAlpha.hdr[i];
    if (Math.abs(halfEnergy * 2 - flux[0]) / flux[0] > 0.003)
      throw new Error("HDR dust age alpha was applied more than once.");
    cases.push(
      "subpixel HDR flux across camera phases",
      "HDR alpha once with independent radial factors",
    );
    checks++;
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { cases, checks, maxError, subpixelFlux: flux, errors };
  });
  assert.deepEqual(result.errors, []);
  assert.deepEqual(pageErrors, []);
  console.log(JSON.stringify(result));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
