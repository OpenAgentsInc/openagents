/** Bake private, animated distant silhouettes from the existing source VAT. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import {
  mkdir,
  readFile,
  readdir,
  realpath,
  link,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

// The openagents checkout, four levels up; baked atlases never go inside it.
const repo = await realpath(
  fileURLToPath(new URL("../../../..", import.meta.url)),
);
assert(
  process.env.AF_BATTLE_ASSET_ROOT && process.env.AF_BATTLE_IMPOSTOR_OUTPUT,
  "Set private AF_BATTLE_ASSET_ROOT and AF_BATTLE_IMPOSTOR_OUTPUT folders.",
);
const input = await realpath(resolve(process.env.AF_BATTLE_ASSET_ROOT));
const requestedOutput = resolve(process.env.AF_BATTLE_IMPOSTOR_OUTPUT);
await mkdir(requestedOutput, { recursive: true });
const output = await realpath(requestedOutput);
assert(
  output !== input && !output.startsWith(input + sep) &&
    output !== repo && !output.startsWith(repo + sep),
);
assert((await readdir(output)).length === 0, "Choose an empty output folder.");
const scene = JSON.parse(
  await readFile(resolve(input, "scene-ground.json"), "utf8"),
);
const originalURL = scene.battleCrowdURL;
assert(/^\/private-assets\/battle-crowd-[a-f0-9]{64}\.json$/.test(originalURL));
const manifest = JSON.parse(
  await readFile(resolve(input, basename(originalURL)), "utf8"),
);
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const uploads = new Map();
const errors = [];
const server = createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://localhost").pathname;
    if (
      req.method === "POST" &&
      /^\/bake\/(paladin|sorceror|zombie|warrok)\/(color|normal)$/.test(path)
    ) {
      const chunks = [];
      let length = 0;
      for await (const part of req) {
        length += part.length;
        assert(length <= 8_388_608);
        chunks.push(part);
      }
      assert.equal(length, 8_388_608);
      const bytes = Buffer.concat(chunks),
        hash = sha(bytes),
        kind = path.split("/")[3];
      const name = `battle-${hash}.impostor-${kind}.bin`;
      await writeFile(resolve(output, name), bytes);
      uploads.set(path, {
        url: `/private-assets/${name}`,
        sha256: hash,
        byteLength: bytes.length,
      });
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify(uploads.get(path)));
      return;
    }
    if (req.method === "GET" && path.startsWith("/private-assets/")) {
      const name = basename(path);
      assert.equal(path, `/private-assets/${name}`);
      const bytes = await readFile(resolve(input, name));
      res.writeHead(200, {
        "Content-Type": name.endsWith(".json")
          ? "application/json"
          : "application/octet-stream",
      });
      res.end(bytes);
      return;
    }
    res.writeHead(200, { "Content-Type": "text/html" });
    res.end("<!doctype html><title>Private battle silhouette bake</title>");
  } catch (e) {
    errors.push(String(e));
    res.writeHead(500);
    res.end("The private bake failed.");
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const origin = `http://127.0.0.1:${server.address().port}`;
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({ serviceWorkers: "block" });
const page = await context.newPage();
page.on("pageerror", (e) => errors.push(String(e)));
let result;
try {
  await page.goto(origin);
  result = await page.evaluate(async (manifest) => {
    const adapter = await navigator.gpu?.requestAdapter({
      powerPreference: "high-performance",
    });
    if (!adapter)
      throw Error("A WebGPU adapter is needed to bake silhouettes.");
    const device = await adapter.requestDevice();
    const gpuErrors = [];
    device.addEventListener("uncapturederror", (e) =>
      gpuErrors.push(String(e.error)),
    );
    const classes = [],
      tileSize = 32,
      atlasSize = 512,
      views = 8;
    const source = `
struct Config {bounds:vec4f,angle:vec4f}
struct Frame {sample:vec4f}
@group(0)@binding(0)var<uniform> config:Config;
@group(0)@binding(1)var position:texture_2d<f32>;
@group(0)@binding(2)var normal:texture_2d<f32>;
@group(0)@binding(3)var albedo:texture_2d_array<f32>;
@group(0)@binding(4)var surfaceSampler:sampler;
@group(0)@binding(5)var<storage,read> frames:array<Frame>;
struct Input {@location(0)uv:vec2f,@location(1)column:u32,@location(2)material:u32}
struct Vertex {@builtin(position)clip:vec4f,@location(0)uv:vec2f,@location(1)n:vec3f,@location(2)@interpolate(flat)material:u32}
@vertex fn vertexMain(input:Input,@builtin(instance_index)instance:u32)->Vertex {
 let f=frames[instance].sample;
 let p=mix(textureLoad(position,vec2i(i32(input.column),i32(f.x)),0).xyz,textureLoad(position,vec2i(i32(input.column),i32(f.y)),0).xyz,f.z);
 let n=mix(textureLoad(normal,vec2i(i32(input.column),i32(f.x)),0).xyz,textureLoad(normal,vec2i(i32(input.column),i32(f.y)),0).xyz,f.z);
 let c=config.angle.x;let s=config.angle.y;
 let px=(c*p.x-s*p.z)/(2.0*config.bounds.z)+.5;
 let py=(p.y-config.bounds.x)/(config.bounds.y-config.bounds.x);
 let tx=f32(instance%16u);let ty=f32(instance/16u);
 var output:Vertex;output.clip=vec4((tx+px)/8.0-1.0,1.0-(ty+1.0-py)/8.0,.5-(s*p.x+c*p.z)/(config.bounds.z*2.1),1);
 output.uv=input.uv;output.n=n;output.material=input.material;return output;
}
struct Pixel {@location(0)color:vec4f,@location(1)normal:vec4f}
@fragment fn fragmentMain(input:Vertex,@builtin(front_facing)front:bool)->Pixel {
 var output:Pixel;output.color=vec4(textureSample(albedo,surfaceSampler,input.uv,i32(input.material)).rgb,1);
 output.normal=vec4(normalize(select(-input.n,input.n,front))*.5+.5,1);return output;
}`;
    const module = device.createShaderModule({ code: source });
    const info = await module.getCompilationInfo();
    if (info.messages.some((m) => m.type === "error"))
      throw Error(JSON.stringify(info.messages));
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vertexMain",
        buffers: [
          {
            arrayStride: 16,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x2" },
              { shaderLocation: 1, offset: 8, format: "uint32" },
              { shaderLocation: 2, offset: 12, format: "uint32" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fragmentMain",
        targets: [{ format: "rgba8unorm-srgb" }, { format: "rgba8unorm" }],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth32float",
        depthWriteEnabled: true,
        depthCompare: "less",
      },
      multisample: { count: 4 },
    });
    const half = (bits) => {
      const sign = bits & 32768 ? -1 : 1,
        exponent = (bits >> 10) & 31,
        fraction = bits & 1023;
      return exponent === 0
        ? sign * fraction * 2 ** -24
        : exponent === 31
          ? sign * Infinity
          : sign * (1 + fraction / 1024) * 2 ** (exponent - 15);
    };
    const fetchBlob = async (blob) => {
      const response = await fetch(blob.url);
      if (!response.ok) throw Error("A source asset could not load.");
      const bytes = await response.arrayBuffer();
      if (bytes.byteLength !== blob.byteLength)
        throw Error("A source asset is incomplete.");
      const digest = [
        ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
      ]
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
      if (digest !== blob.sha256) throw Error("A source asset changed.");
      return bytes;
    };
    for (const model of manifest.classes) {
      const resources = [];
      const makeBuffer = (bytes, usage) => {
        const b = device.createBuffer({
          size: Math.max(32, bytes.byteLength),
          usage: usage | GPUBufferUsage.COPY_DST,
        });
        device.queue.writeBuffer(b, 0, bytes);
        resources.push(b);
        return b;
      };
      const makeTexture = (
        size,
        format,
        data,
        bytesPerRow,
        rowsPerImage,
        sampleCount = 1,
      ) => {
        const usage =
          sampleCount === 1
            ? GPUTextureUsage.TEXTURE_BINDING |
              GPUTextureUsage.COPY_DST |
              GPUTextureUsage.COPY_SRC |
              GPUTextureUsage.RENDER_ATTACHMENT
            : GPUTextureUsage.RENDER_ATTACHMENT;
        const texture = device.createTexture({
          size,
          format,
          sampleCount,
          usage,
        });
        resources.push(texture);
        if (data)
          device.queue.writeTexture(
            { texture },
            data,
            { bytesPerRow, rowsPerImage },
            size,
          );
        return texture;
      };
      const [vertices, indices, positions, normals, materials] =
        await Promise.all(
          [
            model.vertices,
            model.indices,
            model.vat.positions,
            model.vat.normals,
            model.materials.rgba,
          ].map(fetchBlob),
        );
      const values = new Uint16Array(positions);
      let radius = 0,
        minY = Infinity,
        maxY = -Infinity;
      for (let i = 0; i < values.length; i += 4) {
        const x = half(values[i]),
          y = half(values[i + 1]),
          z = half(values[i + 2]);
        radius = Math.max(radius, Math.hypot(x, z));
        minY = Math.min(minY, y);
        maxY = Math.max(maxY, y);
      }
      const padding = (maxY - minY) / 14;
      minY -= padding;
      maxY += padding;
      radius *= 8 / 7;
      const clips = {},
        samples = [];
      for (const name of ["idle", "walk", "attack", "death"]) {
        const original = model.clips[name],
          count = Math.ceil(original.duration * 12) + 1;
        clips[name] = {
          duration: original.duration,
          firstFrame: samples.length / 4,
          frameCount: count,
          loop: original.loop,
        };
        for (let i = 0; i < count; i++) {
          const phase = (i / (count - 1)) * (original.frameCount - 1),
            a = original.firstFrame + Math.floor(phase);
          samples.push(
            a,
            Math.min(a + 1, original.firstFrame + original.frameCount - 1),
            phase % 1,
            0,
          );
        }
      }
      const frames = samples.length / 4;
      if (frames > 256)
        throw Error("The silhouette atlas has too many frames.");
      const vertexBuffer = makeBuffer(vertices, GPUBufferUsage.VERTEX),
        indexBuffer = makeBuffer(indices, GPUBufferUsage.INDEX);
      const sampleBuffer = makeBuffer(
        new Float32Array(samples),
        GPUBufferUsage.STORAGE,
      );
      const positionTexture = makeTexture(
        [model.vat.width, model.vat.height],
        "rgba16float",
        positions,
        model.vat.width * 8,
        model.vat.height,
      );
      const normalTexture = makeTexture(
        [model.vat.width, model.vat.height],
        "rgba16float",
        normals,
        model.vat.width * 8,
        model.vat.height,
      );
      const materialTexture = makeTexture(
        [256, 256, model.materials.layers],
        "rgba8unorm-srgb",
        materials,
        1024,
        256,
      );
      const colorAtlas = makeTexture(
        [atlasSize, atlasSize, views],
        "rgba8unorm-srgb",
      );
      const normalAtlas = makeTexture(
        [atlasSize, atlasSize, views],
        "rgba8unorm",
      );
      const colorMS = makeTexture(
        [atlasSize, atlasSize],
        "rgba8unorm-srgb",
        null,
        0,
        0,
        4,
      );
      const normalMS = makeTexture(
        [atlasSize, atlasSize],
        "rgba8unorm",
        null,
        0,
        0,
        4,
      );
      const depth = makeTexture(
        [atlasSize, atlasSize],
        "depth32float",
        null,
        0,
        0,
        4,
      );
      const encoder = device.createCommandEncoder();
      for (let view = 0; view < views; view++) {
        const angle = (view * Math.PI * 2) / views;
        const config = makeBuffer(
          new Float32Array([
            minY,
            maxY,
            radius,
            0,
            Math.cos(angle),
            Math.sin(angle),
            0,
            0,
          ]),
          GPUBufferUsage.UNIFORM,
        );
        const group = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: config } },
            { binding: 1, resource: positionTexture.createView() },
            { binding: 2, resource: normalTexture.createView() },
            {
              binding: 3,
              resource: materialTexture.createView({ dimension: "2d-array" }),
            },
            {
              binding: 4,
              resource: device.createSampler({
                magFilter: "linear",
                minFilter: "linear",
              }),
            },
            { binding: 5, resource: { buffer: sampleBuffer } },
          ],
        });
        const target = (texture, ms) => ({
          view: ms.createView(),
          resolveTarget: texture.createView({
            dimension: "2d",
            baseArrayLayer: view,
            arrayLayerCount: 1,
          }),
          clearValue: [0, 0, 0, 0],
          loadOp: "clear",
          storeOp: "discard",
        });
        const pass = encoder.beginRenderPass({
          colorAttachments: [
            target(colorAtlas, colorMS),
            target(normalAtlas, normalMS),
          ],
          depthStencilAttachment: {
            view: depth.createView(),
            depthClearValue: 1,
            depthLoadOp: "clear",
            depthStoreOp: "discard",
          },
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, group);
        pass.setVertexBuffer(0, vertexBuffer);
        pass.setIndexBuffer(indexBuffer, "uint32");
        pass.drawIndexed(
          model.lods[0].indexCount,
          frames,
          model.lods[0].firstIndex,
        );
        pass.end();
      }
      const byteLength = atlasSize * atlasSize * views * 4;
      const reads = [colorAtlas, normalAtlas].map((texture) => {
        const buffer = device.createBuffer({
          size: byteLength,
          usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });
        resources.push(buffer);
        encoder.copyTextureToBuffer(
          { texture },
          { buffer, bytesPerRow: atlasSize * 4, rowsPerImage: atlasSize },
          [atlasSize, atlasSize, views],
        );
        return buffer;
      });
      device.queue.submit([encoder.finish()]);
      await device.queue.onSubmittedWorkDone();
      const blobs = [],
        coverage = new Uint8Array(atlasSize * atlasSize * views);
      for (let i = 0; i < reads.length; i++) {
        await reads[i].mapAsync(GPUMapMode.READ);
        const bytes = new Uint8Array(reads[i].getMappedRange()).slice();
        reads[i].unmap();
        const sums = new Uint32Array(views * 256);
        for (let layer = 0; layer < views; layer++) {
          for (let y = 0; y < atlasSize; y++)
            for (let x = 0; x < atlasSize; x++) {
              const pixel = (layer * atlasSize + y) * atlasSize + x;
              const alpha = bytes[pixel * 4 + 3],
                tile = (y >> 5) * 16 + (x >> 5);
              if (tile >= frames && alpha)
                throw Error("An unused silhouette tile contains pixels.");
              if (
                (x % 32 === 0 ||
                  x % 32 === 31 ||
                  y % 32 === 0 ||
                  y % 32 === 31) &&
                alpha
              )
                throw Error("A silhouette touches its tile border.");
              sums[layer * 256 + tile] += alpha;
              if (i === 0) coverage[pixel] = alpha;
              else if (coverage[pixel] !== alpha)
                throw Error("Silhouette color and normal coverage differ.");
            }
          for (let frame = 0; frame < frames; frame++) {
            if (!sums[layer * 256 + frame])
              throw Error("A silhouette frame is empty.");
          }
        }
        const response = await fetch(
          `/bake/${model.kind}/${i ? "normal" : "color"}`,
          { method: "POST", body: bytes },
        );
        if (!response.ok) throw Error("The silhouette could not save.");
        blobs.push(await response.json());
      }
      classes.push({
        kind: model.kind,
        impostor: {
          views,
          tileSize,
          width: atlasSize,
          height: atlasSize,
          frames,
          bounds: { minY, maxY, radius },
          clips,
          color: blobs[0],
          normal: blobs[1],
        },
      });
      resources.forEach((r) => r.destroy());
    }
    await device.queue.onSubmittedWorkDone();
    device.destroy();
    if (gpuErrors.length) throw Error(JSON.stringify(gpuErrors));
    return {
      classes,
      gpuErrors,
      scope:
        "Original VAT source geometry and albedo, near 600-triangle mesh, eight source-local yaw views, twelve samples/second, four-sample coverage resolve.",
    };
  }, manifest);
  assert.equal(errors.length, 0, JSON.stringify(errors));
  const copySources = async (from, to) => {
    for (const entry of await readdir(from, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        await mkdir(resolve(to, entry.name));
        await copySources(resolve(from, entry.name), resolve(to, entry.name));
      } else {
        assert(
          entry.isFile(),
          "Private source assets cannot contain symbolic links.",
        );
        await link(resolve(from, entry.name), resolve(to, entry.name));
      }
    }
  };
  await copySources(input, output);
  for (const baked of result.classes)
    manifest.classes.find((c) => c.kind === baked.kind).impostor =
      baked.impostor;
  manifest.source = {
    ...manifest.source,
    impostors: {
      sampleRate: 12,
      views: 8,
      tileSize: 32,
      sourceManifest: originalURL,
      method: result.scope,
    },
  };
  const bytes = Buffer.from(JSON.stringify(manifest));
  scene.battleCrowdURL = `/private-assets/battle-crowd-${sha(bytes)}.json`;
  await writeFile(resolve(output, basename(scene.battleCrowdURL)), bytes);
  // Replace the hard link before updating this scene; the source stays unchanged.
  const { unlink } = await import("node:fs/promises");
  await unlink(resolve(output, "scene-ground.json"));
  await writeFile(resolve(output, "scene-ground.json"), JSON.stringify(scene));
  await writeFile(
    resolve(output, "battle-impostor-bake.json"),
    JSON.stringify(
      {
        ok: true,
        sourceManifest: originalURL,
        manifest: scene.battleCrowdURL,
        ...result,
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify({
      ok: true,
      output,
      manifest: scene.battleCrowdURL,
      classes: result.classes.map((c) => ({
        kind: c.kind,
        frames: c.impostor.frames,
        bounds: c.impostor.bounds,
      })),
    }),
  );
} finally {
  await context.close();
  await browser.close();
  await new Promise((done) => server.close(done));
}
