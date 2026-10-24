import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";

test("optional request timers leave the actual Rust world identical and off work has no timing calls", async () => {
  const bytes = await readFile(
    new URL("../public/kernel.wasm", import.meta.url),
  );
  const plain = await RuntimeKernel.fromBytes(bytes),
    observed = await RuntimeKernel.fromBytes(bytes);
  const init = {
    type: "init",
    entities: [],
    colliders: [],
    rooms: [],
    groundY: 0,
    spawn: { position: [0, 0, 0], yaw: 0, pitch: 0, mode: "walk" },
  };
  assert.deepEqual(plain.request(init), observed.request(init));
  observed.setProfiling(true);
  const original = Object.getOwnPropertyDescriptor(globalThis, "performance");
  let calls = 0;
  Object.defineProperty(globalThis, "performance", {
    configurable: true,
    value: { now: () => ++calls },
  });
  try {
    for (let frame = 0; frame < 120; frame++) {
      const input = {
        type: "step",
        elapsed: 1 / 60,
        input: {
          forward: 1,
          right: frame < 60 ? 0 : 1,
          up: 0,
          sprint: false,
          jump: frame === 12,
          yawDelta: 0,
          pitchDelta: 0,
        },
      };
      const before = calls;
      const a = plain.request(input);
      assert.equal(calls, before);
      const b = observed.request(input);
      assert.deepEqual(a, b);
      assert(calls > before);
      const timings = observed.lastRequestTimings!;
      assert(Object.values(timings).every((n) => Number.isFinite(n) && n >= 0));
      assert.equal(
        timings.totalMs,
        timings.encodeMs +
          timings.wasmRequestMs +
          timings.decodeMs +
          timings.overheadMs,
      );
    }
    observed.setProfiling(false);
    const before = calls;
    observed.request({ type: "snapshot" });
    assert.equal(calls, before);
    assert.equal(observed.lastRequestTimings, null);
  } finally {
    if (original) Object.defineProperty(globalThis, "performance", original);
    else Reflect.deleteProperty(globalThis, "performance");
  }
});
