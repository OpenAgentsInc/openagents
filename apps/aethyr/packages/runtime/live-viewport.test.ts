import test from "node:test";
import assert from "node:assert/strict";
import { liveViewport } from "./live-viewport.ts";
test("hidden startup and later zero layout retain a positive viewport without blocking load", () => {
  let size = { width: 1280, height: 720 };
  const apply = (width: number, height: number) => {
    const next = liveViewport(width, height);
    if (next) size = next;
  };
  apply(0, 0);
  assert.deepEqual(size, { width: 1280, height: 720 });
  apply(390, 742);
  assert.deepEqual(size, { width: 390, height: 742 });
  apply(0, 742);
  apply(390, 0);
  assert.deepEqual(size, { width: 390, height: 742 });
  apply(1440, 920);
  assert.deepEqual(size, { width: 1440, height: 920 });
  for (const invalid of [-1, 1.5, NaN, Infinity, 16385]) {
    assert.throws(() => apply(invalid, 920));
    assert.throws(() => apply(1440, invalid));
    assert.deepEqual(size, { width: 1440, height: 920 });
  }
  apply(16384, 1);
  assert.deepEqual(size, { width: 16384, height: 1 });
});
