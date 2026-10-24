import type { VegetationTransmission } from "../assets/src/scene.ts";

type V3 = readonly [number, number, number];
export const FOLIAGE_TRANSMISSION_MODEL = "bounded-forward-sheet-v1" as const;
const MAX_G = 0.8;

/** Zero retains the legacy lobe; 1+g selects the owned bounded angular lobe. */
export function foliageTransmissionCode(
  profile: VegetationTransmission | undefined,
): number {
  if (profile === undefined) return 0;
  if (
    !profile ||
    typeof profile !== "object" ||
    Array.isArray(profile) ||
    profile.model !== FOLIAGE_TRANSMISSION_MODEL ||
    Object.keys(profile).length !== 2 ||
    Object.keys(profile).some(
      (key) => key !== "model" && key !== "anisotropy",
    ) ||
    !Number.isFinite(profile.anisotropy) ||
    profile.anisotropy < 0 ||
    profile.anisotropy > MAX_G
  )
    throw new RangeError(
      "Use the supported foliage transmission model and anisotropy from zero to 0.8.",
    );
  return 1 + profile.anisotropy;
}

/**
 * HG uses two directions pointing away from the sheet; forward is L dot V=-1.
 * This min(1,4*pi*pHG) is attenuation, not an additional scattering-energy gain.
 * Mathematical reference: https://pbr-book.org/4ed/Volume_Scattering/Phase_Functions
 * A phase-modulated sheet is an owned heuristic, not a source foliage BSDF.
 */
export function foliageAngularAttenuation(
  cosineBothAway: number,
  anisotropy: number,
): number {
  if (
    !Number.isFinite(cosineBothAway) ||
    Math.abs(cosineBothAway) > 1 ||
    !Number.isFinite(anisotropy) ||
    anisotropy < 0 ||
    anisotropy > MAX_G
  )
    throw new RangeError(
      "Use a finite scattering cosine and bounded foliage anisotropy.",
    );
  const denominator =
    1 + anisotropy * anisotropy + 2 * anisotropy * cosineBothAway;
  return Math.min(
    1,
    (1 - anisotropy * anisotropy) / (denominator * Math.sqrt(denominator)),
  );
}

function vector(value: V3, direction: boolean) {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    ![...value].every(Number.isFinite) ||
    (direction && Math.abs(Math.hypot(...value) - 1) > 1e-6)
  )
    throw new RangeError(
      "Use finite three-component colors and unit foliage directions.",
    );
}
function dot(a: V3, b: V3) {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

/** Unit directions, source subsurface color, and the packed material profile code. */
export function foliageTransmission(
  subsurface: V3,
  normal: V3,
  light: V3,
  view: V3,
  profileCode = 0,
): V3 {
  vector(subsurface, false);
  for (const direction of [normal, light, view]) vector(direction, true);
  if (
    !Number.isFinite(profileCode) ||
    (profileCode !== 0 && (profileCode < 1 || profileCode > 1 + MAX_G))
  )
    throw new RangeError("Use a prepared foliage transmission profile code.");
  const back = Math.max(dot([-normal[0], -normal[1], -normal[2]], light), 0);
  // Preserve operand order and unclamped source values for untagged materials.
  if (profileCode === 0)
    return subsurface.map((value) => value * back * 0.35) as unknown as V3;
  const angular = foliageAngularAttenuation(
    Math.max(-1, Math.min(1, dot(light, view))),
    profileCode - 1,
  );
  return subsurface.map(
    (value) => ((Math.max(0, Math.min(1, value)) * back) / Math.PI) * angular,
  ) as unknown as V3;
}

/** No bindings or normal modification; the caller applies incident light and visibility once. */
export const WGSL_FOLIAGE_TRANSMISSION = `
fn foliageAngularAttenuation(cosineBothAway:f32,g:f32)->f32{
 let denominator=1.0+g*g+2.0*g*clamp(cosineBothAway,-1.0,1.0);
 return min(1.0,(1.0-g*g)/(denominator*sqrt(denominator)));
}
fn foliageTransmission(subsurface:vec3<f32>,n:vec3<f32>,l:vec3<f32>,view:vec3<f32>,profileCode:f32)->vec3<f32>{
 if profileCode==0.0{return subsurface*max(dot(-n,l),0.0)*0.35;}
 let g=clamp(profileCode-1.0,0.0,0.8);
 let angular=foliageAngularAttenuation(dot(l,view),g);
 return clamp(subsurface,vec3(0.0),vec3(1.0))*max(dot(-n,l),0.0)/3.141592653589793*angular;
}
`;
