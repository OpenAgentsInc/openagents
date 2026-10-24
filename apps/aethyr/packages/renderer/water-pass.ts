import {
  buildWaterJets,
  buildWaterSplashes,
  WATER_JET_BREAKUP_METERS,
  type WaterJet,
  type WaterVec3,
  type WaterImpact,
} from "./water.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_SEGMENT_WGSL,
  heightFogComposition,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";

export const PLANAR_WATER_DISTORTION = 0.035;
export const PLANAR_WATER_TOLERANCE = 0.03;

export interface WaterBasin {
  id: string;
  polygonXZ: ReadonlyArray<readonly [number, number]>;
  surfaceY: number;
  floorY?: number;
  /** Column-major local-to-world affine matrix; the resulting surface must be horizontal. */
  model?: readonly number[];
  /** Lets the host omit exactly these source water primitives from opaque drawing. */
  source?: { meshId: string; materialId: string };
}
export interface WaterManifest {
  schema: "aethyrforge.water.v1";
  basins: readonly WaterBasin[];
  jets: readonly WaterJet[];
}
export interface WaterLight {
  position: WaterVec3;
  /** Linear HDR radiance with the host's intensity conversion already applied. */
  color: WaterVec3;
  range: number;
  /** Omitted uses physical inverse-square falloff; a nonnegative exponent selects legacy source falloff. */
  falloffExponent?: number;
}
export interface WaterEnvironment {
  /** Two-layer linear HDR GGX map: solar radiance, then optional constant fog. */
  view: GPUTextureView;
  worldToEnvironment: ArrayLike<number>;
  mipMax: number;
  /** Linear RGB multiplier, including the host's common scene storage scale exactly once. */
  gain: WaterVec3;
  solarRatio?: number;
  hasConstantFog?: boolean;
}
export interface WaterPlanarReflection {
  /** Separate linear HDR texture rendered from the mirrored camera, excluding all water. */
  view: GPUTextureView;
  viewProjection: ArrayLike<number>;
  planeY: number;
  planeTolerance?: number;
  /** Bounded UV perturbation strength for small surface ripples; defaults to .035. */
  distortion?: number;
}
export interface WaterPassFrame {
  width: number;
  height: number;
  /** Existing 4xMSAA HDR color, stored by the opaque pass. */
  colorView: GPUTextureView;
  /** A separate single-sample HDR output; never the sampled sceneColor texture. */
  resolveTarget: GPUTextureView;
  /** Existing depth24plus, 4xMSAA, created with TEXTURE_BINDING. */
  depthView: GPUTextureView;
  /** Resolved, linear HDR opaque color, before exposure and display conversion. */
  sceneColor: GPUTextureView;
  viewProjection: ArrayLike<number>;
  inverseViewProjection: ArrayLike<number>;
  eye: WaterVec3;
  time: number;
  sunDirection?: WaterVec3;
  sunColor?: WaterVec3;
  ambient?: WaterVec3;
  skyZenith?: WaterVec3;
  skyHorizon?: WaterVec3;
  fog?: {
    color: WaterVec3;
    density: number;
    volumeMedium?: FogVolumeMedium;
    volumeRange?: FogVolumeRange;
  };
  lights?: readonly WaterLight[];
  /** Disables the bounded scene-depth reflection search for a cheaper sky-only profile. */
  screenSpaceReflections?: boolean;
  environment?: WaterEnvironment;
  planar?: WaterPlanarReflection;
}

const IDENTITY = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
const MAX_BASINS = 64;
const FRAME_FLOATS = 512;
const LAYOUT: GPUVertexBufferLayout = {
  arrayStride: 48,
  attributes: [
    { shaderLocation: 0, offset: 0, format: "float32x3" },
    { shaderLocation: 1, offset: 12, format: "float32x3" },
    { shaderLocation: 2, offset: 24, format: "float32x2" },
    { shaderLocation: 3, offset: 32, format: "float32x4" },
  ],
};

/** Rebuilds the measured convex basin polygon, without embedding asset coordinates. */
export function buildWaterBasins(basins: readonly WaterBasin[]) {
  if (!Array.isArray(basins) || basins.length > MAX_BASINS)
    throw new Error("Use at most 64 water basins in one pass.");
  const vertices: number[] = [],
    indices: number[] = [];
  const ids = new Set<string>();
  for (const basin of basins as readonly WaterBasin[]) {
    if (
      !basin ||
      typeof basin.id !== "string" ||
      !basin.id.trim() ||
      basin.id.length > 128 ||
      ids.has(basin.id)
    )
      throw new Error(
        "Each water basin needs a unique name up to 128 characters.",
      );
    ids.add(basin.id);
    if (
      !Array.isArray(basin.polygonXZ) ||
      basin.polygonXZ.length < 3 ||
      basin.polygonXZ.length > 256 ||
      !basin.polygonXZ.every(
        (p) => Array.isArray(p) && p.length === 2 && p.every(Number.isFinite),
      ) ||
      !Number.isFinite(basin.surfaceY) ||
      (basin.floorY !== undefined && !Number.isFinite(basin.floorY))
    )
      throw new Error("A water basin needs a finite polygon and water height.");
    const m = basin.model ?? IDENTITY;
    if (
      m.length !== 16 ||
      !m.every(Number.isFinite) ||
      Math.abs(m[3]) + Math.abs(m[7]) + Math.abs(m[11]) > 1e-6 ||
      Math.abs(m[15] - 1) > 1e-6
    )
      throw new Error("Use a finite affine transform for the water basin.");
    const points = basin.polygonXZ.map(([x, z]) => [
      m[0] * x + m[4] * basin.surfaceY + m[8] * z + m[12],
      m[1] * x + m[5] * basin.surfaceY + m[9] * z + m[13],
      m[2] * x + m[6] * basin.surfaceY + m[10] * z + m[14],
    ]);
    if (
      points.some((p) =>
        p.some((n) => !Number.isFinite(n) || Math.abs(n) > 100_000),
      ) ||
      points.some((p) => Math.abs(p[1] - points[0][1]) > 1e-4)
    )
      throw new Error(
        "Keep basin water horizontal and within the world bounds.",
      );
    let area = 0,
      turn = 0;
    const orient = (a: number[], b: number[], c: number[]) =>
      (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
    for (let i = 0; i < points.length; i++) {
      const a = points[i],
        b = points[(i + 1) % points.length],
        c = points[(i + 2) % points.length];
      if (Math.hypot(a[0] - b[0], a[2] - b[2]) < 1e-5)
        throw new Error("Remove repeated points from the water polygon.");
      area += a[0] * b[2] - b[0] * a[2];
      const cross = orient(a, b, c);
      if (Math.abs(cross) > 1e-8) {
        if (turn && Math.sign(cross) !== turn)
          throw new Error("Use a convex water polygon.");
        turn = Math.sign(cross);
      }
      for (let j = i + 2; j < points.length; j++) {
        if (i === 0 && j === points.length - 1) continue;
        const d = points[j],
          e = points[(j + 1) % points.length];
        if (
          orient(a, b, d) * orient(a, b, e) < 0 &&
          orient(d, e, a) * orient(d, e, b) < 0
        )
          throw new Error("Water polygon edges must not cross.");
      }
    }
    if (Math.abs(area) < 1e-6)
      throw new Error("The water polygon needs some area.");
    const depth =
      basin.floorY === undefined ? 0.5 : m[5] * (basin.surfaceY - basin.floorY);
    if (!Number.isFinite(depth) || depth <= 0 || depth > 100)
      throw new Error(
        "Keep the basin floor below the water within 100 meters.",
      );
    const first = vertices.length / 12;
    for (const point of points)
      vertices.push(...point, 0, 1, 0, 0, depth, 1, 0, 0, 1);
    for (let i = 1; i < points.length - 1; i++)
      indices.push(
        first,
        first + (area > 0 ? i + 1 : i),
        first + (area > 0 ? i : i + 1),
      );
  }
  return {
    vertices: new Float32Array(vertices),
    indices: new Uint32Array(indices),
  };
}

export const WATER_PASS_SHADER = /* wgsl */ `
${FOG_SEGMENT_WGSL}
struct Frame {
 view:mat4x4<f32>, inverseView:mat4x4<f32>, eyeTime:vec4<f32>, sun:vec4<f32>, sunColor:vec4<f32>,
 ambient:vec4<f32>, fog:vec4<f32>, viewport:vec4<f32>, optics:vec4<f32>, absorption:vec4<f32>,
 skyZenith:vec4<f32>, skyHorizon:vec4<f32>, impacts:array<vec4<f32>,64>,
 lightPositions:array<vec4<f32>,16>, lightColors:array<vec4<f32>,16>, counts:vec4<f32>,
 environmentRotation:mat4x4<f32>, environmentSettings:vec4<f32>, environmentGain:vec4<f32>,
 planarProjection:mat4x4<f32>, planarSettings:vec4<f32>, volumeMedium:vec4<f32>,volumeRange:vec4<f32>
};
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var sceneColor:texture_2d<f32>;
@group(0) @binding(2) var sceneDepth:texture_depth_multisampled_2d;
@group(0) @binding(3) var colorSampler:sampler;
@group(0) @binding(4) var environmentMap:texture_2d_array<f32>;
@group(0) @binding(5) var environmentSampler:sampler;
@group(0) @binding(6) var planarReflection:texture_2d<f32>;
struct In { @location(0) p:vec3<f32>, @location(1) n:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) tangent:vec4<f32> };
struct Out { @builtin(position) clip:vec4<f32>, @location(0) p:vec3<f32>, @location(1) n:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) tangent:vec4<f32> };
@vertex fn vs(v:In)->Out {var o:Out;o.clip=frame.view*vec4(v.p,1);o.p=v.p;o.n=v.n;o.uv=v.uv;o.tangent=v.tangent;return o;}
@vertex fn vsJet(v:In)->Out {
 let progress=v.tangent.w;let angle=v.uv.x*6.2831853;let time=frame.eyeTime.w;
 let band=sin(v.uv.y*113.0-time*18.849556+angle*3.0)*0.7+sin(v.uv.y*191.0-time*31.415927-angle*5.0)*0.3;
 let envelope=${WATER_JET_BREAKUP_METERS}*progress*progress;
 let p=v.p+v.n*(band*envelope*0.6666667)+v.tangent.xyz*(cos(v.uv.y*137.0-time*25.132741+angle*4.0)*envelope*0.3333333);
 var o:Out;o.p=p;o.clip=frame.view*vec4(p,1);o.n=v.n;o.uv=v.uv;o.tangent=v.tangent;return o;
}
@vertex fn vsSplash(v:In)->Out {
 let age=fract(frame.eyeTime.w/v.tangent.y+v.tangent.x)*v.tangent.y;
 let center=v.p+v.n*age-vec3(0,0.5*v.tangent.w*age*age,0)+vec3(0,0.005,0);
 let view=safeNormal(frame.eyeTime.xyz-center,vec3(0,0,1));let velocity=v.n-vec3(0,v.tangent.w*age,0);
 let projected=velocity-view*dot(velocity,view);let up=safeNormal(projected,safeNormal(cross(view,vec3(1,0,0)),vec3(0,1,0)));
 let right=safeNormal(cross(up,view),vec3(1,0,0));let size=v.tangent.z*sin(3.14159265*age/v.tangent.y);
 let stretch=1.0+min(length(projected)*1.5,1.8);
 var o:Out;o.p=center+(right*v.uv.x+up*v.uv.y*stretch)*size;o.n=view;o.clip=frame.view*vec4(o.p,1);o.uv=v.uv;o.tangent=v.tangent;return o;
}
fn safeNormal(v:vec3<f32>,fallback:vec3<f32>)->vec3<f32>{let q=dot(v,v);if q<0.00000001{return fallback;}return v*inverseSqrt(q);}
fn depthAt(uv:vec2<f32>)->f32 {
 let size=vec2<i32>(textureDimensions(sceneDepth));let pixel=clamp(vec2<i32>(uv*vec2<f32>(size)),vec2(0),size-vec2(1));
 var d=1.0;for(var i=0;i<4;i++){d=min(d,textureLoad(sceneDepth,pixel,i));}return d;
}
fn unproject(uv:vec2<f32>,d:f32)->vec3<f32>{let p=frame.inverseView*vec4(uv*vec2(2,-2)+vec2(-1,1),d,1);return p.xyz/max(abs(p.w),0.000001)*sign(p.w);}
fn rippleNormal(p:vec3<f32>)->vec3<f32>{
 let t=frame.eyeTime.w;var slope=vec2(sin(p.x*19.0+p.z*7.0+t*6.2831853),cos(p.z*21.0-p.x*9.0-t*9.424778))*frame.optics.y;
 for(var i=0u;i<u32(frame.counts.x);i++){
  let source=frame.impacts[i];if abs(p.y-source.y)>0.15{continue;}let delta=p.xz-source.xz;let r=length(delta);
  if r>0.001&&r<1.2{let envelope=exp(-3.0*r)*(1.0-smoothstep(0.8,1.2,r));let phase=r*34.0-t*9.424778;
   slope+=delta/r*(cos(phase)*0.065-sin(phase)*0.006)*envelope*source.w;}
 }return normalize(vec3(-slope.x,1,-slope.y));
}
fn specular(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>)->f32 {
 let h=safeNormal(v+l,n);let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),0.01);let nh=max(dot(n,h),0.0);
 let a=max(0.03,frame.optics.z*frame.optics.z);let a2=a*a;let q=nh*nh*(a2-1.0)+1.0;
 let d=a2/(3.14159265*q*q+0.000001);let visibility=0.5/max(nl*sqrt(nv*nv*(1.0-a2)+a2)+nv*sqrt(nl*nl*(1.0-a2)+a2),0.001);
 return min(20.0,d*visibility*nl);
}
// Bounded screen-space reflection. Opaque color/depth are read-only, with no water feedback.
fn screenReflection(p:vec3<f32>,n:vec3<f32>,ray:vec3<f32>)->vec4<f32>{
 let origin=p+n*0.025;var previous=0.0;var previousGap=-1.0;
 for(var i=1;i<=24;i++){
  let step=f32(i)/24.0;let travel=0.04+step*step*12.0;let point=origin+ray*travel;
  let clip=frame.view*vec4(point,1);if clip.w<=0.0{return vec4(0);}
  let uv=clip.xy/clip.w*vec2(0.5,-0.5)+vec2(0.5);
  if any(uv<vec2(0.002))||any(uv>vec2(0.998)){return vec4(0);}
  let d=depthAt(uv);if d>=0.999999{previous=travel;previousGap=-1.0;continue;}
  let surface=unproject(uv,d);let gap=distance(frame.eyeTime.xyz,point)-distance(frame.eyeTime.xyz,surface);
  if gap>0.0&&previousGap<0.0{
   var lo=previous;var hi=travel;var hitUV=uv;var hit=surface;var error=gap;
   for(var refine=0;refine<5;refine++){
    let middle=(lo+hi)*0.5;let q=origin+ray*middle;let c=frame.view*vec4(q,1);
    hitUV=c.xy/c.w*vec2(0.5,-0.5)+vec2(0.5);let hitDepth=depthAt(hitUV);
    hit=unproject(hitUV,hitDepth);error=distance(frame.eyeTime.xyz,q)-distance(frame.eyeTime.xyz,hit);
    if error>0.0{hi=middle;}else{lo=middle;}
   }
   let q=origin+ray*((lo+hi)*0.5);
   if abs(error)<0.12&&distance(q,hit)<0.18&&dot(hit-p,n)>0.015{
    let edge=min(min(hitUV.x,1.0-hitUV.x),min(hitUV.y,1.0-hitUV.y));
    let confidence=smoothstep(0.005,0.05,edge)*(1.0-smoothstep(9.0,12.0,travel));
    return vec4(textureSampleLevel(sceneColor,colorSampler,hitUV,0.0).rgb,confidence);
   }
   return vec4(0);
  }
  previous=travel;previousGap=gap;
 }return vec4(0);
}
fn waterEnvironmentReflection(reflected:vec3<f32>)->vec3<f32>{
 let d=safeNormal((frame.environmentRotation*vec4(reflected,0)).xyz,vec3(0,1,0));
 let envUV=vec2(fract(atan2(d.x,d.z)/6.283185307),acos(clamp(d.y,-1.0,1.0))/3.141592654);
 let mip=frame.optics.z*frame.environmentSettings.y;
 let solar=textureSampleLevel(environmentMap,environmentSampler,envUV,0,mip).rgb*frame.environmentSettings.z;
 let constant=textureSampleLevel(environmentMap,environmentSampler,envUV,1,mip).rgb*frame.environmentSettings.w;
 return (solar+constant)*frame.environmentGain.rgb;
}
fn shade(p:vec3<f32>,n:vec3<f32>,pixel:vec4<f32>,fallbackDepth:f32,jet:bool)->vec3<f32>{
 let uv0=pixel.xy/frame.viewport.xy;let v=safeNormal(frame.eyeTime.xyz-p,vec3(0,1,0));let d0=depthAt(uv0);
 var thickness=fallbackDepth;var behind=p-n*fallbackDepth;
 if d0<0.999999&&d0>=pixel.z-0.00002{behind=unproject(uv0,d0);if !jet{thickness=max(p.y-behind.y,0.0)/max(abs(v.y),0.25);}}
 thickness=clamp(thickness,0.0,3.0);
 var uv=uv0+n.xz*frame.optics.x*min(thickness,0.5)/max(distance(frame.eyeTime.xyz,p)*0.1,1.0);
 if any(uv<vec2(0.001))||any(uv>vec2(0.999))||depthAt(uv)<pixel.z-0.00002{uv=uv0;}
 let background=textureSampleLevel(sceneColor,colorSampler,uv,0.0).rgb;
 let ior=frame.optics.w;let f0=pow((ior-1.0)/(ior+1.0),2.0);let fresnel=f0+(1.0-f0)*pow(1.0-clamp(dot(n,v),0.0,1.0),5.0);
 let reflected=reflect(-v,n);var reflection=mix(frame.skyHorizon.rgb,frame.skyZenith.rgb,smoothstep(-0.1,0.7,reflected.y));
 if frame.environmentSettings.x>0.5{
  reflection=waterEnvironmentReflection(reflected);
 }
 var planar=vec4(0.0);
 if !jet&&frame.planarSettings.x>0.5&&abs(p.y-frame.planarSettings.y)<=frame.planarSettings.z{
  let projected=frame.planarProjection*vec4(p,1);
  if projected.w>0.0{
   let planarUV=projected.xy/projected.w*vec2(0.5,-0.5)+vec2(0.5)+n.xz*frame.planarSettings.w;
   if all(planarUV>vec2(0.001))&&all(planarUV<vec2(0.999)){
    let edge=min(min(planarUV.x,1.0-planarUV.x),min(planarUV.y,1.0-planarUV.y));
    planar=vec4(textureSampleLevel(planarReflection,colorSampler,planarUV,0.0).rgb,smoothstep(0.001,0.015,edge));
   }
  }
 }
 if !jet&&frame.counts.z>0.5&&planar.a<0.999{let screen=screenReflection(p,n,reflected);reflection=mix(reflection,screen.rgb,screen.a);}
 reflection=mix(reflection,planar.rgb,planar.a);
 reflection+=frame.sunColor.rgb*specular(n,v,safeNormal(frame.sun.xyz,vec3(0,1,0)));
 for(var i=0u;i<u32(frame.counts.y);i++){
  let light=frame.lightPositions[i];let delta=light.xyz-p;let d=length(delta);if d>=light.w{continue;}
  var attenuation=pow(1.0-pow(d/max(light.w,0.001),4.0),2.0)/(d*d+0.0001);
  if frame.lightColors[i].w>=0.0{attenuation=pow(max(0.0,1.0-d*d/(light.w*light.w)),frame.lightColors[i].w);}
  reflection+=frame.lightColors[i].rgb*attenuation*specular(n,v,safeNormal(delta,n));
 }
 let transmitted=exp(-frame.absorption.rgb*thickness)*(1.0-fresnel);
 var added=reflection*fresnel+frame.ambient.rgb*(vec3(1)-exp(-frame.absorption.rgb*thickness))*(1.0-fresnel)*0.1;
 let ray=p-frame.eyeTime.xyz;let len=length(ray);
 let fog=heightFogComposition(frame.eyeTime.y,ray.y/max(len,0.000001),len,frame.fog.w,${FOG_HEIGHT_RATE},1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 added=added*fog.x+frame.fog.rgb*(vec3(1)-transmitted)*fog.y;
 return max(vec3(0),background*transmitted+added);
}
@fragment fn basin(v:Out)->@location(0)vec4<f32>{return vec4(shade(v.p,rippleNormal(v.p),v.clip,v.uv.y,false),1);}
@fragment fn jet(v:Out)->@location(0)vec4<f32>{
 let time=frame.eyeTime.w;let progress=v.tangent.w;
 let flow=sin(v.uv.y*140.0-time*18.849556+v.uv.x*18.849556)*0.25+
  sin(v.uv.y*233.0-time*31.415927-v.uv.x*31.415927)*0.12;
 let longitudinal=safeNormal(cross(v.tangent.xyz,v.n),vec3(0,1,0));
 let crossFlow=cos(v.uv.y*177.0-time*25.132741+v.uv.x*25.132741)*0.3;
 let n=safeNormal(v.n+v.tangent.xyz*flow*(0.3+progress)+longitudinal*crossFlow*(0.2+progress),v.n);
 let color=shade(v.p,n,v.clip,mix(0.035,0.10,progress),true);
 let aeration=smoothstep(0.2,1.0,progress)*pow(max(0.0,flow),2.0)*0.8;
 return vec4(mix(color,frame.ambient.rgb+frame.sunColor.rgb*0.3,aeration),1);
}
@fragment fn splash(v:Out)->@location(0)vec4<f32>{
 let q=dot(v.uv,v.uv);if q>1.0{discard;}
 let n=safeNormal(v.n+vec3(v.uv.x,0,v.uv.y)*0.5,v.n);
 let color=shade(v.p,n,v.clip,0.008,true)+frame.ambient.rgb*0.35+frame.sunColor.rgb*0.12;
 return vec4(color,pow(max(0.0,1.0-q),0.8)*0.4);
}
`;

/** Owns water resources; the host owns frame scheduling and all supplied attachments. */
export class WaterPass {
  private readonly device: GPUDevice;
  private readonly format: GPUTextureFormat;
  private readonly uniform: GPUBuffer;
  private readonly layout: GPUBindGroupLayout;
  private readonly sampler: GPUSampler;
  private readonly environmentSampler: GPUSampler;
  private readonly emptyEnvironment: GPUTexture;
  private readonly basinPipeline: GPURenderPipeline;
  private readonly jetPipeline: GPURenderPipeline;
  private readonly splashPipeline: GPURenderPipeline;
  private readonly values = new Float32Array(FRAME_FLOATS);
  private basinVertices?: GPUBuffer;
  private basinIndices?: GPUBuffer;
  private jetVertices?: GPUBuffer;
  private jetIndices?: GPUBuffer;
  private splashVertices?: GPUBuffer;
  private splashIndices?: GPUBuffer;
  private basinCount = 0;
  private jetCount = 0;
  private splashCount = 0;
  private impacts: WaterImpact[] = [];
  private intermediate?: GPUTexture;
  private size = "";
  private disposed = false;

  private constructor(
    device: GPUDevice,
    format: GPUTextureFormat,
    uniform: GPUBuffer,
    layout: GPUBindGroupLayout,
    sampler: GPUSampler,
    environmentSampler: GPUSampler,
    emptyEnvironment: GPUTexture,
    basin: GPURenderPipeline,
    jet: GPURenderPipeline,
    splash: GPURenderPipeline,
  ) {
    this.device = device;
    this.format = format;
    this.uniform = uniform;
    this.layout = layout;
    this.sampler = sampler;
    this.environmentSampler = environmentSampler;
    this.emptyEnvironment = emptyEnvironment;
    this.basinPipeline = basin;
    this.jetPipeline = jet;
    this.splashPipeline = splash;
  }
  static async create(
    device: GPUDevice,
    options: {
      format?: GPUTextureFormat;
      sampleCount?: 4;
      depthFormat?: GPUTextureFormat;
    } = {},
  ) {
    if ((options.sampleCount ?? 4) !== 4)
      throw new Error("The water pass currently requires 4xMSAA.");
    const format = options.format ?? "rgba16float";
    device.pushErrorScope("validation");
    let uniform: GPUBuffer | undefined,
      emptyEnvironment: GPUTexture | undefined,
      failure: unknown,
      result: WaterPass | undefined;
    try {
      const shader = device.createShaderModule({
        label: "Owned fountain water",
        code: WATER_PASS_SHADER,
      });
      const info = await shader.getCompilationInfo();
      const errors = info.messages.filter((m) => m.type === "error");
      if (errors.length)
        throw new Error(
          errors.map((e) => `Water line ${e.lineNum}: ${e.message}`).join("\n"),
        );
      const layout = device.createBindGroupLayout({
        entries: [
          {
            binding: 0,
            visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
            buffer: { type: "uniform", minBindingSize: FRAME_FLOATS * 4 },
          },
          {
            binding: 1,
            visibility: GPUShaderStage.FRAGMENT,
            texture: { sampleType: "float" },
          },
          {
            binding: 2,
            visibility: GPUShaderStage.FRAGMENT,
            texture: { sampleType: "depth", multisampled: true },
          },
          {
            binding: 3,
            visibility: GPUShaderStage.FRAGMENT,
            sampler: { type: "filtering" },
          },
          {
            binding: 4,
            visibility: GPUShaderStage.FRAGMENT,
            texture: { sampleType: "float", viewDimension: "2d-array" },
          },
          {
            binding: 5,
            visibility: GPUShaderStage.FRAGMENT,
            sampler: { type: "filtering" },
          },
          {
            binding: 6,
            visibility: GPUShaderStage.FRAGMENT,
            texture: { sampleType: "float" },
          },
        ],
      });
      const pipelineLayout = device.createPipelineLayout({
        bindGroupLayouts: [layout],
      });
      const make = (entryPoint: string) =>
        device.createRenderPipelineAsync({
          label: `Water ${entryPoint}`,
          layout: pipelineLayout,
          vertex: {
            module: shader,
            entryPoint:
              entryPoint === "splash"
                ? "vsSplash"
                : entryPoint === "jet"
                  ? "vsJet"
                  : "vs",
            buffers: [LAYOUT],
          },
          fragment: {
            module: shader,
            entryPoint,
            targets: [
              {
                format,
                ...(entryPoint === "splash"
                  ? {
                      blend: {
                        color: {
                          srcFactor: "src-alpha" as const,
                          dstFactor: "one-minus-src-alpha" as const,
                        },
                        alpha: {
                          srcFactor: "one" as const,
                          dstFactor: "one-minus-src-alpha" as const,
                        },
                      },
                    }
                  : {}),
              },
            ],
          },
          primitive: {
            topology: "triangle-list",
            cullMode: entryPoint === "splash" ? "none" : "back",
          },
          depthStencil: {
            format: options.depthFormat ?? "depth24plus",
            depthWriteEnabled: false,
            depthCompare: "less-equal",
          },
          multisample: { count: 4 },
        });
      const [basin, jet, splash] = await Promise.all([
        make("basin"),
        make("jet"),
        make("splash"),
      ]);
      uniform = device.createBuffer({
        label: "Water frame",
        size: FRAME_FLOATS * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      const sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
        addressModeU: "clamp-to-edge",
        addressModeV: "clamp-to-edge",
      });
      const environmentSampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "linear",
        addressModeU: "repeat",
        addressModeV: "clamp-to-edge",
      });
      emptyEnvironment = device.createTexture({
        label: "Empty water environment",
        size: [1, 1, 2],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      device.queue.writeTexture(
        { texture: emptyEnvironment },
        new Uint16Array([0, 0, 0, 0x3c00, 0, 0, 0, 0x3c00]),
        { bytesPerRow: 8, rowsPerImage: 1 },
        [1, 1, 2],
      );
      result = new WaterPass(
        device,
        format,
        uniform,
        layout,
        sampler,
        environmentSampler,
        emptyEnvironment,
        basin,
        jet,
        splash,
      );
    } catch (error) {
      failure = error;
    }
    const error = await device.popErrorScope();
    if (failure || error) {
      uniform?.destroy();
      emptyEnvironment?.destroy();
      throw failure ?? new Error(error!.message);
    }
    return result!;
  }
  get hasContent() {
    return this.basinCount + this.jetCount > 0;
  }
  setSources(sources: WaterManifest) {
    if (this.disposed) throw new Error("This water pass is closed.");
    if (sources?.schema !== "aethyrforge.water.v1")
      throw new Error("Use a supported water manifest.");
    const basins = buildWaterBasins(sources.basins);
    const jets = buildWaterJets(sources.jets);
    const splashes = buildWaterSplashes(jets.impacts);
    const created: GPUBuffer[] = [];
    const upload = (
      data: Float32Array | Uint32Array,
      usage: GPUBufferUsageFlags,
      label: string,
    ) => {
      if (!data.length) return undefined;
      const buffer = this.device.createBuffer({
        label,
        size: data.byteLength,
        usage: usage | GPUBufferUsage.COPY_DST,
      });
      created.push(buffer);
      this.device.queue.writeBuffer(
        buffer,
        0,
        data as Float32Array<ArrayBuffer>,
      );
      return buffer;
    };
    let bv: GPUBuffer | undefined,
      bi: GPUBuffer | undefined,
      jv: GPUBuffer | undefined,
      ji: GPUBuffer | undefined,
      sv: GPUBuffer | undefined,
      si: GPUBuffer | undefined;
    try {
      bv = upload(basins.vertices, GPUBufferUsage.VERTEX, "Basin vertices");
      bi = upload(basins.indices, GPUBufferUsage.INDEX, "Basin indices");
      jv = upload(jets.vertices, GPUBufferUsage.VERTEX, "Jet vertices");
      ji = upload(jets.indices, GPUBufferUsage.INDEX, "Jet indices");
      sv = upload(splashes.vertices, GPUBufferUsage.VERTEX, "Splash vertices");
      si = upload(splashes.indices, GPUBufferUsage.INDEX, "Splash indices");
    } catch (error) {
      for (const buffer of created) buffer.destroy();
      throw error;
    }
    this.basinVertices?.destroy();
    this.basinIndices?.destroy();
    this.jetVertices?.destroy();
    this.jetIndices?.destroy();
    this.splashVertices?.destroy();
    this.splashIndices?.destroy();
    this.basinVertices = bv;
    this.basinIndices = bi;
    this.jetVertices = jv;
    this.jetIndices = ji;
    this.splashVertices = sv;
    this.splashIndices = si;
    this.basinCount = basins.indices.length;
    this.jetCount = jets.indices.length;
    this.splashCount = splashes.indices.length;
    this.impacts = jets.impacts;
  }
  render(encoder: GPUCommandEncoder, frame: WaterPassFrame): boolean {
    if (this.disposed) throw new Error("This water pass is closed.");
    if (!this.hasContent) return false;
    if (
      ![frame.width, frame.height].every(
        (n) =>
          Number.isInteger(n) &&
          n > 0 &&
          n <= this.device.limits.maxTextureDimension2D,
      ) ||
      !Number.isFinite(frame.time) ||
      !frame.eye.every(Number.isFinite) ||
      frame.viewProjection.length !== 16 ||
      frame.inverseViewProjection.length !== 16 ||
      !Array.from(frame.viewProjection).every(Number.isFinite) ||
      !Array.from(frame.inverseViewProjection).every(Number.isFinite)
    )
      throw new Error(
        "Choose a finite water camera and valid viewport dimensions.",
      );
    if (frame.sceneColor === frame.resolveTarget)
      throw new Error("Water needs a separate sampled color and output.");
    const u = this.values;
    u.fill(0);
    u.set(frame.viewProjection, 0);
    u.set(frame.inverseViewProjection, 16);
    u.set([...frame.eye, ((frame.time % 128) + 128) % 128], 32);
    u.set([...(frame.sunDirection ?? [0, 1, 0]), 0], 36);
    u.set([...(frame.sunColor ?? [1, 0.95, 0.85]), 0], 40);
    u.set([...(frame.ambient ?? [0.15, 0.18, 0.22]), 0], 44);
    u.set([...(frame.fog?.color ?? [0, 0, 0]), frame.fog?.density ?? 0], 48);
    u.set([frame.width, frame.height, 0, 0], 52);
    u.set([0.07, 0.017, 0.12, 1.333], 56);
    u.set([0.12, 0.045, 0.02, 0], 60);
    u.set([...(frame.skyZenith ?? [0.25, 0.45, 0.7]), 0], 64);
    u.set([...(frame.skyHorizon ?? [0.65, 0.72, 0.8]), 0], 68);
    this.impacts.forEach((impact, i) =>
      u.set([...impact.position, 1], 72 + i * 4),
    );
    const lights = (frame.lights ?? []).slice(0, 16);
    lights.forEach((light, i) => {
      if (
        ![...light.position, ...light.color, light.range].every(
          Number.isFinite,
        ) ||
        light.range <= 0
      )
        throw new Error(
          "Water lights need finite positions, colors, and a positive range.",
        );
      u.set([...light.position, light.range], 328 + i * 4);
      if (
        light.falloffExponent !== undefined &&
        (!Number.isFinite(light.falloffExponent) ||
          light.falloffExponent < 0 ||
          light.falloffExponent > 128)
      )
        throw new Error("Legacy water light falloff must be from zero to 128.");
      u.set([...light.color, light.falloffExponent ?? -1], 392 + i * 4);
    });
    u.set(
      [
        this.impacts.length,
        lights.length,
        frame.screenSpaceReflections === false ? 0 : 1,
        0,
      ],
      456,
    );
    const environment = frame.environment;
    if (
      environment &&
      (environment.worldToEnvironment.length !== 16 ||
        !Array.from(environment.worldToEnvironment).every(Number.isFinite) ||
        !Number.isInteger(environment.mipMax) ||
        environment.mipMax < 0 ||
        environment.mipMax > 16 ||
        !Number.isFinite(environment.solarRatio ?? 1) ||
        (environment.solarRatio ?? 1) < 0 ||
        (environment.solarRatio ?? 1) > 1e6 ||
        (environment.hasConstantFog !== undefined &&
          typeof environment.hasConstantFog !== "boolean") ||
        !environment.gain.every((n) => Number.isFinite(n) && n >= 0))
    )
      throw new Error(
        "Water environment maps need a finite transform, nonnegative gain, and valid mip count.",
      );
    u.set(environment?.worldToEnvironment ?? IDENTITY, 460);
    u.set(
      [
        environment ? 1 : 0,
        environment?.mipMax ?? 0,
        environment?.solarRatio ?? 1,
        Number(!!environment?.hasConstantFog),
      ],
      476,
    );
    u.set([...(environment?.gain ?? [0, 0, 0]), 0], 480);
    const planar = frame.planar;
    if (
      planar &&
      (planar.viewProjection.length !== 16 ||
        !Array.from(planar.viewProjection).every(Number.isFinite) ||
        !Number.isFinite(planar.planeY) ||
        !Number.isFinite(planar.planeTolerance ?? PLANAR_WATER_TOLERANCE) ||
        (planar.planeTolerance ?? PLANAR_WATER_TOLERANCE) <= 0 ||
        (planar.planeTolerance ?? PLANAR_WATER_TOLERANCE) > 0.25 ||
        !Number.isFinite(planar.distortion ?? PLANAR_WATER_DISTORTION) ||
        (planar.distortion ?? PLANAR_WATER_DISTORTION) < 0 ||
        (planar.distortion ?? PLANAR_WATER_DISTORTION) > 0.1)
    )
      throw new Error(
        "Planar water reflections need a finite camera, plane, tolerance, and bounded distortion.",
      );
    if (
      planar &&
      [frame.colorView, frame.resolveTarget, frame.sceneColor].includes(
        planar.view,
      )
    )
      throw new Error(
        "Planar reflections must use a separate texture rendered without water.",
      );
    u.set(planar?.viewProjection ?? IDENTITY, 484);
    u.set(
      [
        planar ? 1 : 0,
        planar?.planeY ?? 0,
        planar?.planeTolerance ?? PLANAR_WATER_TOLERANCE,
        planar?.distortion ?? PLANAR_WATER_DISTORTION,
      ],
      500,
    );
    const volumeMedium = frame.fog?.volumeMedium ?? ([0, 0, 1, 0] as const);
    const volumeRange = frame.fog?.volumeRange ?? ([0, 0] as const);
    heightFogComposition(
      frame.eye[1],
      0,
      0,
      frame.fog?.density ?? 0,
      FOG_HEIGHT_RATE,
      1,
      0,
      volumeMedium,
      volumeRange,
    );
    u.set(volumeMedium, 504);
    u.set([...volumeRange, 0, 0], 508);
    if (!u.every(Number.isFinite))
      throw new Error("Keep water lighting parameters finite.");
    this.device.queue.writeBuffer(this.uniform, 0, u);
    const group = (color: GPUTextureView) =>
      this.device.createBindGroup({
        layout: this.layout,
        entries: [
          { binding: 0, resource: { buffer: this.uniform } },
          { binding: 1, resource: color },
          { binding: 2, resource: frame.depthView },
          { binding: 3, resource: this.sampler },
          {
            binding: 4,
            resource:
              environment?.view ??
              this.emptyEnvironment.createView({ dimension: "2d-array" }),
          },
          { binding: 5, resource: this.environmentSampler },
          {
            binding: 6,
            resource:
              planar?.view ??
              this.emptyEnvironment.createView({
                dimension: "2d",
                baseArrayLayer: 0,
                arrayLayerCount: 1,
              }),
          },
        ],
      });
    const draw = (
      pipeline: GPURenderPipeline,
      color: GPUTextureView,
      target: GPUTextureView,
      vertices: GPUBuffer,
      indices: GPUBuffer,
      count: number,
    ) => {
      const pass = encoder.beginRenderPass({
        label: "Fountain water",
        colorAttachments: [
          {
            view: frame.colorView,
            resolveTarget: target,
            loadOp: "load",
            storeOp: "store",
          },
        ],
        depthStencilAttachment: { view: frame.depthView, depthReadOnly: true },
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group(color));
      pass.setVertexBuffer(0, vertices);
      pass.setIndexBuffer(indices, "uint32");
      pass.drawIndexed(count);
      if (pipeline === this.jetPipeline && this.splashCount) {
        pass.setPipeline(this.splashPipeline);
        pass.setVertexBuffer(0, this.splashVertices!);
        pass.setIndexBuffer(this.splashIndices!, "uint32");
        pass.drawIndexed(this.splashCount);
      }
      pass.end();
    };
    let jetColor = frame.sceneColor;
    if (this.basinCount) {
      let target = frame.resolveTarget;
      if (this.jetCount) {
        const size = `${frame.width}:${frame.height}`;
        if (this.size !== size) {
          this.intermediate?.destroy();
          this.intermediate = this.device.createTexture({
            label: "Basin color for jet refraction",
            size: [frame.width, frame.height],
            format: this.format,
            usage:
              GPUTextureUsage.RENDER_ATTACHMENT |
              GPUTextureUsage.TEXTURE_BINDING,
          });
          this.size = size;
        }
        target = this.intermediate!.createView();
        jetColor = target;
      }
      draw(
        this.basinPipeline,
        frame.sceneColor,
        target,
        this.basinVertices!,
        this.basinIndices!,
        this.basinCount,
      );
    }
    if (this.jetCount)
      draw(
        this.jetPipeline,
        jetColor,
        frame.resolveTarget,
        this.jetVertices!,
        this.jetIndices!,
        this.jetCount,
      );
    return true;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.uniform.destroy();
    this.emptyEnvironment.destroy();
    this.basinVertices?.destroy();
    this.basinIndices?.destroy();
    this.jetVertices?.destroy();
    this.jetIndices?.destroy();
    this.splashVertices?.destroy();
    this.splashIndices?.destroy();
    this.intermediate?.destroy();
  }
}
