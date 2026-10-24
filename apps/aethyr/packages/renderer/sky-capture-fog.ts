/**
 * Owned infinite-ray height-fog composition for a source realtime SkyLight.
 * The operation is qualified separately from the approximate atmosphere model:
 * tools/reference/skylight-fog.md. No engine shader implementation is copied.
 */
import type { SceneLook } from "../assets/src/scene.ts";
import {
  FOG_HEIGHT_RATE,
  SOURCE_FOG_DENSITY_FACTOR,
  SOURCE_FOG_FALLOFF_FACTOR,
  SOURCE_FOG_INSCATTERING_LINEAR,
  SOURCE_FOG_REFERENCE_HEIGHT,
  sourceFogExtinctionAtHeight,
} from "./fog.ts";

type ObjectValue = Record<string, unknown>;
type RGB = readonly [number, number, number];
export const SKY_CAPTURE_FOG_PROFILE = "source-infinite-height-fog-split-v1";
export interface SkyCaptureFog {
  captureHeightMeters: number;
  baseRGB: RGB;
  lowerHemisphereBlack: true;
}
function object(value: unknown): ObjectValue {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Source sky capture metadata needs objects.");
  return value as ObjectValue;
}
function finite(value: unknown, lo: number, hi: number): number {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < lo ||
    value > hi
  )
    throw new Error("Source sky capture contains an unsupported number.");
  return value;
}
function tuple(value: unknown, names: readonly string[]): number[] {
  if (typeof value !== "string" || value.length > 2048)
    throw new Error(
      "Source sky capture needs recovered color and density values.",
    );
  return names.map((name) => {
    const match = new RegExp(`(?:^|[,(])\\s*${name}=([-+0-9.eE]+)`).exec(value);
    if (!match) throw new Error("A source sky capture value is missing.");
    return finite(Number(match[1]), -1e6, 1e6);
  });
}
function close(a: number, b: number): boolean {
  return Math.abs(a - b) <= 1e-7 * Math.max(0.01, Math.abs(a), Math.abs(b));
}
/** Requires explicit capture visibility and the bounded, qualified source fog family. */
export function sourceSkyCaptureFog(
  look: SceneLook,
  mode: "day" | "night",
): SkyCaptureFog {
  const actors = look.scenarios[mode]?.environment;
  if (!Array.isArray(actors))
    throw new Error("Recover the source sky capture environment.");
  const select = (kind: string) => {
    const matches: Array<{
      actor: ObjectValue;
      component: ObjectValue;
      p: ObjectValue;
    }> = [];
    for (const raw of actors) {
      const actor = object(raw);
      if (object(actor.properties).hidden === true) continue;
      if (!Array.isArray(actor.components))
        throw new Error("Source environment components are missing.");
      for (const value of actor.components) {
        const component = object(value),
          p = object(component.properties);
        if (
          component.class === kind &&
          p.visible !== false &&
          p.hidden_in_game !== true
        )
          matches.push({ actor, component, p });
      }
    }
    if (matches.length !== 1)
      throw new Error(`Capture fog requires one visible ${kind}.`);
    return matches[0];
  };
  const sky = select("SkyLightComponent"),
    fog = select("ExponentialHeightFogComponent");
  const sp = sky.p,
    fp = fog.p;
  if (
    sp.visible !== true ||
    sp.affects_world !== true ||
    sp.real_time_capture !== true ||
    !/\bSLS_CAPTURED_SCENE\b/.test(String(sp.source_type)) ||
    !/\b(MOVABLE|STATIONARY)\b/.test(String(sp.mobility)) ||
    sp.capture_emissive_only !== false ||
    tuple(sp.light_color, ["R", "G", "B"]).some((v) => v !== 255) ||
    sp.lower_hemisphere_is_black !== true ||
    tuple(sp.lower_hemisphere_color, ["R", "G", "B"]).some((v) => v !== 0) ||
    fp.visible !== true ||
    fp.visible_in_real_time_sky_captures !== true
  )
    throw new Error(
      "Capture fog requires explicit realtime captured-scene visibility and a black lower hemisphere.",
    );
  const fogPosition = object(fog.actor.transform).location_cm;
  const fogComponentPosition = object(fog.component.transform).location_cm;
  const skyPosition = object(sky.component.transform).location_cm;
  if (
    !Array.isArray(fogPosition) ||
    fogPosition.length !== 3 ||
    !Array.isArray(fogComponentPosition) ||
    fogComponentPosition.length !== 3 ||
    !Array.isArray(skyPosition) ||
    skyPosition.length !== 3
  )
    throw new Error(
      "Recover source world transforms for the fog and SkyLight.",
    );
  const base = tuple(fp.fog_inscattering_luminance, ["R", "G", "B"]);
  if (
    !close(
      finite(fogPosition[2], -2e6, 2e6) / 100,
      SOURCE_FOG_REFERENCE_HEIGHT,
    ) ||
    !close(finite(fogComponentPosition[2], -2e6, 2e6), fogPosition[2]) ||
    !close(finite(fp.fog_density, 0, 10), SOURCE_FOG_DENSITY_FACTOR) ||
    !close(finite(fp.fog_height_falloff, 0, 10), SOURCE_FOG_FALLOFF_FACTOR) ||
    base.some((v, i) => !close(v, SOURCE_FOG_INSCATTERING_LINEAR[i])) ||
    finite(fp.start_distance, 0, 1e6) !== 0 ||
    finite(fp.fog_max_opacity, 0, 1) !== 1 ||
    finite(fp.fog_cutoff_distance, 0, 1e9) !== 0 ||
    fp.inscattering_color_cubemap !== null ||
    tuple(fp.second_fog_data, ["FogDensity"])[0] !== 0 ||
    tuple(fp.directional_inscattering_luminance, ["R", "G", "B"]).some(
      (v) => v !== 0,
    )
  )
    throw new Error(
      "Capture fog supports the qualified fixed analytic source medium only.",
    );
  return {
    captureHeightMeters: finite(skyPosition[2], -2e6, 2e6) / 100,
    baseRGB: [...SOURCE_FOG_INSCATTERING_LINEAR],
    lowerHemisphereBlack: true,
  };
}
/** Natural-exponential infinite-ray solution; no finite view-distance or volume cutoff. */
export function skyCaptureFogTransmittance(
  height: number,
  directionY: number,
): number {
  finite(directionY, -1, 1);
  const extinction = sourceFogExtinctionAtHeight(height);
  if (directionY <= 0) return 0;
  return Math.exp(-extinction / (FOG_HEIGHT_RATE * directionY));
}
/** Solar and authored-constant terms remain separate for independent runtime scaling. */
export function splitSkyCaptureFog(
  atmosphere: RGB,
  measuredAmbient: RGB,
  directionY: number,
  profile: SkyCaptureFog,
): { solar: [number, number, number]; constant: [number, number, number] } {
  for (const values of [atmosphere, measuredAmbient, profile.baseRGB]) {
    if (!Array.isArray(values) || values.length !== 3)
      throw new Error("Capture fog requires linear RGB triples.");
    for (const value of values) finite(value, 0, 65504);
  }
  const t = skyCaptureFogTransmittance(profile.captureHeightMeters, directionY);
  if (profile.lowerHemisphereBlack !== true)
    throw new Error("Capture fog requires a black lower hemisphere.");
  if (directionY < 0) return { solar: [0, 0, 0], constant: [0, 0, 0] };
  const solar = atmosphere.map(
    (v, c) => v * t + measuredAmbient[c] * (1 - t),
  ) as [number, number, number];
  const constant = profile.baseRGB.map((v) => v * (1 - t)) as [
    number,
    number,
    number,
  ];
  return { solar, constant };
}
/** Parent map orientation is identity and texel centers follow the standard panorama. */
export function splitSkyCaptureFogPanorama(
  pixels: Float32Array,
  width: number,
  height: number,
  measuredAmbient: RGB,
  profile: SkyCaptureFog,
): { solar: Float32Array; constant: Float32Array } {
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 4 ||
    width > 2048 ||
    height !== width / 2 ||
    pixels.length !== width * height * 4
  )
    throw new Error("Use a bounded 2:1 RGBA panorama for capture fog.");
  const solar = new Float32Array(pixels.length),
    constant = new Float32Array(pixels.length);
  for (let y = 0; y < height; y++) {
    const directionY = Math.cos(((y + 0.5) * Math.PI) / height);
    for (let x = 0; x < width; x++) {
      const offset = (y * width + x) * 4;
      const result = splitSkyCaptureFog(
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]],
        measuredAmbient,
        directionY,
        profile,
      );
      solar.set([...result.solar, 1], offset);
      constant.set([...result.constant, 1], offset);
    }
  }
  return { solar, constant };
}
