import assert from "node:assert/strict";
import { test } from "node:test";
import {
  validateWaterAirModel,
  waterAirPhase,
  waterNozzleCoverage,
  waterAirFromDiffuseSH,
} from "../packages/renderer/water-air-scattering.ts";
test("entrained-air phase is normalized with the correct both-away forward direction", () => {
  for (const g of [-0.8, -0.45, 0, 0.45, 0.8]) {
    let integral = 0,
      moment = 0;
    const count = 20000;
    for (let i = 0; i < count; i++) {
      const mu = -1 + (2 * (i + 0.5)) / count,
        p = (waterAirPhase(mu, g) * 4 * Math.PI) / count;
      integral += p;
      moment += mu * p;
    }
    assert.ok(Math.abs(integral - 1) < 1e-6);
    assert.ok(Math.abs(moment + g) < 1e-6);
  }
  assert.ok(waterAirPhase(-1, 0.45) > waterAirPhase(1, 0.45));
  assert.equal(waterAirPhase(0.3, 0), 1 / (4 * Math.PI));
});
test("SH volume convolution preserves constant radiance and reverses the odd-band view sign", () => {
  const sh = new Float64Array(36);
  for (let c = 0; c < 3; c++) sh[c] = [1, 2, 3][c] / 0.2820947918;
  for (const g of [0, 0.45, 0.8])
    assert.deepEqual(waterAirFromDiffuseSH(sh, [-1, 0, 0], g), [1, 2, 3]);
  sh[3 * 4] = 0.2 * (2 / 3);
  sh[6 * 4] = 0.1 * 0.25;
  const output = waterAirFromDiffuseSH(sh, [-1, 0, 0], 0.45);
  assert.ok(
    Math.abs(
      output[0] -
        (1 - 0.2 * 0.45 * 0.4886025119 - 0.1 * 0.45 ** 2 * 0.3153915653),
    ) < 1e-12,
  );
});
test("declared nozzle aeration changes inlet coverage and omission can retain zero inlet", () => {
  assert.equal(waterNozzleCoverage(0, 0), 0);
  assert.equal(waterNozzleCoverage(0, 0.2), 0.2);
  assert.equal(waterNozzleCoverage(1, 0.2), 1);
  assert.equal(
    validateWaterAirModel({
      kind: "henyey-greenstein-v1",
      nozzleFraction: 0.2,
      asymmetry: 0.45,
    }).nozzleFraction,
    0.2,
  );
  assert.throws(() =>
    validateWaterAirModel({
      kind: "henyey-greenstein-v1",
      nozzleFraction: 2,
      asymmetry: 0.45,
    }),
  );
  assert.throws(() => waterAirPhase(1, 0.99));
});

test("the owning turbulence contract rejects unknown phase data without changing omitted defaults", async () => {
  const { validateWaterTurbulence, OWNED_WATER_TURBULENCE } =
    await import("../packages/renderer/water-turbulence.ts");
  const original = { ...OWNED_WATER_TURBULENCE };
  assert.equal(validateWaterTurbulence(original).airModel, undefined);
  assert.throws(() =>
    validateWaterTurbulence({
      ...original,
      airModel: {
        kind: "unknown",
        nozzleFraction: 0.2,
        asymmetry: 0.45,
      } as never,
    }),
  );
  assert.deepEqual(original, { ...OWNED_WATER_TURBULENCE });
});

test("nozzle-only foam is a separate declared model with no unused phase knob", () => {
  assert.equal(
    validateWaterAirModel({ kind: "diffuse-foam-v1", nozzleFraction: 0.2 })
      .kind,
    "diffuse-foam-v1",
  );
  assert.throws(() =>
    validateWaterAirModel({
      kind: "diffuse-foam-v1",
      nozzleFraction: 0.2,
      asymmetry: 0,
    } as never),
  );
});
test("source-energy foam retains coverage settings without an intensity or phase knob", () => {
  for (const kind of [
    "source-energy-foam-v1",
    "source-energy-foam-additive-v1",
  ] as const) {
    const model = { kind, nozzleFraction: 0.2 };
    assert.equal(validateWaterAirModel(model), model);
    for (const addition of [
      { asymmetry: 0 },
      { emissionGain: 2 },
      { exposure: 1 },
    ])
      assert.throws(() =>
        validateWaterAirModel({ ...model, ...addition } as never),
      );
    assert.equal(
      waterNozzleCoverage(0.5, model.nozzleFraction),
      waterNozzleCoverage(0.5, 0.2),
    );
  }
});
