import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import {
  buildGiAcceleration,
  traceGi,
} from "../packages/renderer/gi-acceleration.ts";
import { createGiProbeTrace } from "../packages/renderer/gi-probes.ts";
import { createGiSurfaceTransport } from "../packages/renderer/gi-transport.ts";
import { modelMatrix } from "../packages/renderer/math.ts";

function fixture(alpha = 0) {
  const vertices = new Float32Array(72);
  for (const [i, z] of [1, 0].entries()) {
    for (const [c, p] of [
      [-10, -10, z],
      [10, -10, z],
      [0, 10, z],
    ].entries()) {
      vertices.set(
        [...p, 0, 0, 1, c === 1 ? 1 : 0, c === 2 ? 1 : 0, 1, 0, 0, 1],
        (i * 3 + c) * 12,
      );
    }
  }
  const material = (name: string) => ({
    id: name,
    name,
    baseColorFactor: [1, 1, 1, 1],
    emissiveFactor: [0, 0, 0],
    roughness: 1,
    metallic: 0,
    normalScale: 0,
    uvScale: [1, 1],
    alphaMode: "OPAQUE",
    alphaCutoff: 0.5,
    doubleSided: false,
    packedChannels: "metallic-r-roughness-g-occlusion-b",
  });
  const scene = {
    vertexCount: 6,
    indexCount: 6,
    vertexStride: 48,
    materials: [
      { ...material("card"), alphaMode: "MASK", baseColorUrl: "mask" },
      material("wall"),
    ],
    meshes: [
      {
        name: "two-planes",
        primitives: [
          {
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
            material: 0,
            materialSlot: 0,
          },
          {
            firstIndex: 3,
            indexCount: 3,
            baseVertex: 0,
            material: 1,
            materialSlot: 1,
          },
        ],
      },
    ],
    instances: [
      {
        mesh: 0,
        model: modelMatrix([0, 0, 0]),
        materialOverrides: [],
        actor: "wall",
        component: "wall",
        sourceClass: "StaticMeshActor",
      },
    ],
  } as unknown as CompiledScene;
  const indices = new Uint32Array([0, 1, 2, 3, 4, 5]),
    colors = new Uint8Array(24).fill(255),
    uv1 = new Float32Array(12);
  uv1.set([0, 0, 2, 0, 0, 2, 0, 0, 2, 0, 0, 2]);
  const acceleration = buildGiAcceleration(scene, vertices, indices);
  const sample = () =>
    [0.5, 0.5, 0.5, alpha] as [number, number, number, number];
  return { scene, vertices, colors, uv1, acceleration, sample };
}

test("alpha rejection continues to the real nearest opaque surface with barycentric attributes", () => {
  const f = fixture(),
    transport = createGiSurfaceTransport(f.acceleration, f.scene, f, f.sample);
  const hit = transport.trace([0, 0, 2], [0, 0, -1], 0.001, 64)!;
  assert.equal(hit.material, 1);
  assert.equal(hit.distance, 2);
  assert.equal(hit.sourceTriangle, 1);
  assert.deepEqual(hit.barycentric, [0.25, 0.25, 0.5]);
  assert.deepEqual(hit.context.uv, [0.25, 0.5]);
  assert.deepEqual(hit.context.uv1, [0.5, 1]);
  assert.deepEqual(hit.context.vertexColor, [1, 1, 1, 1]);
  assert.deepEqual(hit.context.geometricNormal, [0, 0, 1]);
  assert.ok(transport.stats.transparentHits >= 1);
  assert.equal(transport.occluded([0, 0, 2], [0, 0, -1], 0.001, 1.5), false);
  assert.equal(transport.occluded([0, 0, 2], [0, 0, -1], 0.001, 3), true);
});
test("a source alpha-covered card blocks both surface and shadow queries", () => {
  const f = fixture(1),
    transport = createGiSurfaceTransport(f.acceleration, f.scene, f, f.sample),
    hit = transport.trace([0, 0, 2], [0, 0, -1], 0.001, 64)!;
  assert.equal(hit.material, 0);
  assert.equal(hit.distance, 1);
  assert.equal(hit.maskedApproximation, true);
  assert.equal(transport.occluded([0, 0, 2], [0, 0, -1], 0.001, 1.5), true);
});
test("geometry reflection preserves hit identity and faces the bounce normal toward the ray", () => {
  const f = fixture();
  f.scene.instances[0].model = [
    -2, 0, 0, 0, 0.5, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ];
  const a = buildGiAcceleration(
      f.scene,
      f.vertices,
      new Uint32Array([0, 1, 2, 3, 4, 5]),
    ),
    transport = createGiSurfaceTransport(a, f.scene, f, f.sample),
    h = transport.trace([0, 0, 2], [0, 0, -1], 0.001, 64)!;
  assert.equal(h.frontFace, false);
  assert.ok(
    Math.hypot(
      h.context.geometricNormal[0],
      h.context.geometricNormal[1],
      h.context.geometricNormal[2] - 1,
    ) < 1e-12,
  );
  assert.equal(h.sourceInstance, 0);
  assert.equal(h.sourceMesh, 0);
});
test("query, alpha and time exhaustion fail explicitly instead of accepting an incomplete field", () => {
  const f = fixture(),
    budget = createGiSurfaceTransport(f.acceleration, f.scene, f, f.sample, {
      maxQueries: 1,
    });
  budget.trace([0, 0, 2], [0, 0, -1], 0.001, 64);
  assert.throws(() => budget.trace([0, 0, 2], [0, 0, -1], 0.001, 64), /budget/);
  const old = createGiSurfaceTransport(f.acceleration, f.scene, f, f.sample, {
    startedAt: 0,
    maxMilliseconds: 1,
  });
  assert.throws(() => old.trace([0, 0, 2], [0, 0, -1], 0.001, 64), /budget/);
  assert.throws(
    () =>
      createGiSurfaceTransport(
        f.acceleration,
        f.scene,
        { ...f, colors: new Uint8Array(1) },
        f.sample,
      ),
    /attributes/,
  );
  assert.throws(
    () =>
      createGiSurfaceTransport(f.acceleration, f.scene, f, f.sample, {
        startedAt: NaN,
      }),
    /start time/,
  );
  assert.throws(
    () => budget.occluded([0, 0, 0], [0, 0, 0], 0, 1),
    /normalized/,
  );
});
test("equivalent tiny local triangles in large instances preserve world intersections", () => {
  const f = fixture();
  for (let v = 0; v < 6; v++) {
    f.vertices[v * 12] *= 1e-5;
    f.vertices[v * 12 + 1] *= 1e-5;
  }
  f.scene.instances[0].model = modelMatrix([0, 0, 0], 0, 1e6);
  const a = buildGiAcceleration(
      f.scene,
      f.vertices,
      new Uint32Array([0, 1, 2, 3, 4, 5]),
    ),
    transport = createGiSurfaceTransport(a, f.scene, f, f.sample);
  const hit = transport.trace([25, 25, 1], [0, 0, -1], 0.001, 64)!;
  assert.ok(Math.abs(hit.distance - 1) < 1e-8);
  assert.ok(
    Math.abs(traceGi(a, [25, 25, 1], [0, 0, -1], 0.001, 64)!.distance - 1) <
      1e-8,
  );
  assert.ok(
    Math.abs(
      createGiProbeTrace(a)([25, 25, 1], [0, 0, -1], 0.001, 64)!.distance - 1,
    ) < 1e-8,
  );
  assert.equal(transport.occluded([25, 25, 1], [0, 0, -1], 0.001, 64), true);
});
