import assert from "node:assert/strict";
import test from "node:test";
import {
  editorialFrame,
  validateFilmTimeline,
} from "../tools/film/timeline.ts";
const camera = {
  eye: [0, 2, 5] as [number, number, number],
  target: [0, 2, 0] as [number, number, number],
  up: [0, 1, 0] as [number, number, number],
  fov: 1,
};
const timeline = () => ({
  schema: "aethyrforge.film.v1",
  width: 4,
  height: 4,
  frameRate: { numerator: 60, denominator: 1 },
  frames: [
    { ptsUs: 0, worldTimeSeconds: 0, camera },
    { ptsUs: 16667, worldTimeSeconds: 0, camera, holdPrevious: true },
  ],
});
test("a held source frame keeps its exact output cadence without another world sample", () => {
  assert.equal(validateFilmTimeline(timeline()).frames.length, 2);
  const changed = timeline();
  changed.frames[1].worldTimeSeconds = 0.01;
  assert.throws(() => validateFilmTimeline(changed), /different scene sample/);
  const brightHold = timeline();
  Object.assign(brightHold.frames[1], { exposure: 4 });
  assert.throws(
    () => validateFilmTimeline(brightHold),
    /different scene sample/,
  );
  const drift = timeline();
  drift.frames[1].ptsUs = 16666;
  assert.throws(() => validateFilmTimeline(drift), /exact output timestamp/);
});
test("film admission rejects unbounded dimensions, poses, fades, and letterbox rows", () => {
  for (const patch of [
    { width: 3 },
    { height: 1082 },
    { frameRate: { numerator: 0, denominator: 1 } },
    { frames: [] },
  ])
    assert.throws(() => validateFilmTimeline({ ...timeline(), ...patch }));
  for (const patch of [
    { black: "false" },
    { holdPrevious: "false" },
    { exposure: NaN },
    { exposure: 64.01 },
    { fade: NaN },
    { fade: 1.01 },
    { letterbox: { top: 2, bottom: 2 } },
    { camera: { ...camera, up: [0, 0, 1] } },
  ])
    assert.throws(() =>
      validateFilmTimeline({
        ...timeline(),
        frames: [{ ...timeline().frames[0], ...patch }],
      }),
    );
});
test("editorial fades and black bars preserve source pixels, alpha, and interior rows", () => {
  const input = Uint8Array.from({ length: 64 }, (_, i) =>
      i % 4 === 3 ? 255 : 100,
    ),
    before = new Uint8Array(input);
  const output = editorialFrame(
    input,
    {
      ptsUs: 0,
      worldTimeSeconds: 0,
      fade: 0.5,
      letterbox: { top: 1, bottom: 1 },
    },
    4,
    4,
  );
  assert.deepEqual(input, before);
  assert.deepEqual([...output.slice(0, 4)], [0, 0, 0, 255]);
  assert.deepEqual([...output.slice(16, 20)], [50, 50, 50, 255]);
  assert.deepEqual([...output.slice(60, 64)], [0, 0, 0, 255]);
  assert.throws(
    () =>
      editorialFrame(
        input.subarray(1),
        { ptsUs: 0, worldTimeSeconds: 0 },
        4,
        4,
      ),
    /byte count/,
  );
});
