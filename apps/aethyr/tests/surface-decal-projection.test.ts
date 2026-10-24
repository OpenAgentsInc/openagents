import assert from "node:assert/strict";
import test from "node:test";
import {
  buildSurfaceDecalDraws,
  packSurfaceDecalDraws,
  SURFACE_DECAL_LIMITS,
  type SurfaceDecalDraw,
  type SurfaceDecalInstance,
} from "../packages/renderer/surface-decals.ts";
import {
  multiply,
  perspective,
  lookAt,
  inverse,
  type V3,
} from "../packages/renderer/math.ts";
const CORNERS = Array.from({ length: 8 }, (_, i) => [
  i & 1 ? 1 : -1,
  i & 2 ? 1 : -1,
  i & 4 ? 1 : -1,
]);
function point(m: ArrayLike<number>, x: number, y: number, z: number) {
  return [0, 1, 2, 3].map(
    (axis) => m[axis] * x + m[axis + 4] * y + m[axis + 8] * z + m[axis + 12],
  );
}
function referenceDraws(
  instances: readonly SurfaceDecalInstance[],
  viewProjection: ArrayLike<number>,
  width: number,
  height: number,
): SurfaceDecalDraw[] {
  if (
    viewProjection.length !== 16 ||
    !Array.from(viewProjection).every(Number.isFinite) ||
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > SURFACE_DECAL_LIMITS.maxDimension ||
    height > SURFACE_DECAL_LIMITS.maxDimension
  )
    throw new Error("Use finite camera data and a supported decal image size.");
  const result: SurfaceDecalDraw[] = [];
  for (const instance of [...instances].sort(
    (a, b) => a.sortOrder - b.sortOrder || a.sourceOrder - b.sourceOrder,
  )) {
    const clip = CORNERS.map((c) => {
      const p = point(instance.worldFromDecal, c[0], c[1], c[2]);
      return point(viewProjection, p[0], p[1], p[2]);
    });
    if (
      clip.every((p) => p[0] < -p[3]) ||
      clip.every((p) => p[0] > p[3]) ||
      clip.every((p) => p[1] < -p[3]) ||
      clip.every((p) => p[1] > p[3]) ||
      clip.every((p) => p[2] < 0) ||
      clip.every((p) => p[2] > p[3])
    )
      continue;
    let scissor: [number, number, number, number] = [0, 0, width, height];
    // A volume crossing the eye/near plane needs the whole viewport as a safe
    // bound. Per-pixel clipping still rejects positions outside that volume.
    if (clip.every((p) => p[3] > 1e-7 && p[2] >= 0)) {
      const x = clip.map((p) => ((p[0] / p[3]) * 0.5 + 0.5) * width);
      const y = clip.map((p) => (0.5 - (p[1] / p[3]) * 0.5) * height);
      const x0 = Math.max(0, Math.floor(Math.min(...x)) - 1);
      const y0 = Math.max(0, Math.floor(Math.min(...y)) - 1);
      const x1 = Math.min(width, Math.ceil(Math.max(...x)) + 1);
      const y1 = Math.min(height, Math.ceil(Math.max(...y)) + 1);
      if (x1 <= x0 || y1 <= y0) continue;
      scissor = [x0, y0, x1 - x0, y1 - y0];
    }
    result.push({ instance, scissor });
  }
  return result;
}

test("decal scratch projection preserves reference rectangles, order and uploaded bits across moving cameras", () => {
  let seed = 0x417aff;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x100000000;
  };
  const instances: SurfaceDecalInstance[] = Array.from(
    { length: 72 },
    (_, i) => {
      const sx = (random() + 0.01) * (i % 3 === 0 ? -1 : 1),
        sy = random() + 0.01,
        sz = random() + 0.01;
      const model = [
        sx,
        0,
        0,
        0,
        0,
        sy,
        0,
        0,
        0,
        0,
        sz,
        0,
        (random() - 0.5) * 30,
        (random() - 0.5) * 10,
        (random() - 0.5) * 30,
        1,
      ];
      return {
        id: String(i),
        material: i % 5,
        sourceOrder: i,
        sortOrder: i % 7,
        worldFromDecal: model,
        worldToDecal: [...inverse(model)],
        bounds: { min: [-100, -100, -100], max: [100, 100, 100] },
        fadeScreenSize: 0.01,
      };
    },
  );
  for (let i = 0; i < 240; i++) {
    const eye: V3 = [
      (random() - 0.5) * 30,
      (random() - 0.5) * 10,
      (random() - 0.5) * 30,
    ];
    const target: V3 = [
      eye[0] + random() - 0.5,
      eye[1] + random() - 0.5,
      eye[2] + 1,
    ];
    const width = i % 2 ? 1280 : 800,
      height = i % 2 ? 720 : 500;
    const view = multiply(
      perspective(0.3 + random() * 2, width / height, 0.01 + random()),
      lookAt(eye, target),
    );
    const expected = referenceDraws(instances, view, width, height);
    const actual = buildSurfaceDecalDraws(instances, view, width, height);
    assert.deepEqual(actual, expected);
    assert.deepEqual(
      new Uint32Array(packSurfaceDecalDraws(actual, width, height).buffer),
      new Uint32Array(packSurfaceDecalDraws(expected, width, height).buffer),
    );
  }
  const m = instances[0].worldFromDecal as number[];
  m[12] += 25;
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  assert.deepEqual(
    buildSurfaceDecalDraws(instances, identity, 320, 200),
    referenceDraws(instances, identity, 320, 200),
  );
});
