import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { colorTemperatureRec709 } from "../packages/renderer/color-temperature.ts";
import {
  lightColor,
  lightIntensity,
  validateLightTemperatureModel,
} from "../packages/renderer/lighting.ts";
import { giBounceLocalLightsIdentityPayload } from "../packages/renderer/gi-bounce.ts";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import type { V3 } from "../packages/renderer/math.ts";

const LUMINANCE: V3 = [
  0.21263900587151027, 0.715168678767756, 0.07219231536073371,
];
const luminance = (rgb: V3) =>
  rgb.reduce((sum, channel, i) => sum + channel * LUMINANCE[i], 0);

test("temperature conversion matches independently evaluated public colorimetry", () => {
  // Krystek UCS → xy → XYZ(Y=1) → BT.709/D65; these are mathematical fixtures,
  // not measurements of a renderer, spectral integration, or gallery lighting.
  const anchors: readonly [number, V3][] = [
    [1000, [4.600247129380747, 0.03975310103254825, 0]],
    [3000, [1.7699310519705815, 0.8447036622338998, 0.2706384811198751]],
    [3500, [1.5641821059841012, 0.8923015058138591, 0.4051370683986489]],
    [4000, [1.4146768561057572, 0.9237257166135044, 0.5341956301201469]],
    [6500, [1.0441551909367264, 0.9832685419219306, 1.0356921488086828]],
    [15000, [0.7846186576734108, 0.9925268280476864, 1.7084279368431907]],
  ];
  for (const [kelvin, expected] of anchors) {
    const actual = colorTemperatureRec709(kelvin);
    actual.forEach((value, i) =>
      assert.ok(
        Math.abs(value - expected[i]) < 2e-12,
        `${kelvin} K channel ${i}: ${value} versus ${expected[i]}`,
      ),
    );
  }
});

test("positive in-gamut light retains unit luminance without peak normalization", () => {
  for (let kelvin = 2500; kelvin <= 15000; kelvin += 25) {
    const rgb = colorTemperatureRec709(kelvin);
    assert.ok(rgb.every((value) => value > 0));
    assert.ok(Math.abs(luminance(rgb) - 1) < 2e-14);
  }
  assert.ok(colorTemperatureRec709(3500)[0] > 1.5);
  assert.ok(colorTemperatureRec709(15000)[2] > 1.7);
});

test("negative gamut channels clamp without a second luminance normalization", () => {
  const rgb = colorTemperatureRec709(1000);
  assert.equal(rgb[2], 0);
  assert.ok(rgb[0] > 4);
  assert.ok(luminance(rgb) > 1.006);
  assert.ok(luminance(rgb) < 1.007);
});

test("6500 K retains the Planckian chromaticity instead of forcing D65 white", () => {
  const rgb = colorTemperatureRec709(6500);
  assert.notDeepEqual(rgb, [1, 1, 1]);
  assert.ok(rgb[0] > 1 && rgb[1] < 1 && rgb[2] > 1);
  assert.ok(Math.abs(luminance(rgb) - 1) < 2e-14);
});

test("the full supported range is finite and continuous, including gamut clipping", () => {
  let previous = colorTemperatureRec709(1000);
  let firstPositiveBlue = 0;
  for (let kelvin = 1001; kelvin <= 15000; kelvin++) {
    const rgb = colorTemperatureRec709(kelvin);
    rgb.forEach((value, i) => {
      assert.ok(Number.isFinite(value) && value >= 0);
      assert.ok(Math.abs(value - previous[i]) < 0.01);
    });
    if (!firstPositiveBlue && rgb[2] > 0) firstPositiveBlue = kelvin;
    previous = rgb;
  }
  assert.ok(firstPositiveBlue > 1800 && firstPositiveBlue < 2500);
  assert.ok(colorTemperatureRec709(firstPositiveBlue)[2] < 0.001);
  assert.notDeepEqual(
    colorTemperatureRec709(12000),
    colorTemperatureRec709(15000),
  );
});

test("finite temperatures clamp to the fit domain and nonfinite inputs fail", () => {
  for (const value of [-Number.MAX_VALUE, -1, 0, 999])
    assert.deepEqual(
      colorTemperatureRec709(value),
      colorTemperatureRec709(1000),
    );
  for (const value of [15001, Number.MAX_VALUE])
    assert.deepEqual(
      colorTemperatureRec709(value),
      colorTemperatureRec709(15000),
    );
  for (const value of [Number.NaN, Infinity, -Infinity])
    assert.throws(
      () => colorTemperatureRec709(value),
      /finite color temperature/,
    );
});

function sourceLight(): SourceLight {
  return {
    id: "owned-cct-lamp",
    label: "Owned temperature fixture",
    kind: "point",
    position: [0, 1, 0],
    direction: [0, -1, 0],
    color: [1, 1, 1],
    intensity: 5,
    intensityUnits: "candelas",
    castShadows: false,
    enabled: true,
    temperatureKelvin: 3500,
    useTemperature: true,
    raw: {},
  };
}

test("omitted models keep legacy light RGB while explicit models preserve luminance", () => {
  const light = sourceLight();
  assert.deepEqual(
    lightColor(light),
    [1, 0.6425229450074307, 0.3516227103182903],
  );
  assert.deepEqual(
    lightColor({ ...light, temperatureModel: "cie-planckian-rec709-v1" }),
    colorTemperatureRec709(3500),
  );
  const tinted = { ...light, color: [0.5, 0.25, 0.125] };
  const expected = colorTemperatureRec709(3500).map(
    (value, i) => value * tinted.color[i],
  );
  assert.deepEqual(
    lightColor({ ...tinted, temperatureModel: "cie-planckian-rec709-v1" }),
    expected,
  );
  assert.equal(lightIntensity(light), 5);
  assert.equal(
    lightIntensity({ ...light, temperatureModel: "cie-planckian-rec709-v1" }),
    5,
  );
});

test("model selection never tints a source with temperature disabled", () => {
  const light = {
    ...sourceLight(),
    useTemperature: false,
    color: [0.1, 0.2, 0.3],
  };
  assert.deepEqual(lightColor(light), light.color);
  assert.deepEqual(
    lightColor({ ...light, temperatureModel: "cie-planckian-rec709-v1" }),
    light.color,
  );
});

test("unknown temperature models fail even when temperature is disabled", () => {
  for (const model of ["unknown", "", null, 1, false]) {
    const light = { ...sourceLight(), temperatureModel: model } as SourceLight;
    assert.throws(() => validateLightTemperatureModel(light), /not supported/);
    assert.throws(() => lightColor(light), /not supported/);
    assert.throws(
      () => lightColor({ ...light, useTemperature: false }),
      /not supported/,
    );
  }
});

test("serialized source model tags invalidate old local-bounce identities", () => {
  const legacy = sourceLight();
  const tagged = JSON.parse(
    JSON.stringify({
      ...legacy,
      temperatureModel: "cie-planckian-rec709-v1",
    }),
  ) as SourceLight;
  const legacyIdentity = giBounceLocalLightsIdentityPayload([legacy]);
  assert.notEqual(giBounceLocalLightsIdentityPayload([tagged]), legacyIdentity);
  const { temperatureModel: _, ...untagged } = tagged;
  assert.equal(giBounceLocalLightsIdentityPayload([untagged]), legacyIdentity);
  assert.deepEqual(untagged, legacy);
});

// The reference fixtures stay in Aethyrforge until the owner decides on the
// Unreal tools (docs/aethyr/provenance.md).
const temperatureReference = new URL(
  "../tools/reference/fixtures/ue58-owned-temperature-api.json",
  import.meta.url,
);
test("independent native core-factory measurements agree within the matrix difference", {
  skip: !existsSync(temperatureReference) && "tools/reference is not imported",
}, () => {
  const reference = JSON.parse(
    readFileSync(temperatureReference, "utf8"),
  ) as {
    api: string;
    samples: { kelvin: number; rgba: number[] }[];
    qualification: {
      numericApiComplete: boolean;
      gpuRenderingPerformed: boolean;
    };
  };
  assert.equal(reference.api, "unreal.LinearColor.set_temperature");
  assert.equal(reference.qualification.numericApiComplete, true);
  assert.equal(reference.qualification.gpuRenderingPerformed, false);
  assert.equal(reference.samples.length, 11);
  // The core factory has a fixed approximate BT.709 matrix. Keep the standard
  // D65 matrix rather than fitting its rounding; this is not a working-light
  // or GPU shading oracle.
  for (const sample of reference.samples)
    colorTemperatureRec709(sample.kelvin).forEach((value, i) =>
      assert.ok(Math.abs(value - sample.rgba[i]) < 0.001),
    );
});
