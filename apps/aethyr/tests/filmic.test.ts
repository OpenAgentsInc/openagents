import assert from "node:assert/strict";
import { test } from "node:test";
import type { SceneLook } from "../packages/assets/src/scene.ts";
import {
  DEFAULT_FILMIC_CONTROLS,
  FILMIC_FLOAT_COUNT,
  FILMIC_GRAY,
  FILMIC_OFFSETS,
  FILMIC_PROFILE,
  evaluateFilmic,
  evaluateFilmicColor,
  gradeFilmicShadow,
  prepareFilmic,
  sourceFilmicControls,
  type FilmicControls,
} from "../packages/renderer/filmic.ts";

const near = (actual: number, expected: number, tolerance = 2e-6) =>
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} != ${expected}`,
  );
const profile = (changes: Partial<FilmicControls> = {}) =>
  prepareFilmic({ ...DEFAULT_FILMIC_CONTROLS, ...changes });
const ramp = Array.from({ length: 1201 }, (_, i) => 10 ** (-30 + i / 20));
const extremeControls: Partial<FilmicControls>[] = [
  {},
  { whiteClip: 1 },
  { slope: 0, toe: 0, shoulder: 0, blackClip: 0, whiteClip: 0 },
  { slope: 1, toe: 1, shoulder: 1, blackClip: 1, whiteClip: 1 },
  { slope: 0, toe: 1, shoulder: 0, blackClip: 1, whiteClip: 1 },
  { slope: 1, toe: 0, shoulder: 1, blackClip: 0, whiteClip: 0 },
  { toe: 1, shoulder: 0 },
  { toe: 0, shoulder: 0 },
  { toe: 1, shoulder: 1 },
  { toe: 0.9, shoulder: 0.4, blackClip: 0.2 },
];

test("film profile is finite, monotone and gray-anchored across the whole control cube", () => {
  // Every corner checks degenerate slope and clipped endpoints, not just defaults.
  const keys = ["slope", "toe", "shoulder", "blackClip", "whiteClip"] as const;
  for (let bits = 0; bits < 32; bits++)
    extremeControls.push(
      Object.fromEntries(keys.map((k, i) => [k, (bits >> i) & 1])),
    );
  for (const controls of extremeControls) {
    const prepared = profile(controls);
    assert.equal(prepared.profile, FILMIC_PROFILE);
    assert.ok([...prepared.packed].every(Number.isFinite));
    near(evaluateFilmic(FILMIC_GRAY, prepared), FILMIC_GRAY, 2e-6);
    assert.equal(evaluateFilmic(0, prepared), 0);
    assert.equal(evaluateFilmic(-1, prepared), 0);
    let last = 0;
    for (const value of ramp) {
      const mapped = evaluateFilmic(value, prepared);
      assert.ok(Number.isFinite(mapped) && mapped >= 0 && mapped <= 1);
      // Rounded f32 coefficients may differ by a few ulps at a join.
      assert.ok(
        mapped >= last - 3e-7,
        `${JSON.stringify(controls)}: ${last} -> ${mapped}`,
      );
      last = mapped;
    }
  }
});

test("ordered and repaired log joins match values and first derivatives", () => {
  for (const controls of [{}, { toe: 0.9, shoulder: 0.65 }, { whiteClip: 1 }]) {
    const prepared = profile(controls);
    const p = prepared.packed;
    assert.ok(p[0] <= p[1]);
    for (const x of new Set([p[0], p[1]])) {
      const h = 1e-4;
      const center = evaluateFilmic(10 ** x, prepared);
      const left = evaluateFilmic(10 ** (x - h), prepared);
      const right = evaluateFilmic(10 ** (x + h), prepared);
      near((center - left) / h, p[3], 0.002);
      near((right - center) / h, p[3], 0.002);
    }
  }
  const repaired = profile().packed;
  near(repaired[0], repaired[1]);
  near(repaired[2], (1 - 0.55 + 0.26) / 2);
});

test("white clipping changes the highlight rolloff while preserving middle gray", () => {
  const defaults = profile(),
    authored = profile({ whiteClip: 1 });
  near(evaluateFilmic(0.18, defaults), evaluateFilmic(0.18, authored));
  assert.ok(evaluateFilmic(1, authored) - evaluateFilmic(1, defaults) > 0.04);
  assert.equal(evaluateFilmic(1e20, authored), 1);
  assert.ok(evaluateFilmic(1e20, defaults) > 0.999999);
});

test("neutral colors stay neutral and saturated highlights converge to white", () => {
  for (const controls of [{}, { whiteClip: 1 }, { blackClip: 0.2 }]) {
    const prepared = profile(controls);
    for (const value of ramp) {
      const output = evaluateFilmicColor([value, value, value], prepared);
      output.forEach((v) => near(v, evaluateFilmic(value, prepared), 2e-6));
    }
    for (const rgb of [
      [1e20, 0, 0],
      [0, 1e20, 0],
      [0, 0, 1e20],
      [10, 2, 0.1],
    ]) {
      const output = evaluateFilmicColor(rgb, prepared);
      assert.ok(output.every((v) => Number.isFinite(v) && v >= 0 && v <= 1));
      if (Math.max(...rgb) > 1e10) output.forEach((v) => near(v, 1));
    }
  }
});

test("owned shadow grade is monotone, lifts source .85 shadows and joins identity at gray", () => {
  for (const exponent of [0, 0.1, 0.85, 1, 2, 4]) {
    let last = 0;
    for (const value of ramp) {
      const mapped = gradeFilmicShadow(value, exponent);
      assert.ok(Number.isFinite(mapped) && mapped >= last);
      if (value >= FILMIC_GRAY) assert.equal(mapped, value);
      last = mapped;
    }
    const h = 1e-7;
    near(
      (FILMIC_GRAY - gradeFilmicShadow(FILMIC_GRAY - h, exponent)) / h,
      1,
      1e-5,
    );
  }
  assert.ok(gradeFilmicShadow(0.01, 0.85) > 0.01);
  near(gradeFilmicShadow(0.01, 1), 0.01, 1e-12);
  const day = profile({ whiteClip: 1, shadowContrast: [0.85, 0.85, 0.85] });
  const color = evaluateFilmicColor([0.01, 0.01, 0.01], day);
  near(color[0], color[1]);
  near(color[1], color[2]);
  near(evaluateFilmicColor([0.18, 0.18, 0.18], day)[0], 0.18);
});

const volume = (settings: string, extra: Record<string, unknown> = {}) => ({
  id: "source-volume",
  class: "PostProcessVolume",
  properties: {
    enabled: true,
    unbound: true,
    blend_weight: 1,
    priority: 0,
    settings,
    ...extra,
  },
});
const look = (actors: Record<string, unknown>[]) =>
  ({
    scenarios: { day: { environment: actors }, night: { environment: [] } },
  }) as unknown as SceneLook;

test("only active source flags apply, with source white clip and day shadow contrast", () => {
  const source = volume(
    "(bOverride_FilmSlope=False,FilmSlope=0.2,bOverride_FilmWhiteClip=True,FilmWhiteClip=1,bOverride_ColorContrastShadows=True,ColorContrastShadows=(X=0.85,Y=0.85,Z=0.85,W=1))",
  );
  const admitted = sourceFilmicControls(look([source]), "day");
  assert.equal(admitted.controls.slope, DEFAULT_FILMIC_CONTROLS.slope);
  assert.equal(admitted.controls.whiteClip, 1);
  assert.deepEqual(admitted.controls.shadowContrast, [0.85, 0.85, 0.85]);
  assert.equal(admitted.appliedOverrides.length, 2);
  assert.deepEqual(admitted.diagnostics, []);
  for (const extra of [
    { enabled: false },
    { unbound: false },
    { blend_weight: 0 },
  ])
    assert.deepEqual(
      sourceFilmicControls(
        look([volume(source.properties.settings, extra)]),
        "day",
      ).controls,
      DEFAULT_FILMIC_CONTROLS,
    );
  const camera = { ...source, class: "CameraActor" };
  assert.deepEqual(
    sourceFilmicControls(look([camera]), "day").controls,
    DEFAULT_FILMIC_CONTROLS,
  );
  assert.deepEqual(
    sourceFilmicControls(undefined, "day").controls,
    DEFAULT_FILMIC_CONTROLS,
  );
  assert.deepEqual(
    sourceFilmicControls(look([source]), "night").controls,
    DEFAULT_FILMIC_CONTROLS,
  );
});

test("known volume parameters blend by weight and stable ascending priority", () => {
  const a = volume("bOverride_FilmWhiteClip=True,FilmWhiteClip=1", {
    priority: 5,
    blend_weight: 0.5,
  });
  const b = volume("bOverride_FilmWhiteClip=True,FilmWhiteClip=0.2", {
    priority: 0,
  });
  near(sourceFilmicControls(look([a, b]), "day").controls.whiteClip, 0.6);
  const shadow = volume(
    "bOverride_ColorContrastShadows=True,ColorContrastShadows=(X=0.5,Y=0.75,Z=1,W=2)",
    { blend_weight: 0.5 },
  );
  assert.deepEqual(
    sourceFilmicControls(look([shadow]), "day").controls.shadowContrast,
    [1, 1.25, 1.5],
  );
});

test("invalid active values leave inherited parameters and report diagnostics", () => {
  const invalid = volume(
    "bOverride_FilmWhiteClip=True,FilmWhiteClip=nan,bOverride_FilmToe=True,FilmToe=2,bOverride_ColorContrastShadows=True,ColorContrastShadows=(X=1,Y=1,Z=1)",
  );
  const result = sourceFilmicControls(look([invalid]), "day");
  assert.deepEqual(result.controls, DEFAULT_FILMIC_CONTROLS);
  assert.equal(result.diagnostics.length, 3);
  assert.equal(result.appliedOverrides.length, 0);
  for (const extra of [
    { blend_weight: 2 },
    { blend_weight: NaN },
    { priority: Infinity },
  ]) {
    const parsed = sourceFilmicControls(
      look([volume("bOverride_FilmWhiteClip=True,FilmWhiteClip=1", extra)]),
      "day",
    );
    assert.deepEqual(parsed.controls, DEFAULT_FILMIC_CONTROLS);
    assert.equal(parsed.diagnostics.length, 1);
  }
  assert.equal(
    sourceFilmicControls(
      look([volume("NotbOverride_FilmWhiteClip=True,FilmWhiteClip=1")]),
      "day",
    ).controls.whiteClip,
    0.04,
  );
});

test("packing snapshots input, aligns five vec4s, and rejects unsupported inputs", () => {
  const shadowContrast: [number, number, number] = [0.85, 1, 1.2];
  const input: FilmicControls = {
    ...DEFAULT_FILMIC_CONTROLS,
    shadowContrast,
  };
  const prepared = prepareFilmic(input);
  input.whiteClip = 1;
  shadowContrast[0] = 2;
  assert.equal(prepared.controls.whiteClip, 0.04);
  assert.equal(prepared.controls.shadowContrast[0], 0.85);
  assert.equal(prepared.packed.length, FILMIC_FLOAT_COUNT);
  assert.deepEqual(Object.values(FILMIC_OFFSETS), [0, 4, 8, 12, 16]);
  assert.equal(prepared.packed[19], 1);
  for (const changes of [
    { slope: -1 },
    { toe: 2 },
    { whiteClip: Infinity },
    { blackClip: NaN },
    { shadowContrast: [1, -1, 1] as const },
  ])
    assert.throws(() => profile(changes));
  assert.throws(() => evaluateFilmic(NaN, prepared));
  assert.throws(() => evaluateFilmicColor([1, Infinity, 0], prepared));
  assert.throws(() => evaluateFilmicColor([1, 0], prepared));
});
