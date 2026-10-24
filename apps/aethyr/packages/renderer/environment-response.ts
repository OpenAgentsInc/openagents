import type { V3 } from "./math.ts";

/** Below 2% reflectance, authored specular also represents micro-occlusion. */
export const LOW_F0_REFLECTANCE = 0.02;

function reflectance(f0: readonly number[]) {
  if (
    f0.length !== 3 ||
    !f0.every((v) => Number.isFinite(v) && v >= 0 && v <= 1)
  )
    throw Error("Use three finite reflectance values from zero to one.");
}

/** This authoring response attenuates reflection; it does not restore diffuse energy. */
export function environmentGrazingReflectance(f0: readonly number[]) {
  reflectance(f0);
  return Math.min(1, Math.max(...f0) / LOW_F0_REFLECTANCE);
}

/** Direct-light Fresnel follows the same authored low-reflectance response. */
export function directFresnel(f0: readonly number[], viewDotHalf: number): V3 {
  reflectance(f0);
  if (!Number.isFinite(viewDotHalf) || viewDotHalf < 0 || viewDotHalf > 1)
    throw Error("Use a finite view-to-half cosine from zero to one.");
  const grazing = (1 - viewDotHalf) ** 5;
  const f90 = environmentGrazingReflectance(f0);
  // Preserve ordinary dielectric and metallic arithmetic.
  return f0.map((value) => value + (f90 - value) * grazing) as V3;
}

/** The existing split-sum table stores A and B independently of F0 and F90. */
export function environmentBrdfWeight(
  f0: readonly number[],
  split: readonly number[],
): V3 {
  reflectance(f0);
  if (
    split.length !== 2 ||
    !split.every((v) => Number.isFinite(v) && v >= 0 && v <= 4)
  )
    throw Error("Use two finite bounded environment BRDF coefficients.");
  const f90 = environmentGrazingReflectance(f0);
  return f0.map((value) => value * split[0] + f90 * split[1]) as V3;
}

/** A traced local reflection already includes visibility; do not occlude it twice. */
export function occludedSkyReflection(
  panorama: V3,
  probeVisibility: number,
  confidence: number,
  local?: V3,
): V3 {
  if (
    ![...panorama, ...(local ?? [])].every(
      (v) => Number.isFinite(v) && v >= 0,
    ) ||
    ![probeVisibility, confidence].every(
      (v) => Number.isFinite(v) && v >= 0 && v <= 1,
    )
  )
    throw Error("Use finite sky radiance and visibility from zero to one.");
  if (local) return [...local];
  const visibility = 1 + (probeVisibility - 1) * confidence;
  return panorama.map((v) => v * visibility) as V3;
}

export const ENVIRONMENT_RESPONSE_WGSL = `
fn environmentGrazingReflectance(f0:vec3<f32>)->f32{
 return clamp(max(f0.r,max(f0.g,f0.b))*50.0,0.0,1.0);
}
fn directFresnel(f0:vec3<f32>,viewDotHalf:f32)->vec3<f32>{
 let f90=environmentGrazingReflectance(f0);
 if f90>=1.0{return f0+(vec3(1.0)-f0)*pow(1.0-viewDotHalf,5.0);}
 return f0+(vec3(f90)-f0)*pow(1.0-viewDotHalf,5.0);
}
fn environmentBrdfWeight(f0:vec3<f32>,split:vec2<f32>)->vec3<f32>{
 let f90=environmentGrazingReflectance(f0);
 if f90>=1.0{return f0*split.x+vec3(split.y);}
 return f0*split.x+vec3(f90*split.y);
}
fn occludedSkyReflection(panorama:vec3<f32>,probeVisibility:f32,confidence:f32,local:vec4<f32>)->vec3<f32>{
 if local.w>0.5{return local.rgb;}
 return panorama*mix(1.0,probeVisibility,confidence);
}
`;
