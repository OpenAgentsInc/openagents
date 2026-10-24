import { test } from "node:test";
import assert from "node:assert/strict";
import {
  temperatureTint,
  lightIntensity,
  lightAttenuation,
  sunAngles,
} from "../packages/renderer/lighting.ts";
test("source light conversion preserves photometric unit differences", () => {
  const point = {
    intensity: 4 * Math.PI,
    intensityUnits: "lumens",
    kind: "point",
  } as any;
  assert.equal(lightIntensity(point), 1);
  assert.equal(
    lightIntensity({ ...point, intensity: 5, intensityUnits: "candelas" }),
    5,
  );
  const spot = {
    ...point,
    kind: "spot",
    intensity: 2 * Math.PI,
    outerConeAngleRadians: Math.PI / 2,
  };
  assert.ok(Math.abs(lightIntensity(spot) - 1) < 1e-8);
});
test("legacy and inverse-square unitless lights retain different source semantics", () => {
  const light = {
    kind: "spot",
    intensity: 20,
    intensityUnits: "unitless",
    range: 11,
    falloffExponent: 5,
    inverseSquaredFalloff: false,
  } as any;
  assert.equal(lightIntensity(light), 20);
  assert.equal(
    lightIntensity({ ...light, inverseSquaredFalloff: true }),
    0.032,
  );
  assert.ok(Math.abs(lightAttenuation(light, 5) - 0.3143614719437349) < 1e-12);
  assert.equal(lightAttenuation(light, 11), 0);
  assert.equal(lightAttenuation(light, 12), 0);
  const physical = {
    ...light,
    intensity: 1,
    intensityUnits: "candelas",
    inverseSquaredFalloff: true,
    range: 1000,
  };
  assert.ok(Math.abs(lightAttenuation(physical, 0.5) - 1 / 0.2501) < 1e-9);
  assert.equal(
    lightIntensity({
      ...light,
      kind: "directional",
      intensity: 25,
      intensityUnits: "lux",
    }),
    25,
  );
  assert.throws(() => lightIntensity({ ...light, intensity: Number.NaN }));
  assert.throws(() =>
    lightIntensity({ ...physical, intensityUnits: "unknown" }),
  );
  assert.throws(() => lightAttenuation({ ...light, falloffExponent: -1 }, 1));
});
test("temperature tint has a white reference and warmer low temperatures", () => {
  assert.deepEqual(temperatureTint(6500), [1, 1, 1]);
  const warm = temperatureTint(3000);
  assert.ok(warm[0] > warm[1] && warm[1] > warm[2]);
});
test("sun controls invert emitted rays to surface-to-light directions", () => {
  assert.deepEqual(sunAngles({ direction: [0, -1, 0] } as any), {
    sunAzimuth: 180,
    sunElevation: 90,
  });
  assert.equal(sunAngles({ direction: [1, 0, 0] } as any).sunAzimuth, 270);
});
