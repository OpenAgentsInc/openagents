import assert from "node:assert/strict";
import { test } from "node:test";
import type { GiProbeManifest } from "../packages/renderer/gi-probes.ts";
import {
  validateProbeManifest,
  probeSettings,
  probeSha256,
  ProbeTextures,
} from "../packages/renderer/probe-textures.ts";

function fixture(): GiProbeManifest {
  return {
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
      url: "/private-assets/moments.bin",
      sha256: "b".repeat(64),
      bytes: 30 * 30 * 8,
      format: "rgba16float",
      width: 30,
      height: 30,
      interior: 8,
      tileSize: 10,
      tilesPerRow: 3,
      rayDistanceMeters: 64,
      distanceConvention: "mean-meters,mean-square-meters-squared",
    },
    scenarios: {
      day: {
        url: "/private-assets/day.bin",
        sha256: "c".repeat(64),
        bytes: 8 * 160,
        sourceEnvironmentSha256: "d".repeat(64),
      },
    },
    lighting: {
      convention: "world-sh9-lambert-E-over-pi",
      gainBaked: false,
      skyBounceCount: 0,
      rayCount: 128,
      alphaPolicy: "masked-solid-preview",
      lowerHemisphere: "world-up-black",
    },
  };
}

test("probe admission bounds grid allocation and rejects different radiance conventions", () => {
  assert.doesNotThrow(() => validateProbeManifest(fixture()));
  for (const mutate of [
    (v: GiProbeManifest) => {
      v.grid.count = 4097;
    },
    (v: GiProbeManifest) => {
      v.grid.spacing[0] = 0;
    },
    (v: GiProbeManifest) => {
      v.grid.origin[1] = NaN;
    },
    (v: GiProbeManifest) => {
      v.moments.height = 20;
    },
    (v: GiProbeManifest) => {
      v.records.shOffset = 8 as 4;
    },
    (v: GiProbeManifest) => {
      v.lighting.gainBaked = true as false;
    },
    (v: GiProbeManifest) => {
      v.scenarios.day!.bytes -= 4;
    },
  ]) {
    const value = fixture();
    mutate(value);
    assert.throws(() => validateProbeManifest(value));
  }
});

test("night geometry selects its own visibility atlas and disabled fields retain a valid grid", () => {
  const value = fixture();
  value.scenarios.night = {
    ...value.scenarios.day!,
    sourceGeometrySha256: "e".repeat(64),
    moments: {
      ...value.moments,
      width: 20,
      height: 40,
      tilesPerRow: 2,
      bytes: 20 * 40 * 8,
    },
  };
  assert.doesNotThrow(() => validateProbeManifest(value));
  assert.deepEqual(
    [...probeSettings(value, true, "night")],
    [0, 1, 0, 1, 4, 2, 4, 64, 2, 2, 2, 2, 20, 40, 10, 8],
  );
  assert.equal(probeSettings(value, false)[3], 0);
  assert.deepEqual([...probeSettings(undefined, true)], Array(16).fill(0));
  value.scenarios.night.moments!.bytes = 16;
  assert.throws(() => validateProbeManifest(value));
});

test("probe integrity hash uses only the requested byte range", async () => {
  const wrapped = new TextEncoder().encode("-abc-");
  assert.equal(
    await probeSha256(wrapped.subarray(1, 4)),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
  );
});

test("optional constant fog appends exactly nine coefficients per probe and rejects damaged data", async () => {
  Object.assign(globalThis, {
    GPUBufferUsage: { STORAGE: 1, COPY_DST: 2 },
    GPUTextureUsage: { COPY_DST: 1, TEXTURE_BINDING: 2 },
  });
  const make = async (constant: boolean) => {
    const manifest = fixture(),
      files = new Map<string, Uint8Array>();
    const moments = new Uint8Array(manifest.moments.bytes);
    manifest.moments.sha256 = await probeSha256(moments);
    files.set(manifest.moments.url, moments);
    const primary = new Float32Array(8 * 40);
    for (let p = 0; p < 8; p++) {
      primary.set([p, 1, 0, 1], p * 40);
      primary.fill(2, p * 40 + 4, p * 40 + 40);
    }
    const bytes = new Uint8Array(primary.buffer);
    manifest.scenarios.day!.sha256 = await probeSha256(bytes);
    files.set(manifest.scenarios.day!.url, bytes);
    if (constant) {
      const bytes = new Uint8Array(new Float32Array(8 * 36).fill(7).buffer),
        url = "/private-assets/constant.bin";
      manifest.scenarios.day!.constantFog = {
        url,
        bytes: bytes.length,
        sha256: await probeSha256(bytes),
      };
      files.set(url, bytes);
    }
    return { manifest, files, primary };
  };
  const mock = () => {
    const allocations: Array<{
      size?: number;
      destroyed: number;
      data?: Uint8Array;
    }> = [];
    const resource = (size?: number) => {
      const v = {
        size,
        destroyed: 0,
        data: undefined as Uint8Array | undefined,
        destroy() {
          v.destroyed++;
        },
      };
      allocations.push(v);
      return v;
    };
    return {
      allocations,
      device: {
        createTexture: () => resource(),
        createBuffer: (d: GPUBufferDescriptor) => resource(d.size),
        queue: {
          writeTexture() {},
          writeBuffer: (
            target: (typeof allocations)[number],
            _offset: number,
            data: Uint8Array,
          ) => {
            target.data = data.slice();
          },
        },
      } as unknown as GPUDevice,
    };
  };
  const original = globalThis.fetch;
  try {
    for (const has of [false, true]) {
      const f = await make(has),
        gpu = mock();
      globalThis.fetch = async (url) => {
        const data = f.files.get(String(url));
        return new Response(data?.slice().buffer, { status: data ? 200 : 404 });
      };
      const textures = await ProbeTextures.create(gpu.device, f.manifest),
        uploaded = (textures.scenarios.get("day") as any).data as Uint8Array;
      assert.equal(textures.hasConstantFog("day"), has);
      assert.equal(textures.hasConstantFog("night"), false);
      assert.equal(uploaded.byteLength, 8 * (160 + (has ? 144 : 0)));
      assert.deepEqual(
        uploaded.subarray(0, 8 * 160),
        new Uint8Array(f.primary.buffer),
      );
      if (has)
        assert.ok(
          new Float32Array(uploaded.buffer, 8 * 160).every((v) => v === 7),
        );
      textures.dispose();
      assert.equal(textures.hasConstantFog("day"), false);
      assert.ok(gpu.allocations.every((v) => v.destroyed === 1));
    }
    for (const damage of ["bytes", "hash", "nonfinite"] as const) {
      const f = await make(true),
        gpu = mock(),
        descriptor = f.manifest.scenarios.day!.constantFog!;
      if (damage === "bytes") descriptor.bytes -= 4;
      if (damage === "hash") descriptor.sha256 = "0".repeat(64);
      if (damage === "nonfinite") {
        const values = new Float32Array(8 * 36);
        values[0] = NaN;
        const bytes = new Uint8Array(values.buffer);
        descriptor.sha256 = await probeSha256(bytes);
        f.files.set(descriptor.url, bytes);
      }
      globalThis.fetch = async (url) => {
        const data = f.files.get(String(url));
        return new Response(data?.slice().buffer, { status: data ? 200 : 404 });
      };
      await assert.rejects(ProbeTextures.create(gpu.device, f.manifest));
      assert.ok(gpu.allocations.every((v) => v.destroyed === 1));
    }
  } finally {
    globalThis.fetch = original;
  }
});
