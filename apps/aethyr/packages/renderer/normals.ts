type V2 = readonly [number, number];
type V3 = readonly [number, number, number];
import type { SourceNormalFrameProfile } from "../assets/src/scene.ts";
import { validateNormalFrameProfile } from "./source-normal-profile.ts";

/** Bits 0/1 select tangent Y and vector Z; bit 2 selects the recovered mesh frame. */
export function normalMapFlags(material: {
  normalConvention?: "opengl" | "directx";
  normalEncoding?: "rgb" | "xy-positive-z";
  normalFrameProfile?: SourceNormalFrameProfile;
}): number {
  if (
    material.normalConvention !== undefined &&
    material.normalConvention !== "opengl" &&
    material.normalConvention !== "directx"
  )
    throw new RangeError("Unknown normal map convention");
  if (
    material.normalEncoding !== undefined &&
    material.normalEncoding !== "rgb" &&
    material.normalEncoding !== "xy-positive-z"
  )
    throw new RangeError("Unknown normal map encoding");
  return (
    Number(material.normalConvention === "directx") |
    (Number(material.normalEncoding === "xy-positive-z") << 1) |
    (Number(validateNormalFrameProfile(material.normalFrameProfile)) << 2)
  );
}

/** Decode before graph blending; tangent-space Y conversion happens afterward. */
export function decodeNormalSample(encoded: V3, flags = 0): V3 {
  if (!Number.isInteger(flags) || flags < 0 || flags > 7)
    throw new RangeError(
      "Normal map flags must be an integer from zero to seven",
    );
  if (!encoded.every(Number.isFinite))
    throw new RangeError("Normal map samples must be finite");
  if (!(flags & 2)) return encoded.map((x) => x * 2 - 1) as unknown as V3;
  const x = Math.max(0, Math.min(1, encoded[0])) * 2 - 1,
    y = Math.max(0, Math.min(1, encoded[1])) * 2 - 1;
  return [x, y, Math.sqrt(Math.max(1 - x * x - y * y, 0))];
}

export interface NormalFrame {
  tangent: V3;
  bitangent: V3;
  normal: V3;
  valid: boolean;
}

/** Preserve the interpolated source basis, including its nonorthogonal lengths. */
export function sourceNormalFrame(
  normal: V3,
  tangent: V3,
  handedness: number,
): NormalFrame {
  if (
    ![...normal, ...tangent, handedness].every(Number.isFinite) ||
    Math.abs(handedness) < 0.5
  )
    throw new RangeError("Source normal frame is incomplete");
  return {
    normal,
    tangent,
    bitangent: cross(normal, tangent).map(
      (x) => x * Math.sign(handedness),
    ) as unknown as V3,
    valid: dot(normal, normal) > 1e-12 && dot(tangent, tangent) > 1e-12,
  };
}

/** Raw projected view coordinates preserve the recovered fuzz graph's linear dot. */
export function sourceNormalFrameView(frame: NormalFrame, unitView: V3): V3 {
  if (
    ![...unitView, ...frame.tangent, ...frame.bitangent, ...frame.normal].every(
      Number.isFinite,
    )
  )
    throw new RangeError("Source view coordinates must be finite");
  return [
    dot(unitView, frame.tangent),
    dot(unitView, frame.bitangent),
    dot(unitView, frame.normal),
  ];
}

/** Column-major affine axes, with each axis scale removed independently. */
export function sourceNormalVertex(
  normal: V3,
  tangent: V3,
  handedness: number,
  model: readonly number[],
): { normal: V3; tangent: V3; handedness: number } {
  if (
    model.length !== 16 ||
    ![...model, ...normal, ...tangent, handedness].every(Number.isFinite)
  )
    throw new RangeError("Source normal transform must be finite");
  const axes = [0, 4, 8].map((i) => model.slice(i, i + 3) as unknown as V3);
  const scale = axes.map((a) => Math.hypot(...a));
  if (scale.some((x) => x < 1e-8))
    throw new RangeError("Source normal transform has an empty axis");
  const r = axes.map((a, i) => a.map((x) => x / scale[i]) as unknown as V3);
  if (
    Math.abs(dot(r[0], r[1])) > 1e-5 ||
    Math.abs(dot(r[0], r[2])) > 1e-5 ||
    Math.abs(dot(r[1], r[2])) > 1e-5
  )
    throw new RangeError("Source normal transform cannot contain shear");
  const transform = (v: V3) =>
    [0, 1, 2].map(
      (i) => r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2],
    ) as unknown as V3;
  return {
    normal: transform(normal),
    tangent: transform(tangent),
    handedness: handedness * Math.sign(dot(r[0], cross(r[1], r[2]))),
  };
}
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const normalize = (a: V3, fallback: V3): V3 => {
  const length = Math.hypot(...a);
  return length > 1e-15
    ? (a.map((x) => x / length) as unknown as V3)
    : fallback;
};

/** Orthonormal UV-gradient frame, not a replacement for mesh-averaged tangents. */
export function normalFrame(
  sourceNormal: V3,
  positionDx: V3,
  positionDy: V3,
  uvDx: V2,
  uvDy: V2,
): NormalFrame {
  if (
    ![...sourceNormal, ...positionDx, ...positionDy, ...uvDx, ...uvDy].every(
      Number.isFinite,
    )
  )
    throw new RangeError("Normal frame inputs must be finite");
  const n = normalize(sourceNormal, [0, 1, 0]);
  const invalid: NormalFrame = {
    tangent: [0, 0, 0],
    bitangent: [0, 0, 0],
    normal: n,
    valid: false,
  };
  const det = uvDx[0] * uvDy[1] - uvDx[1] * uvDy[0];
  const uvScale = Math.hypot(...uvDx) * Math.hypot(...uvDy);
  if (uvScale < 1e-30 || Math.abs(det) <= uvScale * 1e-8) return invalid;
  const sign = det < 0 ? -1 : 1;
  const rawT = positionDx.map(
    (x, i) => (x * uvDy[1] - positionDy[i] * uvDx[1]) * sign,
  ) as unknown as V3;
  const rawB = positionDy.map(
    (x, i) => (x * uvDx[0] - positionDx[i] * uvDy[0]) * sign,
  ) as unknown as V3;
  const projected = rawT.map(
    (x, i) => x - n[i] * dot(n, rawT),
  ) as unknown as V3;
  if (dot(projected, projected) <= 1e-20 || dot(rawB, rawB) <= 1e-20)
    return invalid;
  const tangent = normalize(projected, [1, 0, 0]);
  const handedness = dot(cross(rawT, rawB), n) < 0 ? -1 : 1;
  const bitangent = cross(n, tangent).map(
    (x) => x * handedness,
  ) as unknown as V3;
  return { tangent, bitangent, normal: n, valid: true };
}

export function mapTangentNormal(
  frame: NormalFrame,
  tangentNormal: V3,
  directX = false,
  faceSign = 1,
  strength = 1,
): V3 {
  const unitNormal = normalize(frame.normal, [0, 1, 0]);
  const fallback = unitNormal.map((x) => x * faceSign) as unknown as V3;
  if (!frame.valid) return fallback;
  const [x, y, z] = tangentNormal;
  const result = frame.normal.map(
    (n, i) =>
      frame.tangent[i] * x * strength +
      frame.bitangent[i] * y * (directX ? -1 : 1) * strength +
      n * z,
  ) as unknown as V3;
  return normalize(result, unitNormal).map(
    (n) => n * faceSign,
  ) as unknown as V3;
}

/** Use source-facing N until the final two-sided world-normal sign is applied. */
export const WGSL_NORMAL_FRAME = `
fn normalMapDirectX(flags:f32)->bool{return (u32(flags)&1u)!=0u;}
fn normalMapSourceFrame(flags:f32)->bool{return (u32(flags)&4u)!=0u;}
fn decodeNormalSample(encoded:vec3<f32>,flags:f32)->vec3<f32>{
 if !all(abs(encoded)<=vec3(3.402823e38)){return vec3(0.0,0.0,1.0);}
 if (u32(flags)&2u)==0u{return encoded*2.0-1.0;}
 let xy=clamp(encoded.xy,vec2(0.0),vec2(1.0))*2.0-1.0;
 return vec3(xy,sqrt(max(1.0-dot(xy,xy),0.0)));
}
struct NormalFrame { tangent:vec3<f32>,bitangent:vec3<f32>,normal:vec3<f32>,valid:f32 };
fn sourceNormalFrame(n:vec3<f32>,t:vec3<f32>,handedness:f32)->NormalFrame{
 return NormalFrame(t,cross(n,t)*select(-1.0,1.0,handedness>=0.0),n,select(0.0,1.0,dot(n,n)>1e-12&&dot(t,t)>1e-12));
}
fn normalFrame(sourceNormal:vec3<f32>,positionDx:vec3<f32>,positionDy:vec3<f32>,uvDx:vec2<f32>,uvDy:vec2<f32>)->NormalFrame{
 let n=safeNormalize(sourceNormal,vec3(0.0,1.0,0.0));
 var result=NormalFrame(vec3(0.0),vec3(0.0),n,0.0);
 let determinant=uvDx.x*uvDy.y-uvDx.y*uvDy.x;
 let uvScale=length(uvDx)*length(uvDy);
 if uvScale<1e-30||abs(determinant)<=uvScale*1e-8{return result;}
 let orientation=select(-1.0,1.0,determinant>0.0);
 let rawT=(positionDx*uvDy.y-positionDy*uvDx.y)*orientation;
 let rawB=(positionDy*uvDx.x-positionDx*uvDy.x)*orientation;
 let projected=rawT-n*dot(n,rawT);
 if dot(projected,projected)<=1e-20||dot(rawB,rawB)<=1e-20{return result;}
 let tangent=safeNormalize(projected,vec3(1.0,0.0,0.0));
 let handedness=select(-1.0,1.0,dot(cross(rawT,rawB),n)>=0.0);
 result.tangent=tangent;result.bitangent=cross(n,tangent)*handedness;result.valid=1.0;return result;
}
fn normalFrameView(basis:NormalFrame,view:vec3<f32>,directX:bool)->vec3<f32>{
 return safeNormalize(vec3(dot(view,basis.tangent),dot(view,basis.bitangent)*select(1.0,-1.0,directX),dot(view,basis.normal)),vec3(0.0,0.0,1.0));
}
fn sourceNormalFrameView(basis:NormalFrame,view:vec3<f32>,directX:bool)->vec3<f32>{
 return vec3(dot(view,basis.tangent),dot(view,basis.bitangent)*select(1.0,-1.0,directX),dot(view,basis.normal));
}
fn mapTangentNormal(basis:NormalFrame,tangentNormal:vec3<f32>,directX:bool,faceSign:f32,strength:f32)->vec3<f32>{
 let fallback=safeNormalize(basis.normal,vec3(0.0,1.0,0.0));
 if basis.valid<0.5{return fallback*faceSign;}
 let n=basis.tangent*tangentNormal.x*strength+basis.bitangent*tangentNormal.y*select(1.0,-1.0,directX)*strength+basis.normal*tangentNormal.z;
 return safeNormalize(n,fallback)*faceSign;
}
`;
