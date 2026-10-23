import { ASSETS, type Operation } from "./project.ts";

export type Change = {
  id: string;
  author: string;
  at: string;
  summary: string;
  revision?: number;
  scope: "world" | "draft";
  url?: string;
  earnings: null;
};

export function readChanges(text: string | null): Change[] {
  if (!text) return [];
  if (new TextEncoder().encode(text).length > 64 * 1024)
    throw new Error("The change log is too large.");
  const data = JSON.parse(text);
  if (
    data.schema !== 1 ||
    !Array.isArray(data.changes) ||
    data.changes.length > 64
  )
    throw new Error("The change log could not be opened.");
  return data.changes.map((row: Change) => {
    if (
      !row ||
      typeof row.id !== "string" ||
      row.id.length > 100 ||
      typeof row.author !== "string" ||
      !row.author.trim() ||
      row.author.length > 100 ||
      typeof row.summary !== "string" ||
      row.summary.length > 300 ||
      typeof row.at !== "string" ||
      !Number.isFinite(Date.parse(row.at))
    )
      throw new Error("The change log could not be opened.");
    // A browser draft is not a payment record and cannot assign itself earnings.
    return {
      id: row.id,
      author: row.author,
      at: row.at,
      summary: row.summary,
      revision: Number.isSafeInteger(row.revision) ? row.revision : undefined,
      scope: "draft",
      earnings: null,
    };
  });
}

export function describeChange(op: Operation): string {
  switch (op.kind) {
    case "add_asset":
    case "add_pumpkin":
      return `Added ${ASSETS[op.entity.kind].label.toLowerCase()}`;
    case "move_asset":
    case "move_pumpkin":
      return "Moved the selected object";
    case "resize_asset":
    case "resize_pumpkin":
      return "Resized the selected object";
    case "remove_asset":
    case "remove_pumpkin":
      return "Removed the selected object";
    case "set_tint":
      return `Changed lantern light to ${op.tint}`;
    case "set_behavior":
      return `Changed ghost behavior to ${op.behavior}`;
    case "add_room":
      return "Added a room";
    case "add_wing":
      return "Added a wing";
    case "remove_wall":
      return "Removed a wall";
    case "set_material_source":
      return "Changed the material shader";
    case "undo":
      return "Undid the previous change";
    case "redo":
      return "Restored the previous change";
    case "replace":
      return "Updated the saved scene";
    case "batch":
      return `Made ${op.operations.length} changes: ${op.operations.map(describeChange).join("; ")}`.slice(
        0,
        300,
      );
  }
}
