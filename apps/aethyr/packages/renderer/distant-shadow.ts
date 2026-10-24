import {
  add,
  cross,
  dot,
  lookAt,
  multiply,
  normalize,
  orthographic,
  scale,
  type V3,
} from "./math.ts";

export interface DirectionalShadowFit {
  ahead: number;
  extent: number;
  resolution: number;
  eyeDistance: number;
  near: number;
  far: number;
}
export const SUN_SHADOW_FITS = Object.freeze({
  near: {
    ahead: 10,
    extent: 20,
    resolution: 4096,
    eyeDistance: 200,
    near: 1,
    far: 500,
  },
  far: {
    ahead: 35,
    extent: 90,
    resolution: 2048,
    eyeDistance: 200,
    near: 1,
    far: 500,
  },
  distant: {
    ahead: 300,
    extent: 900,
    resolution: 2048,
    eyeDistance: 1200,
    near: 1,
    far: 2500,
  },
} satisfies Record<string, DirectionalShadowFit>);

/** Fit an owned, texel-snapped light volume; distances use world meters. */
export function fitDirectionalShadow(
  eye: V3,
  forward: V3,
  sunDirection: V3,
  options: DirectionalShadowFit,
) {
  if (
    ![...eye, ...forward, ...sunDirection, ...Object.values(options)].every(
      Number.isFinite,
    )
  )
    throw new RangeError("Shadow volume inputs must be finite");
  if (
    options.ahead < 0 ||
    options.ahead > 100000 ||
    options.extent <= 0 ||
    options.extent > 100000 ||
    !Number.isInteger(options.resolution) ||
    options.resolution < 1 ||
    options.resolution > 16384 ||
    options.near <= 0 ||
    options.far <= options.near ||
    options.far > 1000000 ||
    options.eyeDistance <= options.near ||
    options.eyeDistance >= options.far ||
    Math.hypot(...sunDirection) < 1e-10
  )
    throw new RangeError("Shadow volume bounds are invalid");
  const sun = normalize(sunDirection);
  const horizontal: V3 =
    Math.hypot(forward[0], forward[2]) > 1e-10
      ? normalize([forward[0], 0, forward[2]])
      : [0, 0, -1];
  // A different up axis keeps zenith/nadir lights nonsingular.
  const up: V3 = Math.abs(sun[1]) > 0.999 ? [0, 0, 1] : [0, 1, 0];
  const right = normalize(cross(up, sun)),
    lightUp = cross(sun, right);
  const worldTexel = (options.extent * 2) / options.resolution;
  let center = add(eye, scale(horizontal, options.ahead));
  for (const axis of [right, lightUp]) {
    const p = dot(center, axis);
    center = add(
      center,
      scale(axis, Math.round(p / worldTexel) * worldTexel - p),
    );
  }
  const lightEye = add(center, scale(sun, options.eyeDistance));
  const matrix = multiply(
    orthographic(
      -options.extent,
      options.extent,
      -options.extent,
      options.extent,
      options.near,
      options.far,
    ),
    lookAt(lightEye, center, up),
  );
  return {
    matrix,
    center,
    lightEye,
    right,
    up: lightUp,
    sun,
    horizontal,
    worldTexel,
  };
}

/** Positive W and WebGPU's complete XY/Z clip volume determine coverage. */
export function shadowProjectionPoint(matrix: ArrayLike<number>, point: V3) {
  if (
    matrix.length !== 16 ||
    !Array.from(matrix).every(Number.isFinite) ||
    !point.every(Number.isFinite)
  )
    throw new RangeError("Shadow projection inputs must be finite");
  const clip = [0, 0, 0, 0];
  for (let row = 0; row < 4; row++)
    clip[row] =
      matrix[row] * point[0] +
      matrix[4 + row] * point[1] +
      matrix[8 + row] * point[2] +
      matrix[12 + row];
  const validW = clip[3] > 1e-6;
  const ndc = validW ? clip.slice(0, 3).map((x) => x / clip[3]) : [0, 0, 0];
  const inside =
    validW &&
    Math.abs(ndc[0]) <= 1 &&
    Math.abs(ndc[1]) <= 1 &&
    ndc[2] >= 0 &&
    ndc[2] <= 1;
  return {
    clip,
    ndc,
    inside,
    edge: Math.max(Math.abs(ndc[0]), Math.abs(ndc[1])),
  };
}

export const WGSL_SHADOW_PROJECTION = `
struct ShadowProjection {uv:vec2<f32>,depth:f32,edge:f32,inside:bool};
fn shadowProjection(projection:mat4x4<f32>,p:vec3<f32>)->ShadowProjection{
 let q=projection*vec4(p,1.0);
 if !(q.w>0.000001){return ShadowProjection(vec2(0.0),0.0,0.0,false);}
 let ndc=q.xyz/q.w;let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);
 let inside=all(abs(ndc.xy)<=vec2(1.0))&&ndc.z>=0.0&&ndc.z<=1.0;
 return ShadowProjection(uv,ndc.z,max(abs(ndc.x),abs(ndc.y)),inside);
}
`;
