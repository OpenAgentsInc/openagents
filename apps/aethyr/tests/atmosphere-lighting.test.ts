import assert from "node:assert/strict";
import { test } from "node:test";
import {
  atmosphereOpticalDepth,
  miePhase,
  rayleighPhase,
  renderAtmospherePanorama,
  singleScatteredRadiance,
  type AtmosphereProfile,
  type AtmosphereSun,
  type AtmosphereVector,
} from "../packages/renderer/atmosphere-lighting.ts";
const profile = (): AtmosphereProfile => ({
  planetRadiusMeters: 6_000_000,
  atmosphereHeightMeters: 1000,
  rayleighScatteringPerMeter: [0.0001, 0.0002, 0.0003],
  rayleighScaleHeightMeters: 8000,
  mieScatteringPerMeter: [0.000004, 0.000004, 0.000004],
  mieAbsorptionPerMeter: [0.000001, 0.000001, 0.000001],
  mieScaleHeightMeters: 1500,
  mieAnisotropy: 0.7,
  absorptionPerMeter: [0.00001, 0.00003, 0.000002],
  absorptionTent: {
    peakAltitudeMeters: 500,
    halfWidthMeters: 500,
    peakDensity: 1,
  },
  skyLuminanceFactor: [1, 1, 1],
});
const sun: AtmosphereSun = {
  direction: [0, 0.8, 0.6],
  illuminanceLux: 25,
  color: [1, 0.9, 0.8],
};
const options = { viewSamples: 64, sunSamples: 64, observerAltitudeMeters: 2 };
const close = (a: number, b: number, relative = 1e-5) =>
  assert.ok(
    Math.abs(a - b) <= relative * Math.max(1e-12, Math.abs(b)),
    `${a} != ${b}`,
  );

test("empty scattering medium emits zero, without a solar disk or ground emission", () => {
  const p = profile();
  p.rayleighScatteringPerMeter = p.mieScatteringPerMeter = [0, 0, 0];
  for (const direction of [
    [0, 1, 0],
    sun.direction,
    [0, -1, 0],
  ] as AtmosphereVector[])
    assert.deepEqual(
      singleScatteredRadiance(p, sun, direction, options),
      [0, 0, 0],
    );
});

test("optical depth uses inverse meters and matches the analytic vertical exponential integral", () => {
  const p = profile();
  p.mieScatteringPerMeter =
    p.mieAbsorptionPerMeter =
    p.absorptionPerMeter =
      [0, 0, 0];
  const origin: AtmosphereVector = [0, p.planetRadiusMeters, 0];
  const a = atmosphereOpticalDepth(p, origin, [0, 1, 0], 400, 128);
  const b = atmosphereOpticalDepth(p, origin, [0, 1, 0], 800, 128);
  const expected =
    p.rayleighScaleHeightMeters *
    (1 - Math.exp(-800 / p.rayleighScaleHeightMeters));
  b.forEach((value, c) => {
    assert.ok(value > a[c]);
    close(value, p.rayleighScatteringPerMeter[c] * expected, 1e-7);
  });
  assert.ok(
    b[0] > 0.07 && b[0] < 0.09,
    "A kilometer/meter error changes optical depth by1000.",
  );
});

test("Rayleigh and Cornette–Shanks phase functions integrate to one over solid angle", () => {
  const n = 20000;
  for (const phase of [rayleighPhase, (mu: number) => miePhase(mu, 0.8)]) {
    let total = 0;
    for (let i = 0; i < n; i++)
      total += (phase(-1 + (2 * (i + 0.5)) / n) * 4 * Math.PI) / n;
    close(total, 1, 2e-6);
  }
});

test("sun illuminance and RGB color scale radiance linearly before float16 clipping", () => {
  const p = profile(),
    direction: AtmosphereVector = [0.2, 0.9, 0.3];
  const a = singleScatteredRadiance(p, sun, direction, options);
  const b = singleScatteredRadiance(
    p,
    { ...sun, illuminanceLux: 50 },
    direction,
    options,
  );
  const c = singleScatteredRadiance(
    p,
    { ...sun, color: [0.5, 0.45, 0.4] },
    direction,
    options,
  );
  a.forEach((value, channel) => {
    assert.ok(value > 0);
    close(b[channel], 2 * value, 1e-12);
    close(c[channel], 0.5 * value, 1e-12);
  });
});

test("rotating the sun and view together around world up preserves atmospheric radiance", () => {
  const rotate = ([x, y, z]: AtmosphereVector): AtmosphereVector => [z, y, -x];
  const direction: AtmosphereVector = [0.3, 0.4, 0.8],
    p = profile();
  const a = singleScatteredRadiance(p, sun, direction, options);
  const b = singleScatteredRadiance(
    p,
    { ...sun, direction: rotate(sun.direction) },
    rotate(direction),
    options,
  );
  a.forEach((x, c) => close(x, b[c], 1e-10));
});

test("planet blocks a below-horizon sun; horizon and twilight remain finite", () => {
  const p = profile();
  assert.deepEqual(
    singleScatteredRadiance(
      p,
      { ...sun, direction: [0, -1, 0] },
      [0, 1, 0],
      options,
    ),
    [0, 0, 0],
  );
  assert.deepEqual(
    singleScatteredRadiance(p, sun, [0, -0.001, 1], options),
    [0, 0, 0],
  );
  for (const sy of [-0.01, 0, 0.01])
    for (const vy of [0, 1e-8, 0.001, 1]) {
      const value = singleScatteredRadiance(
        p,
        { ...sun, direction: [0, sy, 1] },
        [0, vy, 1],
        options,
      );
      assert.ok(value.every((x) => Number.isFinite(x) && x >= 0 && x <= 65504));
    }
});

test("view/sun quadrature converges against a higher-sample CPU reference", () => {
  const p = profile();
  for (const direction of [
    [0.2, 1, 0.3],
    [0.4, 0.02, 0.8],
  ] as AtmosphereVector[]) {
    const a = singleScatteredRadiance(p, sun, direction, options);
    const b = singleScatteredRadiance(p, sun, direction, {
      ...options,
      viewSamples: 256,
      sunSamples: 256,
    });
    a.forEach((x, c) => close(x, b[c], 0.012));
  }
});

test("panorama has canonical world directions, a black lower hemisphere, and bounded work", () => {
  const image = renderAtmospherePanorama(profile(), sun, 32, 16, {
    viewSamples: 16,
    sunSamples: 16,
  });
  assert.equal(image.pixels.length, 32 * 16 * 4);
  for (let y = 8; y < 16; y++)
    for (let x = 0; x < 32; x++)
      assert.deepEqual(
        [...image.pixels.slice((y * 32 + x) * 4, (y * 32 + x) * 4 + 4)],
        [0, 0, 0, 1],
      );
  assert.ok(image.statistics.horizontalIlluminanceLux.every((x) => x > 0));
  assert.equal(image.statistics.clippedChannels, 0);
  assert.throws(
    () =>
      renderAtmospherePanorama(profile(), sun, 256, 128, {
        viewSamples: 256,
        sunSamples: 256,
      }),
    /70 million/,
  );
  assert.throws(() => renderAtmospherePanorama(profile(), sun, 128, 128));
  assert.throws(() =>
    singleScatteredRadiance(
      { ...profile(), atmosphereHeightMeters: -1 },
      sun,
      [0, 1, 0],
    ),
  );
  assert.throws(() =>
    singleScatteredRadiance(
      profile(),
      { ...sun, direction: [0, 0, 0] },
      [0, 1, 0],
    ),
  );
});
