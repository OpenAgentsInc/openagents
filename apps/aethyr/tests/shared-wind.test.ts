import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_WIND_FIELD,
  WIND_FIELD_FLOATS,
  packWindField,
  sampleWorldWind,
  sampleWorldWindDisplacement,
  validateWindField,
  type WindFieldSettings,
} from "../packages/renderer/shared-wind.ts";

const wind = (change: Partial<WindFieldSettings> = {}): WindFieldSettings => ({
  ...DEFAULT_WIND_FIELD,
  direction: [0.8, 0.6],
  ...change,
});

test("wind rejects malformed schemas, vectors, unbounded gusts, and extra fields", () => {
  assert.equal(validateWindField(wind()).schema, "aethyrforge.wind.v1");
  for (const change of [
    { schema: "other" },
    { enabled: 1 },
    { direction: [0, 0] },
    { direction: [1, 1] },
    { direction: [NaN, 1] },
    { direction: [0, 1, 0] },
    { speed: -1 },
    { speed: 31 },
    { speed: Infinity },
    { gustStrength: 2 },
    { gustScale: 0 },
    { gustScale: 257 },
    { gustSpeed: 4.1 },
    { seed: -1 },
    { seed: 65536 },
    { seed: 1.5 },
    { extra: 1 },
  ])
    assert.throws(() =>
      validateWindField({ ...wind(), ...change } as WindFieldSettings),
    );
  assert.throws(() => validateWindField(null as unknown as WindFieldSettings));
});

test("wind occupies three vec4 values and leaves other uniform bytes alone", () => {
  const storage = new Float32Array(20).fill(99);
  assert.equal(packWindField(wind(), storage, 4), storage);
  assert.equal(WIND_FIELD_FLOATS, 12);
  assert.deepEqual([...storage.subarray(0, 4)], [99, 99, 99, 99]);
  assert.deepEqual([...storage.subarray(16)], [99, 99, 99, 99]);
  assert.deepEqual(
    [...storage.subarray(6, 16)],
    [4, 1, Math.fround(0.6), 24, Math.fround(0.7), 7, 1, 0, 0, 0],
  );
  assert.equal(packWindField(wind({ enabled: false }))[3], 0);
  assert.equal(packWindField(wind({ enabled: false }))[8], 1);
  assert.throws(() => packWindField(wind(), new Float32Array(11)));
  assert.throws(() => packWindField(wind(), storage, 0.5));
});

test("wind is bounded, coherent, seeded, directional, and independent of frame history", () => {
  const settings = wind(),
    position = [5, 2, -7];
  const first = [...(sampleWorldWind(position, 12, settings) as number[])];
  sampleWorldWind(position, 99, settings);
  assert.deepEqual(sampleWorldWind(position, 12, settings), first);
  assert.notDeepEqual(sampleWorldWind(position, 13, settings), first);
  assert.notDeepEqual(sampleWorldWind(position, 12, wind({ seed: 8 })), first);
  const near = sampleWorldWind([5.0001, 2, -7], 12, settings);
  assert.ok(Math.abs(near[0] - first[0]) < 0.001);
  const output = new Float64Array(3);
  assert.equal(sampleWorldWind(position, 12, settings, output), output);
  for (let time = 0; time < 100; time += 0.5) {
    sampleWorldWind(position, time, settings, output);
    assert.ok(
      Math.hypot(...output) <= settings.speed * (1 + settings.gustStrength),
    );
    assert.ok(
      Math.hypot(...output) >= settings.speed * (1 - settings.gustStrength),
    );
    assert.equal(output[1], 0);
    assert.ok(Math.abs(output[0] * 0.6 - output[2] * 0.8) < 1e-12);
  }
  assert.deepEqual(
    sampleWorldWind(position, 12, wind({ enabled: false })),
    [0, 0, 0],
  );
  assert.deepEqual(
    sampleWorldWind(position, 12, wind({ speed: 0 })),
    [0, 0, 0],
  );
  assert.deepEqual(
    sampleWorldWind(position, 12, wind({ gustStrength: 0 })),
    [3.2, 0, 2.4],
  );
});

test("analytic wind displacement matches numerical integration and supports warmup births", () => {
  const settings = wind(),
    position = [3, 4, 5],
    start = -2,
    end = 7;
  const integrated = sampleWorldWindDisplacement(
    position,
    start,
    end,
    settings,
  );
  const steps = 10000,
    dt = (end - start) / steps;
  let numericalX = 0;
  const output = new Float64Array(3);
  for (let i = 0; i < steps; i++) {
    sampleWorldWind(position, start + (i + 0.5) * dt, settings, output);
    numericalX += output[0] * dt;
  }
  assert.ok(Math.abs(integrated[0] - numericalX) < 1e-6);
  const early = sampleWorldWindDisplacement(position, start, 1, settings);
  const late = sampleWorldWindDisplacement(position, 1, end, settings);
  assert.ok(Math.abs(early[0] + late[0] - integrated[0]) < 1e-12);
  assert.deepEqual(
    sampleWorldWindDisplacement(position, 4, 4, settings),
    [0, 0, 0],
  );
  const steady = wind({ gustSpeed: 0 });
  assert.ok(
    Math.abs(
      sampleWorldWindDisplacement(position, start, end, steady)[0] -
        sampleWorldWind(position, 0, steady)[0] * (end - start),
    ) < 1e-12,
  );
  assert.throws(() => sampleWorldWindDisplacement(position, 5, 4, settings));
  assert.throws(() => sampleWorldWind([NaN, 0, 0], 0, settings));
  assert.throws(() => sampleWorldWind([0, 0, 0], Infinity, settings));
});
