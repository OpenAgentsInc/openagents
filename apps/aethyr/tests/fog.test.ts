import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FOG_HEIGHT_RATE,
  FOG_MAX_OPACITY,
  FOG_START_DISTANCE,
  SOURCE_DIRECTIONAL_INSCATTERING_LINEAR,
  SOURCE_DIRECTIONAL_INSCATTERING_START_DISTANCE,
  SOURCE_FOG_DENSITY_AT_REFERENCE,
  SOURCE_FOG_INSCATTERING_LINEAR,
  SOURCE_VOLUME_ALBEDO_LINEAR,
  SOURCE_VOLUME_DISTANCE,
  SOURCE_VOLUME_EXTINCTION_AT_SOURCE_HEIGHT,
  sourceFogExtinctionAtHeight,
  segmentFogOpticalDepth,
  heightFogComposition,
  FOG_OPTICAL_DEPTH_LIMIT,
} from "../packages/renderer/fog.ts";

function close(actual: number, expected: number, tolerance = 1e-12): void {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} differs from ${expected}`,
  );
}

test("source factors convert to inverse-meter extinction at the existing editor height", () => {
  close(FOG_HEIGHT_RATE, 0.027725887222397813);
  close(SOURCE_FOG_DENSITY_AT_REFERENCE, 0.0022134909024198045);
  close(sourceFogExtinctionAtHeight(-60), 0.012011325347955036);
  close(sourceFogExtinctionAtHeight(0), 0.002275720601955215);
  close(sourceFogExtinctionAtHeight(1.6), 0.002176972893368109);
  close(Math.exp(-sourceFogExtinctionAtHeight(1.6) * 100), 0.8043688958585247);
  assert.ok(sourceFogExtinctionAtHeight(20) < sourceFogExtinctionAtHeight(0));
  for (const invalid of [Number.NaN, Infinity, -Infinity, -20_001, 20_001])
    assert.throws(() => sourceFogExtinctionAtHeight(invalid));
});

test("source analytical transmittance keeps authored color, range and volume distinctions", () => {
  assert.equal(FOG_START_DISTANCE, 0);
  assert.equal(FOG_MAX_OPACITY, 1);
  assert.deepEqual(SOURCE_FOG_INSCATTERING_LINEAR, [0.609237, 0.937705, 1.5]);
  assert.deepEqual(SOURCE_DIRECTIONAL_INSCATTERING_LINEAR, [0, 0, 0]);
  assert.equal(SOURCE_DIRECTIONAL_INSCATTERING_START_DISTANCE, 100);
  assert.equal(SOURCE_VOLUME_DISTANCE, 60);
  close(SOURCE_VOLUME_EXTINCTION_AT_SOURCE_HEIGHT, 0.0125);
  close(SOURCE_VOLUME_ALBEDO_LINEAR, 0.8069522576692516);
  assert.ok(SOURCE_VOLUME_ALBEDO_LINEAR < 232 / 255);
});

test("segment optical depth matches constant density, reversed rays and independent quadrature", () => {
  close(segmentFogOpticalDepth(7, 0.6, 2, 14, 0.1, 0, 0), 1.2);
  const rate = 0.04,
    rho = 0.003,
    start = 3,
    end = 170;
  for (const direction of [-0.9, -0.001, 0, 0.001, 0.9]) {
    const count = 50_000,
      step = (end - start) / count;
    let numerical = 0;
    for (let i = 0; i < count; i++)
      numerical +=
        rho *
        Math.exp(-rate * (2 + direction * (start + (i + 0.5) * step) - 1)) *
        step;
    close(
      segmentFogOpticalDepth(2, direction, start, end, rho, rate, 1),
      numerical,
      3e-7,
    );
    close(
      segmentFogOpticalDepth(2, direction, start, end, rho, rate, 1),
      segmentFogOpticalDepth(
        2 + direction * (start + end),
        -direction,
        start,
        end,
        rho,
        rate,
        1,
      ),
      1e-10,
    );
  }
  const whole = segmentFogOpticalDepth(2, 0.2, 0, 1000, rho, rate, 1);
  close(
    whole,
    segmentFogOpticalDepth(2, 0.2, 0, 60, rho, rate, 1) +
      segmentFogOpticalDepth(2, 0.2, 60, 1000, rho, rate, 1),
  );
});

test("long sky rays keep the exponential-height limit rather than clamping height change", () => {
  close(
    segmentFogOpticalDepth(1, 1, 0, 100_000, 0.002, FOG_HEIGHT_RATE, 1),
    0.002 / FOG_HEIGHT_RATE,
  );
  assert.equal(
    segmentFogOpticalDepth(1, -1, 0, 100_000, 0.002, FOG_HEIGHT_RATE, 1),
    FOG_OPTICAL_DEPTH_LIMIT,
  );
  for (const ray of [-1, -0.5, 0, 0.5, 1]) {
    const tau = segmentFogOpticalDepth(
      2,
      ray,
      0,
      13_540,
      0.002,
      FOG_HEIGHT_RATE,
      1,
    );
    assert.ok(Number.isFinite(tau) && tau >= 0 && tau <= 80);
    assert.equal(
      segmentFogOpticalDepth(2, ray, 0, 13_540, 0, FOG_HEIGHT_RATE, 1),
      0,
    );
    assert.equal(
      segmentFogOpticalDepth(2, ray, 13_540, 13_540, 0.002, FOG_HEIGHT_RATE, 1),
      0,
    );
  }
  close(
    segmentFogOpticalDepth(1, 1e-12, 0, 100, 0.002, FOG_HEIGHT_RATE, 1),
    0.2,
    1e-10,
  );
  for (const args of [
    [0, 2, 0, 10, 0.1, 0.1, 0],
    [0, 0, 11, 10, 0.1, 0.1, 0],
    [0, 0, 0, Infinity, 0.1, 0.1, 0],
    [0, 0, 0, 10, -1, 0.1, 0],
    [0, 0, 0, 10, 0.1, -1, 0],
    [NaN, 0, 0, 10, 0.1, 0.1, 0],
  ])
    assert.throws(() =>
      segmentFogOpticalDepth(
        ...(args as [number, number, number, number, number, number, number]),
      ),
    );
});

test("volume replaces near analytic fill and extinction, with continuous source range boundaries", () => {
  const evaluate = (distance: number, enabled = 1) =>
    heightFogComposition(
      1,
      0,
      distance,
      0.01,
      0,
      1,
      0,
      [2, 0, 1, 0],
      [60, enabled],
    );
  assert.deepEqual(evaluate(0), [1, 0]);
  for (const distance of [1, 25, 60]) {
    const [t, fill] = evaluate(distance);
    close(t, Math.exp(-0.02 * distance));
    assert.equal(fill, 0);
  }
  const [t, fill] = evaluate(100);
  close(t, Math.exp(-0.02 * 60 - 0.01 * 40));
  close(fill, Math.exp(-0.02 * 60) * (1 - Math.exp(-0.01 * 40)));
  close(evaluate(60 - 1e-7)[0], evaluate(60 + 1e-7)[0], 1e-9);
  close(evaluate(60 - 1e-7)[1], evaluate(60 + 1e-7)[1], 1e-9);
  close(evaluate(100, 0)[0], Math.exp(-1));
  close(evaluate(100, 0)[1], 1 - Math.exp(-1));
  const sourceRatio = 0.5 / Math.LN2 ** 2;
  const source = heightFogComposition(
    1,
    0,
    30,
    SOURCE_FOG_DENSITY_AT_REFERENCE,
    FOG_HEIGHT_RATE,
    1,
    0,
    [sourceRatio, FOG_HEIGHT_RATE, 1, 0],
    [60, 1],
  );
  close(
    source[0],
    Math.exp(-SOURCE_FOG_DENSITY_AT_REFERENCE * sourceRatio * 30),
  );
  assert.equal(source[1], 0);
});

test("nonzero volume start composes analytic pre/far fill and attenuates volume light once", () => {
  const [t, fill] = heightFogComposition(
    0,
    0,
    100,
    0.01,
    0,
    0,
    0,
    [2, 0, 0, 10],
    [60, 1],
  );
  const pre = Math.exp(-0.1),
    volume = Math.exp(-1),
    far = Math.exp(-0.4);
  close(t, pre * volume * far);
  close(fill, 1 - pre + pre * volume * (1 - far));
  const surface = 3,
    fogColor = 0.7,
    volumeSource = 0.4;
  close(
    surface * t + fogColor * fill + pre * volumeSource,
    fogColor * (1 - pre) +
      pre * (volumeSource + volume * (fogColor * (1 - far) + far * surface)),
  );
  // A delayed analytic start must not manufacture fill before that start.
  const delayed = heightFogComposition(
    0,
    0,
    100,
    0.01,
    0,
    0,
    80,
    [2, 0, 0, 10],
    [60, 1],
  );
  close(delayed[0], Math.exp(-1.2));
  close(delayed[1], volume * (1 - Math.exp(-0.2)));
  const empty = heightFogComposition(
    0,
    0,
    100,
    0,
    0,
    0,
    0,
    [2, 0, 0, 10],
    [60, 1],
  );
  assert.deepEqual(empty, [1, 0]);
  assert.throws(() =>
    heightFogComposition(0, 0, 100, 0.01, 0, 0, 0, [2, 0, 0, 70], [60, 1]),
  );
  assert.throws(() =>
    heightFogComposition(0, 0, 100, 0.01, 0, 0, 0, [2, 0, 0, 0], [60, 0.5]),
  );
});
