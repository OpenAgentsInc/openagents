import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledMaterial } from "../packages/assets/src/scene.ts";
import {
  buildGiAcceleration,
  giAffectedBounds,
  giBinaryLE,
  giMaterialTransport,
  GiTransport,
  GI_LEAF_BIT,
  traceGi,
} from "../packages/renderer/gi-acceleration.ts";
import {
  cross,
  dot,
  modelMatrix,
  normalize,
  sub,
  transformPoint,
  type V3,
} from "../packages/renderer/math.ts";

const material = (
  name = "stone",
  alphaMode: CompiledMaterial["alphaMode"] = "OPAQUE",
): CompiledMaterial => ({
  id: name,
  name,
  alphaMode,
  baseColorFactor: [1, 1, 1, 1],
  emissiveFactor: [0, 0, 0],
  roughness: 0.5,
  metallic: 0,
  normalScale: 1,
  uvScale: [1, 1],
  alphaCutoff: 0.5,
  doubleSided: false,
  packedChannels: "metallic-r-roughness-g-occlusion-b",
});
function fixture() {
  const points: V3[] = [
      [-1, -1, 0],
      [1, -1, 0],
      [1, 1, 0],
      [-1, 1, 0],
    ],
    vertices = new Float32Array(48);
  points.forEach((p, i) => vertices.set(p, i * 12));
  const scene = {
    vertexCount: 4,
    indexCount: 6,
    vertexStride: 48 as const,
    materials: [
      material(),
      material("leaves", "MASK"),
      material("glass", "BLEND"),
    ],
    meshes: [
      {
        id: "plane",
        name: "Owned plane",
        bounds: { min: [-1, -1, 0], max: [1, 1, 0] },
        primitives: [
          {
            firstIndex: 0,
            indexCount: 6,
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
        materialOverrides: [] as number[],
        actor: "wall",
        component: "mesh",
        sourceClass: "StaticMeshActor",
      },
    ],
  };
  return {
    scene,
    vertices,
    indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
    points,
  };
}
function close(actual: number, expected: number, tolerance = 1e-5): void {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} differs from ${expected}`,
  );
}

test("shared mesh BLAS and reordered TLAS preserve world hit distance and stable source identities", () => {
  const { scene, vertices, indices } = fixture();
  scene.instances.push({
    ...scene.instances[0],
    model: modelMatrix([0, 0, -8]),
    actor: "second",
  });
  const acceleration = buildGiAcceleration(scene, vertices, indices);
  assert.equal(acceleration.stats.uniqueTriangles, 2);
  assert.equal(acceleration.stats.uniqueMeshes, 1);
  assert.equal(acceleration.stats.admittedInstances, 2);
  const hit = traceGi(acceleration, [0.25, 0, 0], [0, 0, -2]);
  assert.ok(hit);
  close(hit.distance, 4);
  assert.equal(hit.sourceInstance, 0);
  assert.equal(hit.sourceMesh, 0);
  assert.equal(hit.material, 0);
  assert.equal(hit.maskedApproximation, false);
  assert.equal(hit.frontFace, true);
  close(
    hit.barycentric.reduce((a, b) => a + b),
    1,
  );
  assert.equal(traceGi(acceleration, [5, 5, 0], [0, 0, -1]), undefined);
  assert.equal(traceGi(acceleration, [0, 0, 0], [1, 0, 0]), undefined);
  assert.equal(
    traceGi(acceleration, [0, 0, 0], [0, 0, -1], 0.001, 3),
    undefined,
  );
  const words = new Uint32Array(acceleration.blasNodes);
  assert.equal(words[7] >>> 31, 1);
  assert.equal(words[7] & ~GI_LEAF_BIT, 2);
});

test("nonuniform scale,reflection and shear retain meter distances and transformed winding", () => {
  const { scene, vertices, indices } = fixture();
  scene.instances[0].model = [
    -3, 0, 0, 0, 0.5, 2, 0, 0, 0.2, 0.3, 2, 0, 0, 0, -6, 1,
  ];
  const hit = traceGi(
    buildGiAcceleration(scene, vertices, indices),
    [0.1, 0.1, 0],
    [0, 0, -1],
  );
  assert.ok(hit);
  close(hit.distance, 6);
  close(hit.normal[2], -1);
  assert.equal(hit.frontFace, false);
});

test("transport excludes skies,fog,clouds and water without excluding wooden water wheels", () => {
  const { scene, vertices, indices } = fixture();
  scene.instances.push({
    ...scene.instances[0],
    model: modelMatrix([0, 0, -1]),
    sourceClass: "FX_fog_Card_C",
  });
  scene.materials.push(
    material("MI_water_01"),
    material("MI_watermill_wood_02"),
  );
  assert.equal(giMaterialTransport(scene.materials[3]), GiTransport.excluded);
  assert.equal(giMaterialTransport(scene.materials[4]), GiTransport.opaque);
  scene.instances.push({
    ...scene.instances[0],
    materialOverrides: [3],
    model: modelMatrix([0, 0, -2]),
  });
  scene.instances.push({
    ...scene.instances[0],
    materialOverrides: [4],
    model: modelMatrix([0, 0, -3]),
  });
  const a = buildGiAcceleration(scene, vertices, indices);
  assert.equal(a.stats.excludedInstances, 2);
  const hit = traceGi(a, [0, 0, 0], [0, 0, -1]);
  assert.ok(hit);
  assert.equal(hit.sourceInstance, 3);
  assert.equal(hit.material, 4);
  close(hit.distance, 3);
});

test("masked transport is declared solid approximation and blend overrides do not occlude", () => {
  const { scene, vertices, indices } = fixture();
  scene.instances[0].materialOverrides = [1];
  scene.instances.push({
    ...scene.instances[0],
    materialOverrides: [2],
    model: modelMatrix([0, 0, -1]),
  });
  const a = buildGiAcceleration(scene, vertices, indices),
    hit = traceGi(a, [0, 0, 0], [0, 0, -1]);
  assert.ok(hit);
  close(hit.distance, 4);
  assert.equal(hit.maskedApproximation, true);
  assert.equal(hit.material, 1);
  scene.instances[0].materialOverrides = [-1];
  assert.equal(
    traceGi(
      buildGiAcceleration(scene, vertices, indices),
      [0, 0, 0],
      [0, 0, -1],
    )?.material,
    0,
  );
});

/** Independent world-space plane/edge reference,without inverse transforms or a hierarchy. */
function brute(
  f: ReturnType<typeof fixture>,
  origin: V3,
  direction: V3,
): number | undefined {
  let nearest = 1000;
  let found = false;
  for (const instance of f.scene.instances)
    for (let t = 0; t < f.indices.length; t += 3) {
      const [a, b, c] = [0, 1, 2].map((j) =>
        transformPoint(
          instance.model.map(Math.fround),
          f.points[f.indices[t + j]],
        ),
      );
      const normal = cross(sub(b, a), sub(c, a)),
        denom = dot(normal, direction);
      if (Math.abs(denom) < 1e-10) continue;
      const distance = dot(normal, sub(a, origin)) / denom;
      if (distance < 0.001 || distance > nearest) continue;
      const p = origin.map((v, i) => v + direction[i] * distance) as V3;
      if (
        [
          [a, b],
          [b, c],
          [c, a],
        ].some(
          ([start, end]) =>
            dot(cross(sub(end, start), sub(p, start)), normal) < -1e-8,
        )
      )
        continue;
      found = true;
      nearest = distance;
    }
  return found ? nearest : undefined;
}
test("deterministic hierarchies agree with independent world-space intersections across many instances", () => {
  const f = fixture();
  f.scene.instances = [];
  for (let i = 0; i < 24; i++) {
    const m = modelMatrix(
      [((i % 6) - 2.5) * 2, (Math.floor(i / 6) - 1.5) * 2, -4 - i * 0.05],
      i * 0.13,
      1.2,
    );
    m[4] += 0.25;
    f.scene.instances.push({
      mesh: 0,
      model: m,
      materialOverrides: [] as number[],
      actor: `wall${i}`,
      component: "mesh",
      sourceClass: "StaticMeshActor",
    });
  }
  const a = buildGiAcceleration(f.scene, f.vertices, f.indices),
    b = buildGiAcceleration(f.scene, f.vertices, f.indices);
  for (const key of [
    "blasNodes",
    "tlasNodes",
    "instances",
    "triangles",
  ] as const)
    assert.deepEqual(giBinaryLE(a[key]), giBinaryLE(b[key]));
  assert.ok(a.stats.maxDepth <= 32);
  let seed = 17,
    hits = 0;
  const rand = () =>
    (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 0x100000000;
  for (let i = 0; i < 200; i++) {
    const origin: V3 = [rand() * 14 - 7, rand() * 10 - 5, 2],
      direction = normalize([rand() * 0.2 - 0.1, rand() * 0.2 - 0.1, -1]);
    const expected = brute(f, origin, direction),
      actual = traceGi(a, origin, direction);
    assert.equal(!!actual, expected !== undefined);
    if (actual) {
      hits++;
      close(actual.distance, expected!, 5e-5);
    }
  }
  assert.ok(hits > 30);
});

test("empty transport remains sky-visible and malformed geometry fails before a hierarchy is published", () => {
  const f = fixture();
  f.scene.instances[0].materialOverrides = [2];
  const empty = buildGiAcceleration(f.scene, f.vertices, f.indices);
  assert.equal(empty.stats.admittedInstances, 0);
  assert.equal(traceGi(empty, [0, 0, 0], [0, 0, -1]), undefined);
  const invalid = fixture();
  invalid.indices[2] = 99;
  assert.throws(() =>
    buildGiAcceleration(invalid.scene, invalid.vertices, invalid.indices),
  );
  invalid.indices[2] = 2;
  invalid.scene.instances[0].model[0] = 0;
  assert.throws(() =>
    buildGiAcceleration(invalid.scene, invalid.vertices, invalid.indices),
  );
  const nonfinite = fixture();
  nonfinite.vertices[0] = Number.NaN;
  assert.throws(() =>
    buildGiAcceleration(nonfinite.scene, nonfinite.vertices, nonfinite.indices),
  );
});

test("geometry invalidation covers old and new walls plus bounded ray reach", () => {
  assert.deepEqual(
    giAffectedBounds(
      { min: [0, 0, 0], max: [1, 2, 3] },
      { min: [10, 0, 0], max: [11, 2, 3] },
      4,
    ),
    { min: [-4, -4, -4], max: [15, 6, 7] },
  );
  assert.throws(() => giAffectedBounds(undefined, undefined));
  assert.throws(() =>
    giAffectedBounds({ min: [0, 0, 0], max: [1, 1, 1] }, undefined, Infinity),
  );
});
