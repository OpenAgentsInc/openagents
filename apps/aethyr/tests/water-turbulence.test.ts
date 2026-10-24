import assert from "node:assert/strict";
import { test } from "node:test";
import {
  OWNED_WATER_TURBULENCE,
  validateWaterTurbulence,
  waterSurfaceField,
  waterAirFraction,
} from "../packages/renderer/water-turbulence.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
test("water height gradients match independent finite differences through reverse captures", () => {
  const p = [0.13, 2.16, -0.83],
    epsilon = 1e-6;
  for (const time of [0, 3, 10, MAX_WORLD_ANIMATION_TIME])
    for (const micro of [false, true]) {
      const field = waterSurfaceField(p, time, 0.004, micro);
      for (let axis = 0; axis < 3; axis++) {
        const plus = [...p],
          minus = [...p];
        plus[axis] += epsilon;
        minus[axis] -= epsilon;
        const derivative =
          (waterSurfaceField(plus, time, 0.004, micro).height -
            waterSurfaceField(minus, time, 0.004, micro).height) /
          (2 * epsilon);
        assert.ok(Math.abs(derivative - field.gradient[axis]) < 1e-7);
      }
      assert.ok(Math.abs(field.height) <= 0.004);
      assert.ok(field.gradient.every(Number.isFinite));
    }
  assert.deepEqual(
    waterSurfaceField(p, 3, 0.004),
    waterSurfaceField(p, 11, 0.004),
  );
  const before = waterSurfaceField(p, 0, 0.004);
  waterSurfaceField(p, 10, 0.004);
  assert.deepEqual(waterSurfaceField(p, 0, 0.004), before);
});
test("air entrainment is zero at zero optical depth and composes as extinction", () => {
  assert.equal(waterAirFraction(0.015, 0, 20), 0);
  assert.equal(waterAirFraction(0, 1, 20), 0);
  const a = waterAirFraction(0.015, 0.6, 20),
    b = waterAirFraction(0.03, 0.6, 20);
  assert.ok(Math.abs(b - (1 - (1 - a) ** 2)) < 1e-15);
  assert.ok(a > 0 && a < 1);
  assert.throws(() => waterAirFraction(0.01, 2, 20));
});
test("water deformation caps prevent unbounded displacement or invalid age", () => {
  assert.equal(
    validateWaterTurbulence({ ...OWNED_WATER_TURBULENCE })
      .geometryAmplitudeMeters,
    0.004,
  );
  for (const property of Object.keys(OWNED_WATER_TURBULENCE))
    assert.throws(() =>
      validateWaterTurbulence({
        ...OWNED_WATER_TURBULENCE,
        [property]: Infinity,
      }),
    );
  assert.throws(() =>
    waterSurfaceField([0, 0, 0], MAX_WORLD_ANIMATION_TIME + 1, 0.004),
  );
  assert.throws(() => waterSurfaceField([0, 0, NaN], 0, 0.004));
});
