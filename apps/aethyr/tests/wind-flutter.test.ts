import assert from "node:assert/strict";
import test from "node:test";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
import {
  WIND_FLUTTER_CLOCK_SCALE,
  WIND_FLUTTER_PERIOD,
  boundedFlutterTime,
  validateWindFlutter,
  windFlutterBound,
  windFlutterOffset,
  type WindFlutter,
} from "../packages/renderer/wind-flutter.ts";

const settings: WindFlutter = {
  profile: "source-triangle-flutter-v1",
  intensity: 0.15,
  weight: 0.3,
  clockScale: WIND_FLUTTER_CLOCK_SCALE,
  smallWaveCm: 1,
  largeWaveCm: 200,
  vertexLod: 0,
};
test("pinned vertices remain fixed and linear color uses source axes", () => {
  assert.deepEqual(
    windFlutterOffset([1, 2, 3], 0, 3, [1, 1, 1], settings),
    [0, 0, 0],
  );
  assert.deepEqual(
    windFlutterOffset([1, 2, 3], 1, 3, [0.2, 0.3, 0.4], {
      ...settings,
      intensity: 0,
    }),
    [0.002, 0.004, 0.003],
  );
  const full = windFlutterOffset([1, 2, 3], 1, 3, [0.2, 0.3, 0.4], settings);
  assert.deepEqual(
    windFlutterOffset([1, 2, 3], 0.5, 3, [0.2, 0.3, 0.4], settings),
    full.map((v) => v * 0.5),
  );
});

test("bounded flutter phase preserves periodicity and long world ages", () => {
  const a = windFlutterOffset(
    [0.31, -1.7, 2.4],
    1,
    3,
    [0.1, 0.2, 0.3],
    settings,
  );
  const b = windFlutterOffset(
    [0.31, -1.7, 2.4],
    1,
    3 + WIND_FLUTTER_PERIOD * 100,
    [0.1, 0.2, 0.3],
    settings,
  );
  a.forEach((v, i) => assert.ok(Math.abs(v - b[i]) < 1e-12));
  const phase = boundedFlutterTime(MAX_WORLD_ANIMATION_TIME);
  assert.ok(phase >= 0 && phase < WIND_FLUTTER_PERIOD);
  assert.deepEqual(
    windFlutterOffset(
      [0.31, -1.7, 2.4],
      1,
      MAX_WORLD_ANIMATION_TIME,
      [0.1, 0.2, 0.3],
      settings,
    ),
    windFlutterOffset([0.31, -1.7, 2.4], 1, phase, [0.1, 0.2, 0.3], settings),
  );
});

test("indexed-independent flutter bound covers colors, phases and source positions", () => {
  for (let i = 0; i < 512; i++) {
    const green = (i % 17) / 16,
      value = windFlutterOffset(
        [Math.sin(i) * 70, Math.cos(i * 0.7) * 20, Math.cos(i) * 100],
        green,
        i * 179.7,
        [(i % 7) / 6, (i % 11) / 10, (i % 5) / 4],
        settings,
      );
    assert.ok(
      Math.hypot(...value) <= windFlutterBound(green, settings) + 1e-12,
    );
  }
  assert.ok(windFlutterBound(1, settings) < 0.027);
});

test("unsupported clocks, waves, samples and inputs fail before evaluation", () => {
  for (const patch of [
    { clockScale: 0.3 },
    { intensity: Infinity },
    { weight: -1 },
    { smallWaveCm: 2 },
    { vertexLod: 1 },
  ])
    assert.throws(() =>
      validateWindFlutter({ ...settings, ...patch } as WindFlutter),
    );
  assert.throws(() => boundedFlutterTime(-1));
  assert.throws(() => boundedFlutterTime(MAX_WORLD_ANIMATION_TIME + 1));
  assert.throws(() =>
    windFlutterOffset([1, 2, NaN], 1, 0, [0, 0, 0], settings),
  );
  assert.throws(() => windFlutterOffset([1, 2, 3], 1, 0, [0, 0, 2], settings));
});
