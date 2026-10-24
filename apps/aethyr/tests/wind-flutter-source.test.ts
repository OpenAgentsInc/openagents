import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import {
  prepareSourceWind,
  sourceWindFlutterMaterial,
  validateSourceWindBuffers,
  type SourceWindManifest,
} from "../packages/renderer/wind-source.ts";
import { WIND_FLUTTER_CLOCK_SCALE } from "../packages/renderer/wind-flutter.ts";

const hash = (bytes: ArrayBuffer) =>
  createHash("sha256").update(new Uint8Array(bytes)).digest("hex");
async function fixture() {
  const vertices = new Float32Array(36);
  vertices[12] = 1;
  vertices[25] = 2;
  const indices = new Uint32Array([0, 1, 2]),
    colors = new Uint8Array([
      255, 0, 0, 255, 255, 255, 0, 255, 255, 128, 0, 255,
    ]),
    color = new ArrayBuffer(20),
    view = new DataView(color);
  view.setUint32(0, 0x41465238, false);
  for (const offset of [4, 8, 12]) view.setUint32(offset, 1, true);
  new Uint8Array(color, 16).set([51, 102, 153, 255]);
  const sourceHash =
    "07d7e27f4d04a38a2e68fa6a49409a62ac293be80ec6c6b74534878e24ad06ab";
  const flutter = {
    profile: "source-triangle-flutter-v1" as const,
    intensity: 0.15,
    weight: 0.3,
    clockScale: WIND_FLUTTER_CLOCK_SCALE,
    smallWaveCm: 1 as const,
    largeWaveCm: 200 as const,
    vertexLod: 0 as const,
    baseColorUrl: "/private-assets/leaf.bin",
    sourceGraphSha256: sourceHash,
    sourceFunctionSha256:
      "67202b42b32d5d4181432ad134f08dfc0182a01f047e2e32d8bbda55fbbd3c9d",
    sourceWindFunctionSha256:
      "0071a94091d98da36f4bbc0f231e36c0211a1af625164cc194bcf2f1ec1ae5a5",
  };
  const model = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 3, 0, 0, 1];
  const scene = {
    scenario: "day",
    vertices: "v",
    indices: "i",
    vertexColors: "c",
    vertexCount: 3,
    indexCount: 3,
    materials: [
      {
        id: "leaf",
        vegetation: {
          baseColorUrl: flutter.baseColorUrl,
          sourceWindEnabled: true,
          source: {
            master:
              "/Game/Medieval_Town/Materials/Masters/MM_vegetation_01.MM_vegetation_01",
            graphSha256: sourceHash,
            parameters: {
              switches: { "Use Wind?": true },
              scalars: {
                "wind speed": 0.5,
                "sway speed": 0.2,
                "sway amount": 0.005,
                "wind strenght": 0.15,
                "wind weight": 0.3,
              },
            },
          },
        },
      },
    ],
    meshes: [{ id: "mesh", bounds: { min: [0, 0, 0], max: [1, 2, 0] } }],
    instances: [{ actor: "owned tree", component: "leaves", mesh: 0, model }],
  } as unknown as CompiledScene;
  const manifest: SourceWindManifest = {
    schema: "aethyrforge.source-wind-sway.v1",
    scenario: "day",
    sourceBoundsSha256: "a".repeat(64),
    source: {
      vertices: "v",
      indices: "i",
      vertexColors: "c",
      vertexCount: 3,
      indexCount: 3,
      instances: 1,
    },
    records: [
      {
        instance: 0,
        actor: "owned tree",
        component: "leaves",
        model,
        radiusMeters: 2,
        pivot: [3, 0, 0],
        worldUp: [0, 1, 0],
        materials: [
          {
            id: "leaf",
            windSpeed: 0.5,
            swaySpeed: 0.2,
            swayAmount: 0.005,
            flutter,
          },
        ],
        treeEvidence: {
          profile: "source-tree-flutter-evidence-v1",
          factsSha256: "b".repeat(64),
          sourceMapSha256: "c".repeat(64),
          sourceMaterialsSha256: "d".repeat(64),
          meshId: "mesh",
          meshSphereRadiusCm: 200,
          componentSphereRadiusCm: 200,
          uniformScale: 1,
          boundsScale: 1,
          materialId: "leaf",
          baseColorSha256: hash(color),
          verticesSha256: hash(vertices.buffer),
          indicesSha256: hash(indices.buffer),
          vertexColorsSha256: hash(colors.buffer),
          bounds: {
            maxPivotDistanceMeters: 2,
            maxVertexGreen: 1,
            swayMeters: 0.02,
            flutterMeters: 0.009,
            additionalMeters: Math.sqrt(3) * 0.01,
            combinedMeters: 0.029 + Math.sqrt(3) * 0.01,
          },
        },
      },
    ],
  };
  const draws = [
    {
      material: 0,
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      firstInstance: 0,
      count: 1,
    },
  ];
  return { vertices, indices, colors, color, scene, manifest, draws };
}

test("qualified flutter expands indexed bounds while preserving ordinary parameters", async () => {
  const f = await fixture(),
    actual = prepareSourceWind(
      f.scene,
      f.draws,
      f.vertices,
      f.indices,
      f.colors,
      f.manifest,
    );
  assert.equal(actual.moving[0], 1);
  assert.ok(
    Math.abs(actual.padding[0] - (0.029 + Math.sqrt(3) * 0.01)) < 1e-12,
  );
  assert.equal(
    sourceWindFlutterMaterial({ day: f.manifest }, f.scene.materials[0])
      ?.clockScale,
    WIND_FLUTTER_CLOCK_SCALE,
  );
  const ordinary = structuredClone(f.manifest);
  delete ordinary.records[0].materials[0].flutter;
  delete ordinary.records[0].treeEvidence;
  const old = prepareSourceWind(
    f.scene,
    f.draws,
    f.vertices,
    f.indices,
    f.colors,
    ordinary,
  );
  assert.deepEqual(actual.parameters, old.parameters);
  assert.equal(old.padding[0], 0.02);
  assert.equal(
    sourceWindFlutterMaterial({ day: ordinary }, f.scene.materials[0]),
    undefined,
  );
  assert.throws(
    () =>
      sourceWindFlutterMaterial(
        { day: f.manifest, night: ordinary },
        f.scene.materials[0],
      ),
    /different flutter settings/,
  );
  for (const change of [
    (m: SourceWindManifest) => {
      m.records[0].treeEvidence!.meshSphereRadiusCm = 100;
    },
    (m: SourceWindManifest) => {
      m.records[0].treeEvidence!.bounds.combinedMeters = 0;
    },
    (m: SourceWindManifest) => {
      m.records[0].materials[0].flutter!.clockScale = 0.3;
    },
    (m: SourceWindManifest) => {
      m.records[0].materials[0].flutter!.sourceFunctionSha256 = "0".repeat(64);
    },
  ]) {
    const bad = structuredClone(f.manifest);
    change(bad);
    assert.throws(() =>
      prepareSourceWind(f.scene, f.draws, f.vertices, f.indices, f.colors, bad),
    );
  }
});

test("optional flutter checks actual input bytes and never fetches for old records", async () => {
  const f = await fixture(),
    saved = globalThis.fetch;
  let requests = 0;
  globalThis.fetch = async () => {
    requests++;
    return new Response(f.color.slice(0));
  };
  const buffers = {
    vertices: f.vertices.buffer,
    indices: f.indices.buffer,
    vertexColors: f.colors.buffer,
  };
  try {
    await validateSourceWindBuffers(
      { day: f.manifest, night: f.manifest },
      buffers,
    );
    assert.equal(requests, 1);
    const bad = structuredClone(f.manifest);
    bad.records[0].treeEvidence!.verticesSha256 = "0".repeat(64);
    await assert.rejects(
      validateSourceWindBuffers({ day: bad }, buffers),
      /geometry/,
    );
    bad.records[0].treeEvidence!.verticesSha256 = hash(f.vertices.buffer);
    bad.records[0].treeEvidence!.baseColorSha256 = "0".repeat(64);
    await assert.rejects(
      validateSourceWindBuffers({ day: bad }, buffers),
      /color map changed/,
    );
    const ordinary = structuredClone(f.manifest);
    delete ordinary.records[0].materials[0].flutter;
    requests = 0;
    await validateSourceWindBuffers({ day: ordinary }, buffers);
    assert.equal(requests, 0);
  } finally {
    globalThis.fetch = saved;
  }
});
