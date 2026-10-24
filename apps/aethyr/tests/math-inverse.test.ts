import assert from "node:assert/strict";
import test from "node:test";
import {
  inverse,
  lookAt,
  modelMatrix,
  multiply,
  normalize,
  orthographic,
  perspective,
  type V3,
} from "../packages/renderer/math.ts";
import {
  fitDirectionalShadow,
  SUN_SHADOW_FITS,
} from "../packages/renderer/distant-shadow.ts";

function referenceInverse(m: ArrayLike<number>): Float32Array {
  const a = Array.from({ length: 4 }, (_, r) =>
    Array.from({ length: 8 }, (_, c) =>
      c < 4 ? m[c * 4 + r] : Number(c - 4 === r),
    ),
  );
  for (let k = 0; k < 4; k++) {
    let pivot = k;
    for (let j = k + 1; j < 4; j++)
      if (Math.abs(a[j][k]) > Math.abs(a[pivot][k])) pivot = j;
    [a[k], a[pivot]] = [a[pivot], a[k]];
    const d = a[k][k];
    if (Math.abs(d) < 1e-12) throw new Error("Camera matrix is singular");
    for (let c = 0; c < 8; c++) a[k][c] /= d;
    for (let r = 0; r < 4; r++)
      if (r !== k) {
        const f = a[r][k];
        for (let c = 0; c < 8; c++) a[r][c] -= f * a[k][c];
      }
  }
  return new Float32Array(
    Array.from({ length: 16 }, (_, i) => a[i % 4][4 + Math.floor(i / 4)]),
  );
}

function sameBits(matrix: ArrayLike<number>) {
  const expected = referenceInverse(matrix),
    actual = inverse(matrix);
  assert.deepEqual(
    new Uint32Array(actual.buffer),
    new Uint32Array(expected.buffer),
  );
  return actual;
}

function random(seed: number) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x1_0000_0000;
  };
}

test("inverse retains every output bit for seeded dense and affine matrices", () => {
  const next = random(0x5ce1a);
  for (let sample = 0; sample < 1024; sample++) {
    const dense = Array.from({ length: 16 }, () => next() * 2 - 1);
    for (const diagonal of [0, 5, 10, 15]) dense[diagonal] += 4;
    sameBits(dense);
    sameBits(new Float32Array(dense));
    const affine = modelMatrix(
      [next() * 2000 - 1000, next() * 200 - 100, next() * 2000 - 1000],
      next() * Math.PI * 2,
      0.05 + next() * 8,
    );
    for (let column = 0; column < 3; column++) {
      const scale = (sample & (1 << column) ? -1 : 1) * (0.1 + next() * 8);
      for (let row = 0; row < 3; row++) affine[column * 4 + row] *= scale;
    }
    sameBits(affine);
    sameBits(new Float32Array(affine));
  }
});

test("inverse retains view, reflected view, perspective and shadow projection bits", () => {
  const poses: [V3, V3][] = [
    [
      [6, 4, 20],
      [6, 3.7, 3.5],
    ],
    [
      [-13.3, 2.1, -24.8],
      [-1, 5, 14],
    ],
    [
      [3, -4, 12],
      [0, 1, -2],
    ],
    [
      [10000, 20, -30000],
      [9990, 18, -30000],
    ],
  ];
  for (const [eye, target] of poses) {
    for (const up of [
      [0, 1, 0],
      [0, -1, 0],
    ] as V3[])
      for (const aspect of [1, 1.6, 2.4]) {
        const view = lookAt(eye, target, up);
        sameBits(view);
        sameBits(multiply(perspective(Math.PI / 3, aspect, 0.1, 2000), view));
        sameBits(multiply(perspective(Math.PI / 2, aspect, 0.05, 40), view));
        sameBits(multiply(orthographic(-20, 20, -10, 10, 0.05, 1000), view));
      }
    const forward = normalize(target.map((v, i) => v - eye[i]) as V3);
    for (const fit of Object.values(SUN_SHADOW_FITS))
      sameBits(
        fitDirectionalShadow(eye, forward, [0.2, 0.8, -0.56], fit).matrix,
      );
  }
});

test("inverse keeps pivot swaps, equal-magnitude pivot ties and signed zero", () => {
  for (const matrix of [
    [0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0],
    [1, -1, 1, -1, 0, 2, 1, 0, 0, 0, 3, 1, 0, 0, 0, 4],
    [1, -0, 0, -0, -0, -1, -0, 0, 0, -0, 1, -0, -0, 0, -0, 1],
  ])
    sameBits(matrix);
});

test("inverse preserves the strict singular pivot threshold", () => {
  for (const value of [0, -0, 1e-13, -1e-13, 1e-12 * (1 - Number.EPSILON)]) {
    const matrix = modelMatrix([0, 0, 0]);
    matrix[0] = value;
    assert.throws(() => referenceInverse(matrix), /Camera matrix is singular/);
    assert.throws(() => inverse(matrix), /Camera matrix is singular/);
  }
  for (const value of [1e-12, -1e-12, 1e-12 * (1 + Number.EPSILON), 1e-11]) {
    const matrix = modelMatrix([0, 0, 0]);
    matrix[0] = value;
    sameBits(matrix);
  }
  const dependent = [1, 2, 3, 4, 2, 4, 6, 8, 0, 1, 0, 1, 1, 0, 0, 1];
  assert.throws(() => referenceInverse(dependent), /Camera matrix is singular/);
  assert.throws(() => inverse(dependent), /Camera matrix is singular/);
});

test("inverse leaves its input untouched and returns independent storage", () => {
  const matrix = multiply(
      perspective(1.1, 1.6),
      lookAt([6, 4, 20], [6, 3.7, 3.5]),
    ),
    before = matrix.slice(),
    first = sameBits(matrix),
    second = sameBits(matrix);
  assert.deepEqual(matrix, before);
  assert.notEqual(first.buffer, second.buffer);
  first.fill(0);
  assert.deepEqual(second, referenceInverse(matrix));
});
