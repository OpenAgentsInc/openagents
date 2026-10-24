import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ProbeProfiles,
  selectProbeProfile,
} from "../packages/renderer/probe-profiles.ts";
import { probeSha256 } from "../packages/renderer/probe-textures.ts";
Object.assign(globalThis, {
  GPUTextureUsage: { COPY_DST: 1, TEXTURE_BINDING: 2 },
  GPUBufferUsage: { COPY_DST: 1, STORAGE: 2 },
});
async function fixture(mode: "day" | "night", count = 8) {
  const dim: [number, number, number] = [2, count / 4, 2],
    width = 20,
    height = Math.ceil(count / 2) * 10;
  const moments = new Uint8Array(width * height * 8),
    data = new Float32Array(count * 40);
  for (let i = 0; i < count; i++) data[i * 40 + 3] = 1;
  const bytes = new Uint8Array(data.buffer);
  const momentHash = await probeSha256(moments),
    recordHash = await probeSha256(bytes);
  const atlas = {
    url: `/private-assets/${mode}-moments.bin`,
    sha256: momentHash,
    bytes: moments.length,
    format: "rgba16float" as const,
    width,
    height,
    interior: 8 as const,
    tileSize: 10 as const,
    tilesPerRow: 2,
    rayDistanceMeters: 64,
    distanceConvention: "mean-meters,mean-square-meters-squared" as const,
  };
  const record = {
    url: `/private-assets/${mode}-records.bin`,
    sha256: recordHash,
    bytes: bytes.length,
    sourceEnvironmentSha256: "e".repeat(64),
    sourceGeometrySha256: "f".repeat(64),
    ...(mode === "night" ? { moments: atlas } : {}),
  };
  const manifest = {
    schema: "aethyrforge.private-sky-probes.v1" as const,
    profile: "occluded-sky-sh9-v1" as const,
    sourceGeometrySha256: "a".repeat(64),
    grid: {
      origin: [0, 1, 0] as [number, number, number],
      spacing: [4, 2, 4] as [number, number, number],
      dimensions: dim,
      count,
    },
    records: {
      strideFloats: 40 as const,
      positionValidOffset: 0 as const,
      shOffset: 4 as const,
    },
    moments:
      mode === "night"
        ? {
            ...atlas,
            url: "/private-assets/unused-day-atlas.bin",
            sha256: "b".repeat(64),
          }
        : atlas,
    scenarios: {
      [mode]: record,
      [mode === "day" ? "night" : "day"]: {
        url: "/private-assets/poison.bin",
        bytes: 0,
        sha256: "broken",
        sourceEnvironmentSha256: "broken",
      },
    },
    lighting: {
      convention: "world-sh9-lambert-E-over-pi" as const,
      gainBaked: false as const,
      skyBounceCount: 0 as const,
      rayCount: 128,
      alphaPolicy:
        mode === "night"
          ? ("alpha-tested-material-v1" as const)
          : ("masked-solid-preview" as const),
      lowerHemisphere: "world-up-black" as const,
    },
  };
  return {
    manifest,
    files: new Map([
      [atlas.url, moments],
      [record.url, bytes],
    ]),
  };
}
function device() {
  const allocations: { destroyed: number; label: string }[] = [];
  const allocate = (d: any) => {
    const item = { destroyed: 0, label: d.label };
    allocations.push(item);
    return {
      ...d,
      destroy() {
        item.destroyed++;
      },
      createView() {
        return {};
      },
    };
  };
  return {
    allocations,
    value: {
      createTexture: allocate,
      createBuffer: allocate,
      queue: { writeTexture() {}, writeBuffer() {} },
    } as unknown as GPUDevice,
  };
}
test("independent profiles load only selected records and moments, preserving grid and alpha policy", async () => {
  const day = await fixture("day"),
    night = await fixture("night", 12),
    mock = device(),
    requests: string[] = [],
    original = globalThis.fetch;
  const files = new Map([...day.files, ...night.files]);
  globalThis.fetch = async (input: any) => {
    const url = String(input);
    requests.push(url);
    const data = files.get(url);
    return new Response(data?.slice().buffer, { status: data ? 200 : 404 });
  };
  try {
    const profiles = await ProbeProfiles.create(mock.value, {
      day: day.manifest,
      night: night.manifest,
    });
    assert.equal(profiles.manifest("day")!.grid.count, 8);
    assert.equal(profiles.manifest("night")!.grid.count, 12);
    assert.equal(
      profiles.manifest("night")!.lighting.alphaPolicy,
      "alpha-tested-material-v1",
    );
    assert.deepEqual([...profiles.get("day").scenarios.keys()], ["day"]);
    assert.deepEqual([...profiles.get("night").scenarios.keys()], ["night"]);
    assert.equal(profiles.get("night").scenarioMoments.size, 0);
    assert.equal(
      profiles.manifest("night")!.moments.url,
      "/private-assets/night-moments.bin",
    );
    assert.deepEqual(new Set(requests), new Set(files.keys()));
    assert.equal(requests.length, 4);
    assert.ok(profiles.bytes > 168);
    profiles.dispose();
    profiles.dispose();
    assert.ok(mock.allocations.every((a) => a.destroyed === 1));
  } finally {
    globalThis.fetch = original;
  }
});
test("missing fields share one fallback; missing requested scenarios and partial load failures clean up", async () => {
  const mock = device(),
    empty = await ProbeProfiles.create(mock.value);
  assert.equal(empty.get("day"), empty.get("night"));
  assert.equal(empty.bytes, 168);
  assert.equal(empty.manifest("day"), undefined);
  empty.dispose();
  const day = await fixture("day"),
    night = await fixture("night");
  assert.throws(
    () => selectProbeProfile({ ...night.manifest, scenarios: {} }, "night"),
    /night town/,
  );
  const invalid = device();
  await assert.rejects(
    ProbeProfiles.create(invalid.value, {
      day: day.manifest,
      night: { ...night.manifest, scenarios: {} },
    }),
    /night town/,
  );
  assert.equal(invalid.allocations.length, 0);
  const failed = device(),
    original = globalThis.fetch;
  globalThis.fetch = async (input: any) => {
    const data = day.files.get(String(input));
    return new Response(data?.slice().buffer, { status: data ? 200 : 404 });
  };
  try {
    await assert.rejects(
      ProbeProfiles.create(failed.value, {
        day: day.manifest,
        night: night.manifest,
      }),
      /could not open/,
    );
    assert.ok(failed.allocations.every((a) => a.destroyed === 1));
    assert.ok(
      failed.allocations.some((a) => a.label === "day occluded sky probes"),
    );
  } finally {
    globalThis.fetch = original;
  }
});
