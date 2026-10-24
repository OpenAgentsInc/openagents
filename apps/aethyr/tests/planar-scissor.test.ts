import assert from "node:assert/strict";
import test from "node:test";
import { planarReceiverScissor as scissor } from "../packages/renderer/planar-scissor.ts";
import {
  multiply,
  lookAt,
  perspective,
  modelMatrix,
  type V3,
} from "../packages/renderer/math.ts";

const identity = modelMatrix([0, 0, 0]);
const box = [-0.5, -0.02, 0.2, 0.5, 0.02, 0.6];
const mirror = modelMatrix([0, 0, 0]);
mirror[5] = -1;
const rect = (...args: Parameters<typeof scissor>) => {
  const value = scissor(...args);
  return value ? [...value] : undefined;
};
function projected(m: ArrayLike<number>, p: number[]) {
  const q = Array.from({ length: 4 }, (_, i) =>
    Math.fround(
      Math.fround(
        Math.fround(Math.fround(m[i] * p[0]) + Math.fround(m[4 + i] * p[1])) +
          Math.fround(m[8 + i] * p[2]),
      ) + m[12 + i],
    ),
  );
  return q;
}
function cover(
  view: ArrayLike<number>,
  reflected: ArrayLike<number>,
  bounds: number[],
  heights: number[],
  planeY = 0,
  distortion = 0.035,
) {
  const found = rect(
    view,
    reflected,
    bounds,
    heights,
    planeY,
    400,
    250,
    distortion,
    0.03,
  );
  assert(found, "The owned fixture must permit a bounded scissor.");
  const [x, y, width, height] = found;
  let samples = 0;
  for (let cell = 0; cell < heights.length; cell++) {
    if (Math.abs(heights[cell] - planeY) > 0.03) continue;
    const b = bounds.slice(cell * 6, cell * 6 + 6);
    for (let ix = 0; ix <= 12; ix++)
      for (let iz = 0; iz <= 12; iz++) {
        const p = [
          b[0] + ((b[3] - b[0]) * ix) / 12,
          heights[cell],
          b[2] + ((b[5] - b[2]) * iz) / 12,
        ];
        const main = projected(view, p);
        if (
          main[3] <= 0 ||
          main[2] < 0 ||
          main[2] > main[3] ||
          Math.abs(main[0]) > main[3] ||
          Math.abs(main[1]) > main[3]
        )
          continue;
        const q = projected(reflected, p);
        if (q[3] <= 0) continue;
        for (let angle = 0; angle < 24; angle++) {
          const u =
              (q[0] / q[3]) * 0.5 +
              0.5 +
              Math.cos((angle / 24) * Math.PI * 2) * distortion,
            v =
              (q[1] / q[3]) * -0.5 +
              0.5 +
              Math.sin((angle / 24) * Math.PI * 2) * distortion;
          if (u <= 0.001 || u >= 0.999 || v <= 0.001 || v >= 0.999) continue;
          // Every LOD0 bilinear tap must have been shaded, including clamp-edge taps.
          const px = u * 400 - 0.5,
            py = v * 250 - 0.5;
          for (const tx of [Math.floor(px), Math.ceil(px)])
            for (const ty of [Math.floor(py), Math.ceil(py)]) {
              const cx = Math.max(0, Math.min(399, tx)),
                cy = Math.max(0, Math.min(249, ty));
              assert(
                cx >= x && cx < x + width && cy >= y && cy < y + height,
                `${cx},${cy} outside ${found}`,
              );
            }
          samples++;
        }
      }
  }
  assert(samples > 0);
  return found;
}

test("all coplanar receivers and actual distortion/filter taps fit one conservative union", () => {
  const bounds = [...box, -0.9, 0.009, 0.3, -0.8, 0.049, 0.5];
  const boundsBefore = [...bounds],
    matrixBefore = [...identity];
  const r = cover(identity, mirror, bounds, [0, 0.029]);
  assert(r[0] < 35);
  assert(r[2] < 350);
  assert.deepEqual(bounds, boundsBefore);
  assert.deepEqual(identity, matrixBefore);
  const small = rect(identity, mirror, box, [0], 0, 400, 250, 0, 0.03)!;
  const large = rect(identity, mirror, box, [0], 0, 400, 250, 0.1, 0.03)!;
  assert(large[2] > small[2] && large[3] > small[3]);
});

test("actual reflected projection covers oblique and near-coplanar water, including negative source scales", () => {
  const view = multiply(perspective(0.9, 1.6), lookAt([3, 3, 5], [0, 0, 0]));
  const reflected = multiply(view, mirror);
  const scaled = modelMatrix([0.3, 0.02, -0.2]);
  scaled[0] = -1.7;
  scaled[10] = -0.8;
  const points: V3[] = [
    [-1, 0, -1],
    [1, 0, -1],
    [1, 0, 1],
    [-1, 0, 1],
  ];
  const world = points.map(([x, y, z]) => [
    scaled[0] * x + scaled[12],
    y + scaled[13],
    scaled[10] * z + scaled[14],
  ]);
  const bounds = [0, 1, 2]
    .map((i) => Math.min(...world.map((p) => p[i])) - (i === 1 ? 0.02 : 0))
    .concat(
      [0, 1, 2].map(
        (i) => Math.max(...world.map((p) => p[i])) + (i === 1 ? 0.02 : 0),
      ),
    );
  const r = cover(view, reflected, bounds, [0.02]);
  assert(r[2] * r[3] < (400 * 250) / 2);
});

test("near-clip intersections, nonfinite and ill-conditioned projection retain the full pass", () => {
  assert.equal(
    scissor(
      identity,
      mirror,
      [-0.5, -0.02, -0.1, 0.5, 0.02, 0.3],
      [0],
      0,
      400,
      250,
      0.035,
      0.03,
    ),
    undefined,
  );
  const invalid = [...identity];
  invalid[0] = NaN;
  assert.equal(
    scissor(invalid, mirror, box, [0], 0, 400, 250, 0.035, 0.03),
    undefined,
  );
  const horizon = [...identity];
  horizon[15] = 1e-8;
  assert.equal(
    scissor(identity, horizon, box, [0], 0, 400, 250, 0.035, 0.03),
    undefined,
  );
  assert.equal(
    scissor(identity, mirror, [0, 0], [0], 0, 400, 250, 0.035, 0.03),
    undefined,
  );
});

test("receivers outside the reflected viewport can still sample an edge after distortion", () => {
  const shifted = [...mirror];
  shifted[12] = 0.35;
  const r = cover(
    identity,
    shifted,
    [0.75, -0.02, 0.2, 0.9, 0.02, 0.5],
    [0],
    0,
    0.1,
  );
  assert.equal(r[0] + r[2], 400);
  const left = [...mirror];
  left[12] = -0.35;
  assert.equal(
    cover(identity, left, [-0.9, -0.02, 0.2, -0.75, 0.02, 0.5], [0], 0, 0.1)[0],
    0,
  );
});

test("fully offscreen, behind-camera or other-height receivers cannot expand the visible union", () => {
  const expected = rect(identity, mirror, box, [0], 0, 400, 250, 0.035, 0.03);
  for (const extra of [
    [3, -0.02, 0.2, 4, 0.02, 0.4],
    [-0.5, -0.02, -2, 0.5, 0.02, -1],
  ]) {
    assert.deepEqual(
      rect(
        identity,
        mirror,
        [...box, ...extra],
        [0, 0],
        0,
        400,
        250,
        0.035,
        0.03,
      ),
      expected,
    );
    const empty = rect(identity, mirror, extra, [0], 0, 400, 250, 0.035, 0.03)!;
    assert.equal(empty[2] * empty[3], 0);
  }
  assert.deepEqual(
    rect(
      identity,
      mirror,
      [...box, -1, 4, 0.1, 1, 4.02, 0.8],
      [0, 4],
      0,
      400,
      250,
      0.035,
      0.03,
    ),
    expected,
  );
});

test("pooled storage does not retain old geometry, dimensions, distortion or matrices", () => {
  const first = rect(identity, mirror, box, [0], 0, 400, 250, 0.035, 0.03)!;
  const resized = rect(identity, mirror, box, [0], 0, 800, 500, 0.035, 0.03)!;
  assert(resized[2] > first[2]);
  assert.deepEqual(
    rect(identity, mirror, box, [0], 0, 400, 250, 0.035, 0.03),
    first,
  );
  assert.notDeepEqual(
    rect(
      identity,
      mirror,
      [-0.2, -0.02, 0.2, 0.2, 0.02, 0.3],
      [0],
      0,
      400,
      250,
      0.035,
      0.03,
    ),
    first,
  );
});
