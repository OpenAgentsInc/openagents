import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import { cross, dot, type V3 } from "./math.ts";

/** Only the ordinary pivot-rotation branch; this is not the complete source flutter graph. */
export interface WindSway {
  enabled: boolean;
  /** Original source component origin, not the mesh bounds center or a baked spline origin. */
  pivot: V3;
  worldUp: V3;
  windDirection: V3;
  /** Frozen exact source component sphere radius, converted from centimeters to meters. */
  radiusMeters: number;
  windSpeed: number;
  swaySpeed: number;
  swayAmount: number;
}
function finite(value: number, minimum: number, maximum: number) {
  if (!Number.isFinite(value) || value < minimum || value > maximum)
    throw Error("Use finite wind inputs within the supported range.");
}
function direction(value: V3): V3 {
  if (value.length !== 3) throw Error("Use a three-component wind direction.");
  value.forEach((n) => finite(n, -1e6, 1e6));
  const length = Math.hypot(...value);
  return length < 1e-10 ? [0, 0, 0] : (value.map((n) => n / length) as V3);
}
export function validateWindSway(settings: WindSway) {
  if (typeof settings.enabled !== "boolean" || settings.pivot.length !== 3)
    throw Error("Use complete wind sway settings.");
  settings.pivot.forEach((n) => finite(n, -1e6, 1e6));
  direction(settings.worldUp);
  direction(settings.windDirection);
  finite(settings.radiusMeters, 1e-6, 1e5);
  finite(settings.windSpeed, -100, 100);
  finite(settings.swaySpeed, -100, 100);
  finite(settings.swayAmount, -1, 1);
}
/** Standard Rodrigues rotation expressed as an offset; source rotation amounts are turns. */
export function windRotationOffset(
  point: V3,
  pivot: V3,
  axis: V3,
  turns: number,
): V3 {
  point.forEach((n) => finite(n, -1e6, 1e6));
  pivot.forEach((n) => finite(n, -1e6, 1e6));
  finite(turns, -1e12, 1e12);
  const a = direction(axis);
  if (Math.hypot(...a) < 1e-10) return [0, 0, 0];
  const r = point.map((n, i) => n - pivot[i]) as V3,
    angle = (turns - Math.floor(turns)) * 2 * Math.PI,
    c = Math.cos(angle),
    s = Math.sin(angle),
    d = dot(a, r),
    perpendicular = cross(a, r);
  return r.map(
    (n, i) => n * (c - 1) + perpendicular[i] * s + a[i] * d * (1 - c),
  ) as V3;
}
/** Source phase uses world centimeters; standard vector algebra is evaluated in canonical Y-up. */
export function windSwayOffset(
  point: V3,
  green: number,
  time: number,
  settings: WindSway,
): V3 {
  validateWindSway(settings);
  finite(green, 0, 1);
  finite(time, 0, MAX_WORLD_ANIMATION_TIME);
  point.forEach((n) => finite(n, -1e6, 1e6));
  if (!settings.enabled || green === 0 || settings.swayAmount === 0)
    return [0, 0, 0];
  const wind = direction(settings.windDirection),
    up = direction(settings.worldUp),
    axis = cross(up, wind);
  const turns =
    dot(settings.pivot, wind) * 100 +
    time * settings.windSpeed * settings.swaySpeed;
  const r = Math.hypot(...point.map((n, i) => n - settings.pivot[i])),
    weight = green * (r / settings.radiusMeters) ** 2 * settings.swayAmount;
  return windRotationOffset(point, settings.pivot, axis, turns).map(
    (n) => n * weight,
  ) as V3;
}
/** Independent of animation phase; do not replace the frozen source radius with this bound. */
export function windSwayBound(
  maxPivotDistanceMeters: number,
  maximumGreen: number,
  settings: WindSway,
) {
  validateWindSway(settings);
  finite(maxPivotDistanceMeters, 0, 1e5);
  finite(maximumGreen, 0, 1);
  return settings.enabled
    ? ((2 * maxPivotDistanceMeters ** 3) / settings.radiusMeters ** 2) *
        Math.abs(settings.swayAmount) *
        maximumGreen
    : 0;
}

export const WIND_SWAY_WGSL = `
fn windUnit(v:vec3<f32>)->vec3<f32>{let size=length(v);if size<.0000000001{return vec3(0.0);}return v/size;}
fn windRotationOffset(point:vec3<f32>,pivot:vec3<f32>,axis:vec3<f32>,turns:f32)->vec3<f32>{
 let a=windUnit(axis);if dot(a,a)<.5{return vec3(0.0);}let r=point-pivot;let angle=fract(turns)*6.28318530718;let c=cos(angle);let s=sin(angle);return r*(c-1.0)+cross(a,r)*s+a*dot(a,r)*(1.0-c);
}
// parameters=(windSpeed,swaySpeed,swayAmount,frozen source radius in meters).
fn windSwayOffset(point:vec3<f32>,pivot:vec3<f32>,up:vec3<f32>,direction:vec3<f32>,parameters:vec4<f32>,time:f32,green:f32,enabled:bool)->vec3<f32>{
 if !enabled||green<=0.0||parameters.z==0.0||parameters.w<=0.0{return vec3(0.0);}
 let wind=windUnit(direction);let axis=cross(windUnit(up),wind);let turns=dot(pivot,wind)*100.0+time*parameters.x*parameters.y;let radius=length(point-pivot);return windRotationOffset(point,pivot,axis,turns)*(clamp(green,0.0,1.0)*pow(radius/parameters.w,2.0)*parameters.z);
}
`;
