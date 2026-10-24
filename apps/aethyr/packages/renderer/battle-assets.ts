import {
  readAsset,
  verifyAssetBytes,
  type AssetDownloadOptions,
} from "../assets/src/asset-download.ts";

export const BATTLE_CLASSES = [
  "paladin",
  "sorceror",
  "zombie",
  "warrok",
] as const;
export const BATTLE_CLIPS = ["idle", "walk", "attack", "death"] as const;
export const BATTLE_VAT_SAMPLE_RATE = 60;
export type BattleClass = (typeof BATTLE_CLASSES)[number];
export type BattleClipName = (typeof BATTLE_CLIPS)[number];
export interface BattleClip {
  duration: number;
  firstFrame: number;
  frameCount: number;
  loop: boolean;
}
interface BattleBlob {
  url: string;
  sha256: string;
  byteLength: number;
}
export interface BattleImpostorLayout {
  views: 8;
  tileSize: 32;
  width: 512;
  height: 512;
  frames: number;
  bounds: { minY: number; maxY: number; radius: number };
  clips: Record<BattleClipName, BattleClip>;
}
export interface BattleImpostorRecipe extends BattleImpostorLayout {
  color: BattleBlob;
  normal: BattleBlob;
}
export interface BattleImpostorAssets extends BattleImpostorLayout {
  color: Uint8Array<ArrayBuffer>;
  normal: Uint8Array<ArrayBuffer>;
}
export interface BattleCrowdClassRecipe {
  kind: BattleClass;
  height: number;
  vertexCount: number;
  indexCount: number;
  vertices: BattleBlob;
  indices: BattleBlob;
  lods: [
    { firstIndex: number; indexCount: number },
    { firstIndex: number; indexCount: number },
  ];
  vat: {
    width: number;
    height: number;
    positions: BattleBlob;
    normals: BattleBlob;
  };
  materials: {
    alphaMode: "OPAQUE";
    width: 256;
    height: 256;
    layers: number;
    rgba: BattleBlob;
  };
  clips: Record<BattleClipName, BattleClip>;
  impostor?: BattleImpostorRecipe;
}
export interface BattleCrowdManifest {
  schema: "aethyrforge.battle-crowd.v1";
  sampleRate: 20 | 60;
  classes: BattleCrowdClassRecipe[];
  source?: Record<string, unknown>;
}
export interface BattleCrowdClassAssets {
  kind: BattleClass;
  height: number;
  /** UV0 f32x2, VAT column u32, and material layer u32; stride 16 bytes. */
  vertices: ArrayBuffer;
  indices: Uint32Array<ArrayBuffer>;
  lods: BattleCrowdClassRecipe["lods"];
  /** Row = baked frame, column = vertex. Both planes are RGBA16float. */
  vat: {
    width: number;
    height: number;
    positions: Uint16Array<ArrayBuffer>;
    normals: Uint16Array<ArrayBuffer>;
  };
  /** Tightly packed layer-major sRGB color, including source color factors. */
  materials: {
    alphaMode: "OPAQUE";
    width: 256;
    height: 256;
    layers: number;
    rgba: Uint8Array<ArrayBuffer>;
  };
  clips: Record<BattleClipName, BattleClip>;
  impostor?: BattleImpostorAssets;
}
export const BATTLE_IMPOSTOR_BYTES = 512 * 512 * 8 * 4;
export function validateBattleImpostorLayout(
  value: BattleImpostorLayout,
  clips: Record<BattleClipName, BattleClip>,
) {
  if (
    !value ||
    value.views !== 8 ||
    value.tileSize !== 32 ||
    value.width !== 512 ||
    value.height !== 512
  )
    throw Error("Prepare a supported distant battle character.");
  integer(value.frames, 8, 256);
  if (!value.bounds) throw Error("Prepare complete distant character bounds.");
  finite(value.bounds.minY, -20, 20);
  finite(value.bounds.maxY, -20, 20);
  finite(value.bounds.radius, 0.01, 20);
  if (value.bounds.maxY <= value.bounds.minY)
    throw Error("Prepare complete distant character bounds.");
  let next = 0;
  for (const name of BATTLE_CLIPS) {
    const clip = value.clips?.[name],
      source = clips[name];
    if (
      !clip ||
      !source ||
      clip.duration !== source.duration ||
      clip.loop !== source.loop ||
      clip.firstFrame !== next ||
      clip.frameCount !== Math.ceil(clip.duration * 12) + 1
    )
      throw Error("Distant battle movements must match their source timing.");
    next += clip.frameCount;
  }
  if (next !== value.frames)
    throw Error("Prepare complete distant battle movements.");
}
export const BATTLE_ASSET_LIMITS = Object.freeze({
  manifestBytes: 512 * 1024,
  blobBytes: 32 * 1024 * 1024,
  totalBytes: 128 * 1024 * 1024,
  vertices: 4096,
  atlasHeight: 4096,
  materialLayers: 8,
  nearTriangles: 600,
  farTriangles: 150,
});
function integer(value: number, min: number, max: number) {
  if (!Number.isInteger(value) || value < min || value > max)
    throw new Error("Prepare a supported battle crowd size.");
}
function finite(value: number, min: number, max: number) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < min ||
    value > max
  )
    throw new Error("The battle crowd contains an unsupported number.");
}
function blob(value: BattleBlob) {
  if (
    !value ||
    !/^[0-9a-f]{64}$/.test(value.sha256) ||
    !new RegExp(
      `^/private-assets/battle-${value.sha256}\\.[a-z-]+\\.bin$`,
    ).test(value.url)
  )
    throw new Error("Prepare the battle crowd with its file hashes.");
  integer(value.byteLength, 1, BATTLE_ASSET_LIMITS.blobBytes);
}
export function validateBattleCrowd(value: BattleCrowdManifest) {
  if (
    value?.schema !== "aethyrforge.battle-crowd.v1" ||
    (value.sampleRate !== 20 && value.sampleRate !== BATTLE_VAT_SAMPLE_RATE) ||
    !Array.isArray(value.classes) ||
    value.classes.length !== 4
  )
    throw new Error("Prepare all four battle crowd characters.");
  let total = 0;
  const seen = new Set<BattleClass>();
  for (const model of value.classes) {
    if (!model || !BATTLE_CLASSES.includes(model.kind) || seen.has(model.kind))
      throw new Error(
        "The battle crowd needs four distinct character classes.",
      );
    seen.add(model.kind);
    finite(model.height, 0.1, 20);
    integer(model.vertexCount, 3, BATTLE_ASSET_LIMITS.vertices);
    integer(
      model.indexCount,
      6,
      (BATTLE_ASSET_LIMITS.nearTriangles + BATTLE_ASSET_LIMITS.farTriangles) *
        3,
    );
    if (
      !model.vat ||
      !model.materials ||
      !model.clips ||
      !Array.isArray(model.lods) ||
      model.lods.length !== 2
    )
      throw new Error(
        "Prepare complete battle crowd animations and materials.",
      );
    integer(model.vat.width, model.vertexCount, BATTLE_ASSET_LIMITS.vertices);
    integer(model.vat.height, 8, BATTLE_ASSET_LIMITS.atlasHeight);
    if (
      (model.vat.width & (model.vat.width - 1)) !== 0 ||
      model.materials.width !== 256 ||
      model.materials.height !== 256 ||
      model.materials.alphaMode !== "OPAQUE"
    )
      throw new Error("Prepare supported battle crowd texture dimensions.");
    integer(model.materials.layers, 1, BATTLE_ASSET_LIMITS.materialLayers);
    const files = [
      model.vertices,
      model.indices,
      model.vat.positions,
      model.vat.normals,
      model.materials.rgba,
    ];
    if (model.impostor) {
      validateBattleImpostorLayout(model.impostor, model.clips);
      for (const file of [model.impostor.color, model.impostor.normal]) {
        if (file?.byteLength !== BATTLE_IMPOSTOR_BYTES)
          throw Error("Distant battle images have inconsistent lengths.");
        files.push(file);
      }
    }
    files.forEach((file) => {
      blob(file);
      total += file.byteLength;
    });
    if (
      model.vertices.byteLength !== model.vertexCount * 16 ||
      model.indices.byteLength !== model.indexCount * 4 ||
      model.vat.positions.byteLength !==
        model.vat.width * model.vat.height * 8 ||
      model.vat.normals.byteLength !== model.vat.positions.byteLength ||
      model.materials.rgba.byteLength !== 256 * 256 * model.materials.layers * 4
    )
      throw new Error("Battle crowd buffers have inconsistent lengths.");
    let first = 0;
    model.lods.forEach((lod, index) => {
      integer(lod.firstIndex, 0, model.indexCount - 3);
      integer(
        lod.indexCount,
        3,
        (index === 0
          ? BATTLE_ASSET_LIMITS.nearTriangles
          : BATTLE_ASSET_LIMITS.farTriangles) * 3,
      );
      if (lod.firstIndex !== first || lod.indexCount % 3)
        throw new Error(
          "Battle crowd LODs need complete nonoverlapping triangles.",
        );
      first += lod.indexCount;
    });
    if (first !== model.indexCount)
      throw new Error("Battle crowd LODs must cover their index buffer.");
    let frame = 0;
    for (const name of BATTLE_CLIPS) {
      const clip = model.clips[name];
      if (!clip || typeof clip.loop !== "boolean")
        throw new Error("Prepare every battle crowd movement.");
      finite(clip.duration, 1 / 120, 60);
      integer(clip.firstFrame, 0, model.vat.height - 2);
      integer(clip.frameCount, 2, 3601);
      if (clip.frameCount !== Math.ceil(clip.duration * value.sampleRate) + 1)
        throw new Error("Battle crowd frames must match their sample rate.");
      if (clip.firstFrame !== frame)
        throw new Error("Battle crowd clips need distinct frame ranges.");
      frame += clip.frameCount;
    }
    if (frame !== model.vat.height)
      throw new Error("Battle crowd frames must match the animation atlas.");
  }
  if (total > BATTLE_ASSET_LIMITS.totalBytes)
    throw new Error("The battle crowd exceeds its memory budget.");
  return value;
}
async function boundedBytes(response: Response, limit: number) {
  if (!response.ok || !response.body)
    throw new Error("The battle crowd could not open.");
  const length = response.headers.get("content-length");
  if (length !== null && (!/^\d+$/.test(length) || Number(length) > limit))
    throw new Error("The battle crowd file is too large.");
  const reader = response.body.getReader(),
    parts: Uint8Array<ArrayBuffer>[] = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) throw new Error("The battle crowd file is too large.");
      parts.push(value);
    }
  } catch (error) {
    await reader.cancel().catch(() => {});
    throw error;
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const part of parts) {
    bytes.set(part, offset);
    offset += part.byteLength;
  }
  return bytes;
}
function halfFinite(value: number) {
  return (value & 0x7c00) !== 0x7c00;
}
export function validateBattleCrowdBuffers(
  recipe: BattleCrowdClassRecipe,
  model: BattleCrowdClassAssets,
) {
  const data = new DataView(model.vertices);
  if (
    model.vertices.byteLength !== recipe.vertexCount * 16 ||
    model.indices.length !== recipe.indexCount
  )
    throw new Error("The battle crowd geometry is incomplete.");
  for (let i = 0; i < recipe.vertexCount; i++) {
    finite(data.getFloat32(i * 16, true), -10000, 10000);
    finite(data.getFloat32(i * 16 + 4, true), -10000, 10000);
    integer(data.getUint32(i * 16 + 8, true), 0, recipe.vertexCount - 1);
    integer(data.getUint32(i * 16 + 12, true), 0, recipe.materials.layers - 1);
  }
  for (const index of model.indices) integer(index, 0, recipe.vertexCount - 1);
  if (
    model.materials.alphaMode !== "OPAQUE" ||
    model.materials.rgba.length !== recipe.materials.rgba.byteLength
  )
    throw new Error("Prepare the battle crowd with opaque source materials.");
  for (let i = 3; i < model.materials.rgba.length; i += 4)
    if (model.materials.rgba[i] !== 255)
      throw new Error("Battle crowd source materials must stay opaque.");
  for (const plane of [model.vat.positions, model.vat.normals])
    for (const component of plane)
      if (!halfFinite(component))
        throw new Error("The battle crowd animation contains an invalid pose.");
  if (
    !!recipe.impostor !== !!model.impostor ||
    (model.impostor &&
      (model.impostor.color.byteLength !== BATTLE_IMPOSTOR_BYTES ||
        model.impostor.normal.byteLength !== BATTLE_IMPOSTOR_BYTES))
  )
    throw Error("The distant battle images are incomplete.");
  return model;
}
export async function loadBattleCrowdAssets(
  url: string,
  options: AssetDownloadOptions = {},
) {
  const match = /^\/private-assets\/battle-crowd-([0-9a-f]{64})\.json$/.exec(
    url,
  );
  if (!match) throw new Error("Choose a prepared battle crowd.");
  const manifest = await readAsset(
    url,
    async (response) => {
      const bytes = await boundedBytes(
        response,
        BATTLE_ASSET_LIMITS.manifestBytes,
      );
      await verifyAssetBytes(bytes, {
        byteLength: bytes.byteLength,
        sha256: match[1],
      });
      return validateBattleCrowd(
        JSON.parse(new TextDecoder().decode(bytes)) as BattleCrowdManifest,
      );
    },
    options,
  );
  const classes: BattleCrowdClassAssets[] = [];
  let bytes = 0;
  for (const model of manifest.classes) {
    const load = (file: BattleBlob) =>
      readAsset(
        file.url,
        async (response) => {
          const data = await boundedBytes(response, file.byteLength);
          await verifyAssetBytes(data, file);
          bytes += data.byteLength;
          return data.buffer;
        },
        options,
      );
    const [vertices, indices, positions, normals, rgba] = await Promise.all([
      load(model.vertices),
      load(model.indices),
      load(model.vat.positions),
      load(model.vat.normals),
      load(model.materials.rgba),
    ]);
    const impostor = model.impostor;
    const sprite = impostor
      ? await Promise.all([load(impostor.color), load(impostor.normal)])
      : undefined;
    classes.push(
      validateBattleCrowdBuffers(model, {
        kind: model.kind,
        height: model.height,
        vertices,
        indices: new Uint32Array(indices),
        lods: model.lods,
        vat: {
          width: model.vat.width,
          height: model.vat.height,
          positions: new Uint16Array(positions),
          normals: new Uint16Array(normals),
        },
        materials: {
          alphaMode: "OPAQUE",
          width: 256,
          height: 256,
          layers: model.materials.layers,
          rgba: new Uint8Array(rgba),
        },
        clips: model.clips,
        ...(impostor && sprite
          ? {
              impostor: {
                ...impostor,
                color: new Uint8Array(sprite[0]),
                normal: new Uint8Array(sprite[1]),
              },
            }
          : {}),
      }),
    );
  }
  return { classes, bytes };
}
