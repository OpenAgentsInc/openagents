/** Owned coverage and shading choices for air mixed into a thin water sheet. */
export type WaterAirModel =
  | { kind: "henyey-greenstein-v1"; nozzleFraction: number; asymmetry: number }
  | {
      kind:
        | "diffuse-foam-v1"
        | "source-energy-foam-v1"
        | "source-energy-foam-additive-v1";
      nozzleFraction: number;
      asymmetry?: never;
    };
export function validateWaterAirModel(model: WaterAirModel) {
  if (
    !model ||
    ![
      "henyey-greenstein-v1",
      "diffuse-foam-v1",
      "source-energy-foam-v1",
      "source-energy-foam-additive-v1",
    ].includes(model.kind) ||
    !Number.isFinite(model.nozzleFraction) ||
    model.nozzleFraction < 0 ||
    model.nozzleFraction > 1
  )
    throw Error("Use a supported, bounded nozzle aeration model.");
  if (model.kind === "henyey-greenstein-v1") {
    if (!Number.isFinite(model.asymmetry) || Math.abs(model.asymmetry) > 0.8)
      throw Error("Use bounded phase asymmetry.");
  } else if (model.asymmetry !== undefined)
    throw Error("Diffuse foam has no phase-asymmetry setting.");
  if (
    (model.kind === "source-energy-foam-v1" ||
      model.kind === "source-energy-foam-additive-v1") &&
    Object.keys(model).some((key) => !["kind", "nozzleFraction"].includes(key))
  )
    throw Error("Source-energy foam accepts only nozzle coverage.");
  return model;
}
/** Both directions point away from the medium: toward the source and toward the viewer.
 * Thus forward scattering peaks at cosine -1, not +1. The phase integral is one.
 * Mathematical reference: https://pbr-book.org/4ed/Volume_Scattering/Phase_Functions
 */
export function waterAirPhase(cosineBothAway: number, g: number) {
  if (
    !Number.isFinite(cosineBothAway) ||
    Math.abs(cosineBothAway) > 1 ||
    !Number.isFinite(g) ||
    Math.abs(g) > 0.8
  )
    throw Error("Use a finite scattering angle and bounded asymmetry.");
  const denominator = 1 + g * g + 2 * g * cosineBothAway;
  return (1 - g * g) / (4 * Math.PI * denominator ** 1.5);
}
export function waterNozzleCoverage(progress: number, nozzleFraction: number) {
  if (
    !Number.isFinite(progress) ||
    !Number.isFinite(nozzleFraction) ||
    nozzleFraction < 0 ||
    nozzleFraction > 1
  )
    throw Error("Use finite stream progression and nozzle coverage.");
  const t = Math.max(0, Math.min(1, (progress - 0.15) / 0.8));
  return nozzleFraction + (1 - nozzleFraction) * t * t * (3 - 2 * t);
}
/** Convert the existing E/pi SH9 representation to incident radiance and convolve HG.
 * Supply the normalized, environment-rotated direction opposite the viewer direction.
 * The l<=2 approximation is clamped only after summing; it adds no intensity gain.
 */
export function waterAirFromDiffuseSH(
  sh: ArrayLike<number>,
  oppositeView: readonly number[],
  g: number,
) {
  if (
    sh.length !== 36 ||
    oppositeView.length !== 3 ||
    oppositeView.some((v) => !Number.isFinite(v)) ||
    Math.abs(Math.hypot(...oppositeView) - 1) > 0.001 ||
    !Number.isFinite(g) ||
    Math.abs(g) > 0.8
  )
    throw Error(
      "Use complete diffuse coefficients and a unit scattering direction.",
    );
  const [x, y, z] = oppositeView,
    b = [
      0.2820947918,
      0.4886025119 * y,
      0.4886025119 * z,
      0.4886025119 * x,
      1.0925484306 * x * y,
      1.0925484306 * y * z,
      0.3153915653 * (3 * z * z - 1),
      1.0925484306 * x * z,
      0.5462742153 * (x * x - y * y),
    ];
  const sum = [0, 0, 0];
  for (let i = 0; i < 9; i++) {
    const factor = i === 0 ? 1 : i < 4 ? 1.5 * g : 4 * g * g;
    for (let c = 0; c < 3; c++) {
      const coefficient = sh[i * 4 + c];
      if (!Number.isFinite(coefficient) || Math.abs(coefficient) > 1e9)
        throw Error("Scattering coefficients exceed their finite range.");
      sum[c] += coefficient * b[i] * factor;
    }
  }
  return sum.map((v) => Math.max(v, 0));
}
export const WGSL_WATER_AIR_SCATTERING = `fn waterAirPhase(cosineBothAway:f32,g:f32)->f32{let denominator=1.0+g*g+2.0*g*clamp(cosineBothAway,-1.0,1.0);return(1.0-g*g)/(12.566370614*denominator*sqrt(denominator));}fn waterNozzleCoverage(progress:f32,nozzle:f32)->f32{let t=clamp((progress-.15)/.8,0.0,1.0);return nozzle+(1.0-nozzle)*t*t*(3.0-2.0*t);}fn waterAirSHFactor(index:u32,g:f32)->f32{if index==0u{return 1.0;}if index<4u{return 1.5*g;}return 4.0*g*g;}`;
