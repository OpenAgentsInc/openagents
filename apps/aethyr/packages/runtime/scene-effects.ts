import {
  DEFAULT_WIND_FIELD,
  validateWindField,
  type WindFieldSettings,
} from "../renderer/shared-wind.ts";
import { DEFAULT_METEOR_IMPACT_RECIPE } from "../renderer/impact-recipe.ts";

/** Live preview parameters. These do not change gameplay, geometry, or saved source. */
export interface SceneEffectsSettings {
  schema: "aethyrforge.scene-effects.v1";
  wind: WindFieldSettings;
  grass: { bladeHeight: number; bladeWidth: number; windStrength: number };
  impact: { intensity: number; smoke: number; sparksCount: number };
}

const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
function keys(value: unknown, expected: readonly string[]) {
  if (
    !record(value) ||
    Object.keys(value).length !== expected.length ||
    !Object.keys(value).every((key) => expected.includes(key))
  )
    throw new Error("Choose supported effect settings.");
  return value;
}
function bounded(value: unknown, min: number, max: number): number {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < min ||
    value > max
  )
    throw new Error("Choose effect values within the available range.");
  return value;
}

/** Validate the complete candidate before returning a detached, immutable value. */
export function validateSceneEffects(value: unknown): SceneEffectsSettings {
  const candidate = keys(value, ["schema", "wind", "grass", "impact"]);
  if (candidate.schema !== "aethyrforge.scene-effects.v1")
    throw new Error("Choose a supported effects recipe.");
  const wind = validateWindField(candidate.wind as WindFieldSettings);
  const grass = keys(candidate.grass, [
    "bladeHeight",
    "bladeWidth",
    "windStrength",
  ]);
  const impact = keys(candidate.impact, ["intensity", "smoke", "sparksCount"]);
  const sparksCount = bounded(impact.sparksCount, 0, 24);
  if (!Number.isInteger(sparksCount))
    throw new Error("Choose a whole number of sparks.");
  return Object.freeze({
    schema: "aethyrforge.scene-effects.v1",
    wind: Object.freeze({
      ...wind,
      direction: Object.freeze([...wind.direction]) as readonly [
        number,
        number,
      ],
    }),
    grass: Object.freeze({
      bladeHeight: bounded(grass.bladeHeight, 0.1, 2),
      bladeWidth: bounded(grass.bladeWidth, 0.02, 0.3),
      windStrength: bounded(grass.windStrength, 0, 2),
    }),
    impact: Object.freeze({
      intensity: bounded(impact.intensity, 0, 4),
      smoke: bounded(impact.smoke, 0, 1),
      sparksCount,
    }),
  });
}

export const DEFAULT_SCENE_EFFECTS = validateSceneEffects({
  schema: "aethyrforge.scene-effects.v1",
  wind: DEFAULT_WIND_FIELD,
  grass: { bladeHeight: 0.75, bladeWidth: 0.05, windStrength: 1 },
  impact: {
    intensity: DEFAULT_METEOR_IMPACT_RECIPE.intensity,
    smoke: DEFAULT_METEOR_IMPACT_RECIPE.smoke,
    sparksCount: DEFAULT_METEOR_IMPACT_RECIPE.sparksCount,
  },
});

export function validSceneEffects(
  value: unknown,
): value is SceneEffectsSettings {
  try {
    validateSceneEffects(value);
    return true;
  } catch {
    return false;
  }
}

/** Summary copies with unchanged values do not notify effect controls. */
export function sameSceneEffects(
  a: SceneEffectsSettings,
  b: SceneEffectsSettings,
) {
  return (
    a.wind.enabled === b.wind.enabled &&
    a.wind.direction[0] === b.wind.direction[0] &&
    a.wind.direction[1] === b.wind.direction[1] &&
    a.wind.speed === b.wind.speed &&
    a.wind.gustStrength === b.wind.gustStrength &&
    a.wind.gustScale === b.wind.gustScale &&
    a.wind.gustSpeed === b.wind.gustSpeed &&
    a.wind.seed === b.wind.seed &&
    a.grass.bladeHeight === b.grass.bladeHeight &&
    a.grass.bladeWidth === b.grass.bladeWidth &&
    a.grass.windStrength === b.grass.windStrength &&
    a.impact.intensity === b.impact.intensity &&
    a.impact.smoke === b.impact.smoke &&
    a.impact.sparksCount === b.impact.sparksCount
  );
}
