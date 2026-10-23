import type { SourceRecord } from "./asset-compiler.ts";
import { DEFAULT_LAYOUT, HouseFloor } from "./layout.ts";
import { readProject, type Project } from "./project.ts";
import { houseGeometry } from "./scene.ts";
import {
  MAX_OUTPUT_BYTES, MAX_SOURCE_BYTES, MAX_SOURCE_COUNT, assetPath, descriptionBytes,
  sameSource, sourceKey, validateImportedAsset,
} from "./asset-import-protocol.ts";
import type { AssetConverter, OwnedAssetDescription, OwnedMeshDescription, SourceBytes } from "./asset-import-protocol.ts";

export const OWNED_SOURCE_EXTENSION = "OPENAGENTS_haunt";
export const MAX_OWNED_RECIPE_BYTES = 80 * 1024;
export const MAX_PALETTE_REPLACEMENTS = 64;
type RGB = [number, number, number];
type Replacement = { from: RGB; to: RGB };
/** External palettes use SourceKind gltf as a local text dependency, not a standalone model. */
export type OwnedPalette = { replacements: Replacement[] } | { source: string };
export type OwnedSourceRecipe =
  | { version: 1; kind: "geometry"; project: Project; palette?: OwnedPalette }
  | { version: 1; kind: "card"; image: string; width: number; height: number };

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("The asset recipe is invalid.");
  return value as Record<string, unknown>;
}
function fields(value: Record<string, unknown>, allowed: string[]): void {
  if (Object.keys(value).some(key => !allowed.includes(key))) throw new Error("The asset recipe contains an unsupported field.");
}
function json(bytes: Uint8Array, limit: number): unknown {
  if (!(bytes instanceof Uint8Array) || !bytes.byteLength || bytes.byteLength > limit) throw new Error("The asset recipe is empty or too large.");
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
}
function dependency(root: SourceRecord, path: unknown, files: readonly SourceBytes[], kind: "gltf" | "image"): SourceBytes {
  if (typeof path !== "string" || !path || path.startsWith("/") || /[\\\0?#:]/.test(path)) {
    throw new Error("Choose a project-local asset file.");
  }
  const relative = assetPath(root.path.slice(0, root.path.lastIndexOf("/") + 1) + path);
  const matches = files.filter(file => file.record.path === relative);
  if (matches.length !== 1 || matches[0].record.kind !== kind) throw new Error(`${relative}: choose one saved ${kind === "image" ? "PNG or JPEG image" : "palette file"}.`);
  const input = matches[0];
  sourceKey(input.record);
  if (!(input.bytes instanceof Uint8Array) || input.bytes.byteLength !== input.record.bytes) throw new Error(`${relative}: source size does not match.`);
  return input;
}
function replacements(value: unknown): Replacement[] {
  if (!Array.isArray(value) || value.length > MAX_PALETTE_REPLACEMENTS) throw new Error("Use up to 64 palette replacements.");
  const keys = new Set<string>();
  return value.map(entry => {
    const pair = object(entry); fields(pair, ["from", "to"]);
    const color = (value: unknown): RGB => {
      if (!Array.isArray(value) || value.length !== 3 || value.some(n => typeof n !== "number" || !Number.isFinite(n) || n < 0 || n > 1)) {
        throw new Error("Choose three finite palette color values between 0 and 1.");
      }
      return value.map(Math.fround) as RGB;
    };
    const from = color(pair.from), to = color(pair.to), key = from.join(",");
    if (keys.has(key)) throw new Error("Choose each palette color once.");
    keys.add(key);
    return { from, to };
  });
}
function palette(recipe: unknown, source: SourceRecord, files: readonly SourceBytes[]): { colors: Replacement[]; sources: SourceRecord[] } {
  if (recipe === undefined) return { colors: [], sources: [] };
  const value = object(recipe);
  if (Object.hasOwn(value, "source")) {
    fields(value, ["source"]);
    const input = dependency(source, value.source, files, "gltf");
    const saved = object(json(input.bytes, MAX_OWNED_RECIPE_BYTES));
    fields(saved, ["version", "replacements"]);
    if (saved.version !== 1) throw new Error("This palette has an unsupported version.");
    return { colors: replacements(saved.replacements), sources: [input.record] };
  }
  fields(value, ["replacements"]);
  return { colors: replacements(value.replacements), sources: [] };
}

function geometryBudget(project: Project): void {
  const layout = project.layout ?? DEFAULT_LAYOUT;
  // Each box emits 36 vertices. Include an extra loop iteration for rounding.
  let boxes = 35 + project.entities.length * 14;
  for (const room of layout.rooms) boxes += (Math.ceil(room.width / 0.7) + 1) * (Math.ceil(room.depth / 1.3) + 1) + 4;
  for (const wall of new HouseFloor(layout).walls) boxes += 5 + Math.ceil(Math.hypot(wall.x2 - wall.x1, wall.z2 - wall.z1) / 0.46);
  // XYZ position, normal, RGB, glow, and one Uint32 index per vertex.
  if (boxes * 36 * 44 > MAX_OUTPUT_BYTES) throw new Error("This house creates too much geometry. Remove a room or asset.");
}
function geometry(source: SourceRecord, value: unknown, colors: Replacement[]): OwnedMeshDescription[] {
  const raw = object(value);
  if (raw.id !== "haunt-webgpu-v1" || raw.material !== "haunt/material.wgsl") throw new Error("Choose a Haunted House project for this recipe.");
  const project = readProject(raw);
  geometryBudget(project);
  const vertices = houseGeometry(project.entities, undefined, project.layout);
  if (vertices.length % 30 !== 0 || vertices.byteLength / 40 * 44 > MAX_OUTPUT_BYTES) throw new Error("The house geometry is too large.");
  const table = new Map(colors.map(pair => [pair.from.join(","), pair.to]));
  const meshes: OwnedMeshDescription[] = [];
  const count = vertices.length / 10;
  for (let start = 0; start < count; start += 65_535) {
    const vertexCount = Math.min(65_535, count - start);
    const positions = new Float32Array(vertexCount * 3), normals = new Float32Array(vertexCount * 3);
    const color = new Float32Array(vertexCount * 3), glow = new Float32Array(vertexCount), indices = new Uint32Array(vertexCount);
    const min: RGB = [Infinity, Infinity, Infinity], max: RGB = [-Infinity, -Infinity, -Infinity];
    for (let at = 0; at < vertexCount; at++) {
      const offset = (start + at) * 10, rgb = vertices.subarray(offset + 6, offset + 9);
      positions.set(vertices.subarray(offset, offset + 3), at * 3);
      normals.set(vertices.subarray(offset + 3, offset + 6), at * 3);
      color.set(table.get([...rgb].join(",")) ?? rgb, at * 3);
      glow[at] = vertices[offset + 9]; indices[at] = at;
      for (let axis = 0; axis < 3; axis++) { min[axis] = Math.min(min[axis], positions[at * 3 + axis]); max[axis] = Math.max(max[axis], positions[at * 3 + axis]); }
    }
    meshes.push({ id: `${source.id}-geometry-${meshes.length}`, source: source.id, positions, normals, colors: color, glow,
      indices, vertexCount, indexCount: vertexCount, bounds: { min, max } });
  }
  return meshes;
}

/** Decode only the required OPENAGENTS_haunt extension; ordinary formats use the core converter. */
export async function compileOwnedSource(source: SourceRecord, bytes: Uint8Array, files: readonly SourceBytes[], convert: AssetConverter): Promise<OwnedAssetDescription | undefined> {
  if (source.kind !== "gltf") return undefined;
  sourceKey(source);
  if (!(bytes instanceof Uint8Array) || bytes.byteLength !== source.bytes || bytes.byteLength > MAX_SOURCE_BYTES) throw new Error("The asset source size does not match.");
  const document = object(json(bytes, MAX_SOURCE_BYTES));
  const extensions = document.extensions;
  if (!extensions || typeof extensions !== "object" || !Object.hasOwn(extensions, OWNED_SOURCE_EXTENSION)) return undefined;
  if (bytes.byteLength > MAX_OWNED_RECIPE_BYTES) throw new Error("The asset recipe is too large.");
  if (object(document.asset).version !== "2.0" || !Array.isArray(document.extensionsRequired) ||
    document.extensionsRequired.length !== 1 || document.extensionsRequired[0] !== OWNED_SOURCE_EXTENSION ||
    !Array.isArray(document.extensionsUsed) || !document.extensionsUsed.includes(OWNED_SOURCE_EXTENSION)) throw new Error("Use glTF 2 with the required Haunted House extension.");
  if (!Array.isArray(files) || files.length > MAX_SOURCE_COUNT) throw new Error("This recipe uses too many source files.");
  const recipe = object((extensions as Record<string, unknown>)[OWNED_SOURCE_EXTENSION]);
  if (recipe.version !== 1) throw new Error("This asset recipe has an unsupported version.");
  let description: OwnedAssetDescription;
  if (recipe.kind === "geometry") {
    fields(recipe, ["version", "kind", "project", "palette"]);
    const chosen = palette(recipe.palette, source, files);
    description = { id: source.id, sources: [source, ...chosen.sources], meshes: geometry(source, recipe.project, chosen.colors), images: [], diagnostics: [] };
  } else if (recipe.kind === "card") {
    fields(recipe, ["version", "kind", "image", "width", "height"]);
    const width = recipe.width, height = recipe.height;
    if (typeof width !== "number" || typeof height !== "number" || !Number.isFinite(width) || !Number.isFinite(height) ||
      width <= 0 || height <= 0 || width > 64 || height > 64 || Math.fround(width / 2) === 0 || Math.fround(height / 2) === 0) throw new Error("Choose card dimensions above 0 and up to 64.");
    const halfWidth = Math.fround(width / 2), halfHeight = Math.fround(height / 2);
    const input = dependency(source, recipe.image, files, "image");
    const image = await convert(input.record, input.bytes.slice(), []);
    validateImportedAsset(image, input.record, [input.record]);
    if (image.images.length !== 1 || image.meshes.length || image.sources.length !== 1 || !sameSource(image.sources[0], input.record)) throw new Error("Choose a PNG or JPEG image for this card.");
    const ids = new Set(image.images.map(image => image.id));
    const unique = (base: string) => { let id = base; while (ids.has(id)) id += "-1"; ids.add(id); return id; };
    const meshId = unique(`${source.id}-card`), material = unique(`${source.id}-card-material`);
    description = { id: source.id, sources: [source, input.record], images: image.images, diagnostics: image.diagnostics,
      meshes: [{ id: meshId, source: source.id, vertexCount: 4, indexCount: 6, positions: new Float32Array([-halfWidth, -halfHeight, 0, halfWidth, -halfHeight, 0, halfWidth, halfHeight, 0, -halfWidth, halfHeight, 0]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]), normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
        uvs: new Float32Array([0, 1, 1, 1, 1, 0, 0, 0]), material, bounds: { min: [-halfWidth, -halfHeight, 0], max: [halfWidth, halfHeight, 0] } }],
      materials: [{ id: material, baseColor: [1, 1, 1, 1], baseColorTexture: image.images[0].id, doubleSided: true, unlit: true }] };
  } else throw new Error("Choose a geometry or card asset recipe.");
  if (descriptionBytes(description) > MAX_OUTPUT_BYTES) throw new Error("The converted asset is too large.");
  validateImportedAsset(description, source, [source, ...files.map(file => file.record)]);
  return description;
}
