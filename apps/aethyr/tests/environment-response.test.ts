import assert from "node:assert/strict";
import test from "node:test";
import {
  directFresnel,
  environmentBrdfWeight,
  environmentGrazingReflectance,
  occludedSkyReflection,
} from "../packages/renderer/environment-response.ts";

test("local reflection replaces the probe-occluded panorama without a second visibility factor", () => {
  const local: [number, number, number] = [0.2, 0.3, 0.4];
  for (const visibility of [0, 0.2, 1])
    for (const confidence of [0, 0.4, 1])
      assert.deepEqual(
        occludedSkyReflection([4, 3, 2], visibility, confidence, local),
        local,
      );
  assert.deepEqual(
    occludedSkyReflection([4, 3, 2], 0, 1, [0, 0, 0]),
    [0, 0, 0],
  );
});

test("missing local reflection retains the previous probe visibility equation", () => {
  for (const visibility of [0, 0.3, 1])
    for (const confidence of [0, 0.4, 1])
      assert.deepEqual(
        occludedSkyReflection([4, 3, 2], visibility, confidence),
        [4, 3, 2].map((v) => v * (1 + (visibility - 1) * confidence)),
      );
  assert.throws(() => occludedSkyReflection([1, 2, 3], -0.1, 1));
  assert.throws(() => occludedSkyReflection([1, 2, 3], 1, 1, [0, NaN, 0]));
});

test("direct-light Fresnel shares the authored grazing endpoint", () => {
  for (const f0 of [
    [0, 0, 0],
    [0.00686862, 0.00686862, 0.00686862],
    [0.01, 0.002, 0.001],
  ]) {
    assert.deepEqual(directFresnel(f0, 1), f0);
    const endpoint = environmentGrazingReflectance(f0);
    for (const value of directFresnel(f0, 0))
      assert.ok(Math.abs(value - endpoint) < 1e-15);
    for (const cosine of [0, 0.01, 0.25, 0.5, 0.9, 1]) {
      directFresnel(f0, cosine).forEach((value, i) => {
        assert.ok(value >= f0[i] && value <= endpoint);
        assert.ok(value <= f0[i] + (1 - f0[i]) * (1 - cosine) ** 5);
      });
    }
  }
});

test("ordinary direct-light Fresnel preserves the previous response", () => {
  for (const f0 of [
    [0.02, 0.02, 0.02],
    [0.04, 0.04, 0.04],
    [0.93, 0.004, 0.002],
    [0.001, 0.001, 0.7],
  ])
    for (const cosine of [0, 0.001, 0.2, 0.8, 1])
      assert.deepEqual(
        directFresnel(f0, cosine),
        f0.map((v) => v + (1 - v) * (1 - cosine) ** 5),
      );
  for (const cosine of [-0.01, 1.01, NaN, Infinity])
    assert.throws(() => directFresnel([0.01, 0.01, 0.01], cosine));
});

test("zero authored specular suppresses both split-sum terms", () => {
  assert.equal(environmentGrazingReflectance([0, 0, 0]), 0);
  assert.deepEqual(environmentBrdfWeight([0, 0, 0], [0.8, 0.2]), [0, 0, 0]);
  assert.deepEqual(
    environmentBrdfWeight([0.01, 0.01, 0.01], [0, 1]),
    [0.5, 0.5, 0.5],
  );
  assert.deepEqual(
    environmentBrdfWeight([0.01, 0.01, 0.01], [1, 0]),
    [0.01, 0.01, 0.01],
  );
});

test("normal reflectance keeps the existing split-sum response exactly", () => {
  for (const f0 of [
    [0.02, 0.02, 0.02],
    [0.04, 0.04, 0.04],
    [0.93, 0.004, 0.002],
    [0.001, 0.001, 0.7],
  ]) {
    assert.equal(environmentGrazingReflectance(f0), 1);
    for (const split of [
      [0.743184, 0.206799],
      [0, 1],
      [1, 0],
    ])
      assert.deepEqual(
        environmentBrdfWeight(f0, split),
        f0.map((v) => v * split[0] + split[1]),
      );
  }
});

test("colored low reflectance uses its maximum channel without tinting F90", () => {
  const split = [0.75, 0.2];
  for (const f0 of [
    [0.01, 0.002, 0.001],
    [0.001, 0.01, 0.002],
  ]) {
    assert.equal(environmentGrazingReflectance(f0), 0.5);
    assert.deepEqual(
      environmentBrdfWeight(f0, split),
      f0.map((v) => v * 0.75 + 0.1),
    );
  }
  const f0 = 0.006868620403110981;
  const value = environmentBrdfWeight(
    [f0, f0, f0],
    [0.7431840216740966, 0.20679916767403483],
  );
  assert.ok(Math.abs(value[0] - 0.076125898056149) < 1e-12);
});

test("reflection remains continuous at the authoring threshold", () => {
  const below = environmentBrdfWeight([0.02 - 1e-10, 0, 0], [0.7, 0.3]);
  const at = environmentBrdfWeight([0.02, 0, 0], [0.7, 0.3]);
  const above = environmentBrdfWeight([0.02 + 1e-10, 0, 0], [0.7, 0.3]);
  for (let i = 0; i < 3; i++) {
    assert.ok(Math.abs(below[i] - at[i]) < 2e-9);
    assert.ok(Math.abs(above[i] - at[i]) < 2e-9);
  }
});

test("reject malformed reflectance and coefficient inputs", () => {
  for (const f0 of [
    [0, 0],
    [-0.1, 0, 0],
    [1.1, 0, 0],
    [NaN, 0, 0],
  ])
    assert.throws(() => environmentBrdfWeight(f0, [1, 0]));
  for (const split of [[1], [Infinity, 0], [0, -0.1], [4.1, 0]])
    assert.throws(() => environmentBrdfWeight([0.01, 0.01, 0.01], split));
});
