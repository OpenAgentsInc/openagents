import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decodeNormalSample,
  mapTangentNormal,
  normalFrame,
  normalMapFlags,
  sourceNormalFrame,
  sourceNormalFrameView,
  sourceNormalVertex,
} from "../packages/renderer/normals.ts";
const close = (actual: readonly number[], expected: readonly number[]) =>
  actual.forEach((v, i) =>
    assert.ok(Math.abs(v - expected[i]) < 1e-10, `${actual} != ${expected}`),
  );

test("normal vector encoding and tangent Y convention are independent", () => {
  assert.equal(normalMapFlags({}), 0);
  assert.equal(normalMapFlags({ normalConvention: "directx" }), 1);
  assert.equal(normalMapFlags({ normalEncoding: "xy-positive-z" }), 2);
  assert.equal(
    normalMapFlags({
      normalConvention: "directx",
      normalEncoding: "xy-positive-z",
    }),
    3,
  );
  const sample = [0.8, 0.5, 0] as const;
  close(decodeNormalSample(sample), [0.6, 0, -1]);
  close(decodeNormalSample(sample, 1), [0.6, 0, -1]);
  close(decodeNormalSample(sample, 2), [0.6, 0, 0.8]);
  close(decodeNormalSample(sample, 3), [0.6, 0, 0.8]);
  close(decodeNormalSample(sample, 6), [0.6, 0, 0.8]);
  close(decodeNormalSample(sample, 7), [0.6, 0, 0.8]);
  assert.throws(() => normalMapFlags({ normalEncoding: "other" } as any));
  assert.throws(() => normalMapFlags({ normalConvention: "other" } as any));
  assert.throws(() => decodeNormalSample([0.5, NaN, 1], 2), /finite/);
  assert.throws(() => decodeNormalSample(sample, 8), /flags/);
  close(decodeNormalSample([2, -1, 0], 2), [1, -1, 0]);
});

test("source normals reconstruct each filtered texture before blending or flattening", () => {
  const a = decodeNormalSample([0.8, 0.5, 0], 2),
    b = decodeNormalSample([0.2, 0.5, 1], 2);
  const blended = a.map((x, i) => (x + b[i]) * 0.5);
  close(blended, [0, 0, 0.8]);
  // Reconstructing after the blend would incorrectly produce Z=1.
  close(decodeNormalSample([0.5, 0.5, 0.5], 2), [0, 0, 1]);
  const flatness = 0.25;
  close(
    a.map((x, i) => x * (1 - flatness) + Number(i === 2) * flatness),
    [0.45, 0, 0.85],
  );
  assert.ok(decodeNormalSample([1, 1, 0], 2).every(Number.isFinite));
});

test("UV axes retain their world directions across viewport reversal and mirrored charts", () => {
  for (const screenY of [-1, 1])
    for (const mirrorU of [-1, 1])
      for (const mirrorV of [-1, 1]) {
        const f = normalFrame(
          [0, 0, 1],
          [2, 0, 0],
          [0, screenY * 3, 0],
          [mirrorU, 0],
          [0, screenY * mirrorV],
        );
        close(mapTangentNormal(f, [1, 0, 0]), [mirrorU, 0, 0]);
        close(mapTangentNormal(f, [0, 1, 0]), [0, mirrorV, 0]);
        close(mapTangentNormal(f, [0, 0, 1]), [0, 0, 1]);
      }
});

test("DirectX conversion and the physical back face affect the complete mapped normal", () => {
  const f = normalFrame([0, 0, 1], [2, 0, 0], [0, -3, 0], [1, 0], [0, -1]);
  const s = 1 / Math.sqrt(3);
  close(mapTangentNormal(f, [1, 1, 1]), [s, s, s]);
  close(mapTangentNormal(f, [1, 1, 1], true), [s, -s, s]);
  close(mapTangentNormal(f, [1, 1, 1], false, -1), [-s, -s, -s]);
  close(mapTangentNormal(f, [1, 1, 1], true, -1), [-s, s, -s]);
  close(mapTangentNormal(f, [1, 1, 1], true, 1, 0), [0, 0, 1]);
});

test("skewed charts produce an orthonormal frame and degenerate UVs preserve the source normal", () => {
  const f = normalFrame([0, 0, 1], [2, 0, 0], [1, 3, 0], [1, 0], [0, 1]);
  close(f.tangent, [1, 0, 0]);
  close(f.bitangent, [0, 1, 0]);
  for (const uv of [
    [0, 0],
    [1, 0],
  ] as const) {
    const invalid = normalFrame([0, 0, 1], [1, 0, 0], [0, 1, 0], [1, 0], uv);
    assert.equal(invalid.valid, false);
    close(mapTangentNormal(invalid, [1, 1, 0], true, -1), [0, 0, -1]);
  }
  assert.throws(
    () => normalFrame([NaN, 0, 1], [1, 0, 0], [0, 1, 0], [1, 0], [0, 1]),
    /finite/,
  );
});

test("source mesh basis removes axis scale and preserves mirror handedness", () => {
  const n = [0.6, 0, 0.8] as const,
    t = [0.8, 0, -0.6] as const;
  for (const mirror of [-1, 1]) {
    const m = [2 * mirror, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0.25, 0, 20, 30, 40, 1];
    const v = sourceNormalVertex(n, t, -1, m);
    close(v.normal, [0.6 * mirror, 0, 0.8]);
    close(v.tangent, [0.8 * mirror, 0, -0.6]);
    assert.equal(v.handedness, -mirror);
    const f = sourceNormalFrame(v.normal, v.tangent, v.handedness);
    close(mapTangentNormal(f, [0, 1, 0]), [0, -1, 0]);
    close(mapTangentNormal(f, [1, 0, 0]), [0.8 * mirror, 0, -0.6]);
    close(mapTangentNormal(f, [0, 0, 1]), [0.6 * mirror, 0, 0.8]);
  }
  assert.throws(
    () =>
      sourceNormalVertex(
        n,
        t,
        1,
        [1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      ),
    /shear/,
  );
  assert.throws(
    () => sourceNormalVertex(n, t, 1, new Array(16).fill(0)),
    /empty axis/,
  );
});

test("source basis derives B after interpolation without orthonormalizing T and N", () => {
  const f = sourceNormalFrame([0, 0.1, 0.9], [0.8, 0.15, -0.05], -1);
  close(f.bitangent, [0.14, -0.72, 0.08]);
  const mapped = mapTangentNormal(f, [0.5, 0.25, 0.75]);
  const raw = [
    0.8 * 0.5 + 0.14 * 0.25,
    0.15 * 0.5 - 0.72 * 0.25 + 0.1 * 0.75,
    -0.05 * 0.5 + 0.08 * 0.25 + 0.9 * 0.75,
  ];
  const len = Math.hypot(...raw);
  close(
    mapped,
    raw.map((v) => v / len),
  );
});

test("an interpolated source normal stays unit length when the mapped vector vanishes", () => {
  const f = sourceNormalFrame([0, 0, 0.9], [0.8, 0, 0], 1);
  close(mapTangentNormal(f, [1, 0, 0], false, 1, 0), [0, 0, 1]);
});

test("native fuzz projects a unit world view without normalizing the source coordinates", () => {
  const f = sourceNormalFrame([0, 0.1, 0.9], [0.8, 0.15, -0.05], -1),
    v = [0.6, 0, 0.8] as const;
  const projected = sourceNormalFrameView(f, v);
  close(projected, [0.44, 0.148, 0.72]);
  const sampled = [0.3, 0.4, 0.5];
  const dot = (a: readonly number[], b: readonly number[]) =>
    a.reduce((sum, x, i) => sum + x * b[i], 0);
  const world = f.normal.map(
    (x, i) =>
      x * sampled[2] + f.tangent[i] * sampled[0] + f.bitangent[i] * sampled[1],
  );
  assert.ok(Math.abs(dot(projected, sampled) - dot(v, world)) < 1e-12);
  assert.ok(Math.abs(Math.hypot(...projected) - 1) > 0.1);
});
