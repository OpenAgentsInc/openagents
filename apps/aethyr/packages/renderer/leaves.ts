import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import type { CompiledMaterial } from "../assets/src/scene.ts";
import type { CreatureFrame } from "./creatures.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import {
  particleCurve,
  particleDisplacement,
  type ParticleCurveKey,
} from "./ambient-particles.ts";
import { FOG_HEIGHT_RATE, FOG_SEGMENT_WGSL } from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import {
  collectVegetationTextures,
  packVegetation,
  VEGETATION_FLOAT_COUNT,
  WGSLMaterialVegetationFields,
  WGSL_VEGETATION_FUNCTIONS,
} from "./vegetation.ts";
import { normalMapFlags, WGSL_NORMAL_FRAME } from "./normals.ts";
import { MaterialTexturePools } from "./texture-pool.ts";
import { cross, normalize, type V3 } from "./math.ts";
export const LEAF_LIMITS = Object.freeze({
  emitters: 64,
  profiles: 8,
  particles: 4096,
  vertices: 10000,
  indices: 30000,
  geometryBytes: 1024 * 1024,
});
export interface LeafBlob {
  url: string;
  sha256: string;
  byteLength: number;
}
export interface LeafProfile {
  id: string;
  spawnRate: number;
  lifetime: [number, number];
  scale: [number, number];
  velocityMin: V3;
  velocityMax: V3;
  gravity: V3;
  drag: [number, number];
  noiseStrength: number;
  noiseFrequency: number;
  alphaCurve: ParticleCurveKey[];
}
export interface LeafEmitter {
  id: string;
  profile: string;
  model: number[];
  radiusMeters: number;
  lifecycle: {
    startSeconds: 0;
    warmupSeconds: 0;
    delaySeconds: 0;
    clock: "world";
  };
}
export interface LeavesManifest {
  schema: "aethyrforge.leaves.v1";
  geometry: {
    vertices: LeafBlob;
    indices: LeafBlob;
    vertexCount: number;
    indexCount: number;
  };
  /** One qualified source petal mesh, position/normal/UV/vertexRGBA, 48-byte float32 stride. */
  material: CompiledMaterial;
  profiles: LeafProfile[];
  scenarios: Record<"day" | "night", LeafEmitter[]>;
  metadata?: Record<string, unknown>;
}
function bounded(n: number, min: number, max: number) {
  if (!Number.isFinite(n) || n < min || n > max)
    throw Error("Prepared leaves contain an unsupported number.");
}
function vector(
  a: ArrayLike<number>,
  length: number,
  min: number,
  max: number,
) {
  if (!a || a.length !== length)
    throw Error("Prepared leaves need complete vectors.");
  for (let i = 0; i < length; i++) bounded(a[i], min, max);
}
function range(a: [number, number], min: number, max: number) {
  vector(a, 2, min, max);
  if (a[0] > a[1]) throw Error("Prepared leaf ranges must be ordered.");
}
function identity(id: string) {
  if (typeof id !== "string" || !id || id.length > 2048)
    throw Error("Prepared leaves need bounded names.");
}
export function validateLeaves(m: LeavesManifest) {
  if (
    m?.schema !== "aethyrforge.leaves.v1" ||
    !m.geometry ||
    !Array.isArray(m.profiles) ||
    !m.profiles.length ||
    m.profiles.length > 8 ||
    !m.material?.vegetation
  )
    throw Error("Prepare a supported petal recipe.");
  const g = m.geometry;
  bounded(g.vertexCount, 1, LEAF_LIMITS.vertices);
  bounded(g.indexCount, 3, LEAF_LIMITS.indices);
  if (
    !Number.isInteger(g.vertexCount) ||
    !Number.isInteger(g.indexCount) ||
    g.indexCount % 3 ||
    g.vertices.byteLength !== g.vertexCount * 48 ||
    g.indices.byteLength !== g.indexCount * 4 ||
    g.vertices.byteLength + g.indices.byteLength > LEAF_LIMITS.geometryBytes
  )
    throw Error("Leaf geometry has inconsistent counts.");
  for (const blob of [g.vertices, g.indices])
    if (
      !/^\/private-assets\/[A-Za-z0-9_-]+\.bin$/.test(blob.url) ||
      !/^[a-f0-9]{64}$/.test(blob.sha256)
    )
      throw Error("Leaves need private geometry files with their hashes.");
  const materials = collectVegetationTextures(m.material);
  if (
    materials.length > 4 ||
    materials.some(
      (t) => !/^\/private-assets\/textures\/[a-f0-9]{64}\.bin$/.test(t.url),
    )
  )
    throw Error("Leaves need bounded prepared material maps.");
  if (
    m.material.alphaMode !== "MASK" ||
    !m.material.doubleSided ||
    !m.material.vegetation.twoSided
  )
    throw Error("This leaf slice supports two-sided masked petals.");
  normalMapFlags(m.material);
  const packedMaterial = packVegetation(m.material, () => 0);
  if (
    !packedMaterial.every((n) => Math.abs(n) <= 16) ||
    m.material.vegetation.color.contrast < 0 ||
    m.material.vegetation.normalFlatness < 0 ||
    m.material.vegetation.normalFlatness > 1
  )
    throw Error(
      "The prepared petal color profile exceeds its supported range.",
    );
  const profiles = new Map<string, LeafProfile>();
  for (const p of m.profiles) {
    identity(p.id);
    if (profiles.has(p.id)) throw Error("Leaf profile names repeat.");
    profiles.set(p.id, p);
    bounded(p.spawnRate, 0.01, 100);
    range(p.lifetime, 0.01, 60);
    range(p.scale, 0.001, 10);
    range(p.drag, 0, 10);
    for (const v of [p.velocityMin, p.velocityMax, p.gravity])
      vector(v, 3, -100, 100);
    bounded(p.noiseStrength, 0, 100);
    bounded(p.noiseFrequency, 0, 1000);
    if (
      !Array.isArray(p.alphaCurve) ||
      !p.alphaCurve.length ||
      p.alphaCurve.length > 32
    )
      throw Error("Leaf age curves exceed their key budget.");
    let last = -Infinity;
    for (const key of p.alphaCurve) {
      bounded(key.time, -1, 2);
      bounded(key.value, 0, 16);
      bounded(key.arriveTangent ?? 0, -1000, 1000);
      bounded(key.leaveTangent ?? 0, -1000, 1000);
      if (
        key.time <= last ||
        ![undefined, "linear", "constant", "cubic"].includes(key.mode)
      )
        throw Error("Leaf age curves must be ordered.");
      last = key.time;
    }
  }
  for (const mode of ["day", "night"] as const) {
    const emitters = m.scenarios?.[mode];
    if (!Array.isArray(emitters) || emitters.length > 64)
      throw Error("Leaves support up to64 emitters per scene.");
    const ids = new Set<string>();
    let population = 0;
    for (const e of emitters) {
      identity(e.id);
      if (ids.has(e.id) || !profiles.has(e.profile))
        throw Error("Leaf emitters need unique names and known profiles.");
      ids.add(e.id);
      vector(e.model, 16, -1e6, 1e6);
      const a = e.model;
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
          Math.abs(a[0] * a[10] - a[2] * a[8] - 1),
        ) > 0.001
      )
        throw Error(
          "This petal slice supports unit-scale upright owner transforms.",
        );
      bounded(e.radiusMeters, 0, 100);
      if (
        !e.lifecycle ||
        e.lifecycle.startSeconds !== 0 ||
        e.lifecycle.warmupSeconds !== 0 ||
        e.lifecycle.delaySeconds !== 0 ||
        e.lifecycle.clock !== "world"
      )
        throw Error("Leaves require qualified zero-warmup world starts.");
      const p = profiles.get(e.profile)!;
      population += Math.ceil(p.spawnRate * p.lifetime[1]) + 1;
    }
    if (population > LEAF_LIMITS.particles)
      throw Error("The complete leaf population exceeds4096 particles.");
  }
  return m;
}
function hash(x: number) {
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d);
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b);
  return (x ^ (x >>> 16)) >>> 0;
}
function seedOf(s: string) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++)
    h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return hash(h);
}
const random = (seed: number, c: number) =>
  hash(seed ^ Math.imul(c + 1, 0x9e3779b9)) / 4294967296;
const mix = (a: number, b: number, u: number) => a + (b - a) * u;
/** Owned closed-form gravity/drag plus bounded harmonic drift; not Niagara's curl/rotation solver. */
export class LeafSampler {
  readonly manifest: LeavesManifest;
  private profiles: Map<string, LeafProfile>;
  private output = new Float32Array(LEAF_LIMITS.particles * 20);
  constructor(m: LeavesManifest) {
    validateLeaves(m);
    this.manifest = structuredClone(m);
    this.profiles = new Map(this.manifest.profiles.map((p) => [p.id, p]));
  }
  sample(mode: "day" | "night", time: number) {
    bounded(time, 0, MAX_WORLD_ANIMATION_TIME);
    if (!["day", "night"].includes(mode))
      throw Error("Choose a prepared leaf scene.");
    let count = 0;
    for (const e of this.manifest.scenarios[mode]) {
      const p = this.profiles.get(e.profile)!,
        head = Math.floor(time * p.spawnRate),
        capacity = Math.ceil(p.spawnRate * p.lifetime[1]) + 1,
        m = e.model,
        base = seedOf(e.id);
      for (let slot = 0; slot < capacity; slot++) {
        const birth = head - slot;
        if (birth < 1) break;
        const age = time - birth / p.spawnRate,
          seed = hash(base ^ Math.imul(birth, 0x27d4eb2d)),
          life = mix(...p.lifetime, random(seed, 1));
        if (age < 0 || age >= life) continue;
        const y = random(seed, 2) * 2 - 1,
          azimuth = random(seed, 3) * Math.PI * 2,
          r = Math.cbrt(random(seed, 4)) * e.radiusMeters,
          radial = Math.sqrt(Math.max(0, 1 - y * y));
        const point = [
            Math.cos(azimuth) * radial * r,
            y * r,
            Math.sin(azimuth) * radial * r,
          ],
          velocity = p.velocityMin.map((n, a) =>
            mix(n, p.velocityMax[a], random(seed, 5 + a)),
          ),
          drag = mix(...p.drag, random(seed, 8));
        const world = [0, 1, 2].map(
            (a) =>
              m[a] * point[0] +
              m[4 + a] * point[1] +
              m[8 + a] * point[2] +
              m[12 + a],
          ),
          initial = [0, 1, 2].map(
            (a) =>
              m[a] * velocity[0] +
              m[4 + a] * velocity[1] +
              m[8 + a] * velocity[2],
          ),
          tangent = [0, 0, 0];
        for (let a = 0; a < 3; a++) {
          world[a] += particleDisplacement(age, initial[a], p.gravity[a], drag);
          tangent[a] =
            initial[a] * Math.exp(-drag * age) +
            (drag > 1e-6
              ? (p.gravity[a] / drag) * (1 - Math.exp(-drag * age))
              : p.gravity[a] * age);
          const frequency = 0.37 + a * 0.071,
            phase =
              random(seed, 10 + a) * Math.PI * 2 + point[a] * p.noiseFrequency,
            amplitude = p.noiseStrength / (1 + frequency * frequency);
          world[a] +=
            amplitude * (Math.sin(phase + age * frequency) - Math.sin(phase));
          tangent[a] +=
            amplitude * frequency * Math.cos(phase + age * frequency);
        }
        const forward =
            Math.hypot(...tangent) < 1e-9
              ? ([1, 0, 0] as V3)
              : normalize(tangent as V3),
          upHint: V3 = Math.abs(forward[1]) > 0.99 ? [0, 0, 1] : [0, 1, 0],
          side = normalize(cross(forward, upHint)),
          up = normalize(cross(side, forward)),
          scale = mix(...p.scale, random(seed, 9));
        const at = count * 20;
        this.output.set(
          [
            forward[0] * scale,
            forward[1] * scale,
            forward[2] * scale,
            0,
            up[0] * scale,
            up[1] * scale,
            up[2] * scale,
            0,
            side[0] * scale,
            side[1] * scale,
            side[2] * scale,
            0,
            ...world,
            1,
            1,
            1,
            1,
            particleCurve(p.alphaCurve, age / life),
          ],
          at,
        );
        count++;
      }
    }
    return { count, data: this.output.subarray(0, count * 20) };
  }
}
export type LeafFrame = CreatureFrame;
export const WGSL_LEAVES = `
${FOG_SEGMENT_WGSL}
struct Frame{view:mat4x4<f32>,rotation:mat4x4<f32>,eye:vec4<f32>,sun:vec4<f32>,sunColor:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,sh:array<vec4<f32>,9>};
@group(0)@binding(0)var<uniform>frame:Frame;
struct Material{maps:vec4<f32>${WGSLMaterialVegetationFields}};
@group(1)@binding(0)var<uniform>material:Material;
@group(1)@binding(1)var layerColors:texture_2d_array<f32>;@group(1)@binding(2)var layerNormals:texture_2d_array<f32>;@group(1)@binding(3)var layerData:texture_2d_array<f32>;@group(1)@binding(4)var texSampler:sampler;
fn safeNormalize(v:vec3<f32>,fallback:vec3<f32>)->vec3<f32>{let size=length(v);return select(fallback,v/max(size,.0000001),size>.0000001);}
${WGSL_NORMAL_FRAME}
${WGSL_VEGETATION_FUNCTIONS}
struct Input{@location(0)position:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)color:vec4<f32>,@location(4)m0:vec4<f32>,@location(5)m1:vec4<f32>,@location(6)m2:vec4<f32>,@location(7)m3:vec4<f32>,@location(8)particle:vec4<f32>};
struct Output{@builtin(position)clip:vec4<f32>,@location(0)world:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)color:vec4<f32>};
@vertex fn vs(v:Input)->Output{let model=mat4x4(v.m0,v.m1,v.m2,v.m3);let p=model*vec4(v.position,1);var o:Output;o.world=p.xyz;o.clip=frame.view*p;o.normal=normalize(mat3x3(v.m0.xyz,v.m1.xyz,v.m2.xyz)*v.normal);o.uv=v.uv;o.color=v.color;return o;}
fn leafSky(n:vec3<f32>)->vec3<f32>{let d=normalize((frame.rotation*vec4(n,0)).xyz);let b=array<f32,9>(.2820947918,.4886025119*d.y,.4886025119*d.z,.4886025119*d.x,1.0925484306*d.x*d.y,1.0925484306*d.y*d.z,.3153915653*(3*d.z*d.z-1),1.0925484306*d.x*d.z,.5462742153*(d.x*d.x-d.y*d.y));var sum=frame.ambient.rgb;for(var i=0;i<9;i++){sum+=frame.sh[i].rgb*b[i];}return max(sum,vec3(0));}
@fragment fn fs(v:Output,@builtin(front_facing)front:bool)->@location(0)vec4<f32>{
 let sourceNormal=normalize(v.normal);let face=select(-1.0,1.0,front);let directX=(u32(material.maps.w)&1u)!=0u;let view=safeNormalize(frame.eye.xyz-v.world,sourceNormal);let basis=normalFrame(sourceNormal,dpdx(v.world),dpdy(v.world),dpdx(v.uv),dpdy(v.uv));let sample=evaluateVegetation(v.uv,v.color,normalFrameView(basis,view,directX),face);
 let n=mapTangentNormal(basis,sample.tangentNormal,directX,face,1.0);if sample.opacity<material.vegetationConfig.w{discard;}
 let direct=frame.sunColor.rgb*max(dot(n,frame.sun.xyz),0.0)/3.14159265359;let transmitted=frame.sunColor.rgb*max(-dot(n,frame.sun.xyz),0.0)/3.14159265359;
 let lit=sample.base*(direct+leafSky(n))+sample.subsurface*transmitted;
 let ray=v.world-frame.eye.xyz;let distance=length(ray);let fog=heightFogComposition(frame.eye.y,ray.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4(clamp((lit*fog.x+frame.fogColor.rgb*fog.y)*${SCENE_LINEAR_SCALE},vec3(0.0),vec3(65000.0)),sample.opacity);
}`;
async function loadBlob(blob: LeafBlob) {
  return readAsset(
    blob.url,
    async (response) => {
      if (
        !response.ok ||
        Number(response.headers.get("content-length")) > blob.byteLength ||
        !response.body
      )
        throw Error("The prepared leaf geometry could not open.");
      let size = 0;
      const chunks: Uint8Array[] = [],
        reader = response.body.getReader();
      try {
        for (;;) {
          const r = await reader.read();
          if (r.done) break;
          size += r.value.length;
          if (size > blob.byteLength)
            throw Error("Leaf geometry exceeds its byte limit.");
          chunks.push(r.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      const bytes = new Uint8Array(size);
      let at = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, at);
        at += chunk.length;
      }
      const hash = [
        ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
      ]
        .map((n) => n.toString(16).padStart(2, "0"))
        .join("");
      if (size !== blob.byteLength || hash !== blob.sha256)
        throw new AssetIntegrityError(
          "Leaf geometry changed; prepare it again.",
        );
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
export function validateLeafGeometry(
  manifest: LeavesManifest,
  vertexBytes: ArrayBuffer,
  indexBytes: ArrayBuffer,
) {
  if (
    vertexBytes.byteLength !== manifest.geometry.vertexCount * 48 ||
    indexBytes.byteLength !== manifest.geometry.indexCount * 4
  )
    throw Error("Leaf geometry byte counts do not match.");
  const vertices = new Float32Array(vertexBytes),
    indices = new Uint32Array(indexBytes);
  if (
    !vertices.every(Number.isFinite) ||
    indices.some((n) => n >= manifest.geometry.vertexCount)
  )
    throw Error("Leaf geometry has invalid vertices or indices.");
  for (let at = 0; at < vertices.length; at += 12) {
    vector(vertices.subarray(at, at + 3), 3, -10000, 10000);
    vector(vertices.subarray(at + 6, at + 8), 2, -10000, 10000);
    vector(vertices.subarray(at + 8, at + 12), 4, 0, 1);
    bounded(Math.hypot(...vertices.subarray(at + 3, at + 6)), 0.5, 2);
  }
}
export class Leaves {
  private sampler: LeafSampler;
  private values = new Float32Array(100);
  private disposed = false;
  private owned: GPUBuffer[] = [];
  readonly bytes: number;
  private constructor(
    private device: GPUDevice,
    private manifest: LeavesManifest,
    private pipeline: GPURenderPipeline,
    private vertices: GPUBuffer,
    private indices: GPUBuffer,
    private instances: GPUBuffer,
    private uniform: GPUBuffer,
    private frameGroup: GPUBindGroup,
    private materialGroup: GPUBindGroup,
    private pools: MaterialTexturePools,
    owned: GPUBuffer[],
  ) {
    this.sampler = new LeafSampler(manifest);
    this.owned = owned;
    this.bytes = owned.reduce((n, b) => n + b.size, 0) + pools.bytes;
  }
  static async create(device: GPUDevice, input: LeavesManifest) {
    validateLeaves(input);
    const manifest = structuredClone(input),
      [vertexBytes, indexBytes] = await Promise.all([
        loadBlob(manifest.geometry.vertices),
        loadBlob(manifest.geometry.indices),
      ]);
    validateLeafGeometry(manifest, vertexBytes, indexBytes);
    const requirements = collectVegetationTextures(manifest.material),
      pools = await MaterialTexturePools.create(
        device,
        new Map([[manifest.material.id, requirements]]),
        {
          maxGpuBytes: 64 * 1024 * 1024,
          maxDecodedBytes: 32 * 1024 * 1024,
        },
      ),
      owned: GPUBuffer[] = [];
    const buffer = (
      data: ArrayBuffer | Float32Array,
      usage: GPUBufferUsageFlags,
    ) => {
      const bytes =
          data instanceof ArrayBuffer
            ? new Uint8Array(data)
            : new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
        b = device.createBuffer({
          size: Math.max(4, bytes.byteLength),
          usage: usage | GPUBufferUsage.COPY_DST,
        });
      owned.push(b);
      device.queue.writeBuffer(b, 0, bytes);
      return b;
    };
    try {
      const module = device.createShaderModule({ code: WGSL_LEAVES }),
        info = await module.getCompilationInfo();
      if (info.messages.some((m) => m.type === "error"))
        throw Error(
          info.messages
            .filter((m) => m.type === "error")
            .map((m) => m.message)
            .join("\n"),
        );
      const pipeline = await device.createRenderPipelineAsync({
        layout: "auto",
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
                { shaderLocation: 3, offset: 32, format: "float32x4" },
              ],
            },
            {
              arrayStride: 80,
              stepMode: "instance",
              attributes: [0, 1, 2, 3, 4].map((i) => ({
                shaderLocation: 4 + i,
                offset: i * 16,
                format: "float32x4" as const,
              })),
            },
          ],
        },
        fragment: {
          module,
          entryPoint: "fs",
          targets: [{ format: "rgba16float" }],
        },
        primitive: { cullMode: "none" },
        depthStencil: {
          format: "depth24plus",
          depthCompare: "less",
          depthWriteEnabled: true,
        },
        multisample: { count: 4, alphaToCoverageEnabled: true },
      });
      const vertices = buffer(vertexBytes, GPUBufferUsage.VERTEX),
        indices = buffer(indexBytes, GPUBufferUsage.INDEX),
        instances = buffer(new Float32Array(4096 * 20), GPUBufferUsage.VERTEX),
        uniform = buffer(new Float32Array(100), GPUBufferUsage.UNIFORM),
        frameGroup = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: { buffer: uniform } }],
        }),
        roles = pools.forMaterial(manifest.material.id, requirements),
        settings = new Float32Array(4 + VEGETATION_FLOAT_COUNT);
      settings[3] = normalMapFlags(manifest.material);
      settings.set(
        packVegetation(manifest.material, (url, role) =>
          roles[role].index(url),
        ),
        4,
      );
      const materialBuffer = buffer(settings, GPUBufferUsage.UNIFORM),
        materialGroup = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(1),
          entries: [
            { binding: 0, resource: { buffer: materialBuffer } },
            ...(["color", "normal", "data"] as const).map((role, i) => ({
              binding: 1 + i,
              resource: roles[role].texture.createView({
                dimension: "2d-array",
              }),
            })),
            {
              binding: 4,
              resource: device.createSampler({
                addressModeU: "repeat",
                addressModeV: "repeat",
                magFilter: "linear",
                minFilter: "linear",
                mipmapFilter: "linear",
              }),
            },
          ],
        });
      return new Leaves(
        device,
        manifest,
        pipeline,
        vertices,
        indices,
        instances,
        uniform,
        frameGroup,
        materialGroup,
        pools,
        owned,
      );
    } catch (error) {
      owned.forEach((b) => b.destroy());
      pools.dispose();
      throw error;
    }
  }
  render(pass: GPURenderPassEncoder, frame: LeafFrame) {
    if (this.disposed) throw Error("Leaf graphics resources were released.");
    const sample = this.sampler.sample(frame.mode, frame.time);
    if (frame.enabled === false || !sample.count) return { count: 0 };
    for (const [a, n] of [
      [frame.viewProjection, 16],
      [frame.diffuseRotation, 16],
      [frame.diffuseSH, 36],
    ] as const)
      vector(a, n, -1e9, 1e9);
    vector(frame.eye, 3, -1e6, 1e6);
    vector(frame.sunDirection, 3, -1, 1);
    vector(frame.sunIlluminance, 3, 0, 1e6);
    vector(frame.ambient ?? [0, 0, 0], 3, 0, 1e6);
    if (Math.abs(Math.hypot(...frame.sunDirection) - 1) > 0.001)
      throw Error("Leaf lighting needs a unit sun direction.");
    const u = this.values;
    u.set(frame.viewProjection);
    u.set(frame.diffuseRotation, 16);
    u.set([...frame.eye, frame.time], 32);
    u.set([...frame.sunDirection, 0], 36);
    u.set([...frame.sunIlluminance, 0], 40);
    u.set([...(frame.ambient ?? [0, 0, 0]), 0], 44);
    bounded(frame.fog?.density ?? 0, 0, 10);
    bounded(frame.fog?.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
    vector(frame.fog?.color ?? [0, 0, 0], 3, 0, 1e6);
    u.set(
      [frame.fog?.density ?? 0, frame.fog?.heightRate ?? FOG_HEIGHT_RATE, 0, 0],
      48,
    );
    u.set([...(frame.fog?.color ?? [0, 0, 0]), 0], 52);
    vector(frame.fog?.volumeMedium ?? [0, 0, 1, 0], 4, -1e6, 1e6);
    vector(frame.fog?.volumeRange ?? [0, 0], 2, 0, 1e6);
    u.set(frame.fog?.volumeMedium ?? [0, 0, 1, 0], 56);
    u.set([...(frame.fog?.volumeRange ?? [0, 0]), 0, 0], 60);
    u.set(frame.diffuseSH, 64);
    this.device.queue.writeBuffer(this.uniform, 0, u);
    this.device.queue.writeBuffer(
      this.instances,
      0,
      sample.data as Float32Array<ArrayBuffer>,
    );
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.frameGroup);
    pass.setBindGroup(1, this.materialGroup);
    pass.setVertexBuffer(0, this.vertices);
    pass.setVertexBuffer(1, this.instances);
    pass.setIndexBuffer(this.indices, "uint32");
    pass.drawIndexed(this.manifest.geometry.indexCount, sample.count);
    return { count: sample.count };
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.owned.forEach((b) => b.destroy());
    this.pools.dispose();
  }
}
