import type { SourceLight } from "../assets/src/scene.ts";
import { CLUSTER_HEADER, CLUSTER_LIMITS } from "./clustered-lights.ts";
import { COMMON } from "./shaders.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_START_DISTANCE,
  segmentFogOpticalDepth,
} from "./fog.ts";

export interface LocalFogProfile {
  enabled: boolean;
  /** Natural-exponential volume extinction at referenceHeight, in inverse meters. */
  extinctionAtReference: number;
  referenceHeight: number;
  heightRate: number;
  albedo: number;
  phaseG: number;
  startDistance: number;
  maxDistance: number;
}
export const LOCAL_FOG_LIMITS = Object.freeze({
  maxWidth: 640,
  maxHeight: 360,
  steps: 24,
  minSteps: 16,
  maxSteps: 32,
  maxDistance: 1000,
});
const inactive: LocalFogProfile = {
  enabled: false,
  extinctionAtReference: 0,
  referenceHeight: 1,
  heightRate: 0,
  albedo: 0,
  phaseG: 0,
  startDistance: 0,
  maxDistance: 60,
};
function finite(value: number, min: number, max: number, name: string) {
  if (!Number.isFinite(value) || value < min || value > max)
    throw new Error(`Use ${name} between ${min} and ${max}.`);
  return value;
}
function profileChecked(profile: LocalFogProfile) {
  if (typeof profile.enabled !== "boolean")
    throw new Error("Use an enabled flag for local fog.");
  finite(profile.extinctionAtReference, 0, 10000, "fog extinction");
  finite(profile.referenceHeight, -20000, 20000, "fog reference height");
  finite(profile.heightRate, 0, 10, "fog height rate");
  finite(profile.albedo, 0, 1, "fog albedo");
  finite(profile.phaseG, -0.95, 0.95, "fog phase distribution");
  finite(profile.startDistance, 0, LOCAL_FOG_LIMITS.maxDistance, "fog start");
  finite(profile.maxDistance, 0, LOCAL_FOG_LIMITS.maxDistance, "fog distance");
  if (profile.startDistance > profile.maxDistance)
    throw new Error("Start local fog before its maximum distance.");
  return profile;
}

/** Reads only recovered exponential-height-volume properties, without substituting a warm tint. */
export function sourceLocalFogProfile(
  environment: readonly Record<string, unknown>[],
): LocalFogProfile {
  for (const actor of environment) {
    const components = actor.components;
    if (!Array.isArray(components)) continue;
    for (const component of components) {
      if (component?.class !== "ExponentialHeightFogComponent") continue;
      const p = component.properties as Record<string, unknown> | undefined;
      if (!p) continue;
      const number = (key: string, fallback: number) =>
        typeof p[key] === "number" ? (p[key] as number) : fallback;
      const transform = (component.transform ?? actor.transform) as
        { location_cm?: number[] } | undefined;
      const sourceHeight = transform?.location_cm?.[2] ?? 0;
      const density = number("fog_density", 0),
        heightRate = (number("fog_height_falloff", 0) / 10) * Math.LN2;
      let albedo = 1;
      if (typeof p.volumetric_fog_albedo === "string") {
        const channels = ["R", "G", "B"].map((name) => {
          const match = (p.volumetric_fog_albedo as string).match(
            new RegExp(`(?:\\(|,)\\s*${name}=([0-9.]+)`),
          );
          if (!match)
            throw new Error("Prepare the source fog albedo channels.");
          const c = finite(Number(match[1]), 0, 255, "fog color channel") / 255;
          return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
        });
        if (Math.max(...channels) - Math.min(...channels) > 1e-5)
          throw new Error("Colored volume albedo needs an RGB fog profile.");
        albedo = channels[0];
      }
      return profileChecked({
        enabled:
          p.enable_volumetric_fog === true &&
          p.visible !== false &&
          p.hidden_in_game !== true &&
          (actor.properties as Record<string, unknown> | undefined)?.hidden !==
            true,
        // UE volume density conversion uses 0.5 in centimeters; positions here are meters.
        extinctionAtReference:
          (density / 10) *
          0.5 *
          number("volumetric_fog_extinction_scale", 1) *
          Math.exp(-heightRate * (1 - sourceHeight / 100)),
        referenceHeight: 1,
        heightRate,
        albedo,
        phaseG: number("volumetric_fog_scattering_distribution", 0),
        startDistance: number("volumetric_fog_start_distance", 0) / 100,
        maxDistance: number("volumetric_fog_distance", 0) / 100,
      });
    }
  }
  return { ...inactive };
}

/** Segment optical depth, saturated at 80, through the enabled volume's range. */
export function localFogOpticalDepth(
  profile: LocalFogProfile,
  eyeHeight: number,
  directionY: number,
  distance: number,
) {
  profileChecked(profile);
  finite(eyeHeight, -20000, 20000, "eye height");
  finite(directionY, -1, 1, "ray height direction");
  finite(distance, 0, 100000, "ray distance");
  const start = Math.min(profile.startDistance, distance);
  return segmentFogOpticalDepth(
    eyeHeight,
    directionY,
    start,
    Math.max(start, Math.min(distance, profile.maxDistance)),
    profile.extinctionAtReference,
    profile.heightRate,
    profile.referenceHeight,
  );
}
export function localFogPhase(cosine: number, g: number) {
  finite(cosine, -1, 1, "phase angle cosine");
  finite(g, -0.95, 0.95, "phase distribution");
  return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * cosine) ** 1.5);
}

export interface LocalFogFrame {
  width: number;
  height: number;
  /** Resolved HDR after transparent surfaces/water, never this pass's own output. */
  sceneColor: GPUTextureView;
  depthView: GPUTextureView;
  frameBuffer: GPUBuffer;
  clusterBuffer: GPUBuffer;
  localShadowView: GPUTextureView;
  shadowSampler: GPUSampler;
  profile: LocalFogProfile;
  /** Reuse these views until their underlying shadow textures are recreated. */
  directional?: {
    scatteringIntensity: number;
    nearShadowView: GPUTextureView;
    farShadowView: GPUTextureView;
    distantShadowView?: GPUTextureView;
  };
  /** Replaces volume extinction at the reference height for live density edits. */
  extinctionAtReference?: number;
  steps?: number;
  /** Incident sky radiance, including source gain once; no medium terms. */
  ambient?: { view: GPUTextureView; sampler: GPUSampler; enabled: boolean };
}

/**
 * Adds local/directional single scattering to an already attenuated surface.
 * MATERIAL and SKY apply heightFogComposition once: the source-volume medium
 * replaces analytic fill/extinction within its range. This pass uses the same
 * volume extinction inside its light integral, including attenuation through any
 * analytic segment before a nonzero volume start. It does not attenuate the
 * surface again. An optional occluded SH9 sky field supplies single scattering;
 * multiple scattering remains absent.
 */
export class LocalFog {
  private output?: GPUTexture;
  private low?: GPUTexture;
  private outputView?: GPUTextureView;
  private lowView?: GPUTextureView;
  private width = 0;
  private height = 0;
  private lowWidth = 0;
  private lowHeight = 0;
  private readonly uniforms: GPUBuffer;
  private scattering: GPUBuffer;
  private scatteringCount = 0;
  private scatteringKey?: string | number;
  private disposed = false;
  private readonly inactiveSunShadow: GPUTexture;
  private readonly inactiveSunShadowView: GPUTextureView;
  private inactiveAmbient?: GPUTexture;
  private inactiveAmbientView?: GPUTextureView;
  private inactiveAmbientSampler?: GPUSampler;
  private readonly groups = new Map<
    string,
    { resources: readonly unknown[]; group: GPUBindGroup }
  >();
  private constructor(
    private readonly device: GPUDevice,
    private readonly integrate: GPURenderPipeline,
    private readonly composite: GPURenderPipeline,
  ) {
    this.inactiveSunShadow = device.createTexture({
      label: "Inactive fog directional shadow",
      size: [1, 1],
      format: "depth32float",
      usage: GPUTextureUsage.TEXTURE_BINDING,
    });
    this.inactiveSunShadowView = this.inactiveSunShadow.createView();
    this.uniforms = device.createBuffer({
      label: "Local fog medium",
      size: 64,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.scattering = device.createBuffer({
      label: "Local fog source strengths",
      size: 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
  }
  static async create(device: GPUDevice) {
    const module = device.createShaderModule({ code: LOCAL_FOG_WGSL });
    const [integrate, composite] = await Promise.all(
      ["integrateFS", "compositeFS"].map((entryPoint) =>
        device.createRenderPipelineAsync({
          label: "Local light fog " + entryPoint,
          layout: "auto",
          vertex: { module, entryPoint: "fogVS" },
          fragment: {
            module,
            entryPoint,
            targets: [{ format: "rgba16float" }],
          },
        }),
      ),
    );
    return new LocalFog(device, integrate, composite);
  }
  get view() {
    if (!this.outputView)
      throw new Error("Resize local fog before presenting it.");
    return this.outputView;
  }
  resize(width: number, height: number) {
    if (this.disposed) throw new Error("Open local fog before resizing it.");
    finite(width, 1, 16384, "fog image width");
    finite(height, 1, 16384, "fog image height");
    if (!Number.isInteger(width) || !Number.isInteger(height))
      throw new Error("Use whole fog image dimensions.");
    if (this.width === width && this.height === height) return;
    this.output?.destroy();
    this.low?.destroy();
    this.groups.delete("composite-input");
    this.width = width;
    this.height = height;
    const scale = Math.min(
      0.5,
      LOCAL_FOG_LIMITS.maxWidth / width,
      LOCAL_FOG_LIMITS.maxHeight / height,
    );
    this.lowWidth = Math.max(1, Math.ceil(width * scale));
    this.lowHeight = Math.max(1, Math.ceil(height * scale));
    const texture = (label: string, size: number[]) =>
      this.device.createTexture({
        label,
        size,
        format: "rgba16float",
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.RENDER_ATTACHMENT |
          GPUTextureUsage.COPY_SRC,
      });
    this.output = texture("Local fog HDR composite", [width, height]);
    this.low = texture("Local fog bounded single scattering", [
      this.lowWidth,
      this.lowHeight,
    ]);
    this.outputView = this.output.createView();
    this.lowView = this.low.createView();
  }
  /** sourceIndices is the cluster builder's exact record-to-source mapping. */
  setLightScattering(
    lights: readonly SourceLight[],
    sourceIndices: ArrayLike<number>,
    revision: string | number,
  ) {
    if (this.disposed)
      throw new Error("Open local fog before changing lights.");
    if (this.scatteringKey === revision) return;
    if (sourceIndices.length > CLUSTER_LIMITS.lights)
      throw new Error("The local fog light list exceeds its limit.");
    const data = new Float32Array(Math.max(4, sourceIndices.length));
    for (let i = 0; i < sourceIndices.length; i++) {
      const index = sourceIndices[i];
      if (!Number.isInteger(index) || index < 0 || index >= lights.length)
        throw new Error(
          "Match fog strengths to the clustered source light list.",
        );
      const raw = lights[index].raw.volumetric_scattering_intensity;
      data[i] = finite(
        typeof raw === "number" ? raw : 1,
        0,
        65536,
        "light fog strength",
      );
    }
    if (this.scattering.size < data.byteLength) {
      this.scattering.destroy();
      this.scattering = this.device.createBuffer({
        label: "Local fog source strengths",
        size: data.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
    }
    this.device.queue.writeBuffer(this.scattering, 0, data);
    this.scatteringCount = sourceIndices.length;
    this.scatteringKey = revision;
  }
  private bindGroup(
    name: string,
    pipeline: GPURenderPipeline,
    index: number,
    resources: readonly unknown[],
    entries: GPUBindGroupEntry[],
  ) {
    const existing = this.groups.get(name);
    if (
      existing &&
      existing.resources.length === resources.length &&
      existing.resources.every((value, i) => value === resources[i])
    )
      return existing.group;
    const group = this.device.createBindGroup({
      layout: pipeline.getBindGroupLayout(index),
      entries,
    });
    this.groups.set(name, { resources, group });
    return group;
  }
  /** Retain input texture-view objects until resize/recreation to reuse all four bindings. */
  render(encoder: GPUCommandEncoder, frame: LocalFogFrame) {
    if (this.disposed) throw new Error("Open local fog before rendering it.");
    this.resize(frame.width, frame.height);
    if (frame.sceneColor === this.outputView)
      throw new Error("Use the scene color from before the fog pass.");
    const profile = profileChecked(frame.profile),
      density = finite(
        frame.extinctionAtReference ?? profile.extinctionAtReference,
        0,
        10000,
        "fog extinction",
      ),
      steps = frame.steps ?? LOCAL_FOG_LIMITS.steps;
    finite(
      steps,
      LOCAL_FOG_LIMITS.minSteps,
      LOCAL_FOG_LIMITS.maxSteps,
      "fog steps",
    );
    if (!Number.isInteger(steps))
      throw new Error("Use whole fog integration steps.");
    const values = new Float32Array([
      this.width,
      this.height,
      this.lowWidth,
      this.lowHeight,
      density,
      profile.heightRate,
      profile.referenceHeight,
      profile.albedo,
      profile.maxDistance,
      profile.phaseG,
      profile.startDistance,
      steps,
      Number(profile.enabled),
      this.scatteringCount,
      finite(
        frame.directional?.scatteringIntensity ?? 0,
        0,
        65536,
        "directional fog strength",
      ),
      Number(frame.ambient?.enabled ?? false),
    ]);
    this.device.queue.writeBuffer(this.uniforms, 0, values);
    const common = { binding: 0, resource: { buffer: frame.frameBuffer } };
    const integrationFrame = this.bindGroup(
      "integration-frame",
      this.integrate,
      0,
      [frame.frameBuffer],
      [common],
    );
    const nearShadow =
        frame.directional?.nearShadowView ?? this.inactiveSunShadowView,
      farShadow =
        frame.directional?.farShadowView ?? this.inactiveSunShadowView,
      distantShadow =
        frame.directional?.distantShadowView ?? this.inactiveSunShadowView;
    if (!this.inactiveAmbient) {
      this.inactiveAmbient = this.device.createTexture({
        label: "Inactive ambient fog",
        dimension: "3d",
        size: [1, 1, 1],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING,
      });
      this.inactiveAmbientView = this.inactiveAmbient.createView({
        dimension: "3d",
      });
      this.inactiveAmbientSampler = this.device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
      });
    }
    const ambientView = frame.ambient?.view ?? this.inactiveAmbientView!,
      ambientSampler = frame.ambient?.sampler ?? this.inactiveAmbientSampler!;
    const integration = this.bindGroup(
      "integration-input",
      this.integrate,
      1,
      [
        frame.depthView,
        frame.clusterBuffer,
        this.scattering,
        frame.localShadowView,
        frame.shadowSampler,
        nearShadow,
        farShadow,
        distantShadow,
        ambientView,
        ambientSampler,
      ],
      [
        { binding: 0, resource: { buffer: this.uniforms } },
        { binding: 1, resource: frame.depthView },
        { binding: 2, resource: { buffer: frame.clusterBuffer } },
        { binding: 3, resource: { buffer: this.scattering } },
        { binding: 4, resource: frame.localShadowView },
        { binding: 5, resource: frame.shadowSampler },
        { binding: 8, resource: nearShadow },
        { binding: 9, resource: farShadow },
        { binding: 10, resource: distantShadow },
        { binding: 11, resource: ambientView },
        { binding: 12, resource: ambientSampler },
      ],
    );
    const pass = encoder.beginRenderPass({
      label: "Integrate local and directional fog",
      colorAttachments: [
        {
          view: this.lowView!,
          loadOp: "clear",
          storeOp: "store",
          clearValue: [0, 0, 0, 0],
        },
      ],
    });
    pass.setPipeline(this.integrate);
    pass.setBindGroup(0, integrationFrame);
    pass.setBindGroup(1, integration);
    pass.draw(3);
    pass.end();
    const compositeFrame = this.bindGroup(
      "composite-frame",
      this.composite,
      0,
      [frame.frameBuffer],
      [common],
    );
    const composite = this.bindGroup(
      "composite-input",
      this.composite,
      1,
      [frame.depthView, frame.sceneColor, this.lowView],
      [
        { binding: 0, resource: { buffer: this.uniforms } },
        { binding: 1, resource: frame.depthView },
        { binding: 6, resource: frame.sceneColor },
        { binding: 7, resource: this.lowView! },
      ],
    );
    const output = encoder.beginRenderPass({
      label: "Composite volumetric fog",
      colorAttachments: [
        {
          view: this.view,
          loadOp: "clear",
          storeOp: "store",
          clearValue: [0, 0, 0, 1],
        },
      ],
    });
    output.setPipeline(this.composite);
    output.setBindGroup(0, compositeFrame);
    output.setBindGroup(1, composite);
    output.draw(3);
    output.end();
    return this.view;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.output?.destroy();
    this.low?.destroy();
    this.uniforms.destroy();
    this.scattering.destroy();
    this.inactiveSunShadow.destroy();
    this.inactiveAmbient?.destroy();
    this.groups.clear();
  }
}

const H = CLUSTER_HEADER;
export const LOCAL_FOG_WGSL =
  COMMON +
  `
struct FogSettings {size:vec4<f32>,medium:vec4<f32>,volume:vec4<f32>,flags:vec4<f32>};
@group(1)@binding(0)var<uniform> fog:FogSettings;
@group(1)@binding(1)var cameraDepth:texture_depth_multisampled_2d;
@group(1)@binding(2)var<storage,read> clusters:array<u32>;
@group(1)@binding(3)var<storage,read> scattering:array<f32>;
@group(1)@binding(4)var localDepth:texture_depth_2d_array;
@group(1)@binding(5)var fogShadowSampler:sampler_comparison;
@group(1)@binding(6)var inputColor:texture_2d<f32>;
@group(1)@binding(7)var lowFog:texture_2d<f32>;
@group(1)@binding(8)var fogSunNear:texture_depth_2d;
@group(1)@binding(9)var fogSunFar:texture_depth_2d;
@group(1)@binding(10)var fogSunDistant:texture_depth_2d;
@group(1)@binding(11)var fogAmbient:texture_3d<f32>;
@group(1)@binding(12)var fogAmbientSampler:sampler;
@vertex fn fogVS(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{let q=vec2(f32((i<<1u)&2u),f32(i&2u));return vec4(q*2.0-1.0,0.0,1.0);}
fn fogDepth(pixel:vec2<i32>)->f32{let p=clamp(pixel,vec2(0),vec2<i32>(fog.size.xy)-vec2(1));var z=1.0;for(var i=0;i<4;i++){z=min(z,textureLoad(cameraDepth,p,i));}return z;}
fn fogWorld(pixel:vec2<f32>,z:f32)->vec3<f32>{let uv=pixel/fog.size.xy;let q=frame.inverseView*vec4(uv*vec2(2.0,-2.0)+vec2(-1.0,1.0),z,1.0);return q.xyz/q.w;}
fn fogEndpoint(pixel:vec2<f32>)->f32{return min(length(fogWorld(pixel,fogDepth(vec2<i32>(pixel)))-frame.eye.xyz),fog.volume.x);}
fn fogDensity(p:vec3<f32>,logDensity:f32)->f32{return exp(clamp(logDensity-fog.medium.y*(p.y-fog.medium.z),-80.0,11.08214255));}
fn fogStepTau(ray:vec3<f32>,distance:f32,logEyeDensity:f32)->f32{
 let travel=max(distance-fog.volume.z,0.0);if travel<=0.0{return 0.0;}
 let k=fog.medium.y*ray.y*travel;var logAverage=0.0;
 if abs(k)<0.001{logAverage=log(1.0-k*0.5+k*k/6.0-k*k*k/24.0);}
 else if k < -20.0{logAverage=-k-log(-k);}
 else if k > 20.0{logAverage=-log(k);}
 else{logAverage=log((1.0-exp(-k))/k);}
 let logTau=logEyeDensity+log(travel)+logAverage;
 if logTau>=4.382026635{return 80.0;}return exp(max(-80.0,logTau));
}
fn fogPreVolumeTransmittance(ray:vec3<f32>)->f32{
 let start=min(${FOG_START_DISTANCE.toFixed(1)},fog.volume.z);
 return exp(-segmentFogOpticalDepth(frame.eye.y,ray.y,start,fog.volume.z,frame.fog.w,${FOG_HEIGHT_RATE},${FOG_REFERENCE_HEIGHT.toFixed(1)}));
}
fn fogPhase(c:f32)->f32{let g=fog.volume.y;let d=max(1.0+g*g-2.0*g*c,0.0025);return (1.0-g*g)/(12.5663706144*d*sqrt(d));}
fn fogFloat(i:u32)->f32{return bitcast<f32>(clusters[i]);}
fn fogVector(i:u32)->vec4<f32>{return vec4(fogFloat(i),fogFloat(i+1u),fogFloat(i+2u),fogFloat(i+3u));}
fn fogVisibility(index:u32,p:vec3<f32>)->f32{
 if index>=32u{return 1.0;}let assigned=frame.lightCones[index].z;if assigned<0.0{return 1.0;}var layer=u32(assigned);let fromLight=p-frame.lightPositions[index].xyz;
 if frame.lightDirections[index].w<0.5{let a=abs(fromLight);if a.x>=a.y&&a.x>=a.z{layer+=select(1u,0u,fromLight.x>=0.0);}else if a.y>=a.z{layer+=select(3u,2u,fromLight.y>=0.0);}else{layer+=select(5u,4u,fromLight.z>=0.0);}}
 let q=frame.localShadows[layer]*vec4(p,1.0);if q.w<=0.0{return 1.0;}let ndc=q.xyz/q.w;let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);if any(uv<vec2(0.0))||any(uv>vec2(1.0))||ndc.z<0.0||ndc.z>1.0{return 1.0;}
 return textureSampleCompareLevel(localDepth,fogShadowSampler,uv,i32(layer),ndc.z-0.00005);
}
fn fogSunPcf(map:texture_depth_2d,projection:mat4x4<f32>,p:vec3<f32>)->f32{
 let q=shadowProjection(projection,p);if !q.inside{return 1.0;}
 return textureSampleCompareLevel(map,fogShadowSampler,q.uv,q.depth-0.00005);
}
fn fogSunFarVisibility(p:vec3<f32>)->f32{
 let q=shadowProjection(frame.farShadow,p);if !q.inside{return fogSunPcf(fogSunDistant,frame.distantShadow,p);}
 let visibility=textureSampleCompareLevel(fogSunFar,fogShadowSampler,q.uv,q.depth-0.00005);if q.edge<=0.8{return visibility;}
 return mix(visibility,fogSunPcf(fogSunDistant,frame.distantShadow,p),smoothstep(0.8,1.0,q.edge));
}
fn fogSunVisibility(p:vec3<f32>)->f32{
 let q=shadowProjection(frame.shadow,p);if !q.inside{return fogSunFarVisibility(p);}
 let nearVisibility=textureSampleCompareLevel(fogSunNear,fogShadowSampler,q.uv,q.depth-0.00005);if q.edge<=0.8{return nearVisibility;}
 return mix(nearVisibility,fogSunFarVisibility(p),smoothstep(0.8,1.0,q.edge));
}
fn fogDirectionalRadiance(ray:vec3<f32>)->vec3<f32>{
 if fog.flags.z<=0.0||frame.sun.w<=0.0{return vec3(0.0);}
 let l=safeNormalize(frame.sun.xyz,vec3(0.0,1.0,0.0));
 return max(frame.sunColor.rgb,vec3(0.0))*frame.sun.w*fog.flags.z*fogPhase(clamp(dot(l,ray),-1.0,1.0));
}
struct FogClusterRay{total:u32,slices:u32,near:f32,scale:f32,forward:vec3<f32>,xy:u32,stride:u32,cells:u32,indices:u32,lights:u32};
fn fogClusterRay(pixel:vec2<f32>)->FogClusterRay{
 var c:FogClusterRay;c.total=min(clusters[${H.lightCount}],u32(fog.flags.y));if c.total==0u{return c;}
 let tilesX=clusters[${H.tilesX}];let tilesY=clusters[${H.tilesY}];let slices=clusters[${H.depthSlices}];let tile=f32(clusters[${H.tileSize}]);let near=fogFloat(${H.near}u);let forward=vec3(fogFloat(${H.forwardX}u),fogFloat(${H.forwardY}u),fogFloat(${H.forwardZ}u));
 let xy=vec2<u32>(clamp(floor(pixel/tile),vec2(0.0),vec2(f32(tilesX-1u),f32(tilesY-1u))));
 c.slices=slices;c.near=near;c.scale=fogFloat(${H.logDepthScale}u);c.forward=forward;c.xy=xy.y*tilesX+xy.x;c.stride=tilesY*tilesX;
 c.cells=clusters[${H.cellsOffset}];c.indices=clusters[${H.indicesOffset}];c.lights=clusters[${H.lightsOffset}];return c;
}
fn fogIncident(world:vec3<f32>,ray:vec3<f32>,c:FogClusterRay)->vec3<f32>{
 if c.total==0u{return vec3(0.0);}
 let depth=max(dot(world-frame.eye.xyz,c.forward),c.near);let slice=u32(clamp(floor(log(depth/c.near)*c.scale),0.0,f32(c.slices-1u)));
 let address=c.cells+(slice*c.stride+c.xy)*2u;let start=clusters[address];let count=clusters[address+1u];let overflow=count==0xffffffffu;let iterations=select(count,c.total,overflow);var radiance=vec3(0.0);
 for(var entry=0u;entry<iterations;entry++){var index=entry;if !overflow{index=clusters[c.indices+start+entry];}if index>=c.total{continue;}let strength=scattering[index];if strength<=0.0{continue;}
 let record=c.lights+index*16u;let position=fogVector(record);let delta=position.xyz-world;let d2=dot(delta,delta);let distance=sqrt(d2);if distance>=position.w{continue;}
 let emission=fogVector(record+4u);let direction=fogVector(record+8u);let cone=fogVector(record+12u);let l=delta/max(distance,0.001);
 var attenuation=pow(max(1.0-pow(distance/position.w,4.0),0.0),2.0)/(d2+0.0001);if cone.w>=0.0{attenuation=pow(max(1.0-d2/(position.w*position.w),0.0),max(cone.w,0.0001));}
 if direction.w>0.5{let outer=min(cone.x,cone.y);let inner=max(cone.x,cone.y);let angle=dot(-l,direction.xyz);var spot=step(outer,angle);if inner-outer>0.00001{let ramp=clamp((angle-outer)/(inner-outer),0.0,1.0);spot=ramp*ramp;}attenuation*=spot;}
 if attenuation<=0.0{continue;}
 var visibility=1.0;if cone.z>=0.0{visibility=fogVisibility(u32(cone.z),world);}radiance+=max(emission.rgb*emission.w,vec3(0.0))*attenuation*strength*visibility*fogPhase(clamp(dot(l,ray),-1.0,1.0));
 }return radiance;
}
fn fogSkyIncident(pixel:vec2<f32>,distance:f32)->vec3<f32>{
 if fog.flags.w<.5||fog.volume.x<=fog.volume.z{return vec3(0.0);}
 let uv=vec3(pixel/fog.size.xy,(distance-fog.volume.z)/(fog.volume.x-fog.volume.z));
 if any(uv<vec3(0.0))||any(uv>vec3(1.0)){return vec3(0.0);}
 return max(textureSampleLevel(fogAmbient,fogAmbientSampler,uv,0.0).rgb,vec3(0.0));
}
@fragment fn integrateFS(@builtin(position)fragment:vec4<f32>)->@location(0)vec4<f32>{
 let pixel=fragment.xy*fog.size.xy/fog.size.zw;let point=fogWorld(pixel,fogDepth(vec2<i32>(pixel)));let end=min(length(point-frame.eye.xyz),fog.volume.x);if fog.flags.x<0.5||fog.medium.x<=0.0||end<=fog.volume.z||(clusters[${H.lightCount}]==0u&&fog.flags.z<=0.0&&fog.flags.w<=0.0){return vec4(vec3(0.0),end);}
 let ray=safeNormalize(point-frame.eye.xyz,vec3(0.0,0.0,-1.0));let steps=u32(fog.volume.w);let ds=(end-fog.volume.z)/f32(steps);var result=vec3(0.0);
 let logDensity=log(fog.medium.x);let logEyeDensity=logDensity-fog.medium.y*(frame.eye.y+ray.y*fog.volume.z-fog.medium.z);
 let directional=fogDirectionalRadiance(ray);let cluster=fogClusterRay(pixel);
 for(var i=0u;i<steps;i++){let distance=fog.volume.z+(f32(i)+0.5)*ds;let p=frame.eye.xyz+ray*distance;let sigma=fogDensity(p,logDensity);let transmittance=exp(-fogStepTau(ray,distance,logEyeDensity));var direct=vec3(0.0);if any(directional>vec3(0.0)){direct=directional*fogSunVisibility(p);}result+=(fogIncident(p,ray,cluster)+direct+fogSkyIncident(pixel,distance))*sigma*fog.medium.w*transmittance*ds;}
 return vec4(clamp(result*fogPreVolumeTransmittance(ray),vec3(0.0),vec3(65000.0)),end);
}
@fragment fn compositeFS(@builtin(position)fragment:vec4<f32>)->@location(0)vec4<f32>{
 let pixel=vec2<i32>(fragment.xy);let base=textureLoad(inputColor,pixel,0);let end=fogEndpoint(fragment.xy);let p=fragment.xy*fog.size.zw/fog.size.xy-vec2(0.5);let first=vec2<i32>(floor(p));let f=fract(p);var sum=vec3(0.0);var total=0.0;
 for(var y=0;y<2;y++){for(var x=0;x<2;x++){let q=clamp(first+vec2(x,y),vec2(0),vec2<i32>(fog.size.zw)-vec2(1));let sample=textureLoad(lowFog,q,0);let tolerance=max(0.03,end*0.02);var w=mix(1.0-f.x,f.x,f32(x))*mix(1.0-f.y,f.y,f32(y));w*=exp(-abs(sample.a-end)/tolerance);if sample.a>end+tolerance{w=0.0;}sum+=sample.rgb*w;total+=w;}}
 let value=sum/max(total,0.00001);return vec4(clamp(base.rgb+value,vec3(0.0),vec3(65000.0)),base.a);
}
`;
