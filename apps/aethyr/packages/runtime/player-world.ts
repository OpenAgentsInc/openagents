import {
  validateStarterGameConfig,
  type StarterGameConfig,
} from "./starter-game.ts";

export const PLAYER_WORLD_SCHEMA = "aethyrforge.player-world.v1" as const;
export const PLAYER_WORLD_LIMITS = Object.freeze({
  bytes: 4 * 1024 * 1024,
  coordinate: 100_000,
  entities: 16_384,
  colliders: 32_768,
  rooms: 128,
});
export type PlayerVector = [number, number, number];
export type PlayerMovementMode = "walk" | "fly";
export interface PlayerEntity {
  id: string;
  kind: string;
  position: PlayerVector;
  yaw: number;
  scale: number;
  collidable: boolean;
  halfExtents?: PlayerVector;
  collisionOffset?: PlayerVector;
}
export interface PlayerCollider {
  id: string;
  min: PlayerVector;
  max: PlayerVector;
}
export interface PlayerRoom {
  id: string;
  center: [number, number];
  size: [number, number];
  height: number;
  floorY: number;
}
/** Immutable authored settings. Live player and objective state belong to the session. */
export interface PlayerWorld {
  schema: typeof PLAYER_WORLD_SCHEMA;
  groundY: number;
  mode: PlayerMovementMode;
  spawn: {
    position: PlayerVector;
    yaw: number;
    pitch: number;
    mode: PlayerMovementMode;
  };
  entities: PlayerEntity[];
  colliders: PlayerCollider[];
  rooms: PlayerRoom[];
  game?: StarterGameConfig;
  presentationURL?: string;
}
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
function fields(
  value: unknown,
  allowed: readonly string[],
): asserts value is Record<string, unknown> {
  if (
    !record(value) ||
    Object.keys(value).some((key) => !allowed.includes(key))
  )
    throw new Error("Use supported published world settings.");
}
function number(
  value: unknown,
  limit: number = PLAYER_WORLD_LIMITS.coordinate,
): number {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    Math.abs(value) > limit
  )
    throw new Error(
      "Choose finite world positions and dimensions within the player limits.",
    );
  return value;
}
function vector(value: unknown, length: 2): [number, number];
function vector(value: unknown, length?: 3): PlayerVector;
function vector(value: unknown, length = 3): number[] {
  if (!Array.isArray(value) || value.length !== length)
    throw new Error("Choose complete world positions and dimensions.");
  return value.map((item) => number(item));
}
function id(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9_.:-]{1,96}$/.test(value))
    throw new Error("Choose a supported object, room, or collision name.");
  return value;
}
function mode(value: unknown): PlayerMovementMode {
  if (value !== "walk" && value !== "fly")
    throw new Error("Choose walking or flying movement.");
  return value;
}
function list<T>(
  value: unknown,
  max: number,
  parse: (value: unknown) => T & { id: string },
): T[] {
  if (!Array.isArray(value) || value.length > max)
    throw new Error(
      "The published world exceeds its object, room, or collision limit.",
    );
  const result = value.map(parse);
  if (new Set(result.map((item) => item.id)).size !== result.length)
    throw new Error(
      "Give each object, room, and collision box a distinct name.",
    );
  return result;
}
function entity(value: unknown): PlayerEntity {
  fields(value, [
    "id",
    "kind",
    "position",
    "yaw",
    "scale",
    "collidable",
    "halfExtents",
    "collisionOffset",
  ]);
  const scale = number(value.scale ?? 1, 20);
  if (
    scale < 0.05 ||
    (value.collidable !== undefined && typeof value.collidable !== "boolean")
  )
    throw new Error("Choose a supported object size and collision setting.");
  const halfExtents =
    value.halfExtents === undefined || value.halfExtents === null
      ? undefined
      : vector(value.halfExtents);
  const collisionOffset =
    value.collisionOffset === undefined || value.collisionOffset === null
      ? undefined
      : vector(value.collisionOffset);
  if (
    halfExtents?.some((item) => item <= 0 || item > 1000) ||
    collisionOffset?.some((item) => Math.abs(item) > 1000) ||
    (value.collidable && !halfExtents)
  )
    throw new Error("Solid objects need complete supported collision bounds.");
  return {
    id: id(value.id),
    kind: id(value.kind),
    position: vector(value.position),
    yaw: number(value.yaw ?? 0),
    scale,
    collidable: value.collidable ?? false,
    ...(halfExtents ? { halfExtents } : {}),
    ...(collisionOffset ? { collisionOffset } : {}),
  };
}

/** Validate a bounded copy before graphics or native simulation is created. */
export function validatePlayerWorld(value: unknown): PlayerWorld {
  fields(value, [
    "schema",
    "groundY",
    "mode",
    "spawn",
    "entities",
    "colliders",
    "rooms",
    "game",
    "presentationURL",
  ]);
  if (value.schema !== PLAYER_WORLD_SCHEMA)
    throw new Error("Open world settings prepared for this player release.");
  const groundY = number(value.groundY);
  const movement = mode(value.mode);
  fields(value.spawn, ["position", "yaw", "pitch", "mode"]);
  const spawn = {
    position: vector(value.spawn.position),
    yaw: number(value.spawn.yaw),
    pitch: number(value.spawn.pitch),
    mode: mode(value.spawn.mode),
  };
  if (
    spawn.mode !== movement ||
    (movement === "walk" && spawn.position[1] < groundY - 0.001)
  )
    throw new Error(
      "Choose an open spawn above the ground using the world's movement mode.",
    );
  const entities = list(value.entities, PLAYER_WORLD_LIMITS.entities, entity);
  const colliders = list(
    value.colliders,
    PLAYER_WORLD_LIMITS.colliders,
    (candidate): PlayerCollider => {
      fields(candidate, ["id", "min", "max"]);
      const min = vector(candidate.min),
        max = vector(candidate.max);
      if (min.some((item, axis) => item >= max[axis]))
        throw new Error("Choose increasing collision bounds.");
      return { id: id(candidate.id), min, max };
    },
  );
  const rooms = list(
    value.rooms,
    PLAYER_WORLD_LIMITS.rooms,
    (candidate): PlayerRoom => {
      fields(candidate, ["id", "center", "size", "height", "floorY"]);
      const size = vector(candidate.size, 2),
        height = number(candidate.height ?? 3),
        floorY = number(candidate.floorY ?? groundY);
      if (
        size.some((item) => item < 2 || item > 32) ||
        height < 2 ||
        height > 12 ||
        Math.abs(floorY - groundY) > 0.001
      )
        throw new Error(
          "Choose supported room dimensions at the world ground level.",
        );
      return {
        id: id(candidate.id),
        center: vector(candidate.center, 2),
        size,
        height,
        floorY,
      };
    },
  );
  const game =
    value.game === undefined
      ? undefined
      : validateStarterGameConfig(value.game);
  if (game)
    for (const target of [game.key, game.exit]) {
      const marker = entities.find((item) => item.id === target.id);
      if (
        !marker ||
        marker.position.some((item, axis) => item !== target.position[axis])
      )
        throw new Error(
          "Match the key and exit rules to their published marker objects.",
        );
    }
  if (
    value.presentationURL !== undefined &&
    (typeof value.presentationURL !== "string" ||
      value.presentationURL.length > 512 ||
      !/^\/(?:starter|private-assets)\/(?:[a-zA-Z0-9_-]+\/)*[a-zA-Z0-9_.-]+\.json$/.test(
        value.presentationURL,
      ))
  )
    throw new Error("Choose a prepared local overview presentation.");
  const world: PlayerWorld = {
    schema: PLAYER_WORLD_SCHEMA,
    groundY,
    mode: movement,
    spawn,
    entities,
    colliders,
    rooms,
    ...(game ? { game } : {}),
    ...(value.presentationURL === undefined
      ? {}
      : { presentationURL: value.presentationURL as string }),
  };
  if (
    new TextEncoder().encode(JSON.stringify(world)).byteLength >
    PLAYER_WORLD_LIMITS.bytes
  )
    throw new Error("The published world settings exceed their size limit.");
  return world;
}
