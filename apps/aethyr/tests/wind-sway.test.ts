import assert from "node:assert/strict";
import { test } from "node:test";
import {
  windRotationOffset,
  windSwayOffset,
  windSwayBound,
  type WindSway,
} from "../packages/renderer/wind-sway.ts";
import { cross, type V3 } from "../packages/renderer/math.ts";
const settings: WindSway = {
  enabled: true,
  pivot: [0, 0, 0],
  worldUp: [0, 1, 0],
  windDirection: [0, 0, 1],
  radiusMeters: 2,
  windSpeed: 0.5,
  swaySpeed: 0.2,
  swayAmount: 0.005,
};
const close = (a: V3, b: V3, tolerance = 1e-10) =>
  a.forEach((n, i) =>
    assert.ok(Math.abs(n - b[i]) < tolerance, `${a} != ${b}`),
  );
test("source rotation branch pins green-zero vertices and has a turn-based, replayable clock", () => {
  const point: V3 = [0, 2, 0];
  close(windSwayOffset(point, 0, 2.5, settings), [0, 0, 0]);
  close(
    windSwayOffset(point, 1, 2.5, { ...settings, enabled: false }),
    [0, 0, 0],
  );
  close(windSwayOffset(point, 1, 2.5, settings), [0, -0.01, 0.01]);
  close(
    windSwayOffset(point, 1, 12.5, settings),
    windSwayOffset(point, 1, 2.5, settings),
  );
  close(
    windSwayOffset(point, 0.25, 2.5, settings),
    windSwayOffset(point, 1, 2.5, settings).map((n) => n * 0.25) as V3,
  );
  // Source world-centimeter origin contributes phase, independent of source bounds center.
  close(
    windSwayOffset([0, 2, 0.0025], 1, 0, {
      ...settings,
      pivot: [0, 0, 0.0025],
    }),
    [0, -0.01, 0.01],
  );
  close(
    windSwayOffset(point, 1, 2.5, { ...settings, windDirection: [0, 1, 0] }),
    [0, 0, 0],
  );
});
test("reflection of the source coordinate basis also reverses the axial vector", () => {
  const map = (v: V3): V3 => [v[0], v[2], v[1]],
    sourcePoint: V3 = [0.3, 1, 0.7],
    sourcePivot: V3 = [0.1, 0.2, 0.1],
    sourceAxis = cross([0, 0, 1], [0, 1, 0]),
    canonicalAxis = cross(map([0, 0, 1]), map([0, 1, 0]));
  close(
    windRotationOffset(map(sourcePoint), map(sourcePivot), canonicalAxis, 0.17),
    map(windRotationOffset(sourcePoint, sourcePivot, sourceAxis, 0.17)),
  );
});
test("the phase-independent conservative bound holds for negative, nonuniform and large pivot offsets", () => {
  for (const scale of [
    [-1, 2, 0.5],
    [0.5, -1, 3],
    [1, 1, 1],
  ])
    for (let i = 0; i < 300; i++) {
      const angle = i * 0.371,
        point: V3 = [
          Math.cos(angle) * scale[0],
          Math.sin(angle) * scale[1],
          Math.cos(angle * 0.7) * scale[2],
        ],
        distance = Math.hypot(...point),
        green = (i % 17) / 16;
      const displacement = windSwayOffset(point, green, i * 0.19, {
        ...settings,
        swayAmount: i % 2 ? -0.005 : 0.005,
      });
      assert.ok(
        Math.hypot(...displacement) <=
          windSwayBound(distance, green, settings) + 1e-12,
      );
    }
  assert.throws(() =>
    windSwayOffset([0, 2, 0], 1, 1, { ...settings, radiusMeters: 0 }),
  );
  assert.throws(() => windSwayOffset([0, 2, 0], NaN, 1, settings));
  assert.throws(() => windSwayOffset([0, 2, 0], 1, Infinity, settings));
});
