import assert from "node:assert/strict";
import test from "node:test";
import {
  createFishMesh,
  FISH_TRIANGLES,
  FISH_VERTEX_STRIDE,
} from "../packages/boids/mesh.ts";

test("the original fish submits exactly 310 real indexed triangles", () => {
  const mesh = createFishMesh();
  assert.equal(mesh.triangleCount, 310);
  assert.equal(mesh.indices.length, FISH_TRIANGLES * 3);
  assert.equal(FISH_TRIANGLES * 100_000, 31_000_000);
  assert.equal(mesh.vertices.byteLength % FISH_VERTEX_STRIDE, 0);
  const count = mesh.vertices.length / 8;
  assert.equal(count, 200);
  assert.ok([...mesh.indices].every((index) => index < count));
});

test("every fish triangle has nonzero area and finite unit normals", () => {
  const { vertices: v, indices } = createFishMesh();
  for (let i = 0; i < v.length; i += 8) {
    for (let c = 0; c < 8; c++) assert.ok(Number.isFinite(v[i + c]));
    assert.ok(Math.abs(Math.hypot(v[i + 3], v[i + 4], v[i + 5]) - 1) < 1e-6);
  }
  for (let i = 0; i < indices.length; i += 3) {
    const a = indices[i] * 8,
      b = indices[i + 1] * 8,
      c = indices[i + 2] * 8;
    const ab = [v[b] - v[a], v[b + 1] - v[a + 1], v[b + 2] - v[a + 2]];
    const ac = [v[c] - v[a], v[c + 1] - v[a + 1], v[c + 2] - v[a + 2]];
    const area = Math.hypot(
      ab[1] * ac[2] - ab[2] * ac[1],
      ab[2] * ac[0] - ab[0] * ac[2],
      ab[0] * ac[1] - ab[1] * ac[0],
    );
    assert.ok(area > 1e-7, `Triangle ${i / 3} must contain real geometry.`);
  }
});

test("fish geometry is deterministic and bounds the complete tail and fins", () => {
  const a = createFishMesh(),
    b = createFishMesh();
  assert.deepEqual(a.vertices, b.vertices);
  assert.deepEqual(a.indices, b.indices);
  const xyz = Array.from({ length: a.vertices.length / 8 }, (_, i) =>
    a.vertices.subarray(i * 8, i * 8 + 3),
  );
  assert.ok(Math.min(...xyz.map((p) => p[0])) < -1.25);
  assert.ok(Math.max(...xyz.map((p) => p[0])) > 0.85);
  assert.ok(Math.max(...xyz.map((p) => p[1])) > 0.5);
  assert.ok(Math.max(...xyz.map((p) => p[2])) > 0.55);
  assert.ok(Math.min(...xyz.map((p) => p[2])) < -0.55);
});
