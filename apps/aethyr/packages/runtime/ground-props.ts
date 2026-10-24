import {
  readAsset,
  verifyAssetBytes,
  type AssetDownloadOptions,
} from "../assets/src/asset-download.ts";
import type { CatalogAsset, CompiledCatalog } from "../assets/src/scene.ts";
import type { V3 } from "../renderer/math.ts";

export const GROUND_PROPS_LIMITS = Object.freeze({
  bytes: 128 * 1024,
  catalogBytes: 256 * 1024,
  entities: 128,
  coordinate: 20_000,
});
export interface GroundPropEntity {
  id: string;
  kind: string;
  position: V3;
  yaw: number;
  scale: number;
  collidable: false;
  /** Unscaled mesh-local bounds; the runtime applies the entity transform. */
  halfExtents: V3;
  collisionOffset: V3;
}
export interface GroundPropsDocument {
  schema: "aethyrforge.ground-props.v1";
  entities: GroundPropEntity[];
  sourceKit?: string;
  license?: string;
  placements?: number;
}
export interface GroundDraft {
  entities: GroundPropEntity[];
  rooms: [];
  removedWalls: [];
}
const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);
const finite = (value: unknown, min: number, max: number): value is number =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  value >= min &&
  value <= max;
const fail = (): never => {
  throw new Error(
    "Choose up to 128 prepared ground decorations with valid positions and sizes.",
  );
};
function vector(value: unknown, min: number, max: number): V3 {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    ![0, 1, 2].every((axis) => finite(value[axis], min, max))
  )
    return fail();
  return [...value] as V3;
}
function placement(
  value: unknown,
): Omit<GroundPropEntity, "halfExtents" | "collisionOffset"> {
  if (
    !record(value) ||
    typeof value.id !== "string" ||
    !/^[A-Za-z0-9_.:-]{1,96}$/.test(value.id) ||
    typeof value.kind !== "string" ||
    !/^quaternius:fantasy-standard:[a-z0-9][a-z0-9_-]*$/.test(value.kind) ||
    value.kind.length > 128 ||
    value.collidable !== false ||
    !finite(value.yaw, -1e6, 1e6) ||
    !finite(value.scale, 0.25, 4)
  )
    return fail();
  return {
    id: value.id,
    kind: value.kind,
    position: vector(
      value.position,
      -GROUND_PROPS_LIMITS.coordinate,
      GROUND_PROPS_LIMITS.coordinate,
    ),
    yaw: value.yaw,
    scale: value.scale,
    collidable: false,
  };
}
function entities<T extends { id: string }>(
  value: unknown,
  parse: (item: unknown) => T,
): T[] {
  if (!Array.isArray(value) || value.length > GROUND_PROPS_LIMITS.entities)
    return fail();
  const result = Array.from(value, parse);
  if (new Set(result.map((item) => item.id)).size !== result.length)
    return fail();
  return result;
}
function decorationsOnly(value: Record<string, unknown>) {
  for (const field of [
    "rooms",
    "removedWalls",
    "removed_walls",
    "colliders",
    "structures",
  ])
    if (
      value[field] !== undefined &&
      (!Array.isArray(value[field]) || value[field].length !== 0)
    )
      return fail();
}
function metadata(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 1024 ||
    /[\u0000-\u001f\u007f]/.test(value)
  )
    return fail();
  return value;
}
/** The ground catalog contains mesh decorations only, without building operations. */
export function validateGroundCatalog(value: unknown): CompiledCatalog {
  if (!record(value) || value.schema !== "aethyrforge.private-catalog.v1")
    return fail();
  const source = metadata(value.source);
  if (
    source === undefined ||
    (value.limitations !== undefined &&
      (!Array.isArray(value.limitations) || value.limitations.length > 32))
  )
    return fail();
  const limitations = Array.from(value.limitations ?? [], (item: unknown) => {
    const text = metadata(item);
    if (text === undefined) return fail();
    return text;
  });
  const assets = entities(value.assets, (item): CatalogAsset => {
    if (
      !record(item) ||
      typeof item.id !== "string" ||
      !/^quaternius:fantasy-standard:[a-z0-9][a-z0-9_-]*$/.test(item.id) ||
      item.id.length > 128 ||
      item.kind !== "prop" ||
      item.plan !== undefined ||
      !Array.isArray(item.wallParts) ||
      item.wallParts.length !== 0 ||
      !Number.isInteger(item.mesh) ||
      !finite(item.mesh, 0, 4095) ||
      item.materialProfile !== "gltf-pbr" ||
      !Array.isArray(item.aliases) ||
      item.aliases.length > 32
    )
      return fail();
    const name = metadata(item.name);
    if (!name || name.length > 128 || !record(item.bounds)) return fail();
    const aliases = Array.from(item.aliases, (alias) => {
      const text = metadata(alias);
      if (!text || text.length > 64) return fail();
      return text;
    });
    const min = vector(
        item.bounds.min,
        -GROUND_PROPS_LIMITS.coordinate,
        GROUND_PROPS_LIMITS.coordinate,
      ),
      max = vector(
        item.bounds.max,
        -GROUND_PROPS_LIMITS.coordinate,
        GROUND_PROPS_LIMITS.coordinate,
      );
    if (max.some((n, i) => n < min[i] || n - min[i] > 200)) return fail();
    return {
      id: item.id,
      name,
      aliases,
      mesh: item.mesh,
      dimensions: vector(item.dimensions, 0, GROUND_PROPS_LIMITS.coordinate),
      bounds: { min, max },
      kind: "prop",
      materialProfile: "gltf-pbr",
      wallParts: [],
    };
  });
  return {
    schema: "aethyrforge.private-catalog.v1",
    assets,
    source,
    limitations,
  };
}
/** Immutable files provide placement bounds; catalog validation follows before editing. */
export function validateGroundProps(value: unknown): GroundPropsDocument {
  if (!record(value) || value.schema !== "aethyrforge.ground-props.v1")
    return fail();
  decorationsOnly(value);
  const result = entities(value.entities, (item) => {
    const entity = placement(item);
    if (!record(item)) return fail();
    const halfExtents = vector(item.halfExtents, 0, 100);
    if (halfExtents.some((n) => n <= 0)) return fail();
    return {
      ...entity,
      halfExtents,
      collisionOffset: vector(
        item.collisionOffset,
        -GROUND_PROPS_LIMITS.coordinate,
        GROUND_PROPS_LIMITS.coordinate,
      ),
    };
  });
  const sourceKit = metadata(value.sourceKit),
    license = metadata(value.license);
  if (
    value.placements !== undefined &&
    (!Number.isInteger(value.placements) || value.placements !== result.length)
  )
    return fail();
  return {
    schema: "aethyrforge.ground-props.v1",
    entities: result,
    ...(sourceKit === undefined ? {} : { sourceKit }),
    ...(license === undefined ? {} : { license }),
    ...(value.placements === undefined ? {} : { placements: result.length }),
  };
}

/** Saved drafts can move decorations, but cannot add structures or alter collision. */
export function validateGroundDraft(
  value: unknown,
  catalog: ReadonlyMap<string, CatalogAsset>,
): GroundDraft {
  if (!record(value)) return fail();
  decorationsOnly(value);
  const result = entities(value.entities, (item) => {
    const entity = placement(item),
      asset = catalog.get(entity.kind);
    if (
      !asset ||
      asset.id !== entity.kind ||
      asset.kind !== "prop" ||
      asset.plan !== undefined ||
      !Number.isInteger(asset.mesh) ||
      asset.mesh < 0 ||
      asset.mesh > 4095 ||
      asset.materialProfile !== "gltf-pbr" ||
      !Array.isArray(asset.wallParts) ||
      asset.wallParts.length !== 0
    )
      return fail();
    const min = vector(
        asset.bounds?.min,
        -GROUND_PROPS_LIMITS.coordinate,
        GROUND_PROPS_LIMITS.coordinate,
      ),
      max = vector(
        asset.bounds?.max,
        -GROUND_PROPS_LIMITS.coordinate,
        GROUND_PROPS_LIMITS.coordinate,
      );
    if (max.some((n, i) => n < min[i])) return fail();
    const halfExtents = max.map((n, i) =>
      Math.max(0.01, (n - min[i]) / 2),
    ) as V3;
    if (halfExtents.some((n) => n > 100)) return fail();
    const collisionOffset = max.map((n, i) => (n + min[i]) / 2) as V3;
    return { ...entity, halfExtents, collisionOffset };
  });
  return { entities: result, rooms: [], removedWalls: [] };
}

/** Verify the bounded immutable file before parsing any of its fields. */
async function loadGroundDocument(
  url: string,
  kind: "props" | "catalog",
  maxBytes: number,
  options: AssetDownloadOptions,
): Promise<unknown> {
  const identity =
    /^\/private-assets\/ground-(props|catalog)-([a-f0-9]{64})\.json$/.exec(url);
  if (!identity || identity[1] !== kind)
    throw new Error("Choose a prepared ground decoration file.");
  return readAsset(
    url,
    async (response) => {
      if (!response.ok || !response.body)
        throw new Error("The ground decorations could not load. Try again.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let length = 0;
      try {
        if (Number(response.headers.get("content-length")) > maxBytes)
          throw new Error("The ground decoration file is too large.");
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          length += value.byteLength;
          if (length > maxBytes)
            throw new Error("The ground decoration file is too large.");
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
      let value: unknown;
      try {
        value = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(bytes),
        );
      } catch {
        throw new Error("The ground decoration file could not open.");
      }
      return value;
    },
    options,
  );
}
export async function loadGroundProps(
  url: string,
  options: AssetDownloadOptions = {},
): Promise<GroundPropsDocument> {
  return validateGroundProps(
    await loadGroundDocument(url, "props", GROUND_PROPS_LIMITS.bytes, options),
  );
}
export async function loadGroundCatalog(
  url: string,
  options: AssetDownloadOptions = {},
): Promise<CompiledCatalog> {
  return validateGroundCatalog(
    await loadGroundDocument(
      url,
      "catalog",
      GROUND_PROPS_LIMITS.catalogBytes,
      options,
    ),
  );
}
