import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SourceWaterStreams,
  validateSourceWaterStreams,
  waterNoiseMips,
  type SourceWaterStreamsManifest,
} from "../packages/renderer/source-water-streams.ts";

const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
function fixture(): SourceWaterStreamsManifest {
  const blob = (name: string, byteLength: number) => ({
    url: "/private-assets/" + name + ".bin",
    sha256: "a".repeat(64),
    byteLength,
  });
  return {
    schema: "aethyrforge.source-water-streams.v1",
    vertices: blob("vertices", 4 * 32),
    indices: blob("indices", 6 * 4),
    vertexCount: 4,
    indexCount: 6,
    meshes: [
      { id: "owned", firstIndex: 0, indexCount: 6, center: [0, 0, 0.5] },
    ],
    noise: { ...blob("noise", 20), width: 1, height: 1, colorSpace: "linear" },
    material: {
      flow: [0, -2],
      powerExponent: 0.3,
      fresnelExponent: 1,
      baseReflectFraction: 0.04,
      depthFadeMeters: 0.25,
      refractionMinimum: 1,
    },
    scenarios: {
      day: [
        {
          id: "original",
          meshId: "owned",
          model: identity,
          parameters: [2, 0.35, 0, 0],
        },
      ],
      night: [],
    },
  };
}
test("persistent source stream contract retains separate scenario placements and source IOR", () => {
  const recipe = fixture();
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  assert.equal(recipe.scenarios.night.length, 0);
  assert.deepEqual(
    [recipe.material.refractionMinimum, recipe.scenarios.day[0].parameters[0]],
    [1, 2],
  );
});
test("source displacement is an explicit bounded profile and omission preserves legacy recipes", () => {
  const recipe = fixture();
  assert.equal(recipe.material.refractionMode, undefined);
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  recipe.material.refractionMode = "source-screen-displacement-v1";
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  (recipe.material as unknown as { refractionMode: string }).refractionMode =
    "arbitrary-offset";
  assert.throws(() => validateSourceWaterStreams(recipe), /profile/);
});
test("source-energy hybrid requires explicit continuous normals and excludes physical-lighting knobs", () => {
  const recipe = fixture();
  recipe.vertexStride = 48;
  recipe.vertices.byteLength = recipe.vertexCount * 48;
  recipe.normal = {
    ...recipe.noise,
    encoding: "unreal-normal-rgb",
    flipGreenChannel: false,
  };
  recipe.material.appearanceProfile = "owned-source-emissive-stream-v1";
  recipe.material.refractionMode = "source-screen-displacement-v1";
  recipe.material.emissiveWater = {
    normalStrength: 0.3,
    normalTiling: 1,
    geometryAmplitudeMeters: 0.004,
    microAmplitudeMeters: 0.0015,
  };
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  for (const mutate of [
    (m: SourceWaterStreamsManifest) => {
      delete m.normal;
    },
    (m: SourceWaterStreamsManifest) => {
      m.normal!.encoding = "unreal-normal-red-splat";
    },
    (m: SourceWaterStreamsManifest) => {
      m.normal!.coverage = "bilinear-positive-area-8x8-v1";
    },
    (m: SourceWaterStreamsManifest) => {
      delete m.material.refractionMode;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.emissiveWater!.geometryAmplitudeMeters = 0.1;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.emissiveWater!.normalTiling = 0;
    },
    (m: SourceWaterStreamsManifest) => {
      (
        m.material.emissiveWater as unknown as Record<string, number>
      ).emissionGain = 2;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.turbulence = {
        geometryAmplitudeMeters: 0,
        microAmplitudeMeters: 0,
        airScatteringPerMeter: 20,
        airAlbedo: 1,
      };
    },
    (m: SourceWaterStreamsManifest) => {
      delete m.material.appearanceProfile;
    },
  ]) {
    const candidate = structuredClone(recipe);
    mutate(candidate);
    assert.throws(() => validateSourceWaterStreams(candidate));
  }
});
test("owned clear water is optional, bounded and cannot silently reinterpret source normals", () => {
  const recipe = fixture();
  recipe.material.appearanceProfile = "owned-clear-stream-v1";
  recipe.material.clearWater = {
    ior: 1.333,
    roughness: 0.08,
    normalStrength: 0.3,
    thicknessMeters: 0.015,
    absorption: [0.08, 0.025, 0.01],
  };
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  for (const mutate of [
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.ior = 0.9;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.roughness = 0;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.normalStrength = Infinity;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.thicknessMeters = 1;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.normalTiling = 0;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.clearWater!.absorption[0] = -1;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.refractionMode = "source-screen-displacement-v1";
    },
    (m: SourceWaterStreamsManifest) => {
      delete m.material.appearanceProfile;
    },
  ]) {
    const candidate = structuredClone(recipe);
    mutate(candidate);
    assert.throws(() => validateSourceWaterStreams(candidate));
  }
  recipe.vertexStride = 48;
  recipe.vertices.byteLength = recipe.vertexCount * 48;
  recipe.normal = {
    ...recipe.noise,
    encoding: "unreal-normal-rgb",
    flipGreenChannel: false,
  };
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  recipe.normal.coverage = "bilinear-positive-area-8x8-v1";
  assert.throws(
    () => validateSourceWaterStreams(recipe),
    /clear|normal|profile/i,
  );
});
test("stream admission rejects resource escapes, unbounded buffers, singular transforms and unknown meshes", () => {
  const mutations = [
    (m: SourceWaterStreamsManifest) => {
      m.noise.url = "https://example.com/texture.bin";
    },
    (m: SourceWaterStreamsManifest) => {
      m.vertices.url = "/private-assets/../vertices.bin";
    },
    (m: SourceWaterStreamsManifest) => {
      m.indices.byteLength++;
    },
    (m: SourceWaterStreamsManifest) => {
      m.indexCount = 4;
    },
    (m: SourceWaterStreamsManifest) => {
      m.noise.width = 4096;
    },
    (m: SourceWaterStreamsManifest) => {
      m.scenarios.day[0].model = new Array(16).fill(0);
    },
    (m: SourceWaterStreamsManifest) => {
      m.scenarios.day[0].meshId = "unknown";
    },
    (m: SourceWaterStreamsManifest) => {
      m.scenarios.day.push(structuredClone(m.scenarios.day[0]));
    },
    (m: SourceWaterStreamsManifest) => {
      m.scenarios.day[0].parameters[1] = 2;
    },
    (m: SourceWaterStreamsManifest) => {
      m.material.powerExponent = NaN;
    },
  ];
  for (const mutate of mutations) {
    const m = fixture();
    mutate(m);
    assert.throws(() => validateSourceWaterStreams(m));
  }
});
test("altered private bytes are rejected before creating shader or GPU resources", async () => {
  let allocations = 0;
  const device = new Proxy(
    {},
    {
      get() {
        allocations++;
        throw new Error("Unexpected GPU allocation");
      },
    },
  ) as GPUDevice;
  await assert.rejects(
    SourceWaterStreams.create(
      device,
      fixture(),
      async (_url, bytes) => new ArrayBuffer(bytes),
    ),
    /changed/,
  );
  assert.equal(allocations, 0);
});

test("linear noise mip chain preserves an odd-width impulse and constant alpha", () => {
  const source = new Uint8Array([0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0, 255]);
  const levels = waterNoiseMips(source, 3, 1);
  assert.deepEqual(
    levels.map(({ width, height }) => [width, height]),
    [
      [3, 1],
      [1, 1],
    ],
  );
  assert.deepEqual([...levels[1].pixels], [85, 0, 0, 255]);
  assert.deepEqual([...source], [0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0, 255]);
  assert.throws(() => waterNoiseMips(new Uint8Array(0), 1, 1));
});

test("source normal recipes require explicit tangent vertices and linear signed-red decoding", () => {
  const recipe = fixture();
  recipe.vertexStride = 48;
  recipe.vertices.byteLength = recipe.vertexCount * 48;
  recipe.normal = {
    ...recipe.noise,
    encoding: "unreal-normal-red-splat",
    flipGreenChannel: false,
  };
  assert.equal(validateSourceWaterStreams(recipe), recipe);
  for (const change of [
    (m: SourceWaterStreamsManifest) => {
      m.vertexStride = 32;
      m.vertices.byteLength = m.vertexCount * 32;
    },
    (m: SourceWaterStreamsManifest) => {
      (m.normal as unknown as { colorSpace: string }).colorSpace = "srgb";
    },
    (m: SourceWaterStreamsManifest) => {
      (m.normal as unknown as { flipGreenChannel: unknown }).flipGreenChannel =
        undefined;
    },
    (m: SourceWaterStreamsManifest) => {
      (m.normal as unknown as { coverage: string }).coverage = "temporal";
    },
  ]) {
    const candidate = structuredClone(recipe);
    change(candidate);
    assert.throws(() => validateSourceWaterStreams(candidate));
  }
});

test("verified resource hashes do not allow invalid geometry to reach GPU allocation", async () => {
  const original = new Float32Array([
    0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0,
    1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1,
  ]);
  const indexBytes = new Uint32Array([0, 1, 2, 1, 3, 2]).buffer;
  const noise = new ArrayBuffer(20),
    header = new DataView(noise);
  header.setUint32(0, 0x41465238, false);
  header.setUint32(4, 1, true);
  header.setUint32(8, 1, true);
  header.setUint32(12, 1, true);
  new Uint8Array(noise, 16).set([128, 128, 255, 255]);
  for (const mutate of [
    (v: Float32Array) => v.fill(0, 3, 6),
    (v: Float32Array) => v.set([0, 0, 1], 8),
    (v: Float32Array) => {
      v[11] = 0;
    },
    (v: Float32Array) => {
      v[0] = Infinity;
    },
  ]) {
    const recipe = fixture(),
      vertices = original.slice();
    mutate(vertices);
    recipe.vertexStride = 48;
    const blobs = new Map([
      [recipe.vertices.url, vertices.buffer],
      [recipe.indices.url, indexBytes],
      [recipe.noise.url, noise],
    ]);
    for (const resource of [recipe.vertices, recipe.indices, recipe.noise]) {
      const bytes = blobs.get(resource.url)!;
      resource.byteLength = bytes.byteLength;
      resource.sha256 = [
        ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
      ]
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
    }
    let allocations = 0;
    const device = new Proxy(
      {},
      {
        get() {
          allocations++;
          throw new Error("Unexpected GPU allocation");
        },
      },
    ) as GPUDevice;
    await assert.rejects(
      SourceWaterStreams.create(device, recipe, async (url) => blobs.get(url)!),
    );
    assert.equal(allocations, 0);
  }
});
