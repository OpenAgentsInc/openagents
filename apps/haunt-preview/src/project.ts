import { IS_TOWN, LAB } from "./lab.ts";
import {
  TownFloor,
  initialTown,
  readTown,
  editTown,
  housePositions,
  isHouse,
  type Town,
} from "./town.ts";
import kit from "./medieval-kit.json" with { type: "json" };
import {
  DEFAULT_LAYOUT,
  HouseFloor,
  readLayout,
  extendLayout,
  type Layout,
} from "./layout.ts";
import catalog from "./assets.json" with { type: "json" };
import type { Pose } from "./scene.ts";

export const PROJECT_ID = LAB.id;
export const SCENE_PATH = LAB.scene;
export const MAX_ASSETS = 64;
export const MAX_SCENE_BYTES = 64 * 1024;
export type AssetKind = keyof typeof catalog;
export const ASSETS = Object.fromEntries(
  Object.entries(catalog).filter(([, a]) =>
    IS_TOWN ? "source" in a : !("source" in a),
  ),
) as typeof catalog;
export const TINTS = {
  warm: [1, 1, 1],
  orange: [1, 0.55, 0.28],
  blue: [0.2, 0.55, 2.5],
  green: [0.25, 1.5, 0.55],
} as const;
export type Tint = keyof typeof TINTS;
export const BEHAVIORS = ["still", "float", "chase"] as const;
export type Behavior = (typeof BEHAVIORS)[number];
export type SceneAsset = {
  id: string;
  kind: AssetKind;
  x: number;
  z: number;
  scale: number;
  behavior?: Behavior;
  y?: number;
};
export type Project = {
  schema: 1;
  id: typeof PROJECT_ID;
  revision: number;
  material: typeof LAB.material;
  tint: Tint;
  entities: SceneAsset[];
  layout?: Layout;
  town?: Town;
};
export type Operation =
  | { kind: "add_asset" | "add_pumpkin"; entity: SceneAsset }
  | {
      kind: "move_asset" | "move_pumpkin";
      id: string;
      x: number;
      z: number;
      y?: number;
    }
  | { kind: "resize_asset" | "resize_pumpkin"; id: string; scale: number }
  | { kind: "remove_asset" | "remove_pumpkin"; id: string }
  | {
      kind: "set_behavior";
      ids: string[];
      behavior: Behavior;
      positions?: { id: string; x: number; z: number; y?: number }[];
    }
  | {
      kind: "add_room" | "add_wing";
      wall: string;
      size: number;
      at?: { x: number; z: number };
    }
  | { kind: "remove_wall"; wall: string }
  | { kind: "set_tint"; tint: Tint }
  | { kind: "undo" | "redo" }
  | { kind: "replace"; project: Project }
  | { kind: "batch"; operations: Operation[] };
export type Transaction = {
  id: string;
  project: typeof PROJECT_ID;
  baseRevision: number;
  operation: Operation;
};
const clone = <T>(x: T): T => structuredClone(x);

const defaultFloor = new HouseFloor();
export const walkable = (x: number, z: number) => defaultFloor.canStand(x, z);
export function initialProject(): Project {
  if (IS_TOWN)
    return {
      schema: 1,
      id: PROJECT_ID,
      revision: 0,
      material: LAB.material,
      tint: "warm",
      ...initialTown(),
    };
  return {
    schema: 1,
    id: PROJECT_ID,
    revision: 0,
    material: LAB.material,
    tint: "warm",
    entities: [
      [-2.5, 0, 1.3],
      [2.5, -4, 1],
      [-0.8, -8, 0.7],
      [2.7, -14, 1.8],
      [-2.8, -12, 0.9],
    ].map(([x, z, scale], i) => ({
      id: `pumpkin-${i + 1}`,
      kind: "pumpkin",
      x,
      z,
      scale,
    })),
  };
}
function validEntity(
  value: unknown,
  floor: {
    canStand: (x: number, z: number, radius?: number, y?: number) => boolean;
  } = defaultFloor,
): asserts value is SceneAsset {
  const e = value as SceneAsset;
  if (
    !e ||
    !Object.hasOwn(ASSETS, e.kind) ||
    typeof e.id !== "string" ||
    !/^[a-zA-Z0-9-]{1,64}$/.test(e.id) ||
    !Number.isFinite(e.x) ||
    !Number.isFinite(e.z) ||
    (!(IS_TOWN && isHouse(e.kind)) &&
      !floor.canStand(e.x, e.z, 0.24, e.y ?? 0)) ||
    (e.y !== undefined &&
      (!IS_TOWN || !Number.isFinite(e.y) || e.y < 0 || e.y > 24)) ||
    !Number.isFinite(e.scale) ||
    e.scale < 0.3 ||
    e.scale > 2.5 ||
    (e.behavior !== undefined &&
      (e.kind !== "ghost" || !BEHAVIORS.includes(e.behavior)))
  )
    throw new Error(
      "Place an asset inside the rooms, with size between 0.3 and 2.5.",
    );
}
/** Retire medieval additions from old Haunted House saves, retaining authored spooky rooms. */
export function cleanHaunt(value: unknown): unknown {
  if (IS_TOWN || !value || typeof value !== "object") return value;
  const p = structuredClone(value) as Project;
  if (Array.isArray(p.entities))
    p.entities = p.entities.filter((e) => !(e && e.kind in kit.meshes));
  if (p.layout && Array.isArray(p.layout.rooms)) {
    const ids = new Set(
      p.layout.rooms.filter((r) => !r.outdoor).map((r) => r.id),
    );
    p.layout.rooms = p.layout.rooms.filter((r) => !r.outdoor);
    if (Array.isArray(p.layout.removedWalls))
      p.layout.removedWalls = p.layout.removedWalls.filter((id) =>
        ids.has(id.split(":")[0]),
      );
  }
  return p;
}
export function projectFloor(project: Project) {
  return IS_TOWN && project.town
    ? new TownFloor(project.entities, project.town)
    : new HouseFloor(project.layout ?? DEFAULT_LAYOUT);
}
export function readProject(value: unknown): Project {
  const p = cleanHaunt(value) as Project;
  if (
    !p ||
    p.schema !== 1 ||
    p.id !== PROJECT_ID ||
    p.material !== LAB.material ||
    !Number.isSafeInteger(p.revision) ||
    p.revision < 0 ||
    !Object.hasOwn(TINTS, p.tint) ||
    !Array.isArray(p.entities) ||
    p.entities.length > MAX_ASSETS
  )
    throw new Error(
      "This scene file has an unsupported version or invalid contents.",
    );
  const layout = p.layout === undefined ? undefined : readLayout(p.layout);
  const town = IS_TOWN ? readTown(p.town, p.entities) : undefined;
  const floor = town
    ? new TownFloor(p.entities, town)
    : layout
      ? new HouseFloor(layout)
      : defaultFloor;
  const ids = new Set<string>();
  for (const e of p.entities) {
    validEntity(e, floor);
    if (ids.has(e.id)) throw new Error("Each asset needs a different name.");
    ids.add(e.id);
  }
  // Copy only project fields. GPU handles, player state, and model data never enter a save.
  const result: Project = {
    schema: 1,
    id: PROJECT_ID,
    revision: p.revision,
    material: LAB.material,
    tint: p.tint,
    ...(layout === undefined || IS_TOWN ? {} : { layout }),
    ...(town ? { town } : {}),
    entities: p.entities.map((e) => ({
      id: e.id,
      kind: e.kind,
      x: e.x,
      z: e.z,
      scale: e.scale,
      ...(e.behavior === undefined ? {} : { behavior: e.behavior }),
      ...(e.y === undefined ? {} : { y: e.y }),
    })),
  };
  if (new TextEncoder().encode(JSON.stringify(result)).length > MAX_SCENE_BYTES)
    throw new Error(
      "This scene is full. Remove a room or asset before adding more.",
    );
  return result;
}

/** One reversible command path for controls, decisions, and Kiln workspace edits. */
export class SceneHistory {
  project: Project;
  past: Project[] = [];
  future: Project[] = [];
  seen = new Set<string>();
  constructor(project = initialProject()) {
    this.project = readProject(project);
  }
  prepare(tx: Transaction): { project: Project; commit: () => void } {
    if (
      !tx ||
      tx.project !== PROJECT_ID ||
      typeof tx.id !== "string" ||
      !tx.id ||
      tx.id.length > 128 ||
      this.seen.has(tx.id)
    )
      throw new Error(
        "This scene change was already used or belongs to another project.",
      );
    if (tx.baseRevision !== this.project.revision)
      throw new Error(
        "The scene changed while this request was pending. Try it again.",
      );
    const before = clone(this.project);
    let next = clone(before);
    const op = tx.operation;
    if (!op || typeof op !== "object")
      throw new Error("Choose a supported scene change.");
    const find = (id: string) => {
      const e = next.entities.find((x) => x.id === id);
      if (!e) throw new Error("Select an existing asset first.");
      return e;
    };
    switch (op.kind) {
      case "add_asset":
      case "add_pumpkin":
        validEntity(op.entity, projectFloor(next));
        if (next.entities.some((e) => e.id === op.entity.id))
          throw new Error("That asset name is already in use.");
        next.entities.push(clone(op.entity));
        if (IS_TOWN && next.town)
          next.town = editTown(next.town, next.entities, op);
        break;
      case "move_asset":
      case "move_pumpkin":
        Object.assign(find(op.id), {
          x: op.x,
          z: op.z,
          ...(IS_TOWN && "y" in op ? { y: op.y } : {}),
        });
        break;
      case "resize_asset":
      case "resize_pumpkin":
        find(op.id).scale = op.scale;
        break;
      case "remove_asset":
      case "remove_pumpkin":
        find(op.id);
        next.entities = next.entities.filter((e) => e.id !== op.id);
        if (IS_TOWN && next.town)
          next.town = editTown(next.town, next.entities, op);
        break;
      case "add_room":
      case "add_wing":
        if (IS_TOWN && next.town) {
          next.town = editTown(next.town, next.entities, op);
          break;
        }
        next.layout = extendLayout(
          next.layout ?? DEFAULT_LAYOUT,
          op.wall,
          op.size,
          op.kind === "add_wing",
          op.at,
          false,
        );
        break;
      case "remove_wall": {
        if (IS_TOWN && next.town) {
          next.town = editTown(next.town, next.entities, op);
          break;
        }
        const layout = next.layout ?? structuredClone(DEFAULT_LAYOUT);
        const floor = new HouseFloor(layout);
        if (!floor.walls.some((w) => w.id === op.wall))
          throw new Error("Choose a wall that is still standing.");
        layout.removedWalls.push(op.wall);
        next.layout = layout;
        break;
      }
      case "set_behavior": {
        if (
          !Array.isArray(op.ids) ||
          op.ids.length < 1 ||
          op.ids.length > MAX_ASSETS ||
          new Set(op.ids).size !== op.ids.length ||
          !BEHAVIORS.includes(op.behavior)
        )
          throw new Error(
            "Choose ghosts and a float, chase, or still behavior.",
          );
        for (const id of op.ids) {
          const ghost = find(id);
          if (ghost.kind !== "ghost")
            throw new Error(
              "Only ghosts can use this behavior. Select a ghost first.",
            );
          ghost.behavior = op.behavior;
          const position = op.positions?.find((p) => p.id === id);
          if (op.behavior === "still" && position)
            Object.assign(ghost, { x: position.x, z: position.z });
        }
        break;
      }
      case "set_tint":
        next.tint = op.tint;
        break;
      case "undo":
        if (!this.past.length)
          throw new Error("There is no scene change to undo.");
        next = clone(this.past.at(-1)!);
        break;
      case "redo":
        if (!this.future.length)
          throw new Error("There is no scene change to redo.");
        next = clone(this.future.at(-1)!);
        break;
      case "batch": {
        if (
          !Array.isArray(op.operations) ||
          op.operations.length < 1 ||
          op.operations.length > 8 ||
          op.operations.some(
            (item) =>
              ![
                "add_asset",
                "add_pumpkin",
                "move_asset",
                "move_pumpkin",
                "resize_asset",
                "resize_pumpkin",
                "remove_asset",
                "remove_pumpkin",
                "add_room",
                "add_wing",
                "remove_wall",
                "set_behavior",
                "set_tint",
              ].includes(item.kind),
          )
        )
          throw new Error("Group between one and eight supported scene edits.");
        const staged = new SceneHistory(before);
        for (const [index, item] of op.operations.entries())
          staged
            .prepare({
              id: `${tx.id}-${index}`,
              project: PROJECT_ID,
              baseRevision: staged.project.revision,
              operation: item,
            })
            .commit();
        next = staged.project;
        break;
      }
      case "replace":
        next = readProject(op.project);
        if (next.revision !== before.revision + 1)
          throw new Error("Save this scene again using its current revision.");
        break;
      default:
        throw new Error("Choose a supported scene change.");
    }
    next.revision = before.revision + 1;
    next = readProject(next);
    return {
      project: next,
      commit: () => {
        if (this.project.revision !== before.revision)
          throw new Error(
            "The scene changed before this edit could be applied.",
          );
        if (op.kind === "undo") {
          this.past.pop();
          this.future.push(before);
        } else if (op.kind === "redo") {
          this.future.pop();
          this.past.push(before);
        } else {
          this.past.push(before);
          this.future = [];
        }
        this.past = this.past.slice(-32);
        this.future = this.future.slice(-32);
        this.project = next;
        this.seen.add(tx.id);
        if (this.seen.size > 128)
          this.seen.delete(this.seen.values().next().value!);
      },
    };
  }
}

export type Decision = {
  action: string;
  asset?: string;
  behavior?: string;
  target_scope?: string;
  placement: string;
  size: string;
  tint: string;
  confidence: number;
  needs_review: boolean;
  model: string;
  model_ms: number;
};
export function exactCoordinates(
  text: string,
): { x: number; z: number } | undefined {
  const number = "([-+]?(?:\\d+(?:\\.\\d*)?|\\.\\d+))";
  const x = text.match(new RegExp(`\\bx\\s*[=:]\\s*${number}`, "i"));
  const z = text.match(new RegExp(`\\bz\\s*[=:]\\s*${number}`, "i"));
  const pair = text.match(
    new RegExp(`\\(\\s*${number}\\s*,\\s*${number}\\s*\\)`),
  );
  if (x && z) return { x: Number(x[1]), z: Number(z[1]) };
  if (pair) return { x: Number(pair[1]), z: Number(pair[2]) };
}
/** Read quantities from the noun phrase, never from coordinates or model output. */
export function assetCount(text: string, kind: AssetKind): number {
  const words = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "hundred",
    "thousand",
  ];
  const nouns = [...ASSETS[kind].aliases]
    .sort((a, b) => b.length - a.length)
    .join("|");
  const match = text.match(
    new RegExp(
      `(?:^|\\s)([^\\s]+)\\s+(?:(?:very|really|huge|enormous|giant|gigantic|big|large|small|little|tiny|normal|medium|regular|orange|wooden|glowing|white|dark|green|stone|medieval|fantasy|two-story|one-story)\\s+)*(?:${nouns})\\b`,
      "i",
    ),
  );
  const token = match?.[1].toLowerCase();
  const previous = match
    ? text.slice(0, match.index).trim().split(/\s+/).at(-1)?.toLowerCase()
    : undefined;
  if (previous && (words.includes(previous) || /^[+\-\d.]/.test(previous)))
    throw new Error("Add between 1 and 8 assets at a time.");
  let count = 1;
  if (token && words.includes(token)) count = words.indexOf(token);
  else if (token && /^[+\-\d.]/.test(token)) count = Number(token);
  else if (
    new RegExp(
      `\\b(?:${ASSETS[kind].aliases.filter((n) => n.endsWith("s")).join("|")})\\b`,
      "i",
    ).test(text)
  )
    throw new Error("Choose a count from 1 to 8 assets.");
  if (!Number.isInteger(count) || count < 1 || count > 8)
    throw new Error("Add between 1 and 8 assets at a time.");
  return count;
}

export const pumpkinCount = (text: string) => assetCount(text, "pumpkin");

/** Place a group on the requested side without stacking it or moving the player. */
function groupPositions(
  count: number,
  kind: AssetKind,
  scale: number,
  placement: string,
  pose: Pose,
  anchor: { x: number; z: number },
  project: Project,
): { x: number; z: number }[] {
  const footprint = (kind: AssetKind, scale: number) => {
    const mesh = kit.meshes[kind as keyof typeof kit.meshes];
    return (
      scale *
      (mesh ? Math.max(mesh.dimensions[0], mesh.dimensions[2]) / 2 : 0.4)
    );
  };
  const radius = footprint(kind, scale);
  const floor = projectFloor(project);
  const candidates: { x: number; z: number; distance: number }[] = [];
  for (let row = 0; row < 36; row++) {
    for (let column = 0; column < 15; column++) {
      const forward = (placement === "ahead" ? 0.8 : -2) + row * 0.4;
      const side =
        placement === "left"
          ? -1.05 - column * 0.35
          : placement === "right"
            ? 1.05 + column * 0.35
            : (column - 7) * 0.35;
      const x =
        Math.round(
          (pose.x + Math.sin(pose.yaw) * forward + Math.cos(pose.yaw) * side) *
            100,
        ) / 100;
      const z =
        Math.round(
          (pose.z - Math.cos(pose.yaw) * forward + Math.sin(pose.yaw) * side) *
            100,
        ) / 100;
      // Explicit coordinates are the group's anchor; relative placement stays player-relative.
      const point =
        placement === "coordinates"
          ? {
              x: Math.round((anchor.x + (column - 7) * 0.35) * 100) / 100,
              z: Math.round((anchor.z + (row - 18) * 0.4) * 100) / 100,
            }
          : { x, z };
      if (Math.hypot(point.x - pose.x, point.z - pose.z) < radius + 0.5)
        continue;
      if (
        !Array.from({ length: 8 }, (_, i) => (i * Math.PI) / 4).every(
          (angle) =>
            floor instanceof TownFloor
              ? floor.canStand(
                  point.x + Math.cos(angle) * radius,
                  point.z + Math.sin(angle) * radius,
                  0.24,
                  pose.y ?? 0,
                )
              : floor.canStand(
                  point.x + Math.cos(angle) * radius,
                  point.z + Math.sin(angle) * radius,
                ),
        )
      )
        continue;
      if (
        project.entities.some(
          (e) =>
            Math.hypot(e.x - point.x, e.z - point.z) <
            radius + footprint(e.kind, e.scale) + 0.1,
        )
      )
        continue;
      candidates.push({
        ...point,
        distance:
          placement === "coordinates"
            ? Math.hypot(point.x - anchor.x, point.z - anchor.z)
            : Math.hypot(point.x - pose.x, point.z - pose.z),
      });
    }
  }
  candidates.sort((a, b) => a.distance - b.distance);
  // Try nearby starting points so an early greedy choice cannot block a group that fits.
  for (const seed of candidates.slice(0, 64)) {
    const placed: { x: number; z: number }[] = [{ x: seed.x, z: seed.z }];
    for (const point of candidates) {
      if (
        placed.every(
          (p) => Math.hypot(p.x - point.x, p.z - point.z) >= radius * 2 + 0.1,
        )
      )
        placed.push({ x: point.x, z: point.z });
      if (placed.length === count) return placed;
    }
  }
  throw new Error(
    "There isn't enough room for that group here. Try smaller assets or move to a more open spot.",
  );
}

export function decisionOperation(
  d: Decision,
  text: string,
  pose: Pose,
  selected: string | null,
  project: Project = initialProject(),
  wallTarget?: { id: string; x: number; z: number; y?: number },
): Operation {
  if (
    !d ||
    !Number.isFinite(d.confidence) ||
    d.confidence < 0.75 ||
    d.confidence > 1 ||
    d.needs_review
  )
    throw new Error(
      "We couldn’t understand that edit. Try the scene controls or rephrase it.",
    );
  const explicitId = text.match(
    new RegExp(`\\b(?:${Object.keys(ASSETS).join("|")})-[a-zA-Z0-9-]+\\b`),
  )?.[0];
  const id = explicitId ?? selected;
  const target = () => {
    const named = Object.entries(ASSETS).filter(([, asset]) =>
      new RegExp(`\\b(?:${asset.aliases.join("|")})\\b`, "i").test(text),
    );
    if (named.length > 1) throw new Error("Select one asset to change.");
    if (named.length === 1 && !explicitId) {
      const kind = named[0][0];
      if (project.entities.find((e) => e.id === id)?.kind !== kind) {
        const matches = project.entities.filter((e) => e.kind === kind);
        if (matches.length !== 1)
          throw new Error(`Select a ${named[0][1].label.toLowerCase()} first.`);
        return matches[0].id;
      }
    }
    if (!id) throw new Error("Select an asset first.");
    return id;
  };
  const scale = () => {
    if (d.size === "exact") {
      const n = text.match(/\bscale\s*[=:]\s*([-+]?(?:\d+(?:\.\d*)?|\.\d+))/i);
      if (!n) throw new Error("Write the exact size as scale=1.2.");
      return Number(n[1]);
    }
    if (d.size === "small") return 0.6;
    if (d.size === "large") return 1.8;
    if (d.size === "huge") return 2.5;
    if (d.size === "normal") return 1;
    throw new Error("Choose small, normal, large, huge, or an exact scale.");
  };
  const position = () => {
    if (d.placement === "coordinates") {
      const p = exactCoordinates(text);
      if (!p) throw new Error("Write a position as x=1, z=-2 or (1, -2).");
      return p;
    }
    if (!["ahead", "left", "right"].includes(d.placement))
      throw new Error(
        "Choose a position ahead, to your left, to your right, or enter x and z.",
      );
    const side =
      d.placement === "left" ? -1.4 : d.placement === "right" ? 1.4 : 0;
    return {
      x:
        Math.round(
          (pose.x + Math.sin(pose.yaw) * 2 + Math.cos(pose.yaw) * side) * 100,
        ) / 100,
      z:
        Math.round(
          (pose.z - Math.cos(pose.yaw) * 2 + Math.sin(pose.yaw) * side) * 100,
        ) / 100,
    };
  };
  switch (d.action) {
    case "add_asset":
    case "add_pumpkin": {
      const kind = d.action === "add_pumpkin" ? "pumpkin" : d.asset;
      if (!kind || !Object.hasOwn(ASSETS, kind))
        throw new Error("Choose an asset from the list.");
      const count = assetCount(text, kind as AssetKind);
      if (project.entities.length + count > MAX_ASSETS)
        throw new Error(
          `This scene has room for ${MAX_ASSETS - project.entities.length} more assets. Remove some before adding this group.`,
        );
      const size = scale(),
        anchor = position();
      const positions =
        IS_TOWN && isHouse(kind) && project.town
          ? housePositions(
              project.entities,
              project.town,
              kind as "cottage" | "townhouse",
              count,
              size,
              pose,
              d.placement,
              anchor,
            )
          : count === 1
            ? [anchor]
            : groupPositions(
                count,
                kind as AssetKind,
                size,
                d.placement,
                pose,
                anchor,
                project,
              );
      const operations: Operation[] = positions.map((p) => ({
        kind: d.action === "add_pumpkin" ? "add_pumpkin" : "add_asset",
        entity: {
          id: `${kind}-${crypto.randomUUID()}`,
          kind: kind as AssetKind,
          ...p,
          scale: size,
          ...(IS_TOWN ? { y: isHouse(kind) ? 0 : (pose.y ?? 0) } : {}),
        },
      }));
      return count === 1 ? operations[0] : { kind: "batch", operations };
    }
    case "move_asset":
    case "move_pumpkin":
      return {
        kind: d.action as "move_asset" | "move_pumpkin",
        id: target(),
        ...position(),
        ...(IS_TOWN ? { y: pose.y ?? 0 } : {}),
      };
    case "resize_asset":
    case "resize_pumpkin":
      return {
        kind: d.action as "resize_asset" | "resize_pumpkin",
        id: target(),
        scale: scale(),
      };
    case "remove_asset":
    case "remove_pumpkin":
      return {
        kind: d.action as "remove_asset" | "remove_pumpkin",
        id: target(),
      };
    case "add_room":
    case "add_wing":
    case "remove_wall": {
      const floor = projectFloor(project);
      const turn =
        d.placement === "left"
          ? -Math.PI / 2
          : d.placement === "right"
            ? Math.PI / 2
            : d.placement === "behind"
              ? Math.PI
              : 0;
      const aimed = floor.pick(pose, turn);
      const wall =
        d.placement === "ahead" && wallTarget
          ? wallTarget
          : aimed
            ? { id: aimed.id, x: aimed.hitX, z: aimed.hitZ }
            : undefined;
      if (!wall || !["ahead", "left", "right", "behind"].includes(d.placement))
        throw new Error("Look toward a wall or choose one in House layout.");
      if (d.action === "remove_wall")
        return { kind: "remove_wall", wall: wall.id };
      const size =
        d.size === "small"
          ? 4
          : d.size === "normal"
            ? 6
            : d.size === "large"
              ? 8
              : d.size === "huge"
                ? 10
                : undefined;
      if (size === undefined)
        throw new Error("Choose a small, normal, large, or huge room.");
      return {
        kind: d.action,
        wall: wall.id,
        size,

        at: { x: wall.x, z: wall.z },
      };
    }
    case "set_behavior": {
      if (!BEHAVIORS.includes(d.behavior as Behavior))
        throw new Error("Choose float, chase, or still for the ghosts.");
      let ids: string[];
      if (d.target_scope === "all_ghosts")
        ids = project.entities
          .filter((e) => e.kind === "ghost")
          .map((e) => e.id);
      else if (d.target_scope === "selected") {
        if (
          !selected ||
          project.entities.find((e) => e.id === selected)?.kind !== "ghost"
        )
          throw new Error("Select a ghost first.");
        ids = [selected];
      } else if (d.target_scope === "named" && explicitId) ids = [explicitId];
      else throw new Error("Choose the selected ghost or all ghosts.");
      if (!ids.length) throw new Error("Add a ghost first.");
      if (
        ids.some(
          (id) => project.entities.find((e) => e.id === id)?.kind !== "ghost",
        )
      )
        throw new Error("Choose an existing ghost.");
      return { kind: "set_behavior", ids, behavior: d.behavior as Behavior };
    }
    case "set_tint":
      if (!Object.hasOwn(TINTS, d.tint))
        throw new Error("Choose warm, orange, blue, or green light.");
      return { kind: "set_tint", tint: d.tint as Tint };
    case "undo":
    case "redo":
      return { kind: d.action };
    default:
      throw new Error(
        "Try adding, moving, resizing, or removing an asset, changing the light, undo, or redo.",
      );
  }
}

/** Upgrade only the known v1 demo layout; custom and current shaders stay intact. */
export function upgradeLegacyShader(source: string): string {
  if (
    source.startsWith("// Lantern Hall") &&
    source.includes("  light: vec4<f32>,") &&
    source.includes("LANTERN_TINT * toon") &&
    !/^\s*tint:\s*vec4<f32>/m.test(source)
  )
    return source
      .replace("  light: vec4<f32>,", "  light: vec4<f32>,\n  tint: vec4<f32>,")
      .replace("LANTERN_TINT * toon", "LANTERN_TINT * scene.tint.xyz * toon");
  return source;
}
