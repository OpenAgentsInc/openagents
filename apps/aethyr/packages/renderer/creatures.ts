import type { WindFieldSettings } from "./shared-wind.ts";
import { readAsset, verifyAssetBytes } from "../assets/src/asset-download.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import {
  particleCurve,
  particleDisplacement,
  type ParticleCurveKey,
} from "./ambient-particles.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_SEGMENT_WGSL,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

export const CREATURE_LIMITS = Object.freeze({
  creatures: 256,
  candidates: 2048,
  emitters: 16,
  profiles: 4,
  vertices: 100000,
  indices: 300000,
  geometryBytes: 8 * 1024 * 1024,
  instanceFloats: 20,
  delayPeriod: 128,
});
type Triple = [number, number, number];
export interface CreatureBlob {
  url: string;
  sha256: string;
  byteLength: number;
}
export interface CreatureMesh {
  id: string;
  firstIndex: number;
  indexCount: number;
  /** UV0 controls source wing displacement; original mesh normals are retained. */
  flap: {
    amplitude: number;
    uvMultiplier: number;
    frequency: [number, number];
  };
  doubleSided: boolean;
  opacityCutoff: number;
}
export interface CreatureProfile {
  id: string;
  kind: "seagull" | "bat";
  mesh: string;
  spawnRate: number;
  spawnProbability: number;
  lifetime: [number, number];
  scale: [number, number];
  sphereRadius: number;
  /** Endpoints retain the source random-vector direction, including descending axes. */
  velocityMin: Triple;
  velocityMax: Triple;
  color: Triple;
  alphaCurve: ParticleCurveKey[];
  alphaMultiplierCurve: ParticleCurveKey[];
  scaleCurve: ParticleCurveKey[];
  ditherCurve: ParticleCurveKey[];
  /** Source numerical inputs; the bounded bat path replaces the Niagara field solver. */
  drag: number;
  vortexStrength: number;
  noiseStrength: number;
  noiseFrequency: number;
  emission: {
    activeSeconds: number;
    delay: [number, number];
    delayFirstLoopOnly: boolean;
    recalculateDelay: boolean;
  };
}
export interface CreatureEmitter {
  id: string;
  profile: string;
  model: number[];
  lifecycle: { startSeconds: number; warmupSeconds: 0; clock: "world" };
}
export interface CreaturesManifest {
  schema: "aethyrforge.creatures.v1";
  /** Little-endian packed pos3/normal3/UV2 float32 and absolute uint32 triangle indices. */
  geometry: {
    vertices: CreatureBlob;
    indices: CreatureBlob;
    vertexCount: number;
    indexCount: number;
  };
  meshes: CreatureMesh[];
  profiles: CreatureProfile[];
  scenarios: Record<"day" | "night", CreatureEmitter[]>;
  metadata?: Record<string, unknown>;
}
function number(value: number, lo: number, hi: number) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < lo ||
    value > hi
  )
    throw new Error("Prepared creatures contain an unsupported number.");
}
function vector(
  value: readonly number[],
  count: number,
  lo: number,
  hi: number,
) {
  if (!Array.isArray(value) || value.length !== count)
    throw new Error("Prepared creatures need complete vectors.");
  value.forEach((v) => number(v, lo, hi));
}
function range(value: [number, number], lo: number, hi: number) {
  vector(value, 2, lo, hi);
  if (value[0] > value[1])
    throw new Error("Prepared creature ranges must be ordered.");
}
function integer(value: number, lo: number, hi: number) {
  number(value, lo, hi);
  if (!Number.isInteger(value))
    throw new Error("Prepared creature counts must be integers.");
}
function identity(value: string) {
  if (typeof value !== "string" || !value || value.length > 2048)
    throw new Error("Prepared creatures need bounded identities.");
}
function curve(value: ParticleCurveKey[]) {
  if (!Array.isArray(value) || value.length < 1 || value.length > 32)
    throw new Error("Prepared creature curves need one to 32 keys.");
  let previous = -Infinity;
  for (const key of value) {
    number(key.time, -1, 2);
    number(key.value, 0, 16);
    number(key.arriveTangent ?? 0, -1000, 1000);
    number(key.leaveTangent ?? 0, -1000, 1000);
    if (
      key.time <= previous ||
      ![undefined, "linear", "cubic", "constant"].includes(key.mode)
    )
      throw new Error("Prepared creature curve keys must be ordered.");
    previous = key.time;
  }
}
function blob(value: CreatureBlob) {
  if (
    !value ||
    typeof value.url !== "string" ||
    !/^\/private-assets\/[A-Za-z0-9._-]+$/.test(value.url) ||
    !/^[a-f0-9]{64}$/.test(value.sha256)
  )
    throw new Error("Prepare a local creature geometry file with its hash.");
  integer(value.byteLength, 4, CREATURE_LIMITS.geometryBytes);
}
export function validateCreatures(value: CreaturesManifest) {
  if (
    value?.schema !== "aethyrforge.creatures.v1" ||
    !value.geometry ||
    !Array.isArray(value.meshes) ||
    !Array.isArray(value.profiles)
  )
    throw new Error("Prepare a supported creature recipe.");
  const g = value.geometry;
  integer(g.vertexCount, 1, CREATURE_LIMITS.vertices);
  integer(g.indexCount, 3, CREATURE_LIMITS.indices);
  blob(g.vertices);
  blob(g.indices);
  if (
    g.indexCount % 3 ||
    g.vertices.byteLength !== g.vertexCount * 32 ||
    g.indices.byteLength !== g.indexCount * 4 ||
    g.vertices.byteLength + g.indices.byteLength > CREATURE_LIMITS.geometryBytes
  )
    throw new Error("Prepared creature geometry has inconsistent byte counts.");
  if (
    value.meshes.length < 1 ||
    value.meshes.length > CREATURE_LIMITS.profiles ||
    value.profiles.length < 1 ||
    value.profiles.length > CREATURE_LIMITS.profiles
  )
    throw new Error("Prepared creatures exceed the profile budget.");
  const meshes = new Set<string>(),
    profiles = new Map<string, CreatureProfile>();
  for (const m of value.meshes) {
    identity(m.id);
    if (meshes.has(m.id))
      throw new Error("Prepared creature mesh identities repeat.");
    meshes.add(m.id);
    integer(m.firstIndex, 0, g.indexCount - 3);
    integer(m.indexCount, 3, g.indexCount);
    if (
      m.firstIndex % 3 ||
      m.indexCount % 3 ||
      m.firstIndex + m.indexCount > g.indexCount ||
      typeof m.doubleSided !== "boolean" ||
      !m.flap
    )
      throw new Error("Prepared creature mesh ranges are incomplete.");
    number(m.opacityCutoff, 0, 1);
    number(m.flap.amplitude, 0, 2);
    number(m.flap.uvMultiplier, 0, 16);
    range(m.flap.frequency, 0, 30);
  }
  for (const p of value.profiles) {
    identity(p.id);
    if (
      profiles.has(p.id) ||
      !meshes.has(p.mesh) ||
      !["seagull", "bat"].includes(p.kind)
    )
      throw new Error("Prepared creatures need unique supported profiles.");
    profiles.set(p.id, p);
    number(p.spawnRate, 0.01, 100);
    number(p.spawnProbability, 0, 1);
    range(p.lifetime, 0.01, 60);
    range(p.scale, 0.001, 10);
    number(p.sphereRadius, 0, 100);
    vector(p.velocityMin, 3, -100, 100);
    vector(p.velocityMax, 3, -100, 100);
    vector(p.color, 3, 0, 16);
    [p.alphaCurve, p.alphaMultiplierCurve, p.scaleCurve, p.ditherCurve].forEach(
      curve,
    );
    number(p.drag, 0, 10);
    number(p.vortexStrength, 0, 100);
    number(p.noiseStrength, 0, 100);
    number(p.noiseFrequency, 0, 1000);
    if (
      !p.emission ||
      typeof p.emission.delayFirstLoopOnly !== "boolean" ||
      typeof p.emission.recalculateDelay !== "boolean"
    )
      throw new Error("Prepared creatures need a qualified emission schedule.");
    number(p.emission.activeSeconds, 0.1, 60);
    range(p.emission.delay, 0, 60);
    const births = p.spawnRate * p.emission.activeSeconds;
    if (
      Math.abs(births - Math.round(births)) > 0.00001 ||
      births < 1 ||
      births > 6000
    )
      throw new Error(
        "This creature schedule needs an integer number of births per active loop.",
      );
  }
  for (const mode of ["day", "night"] as const) {
    const emitters = value.scenarios?.[mode];
    if (!Array.isArray(emitters) || emitters.length > CREATURE_LIMITS.emitters)
      throw new Error("Prepared creatures exceed the emitter budget.");
    const ids = new Set<string>();
    let candidates = 0;
    for (const e of emitters) {
      identity(e.id);
      const p = profiles.get(e.profile);
      if (ids.has(e.id) || !p)
        throw new Error(
          "Prepared creature emitters need unique identities and known profiles.",
        );
      ids.add(e.id);
      vector(e.model, 16, -1e6, 1e6);
      if (
        Math.abs(e.model[3]) + Math.abs(e.model[7]) + Math.abs(e.model[11]) >
          1e-7 ||
        Math.abs(e.model[15] - 1) > 1e-7
      )
        throw new Error("Prepared creature transforms must be affine.");
      const columns = [
          [e.model[0], e.model[1], e.model[2]],
          [e.model[4], e.model[5], e.model[6]],
          [e.model[8], e.model[9], e.model[10]],
        ],
        sizes = columns.map((v) => Math.hypot(...v));
      sizes.forEach((v) => number(v, 0.001, 100));
      if (
        sizes.some((v) => Math.abs(v - sizes[0]) > sizes[0] * 0.0001) ||
        Math.abs(dot(columns[0], columns[1])) > sizes[0] ** 2 * 0.0001 ||
        Math.abs(dot(columns[0], columns[2])) > sizes[0] ** 2 * 0.0001 ||
        Math.abs(dot(columns[1], columns[2])) > sizes[0] ** 2 * 0.0001 ||
        dot(cross(columns[0], columns[1]), columns[2]) <= 0
      )
        throw new Error(
          "This creature profile supports uniform positive owner transforms.",
        );
      if (
        !e.lifecycle ||
        e.lifecycle.warmupSeconds !== 0 ||
        e.lifecycle.clock !== "world"
      )
        throw new Error(
          "Prepared creatures require explicit zero-warmup world starts.",
        );
      number(e.lifecycle.startSeconds, 0, MAX_WORLD_ANIMATION_TIME);
      candidates +=
        Math.ceil(p.spawnRate * p.lifetime[1]) +
        Math.ceil(p.spawnRate * p.emission.activeSeconds);
    }
    if (candidates > CREATURE_LIMITS.candidates)
      throw new Error("Prepared creatures exceed the candidate budget.");
  }
  return value;
}
function hash(x: number) {
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d);
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b);
  return (x ^ (x >>> 16)) >>> 0;
}
function seedOf(text: string) {
  let n = 2166136261;
  for (let i = 0; i < text.length; i++)
    n = Math.imul(n ^ text.charCodeAt(i), 16777619);
  return hash(n);
}
function random(seed: number, channel: number) {
  return hash(seed ^ Math.imul(channel + 1, 0x9e3779b9)) / 4294967296;
}
function mix(a: number, b: number, u: number) {
  return a + (b - a) * u;
}
function dot(a: readonly number[], b: readonly number[]) {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}
function cross(a: readonly number[], b: readonly number[]) {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}
function normalized(
  v: readonly number[],
  fallback: readonly number[] = [1, 0, 0],
) {
  const length = Math.hypot(...v);
  return length > 1e-9 ? v.map((n) => n / length) : [...fallback];
}
interface Schedule {
  prefix: Float64Array;
  delays: Float64Array;
  period: number;
  firstDelay: number;
}
function schedule(p: CreatureProfile, seed: number): Schedule {
  const n = CREATURE_LIMITS.delayPeriod,
    prefix = new Float64Array(n + 1),
    delays = new Float64Array(n),
    fixed = mix(...p.emission.delay, random(seed, 200));
  for (let i = 0; i < n; i++) {
    delays[i] = p.emission.delayFirstLoopOnly
      ? 0
      : p.emission.recalculateDelay
        ? mix(...p.emission.delay, random(seed, 200 + i))
        : fixed;
    prefix[i + 1] = prefix[i] + delays[i] + p.emission.activeSeconds;
  }
  return {
    prefix,
    delays,
    period: prefix[n],
    firstDelay: p.emission.delayFirstLoopOnly ? fixed : 0,
  };
}
/** Positive-time, bounded replay; a periodic owned delay sequence replaces Niagara RNG. */
export class CreatureSampler {
  readonly manifest: CreaturesManifest;
  private profiles: Map<string, CreatureProfile>;
  private meshes: Map<string, CreatureMesh>;
  private schedules = new Map<string, Schedule>();
  private output = new Float32Array(
    CREATURE_LIMITS.creatures * CREATURE_LIMITS.instanceFloats,
  );
  constructor(
    manifest: CreaturesManifest,
    readonly budget: number = CREATURE_LIMITS.creatures,
  ) {
    validateCreatures(manifest);
    integer(budget, 0, CREATURE_LIMITS.creatures);
    this.manifest = structuredClone(manifest);
    this.profiles = new Map(this.manifest.profiles.map((p) => [p.id, p]));
    this.meshes = new Map(this.manifest.meshes.map((m) => [m.id, m]));
    for (const mode of ["day", "night"] as const)
      for (const e of this.manifest.scenarios[mode])
        this.schedules.set(
          e.id,
          schedule(this.profiles.get(e.profile)!, seedOf(e.id)),
        );
  }
  sample(mode: "day" | "night", time: number, eye: readonly number[]) {
    number(time, 0, MAX_WORLD_ANIMATION_TIME);
    vector(eye, 3, -1e6, 1e6);
    if (mode !== "day" && mode !== "night")
      throw new Error("Choose a prepared creature scenario.");
    const candidates: {
      data: number[];
      mesh: string;
      distance: number;
      key: number;
    }[] = [];
    for (const e of this.manifest.scenarios[mode]) {
      const p = this.profiles.get(e.profile)!,
        mesh = this.meshes.get(p.mesh)!,
        base = seedOf(e.id),
        s = this.schedules.get(e.id)!,
        localTime = time - e.lifecycle.startSeconds - s.firstDelay;
      if (localTime <= 0) continue;
      const first = Math.max(
          0,
          Math.floor((localTime - p.lifetime[1]) / s.period),
        ),
        last = Math.floor(localTime / s.period),
        m = e.model;
      for (let block = first; block <= last; block++)
        for (let slot = 0; slot < CREATURE_LIMITS.delayPeriod; slot++) {
          const cycle = block * CREATURE_LIMITS.delayPeriod + slot,
            start = block * s.period + s.prefix[slot] + s.delays[slot];
          if (
            start > localTime ||
            start + p.emission.activeSeconds < localTime - p.lifetime[1]
          )
            continue;
          const count = Math.round(p.emission.activeSeconds * p.spawnRate);
          for (let ordinal = 1; ordinal <= count; ordinal++) {
            const birth = start + ordinal / p.spawnRate,
              age = localTime - birth;
            if (age < 0 || age >= p.lifetime[1]) continue;
            const key = cycle * count + ordinal,
              seed = hash(base ^ hash(key));
            if (random(seed, 0) >= p.spawnProbability) continue;
            const lifetime = mix(...p.lifetime, random(seed, 1));
            if (age >= lifetime) continue;
            const u = age / lifetime,
              azimuth = random(seed, 2) * Math.PI * 2,
              y = random(seed, 3) * 2 - 1,
              radius = Math.cbrt(random(seed, 4)) * p.sphereRadius,
              radial = Math.sqrt(Math.max(0, 1 - y * y));
            const offset = [
                Math.cos(azimuth) * radial * radius,
                y * radius,
                Math.sin(azimuth) * radial * radius,
              ],
              velocity = p.velocityMin.map((v, axis) =>
                mix(v, p.velocityMax[axis], random(seed, 5 + axis)),
              );
            const local = offset.map(
                (v, axis) =>
                  v + particleDisplacement(age, velocity[axis], 0, p.drag),
              ),
              tangent = velocity.map((v) => v * Math.exp(-p.drag * age));
            if (p.kind === "bat") {
              // Owned bounded orbit/drift, not an execution of the source vortex/curl modules.
              const omega = Math.min(
                  3,
                  Math.sqrt(p.vortexStrength / Math.max(1, p.sphereRadius)),
                ),
                phi = random(seed, 12) * Math.PI * 2,
                amplitude = Math.min(
                  p.sphereRadius,
                  p.vortexStrength / Math.max(1, omega * omega),
                );
              local[0] +=
                amplitude * (Math.sin(phi + age * omega) - Math.sin(phi));
              local[2] +=
                amplitude * (Math.cos(phi + age * omega) - Math.cos(phi));
              tangent[0] += amplitude * omega * Math.cos(phi + age * omega);
              tangent[2] -= amplitude * omega * Math.sin(phi + age * omega);
              for (let axis = 0; axis < 3; axis++) {
                const freq = 0.37 + axis * 0.071,
                  phase =
                    random(seed, 15 + axis) * Math.PI * 2 +
                    offset[axis] * p.noiseFrequency,
                  a = Math.min(
                    p.sphereRadius * 0.25,
                    p.noiseStrength / (1 + freq * freq),
                  );
                local[axis] +=
                  a * (Math.sin(phase + freq * age) - Math.sin(phase));
                tangent[axis] += a * freq * Math.cos(phase + freq * age);
              }
            }
            const world = [0, 1, 2].map(
                (axis) =>
                  m[axis] * local[0] +
                  m[4 + axis] * local[1] +
                  m[8 + axis] * local[2] +
                  m[12 + axis],
              ),
              forward = normalized(
                [0, 1, 2].map(
                  (axis) =>
                    m[axis] * tangent[0] +
                    m[4 + axis] * tangent[1] +
                    m[8 + axis] * tangent[2],
                ),
              );
            const ownerUp = normalized([m[4], m[5], m[6]], [0, 1, 0]),
              side = normalized(
                cross(
                  forward,
                  Math.abs(dot(forward, ownerUp)) > 0.99 ? [0, 0, 1] : ownerUp,
                ),
                [0, 0, 1],
              ),
              up = normalized(cross(side, forward), [0, 1, 0]);
            const scale =
                Math.hypot(m[0], m[1], m[2]) *
                mix(...p.scale, random(seed, 8)) *
                Math.max(0, particleCurve(p.scaleCurve, u)),
              particleAlpha = Math.max(
                0,
                particleCurve(p.alphaCurve, u) *
                  particleCurve(p.alphaMultiplierCurve, u),
              ),
              alpha = Math.max(0, Math.min(1, particleCurve(p.ditherCurve, u)));
            if (scale <= 0 || alpha <= 0 || particleAlpha < mesh.opacityCutoff)
              continue;
            const data = [
              ...world,
              scale,
              ...forward,
              mesh.flap.amplitude,
              ...up,
              mix(...mesh.flap.frequency, random(seed, 9)),
              ...p.color,
              alpha,
              mesh.flap.uvMultiplier,
              Number(mesh.doubleSided),
              Math.sin(
                ((time * mix(...mesh.flap.frequency, random(seed, 9))) % 1) *
                  Math.PI *
                  2,
              ),
              0,
            ];
            candidates.push({
              data,
              mesh: p.mesh,
              distance: world.reduce(
                (sum, v, axis) => sum + (v - eye[axis]) ** 2,
                0,
              ),
              key,
            });
          }
        }
    }
    candidates.sort(
      (a, b) =>
        a.distance - b.distance ||
        a.key - b.key ||
        a.mesh.localeCompare(b.mesh),
    );
    const retained = candidates.slice(0, this.budget);
    retained.sort(
      (a, b) =>
        a.mesh.localeCompare(b.mesh) ||
        b.distance - a.distance ||
        a.key - b.key,
    );
    const draws: { mesh: string; firstInstance: number; count: number }[] = [];
    for (let i = 0; i < retained.length; i++) {
      const c = retained[i];
      this.output.set(c.data, i * CREATURE_LIMITS.instanceFloats);
      let draw = draws.at(-1);
      if (!draw || draw.mesh !== c.mesh) {
        draw = { mesh: c.mesh, firstInstance: i, count: 0 };
        draws.push(draw);
      }
      draw.count++;
    }
    return {
      data: this.output.subarray(
        0,
        retained.length * CREATURE_LIMITS.instanceFloats,
      ),
      count: retained.length,
      dropped: candidates.length - retained.length,
      draws,
    };
  }
}

export interface CreatureFrame {
  sharedWind?: WindFieldSettings;
  mode: "day" | "night";
  time: number;
  viewProjection: ArrayLike<number>;
  eye: readonly number[];
  /** Surface-to-light vector and linear RGB illuminance, before storage scaling. */
  sunDirection: readonly number[];
  sunIlluminance: readonly number[];
  /** World-to-environment rotation; SH includes E/pi, source gains and constant/solar layers. */
  diffuseRotation: ArrayLike<number>;
  diffuseSH: ArrayLike<number>;
  /** Raw linear fallback E/pi used only when no SH is supplied. */
  ambient?: readonly number[];
  fog?: {
    profile?: "quick-mmorpg-v1";
    sourceScale?: number;
    density: number;
    color: readonly number[];
    heightRate?: number;
    volumeMedium?: FogVolumeMedium;
    volumeRange?: FogVolumeRange;
  };
  enabled?: boolean;
}
export const WGSL_CREATURES = `
${FOG_SEGMENT_WGSL}
struct Frame{view:mat4x4<f32>,rotation:mat4x4<f32>,eye:vec4<f32>,sun:vec4<f32>,sunColor:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,sh:array<vec4<f32>,9>};
@group(0)@binding(0)var<uniform>frame:Frame;
struct In{@location(0)position:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)center:vec4<f32>,@location(4)forward:vec4<f32>,@location(5)up:vec4<f32>,@location(6)color:vec4<f32>,@location(7)extra:vec4<f32>};
struct Out{@builtin(position)clip:vec4<f32>,@location(0)world:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)color:vec4<f32>,@location(3)@interpolate(flat)doubleSided:f32};
@vertex fn vs(v:In)->Out{
 let side=normalize(cross(v.forward.xyz,v.up.xyz));let basis=mat3x3(v.forward.xyz,v.up.xyz,side);
 let flap=vec3(0.0,v.extra.z*v.forward.w*clamp(v.uv.x*v.extra.x,0.0,1.0),0.0);
 let world=v.center.xyz+basis*(v.position*v.center.w)+flap;
 var o:Out;o.clip=frame.view*vec4(world,1.0);o.world=world;o.normal=normalize(basis*v.normal);o.color=v.color;o.doubleSided=v.extra.y;return o;
}
fn skyDiffuse(normal:vec3<f32>)->vec3<f32>{
 let n=normalize((frame.rotation*vec4(normal,0.0)).xyz);let b=array<f32,9>(.2820947918,.4886025119*n.y,.4886025119*n.z,.4886025119*n.x,1.0925484306*n.x*n.y,1.0925484306*n.y*n.z,.3153915653*(3.0*n.z*n.z-1.0),1.0925484306*n.x*n.z,.5462742153*(n.x*n.x-n.y*n.y));
 var color=frame.ambient.xyz;for(var i=0;i<9;i++){color+=frame.sh[i].xyz*b[i];}return max(color,vec3(0.0));
}
@fragment fn fs(v:Out,@builtin(front_facing)front:bool)->@location(0)vec4<f32>{
 if !front&&v.doubleSided<.5{discard;}let n=select(-v.normal,v.normal,front);
 let raw=v.color.rgb*(frame.sunColor.rgb*max(dot(n,frame.sun.xyz),0.0)/3.14159265359+skyDiffuse(n));
 let delta=v.world-frame.eye.xyz;let distance=length(delta);let fog=heightFogComposition(frame.eye.y,delta.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4((raw*fog.x+frame.fogColor.rgb*fog.y)*${SCENE_LINEAR_SCALE},v.color.a);
}
`;
async function fetchBlob(
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
          "Prepare the private creature geometry before opening this world.",
        );
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let length = 0;
      try {
        for (;;) {
          const next = await reader.read();
          if (next.done) break;
          length += next.value.byteLength;
          if (length > limit)
            throw new Error("Creature geometry exceeds its byte budget.");
          chunks.push(next.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      const bytes = new Uint8Array(length);
      let at = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, at);
        at += chunk.byteLength;
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
export async function checkedCreatureBlob(
  description: CreatureBlob,
  load = fetchBlob,
) {
  blob(description);
  const bytes = await load(
    description.url,
    description.byteLength,
    description,
  );
  if (bytes.byteLength !== description.byteLength)
    throw new Error("Prepared creature geometry has an incorrect byte count.");
  const actual = [
    ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
  ]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
  if (actual !== description.sha256)
    throw new Error("Prepared creature geometry changed; prepare it again.");
  return bytes;
}
/** Independent 4x-MSAA mesh pipeline; call inside the main HDR pass before transparency. */
export class Creatures {
  readonly bytes: number;
  private sampler: CreatureSampler;
  private uniform: GPUBuffer;
  private instances: GPUBuffer;
  private group: GPUBindGroup;
  private settings = new Float32Array(100);
  private meshes: Map<string, CreatureMesh>;
  private constructor(
    private device: GPUDevice,
    manifest: CreaturesManifest,
    private pipeline: GPURenderPipeline,
    private vertices: GPUBuffer,
    private indices: GPUBuffer,
  ) {
    this.bytes =
      manifest.geometry.vertices.byteLength +
      manifest.geometry.indices.byteLength +
      CREATURE_LIMITS.creatures * 80 +
      400;
    this.sampler = new CreatureSampler(manifest);
    this.meshes = new Map(manifest.meshes.map((m) => [m.id, m]));
    this.uniform = device.createBuffer({
      size: 400,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    try {
      this.instances = device.createBuffer({
        size: CREATURE_LIMITS.creatures * 80,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      try {
        this.group = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: { buffer: this.uniform } }],
        });
      } catch (error) {
        this.instances.destroy();
        throw error;
      }
    } catch (error) {
      this.uniform.destroy();
      throw error;
    }
  }
  static async create(
    device: GPUDevice,
    manifest: CreaturesManifest,
    load = fetchBlob,
  ) {
    validateCreatures(manifest);
    const vertexBytes = await checkedCreatureBlob(
        manifest.geometry.vertices,
        load,
      ),
      indexBytes = await checkedCreatureBlob(manifest.geometry.indices, load),
      floats = new Float32Array(vertexBytes),
      elements = new Uint32Array(indexBytes);
    for (let at = 0; at < floats.length; at++)
      number(floats[at], -10000, 10000);
    for (const index of elements)
      if (index >= manifest.geometry.vertexCount)
        throw new Error(
          "Prepared creature triangle indices leave the vertex buffer.",
        );
    for (let at = 0; at < floats.length; at += 8)
      if (Math.hypot(floats[at + 3], floats[at + 4], floats[at + 5]) < 0.5)
        throw new Error("Prepared creatures need usable source normals.");
    const module = device.createShaderModule({ code: WGSL_CREATURES });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 32,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x3" },
              { shaderLocation: 2, offset: 24, format: "float32x2" },
            ],
          },
          {
            arrayStride: 80,
            stepMode: "instance",
            attributes: [3, 4, 5, 6, 7].map((location) => ({
              shaderLocation: location,
              offset: (location - 3) * 16,
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
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "less-equal",
      },
      multisample: { count: 4, alphaToCoverageEnabled: true },
    });
    const owned: GPUBuffer[] = [];
    try {
      const vertices = device.createBuffer({
        size: vertexBytes.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      owned.push(vertices);
      device.queue.writeBuffer(vertices, 0, vertexBytes);
      const indices = device.createBuffer({
        size: indexBytes.byteLength,
        usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
      });
      owned.push(indices);
      device.queue.writeBuffer(indices, 0, indexBytes);
      return new Creatures(device, manifest, pipeline, vertices, indices);
    } catch (error) {
      owned.forEach((buffer) => buffer.destroy());
      throw error;
    }
  }
  render(pass: GPURenderPassEncoder, frame: CreatureFrame) {
    const sampled = this.sampler.sample(frame.mode, frame.time, frame.eye);
    if (frame.enabled === false || sampled.count === 0)
      return { count: 0, dropped: sampled.dropped };
    for (const [values, length] of [
      [frame.viewProjection, 16],
      [frame.diffuseRotation, 16],
      [frame.diffuseSH, 36],
    ] as const) {
      if (values.length !== length)
        throw new Error(
          "Creature lighting needs complete matrices and SH coefficients.",
        );
      for (let i = 0; i < length; i++) number(values[i], -1e9, 1e9);
    }
    vector(frame.sunDirection, 3, -1, 1);
    vector(frame.sunIlluminance, 3, 0, 1e6);
    vector(frame.ambient ?? [0, 0, 0], 3, 0, 1e6);
    if (Math.abs(Math.hypot(...frame.sunDirection) - 1) > 0.001)
      throw new Error("Creature lighting needs a normalized sun direction.");
    const u = this.settings;
    u.set(frame.viewProjection, 0);
    u.set(frame.diffuseRotation, 16);
    u.set([...frame.eye, frame.time], 32);
    u.set([...frame.sunDirection, 0], 36);
    u.set([...frame.sunIlluminance, 0], 40);
    u.set([...(frame.ambient ?? [0, 0, 0]), 0], 44);
    if (frame.fog) {
      number(frame.fog.density, 0, 10000);
      number(frame.fog.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
      vector(frame.fog.color, 3, 0, 1e6);
    }
    u.set(
      [frame.fog?.density ?? 0, frame.fog?.heightRate ?? FOG_HEIGHT_RATE, 0, 0],
      48,
    );
    u.set([...(frame.fog?.color ?? [0, 0, 0]), 0], 52);
    u.set(frame.fog?.volumeMedium ?? [0, 0, 1, 0], 56);
    u.set([...(frame.fog?.volumeRange ?? [0, 0]), 0, 0], 60);
    u.set(frame.diffuseSH, 64);
    this.device.queue.writeBuffer(this.uniform, 0, u);
    this.device.queue.writeBuffer(this.instances, 0, sampled.data);
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.setVertexBuffer(0, this.vertices);
    pass.setVertexBuffer(1, this.instances);
    pass.setIndexBuffer(this.indices, "uint32");
    for (const draw of sampled.draws) {
      const mesh = this.meshes.get(draw.mesh)!;
      pass.drawIndexed(
        mesh.indexCount,
        draw.count,
        mesh.firstIndex,
        0,
        draw.firstInstance,
      );
    }
    return { count: sampled.count, dropped: sampled.dropped };
  }
  dispose() {
    this.vertices.destroy();
    this.indices.destroy();
    this.uniform.destroy();
    this.instances.destroy();
  }
}
