import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import type { V3 } from "./math.ts";

export const WIND_FLUTTER_CLOCK_SCALE = Math.fround(0.285);
export const WIND_FLUTTER_PERIOD = 1 / WIND_FLUTTER_CLOCK_SCALE;
export interface WindFlutter {
  profile: "source-triangle-flutter-v1";
  intensity: number;
  weight: number;
  clockScale: number;
  smallWaveCm: 1;
  largeWaveCm: 200;
  /** Owned vertex sampling policy; source render equivalence remains unqualified. */
  vertexLod: 0;
}

function finite(value: number, minimum: number, maximum: number) {
  if (!Number.isFinite(value) || value < minimum || value > maximum)
    throw Error("Use finite flutter values within the supported range.");
}
export function validateWindFlutter(settings: WindFlutter) {
  if (
    settings.profile !== "source-triangle-flutter-v1" ||
    settings.clockScale !== WIND_FLUTTER_CLOCK_SCALE ||
    settings.smallWaveCm !== 1 ||
    settings.largeWaveCm !== 200 ||
    settings.vertexLod !== 0
  )
    throw Error("Prepare the supported triangle-wave flutter profile.");
  finite(settings.intensity, 0, 1);
  finite(settings.weight, 0, 1);
}

/** Reduce the double-precision world age before packing the shared float clock. */
export function boundedFlutterTime(seconds: number) {
  finite(seconds, 0, MAX_WORLD_ANIMATION_TIME);
  return seconds % WIND_FLUTTER_PERIOD;
}

function smoothTriangle(value: number) {
  const shifted = value + 0.5,
    fraction = shifted - Math.floor(shifted),
    triangle = Math.abs(fraction * 2 - 1);
  return triangle * triangle * (3 - 2 * triangle);
}

/** Independently evaluated periodic node semantics in source centimeters. */
export function windFlutterOffset(
  point: V3,
  green: number,
  seconds: number,
  additionalLinearColor: V3,
  settings: WindFlutter,
): V3 {
  validateWindFlutter(settings);
  if (point.length !== 3 || additionalLinearColor.length !== 3)
    throw Error("Use three-component flutter positions and colors.");
  point.forEach((n) => finite(n, -1e6, 1e6));
  additionalLinearColor.forEach((n) => finite(n, 0, 1));
  finite(green, 0, 1);
  const time = boundedFlutterTime(seconds) * settings.clockScale;
  if (green === 0) return [0, 0, 0];
  // Canonical (X,Y,Z) meters map to source (X,Z,Y) centimeters.
  const source: V3 = [point[0] * 100, point[2] * 100, point[1] * 100],
    small = source.map((n) => smoothTriangle(n / settings.smallWaveCm + time)),
    large = smoothTriangle(source[1] / settings.largeWaveCm + time),
    angle = (Math.hypot(...small) + large) * 2 * Math.PI,
    scale = settings.intensity * settings.weight,
    sourceOffset: V3 = [
      additionalLinearColor[0],
      additionalLinearColor[1] - 10 * Math.sin(angle) * scale,
      additionalLinearColor[2] + 10 * (Math.cos(angle) - 1) * scale,
    ];
  return [
    sourceOffset[0] * 0.01 * green,
    sourceOffset[2] * 0.01 * green,
    sourceOffset[1] * 0.01 * green,
  ];
}

/** Twenty centimeters bounds the rotation diameter; color contributes at most sqrt(3) cm. */
export function windFlutterBound(green: number, settings: WindFlutter) {
  validateWindFlutter(settings);
  finite(green, 0, 1);
  return (
    green * (0.2 * settings.intensity * settings.weight + Math.sqrt(3) * 0.01)
  );
}

export const WIND_FLUTTER_WGSL = `
fn windSmoothTriangle(value:vec3<f32>)->vec3<f32>{let t=abs(fract(value+vec3(.5))*2.0-vec3(1.0));return t*t*(vec3(3.0)-2.0*t);}
// flutter=(enabled,intensity,weight,clockScale); waves=(smallCm,largeCm,LOD0,0).
// phaseSeconds is host-reduced before float packing; ordinary sway retains its own clock.
fn windFlutterOffset(point:vec3<f32>,green:f32,phaseSeconds:f32,additional:vec3<f32>,flutter:vec4<f32>,waves:vec4<f32>)->vec3<f32>{
 if flutter.x<.5||green<=0.0{return vec3(0.0);}
 let source=point.xzy*100.0;let time=phaseSeconds*flutter.w;
 let small=windSmoothTriangle(source/waves.x+vec3(time));
 let large=windSmoothTriangle(source/waves.y+vec3(0.0,time,0.0)).y;
 let angle=(length(small)+large)*6.28318530718;let scale=flutter.y*flutter.z;
 let offset=additional+vec3(0.0,-10.0*sin(angle)*scale,10.0*(cos(angle)-1.0)*scale);
 return offset.xzy*(0.01*clamp(green,0.0,1.0));
}
`;
