import assert from "node:assert/strict";
import { test } from "node:test";
import {
  LeafSampler,
  validateLeaves,
  validateLeafGeometry,
} from "../packages/renderer/leaves.ts";
import { leafFixture } from "./fixtures/leaves.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";

test("petals use zero-warmup positive births, exact closed-form gravity and stable original mesh scales", () => {
  const m = leafFixture(),
    s = new LeafSampler(m);
  assert.equal(s.sample("day", 0).count, 0);
  assert.equal(s.sample("day", 0.49).count, 0);
  const atBirth = s.sample("day", 0.5);
  assert.equal(atBirth.count, 1);
  assert.equal(atBirth.data[13], 1);
  assert.equal(atBirth.data[0], 1);
  const after = s.sample("day", 1);
  assert.equal(after.count, 2);
  assert.equal(after.data[13], 1);
  assert.equal(after.data[33], 0.875);
  assert.ok(after.data.every(Number.isFinite));
  assert.deepEqual(m, leafFixture());
});
test("petal sampling stays bounded through one year and reverse fixed captures do not mutate live state", () => {
  const s = new LeafSampler(leafFixture()),
    early = s.sample("day", 1).data.slice();
  for (const t of [3, 30, MAX_WORLD_ANIMATION_TIME]) {
    const sample = s.sample("day", t);
    assert.ok(sample.count <= 5);
    assert.ok(sample.data.every(Number.isFinite));
  }
  assert.deepEqual(s.sample("day", 1).data, early);
  assert.equal(s.sample("night", 1).count, 0);
  assert.throws(() => s.sample("day", MAX_WORLD_ANIMATION_TIME + 1));
});
test("leaf recipe rejects excessive populations, malformed transforms, lifecycle guesses and material overflow", () => {
  const changes: Array<(m: ReturnType<typeof leafFixture>) => void> = [
    (m) => (m.profiles[0].spawnRate = NaN),
    (m) => (m.profiles[0].lifetime = [3, 2]),
    (m) => (m.scenarios.day[0].model[0] = 2),
    (m) => (m.scenarios.day[0].lifecycle.warmupSeconds = 1 as 0),
    (m) => (m.material.vegetation!.color.contrast = -1),
    (m) => {
      m.profiles[0].spawnRate = 100;
      m.profiles[0].lifetime = [60, 60];
    },
  ];
  for (const change of changes) {
    const m = leafFixture();
    change(m);
    assert.throws(() => validateLeaves(m));
  }
});
test("leaf geometry enforces finite normals, UV/colors, counts and index ownership before GPU allocation", () => {
  const m = leafFixture(),
    vertices = new Float32Array(36),
    indices = new Uint32Array([0, 1, 2]);
  for (let i = 0; i < 3; i++) {
    vertices[i * 12 + 5] = 1;
    vertices.set([1, 1, 1, 1], i * 12 + 8);
  }
  validateLeafGeometry(m, vertices.buffer, indices.buffer);
  indices[0] = 3;
  assert.throws(() => validateLeafGeometry(m, vertices.buffer, indices.buffer));
  indices[0] = 0;
  vertices[5] = 0;
  assert.throws(() => validateLeafGeometry(m, vertices.buffer, indices.buffer));
  vertices[5] = 1;
  vertices[8] = 2;
  assert.throws(() => validateLeafGeometry(m, vertices.buffer, indices.buffer));
});
