import {
  buildClusteredLights,
  CLUSTER_HEADER,
  type ClusteredLightInput,
  type ClusteredLights,
} from "./clustered-lights.ts";
import { WGSL_FOLIAGE_TRANSMISSION } from "./foliage-transmission.ts";

/** Owned CPU metadata; GPU upload storage can be borrowed from a reusable core. */
export type ClusteredLightSummary = Omit<
  ClusteredLights,
  "storage" | "lightFloats" | "cells" | "indices"
>;

/** Separate main/reflection buffers keep view-dependent lists independent. */
export class ClusteredLighting {
  private observing = true;
  setObserving(enabled: boolean) {
    this.observing = enabled;
  }
  private readonly views = new Map<
    string,
    {
      buffer: GPUBuffer;
      signature?: string;
      data?: ClusteredLightSummary;
      lights?: ClusteredLightInput["lights"];
    }
  >();
  constructor(
    private readonly device: GPUDevice,
    private readonly builder: (
      input: ClusteredLightInput,
      revision?: string | number,
    ) => ClusteredLights = buildClusteredLights,
    private readonly observe?: (milliseconds: number) => void,
  ) {}
  buffer(view: string): GPUBuffer {
    let item = this.views.get(view);
    if (!item) {
      item = {
        buffer: this.device.createBuffer({
          label: view + " clustered lights",
          size: 256,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        }),
      };
      this.views.set(view, item);
    }
    return item.buffer;
  }
  /** Revision must include source light edits; shadow indices and camera are checked directly. */
  update(
    view: string,
    input: ClusteredLightInput,
    revision: string | number,
  ): {
    buffer: GPUBuffer;
    bindingChanged: boolean;
    data: ClusteredLightSummary;
  } {
    this.buffer(view);
    const item = this.views.get(view)!;
    const signature = JSON.stringify([
      revision,
      Array.from(input.viewProjection),
      Array.from(input.eye),
      Array.from(input.forward),
      input.width,
      input.height,
      input.tileSize,
      input.depthSlices,
      input.near,
      input.far,
      input.maxIndices,
      [...(input.shadowIndices ?? [])],
    ]);
    if (item.signature === signature && item.lights === input.lights)
      return { buffer: item.buffer, bindingChanged: false, data: item.data! };
    item.signature = undefined;
    const observe = this.observing ? this.observe : undefined;
    const started = observe ? performance.now() : 0;
    const data = this.builder(input, revision);
    observe?.(performance.now() - started);
    const previous = item.buffer;
    let buffer = previous;
    const bindingChanged = previous.size < data.storage.byteLength;
    if (bindingChanged) {
      const size = 2 ** Math.ceil(Math.log2(data.storage.byteLength));
      if (size > this.device.limits.maxStorageBufferBindingSize)
        throw new Error(
          "This lighting view exceeds the graphics device's storage limit.",
        );
      buffer = this.device.createBuffer({
        label: view + " clustered lights",
        size,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
    }
    try {
      this.device.queue.writeBuffer(buffer, 0, data.storage);
    } catch (error) {
      if (bindingChanged) buffer.destroy();
      throw error;
    }
    item.buffer = buffer;
    if (bindingChanged) previous.destroy();
    item.signature = signature;
    item.lights = input.lights;
    // writeBuffer copies immediately. Do not cache Wasm views that another view or
    // memory growth can overwrite or detach before this signature is used again.
    const {
      sourceIndices,
      lightIds,
      viewProjection,
      eye,
      forward,
      grid,
      stats,
    } = data;
    item.data = {
      sourceIndices,
      lightIds,
      viewProjection,
      eye,
      forward,
      grid,
      stats,
    };
    return { buffer: item.buffer, bindingChanged, data: item.data };
  }
  stats(view: string) {
    const stats = this.views.get(view)?.data?.stats;
    return stats ? { ...stats } : null;
  }
  dispose() {
    for (const item of this.views.values()) item.buffer.destroy();
    this.views.clear();
  }
}

const H = CLUSTER_HEADER;
/** Keep complete original and prepared paths without rewriting shader expressions. */
export function buildClusteredLightingWGSL(
  preparedSurfaceBrdf = false,
): string {
  const prepare = preparedSurfaceBrdf
    ? "if iterations==0u{return color;}\n let bs=brdfTerms(n,view,base,rough,metal,dielectric);"
    : "";
  const surface = preparedSurfaceBrdf
    ? "brdfLight(n,view,l,base,metal,bs)"
    : "brdf(n,view,l,base,rough,metal,dielectric)";
  return `
${WGSL_FOLIAGE_TRANSMISSION}
@group(0)@binding(13)var<storage,read> clusteredLights:array<u32>;
fn clusterFloat(word:u32)->f32{return bitcast<f32>(clusteredLights[word]);}
fn clusterVector(word:u32)->vec4<f32>{return vec4(clusterFloat(word),clusterFloat(word+1u),clusterFloat(word+2u),clusterFloat(word+3u));}
fn clusteredSurface(pixel:vec2<f32>,world:vec3<f32>,n:vec3<f32>,view:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,dielectric:f32,subsurface:vec3<f32>,geometric:vec3<f32>)->vec3<f32>{
 return clusteredSurfaceTransmission(pixel,world,n,view,base,rough,metal,dielectric,subsurface,geometric,0.0);
}
fn clusteredSurfaceTransmission(pixel:vec2<f32>,world:vec3<f32>,n:vec3<f32>,view:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,dielectric:f32,subsurface:vec3<f32>,geometric:vec3<f32>,transmissionProfile:f32)->vec3<f32>{
 let total=clusteredLights[${H.lightCount}];if total==0u{return vec3(0.0);}
 let tilesX=clusteredLights[${H.tilesX}];let tilesY=clusteredLights[${H.tilesY}];let slices=clusteredLights[${H.depthSlices}];let tile=f32(clusteredLights[${H.tileSize}]);
 let near=clusterFloat(${H.near}u);let depthScale=clusterFloat(${H.logDepthScale}u);
 let forward=vec3(clusterFloat(${H.forwardX}u),clusterFloat(${H.forwardY}u),clusterFloat(${H.forwardZ}u));
 let depth=max(dot(world-frame.eye.xyz,forward),near);let slice=u32(clamp(floor(log(depth/near)*depthScale),0.0,f32(slices-1u)));
 let xy=vec2<u32>(clamp(floor(pixel/tile),vec2(0.0),vec2(f32(tilesX-1u),f32(tilesY-1u))));
 let cell=(slice*tilesY+xy.y)*tilesX+xy.x;let address=clusteredLights[${H.cellsOffset}]+cell*2u;
 let start=clusteredLights[address];let count=clusteredLights[address+1u];let overflow=count==0xffffffffu;
 let iterations=select(count,total,overflow);var color=vec3(0.0);
 ${prepare}
 for(var entry=0u;entry<iterations;entry++){
  var index=entry;if !overflow{index=clusteredLights[clusteredLights[${H.indicesOffset}]+start+entry];}
  let record=clusteredLights[${H.lightsOffset}]+index*16u;
  let position=clusterVector(record);
  let delta=position.xyz-world;let dist2=dot(delta,delta);
  // Keep a wide rounding margin and the original test at the influence boundary.
  // Tiny ranges retain that test because implementations may flush subnormals.
  let rangeSquared=position.w*position.w;
  if rangeSquared>=1e-30&&dist2>rangeSquared*1.00001{continue;}
  let distance=sqrt(dist2);if distance>=position.w{continue;}
  let emission=clusterVector(record+4u);let direction=clusterVector(record+8u);let cone=clusterVector(record+12u);
  let l=delta/max(distance,0.001);var attenuation=pow(max(1.0-pow(distance/position.w,4.0),0.0),2.0)/(dist2+0.0001);
  if cone.w>=0.0{attenuation=pow(max(1.0-dist2/(position.w*position.w),0.0),max(cone.w,0.0001));}
  if direction.w>0.5{
   let outer=min(cone.x,cone.y);let inner=max(cone.x,cone.y);let angle=dot(-l,direction.xyz);var visibility=step(outer,angle);
   if inner-outer>0.00001{let ramp=clamp((angle-outer)/(inner-outer),0.0,1.0);visibility=ramp*ramp;}attenuation*=visibility;
  }
  if attenuation>0.0&&(dot(n,l)>0.0||dot(subsurface,subsurface)>0.0){
   var visibility=1.0;if cone.z>=0.0{visibility=localVisibility(u32(cone.z),world,geometric);}
   color+=(${surface}+foliageTransmission(subsurface,n,l,view,transmissionProfile))*emission.rgb*emission.w*attenuation*visibility;
  }
 }
 return color;
}
`;
}

/** Same falloff and independently budgeted shadows as the source path. */
export const WGSL_CLUSTERED_LIGHTING = buildClusteredLightingWGSL();
