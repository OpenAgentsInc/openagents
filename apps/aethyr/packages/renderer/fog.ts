/**
 * Owned unit conversion for the imported Medieval Town height-fog profile.
 *
 * UE5.8 SceneCore.cpp scales artist density/falloff by 1/1000 in centimeters.
 * HeightFogCommon.ush integrates the base-2 height distribution and applies
 * base-2 transmittance. Converting both exponentials gives the natural-exponent
 * coefficients below. These are derived formulas, not vendor shader code.
 * Source metadata and active project/CVar settings remain separate facts.
 */
export const SOURCE_FOG_DENSITY_FACTOR = 0.25;
export const SOURCE_FOG_FALLOFF_FACTOR = 0.4;
export const SOURCE_FOG_REFERENCE_HEIGHT = -60;

/** Natural-exponential height rate, in inverse meters. */
export const FOG_HEIGHT_RATE = (SOURCE_FOG_FALLOFF_FACTOR / 10) * Math.LN2;
/** Existing editor density values specify extinction at this height, in meters. */
export const FOG_REFERENCE_HEIGHT = 1;
/** Start integration at the camera; source metadata specifies 0 centimeters. */
export const FOG_START_DISTANCE = 0;
export const FOG_MAX_OPACITY = 1;

/** Natural-exponential analytical extinction at the source reference height. */
export const SOURCE_FOG_EXTINCTION_AT_SOURCE_HEIGHT =
  (SOURCE_FOG_DENSITY_FACTOR / 10) * Math.LN2 ** 2;
/** Inverse-meter extinction at the editor's one-meter reference height. */
export const SOURCE_FOG_DENSITY_AT_REFERENCE =
  SOURCE_FOG_EXTINCTION_AT_SOURCE_HEIGHT *
  Math.exp(
    -FOG_HEIGHT_RATE * (FOG_REFERENCE_HEIGHT - SOURCE_FOG_REFERENCE_HEIGHT),
  );

/** Authored FLinearColor RGB; alpha 3 is excluded, with no sRGB conversion. */
export const SOURCE_FOG_INSCATTERING_LINEAR = Object.freeze([
  0.609237, 0.937705, 1.5,
] as const);
/** Black source directional inscattering remains zero at any sun luminance. */
export const SOURCE_DIRECTIONAL_INSCATTERING_LINEAR = Object.freeze([
  0, 0, 0,
] as const);
export const SOURCE_DIRECTIONAL_INSCATTERING_START_DISTANCE = 100;
export const SOURCE_DIRECTIONAL_INSCATTERING_EXPONENT = 4;

/**
 * The source volume uses a 0.5 match factor instead of analytical ln(2)^2.
 * Keep this difference explicit; overlapping paths must not add extinction twice.
 */
export const SOURCE_VOLUME_EXTINCTION_AT_SOURCE_HEIGHT =
  (SOURCE_FOG_DENSITY_FACTOR / 10) * 0.5;
export const SOURCE_VOLUME_DISTANCE = 60;
export const SOURCE_VOLUME_PHASE_G = 0.2;
export const SOURCE_VOLUME_EXTINCTION_SCALE = 1;
/** FColor 232 converts from sRGB to linear, rather than being divided by 255 only. */
export const SOURCE_VOLUME_ALBEDO_LINEAR = ((232 / 255 + 0.055) / 1.055) ** 2.4;

/** Returns natural-exponential analytical extinction, in inverse meters. */
export function sourceFogExtinctionAtHeight(height: number): number {
  if (!Number.isFinite(height) || Math.abs(height) > 20_000)
    throw new Error("Use a fog sample height within 20,000 meters.");
  return (
    SOURCE_FOG_EXTINCTION_AT_SOURCE_HEIGHT *
    Math.exp(-FOG_HEIGHT_RATE * (height - SOURCE_FOG_REFERENCE_HEIGHT))
  );
}

/** Optical depths above this value are visually indistinguishable from opaque fog. */
export const FOG_OPTICAL_DEPTH_LIMIT = 80;
export const FOG_SEGMENT_LIMITS = Object.freeze({
  distance: 100_000,
  height: 20_000,
  density: 10_000,
  heightRate: 10,
  densityRatio: 1_000_000,
});

function bounded(value: number, min: number, max: number, name: string) {
  if (!Number.isFinite(value) || value < min || value > max)
    throw new Error(`Use ${name} between ${min} and ${max}.`);
  return value;
}

/**
 * Integral of densityAtRef * exp(-heightRate * (y - referenceY)) on a ray segment.
 * Distances and heights use meters, and directionY is a unit ray's Y component.
 * The log-domain integral avoids an incorrect exponent clamp on long sky rays.
 * Returned optical depth saturates at 80; no transport calculation needs a larger value.
 */
export function segmentFogOpticalDepth(
  eyeY: number,
  directionY: number,
  start: number,
  end: number,
  densityAtRef: number,
  heightRate: number,
  referenceY: number,
): number {
  bounded(
    eyeY,
    -FOG_SEGMENT_LIMITS.height,
    FOG_SEGMENT_LIMITS.height,
    "eye height",
  );
  bounded(directionY, -1, 1, "ray height direction");
  bounded(start, 0, FOG_SEGMENT_LIMITS.distance, "fog segment start");
  bounded(end, start, FOG_SEGMENT_LIMITS.distance, "fog segment end");
  bounded(densityAtRef, 0, FOG_SEGMENT_LIMITS.density, "fog extinction");
  bounded(heightRate, 0, FOG_SEGMENT_LIMITS.heightRate, "fog height rate");
  bounded(
    referenceY,
    -FOG_SEGMENT_LIMITS.height,
    FOG_SEGMENT_LIMITS.height,
    "fog reference height",
  );
  const travel = end - start;
  if (densityAtRef === 0 || travel === 0) return 0;
  const k = heightRate * directionY * travel;
  let logAverage: number;
  if (Math.abs(k) < 0.001)
    logAverage = Math.log(1 - k / 2 + (k * k) / 6 - (k * k * k) / 24);
  else if (k < -20) logAverage = -k + Math.log1p(-Math.exp(k)) - Math.log(-k);
  else if (k > 20) logAverage = Math.log1p(-Math.exp(-k)) - Math.log(k);
  else logAverage = Math.log(-Math.expm1(-k) / k);
  const logTau =
    Math.log(densityAtRef) -
    heightRate * (eyeY + directionY * start - referenceY) +
    Math.log(travel) +
    logAverage;
  return logTau >= Math.log(FOG_OPTICAL_DEPTH_LIMIT)
    ? FOG_OPTICAL_DEPTH_LIMIT
    : Math.exp(Math.max(-80, logTau));
}

/** Density ratio relative to analytic density, height rate, reference height, start distance. */
export type FogVolumeMedium = readonly [number, number, number, number];
/** Maximum distance and enabled flag (0 or 1). */
export type FogVolumeRange = readonly [number, number];

/**
 * Surface transport before adding independently integrated volume light.
 * The enabled volume replaces the analytic medium on [start, maxDistance].
 * Returns [surface transmittance, constant analytic fog-color weight]. Analytic
 * fog before a nonzero volume start is retained and attenuates the volume light.
 * Source start 0 reduces the fill weight to Tvolume * (1 - TanalyticFar).
 */
export function heightFogComposition(
  eyeY: number,
  directionY: number,
  distance: number,
  analyticDensityAtRef: number,
  analyticHeightRate: number,
  analyticReferenceY: number,
  analyticStart: number,
  volumeMedium: FogVolumeMedium,
  volumeRange: FogVolumeRange,
): [number, number] {
  bounded(distance, 0, FOG_SEGMENT_LIMITS.distance, "fog distance");
  bounded(analyticStart, 0, FOG_SEGMENT_LIMITS.distance, "analytic fog start");
  if (volumeMedium.length !== 4 || volumeRange.length !== 2)
    throw new Error(
      "Use four volume-medium values and two volume-range values.",
    );
  const [ratio, rate, referenceY, start] = volumeMedium;
  const [maximum, enabled] = volumeRange;
  bounded(ratio, 0, FOG_SEGMENT_LIMITS.densityRatio, "volume density ratio");
  bounded(rate, 0, FOG_SEGMENT_LIMITS.heightRate, "volume height rate");
  bounded(
    referenceY,
    -FOG_SEGMENT_LIMITS.height,
    FOG_SEGMENT_LIMITS.height,
    "volume reference height",
  );
  bounded(start, 0, FOG_SEGMENT_LIMITS.distance, "volume start");
  bounded(
    maximum,
    start,
    FOG_SEGMENT_LIMITS.distance,
    "volume maximum distance",
  );
  if (enabled !== 0 && enabled !== 1)
    throw new Error("Use a zero or one volume enabled flag.");
  const analytic = (first: number, last: number) =>
    segmentFogOpticalDepth(
      eyeY,
      directionY,
      first,
      last,
      analyticDensityAtRef,
      analyticHeightRate,
      analyticReferenceY,
    );
  const first = Math.min(analyticStart, distance);
  // This call validates analytic parameters even when the enabled volume covers the entire ray.
  const fullAnalytic = analytic(first, distance);
  const volumeStart = Math.min(start, distance),
    volumeEnd = Math.min(maximum, distance);
  if (!enabled || volumeEnd <= volumeStart)
    return [Math.exp(-fullAnalytic), -Math.expm1(-fullAnalytic)];
  const before = analytic(first, Math.max(first, volumeStart));
  const within = segmentFogOpticalDepth(
    eyeY,
    directionY,
    volumeStart,
    volumeEnd,
    bounded(
      analyticDensityAtRef * ratio,
      0,
      FOG_SEGMENT_LIMITS.density,
      "volume extinction",
    ),
    rate,
    referenceY,
  );
  const after = analytic(Math.max(first, volumeEnd), distance);
  return [
    Math.exp(-Math.min(before + within + after, FOG_OPTICAL_DEPTH_LIMIT)),
    -Math.expm1(-before) + Math.exp(-before - within) * -Math.expm1(-after),
  ];
}

/** Pure WGSL transport math; no Frame layout, bindings, color, or radiance scaling. */
export const FOG_SEGMENT_WGSL = `fn segmentFogOpticalDepth(eyeY:f32,directionY:f32,start:f32,end:f32,densityAtRef:f32,heightRate:f32,referenceY:f32)->f32{let travel=max(end-start,0.0);if densityAtRef<=0.0||travel<=0.0{return 0.0;}let k=heightRate*directionY*travel;var logAverage=0.0;if abs(k)<0.001{logAverage=log(1.0-k*0.5+k*k/6.0-k*k*k/24.0);}else if k < -20.0{logAverage=-k-log(-k);}else if k > 20.0{logAverage=-log(k);}else{logAverage=log((1.0-exp(-k))/k);}let logTau=log(densityAtRef)-heightRate*(eyeY+directionY*start-referenceY)+log(travel)+logAverage;if logTau>=4.382026635{return 80.0;}return exp(max(-80.0,logTau));}fn fogOneMinusTransmittance(tau:f32)->f32{if tau<0.001{return tau*(1.0-tau*0.5+tau*tau/6.0);}return 1.0-exp(-tau);}fn heightFogComposition(eyeY:f32,directionY:f32,distance:f32,analyticDensityAtRef:f32,analyticHeightRate:f32,analyticReferenceY:f32,analyticStart:f32,volumeMedium:vec4<f32>,volumeRange:vec2<f32>)->vec2<f32>{let finish=max(distance,0.0);let first=min(max(analyticStart,0.0),finish);let vstart=min(max(volumeMedium.w,0.0),finish);let vend=min(max(volumeRange.x,vstart),finish);if volumeRange.y<0.5||vend<=vstart{let tau=segmentFogOpticalDepth(eyeY,directionY,first,finish,analyticDensityAtRef,analyticHeightRate,analyticReferenceY);return vec2(exp(-tau),fogOneMinusTransmittance(tau));}let before=segmentFogOpticalDepth(eyeY,directionY,first,max(first,vstart),analyticDensityAtRef,analyticHeightRate,analyticReferenceY);let within=segmentFogOpticalDepth(eyeY,directionY,vstart,vend,analyticDensityAtRef*volumeMedium.x,volumeMedium.y,volumeMedium.z);let after=segmentFogOpticalDepth(eyeY,directionY,max(first,vend),finish,analyticDensityAtRef,analyticHeightRate,analyticReferenceY);return vec2(exp(-min(before+within+after,80.0)),fogOneMinusTransmittance(before)+exp(-before-within)*fogOneMinusTransmittance(after));}`;
