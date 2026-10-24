/** Particle equations and profiles adapted from Quick_3D_MMORPG (MIT).
 * Source: blood-effect.js, sorceror-effect.js, particle-system.js, level-up-component.js.
 * See third_party/quick-mmorpg/NOTICE. World distances use the declared source scale.
 */
import {
  validateMmorpgActors,
  type MmorpgActorPose,
  type MmorpgImage,
} from "../assets/src/mmorpg.ts";
import { validMmoEvent, type MmoEvent } from "../runtime/mmorpg-protocol.ts";
import {
  checkedCharacterBlob,
  characterTextureBytes,
  characterTextureLevels,
  fetchCharacterBytes,
  validateCharacterPNG,
} from "./character.ts";
import type { CreatureFrame } from "./creatures.ts";
import { FOG_HEIGHT_RATE, FOG_SEGMENT_WGSL } from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

export const MMO_EFFECT_LIMITS = Object.freeze({
  particles: 4096,
  emitters: 128,
  pendingEvents: 128,
  instanceFloats: 12,
  particleFloats: 12,
  textureBytes: 8 * 1024 * 1024,
  uniformBytes: 176,
});
export type MmorpgParticleTextures = Record<
  "blood" | "fire" | "levelUp",
  MmorpgImage
>;
type V3 = [number, number, number];
export interface MmorpgParticleAnchors {
  Head: V3;
  Hips: V3;
  RightHandIndex1: V3;
  LeftHandIndex1: V3;
}
export type MmorpgParticleAttachments = ReadonlyMap<
  number,
  MmorpgParticleAnchors
>;
type Profile = 0 | 1 | 2 | 3 | 4 | 5;
const BLOOD = 0,
  HAND_FIRE = 1,
  HAND_SMOKE = 2,
  HIT_FIRE = 3,
  HIT_SMOKE = 4,
  LEVEL_UP = 5;
const TEXTURES = ["blood", "fire", "levelUp"] as const;
const textureOf = (profile: number) =>
  profile === BLOOD ? 0 : profile === LEVEL_UP ? 2 : 1;
interface Emitter {
  actor: number;
  anchor: keyof MmorpgParticleAnchors;
  profile: Profile;
  born: number;
  life: number;
  seed: number;
  emitted: number;
}
interface PendingEvent {
  event: MmoEvent;
  age: number;
}
const random = (seed: number, salt: number) => {
  let n = (seed ^ Math.imul(salt + 1, 0x9e3779b9)) >>> 0;
  n ^= n >>> 16;
  n = Math.imul(n, 0x7feb352d);
  n ^= n >>> 15;
  n = Math.imul(n, 0x846ca68b);
  n ^= n >>> 16;
  return (n >>> 0) / 4294967296;
};
const clamp = (v: number, a = 0, b = 1) => Math.min(b, Math.max(a, v));
function curve(t: number, keys: readonly (readonly [number, number])[]) {
  for (let i = 1; i < keys.length; i++)
    if (t <= keys[i][0]) {
      const [x, y] = keys[i - 1],
        [end, value] = keys[i];
      return y + clamp((t - x) / (end - x)) * (value - y);
    }
  return keys.at(-1)![1];
}
const ALPHA = [
  [
    [0, 0],
    [0.7, 1],
    [1, 0],
  ],
  [
    [0, 0],
    [0.1, 1],
    [0.7, 1],
    [1, 0],
  ],
  [
    [0, 0],
    [0.7, 1],
    [1, 0],
  ],
  [
    [0, 0],
    [0.5, 1],
    [1, 0],
  ],
  [
    [0, 0],
    [0.7, 1],
    [1, 0],
  ],
  [
    [0, 0],
    [0.1, 1],
    [0.7, 1],
    [1, 0],
  ],
] as const;
const SIZE = [
  [
    [0, 0.5],
    [0.5, 1],
    [1, 0.5],
  ],
  [
    [0, 0.5],
    [0.5, 2.5],
    [1, 0],
  ],
  [
    [0, 0.5],
    [1, 4],
  ],
  [
    [0, 0.5],
    [0.5, 3],
    [1, 0.5],
  ],
  [
    [0, 0.5],
    [0.5, 4],
    [1, 10],
  ],
  [
    [0, 0.05],
    [0.5, 0.25],
    [1, 0],
  ],
] as const;
const srgb = (v: number) =>
  v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
const rgb = (hex: number) => [
  srgb((hex >>> 16) / 255),
  srgb(((hex >>> 8) & 255) / 255),
  srgb((hex & 255) / 255),
];
const COLORS = [
  [
    [0, rgb(0xbb2909)],
    [1, rgb(0x701a08)],
  ],
  [
    [0, rgb(0x00ff00)],
    [0.5, rgb(0x40c040)],
    [1, rgb(0xff4040)],
  ],
  [
    [0, rgb(0x202020)],
    [1, rgb(0x101010)],
  ],
  [
    [0, rgb(0x00ff00)],
    [0.3, rgb(0x00ff00)],
    [0.4, rgb(0xdeec42)],
    [1, rgb(0xf4a776)],
  ],
  [
    [0, rgb(0)],
    [1, rgb(0)],
  ],
  [
    [0, rgb(0x00ff00)],
    [0.5, rgb(0x40c040)],
    [1, rgb(0xff4040)],
  ],
] as const;
/** Source point sprites used a size in source world units, not screen pixels. */
export function mmorpgParticleProfile(profile: number, age: number) {
  if (
    !Number.isInteger(profile) ||
    profile < 0 ||
    profile > 5 ||
    !Number.isFinite(age)
  )
    throw Error("Choose a supported particle profile.");
  const t = clamp(age),
    keys = COLORS[profile];
  let i = 1;
  while (i < keys.length - 1 && t > keys[i][0]) i++;
  const [start, a] = keys[i - 1],
    [end, b] = keys[i],
    f = clamp((t - start) / (end - start));
  return {
    alpha: curve(t, ALPHA[profile]),
    size: curve(t, SIZE[profile]),
    color: [
      a[0] + (b[0] - a[0]) * f,
      a[1] + (b[1] - a[1]) * f,
      a[2] + (b[2] - a[2]) * f,
    ] as V3,
    blend:
      profile === BLOOD || profile === HAND_SMOKE || profile === HIT_SMOKE
        ? 1
        : 0,
  };
}

/** A fixed particle slab, independent of render cadence and without per-particle objects. */
export class MmorpgParticleSampler {
  readonly data = new Float32Array(
    MMO_EFFECT_LIMITS.particles * MMO_EFFECT_LIMITS.instanceFloats,
  );
  private particles = new Float64Array(
    MMO_EFFECT_LIMITS.particles * MMO_EFFECT_LIMITS.particleFloats,
  );
  private live = new Uint8Array(MMO_EFFECT_LIMITS.particles);
  private free = new Uint16Array(MMO_EFFECT_LIMITS.particles);
  private freeCount: number = MMO_EFFECT_LIMITS.particles;
  private emitters: Emitter[] = [];
  private actors = new Map<number, MmorpgActorPose>();
  private pending: PendingEvent[] = [];
  private levels: { actor: number; id: number }[] = [];
  private seenLevelIds = new Set<number>();
  private handStarts = new Map<number, number>();
  private actorStates = new Map<number, { state: string; time: number }>();
  private inferredAttacks: number[] = [];
  private highEventId = -1;
  private lastTime?: number;
  private totals = {
    emitted: 0,
    droppedParticles: 0,
    droppedEmitters: 0,
    droppedEvents: 0,
  };
  private order: number[][] = [[], [], []];
  readonly ranges = [
    { first: 0, count: 0 },
    { first: 0, count: 0 },
    { first: 0, count: 0 },
  ];
  count = 0;
  constructor(readonly worldScale = 1 / 6) {
    if (!Number.isFinite(worldScale) || worldScale < 0.01 || worldScale > 1)
      throw Error(
        "Choose the world's source scale before opening its effects.",
      );
    for (let i = 0; i < this.free.length; i++)
      this.free[i] = this.free.length - 1 - i;
  }
  get cpuBytes() {
    return (
      this.data.byteLength +
      this.particles.byteLength +
      this.live.byteLength +
      this.free.byteLength
    );
  }
  get stats() {
    return {
      particles: this.count,
      emitters: this.emitters.length,
      ...this.totals,
      cpuBytes: this.cpuBytes,
    };
  }
  setMmorpgActors(
    actors: readonly MmorpgActorPose[],
    events: readonly MmoEvent[],
    tick?: number,
  ) {
    validateMmorpgActors(actors);
    if (
      !Array.isArray(events) ||
      events.length > 2048 ||
      !events.every(validMmoEvent) ||
      (tick !== undefined && (!Number.isSafeInteger(tick) || tick < 0))
    )
      throw Error("Choose complete world effect events.");
    this.actors = new Map(actors.map((a) => [a.id, a]));
    for (const id of this.actorStates.keys())
      if (!this.actors.has(id)) this.actorStates.delete(id);
    for (const actor of actors) {
      const previous = this.actorStates.get(actor.id);
      if (
        actor.class === "sorceror" &&
        actor.alive &&
        actor.state === "attack" &&
        (previous?.state !== "attack" ||
          actor.actionTime + 0.05 < previous.time) &&
        !this.inferredAttacks.includes(actor.id)
      )
        this.inferredAttacks.push(actor.id);
      this.actorStates.set(actor.id, {
        state: actor.state,
        time: actor.actionTime,
      });
    }
    for (const id of this.handStarts.keys())
      if (!this.actors.has(id)) this.handStarts.delete(id);
    for (const event of events) {
      if (event.id <= this.highEventId) continue;
      this.highEventId = event.id;
      if (event.kind !== "attack" && event.kind !== "damage") continue;
      const age = tick === undefined ? 0 : (tick - event.tick) / 60;
      if (age < 0 || age > 3) continue;
      if (this.pending.length === MMO_EFFECT_LIMITS.pendingEvents) {
        this.pending.shift();
        this.totals.droppedEvents++;
      }
      this.pending.push({ event, age });
    }
  }
  /** The source XP path is dormant. Call only after an actual awarded-level event. */
  emitLevelUp(actor: number, eventId: number) {
    if (
      !Number.isInteger(actor) ||
      actor < 0 ||
      actor > 0xffffffff ||
      !Number.isSafeInteger(eventId) ||
      eventId < 0
    )
      throw Error("Choose an existing character and level event.");
    if (!this.actors.has(actor) || this.seenLevelIds.has(eventId)) return;
    this.seenLevelIds.add(eventId);
    while (this.seenLevelIds.size > 128)
      this.seenLevelIds.delete(this.seenLevelIds.values().next().value!);
    if (this.levels.length < MMO_EFFECT_LIMITS.pendingEvents)
      this.levels.push({ actor, id: eventId });
    else this.totals.droppedEvents++;
  }
  private spawn(
    profile: Profile,
    anchor: readonly number[],
    seed: number,
    born: number,
    now: number,
  ) {
    const lifeScale =
      profile === BLOOD
        ? 0.5
        : profile === HIT_FIRE || profile === HIT_SMOKE
          ? 1.5
          : profile === LEVEL_UP
            ? 5
            : 1;
    const life = (0.25 + random(seed, 0) * 0.75) * lifeScale;
    if (now - born >= life) return;
    if (!this.freeCount) {
      this.totals.droppedParticles++;
      return;
    }
    const slot = this.free[--this.freeCount],
      p = slot * MMO_EFFECT_LIMITS.particleFloats;
    let x = random(seed, 1) * 2 - 1,
      y = random(seed, 2) * 2 - 1,
      z = random(seed, 3) * 2 - 1;
    if (profile !== LEVEL_UP) {
      const length = Math.hypot(x, y, z) || 1;
      x /= length;
      y /= length;
      z /= length;
    }
    const radius = this.worldScale * (profile === LEVEL_UP ? 4 : 1);
    const speed =
      (profile === HAND_FIRE || profile === HAND_SMOKE
        ? -1
        : profile === HIT_FIRE || profile === HIT_SMOKE
          ? 3
          : 0) * this.worldScale;
    const a = this.particles;
    a[p] = anchor[0] + x * radius;
    a[p + 1] = anchor[1] + y * radius;
    a[p + 2] = anchor[2] + z * radius;
    a[p + 3] = x * speed;
    a[p + 4] = profile === LEVEL_UP ? this.worldScale : y * speed;
    a[p + 5] = z * speed;
    a[p + 6] = born;
    a[p + 7] = life;
    a[p + 8] =
      (0.5 + 0.5 * random(seed, 4)) *
      this.worldScale *
      (profile === LEVEL_UP ? 2 : 1);
    a[p + 9] = random(seed, 5) * Math.PI * 2;
    a[p + 10] = profile;
    this.live[slot] = 1;
    this.totals.emitted++;
  }
  private emitter(
    actor: number,
    anchor: keyof MmorpgParticleAnchors,
    profile: Profile,
    born: number,
    life: number,
    seed: number,
  ) {
    if (this.emitters.length >= MMO_EFFECT_LIMITS.emitters) {
      this.totals.droppedEmitters++;
      return;
    }
    this.emitters.push({
      actor,
      anchor,
      profile,
      born,
      life,
      seed,
      emitted: 0,
    });
  }
  private hands(actor: MmorpgActorPose, born: number, seed: number) {
    if (
      actor.class !== "sorceror" ||
      Math.abs((this.handStarts.get(actor.id) ?? -Infinity) - born) < 0.35
    )
      return;
    this.handStarts.set(actor.id, born);
    for (const [index, bone] of (
      ["RightHandIndex1", "LeftHandIndex1"] as const
    ).entries()) {
      this.emitter(
        actor.id,
        bone,
        HAND_FIRE,
        born,
        2.5,
        seed ^ (index * 277 + 19),
      );
      this.emitter(
        actor.id,
        bone,
        HAND_SMOKE,
        born,
        2.5,
        seed ^ (index * 277 + 113),
      );
    }
  }
  prepare(
    time: number,
    eye: readonly number[],
    attachments: MmorpgParticleAttachments,
  ) {
    if (
      !Number.isFinite(time) ||
      Math.abs(time) > 1e12 ||
      eye.length !== 3 ||
      eye.some((n) => !Number.isFinite(n))
    )
      throw Error("Choose a complete effect camera and clock.");
    if (this.lastTime !== undefined && time < this.lastTime)
      this.clearParticles();
    this.lastTime = time;
    const a = this.particles;
    for (let slot = 0; slot < this.live.length; slot++)
      if (this.live[slot] && time - a[slot * 12 + 6] >= a[slot * 12 + 7]) {
        this.live[slot] = 0;
        this.free[this.freeCount++] = slot;
      }
    for (const { event, age } of this.pending) {
      const actor =
        event.actor === undefined ? undefined : this.actors.get(event.actor);
      if (event.kind === "attack") {
        if (actor)
          this.hands(
            actor,
            time - (actor.state === "attack" ? actor.actionTime : age),
            event.id,
          );
        continue;
      }
      const target =
          event.target === undefined
            ? undefined
            : this.actors.get(event.target),
        anchors = target && attachments.get(target.id);
      if (!anchors) continue;
      for (const [index, bone] of (["Head", "Hips"] as const).entries()) {
        if (actor?.class === "sorceror") {
          for (let i = 0; i < 200; i++)
            this.spawn(
              HIT_FIRE,
              anchors[bone],
              event.id ^ Math.imul(i + 1, 719) ^ (index * 977),
              time - age,
              time,
            );
          for (let i = 0; i < 100; i++)
            this.spawn(
              HIT_SMOKE,
              anchors[bone],
              event.id ^ Math.imul(i + 1, 211) ^ (index * 313),
              time - age,
              time,
            );
        } else
          this.emitter(
            target!.id,
            bone,
            BLOOD,
            time - age,
            0.5,
            event.id ^ (index * 977),
          );
      }
    }
    this.pending.length = 0;
    for (const id of this.inferredAttacks) {
      const actor = this.actors.get(id);
      if (actor)
        this.hands(
          actor,
          time - actor.actionTime,
          actor.id ^ Math.round((time - actor.actionTime) * 60),
        );
    }
    this.inferredAttacks.length = 0;
    for (const event of this.levels) {
      const actor = this.actors.get(event.actor);
      if (actor)
        for (let i = 0; i < 300; i++)
          this.spawn(
            LEVEL_UP,
            actor.position,
            event.id ^ Math.imul(i + 1, 953),
            time,
            time,
          );
    }
    this.levels.length = 0;
    let retained = 0;
    for (const e of this.emitters) {
      const t = clamp(time - e.born, 0, e.life),
        desired = Math.floor(100 * (e.life * t - 0.5 * t * t) + 1e-7),
        anchor = attachments.get(e.actor)?.[e.anchor];
      if (anchor)
        for (let n = e.emitted + 1; n <= desired; n++) {
          const born =
            e.born + e.life - Math.sqrt(Math.max(0, e.life * e.life - n / 50));
          this.spawn(
            e.profile,
            anchor,
            e.seed ^ Math.imul(n, 7919),
            born,
            time,
          );
        }
      e.emitted = desired;
      if (time - e.born < e.life && this.actors.has(e.actor))
        this.emitters[retained++] = e;
    }
    this.emitters.length = retained;
    for (const order of this.order) order.length = 0;
    for (let slot = 0; slot < this.live.length; slot++)
      if (this.live[slot]) this.order[textureOf(a[slot * 12 + 10])].push(slot);
    const distance = (slot: number) => {
      const p = slot * 12,
        age = time - a[p + 6],
        displacement = -Math.expm1(-0.1 * age) / 0.1;
      return (
        (a[p] + a[p + 3] * displacement - eye[0]) ** 2 +
        (a[p + 1] + a[p + 4] * displacement - eye[1]) ** 2 +
        (a[p + 2] + a[p + 5] * displacement - eye[2]) ** 2
      );
    };
    let count = 0;
    for (let texture = 0; texture < 3; texture++) {
      this.order[texture].sort((x, y) => distance(y) - distance(x) || x - y);
      this.ranges[texture].first = count;
      for (const slot of this.order[texture]) {
        const p = slot * 12,
          out = count++ * 12,
          age = Math.max(0, time - a[p + 6]),
          t = clamp(age / a[p + 7]),
          profile = mmorpgParticleProfile(a[p + 10], t),
          displacement = -Math.expm1(-0.1 * age) / 0.1;
        this.data[out] = a[p] + a[p + 3] * displacement;
        this.data[out + 1] = a[p + 1] + a[p + 4] * displacement;
        this.data[out + 2] = a[p + 2] + a[p + 5] * displacement;
        this.data[out + 3] = a[p + 8] * profile.size;
        this.data.set(profile.color, out + 4);
        this.data[out + 7] = profile.alpha;
        this.data[out + 8] = a[p + 9] + age * 0.5;
        this.data[out + 9] = profile.blend;
        this.data[out + 10] = a[p + 10];
        this.data[out + 11] = slot;
      }
      this.ranges[texture].count = count - this.ranges[texture].first;
    }
    this.count = count;
    return {
      data: this.data.subarray(0, count * 12),
      count,
      ranges: this.ranges,
    };
  }
  private clearParticles() {
    this.live.fill(0);
    this.emitters.length = 0;
    this.handStarts.clear();
    this.freeCount = this.free.length;
    for (let i = 0; i < this.free.length; i++)
      this.free[i] = this.free.length - i - 1;
    this.count = 0;
  }
  reset() {
    this.clearParticles();
    this.pending.length = 0;
    this.levels.length = 0;
    this.highEventId = -1;
    this.actorStates.clear();
    this.inferredAttacks.length = 0;
    this.lastTime = undefined;
    this.seenLevelIds.clear();
  }
}

export const MMORPG_EFFECT_WGSL = /* wgsl */ `
${FOG_SEGMENT_WGSL}
struct Frame { view:mat4x4<f32>,right:vec4<f32>,up:vec4<f32>,eye:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32> }
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var sprite:texture_2d<f32>;
@group(0)@binding(2)var spriteSampler:sampler;
struct In { @location(0)center:vec4<f32>,@location(1)color:vec4<f32>,@location(2)extra:vec4<f32> }
struct Out { @builtin(position)clip:vec4<f32>,@location(0)uv:vec2<f32>,@location(1)color:vec4<f32>,@location(2)world:vec3<f32>,@location(3)extra:vec2<f32> }
@vertex fn vs(v:In,@builtin(vertex_index)index:u32)->Out {
 let corners=array<vec2<f32>,6>(vec2(-.5,-.5),vec2(.5,-.5),vec2(-.5,.5),vec2(-.5,.5),vec2(.5,-.5),vec2(.5,.5));let corner=corners[index];
 let world=v.center.xyz+(frame.right.xyz*corner.x+frame.up.xyz*corner.y)*v.center.w;
 var o:Out;o.clip=frame.view*vec4(world,1.0);o.uv=vec2(corner.x+.5,.5-corner.y);o.color=v.color;o.world=world;o.extra=v.extra.xy;return o;
}
@fragment fn fs(v:Out)->@location(0)vec4<f32> {
 let angle=v.extra.x;let cs=cos(angle);let sn=sin(angle);let p=v.uv-vec2(.5);let uv=vec2(p.x*cs+p.y*sn,-p.x*sn+p.y*cs)+vec2(.5);
 if any(uv<vec2(0.0))||any(uv>vec2(1.0)){discard;}
 let sampled=textureSample(sprite,spriteSampler,uv);let alpha=sampled.a*v.color.a;if alpha<.0001{discard;}
 let delta=v.world-frame.eye.xyz;let distance=length(delta);let fog=heightFogComposition(frame.eye.y,delta.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 let radiance=sampled.rgb*v.color.rgb*fog.x+frame.fogColor.rgb*fog.y*v.extra.y;
 return vec4(radiance*alpha*${SCENE_LINEAR_SCALE},alpha*v.extra.y);
}
`;
const MIP_WGSL = /* wgsl */ `
@group(0)@binding(0)var image:texture_2d<f32>;@group(0)@binding(1)var imageSampler:sampler;
struct Out{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>}
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));var o:Out;o.position=vec4(p[i],0.,1.);o.uv=p[i]*vec2(.5,-.5)+vec2(.5);return o;}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{return textureSample(image,imageSampler,v.uv);}
`;
export interface MmorpgEffectOptions {
  depthFormat?: GPUTextureFormat;
  sampleCount?: number;
}
export function validateMmorpgParticleTextures(
  textures: MmorpgParticleTextures,
) {
  let bytes = 0;
  for (const key of TEXTURES) {
    const t = textures?.[key];
    if (
      !t ||
      !/^\/private-assets\/[A-Za-z0-9._-]+\.png$/.test(t.url) ||
      !/^[a-f0-9]{64}$/.test(t.sha256) ||
      !Number.isInteger(t.byteLength) ||
      t.byteLength < 33 ||
      t.byteLength > 8 * 1024 * 1024 ||
      !Number.isInteger(t.width) ||
      !Number.isInteger(t.height) ||
      t.width < 1 ||
      t.width > 1024 ||
      t.height < 1 ||
      t.height > 1024
    )
      throw Error("Prepare the world's particle images before opening it.");
    bytes += characterTextureBytes(t.width, t.height);
  }
  if (bytes > MMO_EFFECT_LIMITS.textureBytes)
    throw Error("The world's particle images are too large.");
  return bytes;
}
export class MmorpgEffects {
  private sampler: MmorpgParticleSampler;
  private uniforms = new Float32Array(MMO_EFFECT_LIMITS.uniformBytes / 4);
  private submitted = { particles: 0, triangles: 0, drawCalls: 0 };
  private disposed = false;
  readonly bytes: number;
  private constructor(
    private device: GPUDevice,
    private pipeline: GPURenderPipeline,
    private instances: GPUBuffer,
    private uniform: GPUBuffer,
    private groups: GPUBindGroup[],
    private textures: GPUTexture[],
    worldScale: number,
    textureBytes: number,
  ) {
    this.sampler = new MmorpgParticleSampler(worldScale);
    this.bytes =
      MMO_EFFECT_LIMITS.particles * 48 +
      MMO_EFFECT_LIMITS.uniformBytes +
      textureBytes;
  }
  static async create(
    device: GPUDevice,
    descriptions: MmorpgParticleTextures,
    worldScale = 1 / 6,
    load = fetchCharacterBytes,
    options: MmorpgEffectOptions = {},
  ) {
    const textureBytes = validateMmorpgParticleTextures(descriptions);
    if (!Number.isFinite(worldScale) || worldScale < 0.01 || worldScale > 1)
      throw Error(
        "Choose the world's source scale before opening its effects.",
      );
    const textures: GPUTexture[] = [];
    let uniform: GPUBuffer | undefined, instances: GPUBuffer | undefined;
    try {
      uniform = device.createBuffer({
        label: "MMO particle camera",
        size: MMO_EFFECT_LIMITS.uniformBytes,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      instances = device.createBuffer({
        label: "MMO particle slab",
        size: MMO_EFFECT_LIMITS.particles * 48,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      const layout = device.createBindGroupLayout({
        entries: [
          {
            binding: 0,
            visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
            buffer: {
              type: "uniform",
              minBindingSize: MMO_EFFECT_LIMITS.uniformBytes,
            },
          },
          { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
          { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
        ],
      });
      const shader = device.createShaderModule({
        label: "MMO source particle sprites",
        code: MMORPG_EFFECT_WGSL,
      });
      const pipeline = await device.createRenderPipelineAsync({
        label: "MMO premultiplied source particles",
        layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
        vertex: {
          module: shader,
          entryPoint: "vs",
          buffers: [
            {
              arrayStride: 48,
              stepMode: "instance",
              attributes: [
                { shaderLocation: 0, offset: 0, format: "float32x4" },
                { shaderLocation: 1, offset: 16, format: "float32x4" },
                { shaderLocation: 2, offset: 32, format: "float32x4" },
              ],
            },
          ],
        },
        fragment: {
          module: shader,
          entryPoint: "fs",
          targets: [
            {
              format: "rgba16float",
              blend: {
                color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
                alpha: { srcFactor: "zero", dstFactor: "one" },
              },
            },
          ],
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: options.depthFormat ?? "depth24plus",
          depthWriteEnabled: false,
          depthCompare: "less-equal",
        },
        multisample: { count: options.sampleCount ?? 4 },
      });
      const mipShader = device.createShaderModule({
        label: "MMO sprite mips",
        code: MIP_WGSL,
      });
      const mipPipeline = await device.createRenderPipelineAsync({
        label: "MMO linear-color sprite mips",
        layout: "auto",
        vertex: { module: mipShader, entryPoint: "vs" },
        fragment: {
          module: mipShader,
          entryPoint: "fs",
          targets: [{ format: "rgba8unorm-srgb" }],
        },
        primitive: { topology: "triangle-list" },
      });
      const sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "linear",
        addressModeU: "clamp-to-edge",
        addressModeV: "clamp-to-edge",
      });
      const groups: GPUBindGroup[] = [];
      for (const name of TEXTURES) {
        const description = descriptions[name],
          bytes = await checkedCharacterBlob(description, load);
        validateCharacterPNG(bytes, description.width, description.height);
        const image = await createImageBitmap(
          new Blob([bytes], { type: "image/png" }),
          { colorSpaceConversion: "none", premultiplyAlpha: "none" },
        );
        const levels = characterTextureLevels(
          description.width,
          description.height,
        );
        try {
          if (
            image.width !== description.width ||
            image.height !== description.height
          )
            throw Error("A particle image has incorrect dimensions.");
          const texture = device.createTexture({
            label: `MMO ${name} sprite`,
            size: [description.width, description.height],
            mipLevelCount: levels,
            format: "rgba8unorm-srgb",
            usage:
              GPUTextureUsage.TEXTURE_BINDING |
              GPUTextureUsage.COPY_DST |
              GPUTextureUsage.RENDER_ATTACHMENT,
          });
          textures.push(texture);
          device.queue.copyExternalImageToTexture(
            { source: image },
            { texture },
            [description.width, description.height],
          );
          const encoder = device.createCommandEncoder({
            label: "MMO sprite mip preparation",
          });
          for (let level = 1; level < levels; level++) {
            const group = device.createBindGroup({
              layout: mipPipeline.getBindGroupLayout(0),
              entries: [
                {
                  binding: 0,
                  resource: texture.createView({
                    baseMipLevel: level - 1,
                    mipLevelCount: 1,
                  }),
                },
                { binding: 1, resource: sampler },
              ],
            });
            const pass = encoder.beginRenderPass({
              colorAttachments: [
                {
                  view: texture.createView({
                    baseMipLevel: level,
                    mipLevelCount: 1,
                  }),
                  loadOp: "clear",
                  storeOp: "store",
                  clearValue: [0, 0, 0, 0],
                },
              ],
            });
            pass.setPipeline(mipPipeline);
            pass.setBindGroup(0, group);
            pass.draw(3);
            pass.end();
          }
          device.queue.submit([encoder.finish()]);
          groups.push(
            device.createBindGroup({
              layout,
              entries: [
                { binding: 0, resource: { buffer: uniform } },
                { binding: 1, resource: texture.createView() },
                { binding: 2, resource: sampler },
              ],
            }),
          );
        } finally {
          image.close();
        }
      }
      return new MmorpgEffects(
        device,
        pipeline,
        instances,
        uniform,
        groups,
        textures,
        worldScale,
        textureBytes,
      );
    } catch (error) {
      for (const texture of textures) texture.destroy();
      uniform?.destroy();
      instances?.destroy();
      throw error;
    }
  }
  get stats() {
    return {
      ...this.sampler.stats,
      ...this.submitted,
      geometryBytes: this.bytes,
    };
  }
  setMmorpgActors(
    actors: readonly MmorpgActorPose[],
    events: readonly MmoEvent[],
    tick?: number,
  ) {
    if (!this.disposed) this.sampler.setMmorpgActors(actors, events, tick);
  }
  emitLevelUp(actor: number, eventId: number) {
    if (!this.disposed) this.sampler.emitLevelUp(actor, eventId);
  }
  prepare(frame: CreatureFrame, attachments: MmorpgParticleAttachments) {
    this.submitted = { particles: 0, triangles: 0, drawCalls: 0 };
    if (this.disposed || frame.enabled === false) return;
    const sampled = this.sampler.prepare(frame.time, frame.eye, attachments);
    if (!sampled.count) return;
    const u = this.uniforms,
      vp = frame.viewProjection;
    for (let i = 0; i < 16; i++) u[i] = vp[i];
    const r = Math.hypot(vp[0], vp[4], vp[8]) || 1,
      up = Math.hypot(vp[1], vp[5], vp[9]) || 1;
    u.set([vp[0] / r, vp[4] / r, vp[8] / r, 0], 16);
    u.set([vp[1] / up, vp[5] / up, vp[9] / up, 0], 20);
    u.set([...frame.eye, 0], 24);
    u.set(
      [frame.fog?.density ?? 0, frame.fog?.heightRate ?? FOG_HEIGHT_RATE, 0, 0],
      28,
    );
    u.set([...(frame.fog?.color ?? [0, 0, 0]), 0], 32);
    u.set(frame.fog?.volumeMedium ?? [0, 0, 0, 0], 36);
    u.set([...(frame.fog?.volumeRange ?? [0, 0]), 0, 0], 40);
    this.device.queue.writeBuffer(this.uniform, 0, u);
    this.device.queue.writeBuffer(this.instances, 0, sampled.data);
    this.submitted.particles = sampled.count;
  }
  render(pass: GPURenderPassEncoder) {
    if (this.disposed || !this.submitted.particles) return;
    pass.setPipeline(this.pipeline);
    pass.setVertexBuffer(0, this.instances);
    for (let i = 0; i < 3; i++) {
      const range = this.sampler.ranges[i];
      if (!range.count) continue;
      pass.setBindGroup(0, this.groups[i]);
      pass.draw(6, range.count, 0, range.first);
      this.submitted.drawCalls++;
      this.submitted.triangles += range.count * 2;
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.instances.destroy();
    this.uniform.destroy();
    for (const texture of this.textures) texture.destroy();
    this.sampler.reset();
  }
}
