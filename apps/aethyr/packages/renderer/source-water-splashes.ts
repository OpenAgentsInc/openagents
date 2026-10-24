import { readAsset, verifyAssetBytes } from "../assets/src/asset-download.ts";
import { decodeRgba8 } from "../assets/src/rgba.ts";
import {
  particleCurve,
  particleDisplacement,
  type ParticleCurveKey,
} from "./ambient-particles.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_SEGMENT_WGSL,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { waterNoiseMips } from "./source-water-streams.ts";

export const WATER_SPLASH_LIMITS = Object.freeze({
  profiles: 4,
  emitters: 64,
  particles: 1024,
  textures: 2,
  textureDimension: 2048,
  textureBytes: 32 * 1024 * 1024,
  stride: 16,
});
export interface WaterSplashTexture {
  id: string;
  url: string;
  sha256: string;
  byteLength: number;
  width: number;
  height: number;
  colorSpace: "linear";
}
export interface WaterSplashProfile {
  id: string;
  noise: string;
  spawnRate: number;
  lifetime: [number, number];
  diameter: [number, number];
  /** Source endpoints can be reversed on an axis; interpolate them as authored. */
  velocityStart: [number, number, number];
  velocityEnd: [number, number, number];
  acceleration: [number, number, number];
  drag: number;
  offset: [number, number, number];
  sphereRadius: number;
  shapeScale: [number, number, number];
  sizeCurve: ParticleCurveKey[];
  alphaCurve: ParticleCurveKey[];
  redCurve: ParticleCurveKey[];
  uvScale: [number, number];
  uvSpeed: [number, number];
  uvFlip: "none" | "random-both";
}
export interface WaterSplashEmitter {
  id: string;
  profile: string;
  model: number[];
}
export interface SourceWaterSplashesManifest {
  schema: "aethyrforge.source-water-splashes.v1";
  /** Omission preserves the full recovered source recipe. */
  appearanceProfile?: "owned-subtle-spray-v1";
  textures: WaterSplashTexture[];
  profiles: WaterSplashProfile[];
  material: {
    radialRadius: number;
    radialDensity: number;
    depthFadeMeters: number;
    /** Owned spatial antialiasing; physical opacity is reduced by the area expansion. */
    minimumFootprintPixels?: 2;
  };
  lifecycle: {
    startSeconds: 0;
    warmupSeconds: 0;
    delaySeconds: 0;
    loop: "continuous";
  };
  scenarios: Record<"day" | "night", WaterSplashEmitter[]>;
  metadata?: Record<string, unknown>;
}
export interface SourceWaterSplashesFrame {
  mode: "day" | "night";
  time: number;
  width: number;
  height: number;
  /** Loaded rgba16float, single sample; opaque depth remains read-only 4xMSAA. */
  colorView: GPUTextureView;
  depthView: GPUTextureView;
  viewProjection: ArrayLike<number>;
  inverseViewProjection: ArrayLike<number>;
  eye: readonly number[];
  effectiveExposure: number;
  fog?: {
    density: number;
    color: readonly number[];
    heightRate?: number;
    volumeMedium?: FogVolumeMedium;
    volumeRange?: FogVolumeRange;
  };
}
function bounded(v: number, lo: number, hi: number) {
  if (!Number.isFinite(v) || v < lo || v > hi)
    throw Error("Prepared water foam contains an unsupported number.");
}
function vector(v: ArrayLike<number>, n: number, lo: number, hi: number) {
  if (!v || v.length !== n)
    throw Error("Prepared water foam has an incomplete vector.");
  for (let i = 0; i < n; i++) bounded(v[i], lo, hi);
}
function keys(v: ParticleCurveKey[]) {
  if (!Array.isArray(v) || v.length < 1 || v.length > 32)
    throw Error("Water foam needs bounded age curves.");
  let last = -Infinity;
  for (const k of v) {
    bounded(k.time, 0, 1);
    bounded(k.value, 0, 16);
    bounded(k.arriveTangent ?? 0, -1000, 1000);
    bounded(k.leaveTangent ?? 0, -1000, 1000);
    if (
      k.time <= last ||
      ![undefined, "linear", "constant", "cubic"].includes(k.mode)
    )
      throw Error("Water foam curve keys must be ordered.");
    last = k.time;
  }
}
export function validateSourceWaterSplashes(m: SourceWaterSplashesManifest) {
  if (
    !m ||
    m.schema !== "aethyrforge.source-water-splashes.v1" ||
    !Array.isArray(m.textures) ||
    m.textures.length < 1 ||
    m.textures.length > 2 ||
    !Array.isArray(m.profiles) ||
    m.profiles.length < 1 ||
    m.profiles.length > 4
  )
    throw Error("Prepare a supported source water foam recipe.");
  const textureIds = new Set<string>();
  let textureBytes = 0;
  for (const t of m.textures) {
    if (
      !t ||
      typeof t.id !== "string" ||
      !t.id ||
      t.id.length > 128 ||
      textureIds.has(t.id) ||
      !/^\/private-assets\/[A-Za-z0-9_-]+\.bin$/.test(t.url) ||
      !/^\w{64}$/.test(t.sha256) ||
      !/^[a-f0-9]{64}$/.test(t.sha256) ||
      t.colorSpace !== "linear"
    )
      throw Error("Use prepared private linear water foam textures.");
    vector([t.width, t.height], 2, 1, 2048);
    if (
      !Number.isInteger(t.width) ||
      !Number.isInteger(t.height) ||
      t.byteLength !== 16 + t.width * t.height * 4
    )
      throw Error("Water foam texture dimensions do not match their data.");
    textureBytes += t.byteLength;
    textureIds.add(t.id);
  }
  if (textureBytes > WATER_SPLASH_LIMITS.textureBytes)
    throw Error("Water foam exceeds its texture budget.");
  const profiles = new Map<string, WaterSplashProfile>();
  for (const p of m.profiles) {
    if (
      !p ||
      typeof p.id !== "string" ||
      !p.id ||
      p.id.length > 128 ||
      profiles.has(p.id) ||
      !textureIds.has(p.noise)
    )
      throw Error(
        "Water foam profiles need unique names and prepared textures.",
      );
    bounded(p.spawnRate, 0.01, 100);
    vector(p.lifetime, 2, 0.01, 2);
    vector(p.diameter, 2, 0.001, 2);
    if (p.lifetime[0] > p.lifetime[1] || p.diameter[0] > p.diameter[1])
      throw Error("Water foam ranges must be ordered.");
    for (const v of [p.velocityStart, p.velocityEnd, p.acceleration, p.offset])
      vector(v, 3, -100, 100);
    bounded(p.drag, 0, 10);
    bounded(p.sphereRadius, 0, 1);
    vector(p.shapeScale, 3, 0, 1);
    keys(p.alphaCurve);
    keys(p.redCurve);
    keys(p.sizeCurve);
    vector(p.uvScale, 2, 0.01, 16);
    vector(p.uvSpeed, 2, -10, 10);
    if (!["none", "random-both"].includes(p.uvFlip))
      throw Error("Use a supported water foam UV profile.");
    profiles.set(p.id, p);
  }
  if (!m.material) throw Error("Water foam material settings are missing.");
  bounded(m.material.radialRadius, 0.01, 2);
  bounded(m.material.radialDensity, 0.01, 16);
  bounded(m.material.depthFadeMeters, 0.001, 10);
  if (
    (m.appearanceProfile !== undefined &&
      m.appearanceProfile !== "owned-subtle-spray-v1") ||
    (m.appearanceProfile === "owned-subtle-spray-v1"
      ? m.material.minimumFootprintPixels !== 2
      : m.material.minimumFootprintPixels !== undefined)
  )
    throw Error("Use a supported fountain spray appearance profile.");
  if (
    !m.lifecycle ||
    m.lifecycle.startSeconds !== 0 ||
    m.lifecycle.warmupSeconds !== 0 ||
    m.lifecycle.delaySeconds !== 0 ||
    m.lifecycle.loop !== "continuous"
  )
    throw Error(
      "Source water foam requires qualified continuous zero-warmup emission.",
    );
  for (const mode of ["day", "night"] as const) {
    const emitters = m.scenarios?.[mode];
    if (!Array.isArray(emitters) || emitters.length > 64)
      throw Error("Use at most64 water foam emitters.");
    const ids = new Set<string>();
    let population = 0;
    for (const e of emitters) {
      if (
        !e ||
        typeof e.id !== "string" ||
        !e.id ||
        e.id.length > 1024 ||
        ids.has(e.id) ||
        !profiles.has(e.profile)
      )
        throw Error(
          "Water foam emitters need unique names and known profiles.",
        );
      vector(e.model, 16, -10000, 10000);
      const a = e.model;
      // This slice supports the source's unit-scale, upright yaw transforms. World gravity then remains vertical in local space.
      if (
        Math.max(
          Math.abs(a[3]),
          Math.abs(a[7]),
          Math.abs(a[11]),
          Math.abs(a[15] - 1),
          Math.abs(a[1]),
          Math.abs(a[4]),
          Math.abs(a[6]),
          Math.abs(a[9]),
          Math.abs(a[5] - 1),
          Math.abs(Math.hypot(a[0], a[2]) - 1),
          Math.abs(Math.hypot(a[8], a[10]) - 1),
          Math.abs(a[0] * a[8] + a[2] * a[10]),
        ) > 0.001
      )
        throw Error(
          "Source water foam requires unit-scale upright owner transforms.",
        );
      population +=
        Math.ceil(
          profiles.get(e.profile)!.spawnRate *
            profiles.get(e.profile)!.lifetime[1],
        ) + 1;
      ids.add(e.id);
    }
    if (population > 1024)
      throw Error("Source water foam exceeds the complete population budget.");
  }
  return m;
}
function hash(v: number) {
  v = Math.imul(v ^ (v >>> 16), 0x7feb352d);
  v = Math.imul(v ^ (v >>> 15), 0x846ca68b);
  return (v ^ (v >>> 16)) >>> 0;
}
function seedOf(s: string) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++)
    h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return hash(h);
}
const random = (seed: number, c: number) =>
  hash(seed ^ Math.imul(c + 1, 0x9e3779b9)) / 4294967296;
const mix = (a: number, b: number, t: number) => a + (b - a) * t;
/** Owned analytic constant-force/drag evaluation; Niagara timestep/random sequences are not reproduced. */
export class WaterSplashSampler {
  readonly manifest: SourceWaterSplashesManifest;
  private profiles: Map<string, WaterSplashProfile>;
  private data = new Float32Array(1024 * 16);
  private output = new Float32Array(1024 * 16);
  private depths = new Float64Array(1024);
  private order: number[] = [];
  constructor(m: SourceWaterSplashesManifest) {
    validateSourceWaterSplashes(m);
    this.manifest = structuredClone(m);
    this.profiles = new Map(this.manifest.profiles.map((p) => [p.id, p]));
  }
  sample(
    mode: "day" | "night",
    time: number,
    eye: readonly number[],
    forward: readonly number[],
  ) {
    bounded(time, 0, MAX_WORLD_ANIMATION_TIME);
    vector(eye, 3, -1e6, 1e6);
    vector(forward, 3, -1, 1);
    if (
      Math.abs(Math.hypot(...forward) - 1) > 0.001 ||
      !["day", "night"].includes(mode)
    )
      throw Error("Water foam needs a prepared scenario and camera direction.");
    let count = 0;
    for (const emitter of this.manifest.scenarios[mode]) {
      const p = this.profiles.get(emitter.profile)!,
        m = emitter.model,
        base = seedOf(emitter.id),
        head = Math.floor(time * p.spawnRate),
        capacity = Math.ceil(p.spawnRate * p.lifetime[1]) + 1;
      for (let slot = 0; slot < capacity; slot++) {
        const birth = head - slot;
        if (birth < 1) break;
        const age = time - birth / p.spawnRate,
          seed = hash(base ^ Math.imul(birth, 0x27d4eb2d)),
          life = mix(...p.lifetime, random(seed, 1));
        if (age < 0 || age >= life) continue;
        const u = age / life,
          azimuth = random(seed, 2) * Math.PI * 2,
          y = random(seed, 3) * 2 - 1,
          r = Math.cbrt(random(seed, 4)) * p.sphereRadius,
          radial = Math.sqrt(Math.max(0, 1 - y * y));
        const local = [
          Math.cos(azimuth) * radial * r * p.shapeScale[0],
          y * r * p.shapeScale[1],
          Math.sin(azimuth) * radial * r * p.shapeScale[2],
        ];
        for (let a = 0; a < 3; a++)
          local[a] +=
            p.offset[a] +
            particleDisplacement(
              age,
              mix(p.velocityStart[a], p.velocityEnd[a], random(seed, 5 + a)),
              p.acceleration[a],
              p.drag,
            );
        const at = count * 16,
          d = this.data;
        for (let a = 0; a < 3; a++)
          d[at + a] =
            m[a] * local[0] +
            m[a + 4] * local[1] +
            m[a + 8] * local[2] +
            m[a + 12];
        d[at + 3] =
          mix(...p.diameter, random(seed, 8)) * particleCurve(p.sizeCurve, u);
        d[at + 4] = particleCurve(p.redCurve, u);
        d[at + 5] = Math.min(1, particleCurve(p.alphaCurve, u));
        d[at + 6] = random(seed, 9) * Math.PI * 2;
        d[at + 7] = this.manifest.textures.findIndex((t) => t.id === p.noise);
        d.set([...p.uvScale, ...p.uvSpeed], at + 8);
        d[at + 12] =
          p.uvFlip === "random-both" && random(seed, 10) < 0.5 ? -1 : 1;
        d[at + 13] =
          p.uvFlip === "random-both" && random(seed, 11) < 0.5 ? -1 : 1;
        d[at + 14] = this.manifest.material.depthFadeMeters;
        d[at + 15] = 0;
        this.depths[count] =
          (d[at] - eye[0]) * forward[0] +
          (d[at + 1] - eye[1]) * forward[1] +
          (d[at + 2] - eye[2]) * forward[2];
        this.order[count] = count;
        count++;
      }
    }
    this.order.length = count;
    this.order.sort((a, b) => this.depths[b] - this.depths[a] || a - b);
    for (let i = 0; i < count; i++)
      this.output.set(
        this.data.subarray(this.order[i] * 16, this.order[i] * 16 + 16),
        i * 16,
      );
    return { count, data: this.output.subarray(0, count * 16) };
  }
}
export const WGSL_SOURCE_WATER_SPLASHES =
  FOG_SEGMENT_WGSL +
  `
struct Frame{view:mat4x4<f32>,inverse:mat4x4<f32>,eye:vec4<f32>,settings:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,forward:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var opaqueDepth:texture_depth_multisampled_2d;
@group(0)@binding(2)var noise0:texture_2d<f32>;
@group(0)@binding(3)var noise1:texture_2d<f32>;
@group(0)@binding(4)var noiseSampler:sampler;
struct Input{@location(0)center:vec4<f32>,@location(1)color:vec4<f32>,@location(2)uvData:vec4<f32>,@location(3)extra:vec4<f32>};
struct Output{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>,@location(1)world:vec3<f32>,@location(2)color:vec4<f32>,@location(3)uvData:vec4<f32>,@location(4)extra:vec4<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32,p:Input)->Output{let corners=array<vec2<f32>,6>(vec2(-.5,-.5),vec2(.5,-.5),vec2(-.5,.5),vec2(-.5,.5),vec2(.5,-.5),vec2(.5,.5));let c=corners[i];let cs=cos(p.color.z);let sn=sin(p.color.z);let q=vec2(c.x*cs-c.y*sn,c.x*sn+c.y*cs);let right=normalize(frame.inverse[0].xyz);let up=normalize(frame.inverse[1].xyz);var expansion=1.0;
 if frame.forward.w>0.0{let center=frame.view*vec4(p.center.xyz,1);let edge=frame.view*vec4(p.center.xyz+right*p.center.w*.5,1);if center.w>0.0&&edge.w>0.0{let pixels=length((edge.xy/edge.w-center.xy/center.w)*frame.settings.xy);expansion=max(1.0,frame.forward.w/max(pixels,.000001));}}
 let world=p.center.xyz+(right*q.x+up*q.y)*p.center.w*expansion;var o:Output;o.position=frame.view*vec4(world,1);o.world=world;o.uv=vec2(c.x,.0-c.y)*p.extra.xy+vec2(.5);o.color=p.color;if frame.forward.w>0.0{o.color.y/=expansion*expansion;}o.uvData=p.uvData;o.extra=p.extra;return o;}
fn splashRadial(uv:vec2<f32>)->f32{let d=max(0.0,1.0-length(uv-vec2(.5))/frame.fog.z);return 1.0-exp(-pow(frame.fog.w*d,2.0));}
@fragment fn fs(v:Output)->@location(0)vec4<f32>{
 let uv=v.uv*v.uvData.xy+v.uvData.zw*frame.eye.w;let a=textureSample(noise0,noiseSampler,uv).r;let b=textureSample(noise1,noiseSampler,uv).r;let noise=select(a,b,v.color.w>.5);
 let pixel=clamp(vec2<i32>(v.position.xy),vec2(0),vec2<i32>(textureDimensions(opaqueDepth))-vec2(1));var depth=1.0;for(var s=0;s<4;s++){depth=min(depth,textureLoad(opaqueDepth,pixel,s));}if v.position.z>depth+.000001{discard;}
 let screenUV=v.position.xy/frame.settings.xy;let q=frame.inverse*vec4(screenUV*vec2(2,-2)+vec2(-1,1),depth,1);var fade=1.0;if depth<.999999{fade=clamp(dot(q.xyz/q.w-v.world,frame.forward.xyz)/v.extra.z,0.0,1.0);}
 var radial=splashRadial(v.uv);if frame.forward.w>0.0{let dx=dpdx(v.uv)*.25;let dy=dpdy(v.uv)*.25;radial=.25*(splashRadial(v.uv-dx-dy)+splashRadial(v.uv+dx-dy)+splashRadial(v.uv-dx+dy)+splashRadial(v.uv+dx+dy));}
 let alpha=clamp(v.color.y,0.0,1.0)*noise*radial*fade;
 let emission=v.color.x*frame.settings.z/max(frame.settings.w,.000001);
 let ray=v.world-frame.eye.xyz;let distance=length(ray);let fog=heightFogComposition(frame.eye.y,ray.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,${FOG_REFERENCE_HEIGHT}.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4((vec3(emission)*fog.x+frame.fogColor.rgb*fog.y)*alpha,alpha);
}
`;
async function fetchFoam(
  url: string,
  limit: number,
  expected?: { byteLength: number; sha256: string },
) {
  return readAsset(
    url,
    async (response) => {
      if (
        !response.ok ||
        Number(response.headers.get("content-length")) > limit ||
        !response.body
      )
        throw Error("Prepare the private fountain foam textures.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let size = 0;
      try {
        for (;;) {
          const p = await reader.read();
          if (p.done) break;
          size += p.value.byteLength;
          if (size > limit)
            throw Error("The fountain foam texture exceeds its byte limit.");
          chunks.push(p.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      const bytes = new Uint8Array(size);
      let at = 0;
      for (const c of chunks) {
        bytes.set(c, at);
        at += c.length;
      }
      if (expected) await verifyAssetBytes(bytes, expected);
      return bytes.buffer;
    },
    {
      init: {
        redirect: "error",
        credentials: "same-origin",
      },
    },
  );
}
export class SourceWaterSplashes {
  private sampler: WaterSplashSampler;
  private settings = new Float32Array(60);
  private group?: GPUBindGroup;
  private depth?: GPUTextureView;
  private disposed = false;
  private constructor(
    private device: GPUDevice,
    private manifest: SourceWaterSplashesManifest,
    private pipeline: GPURenderPipeline,
    private layout: GPUBindGroupLayout,
    private frame: GPUBuffer,
    private instances: GPUBuffer,
    private textures: GPUTexture[],
    private textureSampler: GPUSampler,
    private allocationBytes: number,
  ) {
    this.sampler = new WaterSplashSampler(manifest);
  }
  static async create(
    device: GPUDevice,
    recipe: SourceWaterSplashesManifest,
    load = fetchFoam,
  ) {
    validateSourceWaterSplashes(recipe);
    const manifest = structuredClone(recipe),
      images = [];
    // Finish every byte/hash/format check before allocating any GPU object.
    for (const t of manifest.textures) {
      const bytes = await load(t.url, t.byteLength, t);
      if (bytes.byteLength !== t.byteLength)
        throw Error("The fountain foam texture has the wrong byte count.");
      const digest = [
        ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
      ]
        .map((v) => v.toString(16).padStart(2, "0"))
        .join("");
      if (digest !== t.sha256)
        throw Error("The fountain foam texture changed; prepare it again.");
      const image = decodeRgba8(bytes);
      if (image.width !== t.width || image.height !== t.height)
        throw Error(
          "Fountain foam pixels do not match their prepared dimensions.",
        );
      images.push(image);
    }
    const module = device.createShaderModule({
        code: WGSL_SOURCE_WATER_SPLASHES,
      }),
      info = await module.getCompilationInfo();
    if (info.messages.some((v) => v.type === "error"))
      throw Error("The fountain foam shader could not compile.");
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
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 3, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 4, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });
    const pipeline = await device.createRenderPipelineAsync({
      layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 64,
            stepMode: "instance",
            attributes: [0, 1, 2, 3].map((shaderLocation) => ({
              shaderLocation,
              offset: shaderLocation * 16,
              format: "float32x4" as GPUVertexFormat,
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
      primitive: { topology: "triangle-list", cullMode: "back" },
    });
    const textures: GPUTexture[] = [];
    let frame: GPUBuffer | undefined,
      instances: GPUBuffer | undefined,
      bytes = 0;
    try {
      for (const image of images) {
        const levels = waterNoiseMips(image.pixels, image.width, image.height),
          texture = device.createTexture({
            size: [image.width, image.height],
            format: "rgba8unorm",
            mipLevelCount: levels.length,
            usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
          });
        textures.push(texture);
        for (let i = 0; i < levels.length; i++) {
          const l = levels[i];
          device.queue.writeTexture(
            { texture, mipLevel: i },
            l.pixels as Uint8Array<ArrayBuffer>,
            { bytesPerRow: l.width * 4 },
            [l.width, l.height],
          );
          bytes += l.pixels.byteLength;
        }
      }
      frame = device.createBuffer({
        size: 60 * 4,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      instances = device.createBuffer({
        size: 1024 * 64,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      bytes += frame.size + instances.size;
      return new SourceWaterSplashes(
        device,
        manifest,
        pipeline,
        layout,
        frame,
        instances,
        textures,
        device.createSampler({
          addressModeU: "repeat",
          addressModeV: "repeat",
          magFilter: "linear",
          minFilter: "linear",
          mipmapFilter: "linear",
        }),
        bytes,
      );
    } catch (error) {
      frame?.destroy();
      instances?.destroy();
      for (const texture of textures) texture.destroy();
      throw error;
    }
  }
  render(encoder: GPUCommandEncoder, frame: SourceWaterSplashesFrame) {
    if (this.disposed)
      throw Error("Fountain foam resources have been released.");
    vector(frame.viewProjection, 16, -1e8, 1e8);
    vector(frame.inverseViewProjection, 16, -1e8, 1e8);
    vector(frame.eye, 3, -1e6, 1e6);
    bounded(frame.width, 1, 8192);
    bounded(frame.height, 1, 8192);
    bounded(frame.effectiveExposure, 0.000001, 1e6);
    if (!Number.isInteger(frame.width) || !Number.isInteger(frame.height))
      throw Error("Fountain foam needs whole pixel dimensions.");
    const m = frame.inverseViewProjection;
    const unproject = (z: number) => {
      const w = m[3] * 0 + m[7] * 0 + m[11] * z + m[15];
      return [m[8] * z + m[12], m[9] * z + m[13], m[10] * z + m[14]].map(
        (v) => v / w,
      );
    };
    const near = unproject(0),
      far = unproject(1),
      forward = far.map((v, i) => v - near[i]),
      length = Math.hypot(...forward);
    bounded(length, 0.000001, 1e8);
    forward.forEach((_, i) => (forward[i] /= length));
    const sample = this.sampler.sample(
      frame.mode,
      frame.time,
      frame.eye,
      forward,
    );
    if (!sample.count) return { drawn: 0 };
    const fog = frame.fog ?? { density: 0, color: [0, 0, 0] },
      medium = fog.volumeMedium ?? [0, 0, 1, 0],
      range = fog.volumeRange ?? [0, 0];
    bounded(fog.density, 0, 10);
    vector(fog.color, 3, 0, 1e6);
    bounded(fog.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
    vector(medium, 4, -1e6, 1e6);
    vector(range, 2, 0, 1e6);
    const u = this.settings;
    u.set(Array.from(frame.viewProjection));
    u.set(Array.from(frame.inverseViewProjection), 16);
    u.set([...frame.eye, frame.time], 32);
    u.set(
      [frame.width, frame.height, SCENE_LINEAR_SCALE, frame.effectiveExposure],
      36,
    );
    u.set(
      [
        fog.density,
        fog.heightRate ?? FOG_HEIGHT_RATE,
        this.manifest.material.radialRadius,
        this.manifest.material.radialDensity,
      ],
      40,
    );
    u.set([...fog.color, 0], 44);
    u.set(medium, 48);
    u.set([...range, 0, 0], 52);
    u.set([...forward, this.manifest.material.minimumFootprintPixels ?? 0], 56);
    this.device.queue.writeBuffer(this.frame, 0, u);
    this.device.queue.writeBuffer(
      this.instances,
      0,
      sample.data as Float32Array<ArrayBuffer>,
    );
    if (this.depth !== frame.depthView) {
      this.depth = frame.depthView;
      this.group = this.device.createBindGroup({
        layout: this.layout,
        entries: [
          { binding: 0, resource: { buffer: this.frame } },
          { binding: 1, resource: frame.depthView },
          { binding: 2, resource: this.textures[0].createView() },
          {
            binding: 3,
            resource: (this.textures[1] ?? this.textures[0]).createView(),
          },
          { binding: 4, resource: this.textureSampler },
        ],
      });
    }
    const pass = encoder.beginRenderPass({
      label: "Source fountain foam",
      colorAttachments: [
        { view: frame.colorView, loadOp: "load", storeOp: "store" },
      ],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group!);
    pass.setVertexBuffer(0, this.instances);
    pass.draw(6, sample.count);
    pass.end();
    return { drawn: sample.count };
  }
  get bytes() {
    return this.disposed ? 0 : this.allocationBytes;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.frame.destroy();
    this.instances.destroy();
    for (const t of this.textures) t.destroy();
    this.group = undefined;
    this.depth = undefined;
  }
}
