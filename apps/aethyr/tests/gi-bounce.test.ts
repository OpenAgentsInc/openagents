import assert from "node:assert/strict";
import { test } from "node:test";
import type {
  CompiledScene,
  SourceLight,
} from "../packages/assets/src/scene.ts";
import { buildGiAcceleration } from "../packages/renderer/gi-acceleration.ts";
import { createGiSurfaceTransport } from "../packages/renderer/gi-transport.ts";
import {
  bakeDiffuseBounce,
  giBounceLocalLightsIdentityPayload,
  giBounceMaterialIdentityPayload,
  giBounceCoverageIdentityPayload,
} from "../packages/renderer/gi-bounce.ts";
import { evaluateDiffuseSH9 } from "../packages/renderer/environment-lighting.ts";
import { modelMatrix } from "../packages/renderer/math.ts";

function setup() {
  const vertices = new Float32Array(36);
  [
    [-100, -100, 0],
    [100, -100, 0],
    [0, 100, 0],
  ].forEach((p, i) => vertices.set([...p, 0, 0, 1, 0, 0, 1, 0, 0, 1], i * 12));
  const scene = {
    vertexCount: 3,
    indexCount: 3,
    vertexStride: 48,
    materials: [
      {
        id: "red",
        name: "red",
        baseColorFactor: [0.8, 0.2, 0.1, 1],
        emissiveFactor: [0, 0, 0],
        roughness: 1,
        metallic: 0,
        normalScale: 0,
        uvScale: [1, 1],
        alphaMode: "OPAQUE",
        alphaCutoff: 0.5,
        doubleSided: false,
        packedChannels: "metallic-r-roughness-g-occlusion-b",
      },
    ],
    meshes: [
      {
        name: "red-wall",
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
        model: modelMatrix([0, 0, 0]),
        materialOverrides: [],
        actor: "wall",
        component: "wall",
        sourceClass: "StaticMeshActor",
      },
    ],
  } as unknown as CompiledScene;
  const a = buildGiAcceleration(scene, vertices, new Uint32Array([0, 1, 2])),
    sample = () => [1, 1, 1, 1] as [number, number, number, number],
    transport = createGiSurfaceTransport(
      a,
      scene,
      { vertices, colors: new Uint8Array(12).fill(255) },
      sample,
    ),
    baseRecords = new Float32Array(40);
  baseRecords.set([0, 0, 2, 1]);
  const light = {
    id: "warm",
    label: "warm",
    kind: "point",
    position: [0, 0, 2],
    direction: [0, 0, -1],
    color: [1, 0.4, 0.1],
    intensity: 100,
    intensityUnits: "candelas",
    range: 30,
    castShadows: true,
    enabled: true,
    inverseSquaredFalloff: true,
    useTemperature: false,
    temperatureKelvin: 6500,
    raw: {},
  } as SourceLight;
  const environment = {
    description: {
      url: "owned",
      width: 1,
      height: 1,
      rotation: Array.from(modelMatrix([0, 0, 0])),
      uSign: 1,
      uOffset: 0,
      vSign: 1,
      vOffset: 0,
      tint: [1, 1, 1],
      brightness: 1,
    },
    sample: () => [1, 1, 1] as [number, number, number],
  };
  return {
    scene,
    transport,
    sample,
    baseRecords,
    light,
    environment,
    bounds: a.worldBounds,
  };
}
test("one diffuse bounce carries actual red material color independently for sky, sun and warm locals", () => {
  const f = setup(),
    sun = { ...f.light, kind: "directional" as const },
    b = bakeDiffuseBounce({
      transport: f.transport,
      materials: f.scene.materials,
      sample: f.sample,
      baseRecords: f.baseRecords,
      environment: f.environment,
      lights: { sun, locals: [f.light] },
      bounds: f.bounds,
      rayCount: 128,
    });
  for (const layer of ["sky", "sun", "locals"] as const) {
    const value = evaluateDiffuseSH9(b.layers[layer], [0, 0, -1]);
    assert.ok(value[0] > 0.01);
    assert.ok(value[0] > value[1] * 3);
    assert.ok(value[1] > value[2]);
  }
  assert.ok(b.stats.primaryHits > 0);
  assert.ok(b.stats.localShadowRays > 0);
});
test("sun layer is white per lux while local strength and raw sky radiance retain separate linearity", () => {
  const run = (skyGain: number, localGain: number, sunGain: number) => {
    const f = setup();
    return bakeDiffuseBounce({
      transport: f.transport,
      materials: f.scene.materials,
      sample: f.sample,
      baseRecords: f.baseRecords,
      environment: {
        ...f.environment,
        sample: () => [skyGain, skyGain, skyGain],
      },
      lights: {
        sun: {
          ...f.light,
          kind: "directional",
          intensity: sunGain,
          color: [sunGain, 0.2, 1],
        },
        locals: [{ ...f.light, intensity: f.light.intensity * localGain }],
      },
      rayCount: 64,
    }).layers;
  };
  const a = run(1, 1, 1),
    b = run(2, 3, 100);
  for (let i = 0; i < 36; i++) {
    assert.ok(Math.abs(a.sky[i] * 2 - b.sky[i]) < 1e-5);
    assert.ok(Math.abs(a.locals[i] * 3 - b.locals[i]) < 1e-5);
    assert.equal(a.sun[i], b.sun[i]);
  }
});
test("invalid probes and zero-albedo metals contribute no invented fill", () => {
  const f = setup();
  f.scene.materials[0].metallic = 1;
  const b = bakeDiffuseBounce({
    transport: f.transport,
    materials: f.scene.materials,
    sample: f.sample,
    baseRecords: f.baseRecords,
    environment: f.environment,
    lights: { sun: null, locals: [f.light] },
    rayCount: 32,
  });
  assert.ok(Object.values(b.layers).every((v) => v.every((n) => n === 0)));
  f.baseRecords[3] = 0;
  assert.equal(
    bakeDiffuseBounce({
      transport: f.transport,
      materials: f.scene.materials,
      sample: f.sample,
      baseRecords: f.baseRecords,
      environment: f.environment,
      lights: { sun: null, locals: [] },
      rayCount: 32,
    }).stats.activeProbes,
    0,
  );
  assert.throws(
    () =>
      bakeDiffuseBounce({
        transport: f.transport,
        materials: f.scene.materials,
        sample: f.sample,
        baseRecords: f.baseRecords,
        environment: f.environment,
        lights: { sun: null, locals: [] },
        rayCount: 1,
      }),
    /bounded/,
  );
});
test("worker descriptor identities separate coverage, material and enabled local-light edits", () => {
  const f = setup(),
    scene = {
      ...f.scene,
      vertices: "v",
      vertexColors: "colors",
      vertexUV1: "uv1",
    },
    material = giBounceMaterialIdentityPayload(scene),
    coverage = giBounceCoverageIdentityPayload(scene);
  scene.materials[0].normalConvention = "directx";
  scene.materials[0].normalEncoding = "xy-positive-z";
  assert.equal(giBounceMaterialIdentityPayload(scene), material);
  assert.equal(giBounceCoverageIdentityPayload(scene), coverage);
  scene.materials[0].baseColorFactor[0] = 0.3;
  assert.notEqual(giBounceMaterialIdentityPayload(scene), material);
  assert.equal(giBounceCoverageIdentityPayload(scene), coverage);
  scene.vertexUV1 = "new-uv1";
  assert.notEqual(giBounceMaterialIdentityPayload(scene), material);
  assert.equal(giBounceCoverageIdentityPayload(scene), coverage);
  scene.materials[0].layers = { model: "ue-mm-blend-2-v1" } as any;
  assert.notEqual(giBounceCoverageIdentityPayload(scene), coverage);
  const local = giBounceLocalLightsIdentityPayload([f.light]);
  assert.equal(
    local,
    giBounceLocalLightsIdentityPayload([
      { ...f.light, raw: {}, color: [1, 0.4, 0.1] },
    ]),
  );
  assert.equal(
    local,
    giBounceLocalLightsIdentityPayload([
      { ...f.light, enabled: false, id: "hidden" },
      f.light,
      { ...f.light, kind: "directional" },
    ]),
  );
  assert.notEqual(
    local,
    giBounceLocalLightsIdentityPayload([{ ...f.light, intensity: 101 }]),
  );
});

test("constant fog bounce stays separate and shares the exact material hits and visibility rays", () => {
  const run = (separate: boolean) => {
    const f = setup();
    return bakeDiffuseBounce({
      transport: f.transport,
      materials: f.scene.materials,
      sample: f.sample,
      baseRecords: f.baseRecords,
      environment: {
        ...f.environment,
        sample: () => (separate ? [1, 0.5, 0.25] : [4, 2.5, 1.25]),
        ...(separate
          ? { constantFogSample: () => [3, 2, 1] as [number, number, number] }
          : {}),
      },
      lights: { sun: null, locals: [] },
      rayCount: 64,
      skyRayCount: 4,
    });
  };
  const split = run(true),
    combined = run(false);
  assert.ok(split.constantFog);
  assert.equal(combined.constantFog, undefined);
  for (let i = 0; i < 36; i++)
    assert.ok(
      Math.abs(
        split.layers.sky[i] + split.constantFog[i] - combined.layers.sky[i],
      ) < 1e-6,
    );
  assert.equal(split.stats.skyShadowRays, combined.stats.skyShadowRays);
  assert.equal(split.stats.primaryHits, combined.stats.primaryHits);
  assert.equal(
    split.stats.materialEvaluations,
    combined.stats.materialEvaluations,
  );
  assert.ok(split.constantFog.some((v) => v !== 0));
  const f = setup();
  const constantOnly = bakeDiffuseBounce({
    transport: f.transport,
    materials: f.scene.materials,
    sample: f.sample,
    baseRecords: f.baseRecords,
    environment: {
      ...f.environment,
      sample: () => {
        throw new Error("cached primary must not sample");
      },
      constantFogSample: () => [3, 2, 1],
    },
    layers: [],
    lights: { sun: null, locals: [] },
    rayCount: 64,
    skyRayCount: 4,
  });
  assert.deepEqual(constantOnly.constantFog, split.constantFog);
  assert.ok(
    Object.values(constantOnly.layers).every((layer) =>
      layer.every((v) => v === 0),
    ),
  );
  assert.throws(
    () =>
      bakeDiffuseBounce({
        transport: f.transport,
        materials: f.scene.materials,
        sample: f.sample,
        baseRecords: f.baseRecords,
        environment: { ...f.environment, constantFogSample: () => [NaN, 0, 0] },
        lights: { sun: null, locals: [] },
        rayCount: 32,
      }),
    /bounded raw/,
  );
});
