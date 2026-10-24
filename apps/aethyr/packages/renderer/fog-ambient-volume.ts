import { COMMON } from "./shaders.ts";
import { sh9Basis } from "./environment-lighting.ts";
import type { V3 } from "./math.ts";

export const FOG_AMBIENT_GRID = Object.freeze({
  width: 32,
  height: 18,
  depth: 24,
});
export const FOG_AMBIENT_BYTES = 32 * 18 * 24 * 8;
export interface FogAmbientSkyProfile {
  enabled: boolean;
  scatteringIntensity: number;
  diagnostic?: string;
}
/** Supports the recovered movable realtime sky, without assuming other source-light paths. */
export function sourceFogAmbientSkyProfile(
  environment: unknown,
): FogAmbientSkyProfile {
  const disabled = (diagnostic: string): FogAmbientSkyProfile => ({
    enabled: false,
    scatteringIntensity: 0,
    diagnostic,
  });
  if (!Array.isArray(environment))
    return disabled("Ambient fog needs the sky and fog settings.");
  const candidates: {
    actor: Record<string, unknown>;
    component: Record<string, unknown>;
  }[] = [];
  for (const actor of environment) {
    if (!actor || typeof actor !== "object" || !Array.isArray(actor.components))
      return disabled("Ambient fog needs complete sky and fog settings.");
    for (const component of actor.components) {
      if (
        !component ||
        typeof component !== "object" ||
        typeof component.class !== "string"
      )
        return disabled("Ambient fog needs complete sky and fog settings.");
      if (
        component.class === "SkyLightComponent" ||
        component.class === "ExponentialHeightFogComponent"
      )
        candidates.push({ actor, component });
    }
  }
  const skies = candidates.filter(
      (item) => item.component.class === "SkyLightComponent",
    ),
    fogs = candidates.filter(
      (item) => item.component.class === "ExponentialHeightFogComponent",
    );
  if (skies.length !== 1 || fogs.length !== 1)
    return disabled("Ambient fog needs one sky light and one fog source.");
  for (const item of [...skies, ...fogs]) {
    const p = item.component.properties,
      actorProperties = item.actor.properties;
    if (!p || typeof p !== "object" || Array.isArray(p))
      return disabled("Ambient fog needs complete sky and fog settings.");
    const properties = p as Record<string, unknown>;
    if (
      properties.visible !== true ||
      properties.hidden_in_game === true ||
      item.actor.hidden === true ||
      (actorProperties &&
        typeof actorProperties === "object" &&
        (actorProperties as Record<string, unknown>).hidden === true)
    )
      return disabled(
        "Ambient fog is off while its sky or fog source is hidden.",
      );
  }
  const p = skies[0].component.properties as Record<string, unknown>,
    fog = fogs[0].component.properties as Record<string, unknown>;
  if (
    p.affects_world !== true ||
    !String(p.mobility).includes("MOVABLE") ||
    !String(p.source_type).includes("SLS_CAPTURED_SCENE") ||
    p.real_time_capture !== true ||
    p.cast_volumetric_shadow !== true
  )
    return disabled("Ambient fog is off for this sky light setup.");
  const strength = p.volumetric_scattering_intensity;
  if (
    typeof strength !== "number" ||
    !Number.isFinite(strength) ||
    strength < 0 ||
    strength > 65536
  )
    return disabled(
      "Ambient fog needs a sky light fog strength from 0 to 65536.",
    );
  const phase = fog.volumetric_fog_scattering_distribution;
  if (
    fog.enable_volumetric_fog !== true ||
    typeof phase !== "number" ||
    !Number.isFinite(phase) ||
    Math.abs(phase) > 0.5
  )
    return disabled(
      "Ambient fog needs an enabled fog source with a supported scattering direction.",
    );
  return { enabled: strength > 0, scatteringIntensity: strength };
}

function checked(value: number, min: number, max: number, name: string) {
  if (!Number.isFinite(value) || value < min || value > max)
    throw Error(`Use ${name} from ${min} to ${max}.`);
  return value;
}
/** Undo the retained Lambert E/pi kernel, then apply normalized HG convolution. */
export function fogAmbientBandWeights(g: number) {
  checked(g, -0.5, 0.5, "ambient fog phase");
  return [
    1,
    1.5 * g,
    1.5 * g,
    1.5 * g,
    4 * g * g,
    4 * g * g,
    4 * g * g,
    4 * g * g,
    4 * g * g,
  ];
}
export function fogAmbientSH(
  coefficients: ArrayLike<number>,
  direction: V3,
  g: number,
): V3 {
  if (coefficients.length !== 36)
    throw Error("Use nine padded ambient fog coefficients.");
  const length = Math.hypot(...direction);
  if (!Number.isFinite(length) || length < 1e-12)
    throw Error("Use a finite ambient fog direction.");
  const basis = sh9Basis(direction.map((n) => n / length) as V3),
    weights = fogAmbientBandWeights(g),
    result: V3 = [0, 0, 0];
  for (let i = 0; i < 9; i++)
    for (let c = 0; c < 3; c++) {
      const value = coefficients[i * 4 + c];
      if (!Number.isFinite(value))
        throw Error("Use finite ambient fog coefficients.");
      result[c] += value * basis[i] * weights[i];
    }
  return result.map((n) => Math.max(n, 0)) as V3;
}
export function fogAmbientProbeVisibility(
  distance: number,
  mean: number,
  meanSquared: number,
) {
  checked(distance, 0, 1e6, "probe distance");
  checked(mean, 0, 1e6, "probe mean");
  checked(meanSquared, 0, 1e12, "probe second moment");
  if (distance <= mean) return 1;
  const variance = Math.max(meanSquared - mean * mean, 0.0001),
    gap = distance - mean;
  return (variance / (variance + gap * gap)) ** 3;
}
export interface FogAmbientProbeField {
  origin: V3;
  spacing: V3;
  dimensions: [number, number, number];
  /** Existing position/valid + SH records, followed by optional constant SH. */
  records: Float32Array;
  constantFog: boolean;
  moments: (index: number, direction: V3) => [number, number];
}
/** Reference query: no surface offset, wrap weight, or normalization of blocked weights. */
export function fogAmbientProbeIncident(
  field: FogAmbientProbeField,
  point: V3,
  direction: V3,
  g: number,
  solarRatio: number,
): V3 {
  checked(solarRatio, 0, 65536, "solar lighting ratio");
  const count = field.dimensions.reduce((a, b) => a * b, 1);
  if (
    count > 4096 ||
    !field.dimensions.every((n) => Number.isInteger(n) && n >= 2 && n <= 64) ||
    !field.spacing.every((n) => Number.isFinite(n) && n > 0 && n <= 64) ||
    !field.origin.every(Number.isFinite) ||
    !point.every(Number.isFinite) ||
    field.records.length !== count * (40 + (field.constantFog ? 36 : 0))
  )
    throw Error("Use a complete bounded ambient fog probe field.");
  const q = point.map((n, i) => (n - field.origin[i]) / field.spacing[i]);
  if (q.some((n, i) => n < 0 || n > field.dimensions[i] - 1)) return [0, 0, 0];
  const cell = q.map((n, i) =>
      Math.min(Math.floor(n), field.dimensions[i] - 2),
    ),
    blend = q.map((n, i) => n - cell[i]),
    coefficients = new Float64Array(36);
  let validSum = 0;
  for (let corner = 0; corner < 8; corner++) {
    const bit = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1],
      p = cell.map((n, i) => n + bit[i]),
      index = p[0] + field.dimensions[0] * (p[1] + field.dimensions[1] * p[2]),
      offset = index * 40;
    if (field.records[offset + 3] < 0.5) continue;
    const tri = blend.reduce((a, n, i) => a * (bit[i] ? n : 1 - n), 1);
    validSum += tri;
    const delta = point.map((n, i) => n - field.records[offset + i]) as V3,
      distance = Math.hypot(...delta);
    let visibility = 1;
    if (distance > 1e-6) {
      const moment = field.moments(index, delta.map((n) => n / distance) as V3);
      visibility = fogAmbientProbeVisibility(distance, ...moment);
    }
    for (let i = 0; i < 9; i++)
      for (let c = 0; c < 3; c++)
        coefficients[i * 4 + c] +=
          (field.records[offset + 4 + i * 4 + c] * solarRatio +
            (field.constantFog
              ? field.records[count * 40 + index * 36 + i * 4 + c]
              : 0)) *
          tri *
          visibility;
  }
  if (validSum < 1e-6) return [0, 0, 0];
  for (let i = 0; i < 36; i++) coefficients[i] /= validSum;
  return fogAmbientSH(coefficients, direction, g);
}

export interface FogAmbientVolumeFrame {
  frameBuffer: GPUBuffer;
  probeRecords: GPUBuffer;
  probeMoments: GPUTextureView;
  enabled: boolean;
  phaseG: number;
  scatteringIntensity: number;
  startDistance: number;
  maxDistance: number;
}
/** A bounded incident-radiance cache. It contains no density, albedo, extinction, or exposure. */
export class FogAmbientVolume {
  readonly texture: GPUTexture;
  readonly view: GPUTextureView;
  readonly sampler: GPUSampler;
  readonly bytes = FOG_AMBIENT_BYTES + 32;
  private readonly settings: GPUBuffer;
  private groups?: {
    resources: unknown[];
    frame: GPUBindGroup;
    inputs: GPUBindGroup;
  };
  private disposed = false;
  private constructor(
    private readonly device: GPUDevice,
    private readonly pipeline: GPUComputePipeline,
  ) {
    this.texture = device.createTexture({
      label: "Occluded sky ambient fog field",
      dimension: "3d",
      size: [32, 18, 24],
      format: "rgba16float",
      usage:
        GPUTextureUsage.STORAGE_BINDING |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_SRC,
    });
    this.view = this.texture.createView({ dimension: "3d" });
    this.sampler = device.createSampler({
      minFilter: "linear",
      magFilter: "linear",
      addressModeU: "clamp-to-edge",
      addressModeV: "clamp-to-edge",
      addressModeW: "clamp-to-edge",
    });
    this.settings = device.createBuffer({
      label: "Ambient fog volume parameters",
      size: 32,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
  }
  static async create(device: GPUDevice) {
    const module = device.createShaderModule({ code: FOG_AMBIENT_VOLUME_WGSL });
    const pipeline = await device.createComputePipelineAsync({
      label: "Ambient fog 13824 cells",
      layout: "auto",
      compute: { module, entryPoint: "ambientMain" },
    });
    return new FogAmbientVolume(device, pipeline);
  }
  render(encoder: GPUCommandEncoder, frame: FogAmbientVolumeFrame) {
    if (this.disposed) throw Error("Open ambient fog before rendering it.");
    checked(
      frame.phaseG,
      frame.enabled ? -0.5 : -0.95,
      frame.enabled ? 0.5 : 0.95,
      "ambient fog phase",
    );
    checked(frame.scatteringIntensity, 0, 65536, "sky fog strength");
    checked(frame.startDistance, 0, 1000, "ambient fog start");
    checked(
      frame.maxDistance,
      frame.startDistance,
      1000,
      "ambient fog distance",
    );
    if (typeof frame.enabled !== "boolean")
      throw Error("Choose whether ambient fog is enabled.");
    this.device.queue.writeBuffer(
      this.settings,
      0,
      new Float32Array([
        frame.startDistance,
        frame.maxDistance,
        frame.phaseG,
        frame.scatteringIntensity,
        Number(frame.enabled),
        0,
        0,
        0,
      ]),
    );
    const resources = [
      frame.frameBuffer,
      frame.probeRecords,
      frame.probeMoments,
    ];
    if (
      !this.groups ||
      resources.some((value, i) => value !== this.groups!.resources[i])
    )
      this.groups = {
        resources,
        frame: this.device.createBindGroup({
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: { buffer: frame.frameBuffer } }],
        }),
        inputs: this.device.createBindGroup({
          layout: this.pipeline.getBindGroupLayout(1),
          entries: [
            { binding: 0, resource: { buffer: this.settings } },
            { binding: 1, resource: frame.probeMoments },
            { binding: 2, resource: { buffer: frame.probeRecords } },
            { binding: 3, resource: this.view },
          ],
        }),
      };
    const pass = encoder.beginComputePass({
      label: "Build occluded sky ambient fog",
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.groups.frame);
    pass.setBindGroup(1, this.groups.inputs);
    pass.dispatchWorkgroups(8, 5, 6);
    pass.end();
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.texture.destroy();
    this.settings.destroy();
    this.groups = undefined;
  }
}

export const FOG_AMBIENT_VOLUME_WGSL =
  COMMON +
  `
struct AmbientSettings{range:vec4<f32>,control:vec4<f32>};
@group(1)@binding(0)var<uniform>ambient:AmbientSettings;
@group(1)@binding(1)var ambientMoments:texture_2d<f32>;
@group(1)@binding(2)var<storage,read>ambientRecords:array<vec4<f32>>;
@group(1)@binding(3)var ambientOutput:texture_storage_3d<rgba16float,write>;
fn ambientOct(direction:vec3<f32>)->vec2<f32>{let d=direction/max(dot(abs(direction),vec3(1.0)),.000001);var uv=d.xy;if d.z<0.0{uv=(vec2(1.0)-abs(uv.yx))*select(vec2(-1.0),vec2(1.0),uv>=vec2(0.0));}return uv*.5+vec2(.5);}
fn ambientVisibility(index:u32,delta:vec3<f32>)->f32{
 let distance=length(delta);if distance<.000001{return 1.0;}
 let columns=u32(frame.probeCounts.w);let tile=vec2(f32(index%columns),f32(index/columns));
 let pixel=tile*frame.probeAtlas.z+vec2(1.0)+ambientOct(delta/distance)*frame.probeAtlas.w-vec2(.5);let lo=vec2<i32>(floor(pixel));let f=fract(pixel);
 let a=textureLoad(ambientMoments,lo,0).rg;let b=textureLoad(ambientMoments,lo+vec2(1,0),0).rg;let c=textureLoad(ambientMoments,lo+vec2(0,1),0).rg;let d=textureLoad(ambientMoments,lo+vec2(1,1),0).rg;
 let moments=mix(mix(a,b,f.x),mix(c,d,f.x),f.y);if distance<=moments.x{return 1.0;}
 let variance=max(moments.y-moments.x*moments.x,.0001);let gap=distance-moments.x;return pow(variance/(variance+gap*gap),3.0);
}
fn ambientIncident(world:vec3<f32>,ray:vec3<f32>)->vec3<f32>{
 if frame.probeOrigin.w<.5||frame.iblSettings.x<.5||frame.skyEnabled.x<.5{return vec3(0.0);}
 let q=(world-frame.probeOrigin.xyz)/frame.probeSpacing.xyz;let maximum=frame.probeCounts.xyz-vec3(1.0);if any(q<vec3(0.0))||any(q>maximum){return vec3(0.0);}
 let count=u32(frame.probeCounts.x)*u32(frame.probeCounts.y)*u32(frame.probeCounts.z);let hasConstant=frame.skyLayers.z>.5;
 if count==0u||arrayLength(&ambientRecords)<count*10u+select(0u,count*9u,hasConstant){return vec3(0.0);}
 let cell=vec3<u32>(min(floor(q),maximum-vec3(1.0)));let blend=clamp(q-vec3<f32>(cell),vec3(0.0),vec3(1.0));
 let x=ray.x;let y=ray.y;let z=ray.z;let g=ambient.range.z;
 let basis=array<f32,9>(.2820947918,.4886025119*y,.4886025119*z,.4886025119*x,1.0925484306*x*y,1.0925484306*y*z,.3153915653*(3.0*z*z-1.0),1.0925484306*x*z,.5462742153*(x*x-y*y));
 var result=vec3(0.0);var validSum=0.0;
 for(var corner=0u;corner<8u;corner++){
  let bits=vec3<u32>(corner&1u,(corner>>1u)&1u,(corner>>2u)&1u);let p=cell+bits;let index=p.x+u32(frame.probeCounts.x)*(p.y+u32(frame.probeCounts.y)*p.z);let record=ambientRecords[index*10u];if record.w<.5{continue;}
  let weights=select(vec3(1.0)-blend,blend,bits>vec3(0u));let tri=weights.x*weights.y*weights.z;validSum+=tri;let visibility=ambientVisibility(index,world-record.xyz);var value=vec3(0.0);
  for(var i=0u;i<9u;i++){var coefficient=ambientRecords[index*10u+1u+i].rgb*frame.skyLayers.x;if hasConstant{coefficient+=ambientRecords[count*10u+index*9u+i].rgb;}var factor=1.0;if i>0u{factor=select(4.0*g*g,1.5*g,i<4u);}value+=coefficient*basis[i]*factor;}
  result+=value*tri*visibility;
 }
 if validSum<.000001{return vec3(0.0);}
 return max(result/validSum,vec3(0.0))*max(frame.iblGain.rgb,vec3(0.0))*max(frame.iblGain.w*frame.cloud.z*frame.iblSettings.z*ambient.range.w,0.0);
}
@compute@workgroup_size(4,4,4)fn ambientMain(@builtin(global_invocation_id)id:vec3<u32>){
 if any(id>=vec3(32u,18u,24u)){return;}
 var incident=vec3(0.0);
 if ambient.control.x>.5&&ambient.range.w>0.0&&ambient.range.y>ambient.range.x{
  let uv=(vec2<f32>(id.xy)+vec2(.5))/vec2(32.0,18.0);let far=frame.inverseView*vec4(uv*vec2(2.0,-2.0)+vec2(-1.0,1.0),1.0,1.0);let ray=safeNormalize(far.xyz/far.w-frame.eye.xyz,vec3(0.0,0.0,-1.0));let distance=mix(ambient.range.x,ambient.range.y,(f32(id.z)+.5)/24.0);
  incident=ambientIncident(frame.eye.xyz+ray*distance,ray);
 }
 // RGBA16F output has a declared 65000 radiance ceiling; NaN cannot enter the cache.
 let bounded=select(vec3(0.0),clamp(incident,vec3(0.0),vec3(65000.0)),incident==incident);
 textureStore(ambientOutput,vec3<i32>(id),vec4(bounded,1.0));
}`;
