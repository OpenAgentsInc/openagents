import type { Pose } from "./scene.ts";

export const PROJECT_ID = "haunt-webgpu-v1";
export const SCENE_PATH = "haunt/scene.json";
export const MAX_PUMPKINS = 64;
export const TINTS = {
  warm: [1, 1, 1],
  orange: [1, 0.55, 0.28],
  blue: [0.2, 0.55, 2.5],
  green: [0.25, 1.5, 0.55],
} as const;
export type Tint = keyof typeof TINTS;
export type Pumpkin = {
  id: string;
  kind: "pumpkin";
  x: number;
  z: number;
  scale: number;
};
export type Project = {
  schema: 1;
  id: typeof PROJECT_ID;
  revision: number;
  material: "haunt/material.wgsl";
  tint: Tint;
  entities: Pumpkin[];
};
export type Operation =
  | { kind: "add_pumpkin"; entity: Pumpkin }
  | { kind: "move_pumpkin"; id: string; x: number; z: number }
  | { kind: "resize_pumpkin"; id: string; scale: number }
  | { kind: "remove_pumpkin"; id: string }
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

export function walkable(x: number, z: number): boolean {
  return (
    (Math.abs(x) < 3.18 && z > -4.8 && z < 7.7) ||
    (Math.abs(x) < 0.98 && z > -9.3 && z < -4.6) ||
    (Math.abs(x) < 3.68 && z > -16.7 && z < -9.1)
  );
}
export function initialProject(): Project {
  return {
    schema: 1,
    id: PROJECT_ID,
    revision: 0,
    material: "haunt/material.wgsl",
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
function validEntity(value: unknown): asserts value is Pumpkin {
  const e = value as Pumpkin;
  if (
    !e ||
    e.kind !== "pumpkin" ||
    typeof e.id !== "string" ||
    !/^[a-zA-Z0-9-]{1,64}$/.test(e.id) ||
    !Number.isFinite(e.x) ||
    !Number.isFinite(e.z) ||
    !walkable(e.x, e.z) ||
    !Number.isFinite(e.scale) ||
    e.scale < 0.3 ||
    e.scale > 2.5
  )
    throw new Error(
      "Place a pumpkin inside the rooms, with size between 0.3 and 2.5.",
    );
}
export function readProject(value: unknown): Project {
  const p = value as Project;
  if (
    !p ||
    p.schema !== 1 ||
    p.id !== PROJECT_ID ||
    p.material !== "haunt/material.wgsl" ||
    !Number.isSafeInteger(p.revision) ||
    p.revision < 0 ||
    !Object.hasOwn(TINTS, p.tint) ||
    !Array.isArray(p.entities) ||
    p.entities.length > MAX_PUMPKINS
  )
    throw new Error(
      "This scene file has an unsupported version or invalid contents.",
    );
  const ids = new Set<string>();
  for (const e of p.entities) {
    validEntity(e);
    if (ids.has(e.id)) throw new Error("Each pumpkin needs a different name.");
    ids.add(e.id);
  }
  // Copy only project fields. GPU handles, player state, and model data never enter a save.
  return {
    schema: 1,
    id: PROJECT_ID,
    revision: p.revision,
    material: "haunt/material.wgsl",
    tint: p.tint,
    entities: p.entities.map((e) => ({
      id: e.id,
      kind: "pumpkin",
      x: e.x,
      z: e.z,
      scale: e.scale,
    })),
  };
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
      if (!e) throw new Error("Select an existing pumpkin first.");
      return e;
    };
    switch (op.kind) {
      case "add_pumpkin":
        validEntity(op.entity);
        if (next.entities.some((e) => e.id === op.entity.id))
          throw new Error("That pumpkin name is already in use.");
        next.entities.push(clone(op.entity));
        break;
      case "move_pumpkin":
        Object.assign(find(op.id), { x: op.x, z: op.z });
        break;
      case "resize_pumpkin":
        find(op.id).scale = op.scale;
        break;
      case "remove_pumpkin":
        find(op.id);
        next.entities = next.entities.filter((e) => e.id !== op.id);
        break;
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
                "add_pumpkin",
                "move_pumpkin",
                "resize_pumpkin",
                "remove_pumpkin",
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
export function pumpkinCount(text: string): number {
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
  const match = text.match(
    /(?:^|\s)([^\s]+)\s+(?:(?:very|really|huge|enormous|giant|gigantic|big|large|small|little|tiny|normal|medium|regular|orange)\s+)*pumpkins?\b/i,
  );
  const token = match?.[1].toLowerCase();
  const previous = match
    ? text.slice(0, match.index).trim().split(/\s+/).at(-1)?.toLowerCase()
    : undefined;
  if (previous && (words.includes(previous) || /^[+\-\d.]/.test(previous)))
    throw new Error("Add between 1 and 8 pumpkins at a time.");
  let count = 1;
  if (token && words.includes(token)) count = words.indexOf(token);
  else if (token && /^[+\-\d.]/.test(token)) count = Number(token);
  else if (/\bpumpkins\b/i.test(text))
    throw new Error("Choose a count from 1 to 8 pumpkins.");
  if (!Number.isInteger(count) || count < 1 || count > 8)
    throw new Error("Add between 1 and 8 pumpkins at a time.");
  return count;
}

/** Place a group on the requested side without stacking it or moving the player. */
function groupPositions(
  count: number,
  scale: number,
  placement: string,
  pose: Pose,
  anchor: { x: number; z: number },
  project: Project,
): { x: number; z: number }[] {
  const radius = scale * 0.4;
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
        !Array.from({ length: 8 }, (_, i) => (i * Math.PI) / 4).every((angle) =>
          walkable(
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
            radius + e.scale * 0.4 + 0.1,
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
    "There isn't enough room for that group here. Try smaller pumpkins or move to a more open spot.",
  );
}

export function decisionOperation(
  d: Decision,
  text: string,
  pose: Pose,
  selected: string | null,
  project: Project = initialProject(),
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
  const id = text.match(/\bpumpkin-[a-zA-Z0-9-]+\b/)?.[0] ?? selected;
  const target = () => {
    if (!id) throw new Error("Select a pumpkin first.");
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
    case "add_pumpkin": {
      const count = pumpkinCount(text);
      if (project.entities.length + count > MAX_PUMPKINS)
        throw new Error(
          `This scene has room for ${MAX_PUMPKINS - project.entities.length} more pumpkins. Remove some before adding this group.`,
        );
      const size = scale(),
        anchor = position();
      const positions =
        count === 1
          ? [anchor]
          : groupPositions(count, size, d.placement, pose, anchor, project);
      const operations: Operation[] = positions.map((p) => ({
        kind: "add_pumpkin",
        entity: {
          id: `pumpkin-${crypto.randomUUID()}`,
          kind: "pumpkin",
          ...p,
          scale: size,
        },
      }));
      return count === 1 ? operations[0] : { kind: "batch", operations };
    }
    case "move_pumpkin":
      return { kind: "move_pumpkin", id: target(), ...position() };
    case "resize_pumpkin":
      return { kind: "resize_pumpkin", id: target(), scale: scale() };
    case "remove_pumpkin":
      return { kind: "remove_pumpkin", id: target() };
    case "set_tint":
      if (!Object.hasOwn(TINTS, d.tint))
        throw new Error("Choose warm, orange, blue, or green light.");
      return { kind: "set_tint", tint: d.tint as Tint };
    case "undo":
    case "redo":
      return { kind: d.action };
    default:
      throw new Error(
        "Try adding, moving, resizing, or removing a pumpkin, changing the light, undo, or redo.",
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
