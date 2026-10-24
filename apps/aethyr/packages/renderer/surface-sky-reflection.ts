/** Optional low-order reflection of a local incoming-radiance field.
 * Deconvolution cannot recover frequencies omitted by SH9. This model is a rough
 * reflection approximation, not a replacement for ray visibility or a cubemap.
 * SH convolution: https://graphics.stanford.edu/papers/envmap/
 */
export const SURFACE_SKY_REFLECTION_MODEL = "sh9-ggx-rough-v1";
export const SURFACE_SKY_REFLECTION_MIN_ROUGHNESS = 0.4;

/** Zonal moments of the existing 128-sample N=V=R GGX filter at its six mip knots.
 * Each row is [roughness, mean(P1), mean(P2)], weighted by positive NoL.
 * The finite sample pattern has small azimuthal anisotropy that this zonal
 * approximation omits. These are transport moments, not brightness gains.
 */
export const SURFACE_SKY_GGX_BANDS = Object.freeze([
  Object.freeze([0, 1, 1]),
  Object.freeze([0.2, 0.9881020525772053, 0.9660931134614005]),
  Object.freeze([0.4, 0.9185376404255086, 0.7851864363848458]),
  Object.freeze([0.6, 0.8161982231250541, 0.5474638237751819]),
  Object.freeze([0.8, 0.7280480797372556, 0.3660967247949797]),
  Object.freeze([1, 0.671875, 0.26171875]),
]);

/** Match the default 256-wide, six-level prefilter's linear mip interpolation. */
export function surfaceSkyReflectionBands(roughness: number) {
  if (
    !Number.isFinite(roughness) ||
    roughness < SURFACE_SKY_REFLECTION_MIN_ROUGHNESS ||
    roughness > 1
  )
    return undefined;
  const level = roughness * 5,
    lower = Math.min(4, Math.floor(level)),
    t = level - lower,
    a = SURFACE_SKY_GGX_BANDS[lower],
    b = SURFACE_SKY_GGX_BANDS[lower + 1];
  return [1, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

/** RGB9 coefficients have already received Lambert bands [1, 2/3, 1/4].
 * The fourth result is one only for supported roughness and finite, nonnegative
 * reconstructed RGB. Zero requests the caller's existing reflection fallback;
 * it does not describe a black surface. No AO, probe visibility, source gain,
 * Fresnel, radiance-storage scale, or exposure is applied here.
 */
export function sh9RoughReflection(
  coefficients: ArrayLike<number>,
  direction: ArrayLike<number>,
  roughness: number,
): [number, number, number, number] {
  const bands = surfaceSkyReflectionBands(roughness);
  if (!bands || coefficients.length !== 27 || direction.length !== 3)
    return [0, 0, 0, 0];
  const length = Math.hypot(direction[0], direction[1], direction[2]);
  if (!Number.isFinite(length) || length < 1e-8) return [0, 0, 0, 0];
  const [x, y, z] = [direction[0], direction[1], direction[2]].map(
    (value) => value / length,
  );
  const basis = [
      0.2820947918,
      0.4886025119 * y,
      0.4886025119 * z,
      0.4886025119 * x,
      1.0925484306 * x * y,
      1.0925484306 * y * z,
      0.3153915653 * (3 * z * z - 1),
      1.0925484306 * x * z,
      0.5462742153 * (x * x - y * y),
    ],
    sum = [0, 0, 0];
  for (let k = 0; k < 9; k++) {
    const band = k === 0 ? 0 : k < 4 ? 1 : 2,
      factor = (basis[k] * bands[band]) / [1, 2 / 3, 1 / 4][band];
    for (let c = 0; c < 3; c++) {
      const value = coefficients[k * 3 + c];
      if (!Number.isFinite(value)) return [0, 0, 0, 0];
      sum[c] += value * factor;
    }
  }
  if (sum.some((value) => !Number.isFinite(value) || value < 0))
    return [0, 0, 0, 0];
  return [sum[0], sum[1], sum[2], 1];
}

export const WGSL_SURFACE_SKY_REFLECTION = `
fn sh9RoughReflection(coefficients:array<vec3<f32>,9>,direction:vec3<f32>,rough:f32)->vec4<f32>{
 if !(rough>=0.4&&rough<=1.0)||!all(abs(direction)<=vec3(3.402823e38))||dot(direction,direction)<1e-16{return vec4(0.0);}
 let lengthsq=dot(direction,direction);if !(lengthsq<=3.402823e38){return vec4(0.0);}
 let d=direction*inverseSqrt(lengthsq);let x=d.x;let y=d.y;let z=d.z;
 let knots=array<vec2<f32>,6>(vec2(1.0),vec2(.9881020525772053,.9660931134614005),vec2(.9185376404255086,.7851864363848458),vec2(.8161982231250541,.5474638237751819),vec2(.7280480797372556,.3660967247949797),vec2(.671875,.26171875));
 let level=rough*5.0;let lower=min(4u,u32(floor(level)));let q=mix(knots[lower],knots[lower+1u],level-f32(lower));
 let basis=array<f32,9>(.2820947918,.4886025119*y,.4886025119*z,.4886025119*x,1.0925484306*x*y,1.0925484306*y*z,.3153915653*(3.0*z*z-1.0),1.0925484306*x*z,.5462742153*(x*x-y*y));
 var sum=vec3(0.0);for(var k=0u;k<9u;k++){
  if !all(abs(coefficients[k])<=vec3(3.402823e38)){return vec4(0.0);}
  var factor=1.0;if k>0u{factor=select(q.y*4.0,q.x*1.5,k<4u);}sum+=coefficients[k]*basis[k]*factor;
 }
 if !all(sum>=vec3(0.0))||!all(sum<=vec3(3.402823e38)){return vec4(0.0);}
 return vec4(sum,1.0);
}
`;
