import assert from "node:assert/strict";
import { test } from "node:test";
import { Bloom, BLOOM_WEIGHTS, bloomSize } from "../packages/renderer/bloom.ts";

test("bloom is a normalized contiguous Gaussian with finite aspect-preserving bounds", () => {
  assert.ok(
    Math.abs(
      BLOOM_WEIGHTS[0] +
        2 * BLOOM_WEIGHTS.slice(1).reduce((a, b) => a + b, 0) -
        1,
    ) < 1e-14,
  );
  BLOOM_WEIGHTS.slice(1).forEach((weight, i) =>
    assert.ok(weight > 0 && weight < BLOOM_WEIGHTS[i]),
  );
  assert.deepEqual(bloomSize(1280, 720), [640, 360]);
  assert.deepEqual(bloomSize(3840, 2160), [640, 360]);
  assert.deepEqual(bloomSize(100, 200), [50, 100]);
  assert.deepEqual(bloomSize(1, 1), [1, 1]);
  assert.deepEqual(bloomSize(16384, 16384), [360, 360]);
  for (const size of [
    [0, 1],
    [NaN, 10],
    [10.5, 1],
    [16385, 1],
    [-1, 1],
  ])
    assert.throws(() => bloomSize(size[0], size[1]));
  for (let width = 1; width < 16385; width += 107) {
    const [w, h] = bloomSize(width, 733);
    assert.ok(w <= 640 && h <= 360);
    assert.ok(w > 0 && h > 0);
  }
});

function fakeDevice() {
  Object.assign(globalThis, {
    GPUShaderStage: { FRAGMENT: 2 },
    GPUBufferUsage: { UNIFORM: 1, COPY_DST: 2 },
    GPUTextureUsage: { RENDER_ATTACHMENT: 1, TEXTURE_BINDING: 2, COPY_SRC: 4 },
  });
  const textures: Array<{ destroyed: number; size: number[] }> = [],
    buffers: Array<{ destroyed: number }> = [];
  let groups = 0,
    passes = 0,
    writes = 0,
    failTexture = Infinity,
    failPipeline = Infinity,
    pipelines = 0;
  const device = {
    limits: { maxTextureDimension2D: 16384 },
    createBindGroupLayout: () => ({}),
    createPipelineLayout: () => ({}),
    createShaderModule: () => ({}),
    createRenderPipelineAsync: async () => {
      if (++pipelines === failPipeline) throw Error("pipeline failure");
      return {};
    },
    createBindGroup: () => ({ id: ++groups }),
    createBuffer: () => {
      const b = {
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(b);
      return b;
    },
    createTexture: ({ size }: { size: number[] }) => {
      if (textures.length === failTexture) throw Error("texture failure");
      const t = {
        size,
        destroyed: 0,
        createView: () => ({}),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(t);
      return t;
    },
    queue: {
      writeBuffer() {
        writes++;
      },
    },
  } as unknown as GPUDevice;
  const encoder = {
    beginRenderPass() {
      passes++;
      return { setPipeline() {}, setBindGroup() {}, draw() {}, end() {} };
    },
  } as unknown as GPUCommandEncoder;
  return {
    device,
    encoder,
    textures,
    buffers,
    get groups() {
      return groups;
    },
    get passes() {
      return passes;
    },
    get writes() {
      return writes;
    },
    failTextureAt(n: number) {
      failTexture = n;
    },
    failPipelineAt(n: number) {
      failPipeline = n;
    },
  };
}

test("stable bloom views and groups survive repeated frames, disable clears once, resize disposes old images", async () => {
  const gpu = fakeDevice(),
    bloom = await Bloom.create(gpu.device);
  bloom.resize(3840, 2160);
  assert.equal(bloom.bytes, 5_529_616);
  const view = bloom.view,
    groups = gpu.groups,
    writes = gpu.writes;
  const frame = {
    width: 3840,
    height: 2160,
    source: {} as GPUTextureView,
    enabled: true,
  };
  bloom.render(gpu.encoder, frame);
  assert.equal(gpu.groups, groups + 1);
  assert.equal(gpu.passes, 3);
  bloom.render(gpu.encoder, frame);
  assert.equal(gpu.groups, groups + 1);
  assert.equal(gpu.passes, 6);
  assert.equal(bloom.view, view);
  assert.equal(gpu.writes, writes);
  bloom.render(gpu.encoder, { ...frame, enabled: false });
  assert.equal(gpu.passes, 7);
  bloom.render(gpu.encoder, { ...frame, enabled: false });
  assert.equal(gpu.passes, 7);
  bloom.render(gpu.encoder, frame);
  assert.equal(gpu.passes, 10);
  bloom.render(gpu.encoder, { ...frame, enabled: false });
  assert.equal(gpu.passes, 11);
  assert.throws(() => bloom.render(gpu.encoder, { ...frame, source: view }));
  bloom.resize(100, 100);
  assert.notEqual(bloom.view, view);
  assert.equal(gpu.textures.length, 9);
  assert.ok(gpu.textures.slice(0, 6).every((t) => t.destroyed === 1));
  assert.throws(() => bloom.resize(0, 10));
  bloom.dispose();
  bloom.dispose();
  assert.equal(bloom.bytes, 0);
  assert.ok(gpu.textures.every((t) => t.destroyed === 1));
  assert.ok(gpu.buffers.every((b) => b.destroyed === 1));
  assert.throws(() => bloom.resize(1, 1));
  assert.throws(() => bloom.view);
});

test("partial allocation failures clean candidates and preserve the working output", async () => {
  const gpu = fakeDevice(),
    bloom = await Bloom.create(gpu.device),
    view = bloom.view;
  gpu.failTextureAt(4);
  assert.throws(() => bloom.resize(128, 128), /texture failure/);
  assert.equal(bloom.view, view);
  assert.ok(gpu.textures.slice(0, 3).every((t) => t.destroyed === 0));
  assert.equal(gpu.textures[3].destroyed, 1);
  bloom.dispose();
  assert.ok(gpu.textures.every((t) => t.destroyed === 1));
  const creation = fakeDevice();
  creation.failTextureAt(1);
  await assert.rejects(Bloom.create(creation.device), /texture failure/);
  assert.equal(creation.textures[0].destroyed, 1);
  assert.equal(creation.buffers[0].destroyed, 1);
  const pipeline = fakeDevice();
  pipeline.failPipelineAt(2);
  await assert.rejects(Bloom.create(pipeline.device), /pipeline failure/);
  assert.equal(pipeline.textures.length, 0);
  assert.equal(pipeline.buffers.length, 0);
});
