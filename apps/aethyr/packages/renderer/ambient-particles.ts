import { readAsset, verifyAssetBytes } from "../assets/src/asset-download.ts";
import { decodeRgba8 } from "../assets/src/rgba.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_SEGMENT_WGSL,
  heightFogComposition,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import {
  sampleWorldWindDisplacement,
  validateWindField,
  type WindFieldSettings,
} from "./shared-wind.ts";

export const AMBIENT_PARTICLE_LIMITS = Object.freeze({
  particles: 4096,
  candidates: 16384,
  emitters: 128,
  profiles: 3,
  atlasBytes: 4096 * 4096 * 4 + 16,
  stride: 16,
});
/**
 * Inherited function-input defaults recovered from UE 5.8 asset metadata, not
 * authored material overrides. Epic describes radius and density here:
 * https://dev.epicgames.com/documentation/en-us/unreal-engine/gradient-material-functions-in-unreal-engine
 * The recovered expression graph uses squared exponential density. This owned
 * numerical reconstruction does not reproduce the engine's full material path.
 */
export const DUST_RADIAL_RADIUS = 0.5;
export const DUST_RADIAL_DENSITY = 2.333;
export function dustRadialMask(uv: readonly number[]) {
  vector(uv, 2, -1e6, 1e6);
  const depth = Math.max(
    0,
    1 - Math.hypot(uv[0] - 0.5, uv[1] - 0.5) / DUST_RADIAL_RADIUS,
  );
  return -Math.expm1(-((DUST_RADIAL_DENSITY * depth) ** 2));
}
// Fixed, bounded Simpson quadrature over the finite disk, evaluated at module
// load, never per frame. Independent opacity/emission integrals preserve source
// transmittance and flux for subpixel sprites without a display-gain adjustment.
function radialIntegral(power: 1 | 2) {
  const steps = 2048;
  let sum = 0;
  for (let i = 0; i <= steps; i++) {
    const radius = (i * DUST_RADIAL_RADIUS) / steps;
    const mask = -Math.expm1(
      -((DUST_RADIAL_DENSITY * (1 - radius / DUST_RADIAL_RADIUS)) ** 2),
    );
    const weight = i === 0 || i === steps ? 1 : i % 2 ? 4 : 2;
    sum += weight * 2 * Math.PI * radius * mask ** power;
  }
  return (sum * DUST_RADIAL_RADIUS) / (3 * steps);
}
export const DUST_RADIAL_PRODUCT_INTEGRAL = radialIntegral(2);
export const DUST_RADIAL_OPACITY_INTEGRAL = radialIntegral(1);
export interface ParticleCurveKey {
  time: number;
  value: number;
  mode?: "linear" | "cubic" | "constant";
  arriveTangent?: number;
  leaveTangent?: number;
}
export interface AmbientParticleProfile {
  id: string;
  kind: "dust" | "smoke";
  spawnRate: number;
  lifetime: [number, number];
  size: [number, number];
  velocityMin: [number, number, number];
  velocityMax: [number, number, number];
  acceleration: [number, number, number];
  drag: [number, number];
  color: [number, number, number];
  emission: number;
  sphereRadius: number;
  boxSize: [number, number, number];
  shapeOffset: [number, number, number];
  /** World-wind displacement multiplier; absent recipes use 0.15 when wind is supplied. */
  windResponse?: number;
  noiseStrength: number;
  /** Source spatial frequency; the owned analytic drift does not execute curl fields. */
  noiseFrequency: number;
  alphaCurve: ParticleCurveKey[];
  sizeCurve: ParticleCurveKey[];
  phaseCurve?: ParticleCurveKey[];
  depthFade: number;
  tint?: [number, number, number];
  colorPower?: number;
}
export interface AmbientParticleEmitter {
  id: string;
  profile: string;
  /** Column-major affine owner transform, meters and Y up. */
  model: number[];
  /** Omission preserves the original owned steady-state startup. */
  lifecycle?: AmbientParticleStart;
}
/** Continuous zero-warmup emission; other source lifecycle modes need qualification. */
export interface AmbientParticleStart {
  schema: "aethyrforge.particle-start.v1";
  startSeconds: number;
  warmupSeconds: 0;
  delaySeconds: number;
  loop: "continuous";
}
export interface AmbientParticleManifest {
  schema: "aethyrforge.ambient-particles.v1";
  profiles: AmbientParticleProfile[];
  scenarios: Record<"day" | "night", AmbientParticleEmitter[]>;
  atlas?: {
    url: string;
    sha256: string;
    byteLength: number;
    width: number;
    height: number;
    grid: [number, number];
    colorSpace: "srgb";
  };
  metadata?: Record<string, unknown>;
}

function finite(v: number, lo: number, hi: number) {
  if (!Number.isFinite(v) || v < lo || v > hi)
    throw new Error("Prepared particles contain an unsupported number.");
}
function vector(v: readonly number[], n: number, lo: number, hi: number) {
  if (!Array.isArray(v) || v.length !== n)
    throw new Error("Prepared particles contain an incomplete vector.");
  for (const x of v) finite(x, lo, hi);
}
function curveChecked(v: ParticleCurveKey[]) {
  if (!Array.isArray(v) || v.length < 1 || v.length > 32)
    throw new Error("Prepared particle age curves need one to 32 keys.");
  let previous = -Infinity;
  for (const key of v) {
    finite(key.time, -1, 2);
    finite(key.value, 0, 16);
    finite(key.arriveTangent ?? 0, -1000, 1000);
    finite(key.leaveTangent ?? 0, -1000, 1000);
    if (
      key.time <= previous ||
      ![undefined, "linear", "cubic", "constant"].includes(key.mode)
    )
      throw new Error("Prepared particle curve keys must be ordered.");
    previous = key.time;
  }
}
export function validateAmbientParticles(value: AmbientParticleManifest) {
  if (
    value?.schema !== "aethyrforge.ambient-particles.v1" ||
    !Array.isArray(value.profiles) ||
    value.profiles.length > AMBIENT_PARTICLE_LIMITS.profiles
  )
    throw new Error("Prepare a supported ambient particle recipe.");
  const profiles = new Map<string, AmbientParticleProfile>();
  for (const p of value.profiles) {
    if (
      typeof p.id !== "string" ||
      !p.id ||
      p.id.length > 256 ||
      profiles.has(p.id) ||
      !["dust", "smoke"].includes(p.kind)
    )
      throw new Error(
        "Prepared particle profiles need unique supported identities.",
      );
    finite(p.spawnRate, 0.01, 100);
    vector(p.lifetime, 2, 0.01, 60);
    vector(p.size, 2, 0.00001, 20);
    vector(p.drag, 2, 0, 100);
    if (
      p.lifetime[0] > p.lifetime[1] ||
      p.size[0] > p.size[1] ||
      p.drag[0] > p.drag[1]
    )
      throw new Error("Prepared particle ranges must be ordered.");
    for (const v of [p.velocityMin, p.velocityMax, p.acceleration])
      vector(v, 3, -100, 100);
    vector(p.boxSize, 3, 0, 100);
    vector(p.shapeOffset, 3, -100, 100);
    vector(p.color, 3, 0, 16);
    if (p.tint) vector(p.tint, 3, 0, 16);
    finite(p.emission, 0, 1e6);
    finite(p.sphereRadius, 0, 100);
    finite(p.windResponse ?? 0.15, 0, 2);
    finite(p.noiseStrength, 0, 100);
    finite(p.noiseFrequency, 0, 1000);
    finite(p.depthFade, 0, 10);
    finite(p.colorPower ?? 0.5, 0.01, 16);
    curveChecked(p.alphaCurve);
    curveChecked(p.sizeCurve);
    if (p.phaseCurve) curveChecked(p.phaseCurve);
    profiles.set(p.id, p);
  }
  if (value.profiles.filter((p) => p.kind === "smoke").length > 1)
    throw new Error("This particle profile supports one smoke atlas family.");
  for (const mode of ["day", "night"] as const) {
    const emitters = value.scenarios?.[mode];
    if (
      !Array.isArray(emitters) ||
      emitters.length > AMBIENT_PARTICLE_LIMITS.emitters
    )
      throw new Error("Prepared particles exceed the emitter budget.");
    let candidates = 0;
    const ids = new Set<string>();
    for (const e of emitters) {
      if (
        typeof e.id !== "string" ||
        !e.id ||
        e.id.length > 2048 ||
        ids.has(e.id) ||
        !profiles.has(e.profile)
      )
        throw new Error(
          "Prepared particle emitters need unique identities and known profiles.",
        );
      ids.add(e.id);
      if (e.lifecycle !== undefined) {
        const life = e.lifecycle;
        if (
          !life ||
          typeof life !== "object" ||
          Array.isArray(life) ||
          Object.keys(life).sort().join() !==
            ["schema", "startSeconds", "warmupSeconds", "delaySeconds", "loop"]
              .sort()
              .join() ||
          life.schema !== "aethyrforge.particle-start.v1" ||
          life.warmupSeconds !== 0 ||
          life.loop !== "continuous"
        )
          throw new Error(
            "Prepared particles need a supported continuous zero-warmup start.",
          );
        finite(life.startSeconds, 0, 86400);
        finite(life.delaySeconds, 0, 86400);
        finite(life.startSeconds + life.delaySeconds, 0, 86400);
      }
      vector(e.model, 16, -1e6, 1e6);
      if (
        Math.abs(e.model[3]) + Math.abs(e.model[7]) + Math.abs(e.model[11]) >
          1e-8 ||
        Math.abs(e.model[15] - 1) > 1e-8
      )
        throw new Error("Prepared particles need affine owner transforms.");
      for (let axis = 0; axis < 3; axis++) {
        const scale = Math.hypot(
          e.model[axis * 4],
          e.model[axis * 4 + 1],
          e.model[axis * 4 + 2],
        );
        finite(scale, 0.001, 100);
      }
      const p = profiles.get(e.profile)!;
      candidates += Math.ceil(p.spawnRate * p.lifetime[1]) + 1;
    }
    if (candidates > AMBIENT_PARTICLE_LIMITS.candidates)
      throw new Error("Prepared particles exceed the candidate budget.");
  }
  if (value.atlas) {
    const a = value.atlas;
    if (
      typeof a.url !== "string" ||
      !/^\/private-assets\/[A-Za-z0-9._-]+\.bin$/.test(a.url) ||
      !/^[a-f0-9]{64}$/.test(a.sha256) ||
      a.colorSpace !== "srgb"
    )
      throw new Error("Prepared particles need an authorized private atlas.");
    for (const v of [a.width, a.height])
      if (!Number.isInteger(v) || v < 8 || v > 4096)
        throw new Error("Prepared particle atlas dimensions are invalid.");
    if (
      a.byteLength !== 16 + a.width * a.height * 4 ||
      a.byteLength > AMBIENT_PARTICLE_LIMITS.atlasBytes ||
      a.grid?.[0] !== 8 ||
      a.grid?.[1] !== 8 ||
      a.width % 8 ||
      a.height % 8
    )
      throw new Error("Prepared smoke needs a bounded eight-by-eight atlas.");
  }
  if (value.profiles.some((p) => p.kind === "smoke") && !value.atlas)
    throw new Error("Prepare the smoke atlas before using smoke particles.");
  return value;
}

export function particleCurve(keys: readonly ParticleCurveKey[], age: number) {
  if (age <= keys[0].time) return keys[0].value;
  for (let i = 1; i < keys.length; i++) {
    const b = keys[i],
      a = keys[i - 1];
    if (age > b.time) continue;
    const span = b.time - a.time,
      t = (age - a.time) / span;
    if (a.mode === "constant") return a.value;
    if (a.mode !== "cubic") return a.value + (b.value - a.value) * t;
    const t2 = t * t,
      t3 = t2 * t;
    return Math.max(
      0,
      Math.min(
        16,
        (2 * t3 - 3 * t2 + 1) * a.value +
          (t3 - 2 * t2 + t) * span * (a.leaveTangent ?? 0) +
          (-2 * t3 + 3 * t2) * b.value +
          (t3 - t2) * span * (b.arriveTangent ?? 0),
      ),
    );
  }
  return keys.at(-1)!.value;
}
function hash(v: number) {
  v = Math.imul(v ^ (v >>> 16), 0x7feb352d);
  v = Math.imul(v ^ (v >>> 15), 0x846ca68b);
  return (v ^ (v >>> 16)) >>> 0;
}
function identitySeed(v: string) {
  let h = 2166136261;
  for (let i = 0; i < v.length; i++)
    h = Math.imul(h ^ v.charCodeAt(i), 16777619);
  return hash(h);
}
const random = (seed: number, channel: number) =>
  hash(seed ^ Math.imul(channel + 1, 0x9e3779b9)) / 4294967296;
const mix = (a: number, b: number, t: number) => a + (b - a) * t;
const DRIFT_OMEGA = [0, 1, 2].map(
  (axis) => 2 * Math.PI * (0.17 + axis * 0.031),
);

/** Constant acceleration with linear drag, evaluated without integrating frames. */
export function particleDisplacement(
  age: number,
  velocity: number,
  acceleration: number,
  drag: number,
) {
  if (drag < 1e-5) return velocity * age + acceleration * age * age * 0.5;
  const decay = -Math.expm1(-drag * age);
  return (
    (velocity * decay) / drag + (acceleration / drag) * (age - decay / drag)
  );
}

/** Bounded reusable storage; legacy recipes retain their owned steady-state warmup. */
export class AmbientParticleSampler {
  readonly manifest: AmbientParticleManifest;
  private candidates = new Float32Array(
    AMBIENT_PARTICLE_LIMITS.candidates * 16,
  );
  private output = new Float32Array(AMBIENT_PARTICLE_LIMITS.particles * 16);
  private distances = new Float64Array(AMBIENT_PARTICLE_LIMITS.candidates);
  private depths = new Float64Array(AMBIENT_PARTICLE_LIMITS.candidates);
  private local = new Float64Array(3);
  private windDisplacement = new Float64Array(3);
  private order: number[] = [];
  private profiles: Map<string, AmbientParticleProfile>;
  constructor(
    manifest: AmbientParticleManifest,
    readonly budget: number = AMBIENT_PARTICLE_LIMITS.particles,
  ) {
    validateAmbientParticles(manifest);
    if (
      !Number.isInteger(budget) ||
      budget < 0 ||
      budget > AMBIENT_PARTICLE_LIMITS.particles
    )
      throw new Error("Use a supported ambient particle budget.");
    this.manifest = structuredClone(manifest);
    this.profiles = new Map(this.manifest.profiles.map((p) => [p.id, p]));
  }
  sample(
    mode: "day" | "night",
    time: number,
    eye: readonly number[],
    forward: readonly number[] = [0, 0, -1],
    sharedWind?: WindFieldSettings,
  ) {
    finite(time, 0, MAX_WORLD_ANIMATION_TIME);
    if (sharedWind) validateWindField(sharedWind);
    vector(eye, 3, -1e6, 1e6);
    vector(forward, 3, -1, 1);
    if (Math.abs(Math.hypot(...forward) - 1) > 0.001)
      throw new Error("Particle ordering needs a normalized camera direction.");
    if (mode !== "day" && mode !== "night")
      throw new Error("Choose a prepared particle scenario.");
    let count = 0;
    for (const emitter of this.manifest.scenarios[mode]) {
      const p = this.profiles.get(emitter.profile)!,
        m = emitter.model,
        base = identitySeed(emitter.id);
      const localTime = emitter.lifecycle
        ? time - emitter.lifecycle.startSeconds - emitter.lifecycle.delaySeconds
        : time;
      if (emitter.lifecycle && localTime <= 0) continue;
      const phase = emitter.lifecycle ? 0 : random(base, 0) / p.spawnRate,
        head = Math.floor((localTime + phase) * p.spawnRate),
        capacity = Math.ceil(p.spawnRate * p.lifetime[1]) + 1;
      const sizeScale = Math.cbrt(
        Math.hypot(m[0], m[1], m[2]) *
          Math.hypot(m[4], m[5], m[6]) *
          Math.hypot(m[8], m[9], m[10]),
      );
      for (let slot = 0; slot < capacity; slot++) {
        const birth = head - slot;
        if (emitter.lifecycle && birth < 1) break;
        const seed = hash(base ^ Math.imul(birth, 0x27d4eb2d));
        const age = localTime - birth / p.spawnRate + phase,
          life = mix(p.lifetime[0], p.lifetime[1], random(seed, 1));
        if (age < 0 || age >= life) continue;
        const u = age / life,
          azimuth = random(seed, 2) * Math.PI * 2,
          y = random(seed, 3) * 2 - 1;
        const radius = Math.cbrt(random(seed, 4)) * p.sphereRadius,
          radial = Math.sqrt(Math.max(0, 1 - y * y));
        const local = this.local;
        local[0] = Math.cos(azimuth) * radial * radius;
        local[1] = y * radius;
        local[2] = Math.sin(azimuth) * radial * radius;
        const drag = mix(p.drag[0], p.drag[1], random(seed, 5)),
          noDrag = drag < 1e-5,
          decay = noDrag ? 0 : -Math.expm1(-drag * age),
          accelerationAge = noDrag ? 0 : age - decay / drag,
          dragSquared = drag * drag;
        for (let axis = 0; axis < 3; axis++) {
          local[axis] +=
            (random(seed, 6 + axis) - 0.5) * p.boxSize[axis] +
            p.shapeOffset[axis];
          const velocity = mix(
            p.velocityMin[axis],
            p.velocityMax[axis],
            random(seed, 9 + axis),
          );
          local[axis] += noDrag
            ? velocity * age + p.acceleration[axis] * age * age * 0.5
            : (velocity * decay) / drag +
              (p.acceleration[axis] / drag) * accelerationAge;
          // Owned periodic drift replaces the source spatial curl-field solver.
          const omega = DRIFT_OMEGA[axis],
            phi =
              random(seed, 12 + axis) * Math.PI * 2 +
              local[axis] * p.noiseFrequency;
          local[axis] +=
            (p.noiseStrength / Math.max(0.1, dragSquared + omega * omega)) *
            (Math.sin(phi + age * omega) - Math.sin(phi));
        }
        const wind = this.windDisplacement;
        if (sharedWind && (p.windResponse ?? 0.15) > 0)
          sampleWorldWindDisplacement(
            [m[12], m[13], m[14]],
            time - age,
            time,
            sharedWind,
            wind,
          );
        else wind.fill(0);
        const response = p.windResponse ?? 0.15;
        const offset = count * 16,
          x =
            m[0] * local[0] +
            m[4] * local[1] +
            m[8] * local[2] +
            m[12] +
            wind[0] * response,
          py = m[1] * local[0] + m[5] * local[1] + m[9] * local[2] + m[13],
          z =
            m[2] * local[0] +
            m[6] * local[1] +
            m[10] * local[2] +
            m[14] +
            wind[2] * response;
        const out = this.candidates;
        out[offset] = x;
        out[offset + 1] = py;
        out[offset + 2] = z;
        out[offset + 3] =
          mix(p.size[0], p.size[1], random(seed, 15)) *
          sizeScale *
          particleCurve(p.sizeCurve, u);
        for (let channel = 0; channel < 3; channel++)
          out[offset + 4 + channel] = p.color[channel];
        out[offset + 7] = Math.min(1, particleCurve(p.alphaCurve, u));
        out[offset + 8] = random(seed, 16) * Math.PI * 2;
        out[offset + 9] = p.phaseCurve ? particleCurve(p.phaseCurve, u) : u;
        out[offset + 10] = Number(p.kind === "smoke");
        out[offset + 11] = p.depthFade;
        out[offset + 12] = p.emission;
        for (let channel = 0; channel < 3; channel++)
          out[offset + 13 + channel] = p.tint?.[channel] ?? 1;
        this.distances[count] =
          (x - eye[0]) ** 2 + (py - eye[1]) ** 2 + (z - eye[2]) ** 2;
        this.depths[count] =
          (x - eye[0]) * forward[0] +
          (py - eye[1]) * forward[1] +
          (z - eye[2]) * forward[2];
        this.order[count] = count;
        count++;
      }
    }
    this.order.length = count;
    // Keep nearest candidates under pressure, then compose back to front. Stable
    // candidate indices settle equal distances without changing emitter seeds.
    const retained = Math.min(count, this.budget);
    if (count > retained)
      this.order.sort((a, b) => this.distances[a] - this.distances[b] || a - b);
    this.order.length = retained;
    this.order.sort((a, b) => this.depths[b] - this.depths[a] || a - b);
    let dust = 0,
      smoke = 0;
    for (let i = 0; i < retained; i++) {
      const index = this.order[i];
      for (let field = 0; field < 16; field++)
        this.output[i * 16 + field] = this.candidates[index * 16 + field];
      if (this.output[i * 16 + 10] > 0.5) smoke++;
      else dust++;
    }
    return {
      data: this.output.subarray(0, retained * 16),
      count: retained,
      dust,
      smoke,
      dropped: count - retained,
    };
  }
}

/** Separate pipeline: no bindings or texture slots are added to surface materials. */
export const WGSL_AMBIENT_PARTICLES = `
${FOG_SEGMENT_WGSL}
struct ParticleFrame{view:mat4x4<f32>,inverse:mat4x4<f32>,eye:vec4<f32>,settings:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:ParticleFrame;
@group(0)@binding(1)var opaqueDepth:texture_depth_multisampled_2d;
@group(0)@binding(2)var smokeAtlas:texture_2d<f32>;
@group(0)@binding(3)var atlasSampler:sampler;
struct ParticleIn{@location(0)center:vec4<f32>,@location(1)color:vec4<f32>,@location(2)shape:vec4<f32>,@location(3)extra:vec4<f32>};
struct ParticleOut{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>,@location(1)color:vec4<f32>,@location(2)shape:vec4<f32>,@location(3)extra:vec4<f32>,@location(4)world:vec3<f32>,@location(5)@interpolate(flat)footprint:vec4<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32,p:ParticleIn)->ParticleOut{
 let corners=array<vec2<f32>,6>(vec2(-.5,-.5),vec2(.5,-.5),vec2(-.5,.5),vec2(-.5,.5),vec2(.5,-.5),vec2(.5,.5));
 let corner=corners[i];let cs=cos(p.shape.x);let sn=sin(p.shape.x);let q=select(corner,vec2(corner.x*cs-corner.y*sn,corner.x*sn+corner.y*cs),p.shape.z>.5);
 let right=normalize(frame.inverse[0].xyz);let up=normalize(frame.inverse[1].xyz);
 let centerClip=frame.view*vec4(p.center.xyz,1.0);
 let rightClip=frame.view*vec4(right*p.center.w,0.0);let upClip=frame.view*vec4(up*p.center.w,0.0);
 let pixelScale=frame.settings.xy*.5/max(abs(centerClip.w),.000001);
 let diameter=sqrt(length(rightClip.xy*pixelScale)*length(upClip.xy*pixelScale));
 let expand=select(max(1.0,2.0/max(diameter,.000001)),1.0,p.shape.z>.5);
 let world=p.center.xyz+(right*q.x+up*q.y)*p.center.w*expand;
 var out:ParticleOut;out.position=frame.view*vec4(world,1.0);out.world=world;out.uv=vec2(corner.x*expand+.5,.5-corner.y*expand);out.color=p.color;out.shape=p.shape;out.extra=p.extra;
 out.footprint=vec4((centerClip.xy/centerClip.w*vec2(.5,-.5)+vec2(.5))*frame.settings.xy,diameter,0.0);return out;
}
fn atlasUV(uv:vec2<f32>,index:f32)->vec2<f32>{let cell=vec2(i32(index)%8,i32(index)/8);let size=vec2<f32>(textureDimensions(smokeAtlas));return (vec2<f32>(cell)+clamp(uv,vec2(4.0)/size,vec2(1.0)-vec2(4.0)/size))/8.0;}
fn dustMask(uv:vec2<f32>)->f32{
 let depth=max(0.0,1.0-length(uv-vec2(.5))/${DUST_RADIAL_RADIUS});
 let density=${DUST_RADIAL_DENSITY}*depth;
 return max(0.0,1.0-exp(-density*density));
}
fn dustMasks(uv:vec2<f32>)->vec2<f32>{
 let emissionMask=dustMask(uv);let opacityMask=dustMask(uv);
 return vec2(emissionMask*opacityMask,opacityMask);
}
fn filteredDustMasks(v:ParticleOut,uvX:vec2<f32>,uvY:vec2<f32>)->vec2<f32>{
 let q=abs(v.position.xy-v.footprint.xy);
 let pointWeight=max(0.0,1.0-q.x)*max(0.0,1.0-q.y);
 let pointEnergy=vec2(${DUST_RADIAL_PRODUCT_INTEGRAL},${DUST_RADIAL_OPACITY_INTEGRAL})*v.footprint.z*v.footprint.z*pointWeight;
 if v.footprint.z<=1.0{return pointEnergy;}
 if v.footprint.z>=4.0{return dustMasks(v.uv);}
 var integral=vec2(0.0);
 for(var y=0;y<4;y++){for(var x=0;x<4;x++){
  let offset=(vec2(f32(x),f32(y))+vec2(.5))*.25-vec2(.5);
  integral+=dustMasks(v.uv+uvX*offset.x+uvY*offset.y)*.0625;
 }}
 return mix(pointEnergy,integral,smoothstep(1.0,2.0,v.footprint.z));
}
@fragment fn fs(v:ParticleOut)->@location(0)vec4<f32>{
 let uvX=dpdx(v.uv);let uvY=dpdy(v.uv);
 let pixel=clamp(vec2<i32>(v.position.xy),vec2(0),vec2<i32>(textureDimensions(opaqueDepth))-vec2(1));
 var depth=1.0;for(var sample=0;sample<4;sample++){depth=min(depth,textureLoad(opaqueDepth,pixel,sample));}
 if v.position.z>depth+0.000001{discard;}
 let ray=v.world-frame.eye.xyz;let distance=length(ray);
 let clip=vec4(v.position.x/frame.settings.x*2.0-1.0,1.0-v.position.y/frame.settings.y*2.0,depth,1.0);
 let opaque=frame.inverse*clip;let opaqueWorld=opaque.xyz/opaque.w;
 var fade=1.0;if depth<.999999&&v.shape.w>0.0{fade=clamp((length(opaqueWorld-frame.eye.xyz)-distance)/v.shape.w,0.0,1.0);}
 var alpha=v.color.a*fade;var color=v.color.rgb;var outputAlpha=0.0;
 if v.shape.z>.5{
  let phase=clamp(v.shape.y,0.0,1.0)*63.0;let a=floor(phase);let b=min(a+1.0,63.0);
  let tex=mix(textureSampleLevel(smokeAtlas,atlasSampler,atlasUV(v.uv,a),0.0),textureSampleLevel(smokeAtlas,atlasSampler,atlasUV(v.uv,b),0.0),fract(phase));
  alpha*=tex.a;color*=mix(vec3(1.0),v.extra.yzw,pow(clamp(tex.r,0.0,1.0),frame.fog.z));
  // The source material also multiplies emissive by its depth-faded opacity,
  // independently of the translucent blend's opacity below.
  color*=alpha*v.extra.x*frame.settings.w/max(frame.settings.z,.000001);outputAlpha=alpha;
 }else{
  // Source dust is translucent. Filter emission*opacity independently of the
  // opacity that attenuates the background; age alpha appears once in each.
  let masks=filteredDustMasks(v,uvX,uvY);outputAlpha=alpha*masks.y;
  alpha*=masks.x;color*=v.extra.x*frame.settings.w;
 }
 // Analytical height extinction is an owned match to the surface fog path;
 // source shadowed volume scattering is not evaluated for these unlit sprites.
 let fog=heightFogComposition(frame.eye.y,(v.world.y-frame.eye.y)/max(distance,0.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4(color*fog.x*alpha+frame.fogColor.rgb*fog.y*outputAlpha,outputAlpha);
}
`;

export interface AmbientParticleFrame {
  sharedWind?: WindFieldSettings;
  mode: "day" | "night";
  time: number;
  width: number;
  height: number;
  colorView: GPUTextureView;
  depthView: GPUTextureView;
  viewProjection: ArrayLike<number>;
  inverseViewProjection: ArrayLike<number>;
  eye: readonly number[];
  /** Camera exposure without the scene-linear GPU storage scale. */
  effectiveExposure: number;
  fog?: {
    density: number;
    color: readonly number[];
    heightRate?: number;
    volumeMedium?: FogVolumeMedium;
    volumeRange?: FogVolumeRange;
  };
  enabled?: boolean;
}
async function atlasFetch(
  url: string,
  maxBytes: number,
  expected?: { byteLength: number; sha256: string },
) {
  return readAsset(
    url,
    async (response) => {
      if (
        !response.ok ||
        Number(response.headers.get("content-length")) > maxBytes
      )
        throw new Error(
          "Prepare the private smoke atlas before opening this world.",
        );
      if (!response.body)
        throw new Error("The particle atlas response is incomplete.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let length = 0;
      try {
        for (;;) {
          const next = await reader.read();
          if (next.done) break;
          length += next.value.byteLength;
          if (length > maxBytes)
            throw new Error("The particle atlas exceeds its byte budget.");
          chunks.push(next.value);
        }
      } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      const bytes = new Uint8Array(length);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
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
export class AmbientParticles {
  private readonly sampler: AmbientParticleSampler;
  private readonly uniform: GPUBuffer;
  private readonly instances: GPUBuffer;
  private readonly settings = new Float32Array(56);
  private readonly atlas: GPUTexture;
  private readonly atlasSampler: GPUSampler;
  private group?: GPUBindGroup;
  private depth?: GPUTextureView;
  private constructor(
    private device: GPUDevice,
    manifest: AmbientParticleManifest,
    private pipeline: GPURenderPipeline,
    atlas: GPUTexture,
  ) {
    this.sampler = new AmbientParticleSampler(manifest);
    this.uniform = device.createBuffer({
      size: 56 * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.instances = device.createBuffer({
      size: AMBIENT_PARTICLE_LIMITS.particles * 64,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.atlas = atlas;
    this.atlasSampler = device.createSampler({
      minFilter: "linear",
      magFilter: "linear",
      addressModeU: "clamp-to-edge",
      addressModeV: "clamp-to-edge",
    });
  }
  static async create(
    device: GPUDevice,
    manifest: AmbientParticleManifest,
    load = atlasFetch,
  ) {
    validateAmbientParticles(manifest);
    let pixels: Uint8Array = new Uint8Array([255, 255, 255, 255]),
      width = 1,
      height = 1;
    if (manifest.atlas) {
      const bytes = await load(
        manifest.atlas.url,
        manifest.atlas.byteLength,
        manifest.atlas,
      );
      if (bytes.byteLength !== manifest.atlas.byteLength)
        throw new Error("The prepared smoke atlas has the wrong byte count.");
      const hash = [
        ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
      ]
        .map((v) => v.toString(16).padStart(2, "0"))
        .join("");
      if (hash !== manifest.atlas.sha256)
        throw new Error("The prepared smoke atlas changed; prepare it again.");
      const decoded = decodeRgba8(bytes);
      width = decoded.width;
      height = decoded.height;
      pixels = decoded.pixels;
      if (width !== manifest.atlas.width || height !== manifest.atlas.height)
        throw new Error("The prepared smoke atlas dimensions changed.");
    }
    if (
      width > device.limits.maxTextureDimension2D ||
      height > device.limits.maxTextureDimension2D
    )
      throw new Error("The smoke atlas exceeds this device's texture limit.");
    const module = device.createShaderModule({ code: WGSL_AMBIENT_PARTICLES });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 64,
            stepMode: "instance",
            attributes: [0, 1, 2, 3].map((location) => ({
              shaderLocation: location,
              offset: location * 16,
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
    });
    const atlas = device.createTexture({
      label: "Private smoke flipbook",
      size: [width, height],
      format: "rgba8unorm-srgb",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    try {
      device.queue.writeTexture(
        { texture: atlas },
        pixels,
        { bytesPerRow: width * 4 },
        [width, height],
      );
      return new AmbientParticles(device, manifest, pipeline, atlas);
    } catch (error) {
      atlas.destroy();
      throw error;
    }
  }
  render(encoder: GPUCommandEncoder, frame: AmbientParticleFrame) {
    finite(frame.effectiveExposure, 0.000001, 1e6);
    for (const v of [frame.width, frame.height])
      if (!Number.isInteger(v) || v < 1 || v > 16384)
        throw new Error("Use supported particle frame dimensions.");
    if (
      frame.viewProjection.length !== 16 ||
      frame.inverseViewProjection.length !== 16
    )
      throw new Error("Particle rendering needs complete camera matrices.");
    for (const matrix of [frame.viewProjection, frame.inverseViewProjection])
      for (let i = 0; i < 16; i++) finite(matrix[i], -1e9, 1e9);
    const inv = frame.inverseViewProjection;
    const w = inv[11] * 0.5 + inv[15];
    const forward = [0, 1, 2].map(
      (axis) => (inv[8 + axis] * 0.5 + inv[12 + axis]) / w - frame.eye[axis],
    );
    const norm = Math.hypot(...forward);
    const sampled = this.sampler.sample(
      frame.mode,
      frame.time,
      frame.eye,
      forward.map((v) => v / norm),
      frame.sharedWind,
    );
    if (frame.enabled === false || sampled.count === 0)
      return { count: 0, dust: 0, smoke: 0, dropped: sampled.dropped };
    const settings = this.settings;
    settings.set(frame.viewProjection, 0);
    settings.set(frame.inverseViewProjection, 16);
    settings.set([...frame.eye, frame.time], 32);
    settings.set(
      [frame.width, frame.height, frame.effectiveExposure, SCENE_LINEAR_SCALE],
      36,
    );
    if (frame.fog) {
      finite(frame.fog.density, 0, 10000);
      finite(frame.fog.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
      vector(frame.fog.color, 3, 0, 1e6);
    }
    settings.set(
      [
        frame.fog?.density ?? 0,
        frame.fog?.heightRate ?? FOG_HEIGHT_RATE,
        this.sampler.manifest.profiles.find((p) => p.kind === "smoke")
          ?.colorPower ?? 0.5,
        0,
      ],
      40,
    );
    settings.set([...(frame.fog?.color ?? [0, 0, 0]), 0], 44);
    settings.set(frame.fog?.volumeMedium ?? [0, 0, 1, 0], 48);
    settings.set([...(frame.fog?.volumeRange ?? [0, 0]), 0, 0], 52);
    heightFogComposition(
      frame.eye[1],
      0,
      0,
      frame.fog?.density ?? 0,
      frame.fog?.heightRate ?? FOG_HEIGHT_RATE,
      1,
      0,
      frame.fog?.volumeMedium ?? [0, 0, 1, 0],
      frame.fog?.volumeRange ?? [0, 0],
    );
    this.device.queue.writeBuffer(this.uniform, 0, settings);
    this.device.queue.writeBuffer(this.instances, 0, sampled.data);
    if (this.depth !== frame.depthView) {
      this.depth = frame.depthView;
      this.group = this.device.createBindGroup({
        layout: this.pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: this.uniform } },
          { binding: 1, resource: frame.depthView },
          { binding: 2, resource: this.atlas.createView() },
          { binding: 3, resource: this.atlasSampler },
        ],
      });
    }
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        { view: frame.colorView, loadOp: "load", storeOp: "store" },
      ],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group!);
    pass.setVertexBuffer(0, this.instances);
    pass.draw(6, sampled.count);
    pass.end();
    return {
      count: sampled.count,
      dust: sampled.dust,
      smoke: sampled.smoke,
      dropped: sampled.dropped,
    };
  }
  dispose() {
    this.uniform.destroy();
    this.instances.destroy();
    this.atlas.destroy();
  }
}
