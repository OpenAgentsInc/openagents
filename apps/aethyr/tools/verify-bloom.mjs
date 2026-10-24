// Owned impulse/threshold GPU qualification. No licensed inputs or account access.
// Run: openagents browser run -- node tools/verify-bloom.mjs
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
  response.end("<!doctype html><title>Owned bloom qualification</title>");
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
    const { Bloom, BLOOM_WEIGHTS, BLOOM_WGSL } =
      await import("/packages/renderer/bloom.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU is required.");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (event) =>
      errors.push(event.error.message),
    );
    const info = await device
      .createShaderModule({ code: BLOOM_WGSL })
      .getCompilationInfo();
    const shaderErrors = info.messages
      .filter((m) => m.type === "error")
      .map((m) => `${m.lineNum}: ${m.message}`);
    if (shaderErrors.length) return { shaderErrors, errors };
    const bloom = await Bloom.create(device);
    const copyModule = device.createShaderModule({
      code: `@group(0)@binding(0)var image:texture_2d<f32>;@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.-1.,0.,1.);}@fragment fn fs(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{return textureLoad(image,vec2<i32>(p.xy),0);}`,
    });
    const copy = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: copyModule, entryPoint: "vs" },
      fragment: {
        module: copyModule,
        entryPoint: "fs",
        targets: [{ format: "rgba32float" }],
      },
    });
    const half = new Map([
      [0, 0],
      [0.5, 0x3800],
      [1, 0x3c00],
      [1.5, 0x3e00],
      [2, 0x4000],
      [3, 0x4200],
      [5, 0x4500],
      [9, 0x4880],
    ]);
    const image = (width, height, color = [0, 0, 0]) => {
      const data = new Uint16Array(width * height * 4);
      for (let i = 0; i < width * height; i++)
        data.set([...color.map((v) => half.get(v)), 0x3c00], i * 4);
      return data;
    };
    async function run(width, height, data, enabled = true) {
      const source = device.createTexture({
        size: [width, height],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      device.queue.writeTexture(
        { texture: source },
        data,
        { bytesPerRow: width * 8 },
        [width, height],
      );
      const encoder = device.createCommandEncoder();
      bloom.render(encoder, {
        width,
        height,
        source: source.createView(),
        enabled,
      });
      const [w, h] = bloom.size,
        row = Math.ceil((w * 16) / 256) * 256;
      const output = device.createTexture({
        size: [w, h],
        format: "rgba32float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          { view: output.createView(), loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(copy);
      pass.setBindGroup(
        0,
        device.createBindGroup({
          layout: copy.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: bloom.view }],
        }),
      );
      pass.draw(3);
      pass.end();
      const read = device.createBuffer({
        size: row * h,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      encoder.copyTextureToBuffer(
        { texture: output },
        { buffer: read, bytesPerRow: row },
        [w, h],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const raw = new Float32Array(read.getMappedRange()),
        pixels = [];
      for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++)
          pixels.push([
            ...raw.slice((y * row) / 4 + x * 4, (y * row) / 4 + x * 4 + 3),
          ]);
      read.unmap();
      read.destroy();
      source.destroy();
      output.destroy();
      if (!pixels.flat().every(Number.isFinite))
        throw Error("Bloom produced a non-finite pixel.");
      return { width: w, height: h, pixels };
    }
    const close = (a, b, message, tolerance = 0.002) => {
      if (Math.abs(a - b) > tolerance) throw Error(`${message}: ${a} != ${b}`);
    };
    const cases = [];
    for (const [input, expected] of [
      [0.5, 0],
      [1, 0],
      [3, 2],
    ]) {
      const out = await run(64, 64, image(64, 64, [input, input, input]));
      out.pixels.forEach((p) =>
        p.forEach((v) => close(v, expected, "constant threshold")),
      );
      cases.push(`constant ${input}`);
    }
    const channels = await run(64, 64, image(64, 64, [0.5, 3, 1]));
    channels.pixels.forEach((p) =>
      p.forEach((v, i) => close(v, [0, 2, 0][i], "channel threshold")),
    );
    cases.push("chromatic threshold");
    const impulse = image(64, 64);
    impulse.set(
      [half.get(9), half.get(5), half.get(3), 0x3c00],
      (33 * 64 + 33) * 4,
    );
    const out = await run(64, 64, impulse),
      sum = [0, 0, 0];
    let positive = 0;
    for (let y = 0; y < 32; y++)
      for (let x = 0; x < 32; x++) {
        const p = out.pixels[y * 32 + x],
          dx = Math.abs(x - 16),
          dy = Math.abs(y - 16),
          weight =
            dx <= 4 && dy <= 4 ? BLOOM_WEIGHTS[dx] * BLOOM_WEIGHTS[dy] : 0;
        p.forEach((v, i) => {
          sum[i] += v;
          close(v, [2, 1, 0.5][i] * weight, "contiguous impulse", 0.00012);
        });
        if (p[0] > 0) positive++;
      }
    if (positive !== 81)
      throw Error("An impulse did not fill the complete 9×9 footprint.");
    sum.forEach((v, i) =>
      close(v, [2, 1, 0.5][i], "normalized impulse energy", 0.003),
    );
    for (let d = 1; d <= 4; d++) {
      const left = out.pixels[16 * 32 + 16 - d][0],
        right = out.pixels[16 * 32 + 16 + d][0],
        previous = out.pixels[16 * 32 + 17 - d][0];
      close(left, right, "symmetric impulse", 0.000001);
      if (!(left > 0 && left < previous))
        throw Error("Impulse decay contains detached ghosts.");
    }
    cases.push(
      "contiguous symmetric monotonic impulse",
      "normalized impulse energy",
    );
    const phase = image(64, 64);
    phase.set(
      [half.get(9), half.get(5), half.get(3), 0x3c00],
      (32 * 64 + 32) * 4,
    );
    const shifted = await run(64, 64, phase);
    shifted.pixels.forEach((p, i) =>
      p.forEach((v, c) =>
        close(v, out.pixels[i][c], "2×2 impulse phases", 0.000001),
      ),
    );
    cases.push("2×2 subpixel phases");
    for (let i = 0; i < 2; i++) {
      const disabled = await run(64, 64, image(64, 64, [9, 9, 9]), false);
      disabled.pixels.forEach((p) =>
        p.forEach((v) => close(v, 0, "disabled history", 0)),
      );
    }
    cases.push("disable clears history");
    const resumed = await run(64, 64, image(64, 64, [3, 3, 3]));
    resumed.pixels.forEach((p) =>
      p.forEach((v) => close(v, 2, "reenabled bloom")),
    );
    cases.push("reenable");
    const odd = await run(19, 11, image(19, 11, [3, 3, 3]));
    odd.pixels.forEach((p) =>
      p.forEach((v) => close(v, 2, "odd edge coverage")),
    );
    cases.push("odd resize and edges");
    const w = 1290,
      h = 722,
      capped = image(w, h);
    capped.set([half.get(9), 0, 0, 0x3c00], (361 * w + 645) * 4);
    const cap = await run(w, h, capped),
      total = cap.pixels.reduce((s, p) => s + p[0], 0),
      expected = 8 / (w / cap.width) / (h / cap.height);
    close(total, expected, "capped box preserves an impulse", 0.005);
    if (cap.width > 640 || cap.height > 360)
      throw Error("Bloom allocation exceeded its cap.");
    cases.push("capped footprint impulse");
    await device.queue.onSubmittedWorkDone();
    const bytes = bloom.bytes;
    bloom.dispose();
    bloom.dispose();
    device.destroy();
    return {
      cases,
      impulsePositiveTexels: positive,
      impulseEnergy: sum,
      cappedImpulse: { actual: total, expected },
      bytes,
      shaderErrors,
      errors,
    };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases.length, 11);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
