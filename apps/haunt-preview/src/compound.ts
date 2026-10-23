import { IS_TOWN } from "./lab.ts";
import { TownFloor, isHouse } from "./town.ts";
import kit from "./medieval-kit.json" with { type: "json" };
import {
  assetCount,
  assetScale,
  decisionAsset,
  decisionOperation,
  SceneHistory,
  PROJECT_ID,
  MAX_BATCH_EDITS,
  type Decision,
  type Operation,
  type Project,
  type SceneAsset,
  type AssetKind,
} from "./project.ts";
import type { Pose } from "./scene.ts";

type WallTarget = { id: string; x: number; z: number; y?: number };
const references = [
  "none",
  "new_assets",
  "outside_new_buildings",
  "each_new_building",
  "selected",
  "all_ghosts",
  "named",
  "unsupported",
];
function check(d: Decision) {
  if (
    !d ||
    !Number.isFinite(d.confidence) ||
    d.confidence < 0.75 ||
    d.confidence > 1 ||
    d.needs_review ||
    d.action === "unsupported"
  )
    throw new Error(
      "We couldn’t understand every part of that edit. Try describing the changes separately.",
    );
}
const radius = (kind: AssetKind, scale: number) => {
  const mesh = kit.meshes[kind as keyof typeof kit.meshes];
  return (
    (mesh ? Math.max(mesh.dimensions[0], mesh.dimensions[2]) / 2 : 0.4) * scale
  );
};

/** Plan all parts against temporary scene copies before the worker changes anything. */
export function compileDecision(
  d: Decision,
  text: string,
  pose: Pose,
  selected: string | null,
  project: Project,
  wall?: WallTarget,
): Operation {
  if (d.action !== "compound" && d.placement !== "outside")
    return decisionOperation(d, text, pose, selected, project, wall);
  check(d);
  const staged = new SceneHistory(project),
    operations: Operation[] = [];
  let newest: string[] = [],
    houses: string[] = [];
  const append = (operation: Operation) => {
    const items =
      operation.kind === "batch" ? operation.operations : [operation];
    for (const item of items) {
      if (
        item.kind === "batch" ||
        ["undo", "redo", "replace"].includes(item.kind)
      )
        throw new Error("Use undo and redo on their own.");
      if (operations.length >= MAX_BATCH_EDITS)
        throw new Error("Combine up to 32 scene edits at a time.");
      staged
        .prepare({
          id: crypto.randomUUID(),
          project: PROJECT_ID,
          baseRevision: staged.project.revision,
          operation: item,
        })
        .commit();
      operations.push(item);
    }
  };
  const outside = (
    part: Decision,
    source: string,
    parents: SceneAsset[],
    each: boolean,
  ) => {
    if (
      !IS_TOWN ||
      !staged.project.town ||
      !parents.length ||
      parents.some(
        (parent) =>
          !isHouse(parent.kind) || !staged.project.town!.buildings[parent.id],
      )
    )
      throw new Error("Choose a house to place things outside.");
    const kind = decisionAsset(part.asset);
    if (!kind || isHouse(kind) || part.action !== "add_asset")
      throw new Error("Choose a prop to place outside the houses.");
    const scale = assetScale(part.size, source);
    if (!Number.isFinite(scale) || scale < 0.3 || scale > 2.5)
      throw new Error("Choose a size between 0.3 and 2.5.");
    const count =
      assetCount(
        source,
        kind,
        ["any_asset", "furniture"].includes(part.asset ?? ""),
        each ? 1 : parents.length,
      ) * (each ? parents.length : 1);
    if (count < 1 || count > 8)
      throw new Error("Add between 1 and 8 outside props at a time.");
    const added: string[] = [];
    for (let i = 0; i < count; i++) {
      const parent = parents[i % parents.length],
        plan = staged.project.town!.buildings[parent.id].floors[0];
      const left = Math.min(
        ...plan.rooms.map((r) => parent.x + (r.x - r.width / 2) * parent.scale),
      );
      const right = Math.max(
        ...plan.rooms.map((r) => parent.x + (r.x + r.width / 2) * parent.scale),
      );
      const north = Math.min(
        ...plan.rooms.map((r) => parent.z + (r.z - r.depth / 2) * parent.scale),
      );
      const south = Math.max(
        ...plan.rooms.map((r) => parent.z + (r.z + r.depth / 2) * parent.scale),
      );
      const r = radius(kind, scale),
        floor = new TownFloor(staged.project.entities, staged.project.town!);
      const points: { x: number; z: number }[] = [];
      for (const gap of [1.8, 3, 5, 8]) {
        const margin = r + gap * parent.scale;
        for (const fraction of [0.8, 0.2, 0.5, 0, 1]) {
          const x = left + (right - left) * fraction,
            z = north + (south - north) * fraction;
          points.push(
            { x, z: south + margin },
            { x: right + margin, z },
            { x: left - margin, z },
            { x, z: north - margin },
          );
        }
      }
      const point = points.find((p) => {
        if (Math.hypot(p.x - pose.x, p.z - pose.z) < r + 2.5) return false;
        // Keep a clear approach to the cottage and townhouse front doors.
        if (
          Math.abs(p.x - (parent.x - 2 * parent.scale)) < r + parent.scale &&
          p.z > south
        )
          return false;
        if (
          !Array.from({ length: 8 }, (_, n) => (n * Math.PI) / 4).every((a) => {
            const x = p.x + Math.cos(a) * r,
              z = p.z + Math.sin(a) * r;
            return !floor.roomAt(x, z, 0) && floor.canStand(x, z, 0.24, 0);
          })
        )
          return false;
        return staged.project.entities.every(
          (e) =>
            isHouse(e.kind) ||
            (e.y ?? 0) > 0.5 ||
            Math.hypot(e.x - p.x, e.z - p.z) >=
              r + radius(e.kind, e.scale) + 0.2,
        );
      });
      if (!point)
        throw new Error(
          "There isn't enough clear space outside those houses. Try smaller props.",
        );
      const id = `${kind}-${crypto.randomUUID()}`;
      append({
        kind: "add_asset",
        entity: { id, kind, ...point, scale, y: 0 },
      });
      added.push(id);
    }
    return added;
  };
  if (d.action !== "compound") {
    const candidates = project.entities.filter((e) => isHouse(e.kind));
    candidates.sort((a, b) =>
      a.id === selected
        ? -1
        : b.id === selected
          ? 1
          : Math.hypot(a.x - pose.x, a.z - pose.z) -
            Math.hypot(b.x - pose.x, b.z - pose.z),
    );
    outside(d, text, candidates.slice(0, 1), false);
  } else {
    if (!Array.isArray(d.steps) || d.steps.length < 2 || d.steps.length > 4)
      throw new Error("Combine between two and four changes at a time.");
    const bytes = new TextEncoder().encode(text),
      decoder = new TextDecoder("utf-8", { fatal: true });
    let end = 0;
    const separator = (value: string) =>
      value.replace(/\b(?:and|then|also)\b/gi, "").replace(/[\s,;.]/g, "") ===
      "";
    for (const step of d.steps) {
      if (
        !Number.isInteger(step.start) ||
        !Number.isInteger(step.end) ||
        step.start < end ||
        step.end <= step.start ||
        step.end > bytes.length ||
        !separator(decoder.decode(bytes.slice(end, step.start))) ||
        decoder.decode(bytes.slice(step.start, step.end)) !== step.text ||
        !references.includes(step.reference)
      )
        throw new Error(
          "The combined edit did not match your request. Try the changes separately.",
        );
      end = step.end;
      const part = step.decision;
      check(part);
      if (
        part.action === "compound" ||
        part.steps?.length ||
        ["undo", "redo"].includes(part.action)
      )
        throw new Error("Use undo and redo on their own.");
      if (part.placement === "outside") {
        const parents = (step.reference === "selected" ? [selected] : houses)
          .map((id) => staged.project.entities.find((e) => e.id === id)!)
          .filter(Boolean);
        if (
          !["outside_new_buildings", "each_new_building", "selected"].includes(
            step.reference,
          )
        )
          throw new Error("Choose which houses to place the props outside.");
        newest = outside(
          part,
          step.text,
          parents,
          step.reference === "each_new_building" ||
            /\b(?:each|every|per|apiece)\b/i.test(step.text),
        );
        continue;
      }
      const before = staged.project.entities.map((e) => e.id);
      if (
        step.reference === "new_assets" &&
        ["set_behavior", "move_asset", "resize_asset", "remove_asset"].includes(
          part.action,
        )
      ) {
        if (!newest.length)
          throw new Error("Add the objects before changing them.");
        const amount = step.text.match(
          /\b([-+]?\d+(?:\.\d+)?|zero|one|two|three|four|five|six|seven|eight|nine|ten|twenty)\s+(?:of\s+)?(?:them|those|these)\b/i,
        );
        const numbers = [
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
        ];
        const count = amount
          ? /^[-+]?\d/.test(amount[1])
            ? Number(amount[1])
            : numbers.indexOf(amount[1].toLowerCase())
          : newest.length;
        if (!Number.isInteger(count) || count < 1 || count > newest.length)
          throw new Error("Choose a count that fits the new group.");
        const targets = newest.slice(0, count);
        for (const id of targets)
          append(
            decisionOperation(
              { ...part, target_scope: "selected" },
              step.text,
              pose,
              id,
              staged.project,
              wall,
            ),
          );
      } else {
        if (
          step.reference === "unsupported" &&
          [
            "set_behavior",
            "move_asset",
            "resize_asset",
            "remove_asset",
          ].includes(part.action)
        )
          throw new Error("Choose which objects to change.");
        append(
          decisionOperation(
            {
              ...part,
              target_scope:
                step.reference === "all_ghosts"
                  ? "all_ghosts"
                  : step.reference === "selected"
                    ? "selected"
                    : step.reference === "named"
                      ? "named"
                      : part.target_scope,
            },
            step.text,
            pose,
            selected,
            staged.project,
            wall,
          ),
        );
      }
      const added = staged.project.entities.filter(
        (e) => !before.includes(e.id),
      );
      if (added.length) newest = added.map((e) => e.id);
      const buildings = added.filter((e) => isHouse(e.kind));
      if (buildings.length) houses = buildings.map((e) => e.id);
    }
    if (!separator(decoder.decode(bytes.slice(end))))
      throw new Error("Describe every part of the combined edit.");
  }
  return { kind: "batch", operations };
}
