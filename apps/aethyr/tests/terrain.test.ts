import assert from "node:assert/strict";
import test from "node:test";
import type { TerrainLayerMaterial } from "../packages/assets/src/scene.ts";
import {
  collectTerrainTextures,
  packTerrain,
  TERRAIN_FLOAT_COUNT,
} from "../packages/renderer/terrain.ts";

function fixture(): { terrain: TerrainLayerMaterial } {
  const slot = {
    baseColorUrl: "color",
    normalUrl: "normal",
    mraUrl: "packed",
    colorFactor: [1, 1, 1, 1] as [number, number, number, number],
    emissiveIntensity: 0,
    emissiveColorSpace: "linear" as const,
  };
  return {
    terrain: {
      model: "ue-mm-blend-3-v1",
      slots: [{ ...slot }, { ...slot }, { ...slot }],
      uv: { mode: "objectXY", scale: [1, 1] },
      height: {
        url: "height",
        sourceColorSpace: "linear",
        uvScale: [2, 3],
        contrast: 1,
        useCustom: true,
        customIntensity: 1,
      },
      wetness: {
        url: "wet",
        sourceColorSpace: "srgb",
        colorIntensity: 2,
        roughnessIntensity: 0.8,
      },
      parallax: {
        enabled: true,
        url: "parallax",
        sourceColorSpace: "linear",
        referencePlane: 0.5,
        heightRatio: 0.03,
        uvSet: 0,
      },
      baseRoughnessIntensity: 0.6,
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

test("terrain packs three independent layers and explicit source parallax controls", () => {
  const map: Record<string, number> = {
    color: 1,
    normal: 2,
    packed: 3,
    height: 4,
    wet: 5,
    parallax: 6,
  };
  const values = packTerrain(fixture(), (url) => map[url]!);
  assert.equal(values.length, TERRAIN_FLOAT_COUNT);
  assert.equal(values.byteLength, 12 * 16);
  assert.deepEqual(
    [...values.slice(20, 36)],
    [1, 2, 3, 1, 2, 3, 1, 2, 3, 4, 5, 6, 0, 0, 0, 1],
  );
  assert.equal(values[40], 1);
  assert.equal(values[41], 0.5);
  assert.ok(Math.abs(values[42]! - 0.03) < 1e-8);
  assert.equal(values[37], 2);
});

test("disabled parallax and zero emission omit their texture work", () => {
  const material = fixture();
  material.terrain.parallax.enabled = false;
  material.terrain.slots[0].emissiveUrl = "unused";
  assert.ok(
    !collectTerrainTextures(material).some(
      (value) => value.url === "unused" || value.url === "parallax",
    ),
  );
  packTerrain(material, (url) => {
    assert.notEqual(url, "unused");
    assert.notEqual(url, "parallax");
    return 0;
  });
});

test("terrain texture collection keeps wet color and height data in separate pools", () => {
  const refs = collectTerrainTextures(fixture());
  assert.ok(
    refs.some((value) => value.url === "wet" && value.role === "color"),
  );
  assert.ok(
    refs.some((value) => value.url === "height" && value.role === "data"),
  );
  assert.equal(refs.filter((value) => value.url === "color").length, 1);
});

test("terrain rejects invalid indices and nonfinite source values", () => {
  assert.ok(
    packTerrain({}, () => {
      throw Error("lookup");
    }).every((value) => value === 0),
  );
  assert.throws(() => packTerrain(fixture(), () => -1), /pool index/);
  const material = fixture();
  material.terrain.parallax.heightRatio = NaN;
  assert.throws(() => packTerrain(material, () => 0), /finite/);
});
