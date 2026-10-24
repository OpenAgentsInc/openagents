import assert from "node:assert/strict";
import { test } from "node:test";
import type { SceneLook } from "../packages/assets/src/scene.ts";
import {
  FOG_HEIGHT_RATE,
  SOURCE_FOG_EXTINCTION_AT_SOURCE_HEIGHT,
  SOURCE_FOG_REFERENCE_HEIGHT,
  segmentFogOpticalDepth,
} from "../packages/renderer/fog.ts";
import {
  sourceSkyCaptureFog,
  skyCaptureFogTransmittance,
  splitSkyCaptureFog,
  splitSkyCaptureFogPanorama,
} from "../packages/renderer/sky-capture-fog.ts";

function fixture(): SceneLook {
  const environment = [
    {
      properties: { hidden: false },
      transform: { location_cm: [0, 0, -6000] },
      components: [
        {
          class: "ExponentialHeightFogComponent",
          transform: { location_cm: [0, 0, -6000] },
          properties: {
            visible: true,
            visible_in_real_time_sky_captures: true,
            fog_density: 0.25,
            fog_height_falloff: 0.4,
            fog_inscattering_luminance: "(R=.609237,G=.937705,B=1.5,A=3)",
            start_distance: 0,
            fog_max_opacity: 1,
            fog_cutoff_distance: 0,
            inscattering_color_cubemap: null,
            second_fog_data: "(FogDensity=0)",
            directional_inscattering_luminance: "(R=0,G=0,B=0,A=1)",
          },
        },
      ],
    },
    {
      properties: { hidden: false },
      components: [
        {
          class: "SkyLightComponent",
          transform: { location_cm: [0, 0, 412.5] },
          properties: {
            visible: true,
            affects_world: true,
            real_time_capture: true,
            source_type: "<SkyLightSourceType.SLS_CAPTURED_SCENE: 0>",
            mobility: "<ComponentMobility.MOVABLE: 2>",
            light_color: "(R=255,G=255,B=255,A=255)",
            capture_emissive_only: false,
            lower_hemisphere_is_black: true,
            lower_hemisphere_color: "(R=0,G=0,B=0,A=1)",
          },
        },
      ],
    },
  ];
  return {
    scenarios: {
      day: { environment },
      night: { environment: structuredClone(environment) },
    },
  } as unknown as SceneLook;
}
const rgb = (a: readonly number[], b: readonly number[], epsilon = 1e-12) =>
  a.forEach((v, i) =>
    assert.ok(Math.abs(v - b[i]) <= epsilon, `${v} != ${b[i]}`),
  );

test("infinite source fog matches a sufficiently long independent segment integral", () => {
  for (const direction of [0.1, 0.5, 1]) {
    const finite = Math.exp(
      -segmentFogOpticalDepth(
        4.125,
        direction,
        0,
        100000,
        SOURCE_FOG_EXTINCTION_AT_SOURCE_HEIGHT,
        FOG_HEIGHT_RATE,
        SOURCE_FOG_REFERENCE_HEIGHT,
      ),
    );
    assert.ok(
      Math.abs(skyCaptureFogTransmittance(4.125, direction) - finite) < 1e-12,
    );
  }
  assert.equal(skyCaptureFogTransmittance(4.125, 0), 0);
  assert.ok(
    skyCaptureFogTransmittance(4.125, 0.1) <
      skyCaptureFogTransmittance(4.125, 1),
  );
  assert.ok(
    skyCaptureFogTransmittance(10, 1) > skyCaptureFogTransmittance(4.125, 1),
  );
});

test("solar transport is linear while authored constant fog survives a zero sun", () => {
  const profile = sourceSkyCaptureFog(fixture(), "day"),
    a = splitSkyCaptureFog([0.2, 0.3, 0.4], [0.38, 0.43, 0.5], 0.5, profile);
  const double = splitSkyCaptureFog(
      [0.4, 0.6, 0.8],
      [0.76, 0.86, 1],
      0.5,
      profile,
    ),
    zero = splitSkyCaptureFog([0, 0, 0], [0, 0, 0], 0.5, profile);
  rgb(
    double.solar,
    a.solar.map((v) => v * 2),
  );
  rgb(double.constant, a.constant);
  rgb(zero.solar, [0, 0, 0]);
  rgb(zero.constant, a.constant);
  const t = skyCaptureFogTransmittance(4.125, 0.5);
  rgb(
    a.solar.map((v, i) => v + a.constant[i]),
    [0.2, 0.3, 0.4].map(
      (v, i) => v * t + ([0.38, 0.43, 0.5][i] + profile.baseRGB[i]) * (1 - t),
    ),
  );
  rgb(
    splitSkyCaptureFog([1, 2, 3], [1, 1, 1], -1, profile).constant,
    [0, 0, 0],
  );
});

test("complete source proof binds capture height and rejects unsupported variants", () => {
  assert.equal(
    sourceSkyCaptureFog(fixture(), "day").captureHeightMeters,
    4.125,
  );
  for (const damage of [
    "visibility",
    "realtime",
    "kind",
    "color",
    "position",
  ] as const) {
    const look = fixture(),
      env = look.scenarios.day.environment as any[];
    if (damage === "visibility")
      env[0].components[0].properties.visible_in_real_time_sky_captures = false;
    if (damage === "realtime")
      delete env[1].components[0].properties.real_time_capture;
    if (damage === "kind")
      env[1].components[0].properties.source_type = "SLS_SPECIFIED_CUBEMAP";
    if (damage === "color")
      env[0].components[0].properties.fog_inscattering_luminance =
        "(R=1,G=1,B=1,A=1)";
    if (damage === "position")
      env[1].components[0].transform.location_cm = [0, 0, NaN];
    assert.throws(() => sourceSkyCaptureFog(look, "day"));
  }
});

test("split panorama preserves alpha1, blanks the lower hemisphere, and rejects malformed inputs", () => {
  const p = sourceSkyCaptureFog(fixture(), "day"),
    input = new Float32Array(8 * 4 * 4).fill(1),
    result = splitSkyCaptureFogPanorama(input, 8, 4, [0.3, 0.4, 0.5], p);
  for (const field of [result.solar, result.constant])
    for (let i = 0; i < 32; i++) {
      assert.equal(field[i * 4 + 3], 1);
      if (i >= 16) rgb([...field.slice(i * 4, i * 4 + 3)], [0, 0, 0]);
    }
  input[0] = NaN;
  assert.throws(() =>
    splitSkyCaptureFogPanorama(input, 8, 4, [0.3, 0.4, 0.5], p),
  );
  assert.throws(() =>
    splitSkyCaptureFogPanorama(new Float32Array(1), 8, 4, [0.3, 0.4, 0.5], p),
  );
});
