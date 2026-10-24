import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";

/** Horizontal world wind, in meters per second, sampled independently of UI frames. */
export interface WindFieldSettings {
  schema: "aethyrforge.wind.v1";
  enabled: boolean;
  direction: readonly [number, number];
  speed: number;
  gustStrength: number;
  gustScale: number;
  gustSpeed: number;
  seed: number;
}
export const WIND_FIELD_FLOATS = 12;
export const DEFAULT_WIND_FIELD: WindFieldSettings = Object.freeze({
  schema: "aethyrforge.wind.v1",
  enabled: true,
  direction: Object.freeze([0.8, 0.6] as const),
  speed: 4,
  gustStrength: 0.6,
  gustScale: 24,
  gustSpeed: 0.7,
  seed: 7,
});
function bounded(value: number, min: number, max: number) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < min ||
    value > max
  )
    throw new Error("Choose supported wind settings.");
}
export function validateWindField(value: WindFieldSettings) {
  if (
    value?.schema !== "aethyrforge.wind.v1" ||
    typeof value.enabled !== "boolean" ||
    !Array.isArray(value.direction) ||
    value.direction.length !== 2
  )
    throw new Error("Choose a supported wind field.");
  bounded(value.direction[0], -1, 1);
  bounded(value.direction[1], -1, 1);
  if (Math.abs(Math.hypot(...value.direction) - 1) > 0.0001)
    throw new Error("Wind needs a normalized direction.");
  bounded(value.speed, 0, 30);
  bounded(value.gustStrength, 0, 1);
  bounded(value.gustScale, 1, 256);
  bounded(value.gustSpeed, 0, 4);
  bounded(value.seed, 0, 65535);
  if (!Number.isInteger(value.seed))
    throw new Error("Choose a whole-number wind seed.");
  const keys = Object.keys(value);
  if (
    keys.length !== 8 ||
    keys.some(
      (key) =>
        ![
          "schema",
          "enabled",
          "direction",
          "speed",
          "gustStrength",
          "gustScale",
          "gustSpeed",
          "seed",
        ].includes(key),
    )
  )
    throw new Error("Choose supported wind settings.");
  return value;
}
/** Three aligned vec4 values; state.x distinguishes absent wind from disabled wind. */
export function packWindField(
  value: WindFieldSettings,
  out = new Float32Array(WIND_FIELD_FLOATS),
  offset = 0,
) {
  validateWindField(value);
  if (
    !Number.isInteger(offset) ||
    offset < 0 ||
    out.length < offset + WIND_FIELD_FLOATS
  )
    throw new Error("Wind needs enough uniform storage.");
  out.fill(0, offset, offset + WIND_FIELD_FLOATS);
  out.set(
    [
      value.direction[0],
      value.direction[1],
      value.speed,
      Number(value.enabled),
      value.gustStrength,
      value.gustScale,
      value.gustSpeed,
      value.seed,
      1,
    ],
    offset,
  );
  return out;
}
const TAU = Math.PI * 2;
const PHASE_SEED = 0.013;
const SECOND_X = -0.61;
const SECOND_Z = 0.79;
const SECOND_SEED = 1.71;
const SECOND_RATE = -0.73;
function validateSample(position: readonly number[], time: number) {
  if (position.length !== 3) throw new Error("Wind needs a world position.");
  for (const value of position) bounded(value, -1e6, 1e6);
  bounded(time, -MAX_WORLD_ANIMATION_TIME, MAX_WORLD_ANIMATION_TIME);
}
function phase(position: readonly number[], value: WindFieldSettings) {
  const x = position[0] / value.gustScale;
  const z = position[2] / value.gustScale;
  const seed = value.seed * PHASE_SEED;
  return [
    TAU * (x * value.direction[0] + z * value.direction[1]) + seed,
    TAU * (x * SECOND_X + z * SECOND_Z) + seed * SECOND_SEED,
  ] as const;
}
/** Positive directional gusts have continuous spatial and temporal derivatives. */
export function sampleWorldWind(
  position: readonly number[],
  time: number,
  value: WindFieldSettings = DEFAULT_WIND_FIELD,
  out: Float32Array | Float64Array | number[] = [0, 0, 0],
) {
  validateWindField(value);
  validateSample(position, time);
  const p = phase(position, value);
  const strength = value.enabled
    ? value.speed *
      (1 +
        value.gustStrength *
          0.5 *
          (Math.sin(p[0] + time * value.gustSpeed) +
            Math.sin(p[1] + time * value.gustSpeed * SECOND_RATE)))
    : 0;
  out[0] = value.direction[0] * strength;
  out[1] = 0;
  out[2] = value.direction[1] * strength;
  return out;
}
function sineIntegral(phase: number, rate: number, start: number, end: number) {
  return rate === 0
    ? Math.sin(phase) * (end - start)
    : (Math.cos(phase + rate * start) - Math.cos(phase + rate * end)) / rate;
}
/** Exact frozen-position integral preserves analytic particle seeking and birth times. */
export function sampleWorldWindDisplacement(
  position: readonly number[],
  start: number,
  end: number,
  value: WindFieldSettings = DEFAULT_WIND_FIELD,
  out: Float32Array | Float64Array | number[] = [0, 0, 0],
) {
  validateWindField(value);
  validateSample(position, start);
  validateSample(position, end);
  if (end < start) throw new Error("Wind needs ordered sample times.");
  const p = phase(position, value);
  const distance = value.enabled
    ? value.speed *
      (end -
        start +
        value.gustStrength *
          0.5 *
          (sineIntegral(p[0], value.gustSpeed, start, end) +
            sineIntegral(p[1], value.gustSpeed * SECOND_RATE, start, end)))
    : 0;
  out[0] = value.direction[0] * distance;
  out[1] = 0;
  out[2] = value.direction[1] * distance;
  return out;
}
/** CPU and GPU share this layout, phase constants, units, and gust equation. */
export const SHARED_WIND_WGSL = /* wgsl */ `
struct WindField { direction:vec4<f32>,gust:vec4<f32>,state:vec4<f32> };
fn sampleWorldWind(position:vec3<f32>,time:f32,wind:WindField)->vec3<f32>{
 let p=position.xz/wind.gust.y;let seed=wind.gust.w*${PHASE_SEED};
 let a=${TAU}*dot(p,wind.direction.xy)+seed+time*wind.gust.z;
 let b=${TAU}*dot(p,vec2(${SECOND_X},${SECOND_Z}))+seed*${SECOND_SEED}+time*wind.gust.z*${SECOND_RATE};
 let strength=wind.direction.z*(1.0+wind.gust.x*.5*(sin(a)+sin(b)))*wind.direction.w;
 return vec3(wind.direction.x,0.0,wind.direction.y)*strength;
}
`;
