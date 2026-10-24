import test, { type TestContext } from "node:test";
import assert from "node:assert/strict";
import {
  EmptyWorld,
  EMPTY_WORLD_CLEAR,
  unpackEmptyPixels,
} from "../packages/runtime/empty-world.ts";

function fakeGPU(
  t: TestContext,
  format: "bgra8unorm" | "rgba8unorm" = "bgra8unorm",
) {
  for (const [key, value] of Object.entries({
    GPUTextureUsage: { RENDER_ATTACHMENT: 16, COPY_SRC: 1 },
    GPUBufferUsage: { COPY_DST: 8, MAP_READ: 1 },
    GPUMapMode: { READ: 1 },
  })) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, key);
    Object.defineProperty(globalThis, key, { configurable: true, value });
    t.after(() =>
      previous
        ? Object.defineProperty(globalThis, key, previous)
        : Reflect.deleteProperty(globalThis, key),
    );
  }
  const stats = {
    deviceDestroy: 0,
    bufferDestroy: 0,
    unmaps: 0,
    unconfigure: 0,
    submits: 0,
    usage: 0,
    clears: [] as unknown[],
    failMap: false,
    blockMap: false,
    release: undefined as (() => void) | undefined,
  };
  const canvas = { width: 7, height: 5, getContext: () => context };
  let texture: any;
  const context = {
    configure(value: any) {
      stats.usage = value.usage;
      texture = {
        data: new Uint8Array(canvas.width * canvas.height * 4),
        width: canvas.width,
        height: canvas.height,
        createView() {
          return { texture: this };
        },
      };
    },
    getCurrentTexture: () => texture,
    unconfigure() {
      stats.unconfigure++;
    },
  };
  const device = {
    limits: { maxTextureDimension2D: 8192 },
    destroy() {
      stats.deviceDestroy++;
    },
    queue: {
      submit(buffers: (() => void)[][]) {
        stats.submits++;
        buffers.flat().forEach((operation) => operation());
      },
      onSubmittedWorkDone: async () => {},
    },
    createBuffer(options: any) {
      const data = new ArrayBuffer(options.size);
      return {
        data,
        async mapAsync() {
          if (stats.failMap) throw new Error("Mock readback failed");
          if (stats.blockMap)
            await new Promise<void>((resolve) => {
              stats.release = resolve;
            });
        },
        getMappedRange: () => data,
        unmap() {
          stats.unmaps++;
        },
        destroy() {
          stats.bufferDestroy++;
        },
      };
    },
    createCommandEncoder() {
      const operations: (() => void)[] = [];
      return {
        beginRenderPass(options: any) {
          const attachment = options.colorAttachments[0];
          stats.clears.push(attachment.clearValue);
          operations.push(() => {
            const { r, g, b, a } = attachment.clearValue;
            const rgba = [r, g, b, a].map((n) => Math.round(n * 255));
            const channels =
              format === "bgra8unorm"
                ? [rgba[2], rgba[1], rgba[0], rgba[3]]
                : rgba;
            for (let i = 0; i < attachment.view.texture.data.length; i += 4)
              attachment.view.texture.data.set(channels, i);
          });
          return { end() {} };
        },
        copyTextureToBuffer(source: any, destination: any, size: number[]) {
          operations.push(() => {
            const mapped = new Uint8Array(destination.buffer.data);
            mapped.fill(222);
            for (let y = 0; y < size[1]; y++)
              mapped.set(
                source.texture.data.subarray(
                  y * size[0] * 4,
                  (y + 1) * size[0] * 4,
                ),
                y * destination.bytesPerRow,
              );
          });
        },
        finish: () => operations,
      };
    },
  };
  const gpu = {
    requestAdapter: async () => ({ requestDevice: async () => device }),
    getPreferredCanvasFormat: () => format,
  };
  return {
    canvas: canvas as unknown as HTMLCanvasElement,
    gpu: gpu as unknown as GPU,
    stats,
  };
}

test("empty readback strips aligned row padding and normalizes BGRA without changing alpha", () => {
  const source = new Uint8Array(512).fill(222);
  source.set([3, 2, 1, 4, 7, 6, 5, 8], 0);
  source.set([11, 10, 9, 12, 15, 14, 13, 16], 256);
  assert.deepEqual(
    [...new Uint8Array(unpackEmptyPixels(source, 2, 2, 256, "bgra8unorm"))],
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
  );
  assert.deepEqual(
    [...new Uint8Array(unpackEmptyPixels(source, 2, 2, 256, "rgba8unorm"))],
    [3, 2, 1, 4, 7, 6, 5, 8, 11, 10, 9, 12, 15, 14, 13, 16],
  );
  for (const invoke of [
    () => unpackEmptyPixels(source, 2, 2, 8, "rgba8unorm"),
    () => unpackEmptyPixels(source.subarray(0, 300), 2, 2, 256, "rgba8unorm"),
    () => unpackEmptyPixels(source, 1921, 2, 256, "rgba8unorm"),
    () => unpackEmptyPixels(source, 2, 2, 256, "rgba16float"),
  ])
    assert.throws(invoke);
});

test("the shared empty world clears and captures the same color in both preferred canvas formats", async (t) => {
  for (const format of ["rgba8unorm", "bgra8unorm"] as const) {
    const fake = fakeGPU(t, format),
      world = await EmptyWorld.create(fake.canvas, fake.gpu);
    world.render();
    assert.equal(fake.stats.usage, 17);
    assert.deepEqual(fake.stats.clears[0], EMPTY_WORLD_CLEAR);
    const pixels = new Uint8Array(await world.capturePixels(3, 2));
    assert.equal(pixels.length, 24);
    for (let i = 0; i < pixels.length; i += 4)
      assert.deepEqual([...pixels.subarray(i, i + 4)], [15, 23, 20, 255]);
    assert.deepEqual([fake.canvas.width, fake.canvas.height], [7, 5]);
    assert.equal(world.benchmarkInfo().captures, 1);
    assert.equal(world.benchmarkInfo().readbackHighWaterBytes, 512);
    assert.equal(fake.stats.bufferDestroy, 1);
    assert.equal(fake.stats.unmaps, 1);
    world.dispose();
    world.dispose();
    assert.equal(fake.stats.deviceDestroy, 1);
    assert.equal(fake.stats.unconfigure, 1);
    assert.throws(() => world.render(), /closed/);
  }
});

test("invalid and overlapping empty captures allocate no extra buffers and preserve the live size", async (t) => {
  const fake = fakeGPU(t),
    world = await EmptyWorld.create(fake.canvas, fake.gpu);
  await assert.rejects(world.capturePixels(0, 1), /size/);
  await assert.rejects(world.capturePixels(1921, 1), /size/);
  assert.equal(fake.stats.bufferDestroy, 0);
  fake.stats.blockMap = true;
  const pending = world.capturePixels(3, 2);
  await assert.rejects(world.capturePixels(3, 2), /current screenshot/);
  assert.throws(() => world.resize(8, 8), /screenshot/);
  const submitted = fake.stats.submits;
  world.render();
  assert.equal(fake.stats.submits, submitted);
  fake.stats.release!();
  await pending;
  assert.deepEqual([fake.canvas.width, fake.canvas.height], [7, 5]);
  world.dispose();
});

test("failed and canceled readbacks release buffers, restore only live worlds, and never report a completed capture", async (t) => {
  const failed = fakeGPU(t),
    live = await EmptyWorld.create(failed.canvas, failed.gpu);
  failed.stats.failMap = true;
  await assert.rejects(live.capturePixels(3, 2), /readback failed/);
  assert.deepEqual([failed.canvas.width, failed.canvas.height], [7, 5]);
  assert.equal(failed.stats.bufferDestroy, 1);
  assert.equal(live.benchmarkInfo().captures, 0);
  live.dispose();
  const canceled = fakeGPU(t),
    closing = await EmptyWorld.create(canceled.canvas, canceled.gpu);
  canceled.stats.blockMap = true;
  const pending = closing.capturePixels(3, 2);
  closing.dispose();
  canceled.stats.release!();
  await assert.rejects(pending, /closed/);
  assert.equal(canceled.stats.bufferDestroy, 1);
  assert.equal(canceled.stats.unmaps, 1);
  assert.equal(closing.benchmarkInfo().captures, 0);
  assert.equal(canceled.stats.deviceDestroy, 1);
});
