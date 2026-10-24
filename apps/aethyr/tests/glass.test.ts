import assert from "node:assert/strict";
import test from "node:test";
import type {
  FlameMaterial,
  GlassMaterial,
} from "../packages/assets/src/scene.ts";
import {
  collectGlassTextures,
  GLASS_FLOAT_COUNT,
  packGlass,
} from "../packages/renderer/glass.ts";

function fixture(): { glass: GlassMaterial } {
  return {
    glass: {
      model: "ue-mm-glass-v1",
      color: [0.25, 0.5, 0.75],
      dirtColor: [0.1, 0.2, 0.3],
      colorAmount: 0.1,
      metallic: 0.8,
      uvScale: [2, 3],
      opacity: {
        base: 1.5,
        side1: 0.2,
        side2: 0.5,
        fresnelPower: 2,
        useTexture: true,
        textureValue: 0.7,
      },
      roughness: {
        constant: 0.2,
        main: 0.3,
        intensity: 5,
        power: 0.85,
        useDirt: true,
      },
      normal: {
        enabled: false,
        constant: [0.212231, 0.212231, 1],
        flatness: 1,
      },
      dirt: {
        url: "mask",
        sourceColorSpace: "linear",
        useForColor: true,
        world: {
          url: "world",
          sourceColorSpace: "srgb",
          sizeMeters: 4,
          power: 1.2,
          channel: "g",
          projection: "ue-world-aligned-v1",
        },
      },
      textureColor: [0.3, 0.2, 0.1],
      emissiveColor: [3, 2, 1],
      emissiveIntensity: 10,
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

function flame(): { flame: FlameMaterial } {
  return {
    flame: {
      model: "ue-mm-flame-v1",
      colorUrl: "flame",
      sourceColorSpace: "srgb",
      tint: [0.2, 0.4, 0.6],
      emissiveIntensity: 10000,
      opacityChannel: "a",
      blend: "additive",
      shading: "unlit",
      sourceUnits: "unreal-material-emissive",
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

test("surface profiles preserve raw source values and append aligned fields", () => {
  const packed = packGlass(fixture(), (_url, role) =>
    role === "color" ? 4 : 2,
  );
  assert.equal(packed.length, GLASS_FLOAT_COUNT);
  assert.equal(packed.byteLength, 13 * 16);
  assert.deepEqual([...packed.slice(0, 4)], [1, 0, 1, 1]);
  assert.equal(packed[12], 1.5);
  assert.deepEqual([...packed.slice(24, 28)], [2, 4, 0, 0]);
  assert.deepEqual([...packed.slice(48, 52)], [0, 1, 0, 1]);
  assert.ok(Math.abs(packed[20]! - 0.212231) < 1e-7);
});

test("flame is a separate unlit profile with raw emission and preserved alpha map", () => {
  const packed = packGlass(flame(), () => 7);
  assert.deepEqual([...packed.slice(0, 4)], [0, 1, 0, 0]);
  assert.equal(packed[27], 7);
  assert.equal(packed[47], 10000);
  assert.deepEqual(collectGlassTextures(flame()), [
    { url: "flame", role: "color" },
  ]);
});

test("surface maps keep declared color spaces and omit disabled normal maps", () => {
  const material = fixture();
  material.glass.normal.url = "normal";
  assert.deepEqual(collectGlassTextures(material), [
    { url: "mask", role: "data" },
    { url: "world", role: "color" },
  ]);
  material.glass.normal.enabled = true;
  assert.deepEqual(collectGlassTextures(material).at(-1), {
    url: "normal",
    role: "normal",
  });
});

test("invalid surface combinations, pool indices, and nonfinite parameters are rejected", () => {
  assert.throws(
    () => packGlass({ ...fixture(), ...flame() }, () => 0),
    /together/,
  );
  assert.throws(() => packGlass(fixture(), () => -1), /pool index/);
  assert.throws(() => packGlass(fixture(), () => 0.5), /pool index/);
  const material = fixture();
  material.glass.opacity.fresnelPower = NaN;
  assert.throws(() => packGlass(material, () => 0), /finite/);
  assert.ok(
    packGlass({}, () => {
      throw Error("lookup");
    }).every((value) => value === 0),
  );
});
