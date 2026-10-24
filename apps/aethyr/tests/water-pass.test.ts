import assert from "node:assert/strict";
import { test } from "node:test";
import {
  buildWaterBasins,
  type WaterBasin,
} from "../packages/renderer/water-pass.ts";

const basin: WaterBasin = {
  id: "synthetic-basin",
  polygonXZ: [
    [-2, -2],
    [2, -2],
    [2, 2],
    [-2, 2],
  ],
  surfaceY: 1,
  floorY: 0.5,
};
test("basin geometry preserves its polygon, upward winding, and fallback depth", () => {
  for (const polygonXZ of [basin.polygonXZ, [...basin.polygonXZ].reverse()]) {
    const mesh = buildWaterBasins([{ ...basin, polygonXZ }]);
    assert.equal(mesh.vertices.length, 4 * 12);
    assert.equal(mesh.indices.length, 6);
    assert.equal(mesh.vertices[7], 0.5);
    for (let i = 0; i < mesh.indices.length; i += 3) {
      const [a, b, c] = Array.from(mesh.indices.subarray(i, i + 3)).map(
        (index) =>
          Array.from(mesh.vertices.subarray(index * 12, index * 12 + 3)),
      );
      assert.ok(
        (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]) > 0,
      );
    }
  }
});
test("basin transforms preserve horizontal world height and nonuniform depth scale", () => {
  const mesh = buildWaterBasins([
    {
      ...basin,
      model: [0.6, 0, 0, 0, 0, 0.8, 0, 0, 0, 0, 0.6, 0, 10, 2, 3, 1],
    },
  ]);
  for (let i = 0; i < mesh.vertices.length; i += 12) {
    assert.ok(Math.abs(mesh.vertices[i + 1] - 2.8) < 1e-6);
    assert.ok(Math.abs(mesh.vertices[i + 7] - 0.4) < 1e-6);
  }
  assert.ok(Math.abs(mesh.vertices[0] - 8.8) < 1e-6);
});
test("invalid, crossed, concave, tilted and degenerate basin polygons are rejected", () => {
  for (const patch of [
    {
      polygonXZ: [
        [0, 0],
        [1, 1],
        [0, 1],
        [1, 0],
      ],
    },
    {
      polygonXZ: [
        [0, 0],
        [2, 0],
        [0.5, 0.5],
        [0, 2],
      ],
    },
    {
      polygonXZ: [
        [0, 0],
        [0, 0],
        [1, 0],
      ],
    },
    {
      polygonXZ: [
        [0, 0],
        [1, 0],
        [2, 0],
      ],
    },
    {
      polygonXZ: [
        [NaN, 0],
        [1, 0],
        [0, 1],
      ],
    },
    { surfaceY: Infinity },
    { floorY: 2 },
    { model: [1, 0.1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] },
    { model: [1, 0, 0, 0.1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] },
  ])
    assert.throws(() =>
      buildWaterBasins([{ ...basin, ...patch } as WaterBasin]),
    );
  assert.throws(() => buildWaterBasins([basin, basin]), /unique/);
  assert.throws(
    () =>
      buildWaterBasins(
        Array.from({ length: 65 }, (_, i) => ({ ...basin, id: String(i) })),
      ),
    /64/,
  );
  assert.deepEqual(buildWaterBasins([]), {
    vertices: new Float32Array(),
    indices: new Uint32Array(),
  });
});
