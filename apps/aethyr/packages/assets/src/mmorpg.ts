import {
  readAsset,
  verifyAssetBytes,
  type AssetDownloadOptions,
} from "./asset-download.ts";
import type { CompiledMaterial, CompiledMesh } from "./scene.ts";
import type {
  CharacterBlob,
  CharacterWeapon,
} from "../../renderer/character.ts";

export const MMORPG_CLASSES = [
  "paladin",
  "sorceror",
  "zombie",
  "warrok",
] as const;
export type MmorpgClass = (typeof MMORPG_CLASSES)[number];
export type MmorpgState =
  "idle" | "walk" | "run" | "attack" | "dance" | "death";
export interface MmorpgActorPose {
  id: number;
  class: MmorpgClass;
  name: string;
  position: [number, number, number];
  yaw: number;
  state: MmorpgState;
  actionTime: number;
  animationTime: number;
  health: number;
  maxHealth: number;
  alive: boolean;
  weapon: CharacterWeapon | null;
}
export interface MmorpgImage extends CharacterBlob {
  width: number;
  height: number;
}
export interface MmorpgSceneryDefinition {
  id: string;
  file: string;
  scale: number;
  biomes: ("forest" | "arid" | "desert")[];
  collidable: boolean;
}
export interface MmorpgSceneryPlacement {
  kind: string;
  cell: [number, number];
  position: [number, number, number];
  yaw: number;
  scale: number;
}
export interface MmorpgWorldManifest {
  schema: "mmorpg-world.v1";
  worldScale: number;
  seed: number;
  sourceRepository: string;
  sourceCommit: string;
  characters: Record<MmorpgClass, string>;
  classClips: Record<MmorpgClass, Partial<Record<MmorpgState, number>>>;
  uiURL: string;
  icons: Record<string, MmorpgImage>;
  particleTextures: Record<"blood" | "fire" | "levelUp", MmorpgImage>;
  scenery: {
    vertices: CharacterBlob;
    indices: CharacterBlob;
    vertexCount: number;
    indexCount: number;
    vertexStride: 48;
    meshes: CompiledMesh[];
    materials: CompiledMaterial[];
    catalog: MmorpgSceneryDefinition[];
    placements: MmorpgSceneryPlacement[];
    windowCells: 21;
    cellMetres: number;
    seed: 2;
  };
  terrain?: {
    firstVertex: number;
    vertexCount: number;
    resolution: number;
    sizeMetres: number;
  };
  simulation: {
    seed: number;
    worldScale: number;
    spawn: [number, number, number];
    yaw: number;
    release?: string;
    colliders?: { min: number[]; max: number[] }[];
  };
}
const finite = (v: unknown, min: number, max: number): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= min && v <= max;
const privateURL = (url: unknown): url is string =>
  typeof url === "string" &&
  /^\/private-assets\/[a-zA-Z0-9._/-]+$/.test(url) &&
  !url.includes("..");
function blob(value: CharacterBlob) {
  if (
    !value ||
    !privateURL(value.url) ||
    !finite(value.byteLength, 1, 64 * 1024 * 1024) ||
    !Number.isInteger(value.byteLength) ||
    !/^[0-9a-f]{64}$/.test(value.sha256)
  )
    throw Error("Prepare complete MMORPG world files.");
}
export function validateMmorpgWorld(
  value: MmorpgWorldManifest,
): MmorpgWorldManifest {
  if (
    value?.schema !== "mmorpg-world.v1" ||
    value.worldScale !== 1 / 6 ||
    value.seed !== 1 ||
    !value.characters ||
    !value.classClips ||
    !/^\/private-assets\/mmorpg-ui-[0-9a-f]{64}\.json$/.test(value.uiURL)
  )
    throw Error("Choose a prepared MMORPG world.");
  for (const kind of MMORPG_CLASSES) {
    if (!privateURL(value.characters[kind]) || !value.classClips[kind])
      throw Error("Prepare all four MMORPG character classes.");
    for (const clip of ["idle", "walk", "attack", "death"] as const)
      if (!finite(value.classClips[kind][clip], 0.001, 120))
        throw Error("Prepare the authored character animations.");
  }
  for (const image of [
    ...Object.values(value.icons ?? {}),
    ...Object.values(value.particleTextures ?? {}),
  ]) {
    blob(image);
    if (
      !Number.isInteger(image.width) ||
      !Number.isInteger(image.height) ||
      !finite(image.width, 1, 4096) ||
      !finite(image.height, 1, 4096)
    )
      throw Error("Prepare bounded world pictures.");
  }
  if (
    !value.icons ||
    Object.keys(value.icons).length !== 9 ||
    !value.particleTextures ||
    Object.keys(value.particleTextures).length !== 3
  )
    throw Error("Prepare the original world pictures.");
  const s = value.scenery;
  if (
    !s ||
    s.vertexStride !== 48 ||
    s.windowCells !== 21 ||
    s.seed !== 2 ||
    !finite(s.cellMetres, 8, 9) ||
    !Array.isArray(s.catalog) ||
    s.catalog.length !== 36 ||
    !Array.isArray(s.meshes) ||
    s.meshes.length !== 36 ||
    !Array.isArray(s.materials) ||
    s.materials.length > 128 ||
    !Array.isArray(s.placements) ||
    s.placements.length > 441
  )
    throw Error("Prepare the bounded source scenery window.");
  blob(s.vertices);
  blob(s.indices);
  if (
    s.vertices.byteLength !== s.vertexCount * 48 ||
    s.indices.byteLength !== s.indexCount * 4 ||
    s.indexCount % 3
  )
    throw Error("The scenery geometry sizes do not match.");
  const kinds = new Set(s.catalog.map((x) => x.id));
  if (kinds.size !== 36) throw Error("The scenery catalog repeats an asset.");
  for (const p of s.placements)
    if (
      !kinds.has(p.kind) ||
      p.position.length !== 3 ||
      p.position.some((x) => !finite(x, -20000, 20000)) ||
      !finite(p.yaw, -1e6, 1e6) ||
      !finite(p.scale, 0.001, 2) ||
      p.cell.length !== 2 ||
      p.cell.some((x) => !Number.isInteger(x) || Math.abs(x) > 10000)
    )
      throw Error("Prepare finite scenery placements.");
  const sim = value.simulation;
  if (
    !sim ||
    sim.seed !== 1 ||
    sim.worldScale !== 1 / 6 ||
    sim.spawn?.length !== 3 ||
    sim.spawn.some((x) => !finite(x, -20000, 20000)) ||
    !finite(sim.yaw, -1e6, 1e6)
  )
    throw Error("Prepare the source world spawn.");
  return value;
}
/** Read an immutable document before interpreting any of its fields. */
export async function loadMmorpgDocument(
  url: string,
  kind: "world" | "ui",
  maxBytes: number,
  options: AssetDownloadOptions = {},
): Promise<unknown> {
  const identity = url.match(
    /^\/private-assets\/mmorpg-(world|ui)-([0-9a-f]{64})\.json$/,
  );
  if (!identity || identity[1] !== kind)
    throw Error("Choose a prepared game file.");
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 1 ||
    maxBytes > 2 * 1024 * 1024
  )
    throw Error("Choose a supported game file size.");
  return readAsset(
    url,
    async (response) => {
      if (!response.ok || !response.body)
        throw Error("The world could not finish loading. Try again.");
      const chunks: Uint8Array[] = [];
      let length = 0;
      const reader = response.body.getReader();
      try {
        if (Number(response.headers.get("content-length")) > maxBytes)
          throw Error("The world recipe is too large.");
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          length += value.byteLength;
          if (length > maxBytes) throw Error("The world recipe is too large.");
          chunks.push(value);
        }
      } catch (error) {
        await reader.cancel().catch(() => {});
        throw error;
      } finally {
        reader.releaseLock();
      }
      const bytes = new Uint8Array(length);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      await verifyAssetBytes(bytes, {
        byteLength: length,
        sha256: identity[2],
      });
      return JSON.parse(new TextDecoder().decode(bytes));
    },
    options,
  );
}
export async function loadMmorpgWorld(
  url: string,
  options: AssetDownloadOptions = {},
) {
  return validateMmorpgWorld(
    (await loadMmorpgDocument(
      url,
      "world",
      2 * 1024 * 1024,
      options,
    )) as MmorpgWorldManifest,
  );
}
export function validateMmorpgActors(actors: readonly MmorpgActorPose[]) {
  if (!Array.isArray(actors) || actors.length > 64)
    throw Error("This view supports up to 64 characters.");
  const ids = new Set<number>();
  for (const a of actors) {
    if (
      !a ||
      !Number.isInteger(a.id) ||
      a.id < 0 ||
      a.id > 0xffffffff ||
      ids.has(a.id) ||
      !MMORPG_CLASSES.includes(a.class) ||
      typeof a.name !== "string" ||
      a.name.length > 64 ||
      !Array.isArray(a.position) ||
      a.position.length !== 3 ||
      a.position.some((x: number) => !finite(x, -20000, 20000)) ||
      !finite(a.yaw, -1e6, 1e6) ||
      !["idle", "walk", "run", "attack", "dance", "death"].includes(a.state) ||
      !finite(a.actionTime, 0, 1e12) ||
      !finite(a.animationTime, 0, 1e12) ||
      !finite(a.health, 0, 1e6) ||
      !finite(a.maxHealth, 1, 1e6) ||
      a.health > a.maxHealth ||
      a.alive !== a.health > 0 ||
      (!a.alive && a.state !== "death") ||
      ![null, "weapon.sword1", "weapon.axe1", "weapon.hammer1"].includes(
        a.weapon,
      )
    )
      throw Error("Choose complete world character poses.");
    ids.add(a.id);
  }
}
