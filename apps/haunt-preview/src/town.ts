import {
  HouseFloor,
  readLayout,
  extendLayout,
  type Layout,
  type Room,
  type Wall,
} from "./layout.ts";
import type { SceneAsset, Operation } from "./project.ts";
import type { Pose } from "./scene.ts";

export type Building = { floors: Layout[] };
export type Town = { buildings: Record<string, Building> };
export type TownWall = Wall & {
  building: string;
  level: number;
  y: number;
  height: number;
  local: Wall;
  scale: number;
  door?: boolean;
};
export type KitInstance = {
  mesh: string;
  x: number;
  y: number;
  z: number;
  yaw: number;
  sx: number;
  sy: number;
  sz: number;
};
export const WORLD_EDGE = 48;
export const isHouse = (kind: string) =>
  kind === "cottage" || kind === "townhouse";
const clone = <T>(x: T): T => structuredClone(x);
const localRooms = (id: string, level: number): Layout => ({
  rooms: [
    {
      id: `${id}-f${level}-front`,
      name: "Front room",
      x: 0,
      z: 2.5,
      width: 8,
      depth: 5,
    },
    {
      id: `${id}-f${level}-back`,
      name: "Back room",
      x: 0,
      z: -2.5,
      width: 8,
      depth: 5,
    },
  ],
  removedWalls: [],
});
export function newBuilding(entity: SceneAsset): Building {
  return {
    floors: Array.from(
      { length: entity.kind === "townhouse" ? 2 : 1 },
      (_, level) => localRooms(entity.id, level),
    ),
  };
}
export function initialTown(): { entities: SceneAsset[]; town: Town } {
  const entities: SceneAsset[] = [
    { id: "cottage-starter", kind: "cottage", x: -9, z: -10, scale: 1 },
    { id: "townhouse-starter", kind: "townhouse", x: 10, z: -10, scale: 1 },
    { id: "barrel-starter", kind: "barrel", x: -15, z: -3, scale: 1 },
    { id: "bench-starter", kind: "bench", x: -3, z: 3, scale: 1 },
    { id: "fountain-starter", kind: "fountain", x: 8, z: 9, scale: 1 },
  ];
  return {
    entities,
    town: {
      buildings: Object.fromEntries(
        entities
          .filter((e) => isHouse(e.kind))
          .map((e) => [e.id, newBuilding(e)]),
      ),
    },
  };
}
export function readTown(value: unknown, entities: SceneAsset[]): Town {
  if (!entities.every((e) => e && typeof e.kind === "string"))
    throw new Error("Choose supported town assets.");
  const t = value as Town;
  const houses = entities.filter((e) => isHouse(e.kind));
  if (
    !t ||
    !t.buildings ||
    typeof t.buildings !== "object" ||
    Array.isArray(t.buildings) ||
    houses.length > 12 ||
    Object.keys(t.buildings).length !== houses.length
  )
    throw new Error(
      "Keep the town to twelve houses, each with its saved rooms.",
    );
  const result: Town = { buildings: {} };
  const globalIds = new Set<string>();
  const footprints: {
    id: string;
    left: number;
    right: number;
    top: number;
    bottom: number;
    pad: number;
  }[] = [];
  for (const e of houses) {
    if ((e.y !== undefined && e.y !== 0) || e.scale < 0.6 || e.scale > 2.5)
      throw new Error(
        "Keep houses between size 0.6 and 2.5 so their doors stay usable.",
      );
    const b = t.buildings[e.id];
    if (
      !b ||
      !Array.isArray(b.floors) ||
      b.floors.length !== (e.kind === "townhouse" ? 2 : 1)
    )
      throw new Error("Keep the saved floors with their house.");
    const floors = b.floors.map(readLayout);
    const ids = new Set<string>();
    for (const floor of floors)
      for (const room of floor.rooms) {
        if (ids.has(room.id) || globalIds.has(room.id) || room.outdoor)
          throw new Error("Give each indoor room its own name.");
        ids.add(room.id);
        globalIds.add(room.id);
        const p = {
          id: e.id,
          pad: 1.6 * e.scale,
          left: e.x + (room.x - room.width / 2) * e.scale,
          right: e.x + (room.x + room.width / 2) * e.scale,
          top: e.z + (room.z - room.depth / 2) * e.scale,
          bottom: e.z + (room.z + room.depth / 2) * e.scale,
        };
        if (
          Math.max(
            Math.abs(p.left),
            Math.abs(p.right),
            Math.abs(p.top),
            Math.abs(p.bottom),
          ) >
          WORLD_EDGE - 1
        )
          throw new Error("Keep the building inside the town boundary.");
        if (
          footprints.some(
            (q) =>
              q.id !== e.id &&
              Math.min(p.right + p.pad, q.right + q.pad) -
                Math.max(p.left - p.pad, q.left - q.pad) >
                0.01 &&
              Math.min(p.bottom + p.pad, q.bottom + q.pad) -
                Math.max(p.top - p.pad, q.top - q.pad) >
                0.01,
          )
        )
          throw new Error(
            "That building would overlap another house. Choose a clearer spot.",
          );
        footprints.push(p);
      }
    result.buildings[e.id] = { floors };
  }
  return result;
}
export function editTown(
  town: Town,
  entities: SceneAsset[],
  op: Operation,
): Town {
  const next = clone(town);
  if (op.kind === "add_asset" && isHouse(op.entity.kind))
    next.buildings[op.entity.id] = newBuilding(op.entity);
  if (op.kind === "remove_asset") delete next.buildings[op.id];
  if (
    op.kind === "add_room" ||
    op.kind === "add_wing" ||
    op.kind === "remove_wall"
  ) {
    const floor = new TownFloor(entities, next);
    const wall = floor.surfaces.find((w) => w.id === op.wall);
    if (!wall) throw new Error("Look at a house wall to change its rooms.");
    const building = next.buildings[wall.building];
    const plan = building.floors[wall.level];
    if (op.kind === "remove_wall") {
      if (!floor.walls.some((w) => w.id === op.wall))
        throw new Error("That wall is already open.");
      plan.removedWalls.push(op.wall);
    } else {
      const entity = entities.find((e) => e.id === wall.building)!;
      building.floors[wall.level] = extendLayout(
        plan,
        wall.id,
        op.size,
        op.kind === "add_wing",
        op.at
          ? {
              x: (op.at.x - entity.x) / entity.scale,
              z: (op.at.z - entity.z) / entity.scale,
            }
          : undefined,
      );
    }
  }
  return readTown(next, entities);
}
const b = (r: Room) => ({
  l: r.x - r.width / 2,
  r: r.x + r.width / 2,
  t: r.z - r.depth / 2,
  b: r.z + r.depth / 2,
});
const segmentDistance = (x: number, z: number, w: Wall) =>
  Math.hypot(
    x - Math.max(Math.min(w.x1, w.x2), Math.min(Math.max(w.x1, w.x2), x)),
    z - Math.max(Math.min(w.z1, w.z2), Math.min(Math.max(w.z1, w.z2), z)),
  );
function split(w: Wall, lo: number, hi: number): Wall[] {
  const horizontal = w.z1 === w.z2;
  const from = horizontal ? w.x1 : w.z1,
    to = horizontal ? w.x2 : w.z2;
  if (hi <= from || lo >= to) return [w];
  const pieces: Wall[] = [];
  if (lo > from)
    pieces.push({ ...w, ...(horizontal ? { x2: lo } : { z2: lo }) });
  if (hi < to) pieces.push({ ...w, ...(horizontal ? { x1: hi } : { z1: hi }) });
  return pieces;
}
/** The same wall segments feed collision, aiming, and kit-piece rendering. */
export class TownFloor {
  walls: TownWall[] = [];
  surfaces: TownWall[] = [];
  pieces: KitInstance[] = [];
  levels: { entity: SceneAsset; level: number; plan: Layout }[] = [];
  entities: SceneAsset[];
  town: Town;
  constructor(entities: SceneAsset[], town: Town) {
    this.entities = entities;
    this.town = town;
    for (const entity of entities.filter((e) => isHouse(e.kind))) {
      const building = town.buildings[entity.id];
      if (!building) continue;
      for (const [level, plan] of building.floors.entries()) {
        this.levels.push({ entity, level, plan });
        const floor = new HouseFloor(plan);
        for (const original of floor.surfaces) {
          const door =
            level === 0 &&
            original.exterior &&
            original.side === "south" &&
            Math.abs(original.z1 - 5) < 0.001 &&
            original.roomId.endsWith("-front");
          let parts = [original];
          if (door) parts = split(original, -2.87, -1.13);
          // Leave a stair opening through the center partition on both storeys.
          if (
            entity.kind === "townhouse" &&
            Math.abs(original.z1) < 0.001 &&
            Math.abs(original.z2) < 0.001
          )
            parts = parts.flatMap((w) => split(w, 2.3, 3.75));
          for (const w of parts) {
            const wall: TownWall = {
              ...w,
              local: w,
              building: entity.id,
              level,
              y: level * 4 * entity.scale,
              height: 4 * entity.scale,
              scale: entity.scale,
              door,
              name: `${entity.kind === "cottage" ? "Cottage" : "Townhouse"} ${entities.filter((e) => isHouse(e.kind)).indexOf(entity) + 1} · ${level ? "upstairs · " : ""}${w.name}`,
              x1: entity.x + w.x1 * entity.scale,
              x2: entity.x + w.x2 * entity.scale,
              z1: entity.z + w.z1 * entity.scale,
              z2: entity.z + w.z2 * entity.scale,
            };
            this.surfaces.push(wall);
            if (!w.removed) this.walls.push(wall);
            if (!w.removed && !door) this.pieces.push(this.wallPiece(wall));
          }
          if (door && !original.removed) {
            this.pieces.push({
              mesh: "piece/door-4",
              x: entity.x - 4 * entity.scale,
              y: level * 4 * entity.scale,
              z: entity.z + 5 * entity.scale,
              yaw: 0,
              sx: entity.scale,
              sy: entity.scale,
              sz: entity.scale,
            });
            this.pieces.push({
              mesh: "piece/window-4",
              x: entity.x,
              y: level * 4 * entity.scale,
              z: entity.z + 5 * entity.scale,
              yaw: 0,
              sx: entity.scale,
              sy: entity.scale,
              sz: entity.scale,
            });
          }
        }
      }
    }
  }
  get layout(): Layout {
    return {
      rooms: this.levels
        .filter((i) => i.level === 0)
        .flatMap(({ entity: e, plan }) =>
          plan.rooms.map((r) => ({
            ...r,
            x: e.x + r.x * e.scale,
            z: e.z + r.z * e.scale,
            width: r.width * e.scale,
            depth: r.depth * e.scale,
          })),
        ),
      removedWalls: [],
    };
  }
  wallPiece(w: TownWall): KitInstance {
    const length = Math.hypot(w.x2 - w.x1, w.z2 - w.z1);
    const x = w.side === "north" ? w.x2 : w.x1;
    const z = w.side === "east" ? w.z2 : w.z1;
    const yaw =
      w.side === "north"
        ? Math.PI
        : w.side === "east"
          ? Math.PI / 2
          : w.side === "west"
            ? -Math.PI / 2
            : 0;
    return {
      mesh: w.exterior ? "piece/window-4" : "piece/wall-4-timber",
      x,
      y: w.y,
      z,
      yaw,
      sx: length / 4,
      sy: w.scale,
      sz: w.scale,
    };
  }
  roomAt(x: number, z: number, y = 0) {
    for (const item of this.levels) {
      const e = item.entity;
      if (Math.abs(y - item.level * 4 * e.scale) > 0.4) continue;
      const px = (x - e.x) / e.scale,
        pz = (z - e.z) / e.scale;
      const room = item.plan.rooms.find((r) => {
        const q = b(r);
        return px >= q.l && px <= q.r && pz >= q.t && pz <= q.b;
      });
      if (room)
        return {
          ...item,
          room,
          buildingName: `${e.kind === "cottage" ? "Cottage" : "Townhouse"} ${this.entities.filter((e) => isHouse(e.kind)).indexOf(e) + 1}`,
        };
    }
  }
  stairAt(x: number, z: number) {
    for (const e of this.entities.filter((e) => e.kind === "townhouse")) {
      const px = (x - e.x) / e.scale,
        pz = (z - e.z) / e.scale;
      if (px >= 2.3 && px <= 3.75 && pz >= -4.05 && pz <= 3.05)
        return {
          entity: e,
          y: Math.max(0, Math.min(4, ((3 - pz) / 7) * 4)) * e.scale,
        };
    }
  }
  heightAt(x: number, z: number, fromY = 0): number | undefined {
    if (Math.abs(x) > WORLD_EDGE - 0.4 || Math.abs(z) > WORLD_EDGE - 0.4)
      return;
    const stair = this.stairAt(x, z);
    if (stair && Math.abs(stair.y - fromY) < 0.4) return stair.y;
    if (stair && fromY > 0.4 && stair.y < 3.8 * stair.entity.scale) return;
    if (this.roomAt(x, z, fromY)) {
      const room = this.roomAt(x, z, fromY)!;
      return room.level * 4 * room.entity.scale;
    }
    if (fromY > 0.4) return;
    return 0;
  }
  canStand = (x: number, z: number, radius = 0.24, y = 0) => {
    if (!Number.isFinite(x) || !Number.isFinite(z) || !Number.isFinite(y))
      return false;
    const h = this.heightAt(x, z, y);
    if (h === undefined || Math.abs(h - y) > 0.4) return false;
    return !this.walls.some(
      (w) =>
        y + 1.55 > w.y + 0.1 &&
        y < w.y + w.height - 0.1 &&
        segmentDistance(x, z, w) < radius + 0.14 * w.scale,
    );
  };
  move(pose: Pose, forward: number, right: number, seconds: number) {
    const step =
      (Math.min(0.05, Math.max(0, seconds)) * 2.6) /
      Math.max(1, Math.hypot(forward, right));
    const dx =
      (Math.sin(pose.yaw) * forward + Math.cos(pose.yaw) * right) * step;
    const dz =
      (-Math.cos(pose.yaw) * forward + Math.sin(pose.yaw) * right) * step;
    for (const axis of ["x", "z"] as const) {
      const x = pose.x + (axis === "x" ? dx : 0),
        z = pose.z + (axis === "z" ? dz : 0);
      const h = this.heightAt(x, z, pose.y ?? 0);
      if (h !== undefined && this.canStand(x, z, 0.24, h)) {
        pose.x = x;
        pose.z = z;
        pose.y = h;
      }
    }
  }
  pick(pose: Pose, turn = 0) {
    const dx = Math.sin(pose.yaw + turn),
      dz = -Math.cos(pose.yaw + turn);
    let hit:
      (TownWall & { distance: number; hitX: number; hitZ: number }) | undefined;
    for (const w of this.surfaces) {
      if (w.removed && !w.exterior) continue;
      const horizontal = w.z1 === w.z2;
      const d = horizontal ? (w.z1 - pose.z) / dz : (w.x1 - pose.x) / dx;
      if (
        !Number.isFinite(d) ||
        d < 0.05 ||
        d > 80 ||
        (hit && hit.distance <= d)
      )
        continue;
      const x = pose.x + dx * d,
        z = pose.z + dz * d,
        y = (pose.y ?? 0) + 1.55 + Math.tan(pose.pitch) * d;
      if (
        y < w.y ||
        y > w.y + w.height ||
        x < Math.min(w.x1, w.x2) - 0.001 ||
        x > Math.max(w.x1, w.x2) + 0.001 ||
        z < Math.min(w.z1, w.z2) - 0.001 ||
        z > Math.max(w.z1, w.z2) + 0.001
      )
        continue;
      hit = { ...w, distance: d, hitX: x, hitZ: z };
    }
    return hit;
  }
  guide = (_from: { x: number; z: number }, target: { x: number; z: number }) =>
    target;
}

/** Find clear outdoor plots for relative house requests; explicit single coordinates stay exact. */
export function housePositions(
  entities: SceneAsset[],
  town: Town,
  kind: "cottage" | "townhouse" | readonly ("cottage" | "townhouse")[],
  count: number,
  scale: number,
  pose: Pose,
  placement: string,
  anchor: { x: number; z: number },
) {
  const placed: { x: number; z: number; kind: "cottage" | "townhouse" }[] = [];
  const items = clone(entities),
    next = clone(town);
  const turn =
    placement === "left"
      ? -Math.PI / 2
      : placement === "right"
        ? Math.PI / 2
        : placement === "behind"
          ? Math.PI
          : 0;
  const yaw = pose.yaw + turn;
  const candidates: { x: number; z: number }[] = [];
  if (placement === "coordinates") candidates.push(anchor);
  for (const distance of [14, 24, 34, 44])
    for (const side of [0, -14, 14, -28, 28]) {
      candidates.push(
        placement === "coordinates"
          ? { x: anchor.x + side, z: anchor.z + distance - 14 }
          : {
              x: pose.x + Math.sin(yaw) * distance + Math.cos(yaw) * side,
              z: pose.z - Math.cos(yaw) * distance + Math.sin(yaw) * side,
            },
      );
    }
  if (placement === "anywhere") {
    // Search the whole town after nearby plots, independent of the player's facing.
    for (let z = -36; z <= 36; z += 12)
      for (let x = -36; x <= 36; x += 14) candidates.push({ x, z });
  }
  for (const point of candidates) {
    const e: SceneAsset = {
      id: `pending-house-${placed.length}`,
      kind: typeof kind === "string" ? kind : kind[placed.length % kind.length],
      x: Math.round(point.x * 10) / 10,
      z: Math.round(point.z * 10) / 10,
      scale,
    };
    const proposal = {
      buildings: { ...next.buildings, [e.id]: newBuilding(e) },
    };
    try {
      const valid = readTown(proposal, [...items, e]);
      const floor = new TownFloor([...items, e], valid);
      if (
        items.some(
          (item) =>
            !isHouse(item.kind) &&
            !floor.canStand(item.x, item.z, 0.24, item.y ?? 0),
        )
      )
        continue;
      if (!floor.canStand(pose.x, pose.z, 0.35, pose.y ?? 0)) continue;
    } catch (error) {
      if (placement === "coordinates" && count === 1) throw error;
      continue;
    }
    placed.push({ x: e.x, z: e.z, kind: e.kind as "cottage" | "townhouse" });
    items.push(e);
    next.buildings[e.id] = proposal.buildings[e.id];
    if (placed.length === count) return placed;
  }
  throw new Error(
    "There is not enough clear ground for those houses here. Move to an open spot or choose smaller houses.",
  );
}
