import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import {
  createTightClusteredLightBuilder,
  buildTightClusteredLights,
  buildClusteredLights,
  CLUSTER_LIMITS,
  type ClusteredLightInput,
} from "../packages/renderer/clustered-lights.ts";
import { ClusteredLighting } from "../packages/renderer/clustered-lighting.ts";
import { createCFrameCore } from "../experiments/c-town/frame-core.ts";
import {
  lookAt,
  multiply,
  normalize,
  perspective,
  type V3,
} from "../packages/renderer/math.ts";
const light = (id: string, index = 0): SourceLight => ({
  id,
  label: id,
  kind: index % 3 ? "point" : "spot",
  position: [(index % 5) - 2, index % 3, -5 - index],
  direction: [0, -3, -4],
  color: [0.8, 0.5, 0.2],
  intensity: 10 + index,
  intensityUnits: index % 2 ? "candelas" : "lumens",
  range: 3 + (index % 8),
  castShadows: index % 4 !== 0,
  enabled: true,
  temperatureKelvin: 3000,
  useTemperature: index % 5 === 0,
  innerConeAngleRadians: 0.2,
  outerConeAngleRadians: 0.6,
  inverseSquaredFalloff: index % 7 !== 0,
  falloffExponent: 3,
  raw: {},
});
function input(lights: SourceLight[], i = 0): ClusteredLightInput {
  const eye: V3 = [Math.sin(i) * 3, 2, 4],
    target: V3 = [0, 1, -8];
  return {
    viewProjection: multiply(
      perspective(1.1, 1.6, 0.1, 2000),
      lookAt(eye, target),
    ),
    eye,
    forward: normalize(target.map((n, j) => n - eye[j]) as V3),
    width: 800,
    height: 500,
    lights,
  };
}
function same(actual: any, expected: any) {
  assert.deepEqual(actual, expected);
  assert.deepEqual(
    new Uint32Array(actual.storage.buffer),
    new Uint32Array(expected.storage.buffer),
  );
}

test("prepared tight records retain every result bit across cameras, shadow reassignment, overflow and source ordering", () => {
  const lights = Array.from({ length: 48 }, (_, i) => light(String(i), i));
  lights.push(
    { ...light("disabled"), enabled: false },
    { ...light("sun"), kind: "directional" },
  );
  const build = createTightClusteredLightBuilder(),
    before = structuredClone(lights);
  for (let i = 0; i < 18; i++) {
    const source = input(lights, i * 0.17);
    source.shadowIndices = new Map(
      lights
        .filter((l) => l.castShadows)
        .map((l, j) => [l.id, i % 3 === 0 ? -1 : (j + i) % 32]),
    );
    if (i % 4 === 0) source.maxIndices = 0;
    same(build(source, "base"), buildTightClusteredLights(source));
  }
  const overflow = input([light("wide")]);
  overflow.lights[0].position = [0, 0, 0];
  overflow.lights[0].range = 10000;
  Object.assign(overflow, {
    width: 16384,
    height: 16384,
    tileSize: 64,
    depthSlices: 4,
  });
  same(build(overflow, "overflow"), buildTightClusteredLights(overflow));
  const fallback = {
    ...input(
      Array.from({ length: 65 }, (_, i) => ({
        ...light(String(i), i),
        range: 1e6,
      })),
    ),
    width: 512,
    height: 512,
    tileSize: 8,
    depthSlices: 64,
  };
  const packedFallback = build(fallback, "global-overflow");
  assert.equal(packedFallback.stats.globalOverflow, true);
  same(packedFallback, buildTightClusteredLights(fallback));
  assert.deepEqual(lights, before);
});

test("source identity and explicit revision govern reuse; raw builders still validate fresh data", () => {
  const lights = [light("read")],
    source = input(lights),
    build = createTightClusteredLightBuilder();
  let reads = 0;
  const position = lights[0].position;
  Object.defineProperty(lights[0], "position", {
    configurable: true,
    get() {
      reads++;
      return position;
    },
  });
  build(source, 1);
  const admitted = reads;
  build({ ...source, ...input(lights, 0.2) }, 1);
  assert.equal(reads, admitted);
  build(source, 2);
  assert(reads > admitted);
  const newReads = reads;
  build({ ...source, lights: [...lights] }, 2);
  assert(reads > newReads);
  const rawReads = reads;
  buildTightClusteredLights(source);
  buildTightClusteredLights(source);
  assert(reads > rawReads);
  Object.defineProperty(lights[0], "position", {
    configurable: true,
    value: [NaN, 0, 0],
    writable: true,
  });
  assert.throws(() => build(source, 3), /light position/);
  Object.defineProperty(lights[0], "position", {
    configurable: true,
    value: position,
    writable: true,
  });
  same(build(source, 2), buildTightClusteredLights(source));
  lights[0].color = [0.1, 0.2, 0.3];
  same(build(source, 4), buildTightClusteredLights(source));
  // Omitting revision never grants a stable-source cache hit.
  lights[0].range = 0;
  assert.throws(() => build(source), /range/);
  lights[0].range = 4;
  same(build(source), buildTightClusteredLights(source));
});

test("prepared scratch and metadata never escape through mutable main or reflection outputs", () => {
  const lights = Array.from({ length: 12 }, (_, i) => light(String(i), i)),
    build = createTightClusteredLightBuilder(),
    a = input(lights),
    b = input(lights, 1);
  a.shadowIndices = new Map([["1", 3]]);
  b.shadowIndices = new Map([["1", 7]]);
  const first = build(a, 1),
    saved = structuredClone(first);
  const second = build(b, 1);
  same(first, saved);
  first.lightFloats.fill(99);
  first.lightIds.push("foreign");
  first.sourceIndices.fill(999);
  second.lightIds[0] = "edited";
  same(build(a, 1), buildTightClusteredLights(a));
  same(build(b, 1), buildTightClusteredLights(b));
});

test("invalid source or shadow reprepare preserves a prior valid cache and never skips validation", () => {
  const lights = Array.from({ length: 8 }, (_, i) => ({
      ...light(String(i), i),
      castShadows: true,
    })),
    build = createTightClusteredLightBuilder(),
    a = input(lights);
  const accepted = build(a, 1),
    saved = structuredClone(accepted);
  const bad = input(lights, 0.2);
  bad.shadowIndices = new Map([
    ["0", 4],
    ["1", 0.5],
  ]);
  assert.throws(() => build(bad, 1), /shadow frame index/);
  same(accepted, saved);
  same(build(a, 1), buildTightClusteredLights(a));
  for (const invalid of [
    { id: "" },
    { range: 0 },
    { intensity: Infinity },
    { position: [0, NaN, 0] },
    { color: [-1, 0, 0] },
    { kind: "spot", direction: [0, 0, 0] },
    { kind: "spot", innerConeAngleRadians: 1, outerConeAngleRadians: 0.1 },
    { inverseSquaredFalloff: false, falloffExponent: -1 },
  ]) {
    const malformed = {
      ...a,
      lights: [{ ...light("invalid"), ...invalid } as SourceLight],
    };
    assert.throws(() => build(malformed, 2));
    same(build(a, 1), buildTightClusteredLights(a));
  }
  assert.throws(() =>
    build({ ...a, lights: [light("same"), light("same")] }, 2),
  );
  assert.throws(() =>
    build(
      {
        ...a,
        lights: Array.from({ length: CLUSTER_LIMITS.lights + 1 }, (_, i) =>
          light(String(i)),
        ),
      },
      2,
    ),
  );
});
function gpuFixture(builder: any) {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { STORAGE: 1, COPY_DST: 2 },
  });
  let fail = false,
    failAllocation = false;
  const writes: Uint32Array[] = [];
  const buffers: any[] = [];
  const device: any = {
    limits: { maxStorageBufferBindingSize: 1 << 26 },
    createBuffer: ({ size }: any) => {
      if (failAllocation) {
        failAllocation = false;
        throw Error("mock allocation");
      }
      const buffer = {
        size,
        destroyed: false,
        data: new Uint32Array(0),
        destroy() {
          this.destroyed = true;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    queue: {
      writeBuffer(_b: any, _o: number, data: Uint32Array) {
        assert(!_b.destroyed);
        _b.data = data.slice();
        if (fail) {
          fail = false;
          throw Error("mock upload");
        }
        writes.push(data.slice());
      },
    },
  };
  return {
    lighting: new ClusteredLighting(device, builder),
    writes,
    buffers,
    failAllocation() {
      failAllocation = true;
    },
    fail() {
      fail = true;
    },
    close() {
      if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
      else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
    },
  };
}
test("view signatures include source identity; failed uploads do not commit camera or source hits", () => {
  const f = gpuFixture(createTightClusteredLightBuilder());
  try {
    const lights = [light("1")],
      a = input(lights),
      b = input(lights, 0.2);
    f.lighting.update("main", a, 1);
    f.lighting.update("planar", b, 1);
    const saved = f.writes[0].slice();
    f.lighting.update("main", a, 1);
    assert.equal(f.writes.length, 2);
    assert.deepEqual(f.writes[0], saved);
    const replacement = {
      ...a,
      lights: [{ ...lights[0], color: [0.2, 0.4, 0.8] }],
    };
    f.lighting.update("main", replacement, 1);
    assert.equal(f.writes.length, 3);
    assert.deepEqual(
      f.writes[2],
      buildTightClusteredLights(replacement).storage,
    );
    f.fail();
    assert.throws(() => f.lighting.update("main", b, 2), /mock upload/);
    f.lighting.update("main", b, 2);
    assert.deepEqual(f.writes.at(-1), buildTightClusteredLights(b).storage);
  } finally {
    f.lighting.dispose();
    f.close();
  }
});
test("growth and upload failures retain the old buffer and force old-camera retry", () => {
  const f = gpuFixture(createTightClusteredLightBuilder());
  try {
    const lights = [light("stable")],
      a = { ...input(lights), width: 64, height: 64 },
      b = input(lights, 0.5);
    const accepted = f.lighting.update("main", a, 1),
      previous: any = accepted.buffer;
    const expected = buildTightClusteredLights(a).storage;
    const writesBefore = f.writes.length;
    f.failAllocation();
    assert.throws(() => f.lighting.update("main", b, 1), /mock allocation/);
    assert.equal(f.lighting.buffer("main"), previous);
    assert.equal(previous.destroyed, false);
    f.lighting.update("main", a, 1);
    assert.equal(f.writes.length, writesBefore + 1);
    assert.deepEqual(previous.data, expected);
    f.fail();
    assert.throws(() => f.lighting.update("main", b, 1), /mock upload/);
    assert.equal(f.lighting.buffer("main"), previous);
    assert.equal(previous.destroyed, false);
    assert.equal(f.buffers.at(-1).destroyed, true);
    f.lighting.update("main", a, 1);
    assert.deepEqual(previous.data, expected);
    // Simulate a same-buffer write that changed bytes before throwing. Its old
    // signature must not return until the original bytes have been uploaded.
    const c = { ...a, ...input(lights, 0.2), width: 64, height: 64 };
    f.fail();
    assert.throws(() => f.lighting.update("main", c, 1), /mock upload/);
    const beforeRetry = f.writes.length;
    f.lighting.update("main", a, 1);
    assert.equal(f.writes.length, beforeRetry + 1);
    assert.deepEqual(previous.data, expected);
    const grown = f.lighting.update("main", b, 1);
    assert.equal(grown.bindingChanged, true);
    assert.equal(previous.destroyed, true);
    assert.equal(grown.buffer, f.lighting.buffer("main"));
  } finally {
    f.lighting.dispose();
    f.close();
  }
});
test("one-argument custom or experimental conservative builders keep their original algorithm and inputs", () => {
  let calls = 0;
  const builder = (source: ClusteredLightInput) => {
    calls++;
    return buildClusteredLights(source);
  };
  const f = gpuFixture(builder);
  try {
    const lights = [light("1")],
      a = input(lights),
      b = input(lights, 0.4);
    for (const [i, source] of [a, b].entries()) {
      f.lighting.update("main", source, i);
      assert.deepEqual(f.writes[i], buildClusteredLights(source).storage);
    }
    assert.equal(calls, 2);
  } finally {
    f.lighting.dispose();
    f.close();
  }
});
const cFrame = process.env.AF_PREPARED_C_FRAME_WASM;
test(
  "the real experimental C builder remains conservative and independent of prepared tight records",
  { skip: !cFrame },
  async () => {
    const core = await createCFrameCore(await readFile(cFrame!));
    const f = gpuFixture((source: ClusteredLightInput) =>
      core.buildClusters(source),
    );
    try {
      const lights = Array.from({ length: 24 }, (_, i) => light(String(i), i));
      for (let i = 0; i < 6; i++) {
        const source = input(lights, i * 0.2);
        if (i % 2) source.maxIndices = 0;
        f.lighting.update(i % 2 ? "planar" : "main", source, i);
        assert.deepEqual(f.writes.at(-1), buildClusteredLights(source).storage);
      }
    } finally {
      f.lighting.dispose();
      f.close();
    }
  },
);
const assets = process.env.AF_PREPARED_CLUSTER_ASSETS;
test(
  "actual private Town day/night prepared lists match every raw tight-builder byte",
  { skip: !assets },
  async () => {
    const build = createTightClusteredLightBuilder();
    for (const [filename, mode] of [
      ["scene.json", "day"],
      ["scene-night.json", "night"],
    ] as const) {
      const scene = JSON.parse(
        await readFile(resolve(assets!, filename), "utf8"),
      );
      assert(
        scene.lookURL.startsWith("/private-assets/") &&
          !scene.lookURL.includes(".."),
      );
      const look = JSON.parse(
          await readFile(
            resolve(assets!, scene.lookURL.slice("/private-assets/".length)),
            "utf8",
          ),
        ),
        lights = look.scenarios[mode].lights;
      assert(Array.isArray(lights));
      assert(look.scenarios[mode].cameras.length > 0);
      for (const [i, camera] of look.scenarios[mode].cameras.entries()) {
        const eye = camera.position as V3,
          forward = normalize(
            camera.target.map((n: number, j: number) => n - eye[j]) as V3,
          );
        const source: ClusteredLightInput = {
          viewProjection: multiply(
            perspective(
              (camera.verticalFovDegrees * Math.PI) / 180,
              1280 / 720,
              0.1,
              2000,
            ),
            lookAt(eye, camera.target, camera.up),
          ),
          eye,
          forward,
          width: 1280,
          height: 720,
          lights,
          shadowIndices: new Map(
            lights
              .filter((l: SourceLight) => l.castShadows)
              .slice(i % 8, (i % 8) + 32)
              .map((l: SourceLight, j: number) => [l.id, j]),
          ),
        };
        same(build(source, mode), buildTightClusteredLights(source));
      }
    }
  },
);
