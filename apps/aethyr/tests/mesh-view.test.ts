import { test } from "node:test";
import assert from "node:assert/strict";
import { triangleEdges } from "../packages/renderer/mesh-view.ts";
test("mesh overlay uses each triangle edge and preserves primitive index offsets", () => {
  assert.deepEqual(
    [...triangleEdges(new Uint32Array([4, 5, 6, 7, 8, 9]))],
    [4, 5, 5, 6, 6, 4, 7, 8, 8, 9, 9, 7],
  );
  assert.deepEqual(
    [...triangleEdges(new Uint32Array([4, 5, 6, 7, 8, 9])).slice(3 * 2, 6 * 2)],
    [7, 8, 8, 9, 9, 7],
  );
  assert.throws(() => triangleEdges(new Uint32Array([1, 2])), /mesh/);
});
