import assert from "node:assert/strict";
import test from "node:test";
import {
  classifyRasterOrientation,
  rasterFrontFace,
  splitRasterOrientationDraws,
} from "../packages/renderer/raster-orientation.ts";
import { modelMatrix } from "../packages/renderer/math.ts";
import { compactInstanceDraws } from "../packages/renderer/culling.ts";

const matrix = (x = 1, y = 1, z = 1) => {
  const value = new Float32Array(modelMatrix([0, 0, -5]));
  value[0] = x;
  value[5] = y;
  value[10] = z;
  return value;
};

test("raster orientation follows the shader cross/dot expression and handles mirrored views", () => {
  for (const [x, y, z, expected] of [
    [1, 1, 1, "positive"],
    [-1, 1, 1, "negative"],
    [-1, -1, 1, "positive"],
    [-1, -1, -1, "negative"],
    [0.0001, 4, -500, "negative"],
    [0, 1, 1, "degenerate"],
  ] as const)
    assert.equal(classifyRasterOrientation(matrix(x, y, z)), expected);
  for (const angle of [0, 0.2, 1.2, 3.14]) {
    const rotated = new Float32Array(modelMatrix([100, -2, 300], angle, 2));
    assert.equal(classifyRasterOrientation(rotated), "positive");
    for (let index = 0; index < 3; index++) rotated[index] *= -1;
    assert.equal(classifyRasterOrientation(rotated), "negative");
  }
  assert.equal(rasterFrontFace("ccw", "positive"), "ccw");
  assert.equal(rasterFrontFace("ccw", "negative"), "cw");
  assert.equal(rasterFrontFace("cw", "negative"), "ccw");
  assert.equal(rasterFrontFace("cw", "degenerate"), "cw");
});

test("near-singular, underflowing, and overflowing float32 determinants retain shader culling", () => {
  const cancellation = matrix();
  cancellation[0] = 1;
  cancellation[1] = 1;
  cancellation[4] = 1;
  cancellation[5] = 1 + 1e-7;
  assert.equal(classifyRasterOrientation(cancellation), "degenerate");
  assert.equal(
    classifyRasterOrientation(matrix(1e-20, 1e-20, 1e-20)),
    "degenerate",
  );
  assert.equal(
    classifyRasterOrientation(matrix(1e20, 1e20, 1e20)),
    "degenerate",
  );
  const large = matrix(1e10, 1e10, -1e10);
  assert.equal(classifyRasterOrientation(large), "negative");
  const invalid = matrix();
  invalid[12] = NaN;
  assert.throws(() => classifyRasterOrientation(invalid), /finite/);
  assert.throws(() => classifyRasterOrientation(matrix(), -1), /complete/);
  assert.throws(
    () => classifyRasterOrientation(new Float32Array(15)),
    /complete/,
  );
});

test("splits preserve contiguous source order, exact draw coverage, all metadata, and input arrays", () => {
  const models = new Float32Array(
    [1, 1, -1, -1, 0, 1, -1].flatMap((sign) => [...matrix(sign)]),
  );
  const draws = [
    { firstInstance: 0, count: 7, material: 4, wind: true, firstIndex: 12 },
    { firstInstance: 1, count: 4, material: 8, wind: false, firstIndex: 48 },
    { firstInstance: 7, count: 0, material: 3, wind: true, firstIndex: 0 },
  ];
  const originalModels = models.slice(),
    originalDraws = structuredClone(draws);
  const result = splitRasterOrientationDraws(models, draws);
  assert.deepEqual(
    result.map((draw) => [
      draw.firstInstance,
      draw.count,
      draw.rasterOrientation,
    ]),
    [
      [0, 2, "positive"],
      [2, 2, "negative"],
      [4, 1, "degenerate"],
      [5, 1, "positive"],
      [6, 1, "negative"],
      [1, 1, "positive"],
      [2, 2, "negative"],
      [4, 1, "degenerate"],
      [7, 0, "degenerate"],
    ],
  );
  for (const original of draws) {
    const split = result.filter((draw) => draw.material === original.material);
    assert.deepEqual(
      split.flatMap((draw) =>
        Array.from(
          { length: draw.count },
          (_, index) => draw.firstInstance + index,
        ),
      ),
      Array.from(
        { length: original.count },
        (_, index) => original.firstInstance + index,
      ),
    );
    assert.ok(
      split.every(
        (draw) =>
          draw.wind === original.wind &&
          draw.firstIndex === original.firstIndex,
      ),
    );
  }
  assert.deepEqual(models, originalModels);
  assert.deepEqual(draws, originalDraws);
  assert.throws(
    () => splitRasterOrientationDraws(new Float32Array(17), []),
    /complete/,
  );
  assert.throws(
    () => splitRasterOrientationDraws(models, [{ firstInstance: 6, count: 2 }]),
    /within/,
  );
});

test("stable visibility compaction preserves orientation runs without reordering instances", () => {
  const signs = [1, 1, -1, -1, 1, -1];
  const instances = signs.map((sign, index) => ({
    mesh: 0,
    model: [...matrix(sign)],
  }));
  const models = new Float32Array(
    instances.flatMap((instance) => instance.model),
  );
  const draws = splitRasterOrientationDraws(models, [
    { firstInstance: 0, count: signs.length, wind: true },
  ]);
  const visible = new Uint8Array([0, 1, 0, 1, 1, 0]);
  const result = compactInstanceDraws(models, draws, visible);
  assert.deepEqual(
    result.draws.map((draw) => [
      draw.firstInstance,
      draw.count,
      draw.rasterOrientation,
    ]),
    [
      [0, 1, "positive"],
      [1, 1, "negative"],
      [2, 1, "positive"],
    ],
  );
  for (const draw of result.draws)
    for (
      let index = draw.firstInstance;
      index < draw.firstInstance + draw.count;
      index++
    )
      assert.equal(
        classifyRasterOrientation(result.models, index * 16),
        draw.rasterOrientation,
      );
});
