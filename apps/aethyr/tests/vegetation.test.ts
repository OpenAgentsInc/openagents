import assert from "node:assert/strict";
import test from "node:test";
import type { VegetationMaterial } from "../packages/assets/src/scene.ts";
import {
  collectVegetationTextures,
  packVegetation,
  VEGETATION_FLOAT_COUNT,
  VEGETATION_SURFACE_SKY_MARKER,
  VEGETATION_TRANSMISSION_OFFSET,
} from "../packages/renderer/vegetation.ts";
import { WIND_FLUTTER_CLOCK_SCALE } from "../packages/renderer/wind-flutter.ts";

function fixture(): { vegetation: VegetationMaterial } {
  return {
    vegetation: {
      model: "ue-mm-vegetation-1-v1",
      baseColorUrl: "color",
      normalUrl: "normal",
      mraUrl: "packed",
      mask: {
        url: "mask",
        sourceColorSpace: "linear",
        enabled: false,
        color: [0.3, 0.5, 0.7],
      },
      tint: [1, 0.5, 0.25],
      color: {
        luminance: [0.2, 0.7, 0.1],
        desaturation: 0.05,
        contrast: 1,
        intensity: 1.1,
        overlayColor: [0.1, 0.2, 0.3],
        overlayStrength: 0,
      },
      gradient: { axis: "v", intensity: 0.3 },
      aoAffectsColorIntensity: 1.2,
      normalFlatness: 0,
      roughnessIntensity: 1,
      specularAmount: 0.3,
      fuzz: { coreDarkness: 0.5, power: 1, edgeBrightness: 0.1 },
      subsurface: { color: [0.8, 1, 0.6], intensity: 0.25 },
      alphaCutoff: 0.3333,
      twoSided: true,
      lightingModel: "two-sided-foliage",
      sourceWindEnabled: true,
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

test("vegetation packs full normal detail at flatness zero and source coverage threshold", () => {
  const map: Record<string, number> = {
    color: 1,
    normal: 2,
    packed: 3,
    mask: 4,
  };
  const values = packVegetation(fixture(), (url) => map[url]!);
  const legacyBytes = new Float32Array([
    1, 1, 0, 0.3333, 1, 2, 3, 0, 1, 0.5, 0.25, 0.05, 0.1, 0.2, 0.3, 0, 0.2, 0.7,
    0.1, 1, 1.1, 1.2, 0.3, 0, 0.5, 1, 0.1, 1, 0.3, 0.5, 0.7, 0.3, 0.8, 1, 0.6,
    0.25, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  ]);
  assert.deepEqual(
    new Uint8Array(values.buffer),
    new Uint8Array(legacyBytes.buffer),
  );
  assert.equal(values.length, VEGETATION_FLOAT_COUNT);
  assert.equal(values.byteLength, 12 * 16);
  assert.deepEqual([...values.slice(0, 3)], [1, 1, 0]);
  assert.ok(Math.abs(values[3]! - 0.3333) < 1e-7);
  assert.deepEqual([...values.slice(4, 8)], [1, 2, 3, 0]);
  assert.equal(values[23], 0);
  assert.deepEqual([...values.slice(36, 40)], [0, 1, 1, 0]);
  assert.ok(values.slice(40).every((n) => n === 0));
});

test("optional flutter leaves the SurfaceSky marker at vegetationFlags.w", () => {
  const old = packVegetation(fixture(), () => 0),
    flutter = packVegetation(fixture(), () => 0, {
      profile: "source-triangle-flutter-v1",
      intensity: 0.15,
      weight: 0.3,
      clockScale: WIND_FLUTTER_CLOCK_SCALE,
      smallWaveCm: 1,
      largeWaveCm: 200,
      vertexLod: 0,
    });
  assert.equal(VEGETATION_SURFACE_SKY_MARKER, 39);
  assert.deepEqual(flutter.slice(0, 40), old.slice(0, 40));
  flutter[VEGETATION_SURFACE_SKY_MARKER] = 7;
  assert.deepEqual([...flutter.slice(36, 41)], [0, 1, 1, 7, 1]);
  assert.equal(flutter[43], WIND_FLUTTER_CLOCK_SCALE);
  assert.deepEqual([...flutter.slice(44)], [1, 200, 0, 0]);
});

test("optional transmission uses only spare float47 and preserves legacy bytes", () => {
  const material = fixture();
  const baseline = packVegetation(material, () => 0);
  material.vegetation.transmission = undefined;
  assert.deepEqual(
    packVegetation(material, () => 0),
    baseline,
  );
  material.vegetation.transmission = {
    model: "bounded-forward-sheet-v1",
    anisotropy: 0.5,
  };
  const enabled = packVegetation(material, () => 0);
  assert.equal(VEGETATION_TRANSMISSION_OFFSET, 47);
  assert.equal(enabled.length, 48);
  assert.deepEqual(enabled.slice(0, 47), baseline.slice(0, 47));
  assert.equal(enabled[47], 1.5);
  const flutter = {
    profile: "source-triangle-flutter-v1" as const,
    intensity: 0.15,
    weight: 0.3,
    clockScale: WIND_FLUTTER_CLOCK_SCALE,
    smallWaveCm: 1 as const,
    largeWaveCm: 200 as const,
    vertexLod: 0 as const,
  };
  const both = packVegetation(material, () => 0, flutter);
  material.vegetation.transmission = undefined;
  const flutterOnly = packVegetation(material, () => 0, flutter);
  assert.deepEqual(both.slice(0, 47), flutterOnly.slice(0, 47));
  assert.deepEqual([...both.slice(44)], [1, 200, 0, 1.5]);
  assert.equal(both[VEGETATION_SURFACE_SKY_MARKER], 0);
});

test("transmission profiles are checked before requesting texture uploads", () => {
  const material = fixture();
  material.vegetation.transmission = {
    model: "bounded-forward-sheet-v1",
    anisotropy: Infinity,
  };
  let lookups = 0;
  assert.throws(
    () =>
      packVegetation(material, () => {
        lookups++;
        return 0;
      }),
    /transmission/,
  );
  assert.equal(lookups, 0);
});

test("disabled masks allocate no maps, while enabled color-space roles stay explicit", () => {
  const material = fixture();
  assert.deepEqual(collectVegetationTextures(material), [
    { url: "color", role: "color" },
    { url: "normal", role: "normal" },
    { url: "packed", role: "data" },
  ]);
  material.vegetation.mask.enabled = true;
  material.vegetation.mask.sourceColorSpace = "srgb";
  assert.deepEqual(collectVegetationTextures(material).at(-1), {
    url: "mask",
    role: "color",
  });
});

test("subsurface and fuzz retain separate source controls", () => {
  const values = packVegetation(fixture(), () => 0);
  assert.deepEqual([...values.slice(24, 26)], [0.5, 1]);
  assert.ok(Math.abs(values[35]! - 0.25) < 1e-7);
  assert.ok(Math.abs(values[33]! - 1) < 1e-7);
});

test("vegetation rejects invalid uploads and does no work for absent profiles", () => {
  assert.ok(
    packVegetation({}, () => {
      throw Error("lookup");
    }).every((value) => value === 0),
  );
  assert.throws(() => packVegetation(fixture(), () => -1), /pool index/);
  const material = fixture();
  material.vegetation.fuzz.power = Infinity;
  assert.throws(() => packVegetation(material, () => 0), /finite/);
});
