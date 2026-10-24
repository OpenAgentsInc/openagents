import { readAsset, verifyAssetBytes } from "../assets/src/asset-download.ts";
import { WGSL_WATER_AIR_SCATTERING } from "./water-air-scattering.ts";
import { decodeRgba8 } from "../assets/src/rgba.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_SEGMENT_WGSL,
  heightFogComposition,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import type { SourceLight } from "../assets/src/scene.ts";
import { packWaterLocalLights } from "./water-local-lighting.ts";
import {
  validateWaterTurbulence,
  WGSL_WATER_TURBULENCE,
  type WaterTurbulence,
} from "./water-turbulence.ts";
import type { WaterEnvironment } from "./water-pass.ts";

export const SOURCE_WATER_STREAM_FRAME_FLOATS = 688;
export interface OwnedClearWaterMaterial {
  ior: number;
  roughness: number;
  normalStrength: number;
  /** Owned UV detail frequency; omission preserves the original 1x owned profile. */
  normalTiling?: number;
  thicknessMeters: number;
  absorption: [number, number, number];
}
export interface OwnedEmissiveWaterMaterial {
  normalStrength: number;
  normalTiling: number;
  geometryAmplitudeMeters: number;
  microAmplitudeMeters: number;
}

export interface WaterStreamBlob {
  url: string;
  sha256: string;
  byteLength: number;
}
export interface SourceWaterStreamMesh {
  id: string;
  firstIndex: number;
  indexCount: number;
  center: [number, number, number];
}
export interface SourceWaterStreamInstance {
  id: string;
  meshId: string;
  model: number[];
  /** Recovered Refraction, Opacity_1, Opacity_2, Param4. */
  parameters: [number, number, number, number];
}
export interface SourceWaterStreamsManifest {
  schema: "aethyrforge.source-water-streams.v1";
  vertices: WaterStreamBlob;
  indices: WaterStreamBlob;
  vertexCount: number;
  /** Older prepared recipes omit this field and use position/normal/UV only. */
  vertexStride?: 32 | 48;
  indexCount: number;
  meshes: SourceWaterStreamMesh[];
  noise: WaterStreamBlob & {
    width: number;
    height: number;
    colorSpace: "linear";
  };
  normal?: WaterStreamBlob & {
    width: number;
    height: number;
    colorSpace: "linear";
    encoding: "unreal-normal-red-splat" | "unreal-normal-rgb";
    /** Source asset setting; red extraction is unaffected by green inversion. */
    flipGreenChannel: boolean;
    /** Owned spatial area filter; not a source temporal-AA claim. */
    coverage?: "bilinear-positive-area-8x8-v1";
  };
  material: {
    flow: [number, number];
    powerExponent: number;
    fresnelExponent: number;
    baseReflectFraction: number;
    depthFadeMeters: number;
    /** Source graph IOR minimum. Per-instance parameter 0 supplies its maximum. */
    refractionMinimum: number;
    /** Omission retains the original owned Snell tracer. */
    refractionMode?: "source-screen-displacement-v1";
    /** Omission keeps the recovered material response. */
    appearanceProfile?:
      | "owned-clear-stream-v1"
      | "owned-turbulent-stream-v1"
      | "owned-source-emissive-stream-v1";
    turbulence?: WaterTurbulence;
    clearWater?: OwnedClearWaterMaterial;
    emissiveWater?: OwnedEmissiveWaterMaterial;
  };
  scenarios: Record<"day" | "night", SourceWaterStreamInstance[]>;
  metadata?: Record<string, unknown>;
}
export interface SourceWaterStreamsFrame {
  mode: "day" | "night";
  time: number;
  width: number;
  height: number;
  /** Existing rgba16float, 4xMSAA attachment, loaded after the basin pass. */
  colorView: GPUTextureView;
  /** Optional separate single-sample resolve destination. */
  resolveTarget?: GPUTextureView;
  /** Frozen basin color and a distinct resolve destination; never read/write one texture. */
  refraction?: { source: GPUTexture; resolve: GPUTexture };
  /** Existing read-only depth24plus, 4xMSAA and TEXTURE_BINDING usage. */
  depthView: GPUTextureView;
  viewProjection: ArrayLike<number>;
  inverseViewProjection: ArrayLike<number>;
  eye: readonly number[];
  /** Camera exposure, excluding the common scene-linear storage scale. */
  effectiveExposure: number;
  lighting?: {
    sunDirectionTowardLight: readonly number[];
    /** Incident linear RGB before the common scene storage scale. */
    sunRadianceRGB: readonly number[];
    localLights?: readonly SourceLight[];
    diffuseSH?: ArrayLike<number>;
    diffuseRotation?: ArrayLike<number>;
  };
  /** Uses the basin pass's separate, two-layer HDR environment and gain convention. */
  environment?: WaterEnvironment;
  /** Color already includes the common scene-linear storage scale. */
  fog?: {
    density: number;
    color: readonly number[];
    heightRate?: number;
    volumeMedium?: FogVolumeMedium;
    volumeRange?: FogVolumeRange;
  };
}
export const SOURCE_WATER_STREAM_LIMITS = Object.freeze({
  meshes: 2,
  instances: 64,
  vertices: 131072,
  indices: 786432,
  geometryBytes: 16 * 1024 * 1024,
  textureBytes: 32 * 1024 * 1024,
  textureDimension: 2048,
  refractionPixels: 3840 * 2160,
  refractionDepthBytes: 128 * 1024 * 1024,
});
export const SOURCE_WATER_STREAM_DIAGNOSTICS = Object.freeze([
  "Refraction uses one nearest source-stream interface and the frozen basin color. Offscreen rays and missing depth use the original pixel; overlapping layers are not reconstructed.",
  "Refraction above the private-depth memory limit falls back to ordinary source transparency. Source default-lit specular is not rendered.",
  "Only persistent source mesh emitters are rendered here; source splash and ripple emitters are separate effects.",
]);
export const OWNED_CLEAR_STREAM_DIAGNOSTICS = Object.freeze([
  "Owned clear-stream material: continuous RGB normals, Fresnel transmission/reflection and directional GGX highlights; not source material parity.",
  "One nearest stream interface samples frozen basin color. Screen rays use bounded opaque depth; missing reflections fall back to the lighting environment. Overlapping and offscreen transparent layers are not reconstructed.",
  "Optical thickness is an owned thin-sheet parameter. Local-light specular and multiple internal reflections are not included. A terrestrial-size light-disc bound and the half-float working range cap extreme highlights.",
]);
export const OWNED_TURBULENT_STREAM_DIAGNOSTICS = Object.freeze([
  "Owned flowing-water preview: bounded surface waves, air scattering, dielectric reflections and up to 32 nearby point/spot highlights. It is not source material parity.",
  "Stream color and nearest-depth use identical geometry displacement. Microstructure affects shading only; the geometry bound expands by the maximum 4-millimeter displacement in the prepared profile.",
  "Local-light and sunlight shadows, internal reflection sums, overlapping transparent interfaces, temporal history and detached spray-sheet breakup are not reconstructed.",
]);
export const OWNED_EMISSIVE_STREAM_DIAGNOSTICS = Object.freeze([
  "Owned flowing source-energy material: source noise, exposure compensation, opacity and screen displacement with continuous RGB normals. It does not reproduce the source scalar-red normal or temporal history.",
  "One nearest stream interface samples frozen basin color. Geometry deformation is identical in depth and color passes; overlapping transparent layers and offscreen background remain unavailable.",
]);
/** Controlled UE5.8 plane measurements; full-viewport mono perspective only. */
export const SOURCE_SCREEN_DISPLACEMENT_FACTOR = 0.00023;
export const SOURCE_ZERO_BIAS_DEPTH_INTERVAL_METERS = 0.01;
function finite(value: number, lo: number, hi: number) {
  if (!Number.isFinite(value) || value < lo || value > hi)
    throw new Error("The water stream contains an unsupported number.");
}
function vector(
  value: ArrayLike<number>,
  length: number,
  lo: number,
  hi: number,
) {
  if (!value || value.length !== length)
    throw new Error("The water stream vector has the wrong dimensions.");
  for (let i = 0; i < length; i++) finite(value[i], lo, hi);
}
function integer(value: number, lo: number, hi: number) {
  finite(value, lo, hi);
  if (!Number.isInteger(value))
    throw new Error("The water stream count must be a whole number.");
}
function blob(value: WaterStreamBlob, maximum: number) {
  if (
    !value ||
    !/^\/private-assets\/[A-Za-z0-9_-]+\.bin$/.test(value.url) ||
    !/^[a-f0-9]{64}$/.test(value.sha256)
  )
    throw new Error("Use a prepared private water stream resource.");
  integer(value.byteLength, 1, maximum);
}
export function validateSourceWaterStreams(value: SourceWaterStreamsManifest) {
  if (!value || value.schema !== "aethyrforge.source-water-streams.v1")
    throw new Error("Prepare a supported source water stream recipe.");
  integer(value.vertexCount, 3, SOURCE_WATER_STREAM_LIMITS.vertices);
  integer(value.indexCount, 3, SOURCE_WATER_STREAM_LIMITS.indices);
  if (value.indexCount % 3)
    throw new Error("Water stream indices must describe complete triangles.");
  blob(value.vertices, SOURCE_WATER_STREAM_LIMITS.geometryBytes);
  blob(value.indices, SOURCE_WATER_STREAM_LIMITS.geometryBytes);
  if (
    ![32, 48].includes(value.vertexStride ?? 32) ||
    value.vertices.byteLength !==
      value.vertexCount * (value.vertexStride ?? 32) ||
    value.indices.byteLength !== value.indexCount * 4 ||
    value.vertices.byteLength + value.indices.byteLength >
      SOURCE_WATER_STREAM_LIMITS.geometryBytes
  )
    throw new Error(
      "The prepared water stream geometry has the wrong byte count.",
    );
  if (
    !Array.isArray(value.meshes) ||
    !value.meshes.length ||
    value.meshes.length > 2
  )
    throw new Error("Use one or two source water stream meshes.");
  const ids = new Set<string>();
  for (const mesh of value.meshes) {
    if (
      !mesh ||
      typeof mesh.id !== "string" ||
      !mesh.id ||
      mesh.id.length > 128 ||
      ids.has(mesh.id)
    )
      throw new Error("Water stream meshes need unique names.");
    ids.add(mesh.id);
    integer(mesh.firstIndex, 0, value.indexCount - 3);
    integer(mesh.indexCount, 3, value.indexCount);
    if (
      mesh.indexCount % 3 ||
      mesh.firstIndex % 3 ||
      mesh.firstIndex + mesh.indexCount > value.indexCount
    )
      throw new Error("The water stream mesh index range is invalid.");
    vector(mesh.center, 3, -1000, 1000);
  }
  blob(value.noise, SOURCE_WATER_STREAM_LIMITS.textureBytes);
  integer(value.noise.width, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
  integer(value.noise.height, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
  if (
    value.noise.byteLength !==
      16 + value.noise.width * value.noise.height * 4 ||
    value.noise.colorSpace !== "linear"
  )
    throw new Error("Use the prepared linear water stream noise map.");
  if (value.normal) {
    const normal = value.normal;
    blob(normal, SOURCE_WATER_STREAM_LIMITS.textureBytes);
    integer(normal.width, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
    integer(normal.height, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
    if (
      normal.byteLength !== 16 + normal.width * normal.height * 4 ||
      normal.colorSpace !== "linear" ||
      !["unreal-normal-red-splat", "unreal-normal-rgb"].includes(
        normal.encoding,
      ) ||
      typeof normal.flipGreenChannel !== "boolean" ||
      (normal.coverage !== undefined &&
        normal.coverage !== "bilinear-positive-area-8x8-v1") ||
      value.vertexStride !== 48
    )
      throw new Error(
        "Use the prepared linear source normal map and explicit tangents.",
      );
  }
  const material = value.material;
  if (!material)
    throw new Error("The source water stream material is missing.");
  vector(material.flow, 2, -100, 100);
  finite(material.powerExponent, 0.01, 16);
  finite(material.fresnelExponent, 0.01, 16);
  finite(material.baseReflectFraction, 0, 1);
  finite(material.depthFadeMeters, 0.001, 10);
  finite(material.refractionMinimum, 1, 4);
  if (
    material.refractionMode !== undefined &&
    material.refractionMode !== "source-screen-displacement-v1"
  )
    throw new Error("Choose a supported water stream refraction profile.");
  if (
    material.appearanceProfile === "owned-clear-stream-v1" ||
    material.appearanceProfile === "owned-turbulent-stream-v1"
  ) {
    const clear = material.clearWater;
    if (
      !clear ||
      material.emissiveWater !== undefined ||
      material.refractionMode !== undefined ||
      (value.normal &&
        (value.normal.encoding !== "unreal-normal-rgb" ||
          value.normal.coverage !== undefined))
    )
      throw new Error(
        "Prepare the owned clear stream material and continuous normal map together.",
      );
    finite(clear.ior, 1, 2);
    finite(clear.roughness, 0.04, 1);
    finite(clear.normalStrength, 0, 1);
    finite(clear.normalTiling ?? 1, 0.25, 64);
    finite(clear.thicknessMeters, 0.001, 0.1);
    vector(clear.absorption, 3, 0, 10);
    if (material.appearanceProfile === "owned-turbulent-stream-v1") {
      if (!material.turbulence)
        throw new Error("Prepare the owned turbulent surface settings.");
      validateWaterTurbulence(material.turbulence);
    } else if (material.turbulence !== undefined)
      throw new Error("Turbulence requires its explicit material profile.");
  } else if (material.appearanceProfile === "owned-source-emissive-stream-v1") {
    const emissive = material.emissiveWater;
    if (
      !emissive ||
      Object.keys(emissive).some(
        (key) =>
          ![
            "normalStrength",
            "normalTiling",
            "geometryAmplitudeMeters",
            "microAmplitudeMeters",
          ].includes(key),
      ) ||
      material.clearWater !== undefined ||
      material.turbulence !== undefined ||
      material.refractionMode !== "source-screen-displacement-v1" ||
      !value.normal ||
      value.normal.encoding !== "unreal-normal-rgb" ||
      value.normal.coverage !== undefined
    )
      throw new Error(
        "Prepare the source-energy hybrid and continuous normals together.",
      );
    finite(emissive.normalStrength, 0, 1);
    finite(emissive.normalTiling, 0.25, 64);
    finite(emissive.geometryAmplitudeMeters, 0, 0.015);
    finite(emissive.microAmplitudeMeters, 0, 0.005);
  } else if (
    material.appearanceProfile !== undefined ||
    material.clearWater !== undefined ||
    material.turbulence !== undefined ||
    material.emissiveWater !== undefined ||
    (value.normal && value.normal.encoding !== "unreal-normal-red-splat")
  ) {
    throw new Error(
      "Choose a supported source or owned clear stream appearance.",
    );
  }
  for (const mode of ["day", "night"] as const) {
    const instances = value.scenarios?.[mode];
    if (
      !Array.isArray(instances) ||
      instances.length > SOURCE_WATER_STREAM_LIMITS.instances
    )
      throw new Error("Use at most 64 source water streams per scenario.");
    const instanceIds = new Set<string>();
    for (const instance of instances) {
      if (
        !instance ||
        typeof instance.id !== "string" ||
        !instance.id ||
        instance.id.length > 1024 ||
        instanceIds.has(instance.id) ||
        !ids.has(instance.meshId)
      )
        throw new Error(
          "Water stream placements need unique names and known meshes.",
        );
      instanceIds.add(instance.id);
      vector(instance.model, 16, -100000, 100000);
      const m = instance.model;
      if (
        Math.abs(m[3]) + Math.abs(m[7]) + Math.abs(m[11]) > 1e-6 ||
        Math.abs(m[15] - 1) > 1e-6
      )
        throw new Error("Use an affine water stream transform.");
      const determinant =
        m[0] * (m[5] * m[10] - m[6] * m[9]) -
        m[4] * (m[1] * m[10] - m[2] * m[9]) +
        m[8] * (m[1] * m[6] - m[2] * m[5]);
      if (
        Math.abs(determinant) < 1e-8 ||
        [0, 4, 8].some((i) => Math.hypot(m[i], m[i + 1], m[i + 2]) > 100)
      )
        throw new Error("Use a nonsingular bounded water stream transform.");
      vector(instance.parameters, 4, 0, 4);
      finite(instance.parameters[0], material.refractionMinimum, 4);
      finite(instance.parameters[1], 0, 1);
      finite(instance.parameters[2], 0, 1);
    }
  }
  return value;
}

export const WGSL_SOURCE_WATER_STREAMS =
  FOG_SEGMENT_WGSL +
  WGSL_WATER_TURBULENCE +
  WGSL_WATER_AIR_SCATTERING +
  `
struct Frame{view:mat4x4<f32>,inverse:mat4x4<f32>,eyeTime:vec4<f32>,settings:vec4<f32>,forward:vec4<f32>,material:vec4<f32>,optics:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,features:vec4<f32>,environmentRotation:mat4x4<f32>,environmentSettings:vec4<f32>,environmentGain:vec4<f32>,sunDirection:vec4<f32>,sunRadiance:vec4<f32>,clear:vec4<f32>,absorption:vec4<f32>,turbulence:vec4<f32>,surfaceSettings:vec4<f32>,diffuseRotation:mat4x4<f32>,diffuseSH:array<vec4<f32>,9>,localPositions:array<vec4<f32>,32>,localColors:array<vec4<f32>,32>,localDirections:array<vec4<f32>,32>,localCones:array<vec4<f32>,32>,airSettings:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var opaqueDepth:texture_depth_multisampled_2d;
@group(0)@binding(2)var noiseMap:texture_2d<f32>;
@group(0)@binding(3)var noiseSampler:sampler;
@group(0)@binding(4)var normalMap:texture_2d<f32>;
@group(0)@binding(5)var sceneColor:texture_2d<f32>;
@group(0)@binding(6)var sceneSampler:sampler;
@group(0)@binding(7)var environmentMap:texture_2d_array<f32>;
@group(0)@binding(8)var environmentSampler:sampler;
struct Vertex{@location(0)p:vec3<f32>,@location(1)n:vec3<f32>,@location(2)uv:vec2<f32>,@location(8)tangent:vec4<f32>,@location(3)m0:vec4<f32>,@location(4)m1:vec4<f32>,@location(5)m2:vec4<f32>,@location(6)m3:vec4<f32>,@location(7)parameters:vec4<f32>};
struct Output{@builtin(position)clip:vec4<f32>,@location(0)world:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)@interpolate(flat)parameters:vec4<f32>,@location(4)tangent:vec4<f32>,@location(5)@interpolate(flat)detSign:f32,@location(6)undeformed:vec3<f32>};
fn waterUnit(value:vec3<f32>,fallback:vec3<f32>)->vec3<f32>{let lengthSquared=dot(value,value);if lengthSquared>1e-16{return value*inverseSqrt(lengthSquared);}return fallback;}
fn waterOrthogonal(n:vec3<f32>)->vec3<f32>{let axis=select(vec3<f32>(0,1,0),vec3<f32>(1,0,0),abs(n.y)>.9);return waterUnit(cross(axis,n),vec3(1,0,0));}
fn waterFresnel(cosine:f32)->f32{return frame.optics.x+(1.0-frame.optics.x)*pow(1.0-clamp(cosine,0.0,1.0),frame.material.w);}
@vertex fn vs(v:Vertex)->Output{
 let model=mat4x4(v.m0,v.m1,v.m2,v.m3);var world=(model*vec4(v.p,1)).xyz;let undeformed=world;
 let c0=cross(v.m1.xyz,v.m2.xyz);let c1=cross(v.m2.xyz,v.m0.xyz);let c2=cross(v.m0.xyz,v.m1.xyz);
 let detSign=sign(dot(v.m0.xyz,c0));let n=waterUnit((c0*v.n.x+c1*v.n.y+c2*v.n.z)*detSign,vec3(0,1,0));
 let transformedT=(model*vec4(v.tangent.xyz,0)).xyz;let t=waterUnit(transformedT-n*dot(n,transformedT),waterOrthogonal(n));
 if frame.features.w>1.5{world+=n*waterGeometryField(world,frame.eyeTime.w,frame.turbulence.x).x;}
 var o:Output;o.undeformed=undeformed;o.world=world;o.clip=frame.view*vec4(world,1);o.normal=n;o.uv=v.uv;o.parameters=v.parameters;o.tangent=vec4(t,v.tangent.w*detSign);o.detSign=detSign;return o;
}
fn depthAt(pixel:vec2<i32>)->f32{
 let p=clamp(pixel,vec2(0),vec2<i32>(textureDimensions(opaqueDepth))-vec2(1));
 var depth=1.0;for(var i=0;i<4;i++){depth=min(depth,textureLoad(opaqueDepth,p,i));}return depth;
}
fn worldAt(uv:vec2<f32>,depth:f32)->vec3<f32>{let q=frame.inverse*vec4(uv*vec2(2,-2)+vec2(-1,1),depth,1);return q.xyz/q.w;}
// Owned reimplementation of a measured compatibility profile, not engine shader code.
// Missing depth, unsupported projections and screen/foreground crossings are owned guards.
fn sourceScreenUV(world:vec3<f32>,normal:vec3<f32>,ior:f32,original:vec2<f32>,originalDepth:f32)->vec2<f32>{
 if ior<=1.000001 || originalDepth>=.999999{return original;}
 let rightRow=vec3(frame.view[0].x,frame.view[1].x,frame.view[2].x);
 let upRow=vec3(frame.view[0].y,frame.view[1].y,frame.view[2].y);
 let wRow=vec3(frame.view[0].w,frame.view[1].w,frame.view[2].w);
 let horizontalScale=length(rightRow);let verticalScale=length(upRow);
 if horizontalScale<.000001 || verticalScale<.000001 || abs(length(wRow)-1.0)>.001{return original;}
 let right=rightRow/horizontalScale;let up=upRow/verticalScale;
 if abs(dot(right,up))>.001 || abs(dot(right,frame.forward.xyz))>.001 || abs(dot(up,frame.forward.xyz))>.001{return original;}
 let delta=vec2(dot(normal,right),-dot(normal,up))*(ior-1.0)*frame.settings.x*horizontalScale*${SOURCE_SCREEN_DISPLACEMENT_FACTOR};
 let candidate=original+delta;
 if any(candidate<vec2(0)) || any(candidate>vec2(1)){return original;}
 let depth=depthAt(vec2<i32>(candidate*frame.settings.xy));if depth>=.999999{return original;}
 let separation=dot(worldAt(candidate,depth)-world,frame.forward.xyz);
 if separation<0.0{return original;}
 let attenuated=original+delta*clamp(separation/${SOURCE_ZERO_BIAS_DEPTH_INTERVAL_METERS},0.0,1.0);
 let finalDepth=depthAt(vec2<i32>(attenuated*frame.settings.xy));
 if finalDepth>=.999999 || dot(worldAt(attenuated,finalDepth)-world,frame.forward.xyz)<0.0{return original;}
 return attenuated;
}
// Owned bounded screen-space Snell tracing, not the Unreal distortion implementation.
fn refractedUV(world:vec3<f32>,normal:vec3<f32>,ior:f32,original:vec2<f32>,originalDepth:f32)->vec2<f32>{
 if ior<=1.000001 || originalDepth>=.999999{return original;}
 let incident=waterUnit(world-frame.eyeTime.xyz,frame.forward.xyz);let faceNormal=normal*select(1.0,-1.0,dot(normal,incident)>0.0);
 let direction=refract(incident,faceNormal,1.0/ior);if dot(direction,direction)<.000001{return original;}
 let maxDistance=min(64.0,max(.01,length(worldAt(original,originalDepth)-world)*2.0));
 var previous=0.0;var previousResidual=-1.0;
 for(var step=1;step<=24;step++){
  let fraction=f32(step)/24.0;let distance=maxDistance*fraction*fraction;let point=world+direction*distance;
  let clip=frame.view*vec4(point,1);if clip.w<=0.0{return original;}
  let uv=clip.xy/clip.w*vec2(.5,-.5)+vec2(.5);if any(uv<vec2(0))||any(uv>vec2(1)){return original;}
  let depth=depthAt(vec2<i32>(uv*frame.settings.xy));
  if depth>=.999999{previous=distance;previousResidual=-1.0;continue;}
  let opaque=worldAt(uv,depth);if dot(opaque-world,frame.forward.xyz)<-.0001{return original;}
  let residual=dot(point-opaque,frame.forward.xyz);
  if residual>=0.0 && previousResidual<0.0{
   var lo=previous;var hi=distance;var hitUV=uv;
   for(var refine=0;refine<6;refine++){
    let midpoint=(lo+hi)*.5;let q=frame.view*vec4(world+direction*midpoint,1);let candidate=q.xy/q.w*vec2(.5,-.5)+vec2(.5);
    let d=depthAt(vec2<i32>(candidate*frame.settings.xy));let background=worldAt(candidate,d);
    if d<.999999 && dot(background-world,frame.forward.xyz)>=-.0001 && dot(world+direction*midpoint-background,frame.forward.xyz)>=0.0{hi=midpoint;hitUV=candidate;}else{lo=midpoint;}
   }return clamp(hitUV,vec2(0),vec2(1));
  }previous=distance;previousResidual=residual;
 }return original;
}
// Exact unpolarized Fresnel for the single exterior air-to-water interface.
// IOR 1 is the identity interface even at grazing angles; internal exits/TIR are not modeled.
// Owned parallel thin-sheet ray geometry: the exterior exit ray is parallel to
// the incident ray, shifted by the physical distance between the two interfaces.
// We retain one-interface Fresnel weights; internal reflection sums are omitted.
fn clearSheetUV(world:vec3<f32>,normal:vec3<f32>,original:vec2<f32>,originalDepth:f32)->vec2<f32>{
 if frame.clear.x<=1.000001||originalDepth>=.999999{return original;}
 let incident=waterUnit(world-frame.eyeTime.xyz,frame.forward.xyz);
 let ci=-dot(incident,normal);let viewCosine=dot(incident,frame.forward.xyz);
 if ci<.02||viewCosine<.02{return original;}
 let inside=refract(incident,normal,1.0/frame.clear.x);let ct=-dot(inside,normal);if ct<.02{return original;}
 let shift=inside*(frame.clear.w/ct)-incident*(frame.clear.w/ci);
 let background=worldAt(original,originalDepth);
 let displacedPoint=background+shift-incident*(dot(shift,frame.forward.xyz)/viewCosine);
 let clip=frame.view*vec4(displacedPoint,1);if clip.w<=0.0{return original;}
 let uv=clip.xy/clip.w*vec2(.5,-.5)+vec2(.5);if any(uv<vec2(0))||any(uv>vec2(1)){return original;}
 let depth=depthAt(vec2<i32>(uv*frame.settings.xy));
 if depth>=.999999||dot(worldAt(uv,depth)-world,frame.forward.xyz)<0.0{return original;}
 return uv;
}
fn clearFresnel(cosine:f32)->f32{
 let ior=frame.clear.x;if ior<=1.000001{return 0.0;}
 let ci=clamp(abs(cosine),0.0,1.0);let ct=sqrt(max(0.0,1.0-(1.0-ci*ci)/(ior*ior)));
 let rs=(ci-ior*ct)/max(ci+ior*ct,.000001);let rp=(ior*ci-ct)/max(ior*ci+ct,.000001);
 return clamp((rs*rs+rp*rp)*.5,0.0,1.0);
}
fn clearEnvironment(direction:vec3<f32>,roughness:f32)->vec3<f32>{
 if frame.environmentSettings.x<.5{return vec3(0);}
 let d=waterUnit((frame.environmentRotation*vec4(direction,0)).xyz,vec3(0,1,0));
 let uv=vec2(fract(atan2(d.x,d.z)/6.283185307),acos(clamp(d.y,-1.0,1.0))/3.141592654);let mip=roughness*frame.environmentSettings.y;
 let solar=textureSampleLevel(environmentMap,environmentSampler,uv,0,mip).rgb*frame.environmentSettings.z;
 let constant=textureSampleLevel(environmentMap,environmentSampler,uv,1,mip).rgb*frame.environmentSettings.w;
 return (solar+constant)*frame.environmentGain.rgb;
}
fn clearScreenReflection(world:vec3<f32>,normal:vec3<f32>,direction:vec3<f32>)->vec4<f32>{
 var previous=.01;var previousGap=-1.0;
 for(var step=1;step<=16;step++){
  let distance=.01+16.0*pow(f32(step)/16.0,2.0);let point=world+normal*.005+direction*distance;let clip=frame.view*vec4(point,1);
  if clip.w<=0.0{return vec4(0);}
  let uv=clip.xy/clip.w*vec2(.5,-.5)+vec2(.5);if any(uv<vec2(0))||any(uv>vec2(1)){return vec4(0);}
  let depth=depthAt(vec2<i32>(uv*frame.settings.xy));if depth>=.999999{previous=distance;previousGap=-1.0;continue;}
  let opaque=worldAt(uv,depth);let gap=dot(point-opaque,frame.forward.xyz);
  if gap>=0.0&&previousGap<0.0{
   var lo=previous;var hi=distance;var candidate=uv;
   for(var refine=0;refine<4;refine++){
    let midpoint=(lo+hi)*.5;let p=world+normal*.005+direction*midpoint;let q=frame.view*vec4(p,1);
    candidate=q.xy/q.w*vec2(.5,-.5)+vec2(.5);if any(candidate<vec2(0))||any(candidate>vec2(1)){return vec4(0);}
    let z=depthAt(vec2<i32>(candidate*frame.settings.xy));let hit=worldAt(candidate,z);
    if z<.999999&&dot(p-hit,frame.forward.xyz)>=0.0{hi=midpoint;}else{lo=midpoint;}
   }
   let z=depthAt(vec2<i32>(candidate*frame.settings.xy));let hit=worldAt(candidate,z);let point=world+normal*.005+direction*((lo+hi)*.5);
   if z<.999999&&length(point-hit)<.15&&dot(hit-world,normal)>.005{
    let edge=min(min(candidate.x,1.0-candidate.x),min(candidate.y,1.0-candidate.y));
    return vec4(textureSampleLevel(sceneColor,sceneSampler,candidate,0).rgb,smoothstep(.005,.03,edge)*(1.0-smoothstep(12.0,16.0,distance)));
   }return vec4(0);
  }previous=distance;previousGap=gap;
 }return vec4(0);
}
fn waterSpecular(normal:vec3<f32>,view:vec3<f32>,roughness:f32,light:vec3<f32>)->f32{
 let nl=max(dot(normal,light),0.0);let nv=max(dot(normal,view),.0001);if nl<=0.0{return 0.0;}
 let halfVector=waterUnit(view+light,normal);let nh=max(dot(normal,halfVector),0.0);let vh=max(dot(view,halfVector),0.0);
 let alpha=roughness*roughness;let alpha2=alpha*alpha;let denominator=nh*nh*(alpha2-1.0)+1.0;
 let distribution=alpha2/max(3.141592654*denominator*denominator,1e-12);
 let gv=2.0*nv/(nv+sqrt(alpha2+(1.0-alpha2)*nv*nv));let gl=2.0*nl/(nl+sqrt(alpha2+(1.0-alpha2)*nl*nl));
 let specular=distribution*gv*gl*clearFresnel(vh)/(4.0*nv);
 // A finite terrestrial-size directional disc bounds the unresolved mirror peak.
 let discSolidAngle=2.0*3.141592654*(1.0-cos(.00465));
 return min(specular,1.0/discSolidAngle);
}
fn clearSun(normal:vec3<f32>,view:vec3<f32>,roughness:f32)->vec3<f32>{return frame.sunRadiance.rgb*waterSpecular(normal,view,roughness,waterUnit(frame.sunDirection.xyz,vec3(0,1,0)));}
struct WaterLocalResult{specular:vec3<f32>,diffuse:vec3<f32>};
fn waterLocals(world:vec3<f32>,normal:vec3<f32>,view:vec3<f32>,roughness:f32)->WaterLocalResult{
 var result:WaterLocalResult;result.specular=vec3(0);result.diffuse=vec3(0);
 for(var i=0u;i<u32(frame.surfaceSettings.x);i++){
  let lp=frame.localPositions[i];let delta=lp.xyz-world;let dist2=dot(delta,delta);let dist=sqrt(dist2);let light=delta/max(dist,.001);
  var attenuation=pow(max(1.0-pow(dist/lp.w,4.0),0.0),2.0)/(dist2+.0001);
  let cone=frame.localCones[i];if cone.w>=0.0{attenuation=pow(max(1.0-dist2/(lp.w*lp.w),0.0),max(cone.w,.0001));}
  if frame.localDirections[i].w>.5{let angle=dot(-light,frame.localDirections[i].xyz);var factor=step(cone.y,angle);if cone.x-cone.y>.00001{let ramp=clamp((angle-cone.y)/(cone.x-cone.y),0.0,1.0);factor=ramp*ramp;}attenuation*=factor;}
  let incident=frame.localColors[i].rgb*attenuation;result.specular+=incident*waterSpecular(normal,view,roughness,light);var phase=max(dot(normal,light),0.0)/3.141592654;if frame.airSettings.z>.5{phase=waterAirPhase(dot(light,view),frame.airSettings.y);}result.diffuse+=incident*phase;
 }return result;
}
fn waterDiffuse(normal:vec3<f32>)->vec3<f32>{
 let n=waterUnit((frame.diffuseRotation*vec4(normal,0)).xyz,vec3(0,1,0));
 let b=array<f32,9>(.2820947918,.4886025119*n.y,.4886025119*n.z,.4886025119*n.x,1.0925484306*n.x*n.y,1.0925484306*n.y*n.z,.3153915653*(3*n.z*n.z-1),1.0925484306*n.x*n.z,.5462742153*(n.x*n.x-n.y*n.y));
 var sum=vec3<f32>(0);for(var i=0;i<9;i++){sum+=frame.diffuseSH[i].rgb*b[i];}return max(sum,vec3(0));
}
fn waterAirEnvironment(view:vec3<f32>)->vec3<f32>{
 let n=waterUnit((frame.diffuseRotation*vec4(-view,0)).xyz,vec3(0,1,0));
 let b=array<f32,9>(.2820947918,.4886025119*n.y,.4886025119*n.z,.4886025119*n.x,1.0925484306*n.x*n.y,1.0925484306*n.y*n.z,.3153915653*(3*n.z*n.z-1),1.0925484306*n.x*n.z,.5462742153*(n.x*n.x-n.y*n.y));
 var sum=vec3<f32>(0);for(var i=0u;i<9u;i++){sum+=frame.diffuseSH[i].rgb*b[i]*waterAirSHFactor(i,frame.airSettings.y);}return max(sum,vec3(0));
}
@fragment fn fs(v:Output,@builtin(front_facing)front:bool)->@location(0)vec4<f32>{
 var uvOffset=frame.material.xy*frame.eyeTime.w;if frame.features.w>2.5{uvOffset=frame.environmentSettings.xy;}
 let uv=v.uv+uvOffset;
 let normalUV=select(uv,v.uv*frame.sunDirection.w+uvOffset,frame.features.w>.5);
 let normalTexel=textureSample(normalMap,noiseSampler,normalUV);let sampled=normalTexel.r*2.0-1.0;
 let physicalFront=front==(v.detSign>=0.0);let faceSign=select(-1.0,1.0,physicalFront);
 // Owned degeneracy policy: use the source-facing triangle normal when interpolation cancels N.
 let triangleNormal=waterUnit(cross(dpdy(v.world),dpdx(v.world)),vec3(0,1,0))*faceSign;
 var sourceNormal=waterUnit(v.normal,triangleNormal);
 if frame.features.w>1.5{
  let exactGeometry=waterUnit(cross(dpdy(v.world),dpdx(v.world)),sourceNormal);
  sourceNormal=exactGeometry*select(1.0,-1.0,dot(exactGeometry,sourceNormal)<0.0);
  let gradient=waterMicroField(v.undeformed,frame.eyeTime.w,frame.turbulence.y).yzw;
  sourceNormal=waterUnit(sourceNormal-gradient+sourceNormal*dot(gradient,sourceNormal),sourceNormal);
 }
 if frame.features.w>.5&&frame.features.x>.5{
  let t=waterUnit(v.tangent.xyz-sourceNormal*dot(sourceNormal,v.tangent.xyz),waterOrthogonal(sourceNormal));let b=cross(sourceNormal,t)*v.tangent.w;
  var local=normalTexel.rgb*2.0-vec3(1);local.y*=select(-1.0,1.0,frame.absorption.w>.5);local=vec3(local.xy*frame.clear.z,local.z);local=waterUnit(local,vec3(0,0,1));
  sourceNormal=waterUnit(t*local.x+b*local.y+sourceNormal*local.z,sourceNormal);
 }else if frame.features.x>0.5 && (abs(sampled)>.000001 || frame.features.y>.5){
  let t=waterUnit(v.tangent.xyz-sourceNormal*dot(sourceNormal,v.tangent.xyz),waterOrthogonal(sourceNormal));let b=cross(sourceNormal,t)*v.tangent.w;
  // NORMAL sampler signed red is splatted by the source graph before its tangent-space coordinate conversion.
  let polarity=select(sign(sampled),1.0,frame.features.y>.5);
  sourceNormal=waterUnit((t-b+sourceNormal)*polarity,sourceNormal);
 }
 let n=sourceNormal*faceSign;let eye=waterUnit(frame.eyeTime.xyz-v.world,-frame.forward.xyz);
 if frame.features.w>.5&&frame.features.w<2.5{
  let normal=n*select(1.0,-1.0,dot(n,eye)<0.0);let original=v.clip.xy/frame.settings.xy;let depth=depthAt(vec2<i32>(v.clip.xy));
  let variance=dot(dpdx(normal),dpdx(normal))+dot(dpdy(normal),dpdy(normal));let roughness=clamp(pow(pow(frame.clear.y,4.0)+variance*.25,.25),frame.clear.y,1.0);
  let airNoise=textureSample(noiseMap,noiseSampler,v.uv*8.0+frame.material.xy*frame.eyeTime.w).r;let airWidth=max(fwidth(airNoise),.01);
  if v.clip.z>depth+.000001{discard;}
  let f=clearFresnel(dot(normal,eye));let reflected=reflect(-eye,normal);var reflection=clearEnvironment(reflected,roughness);
  if frame.optics.w>.5{let screen=clearScreenReflection(v.world,normal,reflected);reflection=mix(reflection,screen.rgb,screen.a);}
  var added=reflection*f+clearSun(normal,eye,roughness);var air=0.0;var froth=vec3<f32>(0);
  if frame.features.w>1.5{
   let local=waterLocals(v.world,normal,eye,roughness);added+=local.specular;
   var progression=smoothstep(.15,.95,v.uv.y);if frame.airSettings.w>.5{progression=waterNozzleCoverage(v.uv.y,frame.airSettings.x);}
   let coverage=smoothstep(.65-airWidth,.65+airWidth,airNoise)*progression;
   air=waterAirFraction(frame.clear.w/max(abs(dot(normal,eye)),.1),coverage,frame.turbulence.z);
   var ambient=waterDiffuse(normal);var solarPhase=max(dot(normal,frame.sunDirection.xyz),0.0)/3.141592654;
   if frame.airSettings.z>.5{ambient=waterAirEnvironment(eye);solarPhase=waterAirPhase(dot(frame.sunDirection.xyz,eye),frame.airSettings.y);}
   froth=(ambient+frame.sunRadiance.rgb*solarPhase+local.diffuse)*frame.turbulence.w;
   if frame.surfaceSettings.y>.5{
    let sourceNoise=clamp(textureSample(noiseMap,noiseSampler,uv).r,0.0,1.0);
    let sourceFroth=vec3(pow(sourceNoise,frame.material.z)*frame.settings.w/frame.settings.z)*frame.turbulence.w;
    if frame.surfaceSettings.y>1.5{froth+=sourceFroth;}else{froth=sourceFroth;}
   }
  }
  let ray=v.world-frame.eyeTime.xyz;let distance=length(ray);let fog=heightFogComposition(frame.eyeTime.y,ray.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,${FOG_REFERENCE_HEIGHT}.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
  var fade=1.0;if depth<.999999{fade=clamp(dot(worldAt(original,depth)-v.world,frame.forward.xyz)/frame.optics.y,0.0,1.0);}
  if frame.optics.w>.5{
   let uv=clearSheetUV(v.world,normal,original,depth);let background=textureSampleLevel(sceneColor,sceneSampler,uv,0).rgb;let originalColor=textureSampleLevel(sceneColor,sceneSampler,original,0).rgb;
   let path=frame.clear.w/max(abs(dot(normal,eye)),.1);let transmission=exp(-frame.absorption.rgb*path)*(1.0-f);
   let waterResult=background*transmission+added*fog.x+frame.fogColor.rgb*fog.y*(vec3(1)-transmission);let result=mix(waterResult,froth*fog.x+frame.fogColor.rgb*fog.y,air);
   return vec4(clamp(mix(originalColor,result,fade),vec3(0),vec3(60000)),1);
  }let alpha=f+(1.0-f)*air;return vec4(clamp(((added*(1.0-air)+froth*air)*fog.x+frame.fogColor.rgb*fog.y*alpha)*fade,vec3(0),vec3(60000)),alpha*fade);
 }
 let cosine=dot(n,eye);var fresnel=waterFresnel(cosine);
 if frame.features.y>.5{fresnel=mix(waterFresnel(-cosine),fresnel,normalTexel.a);}
 let noise=clamp(textureSample(noiseMap,noiseSampler,uv).r,0.0,1.0);
 var opacity=mix(v.parameters.y,v.parameters.z,1.0-fresnel);
 let original=v.clip.xy/frame.settings.xy;let depth=depthAt(vec2<i32>(v.clip.xy));
 if frame.optics.w>.5 && v.clip.z>depth+.000001{discard;}
 if depth<.999999{opacity*=clamp(dot(worldAt(original,depth)-v.world,frame.forward.xyz)/frame.optics.y,0.0,1.0);}
 let emission=pow(max(noise,0.0),frame.material.z)*frame.settings.w/frame.settings.z;
 let ray=v.world-frame.eyeTime.xyz;let distance=length(ray);
 let fog=heightFogComposition(frame.eyeTime.y,ray.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,${FOG_REFERENCE_HEIGHT}.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 let color=vec3(emission)*fog.x+frame.fogColor.rgb*fog.y;
 if frame.optics.w>.5{
  let ior=mix(frame.optics.z,v.parameters.x,noise);
  if frame.features.z>.5{
   let positiveUV=sourceScreenUV(v.world,n,ior,original,depth);
   let positiveBackground=textureSampleLevel(sceneColor,sceneSampler,positiveUV,0).rgb;
   if frame.features.y>.5{
    // Coverage averages final colors: signed displacement and opacity are correlated.
    var fade=1.0;if depth<.999999{fade=clamp(dot(worldAt(original,depth)-v.world,frame.forward.xyz)/frame.optics.y,0.0,1.0);}
    let positiveOpacity=mix(v.parameters.y,v.parameters.z,1.0-waterFresnel(cosine))*fade;
    let negativeOpacity=mix(v.parameters.y,v.parameters.z,1.0-waterFresnel(-cosine))*fade;
    let negativeUV=sourceScreenUV(v.world,-n,ior,original,depth);
    let negativeBackground=textureSampleLevel(sceneColor,sceneSampler,negativeUV,0).rgb;
    let positiveColor=positiveBackground*(1.0-positiveOpacity)+color*positiveOpacity;
    let negativeColor=negativeBackground*(1.0-negativeOpacity)+color*negativeOpacity;
    return vec4(mix(negativeColor,positiveColor,normalTexel.a),1);
   }
   return vec4(positiveBackground*(1.0-opacity)+color*opacity,1);
  }
  let refracted=refractedUV(v.world,n,ior,original,depth);
  let background=textureSampleLevel(sceneColor,sceneSampler,refracted,0).rgb;
  return vec4(background*(1.0-opacity)+color*opacity,1);
 }return vec4(color*opacity,opacity);
}
`;

async function fetchPrivate(
  url: string,
  limit: number,
  expected?: { byteLength: number; sha256: string },
): Promise<ArrayBuffer> {
  return readAsset(
    url,
    async (response) => {
      if (
        !response.ok ||
        Number(response.headers.get("content-length")) > limit ||
        !response.body
      )
        throw new Error(
          "Prepare the private source water streams before opening this world.",
        );
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let bytes = 0;
      try {
        while (true) {
          const part = await reader.read();
          if (part.done) break;
          bytes += part.value.byteLength;
          if (bytes > limit)
            throw new Error(
              "The source water stream resource exceeds its byte limit.",
            );
          chunks.push(part.value);
        }
      } catch (error) {
        await reader.cancel();
        throw error;
      }
      const result = new Uint8Array(bytes);
      let offset = 0;
      for (const chunk of chunks) {
        result.set(chunk, offset);
        offset += chunk.length;
      }
      if (expected) await verifyAssetBytes(result, expected);
      return result.buffer;
    },
    {
      init: {
        redirect: "error",
        credentials: "same-origin",
      },
    },
  );
}
async function checkedBlob(
  descriptor: WaterStreamBlob,
  load: typeof fetchPrivate,
) {
  const bytes = await load(descriptor.url, descriptor.byteLength, descriptor);
  if (bytes.byteLength !== descriptor.byteLength)
    throw new Error(
      "The source water stream resource has the wrong byte count.",
    );
  const digest = [
    ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
  ]
    .map((x) => x.toString(16).padStart(2, "0"))
    .join("");
  if (digest !== descriptor.sha256)
    throw new Error(
      "The source water stream resource changed; prepare it again.",
    );
  return bytes;
}
/** Linear data-map box filtering includes every texel, including odd sizes. */
export function waterNoiseMips(
  pixels: Uint8Array,
  width: number,
  height: number,
) {
  integer(width, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
  integer(height, 1, SOURCE_WATER_STREAM_LIMITS.textureDimension);
  if (pixels.byteLength !== width * height * 4)
    throw new Error("The source water noise pixels have the wrong byte count.");
  const levels = [{ pixels, width, height }];
  while (width > 1 || height > 1) {
    const nextWidth = Math.max(1, Math.floor(width / 2));
    const nextHeight = Math.max(1, Math.floor(height / 2));
    const next = new Uint8Array(nextWidth * nextHeight * 4);
    for (let y = 0; y < nextHeight; y++)
      for (let x = 0; x < nextWidth; x++) {
        const firstX = (x * width) / nextWidth,
          lastX = ((x + 1) * width) / nextWidth;
        const firstY = (y * height) / nextHeight,
          lastY = ((y + 1) * height) / nextHeight;
        for (let channel = 0; channel < 4; channel++) {
          let sum = 0;
          for (let sy = Math.floor(firstY); sy < Math.ceil(lastY); sy++)
            for (let sx = Math.floor(firstX); sx < Math.ceil(lastX); sx++) {
              const area =
                (Math.min(sx + 1, lastX) - Math.max(sx, firstX)) *
                (Math.min(sy + 1, lastY) - Math.max(sy, firstY));
              sum += pixels[(sy * width + sx) * 4 + channel] * area;
            }
          next[(y * nextWidth + x) * 4 + channel] = Math.round(
            sum / ((lastX - firstX) * (lastY - firstY)),
          );
        }
      }
    pixels = next;
    width = nextWidth;
    height = nextHeight;
    levels.push({ pixels, width, height });
  }
  return levels;
}
export class SourceWaterStreams {
  readonly diagnostics: readonly string[];
  private readonly instances = new Float32Array(64 * 20);
  private readonly uniforms = new Float32Array(
    SOURCE_WATER_STREAM_FRAME_FLOATS,
  );
  private readonly emptyEnvironmentView: GPUTextureView;
  private group?: {
    depth: GPUTextureView;
    source: GPUTexture;
    environment: GPUTextureView;
    value: GPUBindGroup;
  };
  private nearestDepth?: GPUTexture;
  private disposed = false;
  private constructor(
    private readonly device: GPUDevice,
    private readonly manifest: SourceWaterStreamsManifest,
    private readonly pipeline: GPURenderPipeline,
    private readonly refractivePipeline: GPURenderPipeline,
    private readonly layout: GPUBindGroupLayout,
    private readonly vertices: GPUBuffer,
    private readonly indices: GPUBuffer,
    private readonly placements: GPUBuffer,
    private readonly frame: GPUBuffer,
    private readonly noise: GPUTexture,
    private readonly normal: GPUTexture,
    private readonly emptyScene: GPUTexture,
    private readonly sampler: GPUSampler,
    private readonly sceneSampler: GPUSampler,
    private readonly emptyEnvironment: GPUTexture,
    private readonly environmentSampler: GPUSampler,
  ) {
    this.emptyEnvironmentView = emptyEnvironment.createView({
      dimension: "2d-array",
    });
    this.diagnostics =
      manifest.material.appearanceProfile === "owned-source-emissive-stream-v1"
        ? OWNED_EMISSIVE_STREAM_DIAGNOSTICS
        : manifest.material.appearanceProfile === "owned-turbulent-stream-v1"
          ? OWNED_TURBULENT_STREAM_DIAGNOSTICS
          : manifest.material.appearanceProfile !== undefined
            ? OWNED_CLEAR_STREAM_DIAGNOSTICS
            : SOURCE_WATER_STREAM_DIAGNOSTICS;
  }
  static async create(
    device: GPUDevice,
    recipe: SourceWaterStreamsManifest,
    load = fetchPrivate,
  ) {
    validateSourceWaterStreams(recipe);
    const manifest = structuredClone(recipe);
    const [sourceVertexBytes, indexBytes, noiseBytes, normalBytes] =
      await Promise.all([
        checkedBlob(manifest.vertices, load),
        checkedBlob(manifest.indices, load),
        checkedBlob(manifest.noise, load),
        manifest.normal ? checkedBlob(manifest.normal, load) : undefined,
      ]);
    const stride = (manifest.vertexStride ?? 32) / 4;
    const positions = new Float32Array(sourceVertexBytes),
      indices = new Uint32Array(indexBytes);
    for (let i = 0; i < positions.length; i++)
      finite(positions[i], -1000, 1000);
    for (let i = 0; i < positions.length; i += stride) {
      if (
        Math.hypot(positions[i + 3], positions[i + 4], positions[i + 5]) < 0.01
      )
        throw new Error("The source water stream mesh needs nonzero normals.");
      if (
        stride === 12 &&
        (Math.hypot(positions[i + 8], positions[i + 9], positions[i + 10]) <
          0.01 ||
          Math.abs(Math.abs(positions[i + 11]) - 1) > 0.001)
      )
        throw new Error(
          "The source water stream needs bounded nonzero tangents and a signed handedness.",
        );
    }
    // Keep old prepared recipes usable without inventing a normal-map response.
    const expanded = new Float32Array(manifest.vertexCount * 12);
    for (let i = 0; i < manifest.vertexCount; i++) {
      expanded.set(positions.subarray(i * stride, i * stride + stride), i * 12);
      if (stride === 8) {
        const n = positions.subarray(i * stride + 3, i * stride + 6);
        const axis =
          Math.abs(n[1]) > 0.9 * Math.hypot(...n) ? [1, 0, 0] : [0, 1, 0];
        const tangent = [
          axis[1] * n[2] - axis[2] * n[1],
          axis[2] * n[0] - axis[0] * n[2],
          axis[0] * n[1] - axis[1] * n[0],
        ];
        const length = Math.hypot(...tangent);
        expanded.set([...tangent.map((x) => x / length), 1], i * 12 + 8);
      } else {
        const at = i * stride,
          n = positions.subarray(at + 3, at + 6),
          t = positions.subarray(at + 8, at + 11);
        if (
          Math.hypot(
            n[1] * t[2] - n[2] * t[1],
            n[2] * t[0] - n[0] * t[2],
            n[0] * t[1] - n[1] * t[0],
          ) < 0.0001
        )
          throw new Error(
            "The source water tangent cannot be parallel to its normal.",
          );
      }
    }
    const vertexBytes = expanded.buffer;
    for (const index of indices)
      if (index >= manifest.vertexCount)
        throw new Error(
          "The source water stream index leaves its vertex buffer.",
        );
    const bounds = new Map(
      manifest.meshes.map((mesh) => {
        const min = [Infinity, Infinity, Infinity],
          max = [-Infinity, -Infinity, -Infinity];
        for (
          let i = mesh.firstIndex;
          i < mesh.firstIndex + mesh.indexCount;
          i++
        )
          for (let axis = 0; axis < 3; axis++) {
            const coordinate = positions[indices[i] * stride + axis];
            min[axis] = Math.min(min[axis], coordinate);
            max[axis] = Math.max(max[axis], coordinate);
          }
        return [mesh.id, { min, max }] as const;
      }),
    );
    for (const scenario of [manifest.scenarios.day, manifest.scenarios.night])
      for (const instance of scenario) {
        const box = bounds.get(instance.meshId)!,
          m = instance.model;
        for (let corner = 0; corner < 8; corner++) {
          const point = [0, 1, 2].map(
            (axis) => (corner & (1 << axis) ? box.max : box.min)[axis],
          );
          for (let axis = 0; axis < 3; axis++)
            finite(
              m[axis] * point[0] +
                m[axis + 4] * point[1] +
                m[axis + 8] * point[2] +
                m[axis + 12],
              -100000,
              100000,
            );
        }
      }
    const decoded = decodeRgba8(noiseBytes);
    if (
      decoded.width !== manifest.noise.width ||
      decoded.height !== manifest.noise.height ||
      decoded.width > device.limits.maxTextureDimension2D ||
      decoded.height > device.limits.maxTextureDimension2D
    )
      throw new Error("The source water noise dimensions are unsupported.");
    const mips = waterNoiseMips(decoded.pixels, decoded.width, decoded.height);
    const decodedNormal = normalBytes
      ? decodeRgba8(normalBytes)
      : { pixels: new Uint8Array([128, 128, 255, 255]), width: 1, height: 1 };
    if (
      manifest.normal &&
      (decodedNormal.width !== manifest.normal.width ||
        decodedNormal.height !== manifest.normal.height ||
        decodedNormal.width > device.limits.maxTextureDimension2D ||
        decodedNormal.height > device.limits.maxTextureDimension2D)
    )
      throw new Error("The source water normal dimensions are unsupported.");
    const normalMips = waterNoiseMips(
      decodedNormal.pixels,
      decodedNormal.width,
      decodedNormal.height,
    );
    const module = device.createShaderModule({
      code: WGSL_SOURCE_WATER_STREAMS,
    });
    const compilation = await module.getCompilationInfo();
    const failures = compilation.messages.filter(
      (message) => message.type === "error",
    );
    if (failures.length)
      throw new Error(
        failures
          .map(
            (message) =>
              `Water shader ${message.lineNum}:${message.linePos}: ${message.message}`,
          )
          .join("\n"),
      );
    const layout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "depth", multisampled: true },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 3,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
        {
          binding: 4,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 5,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 6,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
        {
          binding: 7,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 8,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
      ],
    });
    const pipelineLayout = device.createPipelineLayout({
      bindGroupLayouts: [layout],
    });
    const descriptor: GPURenderPipelineDescriptor = {
      layout: pipelineLayout,
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 48,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x3" },
              { shaderLocation: 2, offset: 24, format: "float32x2" },
              { shaderLocation: 8, offset: 32, format: "float32x4" },
            ],
          },
          {
            arrayStride: 80,
            stepMode: "instance",
            attributes: [0, 1, 2, 3, 4].map((i) => ({
              shaderLocation: 3 + i,
              offset: i * 16,
              format: "float32x4" as const,
            })),
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [
          {
            format: "rgba16float",
            blend: {
              color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
              alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
            },
          },
        ],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      multisample: { count: 4 },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
    };
    const pipeline = await device.createRenderPipelineAsync(descriptor);
    const refractivePipeline = await device.createRenderPipelineAsync({
      ...descriptor,
      fragment: {
        module,
        entryPoint: "fs",
        targets: [{ format: "rgba16float" }],
      },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "less",
      },
    });
    const resources: Array<GPUBuffer | GPUTexture> = [];
    try {
      const buffer = (
        size: number,
        usage: GPUBufferUsageFlags,
        data?: ArrayBuffer,
      ) => {
        const b = device.createBuffer({
          size,
          usage: usage | GPUBufferUsage.COPY_DST,
        });
        resources.push(b);
        if (data) device.queue.writeBuffer(b, 0, data);
        return b;
      };
      const vertices = buffer(
        vertexBytes.byteLength,
        GPUBufferUsage.VERTEX,
        vertexBytes,
      );
      const indices = buffer(
        indexBytes.byteLength,
        GPUBufferUsage.INDEX,
        indexBytes,
      );
      const placements = buffer(64 * 80, GPUBufferUsage.VERTEX);
      const frame = buffer(
        SOURCE_WATER_STREAM_FRAME_FLOATS * 4,
        GPUBufferUsage.UNIFORM,
      );
      const noise = device.createTexture({
        size: [decoded.width, decoded.height],
        format: "rgba8unorm",
        mipLevelCount: mips.length,
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      resources.push(noise);
      mips.forEach((level, mipLevel) =>
        device.queue.writeTexture(
          { texture: noise, mipLevel },
          level.pixels,
          { bytesPerRow: level.width * 4 },
          [level.width, level.height],
        ),
      );
      const normal = device.createTexture({
        size: [decodedNormal.width, decodedNormal.height],
        format: "rgba8unorm",
        mipLevelCount: normalMips.length,
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      resources.push(normal);
      normalMips.forEach((level, mipLevel) =>
        device.queue.writeTexture(
          { texture: normal, mipLevel },
          level.pixels,
          { bytesPerRow: level.width * 4 },
          [level.width, level.height],
        ),
      );
      const emptyScene = device.createTexture({
        size: [1, 1],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      resources.push(emptyScene);
      device.queue.writeTexture(
        { texture: emptyScene },
        new Uint8Array(8),
        { bytesPerRow: 8 },
        [1, 1],
      );
      const emptyEnvironment = device.createTexture({
        size: [1, 1, 2],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      resources.push(emptyEnvironment);
      device.queue.writeTexture(
        { texture: emptyEnvironment },
        new Uint8Array(16),
        { bytesPerRow: 8, rowsPerImage: 1 },
        [1, 1, 2],
      );
      const sampler = device.createSampler({
        addressModeU: "repeat",
        addressModeV: "repeat",
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "linear",
      });
      const sceneSampler = device.createSampler({
        addressModeU: "clamp-to-edge",
        addressModeV: "clamp-to-edge",
        minFilter: "linear",
        magFilter: "linear",
      });
      const environmentSampler = device.createSampler({
        addressModeU: "repeat",
        addressModeV: "clamp-to-edge",
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "linear",
      });
      return new SourceWaterStreams(
        device,
        manifest,
        pipeline,
        refractivePipeline,
        layout,
        vertices,
        indices,
        placements,
        frame,
        noise,
        normal,
        emptyScene,
        sampler,
        sceneSampler,
        emptyEnvironment,
        environmentSampler,
      );
    } catch (error) {
      for (const resource of resources) resource.destroy();
      throw error;
    }
  }
  get bytes() {
    let textureBytes = 24;
    for (const texture of [this.noise, this.normal]) {
      let width = texture.width,
        height = texture.height;
      while (true) {
        textureBytes += width * height * 4;
        if (width === 1 && height === 1) break;
        width = Math.max(1, Math.floor(width / 2));
        height = Math.max(1, Math.floor(height / 2));
      }
    }
    return this.disposed
      ? 0
      : this.manifest.vertexCount * 48 +
          this.manifest.indices.byteLength +
          textureBytes +
          (this.nearestDepth
            ? this.nearestDepth.width * this.nearestDepth.height * 4 * 4
            : 0) +
          64 * 80 +
          SOURCE_WATER_STREAM_FRAME_FLOATS * 4;
  }
  render(encoder: GPUCommandEncoder, frame: SourceWaterStreamsFrame) {
    if (this.disposed)
      throw new Error("Open source water streams before drawing them.");
    if (frame.mode !== "day" && frame.mode !== "night")
      throw new Error("Choose a prepared water scenario.");
    finite(frame.time, 0, 86400);
    finite(frame.effectiveExposure, 0.000001, 1e6);
    integer(frame.width, 1, 16384);
    integer(frame.height, 1, 16384);
    vector(frame.viewProjection, 16, -1e8, 1e8);
    vector(frame.inverseViewProjection, 16, -1e8, 1e8);
    vector(frame.eye, 3, -1e6, 1e6);
    let source = this.emptyScene,
      resolveTarget = frame.resolveTarget,
      refractive = false;
    if (frame.refraction) {
      if (frame.resolveTarget)
        throw new Error("Choose one source water resolve destination.");
      const input = frame.refraction.source,
        output = frame.refraction.resolve;
      if (
        input === output ||
        [input, output].some(
          (texture) =>
            texture.format !== "rgba16float" ||
            texture.sampleCount !== 1 ||
            texture.depthOrArrayLayers !== 1 ||
            texture.width !== frame.width ||
            texture.height !== frame.height,
        ) ||
        !(input.usage & GPUTextureUsage.TEXTURE_BINDING) ||
        !(output.usage & GPUTextureUsage.RENDER_ATTACHMENT)
      )
        throw new Error("Use separate matching source water color textures.");
      resolveTarget = output.createView();
      refractive =
        frame.width * frame.height <=
          SOURCE_WATER_STREAM_LIMITS.refractionPixels &&
        frame.width * frame.height * 16 <=
          SOURCE_WATER_STREAM_LIMITS.refractionDepthBytes &&
        frame.width <= this.device.limits.maxTextureDimension2D &&
        frame.height <= this.device.limits.maxTextureDimension2D;
      if (refractive) {
        source = input;
        if (
          this.nearestDepth?.width !== frame.width ||
          this.nearestDepth.height !== frame.height
        ) {
          this.nearestDepth?.destroy();
          this.nearestDepth = this.device.createTexture({
            size: [frame.width, frame.height],
            format: "depth24plus",
            sampleCount: 4,
            usage: GPUTextureUsage.RENDER_ATTACHMENT,
          });
        }
      }
    }
    if (!refractive && this.nearestDepth) {
      this.nearestDepth.destroy();
      this.nearestDepth = undefined;
    }
    const fog = frame.fog ?? { density: 0, color: [0, 0, 0] };
    finite(fog.density, 0, 100);
    finite(fog.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
    vector(fog.color, 3, 0, 1e6);
    const volumeMedium = fog.volumeMedium ?? [0, 0, 0, 0];
    const volumeRange = fog.volumeRange ?? [0, 0];
    heightFogComposition(
      frame.eye[1],
      0,
      0,
      fog.density,
      fog.heightRate ?? FOG_HEIGHT_RATE,
      FOG_REFERENCE_HEIGHT,
      0,
      volumeMedium,
      volumeRange,
    );
    const unproject = (z: number) => {
      const m = frame.inverseViewProjection,
        w = m[11] * z + m[15];
      if (Math.abs(w) < 1e-12)
        throw new Error("The water camera cannot be unprojected.");
      return [0, 1, 2].map((i) => (m[8 + i] * z + m[12 + i]) / w);
    };
    const near = unproject(0),
      far = unproject(0.5),
      forward = far.map((v, i) => v - near[i]),
      length = Math.hypot(...forward);
    if (length < 1e-10)
      throw new Error("The water camera needs a stable viewing direction.");
    forward.forEach((_, i) => (forward[i] /= length));
    const sorted = this.manifest.scenarios[frame.mode]
      .map((instance) => {
        const mesh = this.manifest.meshes.find(
            (m) => m.id === instance.meshId,
          )!,
          m = instance.model,
          c = mesh.center;
        const depth = [0, 1, 2].reduce(
          (sum, axis) =>
            sum +
            (m[axis] * c[0] +
              m[axis + 4] * c[1] +
              m[axis + 8] * c[2] +
              m[axis + 12] -
              frame.eye[axis]) *
              forward[axis],
          0,
        );
        return { instance, mesh, depth };
      })
      .sort(
        (a, b) =>
          b.depth - a.depth || a.instance.id.localeCompare(b.instance.id),
      );
    const u = this.uniforms,
      material = this.manifest.material;
    u.set(Array.from(frame.viewProjection), 0);
    u.set(Array.from(frame.inverseViewProjection), 16);
    u.set(
      [
        ...frame.eye,
        material.appearanceProfile === "owned-turbulent-stream-v1" ||
        material.appearanceProfile === "owned-source-emissive-stream-v1"
          ? frame.time % 8
          : frame.time,
      ],
      32,
    );
    u.set(
      [frame.width, frame.height, frame.effectiveExposure, SCENE_LINEAR_SCALE],
      36,
    );
    u.set([...forward, 0], 40);
    u.set(
      [...material.flow, material.powerExponent, material.fresnelExponent],
      44,
    );
    u.set(
      [
        material.baseReflectFraction,
        material.depthFadeMeters,
        material.refractionMinimum,
        refractive ? 1 : 0,
      ],
      48,
    );
    u.set([fog.density, fog.heightRate ?? FOG_HEIGHT_RATE, 0, 0], 52);
    u.set([...fog.color, 0], 56);
    u.set(volumeMedium, 60);
    u.set([...volumeRange, 0, 0], 64);
    u.set(
      [
        this.manifest.normal ? 1 : 0,
        this.manifest.normal?.coverage ? 1 : 0,
        material.refractionMode === "source-screen-displacement-v1" ? 1 : 0,
        material.appearanceProfile === "owned-source-emissive-stream-v1"
          ? 3
          : material.appearanceProfile === "owned-turbulent-stream-v1"
            ? 2
            : material.appearanceProfile === "owned-clear-stream-v1"
              ? 1
              : 0,
      ],
      68,
    );
    const owned =
        material.appearanceProfile === "owned-clear-stream-v1" ||
        material.appearanceProfile === "owned-turbulent-stream-v1",
      environment = owned ? frame.environment : undefined,
      lighting = owned ? frame.lighting : undefined,
      clear = material.clearWater ?? {
        ior: 1,
        roughness: 1,
        normalStrength: material.emissiveWater?.normalStrength ?? 0,
        normalTiling: material.emissiveWater?.normalTiling ?? 1,
        thicknessMeters: 0.01,
        absorption: [0, 0, 0],
      };
    u.fill(0, 72);
    u.set([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], 72);
    if (environment) {
      vector(environment.worldToEnvironment, 16, -1e6, 1e6);
      vector(environment.gain, 3, 0, 1e6);
      finite(environment.mipMax, 0, 16);
      finite(environment.solarRatio ?? 1, 0, 1e4);
      if (
        environment.hasConstantFog !== undefined &&
        typeof environment.hasConstantFog !== "boolean"
      )
        throw new Error("Use a supported water environment layer description.");
      u.set(Array.from(environment.worldToEnvironment), 72);
      u.set(
        [
          1,
          environment.mipMax,
          environment.solarRatio ?? 1,
          environment.hasConstantFog ? 1 : 0,
        ],
        88,
      );
      u.set([...environment.gain, 0], 92);
    }
    if (lighting) {
      vector(lighting.sunDirectionTowardLight, 3, -1, 1);
      vector(lighting.sunRadianceRGB, 3, 0, 1e4);
      if (Math.abs(Math.hypot(...lighting.sunDirectionTowardLight) - 1) > 0.001)
        throw new Error("Water sunlight needs a normalized direction.");
      u.set([...lighting.sunDirectionTowardLight, 0], 96);
      u.set(
        [...lighting.sunRadianceRGB.map((c) => c * SCENE_LINEAR_SCALE), 0],
        100,
      );
    }
    u[99] = clear.normalTiling ?? 1;
    u.set(
      [clear.ior, clear.roughness, clear.normalStrength, clear.thicknessMeters],
      104,
    );
    u.set(
      [...clear.absorption, this.manifest.normal?.flipGreenChannel ? 1 : 0],
      108,
    );
    if (material.emissiveWater) {
      // Repeat addressing permits a bounded UV phase without changing source flow.
      u.set(
        material.flow.map((speed) => (((speed * frame.time) % 1) + 1) % 1),
        88,
      );
      u.set(
        [
          material.emissiveWater.geometryAmplitudeMeters,
          material.emissiveWater.microAmplitudeMeters,
          0,
          0,
        ],
        112,
      );
    }
    if (material.appearanceProfile === "owned-turbulent-stream-v1") {
      const t = material.turbulence!;
      if (t.airModel?.kind === "source-energy-foam-v1") u[117] = 1;
      if (t.airModel?.kind === "source-energy-foam-additive-v1") u[117] = 2;
      if (t.airModel)
        u.set(
          [
            t.airModel.nozzleFraction,
            t.airModel.kind === "henyey-greenstein-v1"
              ? t.airModel.asymmetry
              : 0,
            t.airModel.kind === "henyey-greenstein-v1" ? 1 : 0,
            1,
          ],
          684,
        );
      u.set(
        [
          t.geometryAmplitudeMeters,
          t.microAmplitudeMeters,
          t.airScatteringPerMeter,
          t.airAlbedo,
        ],
        112,
      );
      if (
        lighting?.diffuseSH !== undefined ||
        lighting?.diffuseRotation !== undefined
      ) {
        vector(lighting.diffuseSH!, 36, -1e9, 1e9);
        vector(lighting.diffuseRotation!, 16, -1e6, 1e6);
        u.set(Array.from(lighting.diffuseRotation!), 120);
        u.set(
          Array.from(lighting.diffuseSH!).map((v) => v * SCENE_LINEAR_SCALE),
          136,
        );
      }
      if (lighting?.localLights?.length && sorted.length) {
        const anchors = sorted.map(({ instance, mesh }) =>
          [0, 1, 2].map(
            (a) =>
              instance.model[a] * mesh.center[0] +
              instance.model[a + 4] * mesh.center[1] +
              instance.model[a + 8] * mesh.center[2] +
              instance.model[a + 12],
          ),
        );
        const packed = packWaterLocalLights(lighting.localLights, anchors);
        u[116] = packed.count;
        u.set(packed.positions, 172);
        u.set(packed.colors, 300);
        u.set(packed.directions, 428);
        u.set(packed.cones, 556);
      }
    }
    this.device.queue.writeBuffer(this.frame, 0, u);
    sorted.forEach(({ instance }, i) => {
      this.instances.set(instance.model, i * 20);
      this.instances.set(instance.parameters, i * 20 + 16);
    });
    if (sorted.length)
      this.device.queue.writeBuffer(
        this.placements,
        0,
        this.instances,
        0,
        sorted.length * 20,
      );
    const environmentView = environment?.view ?? this.emptyEnvironmentView;
    if (
      this.group?.depth !== frame.depthView ||
      this.group.source !== source ||
      this.group.environment !== environmentView
    )
      this.group = {
        depth: frame.depthView,
        source,
        environment: environmentView,
        value: this.device.createBindGroup({
          layout: this.layout,
          entries: [
            { binding: 0, resource: { buffer: this.frame } },
            { binding: 1, resource: frame.depthView },
            { binding: 2, resource: this.noise.createView() },
            { binding: 3, resource: this.sampler },
            { binding: 4, resource: this.normal.createView() },
            { binding: 5, resource: source.createView() },
            { binding: 6, resource: this.sceneSampler },
            { binding: 7, resource: environmentView },
            { binding: 8, resource: this.environmentSampler },
          ],
        }),
      };
    const pass = encoder.beginRenderPass({
      label: "Source fountain streams",
      colorAttachments: [
        {
          view: frame.colorView,
          ...(resolveTarget ? { resolveTarget } : {}),
          loadOp: "load",
          storeOp: "store",
        },
      ],
      depthStencilAttachment: refractive
        ? {
            view: this.nearestDepth!.createView(),
            depthClearValue: 1,
            depthLoadOp: "clear",
            depthStoreOp: "store",
          }
        : { view: frame.depthView, depthReadOnly: true },
    });
    pass.setPipeline(refractive ? this.refractivePipeline : this.pipeline);
    pass.setBindGroup(0, this.group.value);
    pass.setVertexBuffer(0, this.vertices);
    pass.setIndexBuffer(this.indices, "uint32");
    let triangles = 0;
    for (let start = 0; start < sorted.length;) {
      let end = start + 1;
      while (
        end < sorted.length &&
        sorted[end].mesh.id === sorted[start].mesh.id
      )
        end++;
      const mesh = sorted[start].mesh;
      pass.setVertexBuffer(1, this.placements, start * 80);
      pass.drawIndexed(mesh.indexCount, end - start, mesh.firstIndex);
      triangles += (mesh.indexCount / 3) * (end - start);
      start = end;
    }
    pass.end();
    return { instances: sorted.length, triangles, refraction: refractive };
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.vertices.destroy();
    this.indices.destroy();
    this.placements.destroy();
    this.frame.destroy();
    this.noise.destroy();
    this.normal.destroy();
    this.emptyScene.destroy();
    this.emptyEnvironment.destroy();
    this.nearestDepth?.destroy();
    this.group = undefined;
  }
}
