import assert from "node:assert/strict";
import test from "node:test";
import {
  collectLayerTextures,
  LAYER_FLOAT_COUNT,
  packLayers,
} from "../packages/renderer/layers.ts";
import type { TwoLayerMaterial } from "../packages/assets/src/scene.ts";

function fixture(): { layers: TwoLayerMaterial } {
  return {
    layers: {
      model: "ue-mm-blend-2-v1",
      slots: [
        {
          baseColorUrl: "a",
          normalUrl: "n",
          mraUrl: "m",
          colorFactor: [1, 0, 0, 1],
        },
        {
          baseColorUrl: "b",
          normalUrl: "n",
          mraUrl: "m",
          colorFactor: [0, 0, 1, 1],
        },
      ],
      uv: { mode: "objectXY", scale: [1, 1] },
      blend: {
        vertexChannel: "r",
        phase: 0.7,
        contrast: 0.5,
        height: {
          mode: "layer2Alpha",
          customIntensity: 1,
          sourceColorSpace: "linear",
        },
        worldSubtract: {
          url: "world",
          sourceColorSpace: "srgb",
          sizeMeters: 8,
          intensity: 0,
          power: 1,
          channel: "r",
          projection: "ue-world-aligned-v1",
        },
      },
      detailNormal: { url: "n", uvScale: [3, 3], flatness: 1 },
      colorIntensity: 1,
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

test("absent layer profile produces a disabled payload and no pool lookups", () => {
  const packed = packLayers({}, () => {
    throw Error("unexpected texture lookup");
  });
  assert.equal(packed.length, LAYER_FLOAT_COUNT);
  assert.ok(packed.every((value) => value === 0));
  assert.deepEqual(collectLayerTextures({}), []);
});

test("packing preserves typed indices and independent detail UV fields", () => {
  const packed = packLayers(
    fixture(),
    (url, role) =>
      (
        ({
          "color:a": 7,
          "color:b": 8,
          "color:world": 9,
          "normal:n": 3,
          "data:m": 4,
        }) as Record<string, number>
      )[`${role}:${url}`]!,
  );
  assert.equal(packed.byteLength, 9 * 16);
  assert.equal(packed[0], 1);
  assert.equal(packed[1], 1);
  assert.deepEqual([...packed.slice(16, 24)], [7, 3, 4, 8, 3, 4, 0, 9]);
  assert.deepEqual(
    [...packed.slice(24, 36)],
    [0, 1, 8, 1, 0, 1, 3, 1, 3, 3, 0, 0],
  );
  assert.deepEqual([...packed.slice(8, 16)], [1, 0, 0, 1, 0, 0, 1, 1]);
});

test("texture collection deduplicates within roles and keeps color masks in sRGB pool", () => {
  const material = fixture();
  const refs = collectLayerTextures(material);
  assert.deepEqual(refs, [
    { url: "a", role: "color" },
    { url: "n", role: "normal" },
    { url: "m", role: "data" },
    { url: "b", role: "color" },
    { url: "world", role: "color" },
  ]);
  material.layers.blend.height = {
    mode: "maskRed",
    url: "m",
    customIntensity: 1,
    sourceColorSpace: "srgb",
  };
  assert.ok(
    collectLayerTextures(material).some(
      (item) => item.url === "m" && item.role === "color",
    ),
  );
  assert.ok(
    collectLayerTextures(material).some(
      (item) => item.url === "m" && item.role === "data",
    ),
  );
});

test("pool indices and numeric material parameters cannot upload NaN or invalid integers", () => {
  assert.throws(() => packLayers(fixture(), () => -1), /pool index/);
  assert.throws(() => packLayers(fixture(), () => 0.5), /pool index/);
  const material = fixture();
  material.layers.blend.contrast = NaN;
  assert.throws(() => packLayers(material, () => 0), /finite/);
});
