import type { AssetDescription, SourceRecord } from "./asset-compiler.ts";

export const ASSET_IMPORT_VERSION = 1;
export const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
export const MAX_SOURCE_STORE_BYTES = 32 * 1024 * 1024;
export const MAX_SOURCE_COUNT = 64;
export const MAX_OUTPUT_BYTES = 16 * 1024 * 1024;
export const MAX_COMPILED_CACHE_BYTES = 16 * 1024 * 1024;

export type SourceBytes = { record: SourceRecord; bytes: Uint8Array };

// The converter runs in the import worker. Image bytes are decoded RGBA8;
// original PNG/JPEG data stays in SourceBytes and the source store.
export type AssetConverter = (
  source: SourceRecord,
  bytes: Uint8Array,
  files: readonly SourceBytes[],
) => Promise<AssetDescription>;

export type AssetImportRequest =
  | { type: "put"; requestId: string; source: SourceBytes }
  | { type: "compile"; requestId: string; revision: number; source: SourceRecord; files: SourceRecord[] }
  | { type: "cancel"; requestId: string; target: string }
  | { type: "stats"; requestId: string };

export type ImportMetrics = {
  compileMs: number;
  transferredBytes: number;
  outputBytes: number;
  cacheHit: boolean;
};

export type WorkerStats = {
  sourceBytes: number;
  sourceCount: number;
  cacheBytes: number;
  cacheEntries: number;
  conversions: number;
  active: number;
};

export type AssetImportResponse =
  | { type: "stored"; requestId: string; evicted: string[] }
  | { type: "compiled"; version: 1; requestId: string; revision: number; description: AssetDescription; metrics: ImportMetrics }
  | { type: "cancelled"; requestId: string }
  | { type: "stats"; requestId: string; stats: WorkerStats }
  | { type: "error"; requestId: string; source: string; message: string };

export function assetPath(path: string): string {
  if (typeof path !== "string" || path.length === 0 || path.length > 512 ||
    /[\\\0?#:]/.test(path) || path.startsWith("/")) throw new Error("Choose a project-local asset path.");
  const parts: string[] = [];
  for (const part of path.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") {
      if (!parts.length) throw new Error("The asset path leaves this project.");
      parts.pop();
    } else parts.push(part);
  }
  if (!parts.length) throw new Error("Choose an asset file.");
  return parts.join("/");
}

export function sourceKey(source: SourceRecord): string {
  if (!/^[A-Za-z0-9._-]{1,128}$/.test(source.id) ||
    !["glb", "gltf", "image"].includes(source.kind) ||
    assetPath(source.path) !== source.path ||
    !/^[a-f0-9]{64}$/.test(source.hash) ||
    !Number.isSafeInteger(source.bytes) || source.bytes < 1 || source.bytes > MAX_SOURCE_BYTES) {
    throw new Error("The asset source is invalid or too large.");
  }
  return `${source.id}:${source.hash}`;
}

export function descriptionBytes(description: AssetDescription): number {
  return description.meshes.reduce((n, mesh) => n + mesh.positions.byteLength + mesh.indices.byteLength, 0) +
    description.images.reduce((n, image) => n + image.bytes.byteLength, 0);
}

export function descriptionTransfers(description: AssetDescription): ArrayBuffer[] {
  const buffers = new Set<ArrayBuffer>();
  for (const mesh of description.meshes) {
    buffers.add(mesh.positions.buffer as ArrayBuffer);
    buffers.add(mesh.indices.buffer as ArrayBuffer);
  }
  for (const image of description.images) buffers.add(image.bytes.buffer as ArrayBuffer);
  return [...buffers];
}

export function sameSource(a: SourceRecord, b: SourceRecord): boolean {
  return a.id === b.id && a.kind === b.kind && a.path === b.path && a.hash === b.hash && a.bytes === b.bytes;
}

export function validateImportedAsset(description: AssetDescription, source: SourceRecord, inputs: readonly SourceRecord[]): void {
  const fail = () => { throw new Error(`${source.path}: the converted asset is invalid or too large.`); };
  if (!description || description.id !== source.id || !Array.isArray(description.sources) ||
    !Array.isArray(description.meshes) || !Array.isArray(description.images) ||
    !Array.isArray(description.diagnostics) || description.sources.length > MAX_SOURCE_COUNT ||
    description.meshes.length > 64 || description.images.length > 16 || description.diagnostics.length > 64 ||
    description.meshes.length + description.images.length === 0) fail();
  const sourceIds = new Set<string>();
  for (const record of description.sources) {
    sourceKey(record);
    if (sourceIds.has(record.id) || !inputs.some(input => sameSource(input, record))) fail();
    sourceIds.add(record.id);
  }
  if (!description.sources.some(record => sameSource(record, source))) fail();
  const ids = new Set<string>();
  for (const mesh of description.meshes) {
    if (!mesh || typeof mesh.id !== "string" || !mesh.id || mesh.id.length > 256 || ids.has(mesh.id) || !sourceIds.has(mesh.source) ||
      !(mesh.positions instanceof Float32Array) || !(mesh.indices instanceof Uint32Array) ||
      !Number.isInteger(mesh.vertexCount) || mesh.vertexCount < 3 || mesh.vertexCount > 65_536 ||
      !Number.isInteger(mesh.indexCount) || mesh.indexCount < 3 || mesh.indexCount > 196_608 || mesh.indexCount % 3 !== 0 ||
      mesh.positions.length !== mesh.vertexCount * 3 || mesh.indices.length !== mesh.indexCount ||
      !mesh.bounds || mesh.bounds.min.length !== 3 || mesh.bounds.max.length !== 3) fail();
    ids.add(mesh.id);
    for (let axis = 0; axis < 3; axis++) {
      const lo = mesh.bounds.min[axis], hi = mesh.bounds.max[axis];
      if (!Number.isFinite(lo) || !Number.isFinite(hi) || lo > hi) fail();
      for (let at = axis; at < mesh.positions.length; at += 3) {
        const value = mesh.positions[at];
        if (!Number.isFinite(value) || value < lo || value > hi) fail();
      }
    }
    for (const index of mesh.indices) if (index >= mesh.vertexCount) fail();
  }
  for (const image of description.images) {
    if (!image || typeof image.id !== "string" || !image.id || image.id.length > 256 || ids.has(image.id) || !sourceIds.has(image.source) ||
      !["image/png", "image/jpeg"].includes(image.mime) || !(image.bytes instanceof Uint8Array) ||
      !Number.isInteger(image.width) || !Number.isInteger(image.height) ||
      image.width < 1 || image.height < 1 || image.width > 4096 || image.height > 4096 ||
      image.bytes.byteLength !== image.width * image.height * 4 || image.bytes.byteLength > 4 * 1024 * 1024) fail();
    ids.add(image.id);
  }
  for (const diagnostic of description.diagnostics) {
    if (!diagnostic || diagnostic.severity !== "warning" || typeof diagnostic.message !== "string" ||
      diagnostic.message.length > 2048 || typeof diagnostic.code !== "string" || diagnostic.code.length > 128 ||
      typeof diagnostic.source !== "string" || diagnostic.source.length > 512) fail();
  }
  if (descriptionBytes(description) > MAX_OUTPUT_BYTES) fail();
}

export function copyImportedAsset(description: AssetDescription): AssetDescription {
  return {
    id: description.id,
    sources: description.sources.map(record => ({ id: record.id, kind: record.kind, path: record.path, hash: record.hash, bytes: record.bytes })),
    meshes: description.meshes.map(mesh => ({ id: mesh.id, source: mesh.source, vertexCount: mesh.vertexCount, indexCount: mesh.indexCount,
      positions: mesh.positions.slice(), indices: mesh.indices.slice(), bounds: { min: [...mesh.bounds.min], max: [...mesh.bounds.max] } })),
    images: description.images.map(image => ({ id: image.id, source: image.source, mime: image.mime, width: image.width, height: image.height, bytes: image.bytes.slice() })),
    diagnostics: description.diagnostics.map(diagnostic => ({ code: diagnostic.code, severity: diagnostic.severity, source: diagnostic.source, message: diagnostic.message })),
  };
}
