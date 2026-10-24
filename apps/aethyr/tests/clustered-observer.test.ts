import test from "node:test";
import assert from "node:assert/strict";
import { ClusteredLighting } from "../packages/renderer/clustered-lighting.ts";
import { buildClusteredLights } from "../packages/renderer/clustered-lights.ts";
import { perspective } from "../packages/renderer/math.ts";

test("disabling the optional lighting observer stops its clocks while preserving uploaded light lists", () => {
  const savedUsage = Object.getOwnPropertyDescriptor(
      globalThis,
      "GPUBufferUsage",
    ),
    savedPerformance = Object.getOwnPropertyDescriptor(
      globalThis,
      "performance",
    );
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { STORAGE: 1, COPY_DST: 2 },
  });
  let clocks = 0,
    observations = 0;
  Object.defineProperty(globalThis, "performance", {
    configurable: true,
    value: { now: () => ++clocks },
  });
  const writes: Uint8Array[] = [];
  const device = {
    limits: { maxStorageBufferBindingSize: 1 << 26 },
    createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
    queue: {
      writeBuffer(_buffer: unknown, _offset: number, data: ArrayBufferView) {
        writes.push(
          new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice(),
        );
      },
    },
  } as unknown as GPUDevice;
  const lighting = new ClusteredLighting(
    device,
    buildClusteredLights,
    () => observations++,
  );
  const input = {
    viewProjection: perspective(1, 1, 0.1, 100),
    eye: [0, 0, 0] as [number, number, number],
    forward: [0, 0, -1] as [number, number, number],
    width: 128,
    height: 128,
    lights: [],
  };
  try {
    lighting.setObserving(false);
    lighting.update("main", input, 1);
    assert.equal(clocks, 0);
    assert.equal(observations, 0);
    lighting.setObserving(true);
    lighting.update("main", input, 2);
    assert.equal(clocks, 2);
    assert.equal(observations, 1);
    assert.deepEqual(writes[0], writes[1]);
    lighting.setObserving(false);
    lighting.update("main", input, 3);
    assert.equal(clocks, 2);
    assert.equal(observations, 1);
    assert.deepEqual(writes[0], writes[2]);
  } finally {
    lighting.dispose();
    for (const [name, descriptor] of [
      ["GPUBufferUsage", savedUsage],
      ["performance", savedPerformance],
    ] as const)
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else Reflect.deleteProperty(globalThis, name);
  }
});
