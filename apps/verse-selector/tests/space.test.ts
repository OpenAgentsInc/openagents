import { test } from "node:test";
import assert from "node:assert/strict";
import { camera, project, geometry, worlds } from "../src/space.ts";
test("the target projects to center and the world catalog has stable unique ids", () => {
  const { matrix } = camera(0.13, 0.57, 27, [0, 0, 0], 1.6);
  const p = project([0, 0, 0], matrix);
  assert.ok(p.visible);
  assert.ok(Math.abs(p.x - 0.5) < 0.0001 && Math.abs(p.y - 0.5) < 0.0001);
  assert.equal(new Set(worlds.map((w) => w.id)).size, worlds.length);
});
test("bounded geometry contains finite packed vertices for planets and their orbits", () => {
  const data = geometry();
  for (const values of Object.values(data)) {
    assert.equal(values.length % 10, 0);
    assert.ok(values.every(Number.isFinite));
    assert.ok(values.byteLength < 2_000_000);
  }
});
