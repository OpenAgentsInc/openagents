import assert from "node:assert/strict";
import { test } from "node:test";
import {
  buildWaterJets,
  buildWaterSplashes,
  WATER_JET_LIMITS,
  WATER_JET_BREAKUP_METERS,
  type WaterJet,
} from "../packages/renderer/water.ts";

const jet: WaterJet = {
  id: "synthetic-stream",
  origin: [1, 2, 3],
  velocity: [2, 1, -1],
  surfaceY: 0.5,
  radius: 0.025,
};

test("a ballistic stream reaches the declared water plane and uses the renderer vertex layout", () => {
  const mesh = buildWaterJets([jet]);
  assert.equal(mesh.vertices.length, 25 * 9 * 12);
  assert.equal(mesh.indices.length, 24 * 8 * 6);
  const impact = mesh.impacts[0];
  const t = impact.flightTime;
  assert.ok(t > 0);
  assert.ok(
    Math.abs(
      jet.origin[1] + jet.velocity[1] * t - 4.905 * t * t - jet.surfaceY,
    ) < 1e-10,
  );
  assert.deepEqual(impact.position, [1 + 2 * t, 0.5, 3 - t]);
  const last = mesh.vertices.subarray(24 * 9 * 12, 25 * 9 * 12);
  const center = [0, 1, 2].map(
    (axis) =>
      Array.from({ length: 8 }, (_, i) => last[i * 12 + axis]).reduce(
        (a, b) => a + b,
      ) / 8,
  );
  center.forEach((n, i) => assert.ok(Math.abs(n - impact.position[i]) < 1e-6));
  assert.ok(Math.abs(last[7] - t) < 1e-6);
});

test("transported frames remain finite and orthonormal across an arc apex", () => {
  const mesh = buildWaterJets([
    { ...jet, velocity: [0.001, 8, 0], segments: 256 },
  ]);
  assert.ok(mesh.vertices.every(Number.isFinite));
  for (let i = 0; i < mesh.vertices.length; i += 12) {
    const normal = mesh.vertices.subarray(i + 3, i + 6),
      tangent = mesh.vertices.subarray(i + 8, i + 11);
    assert.ok(Math.abs(Math.hypot(...normal) - 1) < 1e-6);
    assert.ok(Math.abs(Math.hypot(...tangent) - 1) < 1e-6);
    assert.ok(
      Math.abs(normal.reduce((sum, n, axis) => sum + n * tangent[axis], 0)) <
        1e-6,
    );
  }
  for (let ring = 1; ring <= 256; ring++) {
    const a = (ring - 1) * 9 * 12 + 3,
      b = ring * 9 * 12 + 3;
    const dot = [0, 1, 2].reduce(
      (sum, axis) => sum + mesh.vertices[a + axis] * mesh.vertices[b + axis],
      0,
    );
    assert.ok(dot > 0.99, "The transported normal must not flip at the apex.");
  }
});

test("triangle winding faces outward and every index stays inside its mesh", () => {
  const mesh = buildWaterJets([jet]);
  for (let i = 0; i < mesh.indices.length; i += 3) {
    const ids = Array.from(mesh.indices.subarray(i, i + 3));
    assert.ok(ids.every((id) => id < mesh.vertices.length / 12));
    const points = ids.map((id) =>
      Array.from(mesh.vertices.subarray(id * 12, id * 12 + 3)),
    );
    const a = points[1].map((n, axis) => n - points[0][axis]),
      b = points[2].map((n, axis) => n - points[0][axis]);
    const cross = [
      a[1] * b[2] - a[2] * b[1],
      a[2] * b[0] - a[0] * b[2],
      a[0] * b[1] - a[1] * b[0],
    ];
    const normal = mesh.vertices.subarray(ids[0] * 12 + 3, ids[0] * 12 + 6);
    assert.ok(cross.reduce((sum, n, axis) => sum + n * normal[axis], 0) > 0);
  }
});

test("bounds enclose every float32 vertex and multiple streams use separate index ranges", () => {
  const mesh = buildWaterJets([
    jet,
    { ...jet, id: "second-stream", origin: [-4, 5, -6] },
  ]);
  assert.ok(mesh.bounds);
  for (let i = 0; i < mesh.vertices.length; i += 12)
    for (let axis = 0; axis < 3; axis++)
      assert.ok(
        mesh.vertices[i + axis] >= mesh.bounds.min[axis] &&
          mesh.vertices[i + axis] <= mesh.bounds.max[axis],
      );
  const firstCount = 25 * 9;
  assert.ok(mesh.indices.subarray(24 * 8 * 6).every((id) => id >= firstCount));
  assert.equal(mesh.impacts.length, 2);
  assert.deepEqual(buildWaterJets([]), {
    vertices: new Float32Array(),
    indices: new Uint32Array(),
    impacts: [],
    bounds: null,
  });
});

test("downward vertical streams work, while upward self-overlapping and stationary streams fail", () => {
  assert.ok(
    buildWaterJets([{ ...jet, velocity: [0, -1, 0] }]).vertices.every(
      Number.isFinite,
    ),
  );
  assert.throws(
    () => buildWaterJets([{ ...jet, velocity: [0, 1, 0] }]),
    /horizontal/,
  );
  assert.throws(
    () => buildWaterJets([{ ...jet, velocity: [0, 0, 0] }]),
    /finite/,
  );
  assert.throws(
    () =>
      buildWaterJets([{ ...jet, origin: [1, 0.5, 3], velocity: [1, -1, 0] }]),
    /within 10 seconds/,
  );
});

test("invalid parameters and oversized allocations fail before producing a mesh", () => {
  for (const patch of [
    { origin: [NaN, 2, 3] },
    { velocity: [0, Infinity, 1] },
    { radius: 0 },
    { radius: 2 },
    { gravity: 0 },
    { surfaceY: 3 },
    { segments: 1 },
    { sides: 2 },
    { segments: 2.5 },
    { origin: [100_001, 2, 3] },
    { origin: [100_000, 2, 3] },
    {
      origin: [1, 99_999, 3],
      velocity: [1, 20, 0],
      surfaceY: 99_998,
      gravity: 100,
    },
    { origin: [1, 1000, 3] },
    { id: "" },
  ])
    assert.throws(() => buildWaterJets([{ ...jet, ...patch } as WaterJet]));
  assert.throws(() => buildWaterJets([jet, jet]), /unique/);
  assert.throws(
    () =>
      buildWaterJets(
        Array.from({ length: WATER_JET_LIMITS.jets + 1 }, (_, i) => ({
          ...jet,
          id: String(i),
        })),
      ),
    /64/,
  );
  assert.throws(
    () =>
      buildWaterJets(
        Array.from({ length: 32 }, (_, i) => ({
          ...jet,
          id: String(i),
          segments: 256,
          sides: 32,
        })),
      ),
    /Reduce/,
  );
});

test("optional impact widening retains the nozzle and produces outward taper normals", () => {
  const source = { ...jet, endRadius: 0.075, taper: 1.5 };
  const mesh = buildWaterJets([source]);
  for (const [ring, radius] of [
    [0, source.radius],
    [24, source.endRadius],
  ]) {
    const time = (mesh.impacts[0].flightTime * ring) / 24;
    const center = [1 + 2 * time, 2 + time - 4.905 * time * time, 3 - time];
    const p = mesh.vertices.subarray(ring * 9 * 12, ring * 9 * 12 + 3);
    assert.ok(
      Math.abs(Math.hypot(...p.map((n, i) => n - center[i])) - radius) < 1e-6,
    );
  }
  assert.equal(mesh.vertices[11], 0);
  assert.equal(mesh.vertices[24 * 9 * 12 + 11], 1);
  assert.ok(mesh.vertices.every(Number.isFinite));
  for (const patch of [
    { endRadius: NaN },
    { endRadius: 0 },
    { taper: 0.5 },
    { taper: 5 },
  ])
    assert.throws(() => buildWaterJets([{ ...source, ...patch }]));
});

test("impact spray is bounded, deterministic, finite, and reaches its declared height", () => {
  const splash = { radius: 0.22, height: 0.12, droplets: 32, size: 0.011 };
  const impacts = buildWaterJets([{ ...jet, splash }]).impacts;
  const mesh = buildWaterSplashes(impacts);
  assert.deepEqual(mesh, buildWaterSplashes(impacts));
  assert.equal(mesh.vertices.length, 32 * 4 * 12);
  assert.equal(mesh.indices.length, 32 * 6);
  assert.ok(mesh.vertices.every(Number.isFinite));
  for (let i = 0; i < mesh.vertices.length; i += 48) {
    const v = mesh.vertices.subarray(i, i + 12),
      lifetime = v[9];
    assert.ok(v[8] >= 0 && v[8] < 1);
    assert.ok(v[4] ** 2 / (2 * v[11]) <= splash.height + 1e-6);
    assert.ok(Math.hypot(v[3], v[5]) * lifetime <= splash.radius + 1e-6);
    assert.ok(v[10] > 0 && v[10] <= splash.size);
    assert.ok(Math.abs(v[4] * lifetime - 0.5 * v[11] * lifetime ** 2) < 1e-6);
  }
  assert.deepEqual(
    buildWaterSplashes(buildWaterJets([jet]).impacts).indices,
    new Uint32Array(),
  );
  for (const patch of [
    { radius: Infinity },
    { height: 0 },
    { droplets: 65 },
    { size: -0.1 },
  ]) {
    assert.throws(() =>
      buildWaterJets([{ ...jet, splash: { ...splash, ...patch } }]),
    );
    assert.throws(() =>
      buildWaterSplashes([{ ...impacts[0], splash: { ...splash, ...patch } }]),
    );
  }
});

test("stream bounds include the complete animated surface envelope", () => {
  const mesh = buildWaterJets([{ ...jet, endRadius: 0.075 }]);
  for (let axis = 0; axis < 3; axis++) {
    const values = Array.from(
      { length: mesh.vertices.length / 12 },
      (_, i) => mesh.vertices[i * 12 + axis],
    );
    assert.ok(
      Math.abs(
        Math.min(...values) - mesh.bounds!.min[axis] - WATER_JET_BREAKUP_METERS,
      ) < 1e-10,
    );
    assert.ok(
      Math.abs(
        mesh.bounds!.max[axis] - Math.max(...values) - WATER_JET_BREAKUP_METERS,
      ) < 1e-10,
    );
  }
  assert.throws(
    () =>
      buildWaterJets([
        { ...jet, origin: [99_999.97, 2, 3], velocity: [0, -1, 0] },
      ]),
    /world bounds/,
  );
});
