/** Bounded, serializable asset descriptions for the Haunted House compiler. */
import { sourceBytesHash } from "./protocol.ts";

export const MAX_ASSET_BYTES = 8 * 1024 * 1024;
export const MAX_MESH_VERTICES = 65_536;
export const MAX_MESH_INDICES = 196_608;
export const MAX_IMAGES = 16;
export const MAX_IMAGE_BYTES = 4 * 1024 * 1024;

export type SourceKind = "glb" | "gltf" | "image";
export type AssetDiagnostic = {
  code: string;
  severity: "error" | "warning";
  source: string;
  message: string;
};

export type SourceRecord = {
  id: string;
  kind: SourceKind;
  path: string;
  bytes: number;
  hash: string;
};

export type MeshDescription = {
  id: string;
  source: string;
  positions: Float32Array;
  indices: Uint32Array;
  vertexCount: number;
  indexCount: number;
  bounds: { min: [number, number, number]; max: [number, number, number] };
};

export type ImageDescription = {
  id: string;
  source: string;
  mime: "image/png" | "image/jpeg";
  width: number;
  height: number;
  bytes: Uint8Array;
};

export type AssetDescription = {
  id: string;
  sources: SourceRecord[];
  meshes: MeshDescription[];
  images: ImageDescription[];
  diagnostics: AssetDiagnostic[];
};

export type GlbContainer = { json: unknown; bin?: Uint8Array };

const GLB_MAGIC = 0x46546c67;
const GLB_VERSION = 2;
const GLB_JSON_CHUNK = 0x4e4f534a;
const GLB_BIN_CHUNK = 0x004e4942;
const GLB_HEADER_BYTES = 12;
const GLB_CHUNK_HEADER_BYTES = 8;

function glbError(message: string): RangeError {
  return new RangeError(`invalid GLB: ${message}`);
}

function readU32(bytes: Uint8Array, offset: number): number {
  return (
    bytes[offset] |
    (bytes[offset + 1] << 8) |
    (bytes[offset + 2] << 16) |
    (bytes[offset + 3] << 24)
  ) >>> 0;
}

/** Parse a bounded GLB container, preserving the exact BIN view bytes. */
export function parseGlb(bytes: Uint8Array): GlbContainer {
  if (!(bytes instanceof Uint8Array)) throw glbError("input is not a Uint8Array");
  const offset = bytes.byteOffset;
  const length = bytes.byteLength;
  if (offset < 0 || length < 0 || offset + length > bytes.buffer.byteLength) {
    throw glbError("input view exceeds its backing buffer");
  }
  if (length > MAX_ASSET_BYTES) {
    throw glbError(`input exceeds ${MAX_ASSET_BYTES} bytes`);
  }
  if (length < GLB_HEADER_BYTES) throw glbError("truncated header");
  if (readU32(bytes, 0) !== GLB_MAGIC) throw glbError("bad magic");
  if (readU32(bytes, 4) !== GLB_VERSION) throw glbError("unsupported version");
  const declared = readU32(bytes, 8);
  if (declared !== length) throw glbError(`declared length ${declared} != ${length}`);

  let cursor = GLB_HEADER_BYTES;
  let json: unknown;
  let sawJson = false;
  let bin: Uint8Array | undefined;
  let sawBin = false;
  let ordinal = 0;
  while (cursor < length) {
    ordinal += 1;
    if (cursor + GLB_CHUNK_HEADER_BYTES > length) throw glbError("truncated chunk header");
    const chunkLength = readU32(bytes, cursor);
    const chunkType = readU32(bytes, cursor + 4);
    const payloadStart = cursor + GLB_CHUNK_HEADER_BYTES;
    if (chunkLength % 4 !== 0) throw glbError("chunk length is not 4-byte aligned");
    if (payloadStart + chunkLength > length) throw glbError("truncated chunk payload");
    const payload = bytes.subarray(payloadStart, payloadStart + chunkLength);
    if (chunkType === GLB_JSON_CHUNK) {
      if (sawJson) throw glbError("duplicate JSON chunk");
      if (cursor !== GLB_HEADER_BYTES) throw glbError("JSON chunk is not first");
      sawJson = true;
      let text: string;
      try {
        text = new TextDecoder("utf-8", { fatal: true }).decode(payload);
      } catch {
        throw glbError("malformed UTF-8 JSON chunk");
      }
      try {
        json = JSON.parse(text);
      } catch {
        throw glbError("malformed JSON chunk");
      }
    } else if (chunkType === GLB_BIN_CHUNK) {
      if (!sawJson) throw glbError("BIN chunk precedes JSON chunk");
      if (ordinal !== 2) throw glbError("BIN chunk is not second");
      if (sawBin) throw glbError("duplicate BIN chunk");
      sawBin = true;
      bin = payload;
    }
    cursor = payloadStart + chunkLength;
  }
  if (!sawJson) throw glbError("missing JSON chunk");
  return bin === undefined ? { json } : { json, bin };
}

type GltfAccessor = {
  bufferView?: number;
  byteOffset?: number;
  componentType: number;
  count: number;
  type: string;
  normalized?: boolean;
  sparse?: unknown;
};

type GltfBufferView = {
  buffer: number;
  byteOffset?: number;
  byteLength: number;
  byteStride?: number;
};

type GltfPrimitive = {
  attributes: Record<string, number>;
  indices?: number;
  mode?: number;
};

const GLTF_FLOAT = 5126;
const GLTF_UNSIGNED_SHORT = 5123;
const GLTF_UNSIGNED_INT = 5125;
const GLTF_TRIANGLES = 4;

function meshError(message: string): RangeError {
  return new RangeError(`invalid GLB mesh: ${message}`);
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function asArray(value: unknown): unknown[] | undefined {
  return Array.isArray(value) ? value : undefined;
}

function finiteInt(value: unknown, name: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw meshError(`${name} is not a non-negative integer`);
  }
  return value;
}

/** Extract bounded triangle meshes from one embedded GLB buffer. */
export function parseGlbMeshes(source: string, container: GlbContainer): MeshDescription[] {
  if (!container.bin) throw meshError("missing embedded BIN buffer");
  const bin = container.bin;
  const root = asRecord(container.json);
  if (!root) throw meshError("JSON root is not an object");
  const buffers = asArray(root.buffers);
  if (!buffers || buffers.length !== 1) throw meshError("requires exactly one buffer");
  const buffer = asRecord(buffers[0]);
  if (!buffer || buffer.uri !== undefined) throw meshError("buffer is external");
  const declared = finiteInt(buffer.byteLength, "buffer.byteLength");
  if (declared !== bin.byteLength) throw meshError("buffer.byteLength does not match BIN");
  const views = asArray(root.bufferViews) ?? [];
  const accessors = asArray(root.accessors) ?? [];
  const meshes = asArray(root.meshes) ?? [];
  const out: MeshDescription[] = [];
  for (let meshIndex = 0; meshIndex < meshes.length; meshIndex++) {
    const mesh = asRecord(meshes[meshIndex]);
    const primitives = mesh ? asArray(mesh.primitives) : undefined;
    if (!primitives) throw meshError(`mesh ${meshIndex} has no primitives`);
    for (let primitiveIndex = 0; primitiveIndex < primitives.length; primitiveIndex++) {
      const primitive = asRecord(primitives[primitiveIndex]);
      if (!primitive) throw meshError(`mesh ${meshIndex} primitive ${primitiveIndex} is not an object`);
      const mode = primitive.mode === undefined ? GLTF_TRIANGLES : finiteInt(primitive.mode, "mode");
      if (mode !== GLTF_TRIANGLES) throw meshError("only triangle primitives are supported");
      const attributes = asRecord(primitive.attributes);
      const positionAccessor = attributes ? attributes.POSITION : undefined;
      if (typeof positionAccessor !== "number") throw meshError("missing POSITION attribute");
      const position = readAccessor(accessors, views, bin, positionAccessor, "POSITION");
      if (position.componentType !== GLTF_FLOAT || position.type !== "VEC3") {
        throw meshError("POSITION must be FLOAT VEC3");
      }
      if (position.normalized || position.sparse !== undefined) throw meshError("POSITION is normalized or sparse");
      const vertexCount = position.count;
      if (vertexCount === 0) throw meshError("POSITION is empty");
      if (vertexCount > MAX_MESH_VERTICES) throw meshError(`more than ${MAX_MESH_VERTICES} vertices`);
      let indices: Uint32Array;
      let indexCount: number;
      if (primitive.indices === undefined) {
        indexCount = vertexCount;
        if (indexCount > MAX_MESH_INDICES) throw meshError(`more than ${MAX_MESH_INDICES} indices`);
        indices = new Uint32Array(indexCount);
        for (let i = 0; i < indexCount; i++) indices[i] = i;
      } else {
        if (typeof primitive.indices !== "number") throw meshError("indices is not an accessor index");
        const index = readAccessor(accessors, views, bin, primitive.indices, "indices");
        if (index.type !== "SCALAR" || (index.componentType !== GLTF_UNSIGNED_SHORT && index.componentType !== GLTF_UNSIGNED_INT)) {
          throw meshError("indices must be UNSIGNED_SHORT or UNSIGNED_INT SCALAR");
        }
        if (index.normalized || index.sparse !== undefined) throw meshError("indices are normalized or sparse");
        indexCount = index.count;
        if (indexCount === 0 || indexCount % 3 !== 0) throw meshError("index count is not a positive multiple of 3");
        if (indexCount > MAX_MESH_INDICES) throw meshError(`more than ${MAX_MESH_INDICES} indices`);
        indices = new Uint32Array(indexCount);
        const width = index.componentType === GLTF_UNSIGNED_SHORT ? 2 : 4;
        const stride = index.byteStride ?? width;
        if (stride < width) throw meshError("index byteStride is too small");
        const start = index.byteOffset;
        const end = start + (indexCount - 1) * stride + width;
        if (end > index.view.byteLength) throw meshError("indices exceed bufferView bounds");
        for (let i = 0; i < indexCount; i++) {
          const at = start + i * stride;
          const value = width === 2 ? bin[index.view.byteOffset + at] | (bin[index.view.byteOffset + at + 1] << 8) : readU32(bin, index.view.byteOffset + at);
          if (value >= vertexCount) throw meshError("index exceeds vertex count");
          indices[i] = value;
        }
      }
      const positions = new Float32Array(vertexCount * 3);
      const stride = position.byteStride ?? 12;
      if (stride < 12) throw meshError("POSITION byteStride is too small");
      const start = position.byteOffset;
      const end = start + (vertexCount - 1) * stride + 12;
      if (end > position.view.byteLength) throw meshError("POSITION exceeds bufferView bounds");
      const min: [number, number, number] = [Infinity, Infinity, Infinity];
      const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
      for (let i = 0; i < vertexCount; i++) {
        const at = position.view.byteOffset + start + i * stride;
        for (let axis = 0; axis < 3; axis++) {
          const value = readFloat32(bin, at + axis * 4);
          if (!Number.isFinite(value)) throw meshError("POSITION contains a non-finite value");
          positions[i * 3 + axis] = value;
          if (value < min[axis]) min[axis] = value;
          if (value > max[axis]) max[axis] = value;
        }
      }
      out.push({
        id: `${source}#mesh-${meshIndex}-primitive-${primitiveIndex}`,
        source,
        positions,
        indices,
        vertexCount,
        indexCount,
        bounds: { min, max },
      });
    }
  }
  return out;
}

function readAccessor(
  accessors: unknown[],
  views: unknown[],
  bin: Uint8Array,
  index: number,
  name: string,
): { componentType: number; type: string; count: number; byteOffset: number; byteStride?: number; normalized?: boolean; sparse?: unknown; view: { byteOffset: number; byteLength: number } } {
  const accessor = asRecord(accessors[index]);
  if (!accessor) throw meshError(`${name} accessor ${index} is missing`);
  const componentType = finiteInt(accessor.componentType, `${name}.componentType`);
  const count = finiteInt(accessor.count, `${name}.count`);
  const type = accessor.type;
  if (typeof type !== "string") throw meshError(`${name}.type is missing`);
  const byteOffset = accessor.byteOffset === undefined ? 0 : finiteInt(accessor.byteOffset, `${name}.byteOffset`);
  if (accessor.bufferView === undefined) throw meshError(`${name} has no bufferView`);
  const viewIndex = finiteInt(accessor.bufferView, `${name}.bufferView`);
  const view = asRecord(views[viewIndex]);
  if (!view) throw meshError(`${name} bufferView ${viewIndex} is missing`);
  if (finiteInt(view.buffer, `${name}.buffer`) !== 0) throw meshError(`${name} bufferView is not buffer 0`);
  const viewOffset = view.byteOffset === undefined ? 0 : finiteInt(view.byteOffset, `${name}.bufferView.byteOffset`);
  const viewLength = finiteInt(view.byteLength, `${name}.bufferView.byteLength`);
  if (viewOffset + viewLength > bin.byteLength) throw meshError(`${name} bufferView exceeds BIN`);
  const byteStride = view.byteStride === undefined ? undefined : finiteInt(view.byteStride, `${name}.byteStride`);
  return {
    componentType,
    type,
    count,
    byteOffset,
    byteStride,
    normalized: accessor.normalized === true,
    sparse: accessor.sparse,
    view: { byteOffset: viewOffset, byteLength: viewLength },
  };
}

function readFloat32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getFloat32(0, true);
}

export function stableId(path: string): string {
  return path.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase();
}

export async function describeSource(
  id: string,
  kind: SourceKind,
  path: string,
  bytes: Uint8Array,
): Promise<SourceRecord> {
  if (bytes.byteLength > MAX_ASSET_BYTES) {
    throw new RangeError(`${path}: source exceeds ${MAX_ASSET_BYTES} bytes`);
  }
  const hash = await sourceBytesHash(bytes);
  return { id, kind, path, bytes: bytes.byteLength, hash };
}

export function validateDescription(value: AssetDescription): AssetDiagnostic[] {
  const out: AssetDiagnostic[] = [];
  const seen = new Set<string>();
  for (const source of value.sources) {
    if (seen.has(source.id)) {
      out.push({ code: "duplicate-source", severity: "error", source: source.path, message: `duplicate id ${source.id}` });
    }
    seen.add(source.id);
    if (!/^[a-f0-9]{64}$/.test(source.hash)) {
      out.push({ code: "bad-hash", severity: "error", source: source.path, message: "source hash is not SHA-256" });
    }
  }
  for (const mesh of value.meshes) {
    if (mesh.vertexCount > MAX_MESH_VERTICES || mesh.indexCount > MAX_MESH_INDICES) {
      out.push({ code: "mesh-limit", severity: "error", source: mesh.source, message: "mesh exceeds bounded vertex/index limits" });
    }
    if (mesh.positions.length !== mesh.vertexCount * 3 || mesh.indices.length !== mesh.indexCount) {
      out.push({ code: "mesh-shape", severity: "error", source: mesh.source, message: "mesh buffers do not match counts" });
    }
  }
  if (value.images.length > MAX_IMAGES) {
    out.push({ code: "image-count", severity: "error", source: value.id, message: `more than ${MAX_IMAGES} images` });
  }
  for (const image of value.images) {
    if (image.bytes.byteLength > MAX_IMAGE_BYTES) {
      out.push({ code: "image-limit", severity: "error", source: image.source, message: "image exceeds byte limit" });
    }
  }
  return out;
}

/** Compile one GLB source into a bounded asset description. */
export async function compileAsset(
  source: SourceRecord,
  bytes: Uint8Array,
  files: readonly { record: SourceRecord; bytes: Uint8Array }[],
): Promise<AssetDescription> {
  const diagnostics: AssetDiagnostic[] = [];
  const sources: SourceRecord[] = [source];
  const meshes: MeshDescription[] = [];
  const images: ImageDescription[] = [];
  if (source.kind !== "glb") {
    diagnostics.push({ code: "unsupported-kind", severity: "error", source: source.id, message: `unsupported source kind ${source.kind}` });
    return { id: source.id, sources, meshes, images, diagnostics };
  }
  if (bytes.byteLength > MAX_ASSET_BYTES) {
    diagnostics.push({ code: "asset-limit", severity: "error", source: source.id, message: "source exceeds byte limit" });
    return { id: source.id, sources, meshes, images, diagnostics };
  }
  if (bytes.byteLength !== source.bytes) {
    diagnostics.push({ code: "byte-mismatch", severity: "error", source: source.id, message: "source bytes do not match record" });
    return { id: source.id, sources, meshes, images, diagnostics };
  }
  let container: GlbContainer;
  try {
    container = parseGlb(bytes);
  } catch (error) {
    diagnostics.push({ code: "glb-invalid", severity: "error", source: source.id, message: error instanceof Error ? error.message : String(error) });
    return { id: source.id, sources, meshes, images, diagnostics };
  }
  let parsed: MeshDescription[];
  try {
    parsed = parseGlbMeshes(source.id, container);
  } catch (error) {
    diagnostics.push({ code: "mesh-invalid", severity: "error", source: source.id, message: error instanceof Error ? error.message : String(error) });
    return { id: source.id, sources, meshes, images, diagnostics };
  }
  for (const mesh of parsed) {
    if (mesh.vertexCount > MAX_MESH_VERTICES) {
      diagnostics.push({ code: "vertex-limit", severity: "error", source: source.id, message: `mesh ${mesh.id} exceeds vertex limit` });
      continue;
    }
    if (mesh.indexCount > MAX_MESH_INDICES) {
      diagnostics.push({ code: "index-limit", severity: "error", source: source.id, message: `mesh ${mesh.id} exceeds index limit` });
      continue;
    }
    meshes.push({ ...mesh, source: source.id });
  }
  return { id: source.id, sources, meshes, images, diagnostics };
}

/** Bounded content-hash cache with explicit per-key invalidation. */
export class AssetCache<T> {
  readonly capacity: number;
  private readonly entries = new Map<string, { hash: string; value: T; deps: string[] }>();

  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`AssetCache capacity must be a positive integer, got ${capacity}`);
    }
    this.capacity = capacity;
  }

  get size(): number {
    return this.entries.size;
  }

  get(key: string, hash: string): T | undefined {
    const entry = this.entries.get(key);
    if (!entry || entry.hash !== hash) return undefined;
    this.entries.delete(key);
    this.entries.set(key, entry);
    return entry.value;
  }

  set(key: string, hash: string, value: T, deps: string[] = []): void {
    this.entries.delete(key);
    this.entries.set(key, { hash, value, deps: [...new Set(deps)] });
    while (this.entries.size > this.capacity) {
      const oldest = this.entries.keys().next().value;
      if (oldest === undefined) break;
      this.entries.delete(oldest);
    }
  }

  invalidate(key: string): boolean {
    const doomed = new Set<string>([key]);
    let grew = true;
    while (grew) {
      grew = false;
      for (const [id, entry] of this.entries) {
        if (doomed.has(id)) continue;
        if (entry.deps.some((dep) => doomed.has(dep))) {
          doomed.add(id);
          grew = true;
        }
      }
    }
    let removed = false;
    for (const id of doomed) {
      if (this.entries.delete(id)) removed = true;
    }
    return removed;
  }

  clear(): void {
    this.entries.clear();
  }
}
