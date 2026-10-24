/** Owned character artifact and animation sampling, adapted from public Verse. */
import { inverse, multiply, type V3 } from "./math.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";

export const CHARACTER_LIMITS = Object.freeze({
  vertices: 100000,
  indices: 300000,
  joints: 128,
  clips: 16,
  keys: 4096,
  materials: 32,
  blobBytes: 16 * 1024 * 1024,
  manifestBytes: 16 * 1024 * 1024,
  textureBytes: 64 * 1024 * 1024,
});
export const CHARACTER_MOTIONS = [
  "idle",
  "walk",
  "run",
  "jump",
  "backpedal",
  "strafe_left",
  "strafe_right",
] as const;
export const QUICK_MMORPG_MOTIONS = [
  "idle",
  "walk",
  "run",
  "attack",
  "dance",
  "death",
] as const;
export type CharacterMotion =
  (typeof CHARACTER_MOTIONS)[number] | "death" | "attack" | "dance";
export interface CharacterAnimationLayer {
  animation: CharacterMotion;
  time: number;
  weight: number;
}
export interface CharacterBlend {
  from: CharacterMotion;
  fromTime: number;
  weight: number;
  /** Source mixer actions in first-activation order, including interrupted fades. */
  layers?: readonly CharacterAnimationLayer[];
}
export interface CharacterTexture extends CharacterBlob {
  width: number;
  height: number;
}
export function characterTextureLevels(width: number, height: number) {
  return Math.floor(Math.log2(Math.max(width, height))) + 1;
}
export function characterTextureBytes(width: number, height: number) {
  let bytes = 0;
  for (let level = 0; level < characterTextureLevels(width, height); level++)
    bytes += Math.max(1, width >> level) * Math.max(1, height >> level) * 4;
  return bytes;
}
export interface CharacterBlob {
  url: string;
  sha256: string;
  byteLength: number;
}
export interface CharacterKey {
  time: number;
  value: number[];
}
export interface CharacterLocal {
  translation: number[];
  rotation: number[];
  scale: number[];
}
export interface CharacterJoint extends CharacterLocal {
  parent: number;
  inverseBind: number[];
}
export interface CharacterTrack {
  joint: number;
  translation: CharacterKey[];
  rotation: CharacterKey[];
  scale: CharacterKey[];
}
export interface CharacterClip {
  name: CharacterMotion;
  duration: number;
  distance: number;
  tracks: CharacterTrack[];
  loop?: boolean;
}
export interface CharacterManifest {
  schema: "aethyrforge.character.v1";
  name: string;
  controller?: { profile: "quick-mmorpg-v1"; worldScale: number };
  animationProfile?: "quick-mmorpg-zombie-v1" | "quick-mmorpg-npc-v1";
  mmorpgClass?: "paladin" | "sorceror" | "zombie" | "warrok";
  geometry: {
    vertices: CharacterBlob;
    indices: CharacterBlob;
    vertexCount: number;
    indexCount: number;
  };
  joints: CharacterJoint[];
  clips: CharacterClip[];
  materials: {
    name: string;
    color: number[];
    alphaCutoff: number;
    doubleSided: boolean;
    texture?: CharacterTexture;
    normalTexture?: CharacterTexture;
    emissiveTexture?: CharacterTexture;
    emissiveColor?: number[];
    normalScale?: number;
    metallic?: number;
    roughness?: number;
  }[];
  draws: {
    material: number;
    firstIndex: number;
    indexCount: number;
    part?: CharacterWeapon;
  }[];
  metadata?: Record<string, unknown>;
}
export type CharacterWeapon =
  "weapon.sword1" | "weapon.axe1" | "weapon.hammer1";
export interface CharacterRenderPose {
  /** Feet in world meters; native model height is retained. */
  position: V3;
  /** Rotation around +Y, radians; the retained model faces +Z. */
  yaw: number;
  animation: CharacterMotion;
  /** Clip time, advanced from admitted distance for a gait. */
  time: number;
  visible: boolean;
  blend?: CharacterBlend;
}
function finite(value: number, lo: number, hi: number) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < lo ||
    value > hi
  )
    throw new Error("The prepared character contains an unsupported number.");
}
function count(value: number, lo: number, hi: number) {
  finite(value, lo, hi);
  if (!Number.isInteger(value))
    throw new Error("The prepared character needs integer counts.");
}
function vector(value: number[], length: number, lo = -10000, hi = 10000) {
  if (!Array.isArray(value) || value.length !== length)
    throw new Error("The prepared character needs complete vectors.");
  value.forEach((v) => finite(v, lo, hi));
}
function quaternion(value: number[]) {
  vector(value, 4, -1.001, 1.001);
  if (Math.abs(Math.hypot(...value) - 1) > 0.01)
    throw new Error("The prepared character needs unit rotations.");
}
function local(value: CharacterLocal) {
  vector(value.translation, 3);
  quaternion(value.rotation);
  vector(value.scale, 3, 0.0001, 100);
}
function blob(value: CharacterBlob) {
  if (
    !value ||
    typeof value.url !== "string" ||
    !/^\/(?:private-assets|starter)\/[A-Za-z0-9._-]+$/.test(value.url) ||
    !/^[a-f0-9]{64}$/.test(value.sha256)
  )
    throw new Error("Prepare a local character file with its hash.");
  count(value.byteLength, 1, CHARACTER_LIMITS.blobBytes);
}
export function validateCharacter(value: CharacterManifest): CharacterManifest {
  if (
    value?.schema !== "aethyrforge.character.v1" ||
    typeof value.name !== "string" ||
    !value.name ||
    value.name.length > 255 ||
    !value.geometry ||
    !Array.isArray(value.joints) ||
    !Array.isArray(value.clips) ||
    !Array.isArray(value.materials) ||
    !Array.isArray(value.draws)
  )
    throw new Error("Prepare a supported character recipe.");
  const g = value.geometry;
  count(g.vertexCount, 1, CHARACTER_LIMITS.vertices);
  count(g.indexCount, 3, CHARACTER_LIMITS.indices);
  blob(g.vertices);
  blob(g.indices);
  if (
    g.vertices.byteLength !== g.vertexCount * 64 ||
    g.indices.byteLength !== g.indexCount * 4 ||
    g.indexCount % 3
  )
    throw new Error("The prepared character has inconsistent geometry sizes.");
  count(value.joints.length, 1, CHARACTER_LIMITS.joints);
  for (let i = 0; i < value.joints.length; i++) {
    const joint = value.joints[i];
    local(joint);
    count(joint.parent, -1, i - 1);
    vector(joint.inverseBind, 16);
    if (
      Math.abs(joint.inverseBind[3]) > 1e-5 ||
      Math.abs(joint.inverseBind[7]) > 1e-5 ||
      Math.abs(joint.inverseBind[11]) > 1e-5 ||
      Math.abs(joint.inverseBind[15] - 1) > 1e-5
    )
      throw new Error("Character bind matrices must be affine.");
    inverse(joint.inverseBind);
  }
  const quick = value.controller?.profile === "quick-mmorpg-v1";
  const zombie =
    value.animationProfile === "quick-mmorpg-zombie-v1" ||
    value.animationProfile === "quick-mmorpg-npc-v1";
  if (
    value.mmorpgClass !== undefined &&
    !["paladin", "sorceror", "zombie", "warrok"].includes(value.mmorpgClass)
  )
    throw new Error("Choose an original MMORPG character class.");
  if (value.animationProfile !== undefined && (!zombie || value.controller))
    throw new Error("Choose a supported character animation profile.");
  if (value.controller) {
    if (!quick) throw new Error("Choose a supported character controller.");
    finite(value.controller.worldScale, 0.01, 1);
  }
  const required = zombie
    ? ["idle", "walk", "attack", "death"]
    : quick
      ? QUICK_MMORPG_MOTIONS
      : CHARACTER_MOTIONS;
  const allowed: readonly string[] = quick
    ? QUICK_MMORPG_MOTIONS
    : zombie
      ? required
      : [...CHARACTER_MOTIONS, "death"];
  count(value.clips.length, required.length, CHARACTER_LIMITS.clips);
  const names = new Set<string>();
  let totalKeys = 0;
  for (const clip of value.clips) {
    if (!allowed.includes(clip.name) || names.has(clip.name))
      throw new Error("Character movement clips need unique supported names.");
    names.add(clip.name);
    if (clip.loop !== undefined && typeof clip.loop !== "boolean")
      throw new Error("Character clip looping must be explicit.");
    finite(clip.duration, 0.0001, 120);
    finite(clip.distance, 0, 100);
    if (!Array.isArray(clip.tracks))
      throw new Error("Character clips need joint tracks.");
    count(clip.tracks.length, 0, value.joints.length);
    const joints = new Set<number>();
    for (const track of clip.tracks) {
      count(track.joint, 0, value.joints.length - 1);
      if (joints.has(track.joint))
        throw new Error("Character clips cannot repeat a joint track.");
      joints.add(track.joint);
      for (const [channel, width] of [
        ["translation", 3],
        ["rotation", 4],
        ["scale", 3],
      ] as const) {
        if (!Array.isArray(track[channel]))
          throw new Error("Character tracks need complete channels.");
        count(track[channel].length, 0, CHARACTER_LIMITS.keys);
        let last = -Infinity;
        for (const key of track[channel]) {
          finite(key.time, 0, clip.duration + 1e-5);
          vector(key.value, width);
          if (channel === "rotation") quaternion(key.value);
          if (channel === "scale") vector(key.value, 3, 0.0001, 100);
          if (key.time <= last)
            throw new Error("Character keys must be ordered.");
          last = key.time;
          if (++totalKeys > (value.mmorpgClass ? 150000 : 100000))
            throw new Error("The character exceeds its total key budget.");
        }
      }
    }
  }
  if (required.some((name) => !names.has(name)))
    throw new Error("The character needs every movement clip.");
  count(value.materials.length, 1, CHARACTER_LIMITS.materials);
  let textureBytes = 0;
  const textures = new Set<string>();
  for (const material of value.materials) {
    vector(material.color, 4, 0, 1);
    finite(material.alphaCutoff, 0, 1);
    if (
      typeof material.doubleSided !== "boolean" ||
      typeof material.name !== "string" ||
      !material.name ||
      material.name.length > 255
    )
      throw new Error("Character materials need complete settings.");
    if (material.metallic !== undefined) finite(material.metallic, 0, 1);
    if (material.roughness !== undefined) finite(material.roughness, 0, 1);
    if (material.normalScale !== undefined) finite(material.normalScale, 0, 4);
    if (material.emissiveColor !== undefined)
      vector(material.emissiveColor, 3, 0, 16);
    for (const [role, texture] of [
      ["base", material.texture],
      ["normal", material.normalTexture],
      ...(material.emissiveTexture
        ? [["base", material.emissiveTexture] as const]
        : []),
    ] as const) {
      if (texture) {
        blob(texture);
        count(texture.width, 1, 2048);
        count(texture.height, 1, 2048);
      }
      const key = `${role}:${texture?.sha256 ?? "fallback"}:${texture?.width ?? 1}:${texture?.height ?? 1}`;
      if (!textures.has(key)) {
        textureBytes += characterTextureBytes(
          texture?.width ?? 1,
          texture?.height ?? 1,
        );
        textures.add(key);
      }
    }
    if (
      !material.emissiveTexture &&
      material.emissiveColor?.some((value) => value > 0)
    ) {
      const key = "base:fallback:1:1";
      if (!textures.has(key)) {
        textureBytes += 4;
        textures.add(key);
      }
    }
  }
  if (textureBytes > CHARACTER_LIMITS.textureBytes)
    throw new Error("The character exceeds its texture budget.");

  count(value.draws.length, 1, 64);
  let end = 0;
  for (const draw of value.draws) {
    if (
      draw.part !== undefined &&
      !["weapon.sword1", "weapon.axe1", "weapon.hammer1"].includes(draw.part)
    )
      throw new Error("Choose a supported attached weapon.");
    count(draw.material, 0, value.materials.length - 1);
    count(draw.firstIndex, 0, g.indexCount - 3);
    count(draw.indexCount, 3, g.indexCount);
    if (
      draw.firstIndex !== end ||
      draw.firstIndex % 3 ||
      draw.indexCount % 3 ||
      draw.firstIndex + draw.indexCount > g.indexCount
    )
      throw new Error(
        "Character draws need complete nonoverlapping triangles.",
      );
    end += draw.indexCount;
  }
  if (end !== g.indexCount)
    throw new Error("Character draws must cover the geometry.");
  return value;
}

export function validateCharacterGeometry(
  manifest: CharacterManifest,
  vertices: Float32Array,
  indices: Uint32Array,
) {
  if (
    vertices.length !== manifest.geometry.vertexCount * 16 ||
    indices.length !== manifest.geometry.indexCount
  )
    throw new Error("Character buffers have incorrect lengths.");
  for (const index of indices)
    if (index >= manifest.geometry.vertexCount)
      throw new Error("Character triangles leave the vertex buffer.");
  for (let i = 0; i < vertices.length; i += 16) {
    for (let k = 0; k < 8; k++) finite(vertices[i + k], -10000, 10000);
    if (
      Math.abs(
        Math.hypot(vertices[i + 3], vertices[i + 4], vertices[i + 5]) - 1,
      ) > 0.01
    )
      throw new Error("Character vertices need unit normals.");
    let weight = 0;
    for (let k = 0; k < 4; k++) {
      count(vertices[i + 8 + k], 0, manifest.joints.length - 1);
      finite(vertices[i + 12 + k], 0, 1);
      weight += vertices[i + 12 + k];
    }
    if (Math.abs(weight - 1) > 0.001)
      throw new Error("Character skin weights must total one.");
  }
}

/** Shortest-arc unit-quaternion interpolation, including opposite encodings. */
export function characterSlerp(
  a: readonly number[],
  b: readonly number[],
  t: number,
): number[] {
  let cosine = a.reduce((sum, value, i) => sum + value * b[i], 0);
  const sign = cosine < 0 ? -1 : 1;
  cosine = Math.min(1, Math.abs(cosine));
  if (1 - cosine * cosine <= Number.EPSILON) {
    const q = a.map((v, i) => v + (b[i] * sign - v) * t);
    const n = Math.hypot(...q);
    return q.map((v) => v / n);
  }
  const sine = Math.sqrt(1 - cosine * cosine),
    angle = Math.atan2(sine, cosine);
  return a.map(
    (v, i) =>
      (v * Math.sin((1 - t) * angle) + b[i] * sign * Math.sin(t * angle)) /
      sine,
  );
}
function sample(
  keys: CharacterKey[],
  time: number,
  rest: number[],
  rotation: boolean,
) {
  if (!keys.length) return [...rest];
  if (time <= keys[0].time) return [...keys[0].value];
  if (time >= keys.at(-1)!.time) return [...keys.at(-1)!.value];
  let lo = 0,
    hi = keys.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >>> 1;
    if (keys[mid].time > time) hi = mid;
    else lo = mid;
  }
  const a = keys[lo],
    b = keys[hi],
    t = (time - a.time) / (b.time - a.time);
  return rotation
    ? characterSlerp(a.value, b.value, t)
    : a.value.map((v, i) => v + (b.value[i] - v) * t);
}
export function sampleCharacterLocals(
  manifest: CharacterManifest,
  motion: CharacterMotion,
  time: number,
): CharacterLocal[] {
  finite(time, -1e12, 1e12);
  const clip = manifest.clips.find((c) => c.name === motion);
  if (!clip) throw new Error("Choose a prepared character movement.");
  // A death is a one-shot pose. Paused or long-lived bodies hold its final key.
  const wrapped =
    (clip.loop ?? motion !== "death") === false
      ? Math.min(clip.duration, Math.max(0, time))
      : ((time % clip.duration) + clip.duration) % clip.duration;
  const locals = manifest.joints.map((joint) => ({
    translation: [...joint.translation],
    rotation: [...joint.rotation],
    scale: [...joint.scale],
  }));
  for (const track of clip.tracks) {
    const joint = manifest.joints[track.joint];
    locals[track.joint] = {
      translation: sample(track.translation, wrapped, joint.translation, false),
      rotation: sample(track.rotation, wrapped, joint.rotation, true),
      scale: sample(track.scale, wrapped, joint.scale, false),
    };
  }
  return locals;
}
/** Source normal blending is ordered per animated property, with unused weight from rest. */
export function sampleCharacterLayers(
  manifest: CharacterManifest,
  layers: readonly CharacterAnimationLayer[],
): CharacterLocal[] {
  if (!Array.isArray(layers) || layers.length > 6)
    throw new Error("Blend at most six character actions.");
  const seen = new Set<CharacterMotion>();
  const sampled = layers.map((layer) => {
    if (!layer || seen.has(layer.animation))
      throw new Error("Blend each character action once.");
    seen.add(layer.animation);
    finite(layer.weight, 0, 1);
    finite(layer.time, -1e12, 1e12);
    const clip = manifest.clips.find((c) => c.name === layer.animation);
    if (!clip) throw new Error("Choose a prepared character movement.");
    return {
      layer,
      clip,
      locals: sampleCharacterLocals(manifest, layer.animation, layer.time),
    };
  });
  const locals = manifest.joints.map((joint) => ({
    translation: [...joint.translation],
    rotation: [...joint.rotation],
    scale: [...joint.scale],
  }));
  const totals = manifest.joints.map(() => ({
    translation: 0,
    rotation: 0,
    scale: 0,
  }));
  for (const { layer, clip, locals: incoming } of sampled) {
    if (layer.weight === 0) continue;
    for (const track of clip.tracks) {
      const index = track.joint;
      for (const channel of ["translation", "rotation", "scale"] as const) {
        if (track[channel].length === 0) continue;
        const previous = totals[index][channel],
          total = previous + layer.weight;
        const t = layer.weight / total;
        locals[index][channel] =
          previous === 0
            ? [...incoming[index][channel]]
            : channel === "rotation"
              ? characterSlerp(
                  locals[index][channel],
                  incoming[index][channel],
                  t,
                )
              : locals[index][channel].map(
                  (v, k) => v * (1 - t) + incoming[index][channel][k] * t,
                );
        totals[index][channel] = total;
      }
    }
  }
  for (let index = 0; index < locals.length; index++) {
    for (const channel of ["translation", "rotation", "scale"] as const) {
      const weight = totals[index][channel];
      if (weight === 0 || weight >= 1) continue;
      const rest = manifest.joints[index][channel];
      locals[index][channel] =
        channel === "rotation"
          ? characterSlerp(locals[index][channel], rest, 1 - weight)
          : locals[index][channel].map(
              (v, k) => v * weight + rest[k] * (1 - weight),
            );
    }
  }
  return locals;
}
/** Keep authored gait phase tied to distance that collision actually permits. */
export function characterClipTime(
  manifest: CharacterManifest,
  motion: CharacterMotion,
  elapsedSeconds: number,
  travelMeters: number,
) {
  finite(elapsedSeconds, 0, 1e12);
  finite(travelMeters, 0, 1e12);
  const clip = manifest.clips.find((candidate) => candidate.name === motion);
  if (!clip) throw new Error("Choose a prepared character movement.");
  return clip.distance > 0
    ? (travelMeters / clip.distance) * clip.duration
    : elapsedSeconds;
}
function localMatrix(local: CharacterLocal) {
  const [x, y, z, w] = local.rotation,
    [sx, sy, sz] = local.scale,
    [tx, ty, tz] = local.translation;
  return new Float32Array([
    (1 - 2 * (y * y + z * z)) * sx,
    2 * (x * y + z * w) * sx,
    2 * (x * z - y * w) * sx,
    0,
    2 * (x * y - z * w) * sy,
    (1 - 2 * (x * x + z * z)) * sy,
    2 * (y * z + x * w) * sy,
    0,
    2 * (x * z + y * w) * sz,
    2 * (y * z - x * w) * sz,
    (1 - 2 * (x * x + y * y)) * sz,
    0,
    tx,
    ty,
    tz,
    1,
  ]);
}
/** Position and inverse-transpose normal palettes, 32 floats per joint. */
export function characterPalette(
  manifest: CharacterManifest,
  locals: CharacterLocal[],
  out = new Float32Array(manifest.joints.length * 32),
) {
  if (
    locals.length !== manifest.joints.length ||
    out.length !== manifest.joints.length * 32
  )
    throw new Error("Character pose has an incorrect joint count.");
  const globals: Float32Array[] = [];
  for (let i = 0; i < locals.length; i++) {
    const joint = manifest.joints[i],
      matrix = localMatrix(locals[i]);
    const global =
      joint.parent < 0 ? matrix : multiply(globals[joint.parent], matrix);
    globals.push(global);
    const skin = multiply(global, joint.inverseBind),
      normal = inverse(skin);
    out.set(skin, i * 32);
    for (let c = 0; c < 4; c++)
      for (let r = 0; r < 4; r++)
        out[i * 32 + 16 + c * 4 + r] = normal[r * 4 + c];
  }
  return out;
}
export class CharacterAnimator {
  private previous?: CharacterLocal[];
  private current?: CharacterLocal[];
  private motion?: CharacterMotion;
  private transitionTime = 0;
  private lastClock = -Infinity;
  private palette: Float32Array<ArrayBuffer>;
  constructor(readonly manifest: CharacterManifest) {
    validateCharacter(manifest);
    this.palette = new Float32Array(manifest.joints.length * 32);
  }
  reset() {
    this.previous = undefined;
    this.current = undefined;
    this.motion = undefined;
    this.transitionTime = 0;
    this.lastClock = -Infinity;
  }
  sample(
    motion: CharacterMotion,
    time: number,
    clock: number,
    blend?: CharacterBlend,
  ) {
    finite(clock, -1e12, 1e12);
    finite(time, -1e12, 1e12);
    if (
      blend?.layers &&
      !this.manifest.clips.some((clip) => clip.name === motion)
    )
      throw new Error("Choose a prepared character movement.");
    const target = blend?.layers
      ? sampleCharacterLayers(this.manifest, blend.layers)
      : sampleCharacterLocals(this.manifest, motion, time);
    if (blend || this.manifest.controller?.profile === "quick-mmorpg-v1") {
      if (blend) {
        finite(blend.fromTime, -1e12, 1e12);
        finite(blend.weight, 0, 1);
        if (blend.layers) {
          this.current = target;
        } else {
          const source = sampleCharacterLocals(
              this.manifest,
              blend.from,
              blend.fromTime,
            ),
            t = blend.weight;
          this.current = target.map((local, i) => ({
            translation: local.translation.map(
              (v, k) => source[i].translation[k] * (1 - t) + v * t,
            ),
            rotation: characterSlerp(source[i].rotation, local.rotation, t),
            scale: local.scale.map(
              (v, k) => source[i].scale[k] * (1 - t) + v * t,
            ),
          }));
        }
      } else this.current = target;
      this.previous = undefined;
      this.motion = motion;
      this.lastClock = clock;
      return characterPalette(this.manifest, this.current, this.palette);
    }
    if (clock < this.lastClock) {
      this.current = undefined;
      this.previous = undefined;
    }
    if (motion !== this.motion) {
      this.previous = this.current;
      this.transitionTime = clock;
      this.motion = motion;
    }
    const t = Math.min(1, Math.max(0, (clock - this.transitionTime) / 0.12));
    this.current =
      this.previous && t < 1
        ? target.map((local, i) => ({
            translation: local.translation.map(
              (v, k) => this.previous![i].translation[k] * (1 - t) + v * t,
            ),
            rotation: characterSlerp(
              this.previous![i].rotation,
              local.rotation,
              t,
            ),
            scale: local.scale.map(
              (v, k) => this.previous![i].scale[k] * (1 - t) + v * t,
            ),
          }))
        : target;
    this.lastClock = clock;
    return characterPalette(this.manifest, this.current, this.palette);
  }
}
export async function fetchCharacterBytes(
  url: string,
  limit: number,
  expected?: CharacterBlob,
) {
  return readAsset(
    url,
    async (response) => {
      if (
        !response.ok ||
        !response.body ||
        Number(response.headers.get("content-length")) > limit
      )
        throw new Error("Prepare the character before opening this world.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let size = 0;
      try {
        for (;;) {
          const next = await reader.read();
          if (next.done) break;
          size += next.value.byteLength;
          if (size > limit)
            throw new Error("The character file exceeds its byte budget.");
          chunks.push(next.value);
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
      if (expected) {
        const hash = [
          ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
        ]
          .map((v) => v.toString(16).padStart(2, "0"))
          .join("");
        if (size !== expected.byteLength || hash !== expected.sha256)
          throw new AssetIntegrityError(
            "The character file is incomplete. Try again.",
          );
      }
      return bytes.buffer;
    },
    { init: { credentials: "same-origin", redirect: "error" } },
  );
}
export async function checkedCharacterBlob(
  description: CharacterBlob,
  load = fetchCharacterBytes,
) {
  blob(description);
  const bytes = await load(
    description.url,
    description.byteLength,
    description,
  );
  if (bytes.byteLength !== description.byteLength)
    throw new Error("The character file has an incorrect byte count.");
  const hash = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
  if (hash !== description.sha256)
    throw new Error("The prepared character changed; prepare it again.");
  return bytes;
}
/** Check the declared PNG extent before invoking the browser image decoder. */
export function validateCharacterPNG(
  bytes: ArrayBuffer,
  width: number,
  height: number,
) {
  const data = new Uint8Array(bytes),
    view = new DataView(bytes);
  if (
    data.length < 33 ||
    [137, 80, 78, 71, 13, 10, 26, 10].some((v, i) => data[i] !== v) ||
    view.getUint32(8) !== 13 ||
    view.getUint32(12) !== 0x49484452 ||
    view.getUint32(16) !== width ||
    view.getUint32(20) !== height
  )
    throw new Error("The character texture has an incorrect PNG extent.");
}
export async function loadCharacterManifest(
  url = "/private-assets/character-druid.json",
  load = fetchCharacterBytes,
) {
  if (
    !/^\/private-assets\/character-quick-mmorpg-(?:paladin|sorceror|zombie|warrok)-[0-9a-f]{64}\.json$/.test(
      url,
    ) &&
    ![
      "/private-assets/character-druid.json",
      "/private-assets/character-knight.json",
      "/starter/character.json",
    ].includes(url)
  )
    throw new Error("Choose the prepared local character recipe.");
  return validateCharacter(
    JSON.parse(
      new TextDecoder().decode(await load(url, CHARACTER_LIMITS.manifestBytes)),
    ),
  );
}
