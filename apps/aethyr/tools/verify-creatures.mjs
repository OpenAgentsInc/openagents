// Original asset-free mesh, lighting, MSAA coverage and occlusion qualification.
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
  res.end("<!doctype html><title>Owned creature qualification</title>");
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
    const { Creatures, WGSL_CREATURES } =
      await import("/packages/renderer/creatures.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const module = device.createShaderModule({ code: WGSL_CREATURES }),
      shaderErrors = (await module.getCompilationInfo()).messages
        .filter((m) => m.type === "error")
        .map((m) => m.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const vertices = new Float32Array([
        -0.8, -0.8, 0.4, 0, 0, -1, 0, 0, 0.8, -0.8, 0.4, 0, 0, -1, 1, 0, 0, 0.8,
        0.4, 0, 0, -1, 0.5, 1,
      ]),
      indices = new Uint32Array([0, 1, 2]);
    const hash = async (bytes) =>
        [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
          .map((v) => v.toString(16).padStart(2, "0"))
          .join(""),
      vhash = await hash(vertices.buffer),
      ihash = await hash(indices.buffer);
    const manifest = (alpha) => ({
      schema: "aethyrforge.creatures.v1",
      geometry: {
        vertices: {
          url: "/private-assets/owned-v.bin",
          sha256: vhash,
          byteLength: vertices.byteLength,
        },
        indices: {
          url: "/private-assets/owned-i.bin",
          sha256: ihash,
          byteLength: indices.byteLength,
        },
        vertexCount: 3,
        indexCount: 3,
      },
      meshes: [
        {
          id: "owned-triangle",
          firstIndex: 0,
          indexCount: 3,
          doubleSided: true,
          opacityCutoff: 0.3333,
          flap: { amplitude: 0, uvMultiplier: 1, frequency: [1, 1] },
        },
      ],
      profiles: [
        {
          id: "owned-bird",
          kind: "seagull",
          mesh: "owned-triangle",
          spawnRate: 2,
          spawnProbability: 1,
          lifetime: [2, 2],
          scale: [1, 1],
          sphereRadius: 0,
          velocityMin: [0, 0, 0],
          velocityMax: [0, 0, 0],
          color: [1, 1, 1],
          alphaCurve: [{ time: 0, value: 1 }],
          alphaMultiplierCurve: [{ time: 0, value: 1 }],
          scaleCurve: [{ time: 0, value: 1 }],
          ditherCurve: [{ time: 0, value: alpha }],
          drag: 0,
          vortexStrength: 0,
          noiseStrength: 0,
          noiseFrequency: 0,
          emission: {
            activeSeconds: 1,
            delay: [0, 0],
            delayFirstLoopOnly: false,
            recalculateDelay: false,
          },
        },
      ],
      scenarios: {
        day: [
          {
            id: "owned",
            profile: "owned-bird",
            model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            lifecycle: { startSeconds: 0, warmupSeconds: 0, clock: "world" },
          },
        ],
        night: [],
      },
    });
    const load = async (url) =>
      url.endsWith("owned-v.bin") ? vertices.buffer : indices.buffer;
    const half = (n) => {
      const sign = n & 0x8000 ? -1 : 1,
        e = (n >>> 10) & 31,
        m = n & 1023;
      return (
        sign *
        (e === 0
          ? (Math.pow(2, -14) * m) / 1024
          : e === 31
            ? Infinity
            : Math.pow(2, e - 15) * (1 + m / 1024))
      );
    };
    const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      cases = [];
    const occluder = device.createShaderModule({
        code: "@vertex fn vs(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let p=array<vec2<f32>,6>(vec2(-1.,-1.),vec2(0.,-1.),vec2(-1.,1.),vec2(-1.,1.),vec2(0.,-1.),vec2(0.,1.));return vec4(p[i],.2,1.);}@fragment fn fs()->@location(0)vec4<f32>{return vec4(0.);}",
      }),
      occlusionPipeline = await device.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module: occluder, entryPoint: "vs" },
        fragment: {
          module: occluder,
          entryPoint: "fs",
          targets: [{ format: "rgba16float", writeMask: 0 }],
        },
        primitive: { topology: "triangle-list" },
        depthStencil: {
          format: "depth24plus",
          depthWriteEnabled: true,
          depthCompare: "always",
        },
        multisample: { count: 4 },
      });
    for (const [label, alpha, depthClear, occlusion, sun, sh] of [
      ["direct", 1, 1, false, 1, 0],
      ["double-direct", 1, 1, false, 2, 0],
      ["hidden", 1, 0.2, false, 1, 0],
      ["alpha-half", 0.5, 1, false, 1, 0],
      ["half-occluded", 1, 1, true, 1, 0],
      ["sh-only", 1, 1, false, 0, 1],
    ]) {
      device.pushErrorScope("validation");
      const creatures = await Creatures.create(device, manifest(alpha), load),
        color = device.createTexture({
          size: [64, 64],
          sampleCount: 4,
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        }),
        resolved = device.createTexture({
          size: [64, 64],
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        }),
        depth = device.createTexture({
          size: [64, 64],
          sampleCount: 4,
          format: "depth24plus",
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        }),
        read = device.createBuffer({
          size: 64 * 512,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: color.createView(),
              resolveTarget: resolved.createView(),
              loadOp: "clear",
              storeOp: "discard",
              clearValue: [0.2, 0.2, 0.2, 1],
            },
          ],
          depthStencilAttachment: {
            view: depth.createView(),
            depthClearValue: depthClear,
            depthLoadOp: "clear",
            depthStoreOp: "discard",
          },
        });
      if (occlusion) {
        pass.setPipeline(occlusionPipeline);
        pass.draw(6);
      }
      const coeff = new Float32Array(36);
      coeff[0] = sh / 0.2820947918;
      coeff[1] = (sh * 0.5) / 0.2820947918;
      coeff[2] = (sh * 0.25) / 0.2820947918;
      const stats = creatures.render(pass, {
        mode: "day",
        time: 0.5,
        viewProjection: identity,
        eye: [0, 0, -1],
        sunDirection: [0, 0, -1],
        sunIlluminance: [
          Math.PI * sun,
          Math.PI * 0.5 * sun,
          Math.PI * 0.25 * sun,
        ],
        diffuseRotation: identity,
        diffuseSH: coeff,
      });
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: resolved },
        { buffer: read, bytesPerRow: 512 },
        [64, 64],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const values = new Uint16Array(read.getMappedRange()),
        sample = (x, y) =>
          [0, 1, 2, 3].map((c) => half(values[(y * 64 + x) * 4 + c])),
        center = sample(32, 32),
        left = sample(22, 32),
        right = sample(42, 32);
      let partial = 0;
      for (let y = 0; y < 64; y++)
        for (let x = 0; x < 64; x++) {
          const r = sample(x, y)[0];
          if (r > 0.011 && r < 0.19) partial++;
        }
      read.unmap();
      const validation = await device.popErrorScope();
      cases.push({
        label,
        center,
        left,
        right,
        partial,
        count: stats.count,
        bytes: creatures.bytes,
        validation: validation?.message,
      });
      creatures.dispose();
      [color, resolved, depth].forEach((t) => t.destroy());
      read.destroy();
    }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return { cases, errors, shaderErrors };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  for (const row of result.cases) {
    assert.equal(row.validation, undefined, row.validation);
    assert.equal(row.count, 1);
    assert.equal(row.bytes, 96 + 12 + 256 * 80 + 400);
  }
  const by = Object.fromEntries(result.cases.map((c) => [c.label, c])),
    near = (a, b) => assert.ok(Math.abs(a - b) < 0.0002, `${a} != ${b}`);
  near(by.direct.center[0], 0.01);
  near(by.direct.center[1], 0.005);
  near(by.direct.center[2], 0.0025);
  near(by["double-direct"].center[0], 0.02);
  near(by.hidden.center[0], 0.2);
  near(by["alpha-half"].center[0], 0.105);
  near(by["half-occluded"].left[0], 0.2);
  near(by["half-occluded"].right[0], 0.01);
  near(by["sh-only"].center[0], 0.01);
  assert.ok(by.direct.partial > 0);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
