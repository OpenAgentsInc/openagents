import { test } from "node:test";
import assert from "node:assert/strict";
import type { AssetDescription } from "../src/asset-compiler.ts";
import { AssetRuntimeAdapter } from "../src/asset-runtime-adapter.ts";
import type { OwnedAssetDescription } from "../src/asset-import-protocol.ts";

Object.defineProperty(globalThis, "GPUBufferUsage", { value: { VERTEX: 32, INDEX: 16, COPY_DST: 8, COPY_SRC: 4 }, configurable: true });
Object.defineProperty(globalThis, "GPUTextureUsage", { value: { TEXTURE_BINDING: 4, COPY_DST: 2, COPY_SRC: 1 }, configurable: true });

function device() {
  const buffers: { bytes: Uint8Array; destroyed: boolean; destroy(): void }[] = [];
  const textures: { bytes?: Uint8Array; destroyed: boolean; destroy(): void }[] = [];
  const errors: (GPUError | null | Promise<GPUError | null>)[] = [];
  const scopes: string[] = [];
  const value = {
    pushErrorScope(filter: string) { scopes.push(filter); },
    popErrorScope() { scopes.pop(); return Promise.resolve(errors.shift() ?? null); },
    createBuffer(descriptor: GPUBufferDescriptor) {
      const buffer = { bytes: new Uint8Array(Number(descriptor.size)), destroyed: false, destroy() { this.destroyed = true; } };
      buffers.push(buffer); return buffer;
    },
    createTexture() {
      const texture = { destroyed: false, destroy() { this.destroyed = true; } };
      textures.push(texture); return texture;
    },
    queue: {
      writeBuffer(buffer: typeof buffers[number], offset: number, data: ArrayBuffer, start: number, length: number) {
        buffer.bytes.set(new Uint8Array(data, start, length), offset);
      },
      writeTexture(target: { texture: typeof textures[number] }, bytes: Uint8Array) { target.texture.bytes = bytes.slice(); },
    },
  };
  return { gpu: value as unknown as GPUDevice, buffers, textures, errors, scopes };
}

function description(): AssetDescription {
  return { id: "mesh", sources: [], diagnostics: [], meshes: [{ id: "triangle", source: "mesh", vertexCount: 3, indexCount: 3,
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]), indices: new Uint32Array([0, 1, 2]),
    bounds: { min: [0, 0, 0], max: [1, 1, 0] } }], images: [{ id: "card", source: "mesh", mime: "image/png", width: 1, height: 1,
      bytes: new Uint8Array([255, 16, 32, 255]) }] };
}

test("owned normals, palette, glow, UVs, and image material reach separate GPU resources", async () => {
  const fake = device(), runtime = new AssetRuntimeAdapter(fake.gpu);
  const input: OwnedAssetDescription = description();
  Object.assign(input.meshes[0], { normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
    colors: new Float32Array(9).fill(0.5), glow: new Float32Array([1, 0, 0]),
    uvs: new Float32Array([0, 0, 1, 0, 0, 1]), material: "card-material" });
  input.materials = [{ id: "card-material", baseColor: [1, 1, 1, 1], doubleSided: true, unlit: true, baseColorTexture: "card" }];
  runtime.commit("mesh", 1, await runtime.prepare(input));
  const resources = runtime.resources("mesh")!.meshes[0];
  assert.equal(resources.material, "card-material");
  for (const [name, field] of [["normal", "normals"], ["color", "colors"], ["glow", "glow"], ["uv", "uvs"]] as const) {
    const buffer = resources.attributes[name] as unknown as { bytes: Uint8Array };
    assert.deepEqual(new Float32Array(buffer.bytes.buffer), input.meshes[0][field]);
  }
  input.meshes[0].colors![0] = 0;
  input.materials[0].baseColorTexture = "changed";
  const saved = runtime.get("mesh")!.description;
  assert.equal(saved.meshes[0].colors![0], 0.5);
  assert.equal(saved.materials![0].baseColorTexture, "card");
  assert.deepEqual(runtime.stats(), { assets: 1, buffers: 6, textures: 1 });
  runtime.dispose();
  assert.ok(fake.buffers.every(buffer => buffer.destroyed));
});

test("runtime uploads compiled mesh/image bytes and replaces only that asset", async () => {
  const fake = device(), runtime = new AssetRuntimeAdapter(fake.gpu);
  const input = description();
  const first = await runtime.prepare(input);
  runtime.commit("mesh", 1, first);
  assert.equal(new Float32Array(fake.buffers[0].bytes.buffer)[3], 1);
  assert.deepEqual([...fake.textures[0].bytes!], [255, 16, 32, 255]);
  input.meshes[0].positions[3] = 99;
  assert.equal(runtime.get("mesh")!.description.meshes[0].positions[3], 1);
  assert.equal(runtime.resources("mesh")!.meshes[0].indexCount, 3);
  const unrelated = description(); unrelated.id = "other";
  runtime.commit("other", 1, await runtime.prepare(unrelated));
  const otherResources = runtime.resources("other")!.meshes[0].vertex;
  runtime.commit("mesh", 2, await runtime.prepare(description()));
  assert.equal(runtime.get("mesh")!.revision, 2);
  assert.equal(runtime.resources("other")!.meshes[0].vertex, otherResources);
  assert.equal(fake.buffers[0].destroyed, true);
  assert.equal(fake.textures[0].destroyed, true);
  assert.deepEqual(runtime.stats(), { assets: 2, buffers: 4, textures: 2 });
  runtime.dispose(); runtime.dispose();
  assert.deepEqual(runtime.stats(), { assets: 0, buffers: 0, textures: 0 });
  assert.ok(fake.buffers.every(buffer => buffer.destroyed));
  assert.ok(fake.textures.every(texture => texture.destroyed));
  assert.deepEqual(fake.scopes, []);
});

test("failed GPU validation discards candidates while keeping accepted resources", async () => {
  const fake = device(), runtime = new AssetRuntimeAdapter(fake.gpu);
  runtime.commit("mesh", 1, await runtime.prepare(description()));
  const old = runtime.resources("mesh")!.meshes[0].vertex;
  fake.errors.push({ message: "bad upload" } as GPUError);
  await assert.rejects(runtime.prepare(description()), /could not be uploaded/);
  assert.equal(runtime.resources("mesh")!.meshes[0].vertex, old);
  assert.equal(runtime.get("mesh")!.revision, 1);
  assert.equal(fake.buffers[0].destroyed, false);
  assert.ok(fake.buffers.slice(2).every(buffer => buffer.destroyed));
  assert.deepEqual(runtime.stats(), { assets: 1, buffers: 2, textures: 1 });
  runtime.dispose();
});

test("closing during asynchronous preparation cleans staged resources", async () => {
  const fake = device(), runtime = new AssetRuntimeAdapter(fake.gpu);
  let finish!: (value: GPUError | null) => void;
  fake.errors.push(new Promise(resolve => { finish = resolve; }));
  const pending = runtime.prepare(description());
  runtime.dispose();
  finish(null);
  await assert.rejects(pending);
  assert.ok(fake.buffers.every(buffer => buffer.destroyed));
  assert.ok(fake.textures.every(texture => texture.destroyed));
  assert.deepEqual(runtime.stats(), { assets: 0, buffers: 0, textures: 0 });
});
