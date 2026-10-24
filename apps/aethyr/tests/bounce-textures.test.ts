import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BounceTextures,
  validateBounceManifest,
} from "../packages/renderer/bounce-textures.ts";
import {
  giBounceBaseIdentity,
  giBounceLocalLightsIdentityPayload,
  type GiBounceManifest,
} from "../packages/renderer/gi-bounce.ts";
import type { GiProbeManifest } from "../packages/renderer/gi-probes.ts";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import { GI_MATERIAL_PROFILE } from "../packages/renderer/gi-materials.ts";
import { probeSha256 } from "../packages/renderer/probe-textures.ts";

const hash = (s: string) => probeSha256(new TextEncoder().encode(s));
const lights: SourceLight[] = [
  {
    id: "owned-lamp",
    label: "Owned lamp",
    kind: "point",
    position: [0, 2, 0],
    direction: [0, -1, 0],
    color: [1, 0.5, 0.25],
    intensity: 2,
    intensityUnits: "candela",
    range: 4,
    enabled: true,
    castShadows: true,
    temperatureKelvin: 6500,
    useTemperature: false,
    raw: {},
  },
];
async function fixture() {
  const base: GiProbeManifest = {
    schema: "aethyrforge.private-sky-probes.v1",
    profile: "occluded-sky-sh9-v1",
    sourceGeometrySha256: "a".repeat(64),
    grid: {
      origin: [0, 1, 0],
      spacing: [4, 2, 4],
      dimensions: [2, 2, 2],
      count: 8,
    },
    records: { strideFloats: 40, positionValidOffset: 0, shOffset: 4 },
    moments: {
      url: "/private-assets/base-moments.bin",
      sha256: "b".repeat(64),
      bytes: 20 * 40 * 8,
      format: "rgba16float",
      width: 20,
      height: 40,
      interior: 8,
      tileSize: 10,
      tilesPerRow: 2,
      rayDistanceMeters: 64,
      distanceConvention: "mean-meters,mean-square-meters-squared",
    },
    scenarios: {},
    lighting: {
      convention: "world-sh9-lambert-E-over-pi",
      gainBaked: false,
      skyBounceCount: 0,
      rayCount: 64,
      alphaPolicy: "alpha-tested-material-v1",
      lowerHemisphere: "world-up-black",
    },
  };
  const files = new Map<string, Uint8Array>();
  const scenarios: GiBounceManifest["scenarios"] = {};
  const bindings: GiBounceManifest["base"]["scenarios"] = {};
  for (const [index, mode] of (["day", "night"] as const).entries()) {
    base.scenarios[mode] = {
      url: `/private-assets/${mode}-base.bin`,
      sha256: String(index + 1).repeat(64),
      bytes: 8 * 160,
      sourceEnvironmentSha256: String(index + 3).repeat(64),
      sourceGeometrySha256: "a".repeat(64),
    };
    bindings[mode] = {
      recordsSha256: base.scenarios[mode]!.sha256,
      momentsSha256: base.moments.sha256,
    };
    const layers = {} as NonNullable<
      GiBounceManifest["scenarios"]["day"]
    >["layers"];
    for (const [layerIndex, layer] of (
      ["sky", "sun", "locals"] as const
    ).entries()) {
      const values = new Float32Array(8 * 36).fill(index * 10 + layerIndex + 1);
      const bytes = new Uint8Array(values.buffer),
        url = `/private-assets/${mode}-${layer}.bin`;
      files.set(url, bytes);
      layers[layer] = {
        url,
        bytes: bytes.length,
        sha256: await probeSha256(bytes),
      };
    }
    scenarios[mode] = {
      sourceGeometrySha256: "a".repeat(64),
      sourceCoverageSha256: "c".repeat(64),
      sourceMaterialSha256: "d".repeat(64),
      sourceEnvironmentSha256: base.scenarios[mode]!.sourceEnvironmentSha256,
      sourceSunDirection: [0, -1, 0],
      sourceLocalLightsSha256: await hash(
        giBounceLocalLightsIdentityPayload(lights),
      ),
      layers,
    };
  }
  const manifest: GiBounceManifest = {
    schema: "aethyrforge.private-bounce-probes.v1",
    profile: "alpha-lambert-one-bounce-v1",
    base: {
      identitySha256: await hash(giBounceBaseIdentity(base)),
      scenarios: bindings,
    },
    grid: structuredClone(base.grid),
    layout: {
      order: "layer-major-sh9-rgb-vec4",
      layers: ["sky", "sun", "locals"],
      coefficientStrideFloats: 4,
      coefficientsPerProbe: 9,
      bytesPerProbePerLayer: 144,
    },
    scenarios,
    lighting: {
      convention: "world-sh9-lambert-E-over-pi",
      diffuseBounceCount: 1,
      gainBaked: false,
      localSourceStrengthBaked: true,
      sun: "white-per-unit-lux",
      alphaPolicy: "alpha-tested-material-v1",
      materialProfile: GI_MATERIAL_PROFILE,
      emissiveGi: false,
    },
    limitations: [],
  };
  return { base, manifest, files };
}
function mock() {
  Object.assign(globalThis, { GPUBufferUsage: { STORAGE: 1, COPY_DST: 2 } });
  const allocations: Array<{
    label: string;
    size: number;
    destroyed: number;
    data?: Float32Array;
  }> = [];
  return {
    allocations,
    device: {
      createBuffer(d: GPUBufferDescriptor) {
        const b = {
          label: d.label ?? "",
          size: d.size,
          destroyed: 0,
          data: undefined as Float32Array | undefined,
          destroy() {
            b.destroyed++;
          },
        };
        allocations.push(b);
        return b;
      },
      queue: {
        writeBuffer(
          buffer: (typeof allocations)[number],
          _offset: number,
          data: Float32Array,
        ) {
          buffer.data = data.slice();
        },
      },
    } as unknown as GPUDevice,
  };
}
async function withFetch<T>(
  files: Map<string, Uint8Array>,
  run: (requests: string[]) => Promise<T>,
) {
  const original = globalThis.fetch,
    requests: string[] = [];
  globalThis.fetch = async (input) => {
    const url = String(input);
    requests.push(url);
    const data = files.get(url);
    return new Response(data?.slice().buffer, { status: data ? 200 : 404 });
  };
  try {
    return await run(requests);
  } finally {
    globalThis.fetch = original;
  }
}

test("bounce admission binds exact base hashes, grid, selected records, visibility and layer packing", async () => {
  const { base, manifest } = await fixture();
  await validateBounceManifest(base, manifest, "day");
  const reordered = structuredClone(base);
  reordered.scenarios = {
    night: base.scenarios.night,
    day: base.scenarios.day,
  };
  await validateBounceManifest(reordered, manifest, "night");
  for (const mutate of [
    (v: GiBounceManifest) => {
      v.base.identitySha256 = "0".repeat(64);
    },
    (v: GiBounceManifest) => {
      v.base.scenarios.day!.recordsSha256 = "0".repeat(64);
    },
    (v: GiBounceManifest) => {
      v.base.scenarios.day!.momentsSha256 = "0".repeat(64);
    },
    (v: GiBounceManifest) => {
      v.grid.origin[0] = 1;
    },
    (v: GiBounceManifest) => {
      v.layout.layers = [
        "locals",
        "sun",
        "sky",
      ] as unknown as typeof v.layout.layers;
    },
    (v: GiBounceManifest) => {
      v.scenarios.day!.sourceEnvironmentSha256 = "f".repeat(64);
    },
    (v: GiBounceManifest) => {
      v.scenarios.day!.layers.sky.bytes--;
    },
  ]) {
    const invalid = structuredClone(manifest);
    mutate(invalid);
    await assert.rejects(validateBounceManifest(base, invalid, "day"));
  }
});

test("three hashed layers occupy one buffer per mode and stale sunlight or locals disable only that layer", async () => {
  const { base, manifest, files } = await fixture(),
    gpu = mock();
  await withFetch(files, async (requests) => {
    const textures = await BounceTextures.create(
      gpu.device,
      { day: base, night: base },
      { day: manifest, night: manifest },
      { day: lights, night: lights },
    );
    assert.equal(requests.length, 6);
    assert.equal(gpu.allocations.length, 3);
    assert.equal(textures.bytes, 16 + 2 * 8 * 432);
    for (const [index, mode] of (["day", "night"] as const).entries()) {
      const allocation = gpu.allocations.find(
        (a) => a.label === mode + " diffuse bounce layers",
      )!;
      assert.equal(allocation.size, 8 * 432);
      for (let layer = 0; layer < 3; layer++)
        assert.ok(
          allocation
            .data!.subarray(layer * 8 * 36, (layer + 1) * 8 * 36)
            .every((n) => n === index * 10 + layer + 1),
        );
      assert.deepEqual([...textures.settings(mode, [0, 3, 0])], [1, 1, 1, 8]);
      assert.deepEqual([...textures.settings(mode, [1, 0, 0])], [1, 0, 1, 8]);
    }
    const changed = lights.map((l) => ({ ...l, intensity: 3 }));
    const pending = textures.setLocalLights("day", changed);
    assert.deepEqual([...textures.settings("day", [0, 1, 0])], [1, 1, 0, 8]);
    assert.equal(await pending, false);
    assert.equal(await textures.setLocalLights("day", lights), true);
    const newTemperatureModel = lights.map((light) => ({
      ...light,
      temperatureModel: "cie-planckian-rec709-v1" as const,
    }));
    assert.equal(
      await textures.setLocalLights("day", newTemperatureModel),
      false,
    );
    assert.deepEqual([...textures.settings("day", [0, 1, 0])], [1, 1, 0, 8]);
    assert.equal(await textures.setLocalLights("day", lights), true);
    const older = textures.setLocalLights("day", lights),
      newer = textures.setLocalLights("day", changed);
    await Promise.all([older, newer]);
    assert.deepEqual([...textures.settings("day", [0, 1, 0])], [1, 1, 0, 8]);
    await textures.setLocalLights("day", lights);
    assert.deepEqual([...textures.settings("day", [NaN, 1, 0])], [1, 0, 1, 8]);
    textures.dispose();
    textures.dispose();
    assert.ok(gpu.allocations.every((a) => a.destroyed === 1));
  });
});

test("unknown sources and absent fields stay inactive, while failed or nonfinite loads clean up earlier fields", async () => {
  const { base, manifest, files } = await fixture();
  const emptyGpu = mock(),
    empty = await BounceTextures.create(emptyGpu.device);
  assert.equal(empty.get("day"), empty.get("night"));
  assert.equal(emptyGpu.allocations[0].size, 16);
  assert.deepEqual([...empty.settings("day", [0, 1, 0])], [0, 0, 0, 0]);
  empty.dispose();
  const unknownGpu = mock();
  await withFetch(files, async () => {
    const unknown = await BounceTextures.create(
      unknownGpu.device,
      { day: base },
      { day: manifest },
    );
    assert.deepEqual([...unknown.settings("day", [0, 1, 0])], [1, 1, 0, 8]);
    unknown.dispose();
  });
  const invalid = new Float32Array(8 * 36);
  invalid[2] = NaN;
  const bytes = new Uint8Array(invalid.buffer),
    bad = structuredClone(manifest);
  bad.scenarios.night!.layers.sky.sha256 = await probeSha256(bytes);
  files.set(bad.scenarios.night!.layers.sky.url, bytes);
  const failedGpu = mock();
  await withFetch(files, async () => {
    await assert.rejects(
      BounceTextures.create(
        failedGpu.device,
        { day: base, night: base },
        { day: bad, night: bad },
      ),
      /finite/,
    );
    assert.ok(
      failedGpu.allocations.some(
        (a) => a.label === "day diffuse bounce layers",
      ),
    );
    assert.ok(failedGpu.allocations.every((a) => a.destroyed === 1));
  });
  const corrupted = structuredClone(manifest);
  corrupted.scenarios.day!.layers.sky.sha256 = "f".repeat(64);
  await withFetch(files, async () => {
    const gpu = mock();
    await assert.rejects(
      BounceTextures.create(gpu.device, { day: base }, { day: corrupted }),
      /incomplete/,
    );
    assert.ok(gpu.allocations.every((a) => a.destroyed === 1));
  });
});

test("constant fog binds its base field and appends an independent fourth upload without renaming legacy layers", async () => {
  const { base, manifest, files } = await fixture();
  base.scenarios.day!.constantFog = {
    url: "/private-assets/base-constant.bin",
    sha256: "e".repeat(64),
    bytes: 8 * 144,
  };
  const bytes = new Uint8Array(new Float32Array(8 * 36).fill(7).buffer),
    url = "/private-assets/day-constant.bin";
  files.set(url, bytes);
  manifest.scenarios.day!.constantFog = {
    url,
    sha256: await probeSha256(bytes),
    bytes: bytes.length,
  };
  manifest.base.identitySha256 = await hash(giBounceBaseIdentity(base));
  await validateBounceManifest(base, manifest, "day");
  const gpu = mock();
  await withFetch(files, async () => {
    const textures = await BounceTextures.create(
      gpu.device,
      { day: base, night: base },
      { day: manifest, night: manifest },
    );
    assert.equal(textures.hasConstantFog("day"), true);
    assert.equal(textures.hasConstantFog("night"), false);
    const day = gpu.allocations.find(
      (v) => v.label === "day diffuse bounce layers",
    )!;
    assert.equal(day.size, 8 * 144 * 4);
    for (let layer = 0; layer < 3; layer++)
      assert.ok(
        day
          .data!.subarray(layer * 8 * 36, (layer + 1) * 8 * 36)
          .every((v) => v === layer + 1),
      );
    assert.ok(day.data!.subarray(3 * 8 * 36).every((v) => v === 7));
    assert.equal(
      gpu.allocations.find((v) => v.label === "night diffuse bounce layers")!
        .size,
      8 * 144 * 3,
    );
    textures.dispose();
    assert.equal(textures.hasConstantFog("day"), false);
  });
  for (const damage of [
    "missing-base",
    "missing-bounce",
    "bytes",
    "stale-base",
  ] as const) {
    const b = structuredClone(base),
      m = structuredClone(manifest);
    if (damage === "missing-base") delete b.scenarios.day!.constantFog;
    if (damage === "missing-bounce") delete m.scenarios.day!.constantFog;
    if (damage === "bytes") m.scenarios.day!.constantFog!.bytes--;
    if (damage === "stale-base")
      b.scenarios.day!.constantFog!.sha256 = "f".repeat(64);
    await assert.rejects(validateBounceManifest(b, m, "day"));
  }
  const invalid = new Float32Array(8 * 36);
  invalid[0] = Infinity;
  const invalidBytes = new Uint8Array(invalid.buffer);
  manifest.scenarios.day!.constantFog!.sha256 = await probeSha256(invalidBytes);
  files.set(url, invalidBytes);
  const failed = mock();
  await withFetch(files, async () => {
    await assert.rejects(
      BounceTextures.create(failed.device, { day: base }, { day: manifest }),
      /finite/,
    );
    assert.ok(failed.allocations.every((v) => v.destroyed === 1));
  });
});
