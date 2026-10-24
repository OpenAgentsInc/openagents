// Owned synthetic GPU qualification for static surface SH transfer and interpolation.
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
  res.end("<title>Owned surface sky qualification</title>");
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
        SurfaceSky,
        surfaceSkyWGSL,
        evaluateSurfaceSky,
        bakeSurfaceSkyPoint,
      } = await import("/packages/renderer/surface-sky.ts"),
      { surfaceFixture } = await import("/tests/fixtures/surface-sky.ts");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw Error("WebGPU required");
    const device = await adapter.requestDevice(),
      errors = [];
    device.addEventListener("uncapturederror", (e) =>
      errors.push(e.error.message),
    );
    const code =
      surfaceSkyWGSL(16) +
      `
struct Settings{settings:vec4<f32>,normal:vec4<f32>,lookup:vec4<f32>};@group(0)@binding(0)var<uniform>u:Settings;
struct Out{@builtin(position)p:vec4<f32>,@location(0)x0:vec3<f32>,@location(1)x1:vec3<f32>,@location(2)x3:vec2<f32>,@location(3)x4:vec4<f32>,@location(4)x5:vec3<f32>,@location(5)x6:vec2<f32>,@location(6)@interpolate(flat)x7:f32,@location(7)x8:vec3<f32>,@location(8)a:vec4<f32>,@location(9)b:vec4<f32>,@location(10)c:vec4<f32>,@location(11)d:vec4<f32>,@location(12)e:vec4<f32>,@location(13)f:vec4<f32>,@location(14)g:vec4<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let q=vec2(f32((i<<1u)&2u),f32(i&2u));var o:Out;o.p=vec4(q*2.0-1.0,0.0,1.0);let vertex=select(i,u32(u.lookup[i]),u.lookup.w==1.0||u.lookup.w==5.0);var c=surfaceSkyVertex(u.settings.x,vertex,u.settings.y,u.settings.z>.5);if u.lookup.w==2.0{c.g.w=u.lookup.x;}o.a=c.a;o.b=c.b;o.c=c.c;o.d=c.d;o.e=c.e;o.f=c.f;o.g=c.g;return o;}
@fragment fn fs(v:Out,@builtin(front_facing)front:bool)->@location(0)vec4<f32>{let c=SurfaceSkyVertex(v.a,v.b,v.c,v.d,v.e,v.f,v.g);let normal=select(u.normal.xyz,-u.normal.xyz,!front&&u.normal.w>.5);let s=surfaceSkyIrradiance(c,normal);if u.lookup.w>=4.0{return vec4(vec3(select(0.0,1.0,c.g.w<=-.9999)),1.0);}let interfaceCheck=v.x0.x+v.x1.y+v.x3.x+v.x4.z+v.x5.x+v.x6.x+v.x7+v.x8.x;return vec4(select(vec3(.3),s.rgb,s.w>.5)*u.settings.w+vec3(interfaceCheck*u.normal.w),1.0);}`;
    const module = device.createShaderModule({ code }),
      shaderErrors = (await module.getCompilationInfo()).messages
        .filter((x) => x.type === "error")
        .map((x) => x.message);
    if (shaderErrors.length) return { shaderErrors, errors };
    const layout = device.createBindGroupLayout({
        entries: [
          {
            binding: 0,
            visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
            buffer: { type: "uniform" },
          },
          {
            binding: 16,
            visibility: GPUShaderStage.VERTEX,
            buffer: { type: "read-only-storage" },
          },
        ],
      }),
      pipeline = await device.createRenderPipelineAsync({
        layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
        vertex: { module, entryPoint: "vs" },
        fragment: {
          module,
          entryPoint: "fs",
          targets: [{ format: "rgba16float" }],
        },
      });
    const realFetch = globalThis.fetch,
      maps = new Map();
    globalThis.fetch = async (url) =>
      maps.has(String(url))
        ? new Response(maps.get(String(url)).slice(0))
        : realFetch(url);
    const digest = async (b) =>
      [...new Uint8Array(await crypto.subtle.digest("SHA-256", b))]
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
    const half = (n) => {
      const s = n & 32768 ? -1 : 1,
        e = (n >> 10) & 31,
        m = n & 1023;
      return (
        s *
        (e === 0
          ? m * 2 ** -24
          : e === 31
            ? Infinity
            : (1 + m / 1024) * 2 ** (e - 15))
      );
    };
    const cases = [];
    for (const [label, solar, marker, enabled, normal, gain] of [
      ["solar-zero", 0, 1, 1, [0, 1, 0], 1],
      ["solar-one", 1, 1, 1, [0, 1, 0], 1],
      ["solar-two", 2, 1, 1, [0, 1, 0], 1],
      ["legacy", 1, 0, 1, [0, 1, 0], 1],
      ["disabled", 1, 1, 0, [0, 1, 0], 1],
      ["bad-selection", 1, 99, 1, [0, 1, 0], 1],
      ["blocked-replaces", 1, 1, 1, [0, 1, 0], 1],
      ["normal-up", 1, 1, 1, [0, 1, 0], 1],
      ["normal-down", 1, 1, 1, [0, -1, 0], 1],
      ["interpolation", 1, 1, 1, [0, 1, 0], 1],
      ["compose-before-clamp", 1, 1, 1, [0, 1, 0], 1],
      ["source-gain-storage", 1, 1, 1, [0, 1, 0], 0.03],
      ["sparse-equivalent", 1, 1, 1, [0, 1, 0], 1],
      ["sparse-missing", 1, 1, 1, [0, 1, 0], 1],
      ["dense-reflection", 1, 1, 1, [0, 1, 0], 1],
      ["sparse-reflection", 1, 1, 1, [0, 1, 0], 1],
      ["dense-reflection-sign", 1, 1, 1, [0, 1, 0], 1],
      ["sparse-reflection-sign", 1, 1, 1, [0, 1, 0], 1],
      ["partial-zero", 1, 1, 1, [0, 1, 0], 1],
      ["partial-positive", 1, 1, 1, [0, 1, 0], 1],
      ["partial-negative", 1, 1, 1, [0, 1, 0], 1],
    ]) {
      const m = surfaceFixture(),
        values = new Float32Array(216);
      for (let i = 0; i < 3; i++) {
        values[i * 72 + 3] = 1;
        for (let c = 0; c < 3; c++) {
          values[i * 72 + c] = 1 / 0.2820947918;
          values[i * 72 + 36 + c] = 2 / 0.2820947918;
        }
      }
      let expected = solar + 2;
      if (["legacy", "disabled", "bad-selection"].includes(label))
        expected = 0.3;
      if (label === "blocked-replaces") {
        values.fill(0);
        for (let i = 0; i < 3; i++) values[i * 72 + 3] = 1;
        expected = 0;
      }
      if (label.startsWith("normal-")) {
        for (let i = 0; i < 3; i++)
          for (let c = 0; c < 3; c++) {
            values[i * 72 + 36 + c] = 0;
            values[i * 72 + 4 + c] = 0.5 / 0.4886025119;
          }
        expected = label === "normal-up" ? 1.5 : 0.5;
      }
      if (label === "interpolation") {
        for (let i = 0; i < 3; i++)
          for (let c = 0; c < 3; c++) {
            values[i * 72 + c] = (i + 1) / 0.2820947918;
            values[i * 72 + 36 + c] = 0;
          }
        expected = 1 * 0.5 + 2 * 0.265625 + 3 * 0.234375;
      }
      if (label === "compose-before-clamp") {
        for (let i = 0; i < 3; i++)
          for (let c = 0; c < 3; c++) values[i * 72 + c] = -1 / 0.2820947918;
        expected = 1;
      }
      const lookup = [0, 0, 0, 0];
      if (label.startsWith("sparse-")) {
        m.selections[0].firstVertex = 4;
        m.selections[0].vertexIndices = [4, 99, 0xffffff];
        lookup.splice(
          0,
          4,
          4,
          label === "sparse-missing" ? 100 : 99,
          0xffffff,
          1,
        );
        if (label === "sparse-missing") expected = 0.3;
      }
      if (label.includes("reflection"))
        m.reflection = { model: "sh9-ggx-rough-v1" };
      if (label.endsWith("-sign")) {
        lookup[3] = label.startsWith("sparse-") ? 5 : 4;
        expected = 1;
      }
      if (label.startsWith("partial-")) {
        lookup[0] =
          label === "partial-zero"
            ? 0
            : label === "partial-positive"
              ? 0.5
              : -0.5;
        lookup[3] = 2;
        expected = 0.3;
      }
      m.scenarios.day.sha256 = await digest(values.buffer);
      maps.set(m.scenarios.day.url, values.buffer);
      device.pushErrorScope("validation");
      const owner = await SurfaceSky.create(device, m),
        uniform = device.createBuffer({
          size: 48,
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        }),
        target = device.createTexture({
          size: [16, 16],
          format: "rgba16float",
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        }),
        read = device.createBuffer({
          size: 4096,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
      device.queue.writeBuffer(
        uniform,
        0,
        new Float32Array([
          marker,
          solar,
          enabled,
          gain,
          ...normal,
          0,
          ...lookup,
        ]),
      );
      const group = device.createBindGroup({
          layout,
          entries: [
            { binding: 0, resource: { buffer: uniform } },
            { binding: 16, resource: { buffer: owner.get("day") } },
          ],
        }),
        encoder = device.createCommandEncoder(),
        p = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: target.createView(),
              loadOp: "clear",
              storeOp: "store",
              clearValue: [0, 0, 0, 1],
            },
          ],
        });
      p.setPipeline(pipeline);
      p.setBindGroup(0, group);
      p.draw(3);
      p.end();
      encoder.copyTextureToBuffer(
        { texture: target },
        { buffer: read, bytesPerRow: 256 },
        [16, 16],
      );
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const data = new Uint16Array(read.getMappedRange()),
        pixel = [0, 1, 2].map((c) => half(data[(8 * 256 + 8 * 8) / 2 + c]));
      read.unmap();
      const error = await device.popErrorScope();
      if (error) errors.push(error.message);
      cases.push({
        label,
        pixel,
        expected: expected * gain,
        bytes: owner.bytes,
      });
      owner.dispose();
      uniform.destroy();
      target.destroy();
      read.destroy();
    }
    // Exhaust every address around the maximum map, including both endpoints and
    // every interior miss. Dense/sparse coefficient records must be identical.
    const addressCount = 32768,
      queryCount = 65537,
      values = new Float32Array(addressCount * 72);
    for (let i = 0; i < addressCount; i++) {
      values[i * 72] = i + 1;
      values[i * 72 + 3] = 1;
    }
    const addressModule = device.createShaderModule({
      code:
        surfaceSkyWGSL(16) +
        `
@group(0)@binding(0)var<storage,read_write>outputs:array<vec2<f32>>;
@compute @workgroup_size(64)fn cs(@builtin(global_invocation_id)id:vec3<u32>){if id.x>=65537u{return;}let c=surfaceSkyVertex(1.0,id.x,1.0,true);outputs[id.x]=vec2(c.a.x,c.g.w);}`,
    });
    const addressShaderErrors = (
      await addressModule.getCompilationInfo()
    ).messages
      .filter((m) => m.type === "error")
      .map((m) => m.message);
    shaderErrors.push(...addressShaderErrors);
    if (addressShaderErrors.length) throw Error(addressShaderErrors.join("\n"));
    const addressPipeline = await device.createComputePipelineAsync({
      layout: "auto",
      compute: { module: addressModule, entryPoint: "cs" },
    });
    let addressCases = 0;
    const exact = [];
    for (const sparse of [false, true]) {
      const m = surfaceFixture();
      m.vertexCount = m.selections[0].vertexCount = addressCount;
      m.scenarios.day.bytes = values.byteLength;
      m.scenarios.day.sha256 = await digest(values.buffer);
      maps.set(m.scenarios.day.url, values.buffer);
      if (sparse) {
        m.selections[0].vertexIndices = Array.from(
          { length: addressCount },
          (_, i) => i * 2,
        );
        m.reflection = { model: "sh9-ggx-rough-v1" };
      }
      const owner = await SurfaceSky.create(device, m),
        output = device.createBuffer({
          size: queryCount * 8,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        }),
        read = device.createBuffer({
          size: queryCount * 8,
          usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });
      const group = device.createBindGroup({
        layout: addressPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: output } },
          { binding: 16, resource: { buffer: owner.get("day") } },
        ],
      });
      const encoder = device.createCommandEncoder(),
        pass = encoder.beginComputePass();
      pass.setPipeline(addressPipeline);
      pass.setBindGroup(0, group);
      pass.dispatchWorkgroups(Math.ceil(queryCount / 64));
      pass.end();
      encoder.copyBufferToBuffer(output, 0, read, 0, queryCount * 8);
      device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      const result = new Float32Array(read.getMappedRange()).slice();
      read.unmap();
      for (let i = 0; i < queryCount; i++) {
        const rank = sparse ? i / 2 : i,
          valid = Number.isInteger(rank) && rank < addressCount;
        if (
          result[i * 2] !== (valid ? rank + 1 : 0) ||
          result[i * 2 + 1] !== (valid ? (sparse ? -1 : 1) : 0)
        )
          throw Error(
            JSON.stringify({
              sparse,
              vertex: i,
              rank,
              observed: [result[i * 2], result[i * 2 + 1]],
            }),
          );
        addressCases++;
      }
      exact.push(
        Array.from(
          { length: addressCount },
          (_, i) => result[(sparse ? i * 2 : i) * 2],
        ),
      );
      owner.dispose();
      output.destroy();
      read.destroy();
    }
    if (exact[0].some((v, i) => v !== exact[1][i]))
      throw Error("Dense/sparse coefficient lookup differs");
    // The CPU quadrature is a separate directional reference for the GPU's same SH layout.
    const fixture = surfaceFixture(),
      baked = bakeSurfaceSkyPoint(
        { position: [0, 0, 0], geometricNormal: [0, 1, 0] },
        fixture.transport,
        () => false,
        () => [1, 2, 3],
      );
    const cpu = evaluateSurfaceSky(baked.coefficients, [0, 1, 0], 1);
    globalThis.fetch = realFetch;
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    return {
      shaderErrors,
      errors,
      cases,
      addressCases,
      cpu,
      limits: { userVaryings: 15, frontFacing: 1, components: 49 },
    };
  });
  assert.deepEqual(pageErrors, []);
  assert.deepEqual(result.shaderErrors, []);
  assert.deepEqual(result.errors, []);
  assert.equal(result.cases.length, 21);
  assert.equal(result.addressCases, 131074);
  for (const c of result.cases)
    for (const x of c.pixel)
      assert.ok(Math.abs(x - c.expected) < 0.003, JSON.stringify(c));
  result.ok = true;
  console.log(JSON.stringify(result, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
