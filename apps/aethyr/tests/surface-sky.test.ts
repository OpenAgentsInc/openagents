import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  bakeSurfaceSkyPoint,
  evaluateSurfaceSky,
  packSurfaceSky,
  validateSurfaceSky,
  surfaceSkySceneIdentityPayload,
  surfaceSkySelection,
  SurfaceSky,
  validateSurfaceSkySources,
  surfaceSkyPackedFloatCount,
  type SurfaceSkyManifest,
} from "../packages/renderer/surface-sky.ts";
import { buildSurfaceSkySamples } from "../packages/renderer/surface-sky-bake.ts";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { surfaceFixture } from "./fixtures/surface-sky.ts";
const near = (a: number, b: number, e = 0.004) =>
  assert.ok(Math.abs(a - b) < e, `${a} differs from ${b}`);
test("surface sky integrates camera-independent raw E/pi and composes solar and constant before its only clamp", () => {
  const m = surfaceFixture(),
    p = { position: [0, 0, 0], geometricNormal: [0, 1, 0] } as const;
  const result = bakeSurfaceSkyPoint(
    { position: [...p.position], geometricNormal: [...p.geometricNormal] },
    m.transport,
    () => false,
    () => [1, 2, 3],
    () => [2, 1, 0],
  );
  for (const n of [
    [0, 1, 0],
    [0, -1, 0],
    [1, 0, 0],
  ] as const) {
    const value = evaluateSurfaceSky(result.coefficients, [...n], 2);
    [4, 5, 6].forEach((v, c) => near(value[c], v, 0.012));
    const zero = evaluateSurfaceSky(result.coefficients, [...n], 0);
    [2, 1, 0].forEach((v, c) => near(zero[c], v, 0.005));
  }
  const coefficients = new Float32Array(72);
  coefficients[0] = -1 / 0.2820947918;
  coefficients[36] = 2 / 0.2820947918;
  near(evaluateSurfaceSky(coefficients, [0, 1, 0], 1)[0], 1, 1e-6);
});
test("near and far transport are disjoint, fully blocked stays valid zero and incident-side normal offsets preserve both thin-sheet sides", () => {
  const m = surfaceFixture(),
    ranges: number[][] = [],
    originSigns = new Set<number>();
  let envReads = 0;
  const result = bakeSurfaceSkyPoint(
    { position: [1, 2, 3], geometricNormal: [0, 1, 0] },
    m.transport,
    (o, d, lo, hi) => {
      ranges.push([lo, hi]);
      originSigns.add(Math.sign(o[1] - 2));
      assert.equal(Math.sign(o[1] - 2), Math.sign(d[1]));
      return hi === 5000;
    },
    () => {
      envReads++;
      return [1, 1, 1];
    },
  );
  assert.equal(result.farBlocked, 512);
  assert.equal(result.nearBlocked, 0);
  assert.equal(envReads, 0);
  assert.equal(result.coefficients[3], 1);
  assert.deepEqual(
    evaluateSurfaceSky(result.coefficients, [0, 1, 0], 1),
    [0, 0, 0],
  );
  assert.deepEqual([...originSigns].sort(), [-1, 1]);
  assert.deepEqual(ranges.slice(0, 2), [
    [0.0005, 10],
    [10, 5000],
  ]);
  const blocked = bakeSurfaceSkyPoint(
    { position: [0, 0, 0], geometricNormal: [0, 1, 0] },
    m.transport,
    () => true,
    () => {
      throw Error("No blocked ray may sample sky");
    },
  );
  assert.equal(blocked.nearBlocked, 512);
  assert.equal(blocked.farBlocked, 0);
});
test("record packing guards sparse validity, independent selected ranges and finite allocations", () => {
  const m = surfaceFixture(),
    values = new Float32Array(3 * 72);
  values[3] = values[75] = values[147] = 1;
  const packed = packSurfaceSky(m, values);
  assert.deepEqual([...packed.slice(0, 4)], [0, 3, 1, 1]);
  assert.equal(packed.length, 220);
  assert.equal(surfaceSkySelection(m, "Owned plant", "Mesh", "owned-leaf"), 1);
  assert.equal(surfaceSkySelection(m, "Other", "Mesh", "owned-leaf"), 0);
  values[4] = NaN;
  assert.throws(() => packSurfaceSky(m, values), /nonfinite/);
  values[4] = 0;
  values[7] = 1;
  assert.throws(() => packSurfaceSky(m, values), /padding/);
  const bad = structuredClone(m);
  bad.vertexCount = 1000000;
  assert.throws(() => validateSurfaceSky(bad));
});

test("sparse addresses preserve records, exact float IDs and vec4 alignment while legacy bytes stay unchanged", () => {
  const m = surfaceFixture(),
    values = new Float32Array(216);
  values[3] = values[75] = values[147] = 1;
  const legacy = packSurfaceSky(m, values);
  const sparse = structuredClone(m);
  sparse.selections[0].firstVertex = 2;
  sparse.selections[0].vertexIndices = [2, 90, 0xffffff];
  const packed = packSurfaceSky(sparse, values);
  assert.deepEqual([...packed.slice(0, 8)], [4, 3, 2, 3, 2, 90, 0xffffff, 0]);
  assert.deepEqual(packed.slice(8), legacy.slice(4));
  assert.equal(packed.length, surfaceSkyPackedFloatCount(sparse));
  const mixed = structuredClone(sparse);
  mixed.vertexCount = 4;
  mixed.scenarios.day!.bytes = 4 * 72 * 4;
  mixed.selections.push({
    ...structuredClone(mixed.selections[0]),
    actor: "Second plant",
    firstVertex: 9,
    vertexCount: 1,
    recordStart: 3,
    vertexIndices: [9],
  });
  const mixedValues = new Float32Array(4 * 72);
  mixedValues.set(values);
  const combined = packSurfaceSky(mixed, mixedValues);
  assert.deepEqual(
    [...combined.slice(0, 16)],
    [8, 3, 4, 3, 12, 1, 58, 3, 2, 90, 0xffffff, 0, 9, 0, 0, 0],
  );
  assert.deepEqual(combined.slice(16), mixedValues);
  sparse.reflection = { model: "sh9-ggx-rough-v1" };
  assert.equal(packSurfaceSky(sparse, values)[3], 4);
  m.reflection = { model: "sh9-ggx-rough-v1" };
  assert.equal(packSurfaceSky(m, values)[3], 2);
  const max = surfaceFixture();
  max.vertexCount = max.selections[0].vertexCount = 32768;
  max.scenarios.day!.bytes = 32768 * 72 * 4;
  max.selections[0].vertexIndices = Array.from(
    { length: 32768 },
    (_, i) => i * 2,
  );
  const big = packSurfaceSky(max, new Float32Array(32768 * 72));
  assert.equal(big.byteLength, (4 + 32768 + 32768 * 72) * 4);
  assert.equal(big[2], (4 + 32768) / 4);
  assert.ok(big.byteLength * 2 + 16 < 32 * 1024 * 1024);
  for (const mutate of [
    (s: SurfaceSkyManifest) => (s.selections[0].vertexIndices = [2, 90]),
    (s: SurfaceSkyManifest) => (s.selections[0].vertexIndices = [2, 2, 90]),
    (s: SurfaceSkyManifest) => (s.selections[0].vertexIndices = [2, 100, 90]),
    (s: SurfaceSkyManifest) =>
      (s.selections[0].vertexIndices = [2, 90, 0x1000000]),
    (s: SurfaceSkyManifest) => (s.selections[0].vertexIndices = [2, 90, 100.5]),
    (s: SurfaceSkyManifest) => (s.selections[0].firstVertex = 1),
    (s: SurfaceSkyManifest) => (s.reflection = { model: "unknown" } as any),
    (s: SurfaceSkyManifest) =>
      (s.reflection = { model: "sh9-ggx-rough-v1", gain: 1 } as any),
  ]) {
    const invalid = structuredClone(sparse);
    mutate(invalid);
    assert.throws(() => validateSurfaceSky(invalid));
  }
});
test("scene identity includes source materials, model, colors and UVs but excludes roof grids and unrelated effect links", () => {
  const s = {
    vertices: "a",
    indices: "b",
    vertexColors: "c",
    vertexUV1: "d",
    vertexCount: 1,
    indexCount: 3,
    vertexStride: 48,
    materials: [{ id: "x", normalEncoding: "rgb" }],
    meshes: [],
    instances: [],
  } as unknown as CompiledScene;
  const original = surfaceSkySceneIdentityPayload(s),
    changed = structuredClone(s);
  changed.probesURL = "/different-grid.json";
  changed.leavesURL = "/effect.json";
  assert.equal(surfaceSkySceneIdentityPayload(changed), original);
  changed.vertexUV1 = "e";
  assert.notEqual(surfaceSkySceneIdentityPayload(changed), original);
  changed.vertexUV1 = "d";
  changed.materials[0].normalEncoding = "xy-positive-z";
  assert.notEqual(surfaceSkySceneIdentityPayload(changed), original);
});
test("source sample selection preserves affine geometry and geometric normals, rejects stale identities and degenerate selection", () => {
  const vertices = new Float32Array(36);
  vertices.set([0, 0, 0], 0);
  vertices.set([1, 0, 0], 12);
  vertices.set([0, 1, 0], 24);
  const scene = {
    vertexCount: 3,
    indexCount: 3,
    materials: [{ id: "owned-leaf", alphaMode: "MASK" }],
    instances: [
      {
        actor: "Owned plant",
        component: "Mesh",
        materialOverrides: [-1],
        mesh: 0,
        model: [2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 5, 6, 7, 1],
      },
    ],
    meshes: [
      {
        id: "owned-triangle",
        primitives: [
          {
            material: 0,
            materialSlot: 0,
            baseVertex: 0,
            firstIndex: 0,
            indexCount: 3,
          },
        ],
      },
    ],
  } as unknown as CompiledScene;
  const selector = [
      { actor: "Owned plant", component: "Mesh", materialIds: ["owned-leaf"] },
    ],
    result = buildSurfaceSkySamples(
      scene,
      vertices,
      new Uint32Array([0, 1, 2]),
      selector,
    );
  assert.deepEqual(result.points[1]?.position, [7, 6, 7]);
  assert.deepEqual(result.points[0]?.geometricNormal, [0, 0, 1]);
  assert.equal(result.selections[0].vertexIndices, undefined);
  const scattered = new Float32Array(7 * 12);
  scattered.set(vertices.slice(0, 12), 0);
  scattered.set(vertices.slice(12, 24), 2 * 12);
  scattered.set(vertices.slice(24, 36), 6 * 12);
  const sparse = buildSurfaceSkySamples(
    { ...scene, vertexCount: 7 },
    scattered,
    new Uint32Array([0, 2, 6]),
    selector,
  );
  assert.deepEqual(sparse.selections[0].vertexIndices, [0, 2, 6]);
  assert.equal(sparse.points.length, 3);
  assert.deepEqual(sparse.points[1]?.position, [7, 6, 7]);
  assert.throws(
    () =>
      buildSurfaceSkySamples(
        scene,
        vertices,
        new Uint32Array([0, 1, 4]),
        selector,
      ),
    /vertex/,
  );
  assert.throws(
    () =>
      buildSurfaceSkySamples(
        scene,
        vertices,
        new Uint32Array([0, 0, 0]),
        selector,
      ),
    /triangles/,
  );
  selector[0].materialIds = ["wrong"];
  assert.throws(() =>
    buildSurfaceSkySamples(
      scene,
      vertices,
      new Uint32Array([0, 1, 2]),
      selector,
    ),
  );
});
test("partial mode load failure destroys every allocated buffer and fallback", async () => {
  const priorUsage = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUBufferUsage",
  );
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { STORAGE: 128, COPY_DST: 8 },
    configurable: true,
  });
  const manifest = surfaceFixture(),
    values = new Float32Array(216),
    bytes = new Uint8Array(values.buffer);
  values[3] = values[75] = values[147] = 1;
  manifest.scenarios.day!.sha256 = createHash("sha256")
    .update(bytes)
    .digest("hex");
  manifest.scenarios.night = {
    ...manifest.scenarios.day!,
    url: "/private-assets/missing.bin",
  };
  manifest.sourceBlobs.night = manifest.sourceBlobs.day;
  const buffers: Array<{ destroyed: boolean; destroy(): void }> = [],
    device = {
      createBuffer() {
        const b = {
          destroyed: false,
          destroy() {
            this.destroyed = true;
          },
        };
        buffers.push(b);
        return b;
      },
      queue: { writeBuffer() {} },
    } as unknown as GPUDevice;
  const fetch = globalThis.fetch;
  globalThis.fetch = async (input) =>
    new Response(
      String(input).includes("missing") ? null : bytes.slice().buffer,
      { status: String(input).includes("missing") ? 404 : 200 },
    );
  try {
    await assert.rejects(SurfaceSky.create(device, manifest), /missing/);
    assert.equal(buffers.length, 2);
    assert.ok(buffers.every((b) => b.destroyed));
  } finally {
    globalThis.fetch = fetch;
    if (priorUsage)
      Object.defineProperty(globalThis, "GPUBufferUsage", priorUsage);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("alpha-aware near blockers and far blockers affect transfer independently", async () => {
  const { buildGiAcceleration } =
      await import("../packages/renderer/gi-acceleration.ts"),
    { createGiSurfaceTransport } =
      await import("../packages/renderer/gi-transport.ts");
  const vertices = new Float32Array(72);
  for (let plane = 0; plane < 2; plane++)
    for (const [i, p] of [
      [-100, -100],
      [100, -100],
      [0, 100],
    ].entries())
      vertices.set(
        [p[0], p[1], plane === 0 ? 1 : 20, 0, 0, 1, 0, 0, 1, 0, 0, 1],
        (plane * 3 + i) * 12,
      );
  const material = {
    id: "owned",
    name: "owned",
    baseColorFactor: [1, 1, 1, 1],
    uvScale: [1, 1],
    alphaMode: "MASK",
    alphaCutoff: 0.5,
    doubleSided: true,
    baseColorUrl: "/private-assets/alpha.bin",
    packedChannels: "metallic-r-roughness-g-occlusion-b",
  };
  const scene = {
    vertexCount: 6,
    indexCount: 6,
    vertexStride: 48,
    materials: [material, { ...material, id: "far", alphaMode: "OPAQUE" }],
    meshes: [
      {
        name: "owned planes",
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
        actor: "owned",
        component: "owned",
        sourceClass: "StaticMeshActor",
        materialOverrides: [-1],
        model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      },
    ],
  } as unknown as CompiledScene;
  const acceleration = buildGiAcceleration(
      scene,
      vertices,
      new Uint32Array([0, 1, 2, 3, 4, 5]),
    ),
    attributes = { vertices, colors: new Uint8Array(24).fill(255) },
    transport = (alpha: number) =>
      createGiSurfaceTransport(acceleration, scene, attributes, () => [
        1,
        1,
        1,
        alpha,
      ]);
  const transparent = transport(0),
    opaque = transport(1);
  assert.equal(transparent.occluded([0, 0, 0], [0, 0, 1], 0.001, 10), false);
  assert.equal(transparent.occluded([0, 0, 0], [0, 0, 1], 10, 100), true);
  assert.equal(opaque.occluded([0, 0, 0], [0, 0, 1], 0.001, 10), true);
  const profile = surfaceFixture().transport,
    a = bakeSurfaceSkyPoint(
      { position: [0, 0, 0], geometricNormal: [0, 0, 1] },
      profile,
      transparent.occluded,
      () => [1, 1, 1],
    ),
    b = bakeSurfaceSkyPoint(
      { position: [0, 0, 0], geometricNormal: [0, 0, 1] },
      profile,
      opaque.occluded,
      () => [1, 1, 1],
    );
  assert.equal(a.nearBlocked, 0);
  assert.ok(a.farBlocked > 0);
  assert.ok(b.nearBlocked > 0);
  assert.ok(b.farBlocked < a.farBlocked);
  assert.ok(transparent.stats.transparentHits > 0);
});

test("source admission checks all immutable inputs and exact selected model/material/index addresses before GPU allocation", async () => {
  const m = surfaceFixture(),
    selection = m.selections[0],
    data = new Map<string, Uint8Array>(),
    input = (name: string, bytes: Uint8Array) => {
      const url = "/private-assets/" + name + ".bin";
      data.set(url, bytes);
      return {
        url,
        bytes: bytes.byteLength,
        sha256: createHash("sha256").update(bytes).digest("hex"),
      };
    };
  const vertices = input("vertices", new Uint8Array(144)),
    indices = input(
      "indices",
      new Uint8Array(new Uint32Array([0, 1, 2]).buffer),
    ),
    colors = input("colors", new Uint8Array(12)),
    environment = input("environment", new Uint8Array(8));
  const scene = {
    scenario: "day",
    vertices: vertices.url,
    indices: indices.url,
    vertexColors: colors.url,
    vertexCount: 3,
    indexCount: 3,
    vertexStride: 48,
    materials: [{ id: "owned-leaf", alphaMode: "OPAQUE" }],
    meshes: [
      {
        id: selection.mesh,
        primitives: [
          {
            material: 0,
            materialSlot: 0,
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
          },
        ],
      },
    ],
    instances: [
      {
        actor: selection.actor,
        component: selection.component,
        mesh: 0,
        model: [...selection.model],
        materialOverrides: [-1],
      },
    ],
  } as unknown as CompiledScene;
  const env = {
    url: environment.url,
    width: 1,
    height: 1,
    rotation: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    uSign: 1,
    uOffset: 0,
    vSign: 1,
    vOffset: 0,
    tint: [1, 1, 1],
    brightness: 1,
  } as import("../packages/assets/src/scene.ts").EnvironmentMap;
  const { surfaceSkyEnvironmentIdentityPayload } =
    await import("../packages/renderer/surface-sky.ts");
  m.scenarios.day!.sourceSceneSha256 = createHash("sha256")
    .update(surfaceSkySceneIdentityPayload(scene))
    .digest("hex");
  m.scenarios.day!.sourceEnvironmentSha256 = createHash("sha256")
    .update(surfaceSkyEnvironmentIdentityPayload(env))
    .digest("hex");
  m.sourceBlobs.day = [vertices, indices, colors, environment];
  const fetch = globalThis.fetch,
    requests = new Map<string, number>();
  globalThis.fetch = async (url) => {
    const key = String(url);
    requests.set(key, (requests.get(key) ?? 0) + 1);
    return new Response(data.get(key)!.slice().buffer);
  };
  try {
    const cache = new Map();
    await validateSurfaceSkySources(m, { day: scene }, { day: env }, cache);
    scene.probesURL = "/new-taller-grid.json";
    await validateSurfaceSkySources(m, { day: scene }, { day: env }, cache);
    assert.equal(requests.get(vertices.url), 1);
    assert.equal(requests.get(indices.url), 2);
    const sparse = structuredClone(m);
    sparse.selections[0].vertexIndices = [0, 1, 2];
    await validateSurfaceSkySources(
      sparse,
      { day: scene },
      { day: env },
      cache,
    );
    sparse.selections[0].vertexIndices = [0, 1, 3];
    await assert.rejects(
      validateSurfaceSkySources(sparse, { day: scene }, { day: env }, cache),
      /vertex range/,
    );
    const missing = structuredClone(m);
    missing.sourceBlobs.day!.pop();
    await assert.rejects(
      validateSurfaceSkySources(missing, { day: scene }, { day: env }),
      /source list/,
    );
    const moved = structuredClone(m);
    moved.selections[0].model[12] = 1;
    await assert.rejects(
      validateSurfaceSkySources(moved, { day: scene }, { day: env }),
      /model changed/,
    );
    const range = structuredClone(m);
    range.selections[0].firstVertex = 1;
    await assert.rejects(
      validateSurfaceSkySources(range, { day: scene }, { day: env }),
      /vertex range/,
    );
    const material = structuredClone(m);
    material.selections[0].materialIds = ["wrong"];
    await assert.rejects(
      validateSurfaceSkySources(material, { day: scene }, { day: env }),
      /vertex range/,
    );
    await assert.rejects(
      validateSurfaceSkySources(
        m,
        { day: scene },
        { day: { ...env, brightness: 2 } },
      ),
      /Rebuild/,
    );
    data.set(indices.url, new Uint8Array(new Uint32Array([0, 1, 4]).buffer));
    const changed = structuredClone(m);
    changed.sourceBlobs.day![1].sha256 = createHash("sha256")
      .update(data.get(indices.url)!)
      .digest("hex");
    await assert.rejects(
      validateSurfaceSkySources(changed, { day: scene }, { day: env }),
      /index leaves/,
    );
    await assert.rejects(
      validateSurfaceSkySources(m, { day: scene }, { day: env }),
      /hash/,
    );
  } finally {
    globalThis.fetch = fetch;
  }
});
