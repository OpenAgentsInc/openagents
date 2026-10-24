import { test } from "node:test";
import assert from "node:assert/strict";
import { rayBox, rayFromNdc } from "../packages/renderer/picking.ts";
import {
  lookAt,
  multiply,
  normalize,
  orthographic,
  perspective,
  sub,
} from "../packages/renderer/math.ts";
test("picking rejects objects behind and off the reticle", () => {
  const b = { min: [-1, -1, -1], max: [1, 1, 1] } as const;
  assert.equal(rayBox([0, 0, 5], [0, 0, -1], b as any), 4);
  assert.equal(rayBox([3, 0, 5], [0, 0, -1], b as any), null);
  assert.equal(rayBox([0, 0, 5], [0, 0, 1], b as any), null);
});
test("picking transformed bounds reports world distance and respects rotated narrow walls", () => {
  const b = { min: [-0.1, 0, -2], max: [0.1, 3, 2] } as any;
  const t = { position: [5, 0, 0], yaw: Math.PI / 2, scale: 2 } as any;
  assert.ok(Math.abs(rayBox([5, 1, 5], [0, 0, -1], b, t)! - 4.8) < 1e-6);
  assert.equal(rayBox([10, 1, 5], [0, 0, -1], b, t), null);
});
test("screen rays unproject WebGPU clip depths with the camera's actual pose", () => {
  const eye = [10, 5, 20] as [number, number, number],
    target = [2, 3, -10] as [number, number, number];
  const viewProjection = multiply(
    perspective(Math.PI / 3, 1.5, 0.1, 2000),
    lookAt(eye, target),
  );
  const direction = rayFromNdc(viewProjection, [0, 0]),
    expected = normalize(sub(target, eye));
  assert(Math.abs(Math.hypot(...direction) - 1) < 1e-12);
  direction.forEach((v, i) => assert(Math.abs(v - expected[i]) < 1e-5));
  const front = multiply(
    perspective(Math.PI / 3, 1.5, 0.1, 2000),
    lookAt([0, 0, 0], [0, 0, -1]),
  );
  const corner = rayFromNdc(front, [1, 1]),
    wanted = normalize([
      1.5 * Math.tan(Math.PI / 6),
      Math.tan(Math.PI / 6),
      -1,
    ]);
  corner.forEach((v, i) => assert(Math.abs(v - wanted[i]) < 1e-6));
  const box = { min: [-0.2, -0.2, -5], max: [0.2, 0.2, -4] } as any;
  assert.equal(rayBox([0, 0, 0], rayFromNdc(front, [0, 0]), box), 4);
  assert.equal(rayBox([0, 0, 0], corner, box), null);
});
test("orthographic screen rays stay parallel and invalid camera inputs cannot pick", () => {
  const projection = multiply(
    orthographic(-10, 10, -10, 10, 0.1, 100),
    lookAt([0, 2, 5], [0, 2, -5]),
  );
  assert.deepEqual(rayFromNdc(projection, [-1, 1]), [0, 0, -1]);
  assert.deepEqual(rayFromNdc(projection, [1, -1]), [0, 0, -1]);
  const valid = perspective(Math.PI / 3, 1.5, 0.1, 2000);
  for (const matrix of [
    [],
    Array(16).fill(0),
    [...valid].map((n, i) => (i === 2 ? NaN : n)),
    [...valid].map((n, i) => (i === 2 ? Infinity : n)),
  ])
    assert.throws(() => rayFromNdc(matrix, [0, 0]));
  for (const ndc of [[0], [NaN, 0], [0, Infinity], [1.001, 0]])
    assert.throws(() => rayFromNdc(valid, ndc as any));
});
