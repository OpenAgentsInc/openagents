import assert from "node:assert/strict";
import { test } from "node:test";
import {
  bakeSkyProbes,
  createGiProbeTrace,
  giEnvironmentIdentityPayload,
  giEnvironmentSampler,
  giMomentVisibility,
  giOctDecode,
  giOctEncode,
  giSceneIdentityPayload,
  type GiProbeManifest,
  type GiProbeTrace,
} from "../packages/renderer/gi-probes.ts";
import {
  buildGiAcceleration,
  traceGi,
} from "../packages/renderer/gi-acceleration.ts";
import { evaluateDiffuseSH9 } from "../packages/renderer/environment-lighting.ts";
import {
  dot,
  modelMatrix,
  normalize,
  type V3,
} from "../packages/renderer/math.ts";

const grid: GiProbeManifest["grid"] = {
  origin: [-1, 1, 0],
  spacing: [2, 1, 1],
  dimensions: [2, 2, 2],
  count: 8,
};
const coefficient = (records: Float32Array, index: number) =>
  records.subarray(index * 40 + 4, index * 40 + 40);
function close(actual: number, expected: number, tolerance = 0.02) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} differs from ${expected}`,
  );
}

test("open constant sky has the same world SH9 E/pi convention as the environment field", () => {
  const bake = bakeSkyProbes(
    () => undefined,
    { day: () => [0.2, 0.5, 2] },
    grid,
    { rayCount: 128 },
  );
  assert.equal(bake.active, 8);
  assert.equal(bake.relocated, 0);
  assert.equal(bake.traceCount, 8 * 128);
  assert.equal(bake.records.day.length, 8 * 40);
  for (let probe = 0; probe < 8; probe++)
    for (const n of [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
      [-1, 0, 0],
    ] as V3[])
      evaluateDiffuseSH9(coefficient(bake.records.day, probe), n).forEach(
        (v, c) => close(v, [0.2, 0.5, 2][c]),
      );
  assert.equal(bake.width, 30);
  assert.equal(bake.height, 30);
  assert.equal(bake.moments[(1 * 30 + 1) * 4], 64);
  assert.equal(bake.moments[(1 * 30 + 1) * 4 + 1], 4096);
});

test("solar and constant fog probe projection uses one geometry pass and preserves the primary record", () => {
  const single = bakeSkyProbes(
    () => undefined,
    { day: () => [1, 0.5, 0.25] },
    grid,
    { rayCount: 64 },
  );
  const split = bakeSkyProbes(
    () => undefined,
    { day: () => [1, 0.5, 0.25], constantFog: () => [3, 2, 1] },
    grid,
    { rayCount: 64 },
  );
  assert.deepEqual(split.records.day, single.records.day);
  assert.deepEqual(split.moments, single.moments);
  assert.equal(split.traceCount, single.traceCount);
  for (let p = 0; p < grid.count; p++) {
    assert.deepEqual(
      split.records.constantFog.subarray(p * 40, p * 40 + 4),
      split.records.day.subarray(p * 40, p * 40 + 4),
    );
    const value = evaluateDiffuseSH9(
      coefficient(split.records.constantFog, p),
      [0, 1, 0],
    );
    value.forEach((v, c) => close(v, [3, 2, 1][c], 0.03));
  }
});

test("a real geometric wall blocks directional sky and moments reject interpolation across it", () => {
  const trace: GiProbeTrace = (origin, direction, min, max) => {
    if (Math.abs(direction[0]) < 1e-10) return;
    const distance = -origin[0] / direction[0];
    if (distance < min || distance > max) return;
    return {
      distance,
      normal: origin[0] < 0 ? [-1, 0, 0] : [1, 0, 0],
      frontFace: true,
      maskedApproximation: false,
    };
  };
  const b = bakeSkyProbes(
    trace,
    { day: (d) => (d[0] > 0.2 ? [1, 1, 1] : [0, 0, 0]) },
    grid,
    { rayCount: 128 },
  );
  assert.equal(b.active, 8);
  assert.deepEqual([...coefficient(b.records.day, 0)], Array(36).fill(0));
  close(
    evaluateDiffuseSH9(coefficient(b.records.day, 1), [1, 0, 0])[0],
    0.96,
    0.05,
  );
  assert.ok(giMomentVisibility(1, 1.01, 2) < 0.001);
  assert.equal(giMomentVisibility(2, 4, 1), 1);
});

test("bounded relocation recovers a near-surface probe instead of making a black valid record", () => {
  const localGrid = {
    origin: [0, 0, 0] as V3,
    spacing: [1, 1, 1] as V3,
    dimensions: [2, 2, 2] as V3,
    count: 8,
  };
  const b = bakeSkyProbes(
    (origin) =>
      Math.hypot(...origin) < 0.2
        ? {
            distance: 0.2,
            normal: [0, 1, 0],
            frontFace: false,
            maskedApproximation: false,
          }
        : undefined,
    { sky: () => [1, 1, 1] },
    localGrid,
    { rayCount: 64 },
  );
  assert.equal(b.relocated, 1);
  assert.equal(b.active, 8);
  assert.equal(b.records.sky[3], 1);
  assert.ok(Math.hypot(...b.records.sky.subarray(0, 3)) > 0.2);
  assert.ok(Math.hypot(...b.records.sky.subarray(0, 3)) < 0.45);
  close(
    evaluateDiffuseSH9(coefficient(b.records.sky, 0), [0, 1, 0])[0],
    0.96,
    0.05,
  );
});

test("octahedral directions roundtrip and one-pixel tile borders use the folded neighbors", () => {
  for (const n of [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [0.2, -0.7, -0.4],
  ] as V3[]) {
    const original = normalize(n),
      uv = giOctEncode(original);
    close(dot(original, giOctDecode(...uv)), 1, 1e-12);
  }
  assert.throws(() => giOctEncode([0, 0, 0]));
  const b = bakeSkyProbes(
    (_o, d) => ({
      distance: 10 + d[0] * 3 + d[1] * 2,
      normal: [0, 1, 0],
      frontFace: true,
      maskedApproximation: false,
    }),
    { sky: () => [1, 1, 1] },
    grid,
    { rayCount: 64 },
  );
  const mean = (x: number, y: number) => b.moments[(y * b.width + x) * 4];
  for (let y = 1; y <= 8; y++) {
    assert.equal(mean(0, y), mean(1, 9 - y));
    assert.equal(mean(9, y), mean(8, 9 - y));
  }
  for (let x = 1; x <= 8; x++) {
    assert.equal(mean(x, 0), mean(9 - x, 1));
    assert.equal(mean(x, 9), mean(9 - x, 8));
  }
  assert.equal(mean(0, 0), mean(8, 8));
});

test("geometry identity ignores artifact links and JSON key order but tracks actual edits", () => {
  const source = {
    vertices: "v",
    indices: "i",
    vertexCount: 1,
    indexCount: 3,
    vertexStride: 48,
    materials: [{ name: "stone", alphaMode: "OPAQUE" }],
    meshes: [],
    instances: [],
    probesURL: "old",
  } as any;
  const first = giSceneIdentityPayload(source);
  assert.equal(
    first,
    giSceneIdentityPayload({
      ...source,
      probesURL: "new",
      decalsURL: "decal",
      materials: [{ alphaMode: "OPAQUE", name: "stone" }],
    }),
  );
  assert.notEqual(first, giSceneIdentityPayload({ ...source, vertexCount: 2 }));
  for (const normalConvention of ["opengl", "directx"])
    assert.equal(
      first,
      giSceneIdentityPayload({
        ...source,
        materials: [{ ...source.materials[0], normalConvention }],
      }),
    );
  for (const normalEncoding of ["rgb", "xy-positive-z"])
    assert.equal(
      first,
      giSceneIdentityPayload({
        ...source,
        materials: [{ ...source.materials[0], normalEncoding }],
      }),
    );
  for (const change of [
    { baseColorFactor: [0.5, 0.5, 0.5, 1] },
    { normalUrl: "new-normal" },
    { unknownFutureField: 1 },
  ])
    assert.notEqual(
      first,
      giSceneIdentityPayload({
        ...source,
        materials: [{ ...source.materials[0], ...change }],
      }),
    );
  assert.equal(
    giEnvironmentIdentityPayload({ url: "a", brightness: 10 } as any),
    giEnvironmentIdentityPayload({ brightness: 10, url: "a" } as any),
  );
});

test("allocation-light probe queries match the independent CPU contract under reflection and material overrides", () => {
  const vertices = new Float32Array(36);
  vertices.set([-1, -1, 0], 0);
  vertices.set([1, -1, 0], 12);
  vertices.set([0, 1, 0], 24);
  const scene = {
    vertexCount: 3,
    indexCount: 3,
    vertexStride: 48,
    materials: [
      { name: "stone", alphaMode: "OPAQUE" },
      { name: "leaves", alphaMode: "MASK" },
    ],
    meshes: [
      {
        id: "plane",
        name: "plane",
        primitives: [
          {
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
            material: 0,
            materialSlot: 0,
          },
        ],
      },
    ],
    instances: [
      {
        mesh: 0,
        model: modelMatrix([0, 0, -4]),
        materialOverrides: [],
        actor: "a",
        component: "a",
        sourceClass: "StaticMeshActor",
      },
      {
        mesh: 0,
        model: [-2, 0, 0, 0, 0.5, 2, 0, 0, 0, 0, 1, 0, 3, 0, -5, 1],
        materialOverrides: [1],
        actor: "b",
        component: "b",
        sourceClass: "StaticMeshActor",
      },
    ],
  } as any;
  const a = buildGiAcceleration(scene, vertices, new Uint32Array([0, 1, 2])),
    fast = createGiProbeTrace(a);
  for (let x = -1; x < 5; x += 0.15) {
    const origin: V3 = [x, 0.2, 0],
      direction: V3 = [0, 0, -1],
      expected = traceGi(a, origin, direction, 0.01, 64),
      actual = fast(origin, direction, 0.01, 64);
    assert.equal(!!actual, !!expected);
    if (actual && expected) {
      close(actual.distance, expected.distance, 1e-8);
      assert.equal(actual.frontFace, expected.frontFace);
      assert.equal(actual.maskedApproximation, expected.maskedApproximation);
      actual.normal.forEach((n, i) => close(n, expected.normal[i], 1e-8));
    }
  }
});

test("sky sampling retains raw radiance and oversized preparation fails before tracing", () => {
  const pixels = new Float32Array([0.2, 0.5, 2, 1, 0.2, 0.5, 2, 1]),
    sampler = giEnvironmentSampler(
      { width: 2, height: 1, pixels },
      modelMatrix([0, 0, 0]),
    );
  assert.deepEqual(sampler([0, 1, 0]), [0.2, 0.5, 2].map(Math.fround));
  assert.throws(() =>
    bakeSkyProbes(
      () => {
        throw new Error("must not trace");
      },
      { sky: () => [1, 1, 1] },
      { ...grid, dimensions: [64, 64, 64], count: 64 ** 3 },
    ),
  );
  assert.throws(() =>
    bakeSkyProbes(() => undefined, { sky: () => [-1, 0, 0] }, grid),
  );
});
