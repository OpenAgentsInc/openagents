import type {
  MeasuredFogAmbientRecipe,
  SceneLook,
} from "../assets/src/scene.ts";
import {
  validateAtmosphere,
  type AtmosphereProfile,
  type AtmosphereSun,
  type AtmosphereVector,
} from "./atmosphere-lighting.ts";
import { lightColor } from "./lighting.ts";
import {
  FOG_MAX_OPACITY,
  FOG_START_DISTANCE,
  SOURCE_FOG_DENSITY_FACTOR,
  SOURCE_FOG_FALLOFF_FACTOR,
  SOURCE_FOG_INSCATTERING_LINEAR,
  SOURCE_FOG_REFERENCE_HEIGHT,
} from "./fog.ts";

type ObjectValue = Record<string, unknown>;
type Mode = "day" | "night";
export const FOG_AMBIENT_RECIPE_LIMIT = 128 * 1024;
/** Lux scaling is an owned linear-transport model, not an extra measured gain. */
export const FOG_AMBIENT_SUN_SCALING = "linear-incident-lux-owned-v1";
const MODES = ["day", "night"] as const;
const zero = (): AtmosphereVector => [0, 0, 0];
const object = (value: unknown): ObjectValue => {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Fog ambient data needs an object.");
  return value as ObjectValue;
};
function number(value: unknown, minimum = 0, maximum = 1e6): number {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < minimum ||
    value > maximum
  )
    throw new Error("Fog ambient data contains an unsupported number.");
  return value;
}
function rgb(value: unknown, minimum = 0, maximum = 65504): AtmosphereVector {
  if (!Array.isArray(value) || value.length !== 3)
    throw new Error("Fog ambient RGB needs three values.");
  return value.map((v) => number(v, minimum, maximum)) as AtmosphereVector;
}
function unit(value: unknown): AtmosphereVector {
  const direction = rgb(value, -1, 1),
    length = Math.hypot(...direction);
  if (Math.abs(length - 1) > 1e-6)
    throw new Error("Fog ambient sunlight needs a unit direction.");
  return direction.map((v) => v / length) as AtmosphereVector;
}
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, item]) => [key, canonical(item)]),
    );
  return value;
}
const encoded = (value: unknown) => JSON.stringify(canonical(value));
async function digest(value: string): Promise<string> {
  const hash = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(value),
  );
  return [...new Uint8Array(hash)]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
}
function close(actual: unknown, expected: unknown, epsilon = 1e-10): boolean {
  if (typeof actual === "number" && typeof expected === "number")
    return (
      Number.isFinite(actual) &&
      Number.isFinite(expected) &&
      Math.abs(actual - expected) <=
        epsilon * Math.max(1e-2, Math.abs(actual), Math.abs(expected))
    );
  if (Array.isArray(actual) && Array.isArray(expected))
    return (
      actual.length === expected.length &&
      actual.every((v, i) => close(v, expected[i], epsilon))
    );
  if (
    actual &&
    expected &&
    typeof actual === "object" &&
    typeof expected === "object"
  ) {
    const a = Object.keys(actual).sort(),
      b = Object.keys(expected).sort();
    return (
      JSON.stringify(a) === JSON.stringify(b) &&
      a.every((key) =>
        close(
          (actual as ObjectValue)[key],
          (expected as ObjectValue)[key],
          epsilon,
        ),
      )
    );
  }
  return actual === expected;
}
function tuple(value: unknown, keys: readonly string[]): number[] {
  if (typeof value !== "string" || value.length > 2048)
    throw new Error("Recover the explicit source fog and atmosphere colors.");
  return keys.map((key) => {
    const found = new RegExp(`(?:^|[,(])\\s*${key}=([-+0-9.eE]+)`).exec(value);
    if (!found) throw new Error("A source fog color or coordinate is missing.");
    return number(Number(found[1]), -1e6, 1e6);
  });
}
function profileFromControls(atmosphere: ObjectValue): AtmosphereProfile {
  const coefficient = (color: string, factor: string) =>
    rgb(atmosphere[color]).map(
      (v) => (v * number(atmosphere[factor])) / 1000,
    ) as AtmosphereVector;
  const tent = rgb(atmosphere.absorptionTentKm);
  const profile: AtmosphereProfile = {
    planetRadiusMeters: number(atmosphere.bottomRadiusKm) * 1000,
    atmosphereHeightMeters: number(atmosphere.heightKm) * 1000,
    rayleighScatteringPerMeter: coefficient("rayleighRGB", "rayleighScale"),
    rayleighScaleHeightMeters: number(atmosphere.rayleighHeightKm) * 1000,
    mieScatteringPerMeter: coefficient(
      "mieScatteringRGB",
      "mieScatteringScale",
    ),
    mieAbsorptionPerMeter: coefficient(
      "mieAbsorptionRGB",
      "mieAbsorptionScale",
    ),
    mieScaleHeightMeters: number(atmosphere.mieHeightKm) * 1000,
    mieAnisotropy: number(atmosphere.mieAnisotropy, -0.99, 0.99),
    absorptionPerMeter: coefficient(
      "otherAbsorptionRGB",
      "otherAbsorptionScale",
    ),
    absorptionTent: {
      peakAltitudeMeters: tent[0] * 1000,
      peakDensity: tent[1],
      halfWidthMeters: tent[2] * 1000,
    },
    skyLuminanceFactor: rgb(atmosphere.skyLuminanceRGB),
  };
  validateAtmosphere(profile);
  return profile;
}
function extrasFromControls(atmosphere: ObjectValue) {
  const ground = rgb(atmosphere.groundSrgbBytes, 0, 255);
  if (ground.some((v) => !Number.isInteger(v)))
    throw new Error("Atmosphere ground color needs sRGB bytes.");
  return {
    multiScatteringFactor: number(atmosphere.multiScatteringFactor),
    groundSrgbBytes: ground,
    aerialStartKm: number(atmosphere.aerialStartKm),
    aerialDistanceScale: number(atmosphere.aerialDistanceScale),
    minLightElevationDegrees: number(
      atmosphere.minLightElevationDegrees,
      -90,
      90,
    ),
    heightFogContribution: 1,
    aerialLuminanceRGB: [1, 1, 1],
    transformMode: "planet-top-at-absolute-world-origin",
  };
}
function fogFromLook(look: SceneLook, mode: Mode) {
  const matches: Array<{ actor: ObjectValue; properties: ObjectValue }> = [];
  for (const value of look.scenarios[mode].environment) {
    const actor = object(value);
    if (
      actor.class !== "ExponentialHeightFog" ||
      object(actor.properties).hidden === true
    )
      continue;
    if (!Array.isArray(actor.components))
      throw new Error("Recover the source height-fog component.");
    for (const raw of actor.components) {
      const component = object(raw),
        properties = object(component.properties);
      if (
        component.class === "ExponentialHeightFogComponent" &&
        properties.visible !== false &&
        properties.hidden_in_game !== true
      )
        matches.push({ actor, properties });
    }
  }
  if (matches.length !== 1)
    throw new Error(
      "The measured fog profile requires one source height-fog component.",
    );
  const { actor, properties: p } = matches[0],
    position = rgb(object(actor.transform).location_cm, -1e7, 1e7);
  if (
    p.inscattering_color_cubemap !== null ||
    tuple(p.second_fog_data, ["FogDensity"])[0] !== 0 ||
    number(p.fog_cutoff_distance) !== 0
  )
    throw new Error(
      "This measured fog profile excludes cubemap, second-layer and cutoff variants.",
    );
  const fog = {
    referenceHeightMeters: position[2] / 100,
    density: number(p.fog_density, 0, 10),
    heightFalloff: number(p.fog_height_falloff, 0, 10),
    inscatteringRGB: rgb(tuple(p.fog_inscattering_luminance, ["R", "G", "B"])),
    ambientContributionRGB: rgb(
      tuple(p.sky_atmosphere_ambient_contribution_color_scale, ["R", "G", "B"]),
    ),
    startMeters: number(p.start_distance) / 100,
    maxOpacity: number(p.fog_max_opacity, 0, 1),
  };
  const { ambientContributionRGB: _contribution, ...sourceMedium } = fog;
  if (
    !close(
      sourceMedium,
      {
        referenceHeightMeters: SOURCE_FOG_REFERENCE_HEIGHT,
        density: SOURCE_FOG_DENSITY_FACTOR,
        heightFalloff: SOURCE_FOG_FALLOFF_FACTOR,
        inscatteringRGB: SOURCE_FOG_INSCATTERING_LINEAR,
        startMeters: FOG_START_DISTANCE,
        maxOpacity: FOG_MAX_OPACITY,
      },
      1e-7,
    )
  )
    throw new Error(
      "The measured fog profile requires the supported source height, density, falloff and base color.",
    );
  return fog;
}
function activeBinding(environments: unknown, look: SceneLook, mode: Mode) {
  const environment = object(environments),
    metadata = object(object(environment.atmosphere)[mode]);
  if (environment.schema !== "aethyrforge.private-environments.v1")
    throw new Error("Use prepared lighting environments for measured fog.");
  const profile = object(metadata.profile) as unknown as AtmosphereProfile;
  validateAtmosphere(profile);
  const p = object(metadata.sourceProperties),
    sun = look.scenarios[mode].sun;
  if (
    typeof p.transform_mode !== "string" ||
    !/\bPLANET_TOP_AT_ABSOLUTE_WORLD_ORIGIN\b/.test(p.transform_mode)
  )
    throw new Error(
      "The measured atmosphere must use planet top at absolute world origin.",
    );
  if (
    !sun ||
    sun.kind !== "directional" ||
    sun.intensityUnits !== "lux" ||
    !sun.enabled
  )
    throw new Error(
      "The measured fog profile requires the source directional lux light.",
    );
  const referenceSun: AtmosphereSun = {
    direction: unit(sun.direction.map((v) => -v)),
    color: rgb(lightColor(sun)),
    illuminanceLux: number(sun.intensity, 0.001, 1e6),
  };
  const preparedSun = object(metadata.sun);
  if (
    !close(preparedSun.direction, referenceSun.direction, 1e-7) ||
    !close(preparedSun.color, referenceSun.color, 1e-7) ||
    preparedSun.illuminanceLux !== referenceSun.illuminanceLux
  )
    throw new Error(
      "Atmosphere lighting was prepared for a different source sun.",
    );
  const extras = {
    multiScatteringFactor: number(p.multi_scattering_factor),
    groundSrgbBytes: rgb(tuple(p.ground_albedo, ["R", "G", "B"]), 0, 255),
    aerialStartKm: number(p.aerial_perspective_start_depth),
    aerialDistanceScale: number(p.aerial_pespective_view_distance_scale),
    minLightElevationDegrees: number(
      p.transmittance_min_light_elevation_angle,
      -90,
      90,
    ),
    heightFogContribution: number(p.height_fog_contribution),
    aerialLuminanceRGB: rgb(
      tuple(p.sky_and_aerial_perspective_luminance_factor, ["R", "G", "B"]),
    ),
    transformMode: "planet-top-at-absolute-world-origin",
  };
  return {
    scenario: mode,
    profile,
    extras,
    referenceSun,
    sunConfig: {
      baseColor: rgb(sun.color),
      useTemperature: sun.useTemperature,
      temperatureKelvin: sun.temperatureKelvin,
    },
    heightFog: fogFromLook(look, mode),
  };
}
function validateMeasuredControls(
  raw: ObjectValue,
  binding: ReturnType<typeof activeBinding>,
) {
  const controls = object(raw.controls),
    atmosphere = object(controls.atmosphere),
    sun = object(controls.sun),
    fog = object(controls.heightFog);
  if (
    !close(profileFromControls(atmosphere), binding.profile) ||
    !close(extrasFromControls(atmosphere), binding.extras)
  )
    throw new Error(
      "Rebuild measured fog for the current atmosphere configuration.",
    );
  if (
    !close(fog.ambientContributionRGB, [1, 1, 1]) ||
    !close(
      {
        ...fog,
        ambientContributionRGB: binding.heightFog.ambientContributionRGB,
      },
      binding.heightFog,
    )
  )
    throw new Error(
      "Rebuild measured fog for the current source height-fog controls.",
    );
  const rotation = rgb(sun.rotationDegrees, -360, 360),
    pitch = (rotation[0] * Math.PI) / 180,
    yaw = (rotation[1] * Math.PI) / 180;
  const direction = [
    -Math.cos(pitch) * Math.cos(yaw),
    -Math.sin(pitch),
    -Math.cos(pitch) * Math.sin(yaw),
  ];
  const bytes = rgb(sun.srgbBytes, 0, 255);
  if (bytes.some((v) => !Number.isInteger(v)))
    throw new Error("Measured sunlight needs sRGB bytes.");
  const baseColor = bytes.map((v) =>
    v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4,
  );
  if (
    !close(direction, binding.referenceSun.direction, 1e-7) ||
    !close(baseColor, binding.sunConfig.baseColor) ||
    sun.useTemperature !== binding.sunConfig.useTemperature ||
    sun.temperatureKelvin !== binding.sunConfig.temperatureKelvin ||
    sun.intensityLux !== binding.referenceSun.illuminanceLux
  )
    throw new Error("Rebuild measured fog for the current source sunlight.");
}

function repeatedRGB(value: unknown): AtmosphereVector {
  if (!Array.isArray(value) || value.length !== 3)
    throw new Error("Fog receivers need three repeated RGB captures.");
  const samples = value.map((v) => rgb(v, -65504, 65504));
  if (
    samples.some((v) => v.some((c, i) => Math.abs(c - samples[2][i]) > 0.003))
  )
    throw new Error("The repeated fog captures did not converge.");
  return samples[2];
}

/** Independently check isolation and reference response, rather than a reported pass. */
function validateSunlight(calibration: ObjectValue) {
  const sunlight = object(calibration.sunlight),
    samples = object(sunlight.samples);
  const off = repeatedRGB(samples.off),
    on = repeatedRGB(samples.on),
    noInteraction = repeatedRGB(samples.noInteraction),
    dark = repeatedRGB(samples.dark);
  if (
    off.some((v) => v < 0.01) ||
    dark.some((v) => Math.abs(v) > 0.001) ||
    noInteraction.some((v, i) => Math.abs(v - off[i]) > 0.003)
  )
    throw new Error("The measured sunlight isolation checks failed.");
  const expected = rgb(sunlight.publicGroundTransmittance, 0, 1),
    ratios = on.map((v, i) => v / off[i]),
    maxRatioError = Math.max(
      ...ratios.map((v, i) => Math.abs(v - expected[i])),
    );
  if (
    maxRatioError > 0.005 ||
    !close(ratios, sunlight.ratios, 1e-7) ||
    !close(maxRatioError, sunlight.maxRatioError, 1e-7)
  )
    throw new Error(
      "The measured sunlight differs from its ground-reference response.",
    );
}

/** The separate double-lux run must reproduce twice the fitted additive basis. */
function validateLinearity(
  calibration: ObjectValue,
  basis: AtmosphereVector,
  referenceLux: number,
) {
  const proof = object(calibration.linearity);
  if (
    proof.referenceMultiplier !== 2 ||
    proof.qualified !== true ||
    !Array.isArray(proof.measurements) ||
    proof.measurements.length !== 2
  )
    throw new Error(
      "Measured fog needs its original double-illuminance controls.",
    );
  const rows = new Map<
    string,
    { black: AtmosphereVector; white: AtmosphereVector }
  >();
  for (const value of proof.measurements) {
    const row = object(value),
      raw = object(row.raw);
    if (
      row.distanceMeters !== 1000 ||
      row.intensityLux !== referenceLux * 2 ||
      !["atmosphere-off", "atmosphere-on"].includes(String(row.state)) ||
      rows.has(String(row.state))
    )
      throw new Error("The double-illuminance receiver identity changed.");
    rows.set(String(row.state), {
      black: repeatedRGB(raw.black),
      white: repeatedRGB(raw.white),
    });
  }
  const off = rows.get("atmosphere-off")!,
    on = rows.get("atmosphere-on")!,
    transmittance = off.white.map((v, i) => v - off.black[i]),
    t = transmittance.reduce((a, b) => a + b) / 3,
    weight = 1 - t;
  if (
    weight <= 0.02 ||
    weight >= 0.98 ||
    transmittance.some((v) => Math.abs(v - t) > 0.003) ||
    on.white.some(
      (v, i) => Math.abs(v - on.black[i] - transmittance[i]) > 0.003,
    )
  )
    throw new Error("The double-illuminance extinction check failed.");
  const observed = on.black.map((v, i) => (v - off.black[i]) / weight),
    maxAbsoluteError = Math.max(
      ...observed.map((v, i) => Math.abs(v - basis[i] * 2)),
    );
  if (
    maxAbsoluteError > 0.005 ||
    !close(observed, proof.observedAmbientRGB, 1e-7) ||
    !close(maxAbsoluteError, proof.maxAbsoluteError, 1e-7)
  )
    throw new Error("The double-illuminance color check failed.");
}

/** Recompute the original three-distance additive fit, independent of reported RGB. */
function measuredBasis(calibration: ObjectValue): AtmosphereVector {
  if (
    calibration.runtimeAdmission !== true ||
    !Array.isArray(calibration.rawInput) ||
    calibration.rawInput.length !== 4 ||
    calibration.rawInput
      .slice(0, 3)
      .some((v) => Math.abs(number(v) - 1) > 0.001) ||
    !close(calibration.readbackSanity, [0.25, 0.5, 2, 1])
  )
    throw new Error(
      "The fog measurement did not pass its raw-input and readback gates.",
    );
  if (
    !Array.isArray(calibration.measurements) ||
    calibration.measurements.length > 32
  )
    throw new Error("Fog measurements need bounded original receiver rows.");
  const rows = new Map<
    string,
    { black: AtmosphereVector; white: AtmosphereVector }
  >();
  for (const value of calibration.measurements) {
    const row = object(value),
      raw = object(row.raw),
      samples = {} as { black: AtmosphereVector; white: AtmosphereVector };
    for (const color of ["black", "white"] as const) {
      samples[color] = repeatedRGB(raw[color]);
    }
    const key = `${row.distanceMeters}:${row.state}`;
    if (rows.has(key)) throw new Error("Fog measurement rows repeat.");
    rows.set(key, samples);
  }
  const identity = rows.get("100:no-fog");
  if (
    !identity ||
    identity.black.some((v) => Math.abs(v) > 0.001) ||
    identity.white.some((v) => Math.abs(v - 1) > 0.001)
  )
    throw new Error("The no-fog receiver identity check failed.");
  const points: Array<{
      weight: number;
      off: { black: AtmosphereVector; white: AtmosphereVector };
      on: { black: AtmosphereVector; white: AtmosphereVector };
    }> = [],
    errors = {
      maxFitResidual: 0,
      maxAmbientZeroResidual: 0,
      maxGrayTransmittanceResidual: 0,
    };
  for (const distance of [100, 500, 1000]) {
    const off = rows.get(`${distance}:atmosphere-off`),
      on = rows.get(`${distance}:atmosphere-on`),
      zero = rows.get(`${distance}:atmosphere-on-ambient-zero`);
    if (!off || !on || !zero)
      throw new Error(
        "The fog measurement needs all three distances and isolation states.",
      );
    const transmittance = off.white.map((v, i) => v - off.black[i]),
      t = transmittance.reduce((a, b) => a + b) / 3;
    if (t <= 0.03 || t >= 0.98)
      throw new Error("Fog receiver extinction is outside the measured range.");
    errors.maxGrayTransmittanceResidual = Math.max(
      errors.maxGrayTransmittanceResidual,
      ...transmittance.map((v) => Math.abs(v - t)),
    );
    errors.maxAmbientZeroResidual = Math.max(
      errors.maxAmbientZeroResidual,
      ...["black", "white"].flatMap((key) =>
        zero[key as "black"].map((v, i) =>
          Math.abs(v - off[key as "black"][i]),
        ),
      ),
    );
    points.push({ weight: 1 - t, off, on });
  }
  const denominator = points.reduce((sum, p) => sum + p.weight ** 2, 0);
  const basis = [0, 1, 2].map(
    (c) =>
      points.reduce(
        (sum, p) => sum + p.weight * (p.on.black[c] - p.off.black[c]),
        0,
      ) / denominator,
  ) as AtmosphereVector;
  const base = [0, 1, 2].map(
    (c) =>
      points.reduce((sum, p) => sum + p.weight * p.off.black[c], 0) /
      denominator,
  );
  errors.maxFitResidual = Math.max(
    ...points.flatMap((p) =>
      base.map((b, c) => Math.abs(p.on.black[c] - p.weight * (b + basis[c]))),
    ),
  );
  if (
    errors.maxFitResidual > 0.005 ||
    errors.maxAmbientZeroResidual > 0.005 ||
    errors.maxGrayTransmittanceResidual > 0.003 ||
    !close(errors, calibration.errors, 1e-7)
  )
    throw new Error("The measured fog fit or isolation checks failed.");
  return rgb(basis);
}

/** The publisher adds an exact binding to normalized active inputs, never a fitted gain. */
export async function bindFogAmbientMeasurement(
  value: unknown,
  mode: Mode,
  environments: unknown,
  look: SceneLook,
): Promise<MeasuredFogAmbientRecipe> {
  if (
    new TextEncoder().encode(JSON.stringify(value)).length >
    FOG_AMBIENT_RECIPE_LIMIT
  )
    throw new Error("The measured fog profile exceeds 128 KiB.");
  const raw = object(value);
  if (
    raw.schema !== "aethyrforge.measured-fog-ambient.v1" ||
    raw.scenario !== mode ||
    raw.space !== "scene-linear-rec709"
  )
    throw new Error(
      "Use a measured scene-linear fog profile for this scenario.",
    );
  const includes = object(raw.includes);
  if (
    Object.keys(includes).sort().join() !==
      [
        "skyLightIntensity",
        "sceneStorageScale",
        "exposure",
        "sourceAmbientContributionScale",
      ]
        .sort()
        .join() ||
    Object.values(includes).some((v) => v !== false)
  )
    throw new Error(
      "Measured fog must exclude Skylight, storage, exposure and source contribution gains.",
    );
  if (
    typeof raw.controlsJson !== "string" ||
    raw.controlsJson.length > 32 * 1024 ||
    raw.controlsSha256 !== (await digest(raw.controlsJson)) ||
    encoded(JSON.parse(raw.controlsJson)) !== encoded(raw.controls)
  )
    throw new Error("The measured fog controls or their hash changed.");
  const binding = activeBinding(environments, look, mode);
  validateMeasuredControls(raw, binding);
  const calibration = object(raw.calibration),
    basis = measuredBasis(calibration);
  validateSunlight(calibration);
  validateLinearity(calibration, basis, binding.referenceSun.illuminanceLux);
  if (
    !close(calibration.fixedAtmosphere, {
      transformMode: "planet-top-at-absolute-world-origin",
      heightFogContribution: 1,
      skyAndAerialPerspectiveLuminanceRGB: [1, 1, 1],
      sunLightIndex: 0,
      perPixelSunTransmittance: false,
    })
  )
    throw new Error(
      "The measured fog capture used unsupported fixed atmosphere controls.",
    );
  if (!close(basis, rgb(raw.ambientRGB), 1e-8))
    throw new Error(
      "The measured fog RGB does not match its original receiver fit.",
    );
  const bindingSha256 = await digest(encoded(binding));
  if (raw.bindingSha256 !== undefined && raw.bindingSha256 !== bindingSha256)
    throw new Error(
      "The prepared fog profile is stale for the active source configuration.",
    );
  return structuredClone({
    ...raw,
    bindingSha256,
  }) as unknown as MeasuredFogAmbientRecipe;
}
export interface PreparedFogAmbient {
  readonly scenario: Mode;
  readonly bindingSha256: string;
  /** Source contribution RGB is already applied exactly once; no other gains. */
  readonly basisRGB: readonly [number, number, number];
  readonly referenceSun: Readonly<AtmosphereSun>;
  readonly sunScaling: typeof FOG_AMBIENT_SUN_SCALING;
}
export async function loadFogAmbientProfiles(
  environments: unknown,
  look: SceneLook | undefined,
): Promise<{
  profiles: Partial<Record<Mode, PreparedFogAmbient>>;
  diagnostics: string[];
}> {
  const profiles: Partial<Record<Mode, PreparedFogAmbient>> = {},
    diagnostics: string[] = [];
  if (!environments || !look) return { profiles, diagnostics };
  let records: ObjectValue;
  try {
    const source = object(environments).fogAmbient;
    if (source === undefined) return { profiles, diagnostics };
    records = object(source);
  } catch {
    return {
      profiles,
      diagnostics: [
        "Measured fog profiles are malformed; their contribution is disabled.",
      ],
    };
  }
  for (const mode of MODES) {
    const value = records[mode];
    if (value === undefined) continue;
    try {
      if (typeof object(value).bindingSha256 !== "string")
        throw new Error("Publish the measured fog binding before loading it.");
      const recipe = await bindFogAmbientMeasurement(
          value,
          mode,
          environments,
          look,
        ),
        binding = activeBinding(environments, look, mode);
      profiles[mode] = Object.freeze({
        scenario: mode,
        bindingSha256: recipe.bindingSha256,
        basisRGB: Object.freeze(
          recipe.ambientRGB.map(
            (v, c) => v * binding.heightFog.ambientContributionRGB[c],
          ),
        ) as readonly [number, number, number],
        referenceSun: Object.freeze({
          ...binding.referenceSun,
          direction: Object.freeze([
            ...binding.referenceSun.direction,
          ]) as unknown as AtmosphereVector,
          color: Object.freeze([
            ...binding.referenceSun.color,
          ]) as unknown as AtmosphereVector,
        }),
        sunScaling: FOG_AMBIENT_SUN_SCALING,
      });
    } catch (error) {
      diagnostics.push(
        `${mode}: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }
  return { profiles, diagnostics };
}
/** Raw additive fog RGB. Host applies only the analytic fill weight and shared scale. */
export function fogAmbientRadiance(
  profile: PreparedFogAmbient | undefined,
  current: { skySource: "kit" | "generated"; sun: AtmosphereSun },
): AtmosphereVector {
  if (!profile || current.skySource !== "kit") return zero();
  const sun = current.sun;
  if (
    !Number.isFinite(sun.illuminanceLux) ||
    sun.illuminanceLux < 0 ||
    sun.illuminanceLux > 1e6 ||
    !close(sun.direction, profile.referenceSun.direction, 1e-7) ||
    !close(sun.color, profile.referenceSun.color, 1e-7)
  )
    return zero();
  const ratio = sun.illuminanceLux / profile.referenceSun.illuminanceLux;
  return profile.basisRGB.map((v) => v * ratio) as AtmosphereVector;
}
