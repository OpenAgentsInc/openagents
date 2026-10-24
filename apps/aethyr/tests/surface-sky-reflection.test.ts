import assert from "node:assert/strict";
import test from "node:test";
import {
  SURFACE_SKY_GGX_BANDS,
  sh9RoughReflection,
  surfaceSkyReflectionBands,
} from "../packages/renderer/surface-sky-reflection.ts";

/** Direct one-dimensional quadrature of NoL-weighted GGX, independent of Hammersley. */
function continuousBands(roughness: number) {
  const count = 32768,
    a = 1 + roughness ** 4,
    b = roughness ** 4 - 1,
    sums = [0, 0, 0];
  for (let i = 0; i <= count; i++) {
    const z = i / count,
      weight =
        ((i === 0 || i === count ? 1 : i % 2 ? 4 : 2) * z) / (a + b * z) ** 2;
    sums[0] += weight;
    sums[1] += weight * z;
    sums[2] += (weight * (3 * z * z - 1)) / 2;
  }
  return [1, sums[1] / sums[0], sums[2] / sums[0]];
}

test("finite prefilter moments stay near an independent continuous GGX integral", () => {
  for (const [roughness, q1, q2] of SURFACE_SKY_GGX_BANDS.slice(2)) {
    const integral = continuousBands(roughness);
    assert.ok(Math.abs(q1 - integral[1]) < 0.0053);
    assert.ok(Math.abs(q2 - integral[2]) < 0.0118);
  }
  for (let i = 0; i <= 600; i++) {
    const roughness = 0.4 + i / 1000,
      q = surfaceSkyReflectionBands(roughness)!;
    assert.ok(q.every((v) => v >= 0 && v <= 1));
    if (i) {
      const previous = surfaceSkyReflectionBands(roughness - 0.001)!;
      assert.ok(q[1] <= previous[1] + 1e-12 && q[2] <= previous[2] + 1e-12);
    }
  }
});

test("constant incoming radiance is exact and independent of roughness or view direction", () => {
  const c = new Float64Array(27);
  c.set([2 / 0.2820947918, 0.5 / 0.2820947918, 7 / 0.2820947918]);
  for (const roughness of [0.4, 0.43, 0.5, 0.7, 1])
    for (const direction of [
      [1, 0, 0],
      [0, -1, 0],
      [0.2, 0.4, 0.6],
    ]) {
      const actual = sh9RoughReflection(c, direction, roughness);
      actual.forEach((v, i) =>
        assert.ok(Math.abs(v - [2, 0.5, 7, 1][i]) < 1e-12),
      );
    }
  assert.deepEqual(
    sh9RoughReflection(new Float32Array(27), [0, 0, 1], 0.5),
    [0, 0, 0, 1],
  );
});

test("individual radiance bands deconvolve Lambert once and reconvolve the zonal filter", () => {
  const c = new Float64Array(27);
  c[0] = 10 / 0.2820947918;
  // L1 z and L2 (3z²-1); the stored field already includes Lambert convolution.
  c[2 * 3] = 2 / 3 / 0.4886025119;
  c[6 * 3] = 0.25 / (2 * 0.3153915653);
  for (const roughness of [0.4, 0.47, 0.6, 0.9, 1]) {
    const q = surfaceSkyReflectionBands(roughness)!;
    assert.ok(
      Math.abs(
        sh9RoughReflection(c, [0, 0, 1], roughness)[0] - (10 + q[1] + q[2]),
      ) < 1e-10,
    );
    assert.ok(
      Math.abs(
        sh9RoughReflection(c, [0, 0, -1], roughness)[0] - (10 - q[1] + q[2]),
      ) < 1e-10,
    );
    assert.ok(
      Math.abs(
        sh9RoughReflection(c, [1, 0, 0], roughness)[0] - (10 - q[2] / 2),
      ) < 1e-10,
    );
  }
});

test("negative SH ringing and unsupported inputs request fallback rather than invented radiance", () => {
  const c = new Float64Array(27);
  c[2 * 3] = 1;
  assert.deepEqual(sh9RoughReflection(c, [0, 0, -1], 0.5), [0, 0, 0, 0]);
  assert.equal(sh9RoughReflection(c, [0, 0, 1], 0.5)[3], 1);
  for (const roughness of [0, 0.399999, 1.000001, NaN, Infinity])
    assert.deepEqual(sh9RoughReflection(c, [0, 0, 1], roughness), [0, 0, 0, 0]);
  for (const d of [
    [0, 0, 0],
    [NaN, 0, 1],
    [Infinity, 0, 1],
  ])
    assert.deepEqual(sh9RoughReflection(c, d, 0.5), [0, 0, 0, 0]);
  c[0] = NaN;
  assert.deepEqual(sh9RoughReflection(c, [0, 0, 1], 0.5), [0, 0, 0, 0]);
  assert.deepEqual(
    sh9RoughReflection(new Float32Array(26), [0, 0, 1], 0.5),
    [0, 0, 0, 0],
  );
});
