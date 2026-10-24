import test from "node:test";
import assert from "node:assert/strict";
import {
  perspective,
  lookAt,
  multiply,
  inverse,
  direction,
  modelMatrix,
  transformPoint,
} from "../packages/renderer/math.ts";
const close = (a: number, b: number) =>
  assert.ok(Math.abs(a - b) < 1e-4, `${a} != ${b}`);
test("WebGPU projection maps the camera near and far planes to0and1", () => {
  const p = perspective(1, 16 / 9, 0.1, 2000);
  const project = (z: number) => (p[10] * z + p[14]) / -z;
  close(project(-0.1), 0);
  close(project(-2000), 1);
});
test("camera matrices roundtrip and preserve canonical forward orientation", () => {
  const p = multiply(perspective(1, 16 / 9), lookAt([10, 4, 25], [10, 4, 0]));
  const roundtrip = multiply(p, inverse(p));
  roundtrip.forEach((x, i) => close(x, i % 5 === 0 ? 1 : 0));
  assert.deepEqual(direction(0, 0), [0, 0, -1]);
});
test("authored model matrices keep meters,translation,and yaw", () => {
  const m = modelMatrix([10, 2, -3], Math.PI / 2, 2);
  const p = transformPoint(m, [0, 0, -1]);
  close(p[0], 8);
  close(p[1], 2);
  close(p[2], -3);
});
