import assert from "node:assert/strict";
import { test } from "node:test";
import {
  buildInstanceBounds,
  compactInstanceDraws,
  frustumPlanes,
  frustumVisibility,
  visibleDraws,
} from "../packages/renderer/culling.ts";
import {
  lookAt,
  modelMatrix,
  multiply,
  orthographic,
  perspective,
  transformPoint,
} from "../packages/renderer/math.ts";

const identity = modelMatrix([0, 0, 0]);

test("GPU compaction keeps source order and one draw per batch without mixing materials or geometry", () => {
  const models = new Float32Array(
    Array.from({ length: 8 }, (_, i) =>
      modelMatrix([i, i * 2, -i], i * 0.1),
    ).flat(),
  );
  const original = models.slice();
  const draws = [
    {
      firstInstance: 0,
      count: 4,
      material: 2,
      baseVertex: -3,
      firstIndex: 90,
      indexCount: 12,
    },
    {
      firstInstance: 0,
      count: 4,
      material: 7,
      baseVertex: 200,
      firstIndex: 102,
      indexCount: 6,
    },
    {
      firstInstance: 4,
      count: 3,
      material: 9,
      baseVertex: 1000,
      firstIndex: 108,
      indexCount: 36,
    },
    {
      firstInstance: 7,
      count: 1,
      material: 1,
      baseVertex: 50,
      firstIndex: 144,
      indexCount: 3,
    },
  ];
  const mask = new Uint8Array([0, 1, 0, 1, 0, 1, 1, 0]);
  const compacted = compactInstanceDraws(models, draws, mask);
  assert.deepEqual(compacted.draws, [
    { ...draws[0], firstInstance: 0, count: 2 },
    { ...draws[1], firstInstance: 0, count: 2 },
    { ...draws[2], firstInstance: 2, count: 2 },
  ]);
  for (const [gpuIndex, sourceIndex] of [1, 3, 5, 6].entries())
    assert.deepEqual(
      compacted.models.subarray(gpuIndex * 16, gpuIndex * 16 + 16),
      models.subarray(sourceIndex * 16, sourceIndex * 16 + 16),
    );
  assert.deepEqual(models, original);
  assert.deepEqual(mask, new Uint8Array([0, 1, 0, 1, 0, 1, 1, 0]));
  assert.equal(draws[0].count, 4);
  assert.notEqual(compacted.models.buffer, models.buffer);
});

test("compaction handles empty/all-visible views and rejects malformed matrices, flags and ranges", () => {
  const models = new Float32Array(
    [modelMatrix([1, 2, 3]), modelMatrix([4, 5, 6])].flat(),
  );
  const draws = [{ firstInstance: 0, count: 2, material: 4 }];
  assert.deepEqual(
    compactInstanceDraws(models, draws, new Uint8Array([1, 1])),
    { models, draws },
  );
  assert.deepEqual(compactInstanceDraws(models, draws, new Uint8Array(2)), {
    models: new Float32Array(),
    draws: [],
  });
  assert.deepEqual(
    compactInstanceDraws(new Float32Array(), [], new Uint8Array()),
    { models: new Float32Array(), draws: [] },
  );
  assert.throws(
    () => compactInstanceDraws(new Float32Array(15), [], new Uint8Array()),
    /16 floats/,
  );
  assert.throws(
    () => compactInstanceDraws(models, draws, new Uint8Array(1)),
    /16 floats/,
  );
  assert.throws(
    () => compactInstanceDraws(models, draws, new Uint8Array([1, 2])),
    /zero or one/,
  );
  for (const [offset, value, message] of [
    [4, NaN, /finite/],
    [31, Infinity, /finite/],
    [19, 1, /affine/],
    [15, 0, /affine/],
  ] as const) {
    const invalid = models.slice();
    invalid[offset] = value;
    assert.throws(
      () => compactInstanceDraws(invalid, draws, new Uint8Array(2)),
      message,
    );
  }
  for (const draw of [
    { firstInstance: -1, count: 1 },
    { firstInstance: 0.5, count: 1 },
    { firstInstance: 0, count: 3 },
    { firstInstance: 1, count: Infinity },
    { firstInstance: 0, count: -1 },
  ])
    assert.throws(
      () => compactInstanceDraws(models, [draw], new Uint8Array(2)),
      /inside/,
    );
});
function fixture(
  models: number[][],
  min = [-0.1, -0.1, -0.1],
  max = [0.1, 0.1, 0.1],
) {
  return {
    meshes: [
      { id: "owned:box", name: "box", primitives: [], bounds: { min, max } },
    ],
    instances: models.map((model) => ({
      mesh: 0,
      model,
      materialOverrides: [],
      actor: "owned",
      component: "owned",
      sourceClass: "fixture",
    })),
  };
}

test("world bounds enclose all eight corners under rotation, reflection, nonuniform scale and shear", () => {
  const models = [
    modelMatrix([12, -3, 7], 0.7, 2),
    [-2, 0.5, 0, 0, 0, 3, 0.2, 0, 1, 0, 0.25, 0, -4, 8, 3, 1],
  ];
  const scene = fixture(models, [-2, -1, -0.5], [3, 4, 2]);
  const bounds = buildInstanceBounds(scene);
  assert.equal(bounds.count, 2);
  for (let i = 0; i < models.length; i++) {
    const center = Array.from(bounds.spheres.subarray(i * 4, i * 4 + 3));
    let maxDistance = 0;
    for (let c = 0; c < 8; c++) {
      const p = transformPoint(models[i], [
        c & 1 ? 3 : -2,
        c & 2 ? 4 : -1,
        c & 4 ? 2 : -0.5,
      ]);
      p.forEach((n, axis) =>
        assert.ok(
          n >= bounds.aabbs[i * 6 + axis] &&
            n <= bounds.aabbs[i * 6 + axis + 3],
        ),
      );
      maxDistance = Math.max(
        maxDistance,
        Math.hypot(...p.map((n, axis) => n - center[axis])),
      );
    }
    assert.ok(Math.abs(bounds.spheres[i * 4 + 3] - maxDistance) < 1e-12);
  }
});

test("WebGPU identity frustum accepts touching bounds and rejects all six outside planes", () => {
  const positions: [number, number, number][] = [
    [0, 0, 0.5],
    [-1.1, 0, 0.5],
    [1.1, 0, 0.5],
    [0, -1.1, 0.5],
    [0, 1.1, 0.5],
    [0, 0, -0.1],
    [0, 0, 1.1],
    [-1.101, 0, 0.5],
    [1.101, 0, 0.5],
    [0, -1.101, 0.5],
    [0, 1.101, 0.5],
    [0, 0, -0.101],
    [0, 0, 1.101],
  ];
  const bounds = buildInstanceBounds(
    fixture(positions.map((p) => modelMatrix(p))),
  );
  const output = new Uint8Array(positions.length).fill(255);
  assert.equal(frustumVisibility(bounds, identity, output), output);
  assert.deepEqual(Array.from(output), [1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0]);
  assert.deepEqual(
    Array.from(frustumPlanes(identity).subarray(16, 20), (n) => n + 0),
    [0, 0, 1, 0],
  );
});

test("perspective and orthographic near/far planes use zero-to-one depth and reject behind-camera objects", () => {
  const positions: [number, number, number][] = [
    [0, 0, -1],
    [0, 0, -10],
    [0, 0, -0.99],
    [0, 0, -10.01],
    [0, 0, 1],
    [0, 0, -5],
    [5, 0, -5],
    [5.01, 0, -5],
  ];
  const bounds = buildInstanceBounds(
    fixture(
      positions.map((p) => modelMatrix(p)),
      [0, 0, 0],
      [0, 0, 0],
    ),
  );
  assert.deepEqual(
    Array.from(frustumVisibility(bounds, perspective(Math.PI / 2, 1, 1, 10))),
    [1, 1, 0, 0, 0, 1, 1, 0],
  );
  assert.deepEqual(
    Array.from(frustumVisibility(bounds, orthographic(-2, 2, -2, 2, 1, 10))),
    [1, 1, 0, 0, 0, 1, 0, 0],
  );
  const camera = multiply(
    perspective(Math.PI / 2, 1, 1, 10),
    lookAt([8, 2, 6], [8, 2, 0]),
  );
  const moved = buildInstanceBounds(
    fixture(
      [modelMatrix([8, 2, 1]), modelMatrix([8, 2, 7])],
      [0, 0, 0],
      [0, 0, 0],
    ),
  );
  assert.deepEqual(Array.from(frustumVisibility(moved, camera)), [1, 0]);
});

test("world AABB removes false sphere overlaps and infinite-far projections remain conservative", () => {
  const stretched = buildInstanceBounds(
    fixture([modelMatrix([0, 2, 0.5])], [-5, -0.1, -0.1], [5, 0.1, 0.1]),
  );
  assert.ok(stretched.spheres[3] > 5);
  assert.deepEqual(Array.from(frustumVisibility(stretched, identity)), [0]);
  const infinite = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, -1, 0, 0, -1, 0];
  const distant = buildInstanceBounds(
    fixture(
      [modelMatrix([0, 0, -1e9]), modelMatrix([0, 0, 0.1])],
      [0, 0, 0],
      [0, 0, 0],
    ),
  );
  assert.deepEqual(Array.from(frustumVisibility(distant, infinite)), [1, 0]);
});

test("draw compaction preserves geometry, material, base vertex and original ordering", () => {
  const draw = {
    firstInstance: 1,
    count: 7,
    firstIndex: 90,
    indexCount: 36,
    baseVertex: -12,
    material: 4,
    name: "owned",
  };
  const draws = [draw, { ...draw, firstInstance: 8, count: 2, material: 9 }];
  const mask = new Uint8Array([1, 0, 1, 1, 0, 1, 1, 1, 1, 0]);
  assert.deepEqual(visibleDraws(draws, mask), [
    { ...draw, firstInstance: 2, count: 2 },
    { ...draw, firstInstance: 5, count: 3 },
    { ...draws[1], firstInstance: 8, count: 1 },
  ]);
  assert.equal(draw.count, 7);
  assert.deepEqual(visibleDraws(draws, new Uint8Array(10)), []);
  assert.deepEqual(visibleDraws(draws, new Uint8Array(10).fill(1)), draws);
  assert.throws(
    () => visibleDraws([{ ...draw, firstInstance: 9 }], mask),
    /inside/,
  );
  assert.throws(() => visibleDraws([{ ...draw, count: -0.1 }], mask), /inside/);
});

test("invalid mesh bounds, transforms, view matrices and visibility buffers fail before use", () => {
  for (const model of [
    [...identity.slice(0, 15), NaN],
    identity.map((n, i) => (i === 3 ? 1 : n)),
    identity.slice(1),
  ])
    assert.throws(() => buildInstanceBounds(fixture([model])), /affine/);
  assert.throws(
    () => buildInstanceBounds(fixture([identity], [1, 0, 0], [0, 1, 1])),
    /ordered/,
  );
  const missing = fixture([identity]);
  missing.instances[0].mesh = 1;
  assert.throws(() => buildInstanceBounds(missing), /valid mesh/);
  assert.throws(() => frustumPlanes(new Float32Array(16)), /view-projection/);
  assert.throws(
    () => frustumPlanes([...identity.slice(0, 15), NaN]),
    /view-projection/,
  );
  const bounds = buildInstanceBounds(fixture([identity]));
  assert.throws(
    () => frustumVisibility(bounds, identity, new Uint8Array(2)),
    /match/,
  );
  assert.deepEqual(
    Array.from(frustumVisibility(buildInstanceBounds(fixture([])), identity)),
    [],
  );
});
