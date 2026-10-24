import assert from "node:assert/strict";
import { test } from "node:test";
import { MaterialTexturePools } from "../packages/renderer/texture-pool.ts";
import {
  copyTextureRequirements,
  planMaterialTexturePools,
  textureBucketBytes,
} from "../packages/renderer/texture-pool-plan.ts";
import type { LayerTexture } from "../packages/renderer/layers.ts";
const limits = { maxTextureArrayLayers: 256, maxTextureDimension2D: 8192 };
const refs = (
  ...entries: Array<[LayerTexture["role"], string]>
): LayerTexture[] => entries.map(([role, url]) => ({ role, url }));

test("material-local size classes duplicate only shared maps that need two resolutions", () => {
  const requests = new Map([
    ["small", refs(["data", "shared"])],
    ["large", refs(["data", "shared"], ["data", "large"])],
    ["other", refs(["normal", "normal"], ["color", "shared"])],
    ["empty", []],
  ]);
  const sizes = new Map([
    ["shared", { width: 2, height: 2 }],
    ["large", { width: 9, height: 3 }],
    ["normal", { width: 4, height: 4 }],
  ]);
  const plan = planMaterialTexturePools(requests, sizes, limits);
  assert.equal(plan.materials.get("small")!.data, "data:2");
  assert.equal(plan.materials.get("large")!.data, "data:16");
  assert.equal(plan.materials.get("other")!.normal, "normal:4");
  assert.equal(plan.materials.get("empty")!.data, "data:1");
  assert.deepEqual(plan.buckets.find((b) => b.key === "data:2")!.urls, [
    "shared",
  ]);
  assert.deepEqual(plan.buckets.find((b) => b.key === "data:16")!.urls, [
    "large",
    "shared",
  ]);
  assert.equal(plan.buckets.find((b) => b.key === "normal:4")!.size, 4);
  assert.equal(
    plan.decodedBytes,
    (4 + 27 + 16) * 4,
    "One shared source is decoded once across roles and buckets.",
  );
  assert.equal(
    plan.uploadBytes,
    (4 + 27 + 16 + 4) * 4 + 12,
    "The color/data GPU uploads have separate formats.",
  );
  assert.equal(
    plan.bytes,
    plan.buckets.reduce(
      (s, b) => s + textureBucketBytes(b.size, b.urls.length + 1),
      0,
    ),
  );
  assert.equal(plan.peakGpuBytes, plan.bytes + plan.uploadBytes);
  const reversed = planMaterialTexturePools(
    new Map([...requests].reverse()),
    sizes,
    limits,
  );
  assert.deepEqual(
    reversed.buckets,
    plan.buckets,
    "URL indices do not depend on material traversal order.",
  );
});

test("GPU allocation counts exact mips, respects device layers, and bounds decoded/upload memory", () => {
  assert.equal(textureBucketBytes(1, 1), 4);
  assert.equal(textureBucketBytes(4, 3), (16 + 4 + 1) * 4 * 3);
  const requests = new Map([
      ["material", refs(["normal", "a"], ["normal", "b"])],
    ]),
    sizes = new Map([
      ["a", { width: 16, height: 16 }],
      ["b", { width: 16, height: 16 }],
    ]);
  assert.throws(
    () =>
      planMaterialTexturePools(requests, sizes, {
        ...limits,
        maxTextureArrayLayers: 2,
      }),
    /array-layer/,
  );
  assert.throws(
    () =>
      planMaterialTexturePools(requests, sizes, {
        ...limits,
        maxTextureDimension2D: 8,
      }),
    /dimensions/,
  );
  assert.throws(
    () =>
      planMaterialTexturePools(requests, sizes, limits, {
        maxDecodedBytes: 2047,
      }),
    /Decoded/,
  );
  const plan = planMaterialTexturePools(requests, sizes, limits);
  assert.throws(
    () =>
      planMaterialTexturePools(requests, sizes, limits, {
        maxGpuBytes: plan.peakGpuBytes - 1,
      }),
    /GPU byte budget/,
  );
  assert.doesNotThrow(() =>
    planMaterialTexturePools(requests, sizes, limits, {
      maxGpuBytes: plan.peakGpuBytes,
    }),
  );
  const capped = planMaterialTexturePools(
    new Map([["large", refs(["data", "source4k"])]]),
    new Map([["source4k", { width: 4096, height: 4096 }]]),
    limits,
  );
  assert.equal(capped.buckets.find((b) => b.key === "data:2048")!.size, 2048);
  assert.equal(capped.decodedBytes, 4096 * 4096 * 4);
});

test("planner rejects invalid roles, references, dimensions, and excessive collections", () => {
  assert.throws(
    () =>
      copyTextureRequirements(
        new Map([
          ["bad", [{ role: "other", url: "map" } as unknown as LayerTexture]],
        ]),
      ),
    /role/,
  );
  assert.throws(
    () => copyTextureRequirements(new Map([["bad", refs(["color", ""])]])),
    /URL/,
  );
  assert.throws(
    () =>
      copyTextureRequirements(
        new Map(Array.from({ length: 4097 }, (_, i) => [`m${i}`, []])),
      ),
    /4096/,
  );
  assert.throws(
    () =>
      copyTextureRequirements(
        new Map([
          [
            "many",
            Array.from({ length: 2049 }, (_, i) => ({
              role: "data" as const,
              url: `m${i}`,
            })),
          ],
        ]),
      ),
    /2048/,
  );
  assert.throws(
    () =>
      planMaterialTexturePools(
        new Map([["bad", refs(["data", "missing"])]]),
        new Map(),
        limits,
      ),
    /missing/,
  );
  assert.throws(
    () =>
      planMaterialTexturePools(
        new Map([["bad", refs(["data", "image"])]]),
        new Map([["image", { width: Number.NaN, height: 1 }]]),
        limits,
      ),
    /dimensions/,
  );
  assert.throws(() => textureBucketBytes(3, 1));
  const original = refs(["data", "map"]),
    copied = copyTextureRequirements(new Map([["m", original]]));
  original[0].url = "changed";
  original.push({ role: "normal", url: "new" });
  assert.deepEqual(copied.get("m"), [{ role: "data", url: "map" }]);
});

function pngHeader(width: number, height: number): Uint8Array<ArrayBuffer> {
  const bytes = new Uint8Array(24),
    view = new DataView(bytes.buffer);
  view.setUint32(0, 0x89504e47);
  view.setUint32(4, 0x0d0a1a0a);
  view.setUint32(16, width);
  view.setUint32(20, height);
  return bytes;
}

test("shared PNG loads and decodes once and closes its bitmap when GPU planning rejects", async () => {
  const oldFetch = globalThis.fetch,
    oldBitmap = globalThis.createImageBitmap;
  let fetches = 0,
    decodes = 0,
    closes = 0;
  try {
    globalThis.fetch = async () => {
      fetches++;
      return new Response(pngHeader(2, 2));
    };
    globalThis.createImageBitmap = (async () => {
      decodes++;
      return {
        width: 2,
        height: 2,
        close: () => {
          closes++;
        },
      };
    }) as unknown as typeof createImageBitmap;
    const requirements = new Map([
      ["one", refs(["color", "shared.png"], ["data", "shared.png"])],
      ["two", refs(["normal", "shared.png"])],
    ]);
    await assert.rejects(
      MaterialTexturePools.create({ limits } as GPUDevice, requirements, {
        maxGpuBytes: 1,
      }),
      /GPU byte budget/,
    );
    assert.equal(fetches, 1);
    assert.equal(decodes, 1);
    assert.equal(closes, 1);
  } finally {
    globalThis.fetch = oldFetch;
    globalThis.createImageBitmap = oldBitmap;
  }
});

test("decode failure drains workers and closes successful bitmaps before allocating GPU resources", async () => {
  const oldFetch = globalThis.fetch,
    oldBitmap = globalThis.createImageBitmap;
  let closes = 0,
    allocations = 0;
  try {
    globalThis.fetch = async (url) =>
      new Response(
        String(url).endsWith(".png") ? pngHeader(1, 1) : new Uint8Array(8),
      );
    globalThis.createImageBitmap = (async () => ({
      width: 1,
      height: 1,
      close: () => {
        closes++;
      },
    })) as unknown as typeof createImageBitmap;
    const device = {
      limits,
      createTexture: () => {
        allocations++;
        throw new Error("Unexpected allocation");
      },
    } as unknown as GPUDevice;
    await assert.rejects(
      MaterialTexturePools.create(
        device,
        new Map([["m", refs(["color", "good.png"], ["data", "bad.bin"])]]),
      ),
      /header/,
    );
    assert.equal(closes, 1);
    assert.equal(allocations, 0);
  } finally {
    globalThis.fetch = oldFetch;
    globalThis.createImageBitmap = oldBitmap;
  }
});
