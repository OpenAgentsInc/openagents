import type { DesktopRuntimeTranscriptEntry } from "./chat-contract.ts";
import type { ClaudeLocalEvent } from "./claude-local-contract.ts";
import type { WorkbenchItem } from "./workbench-item-contract.ts";

type ChildActivityEvent = Extract<ClaudeLocalEvent, { kind: "child_activity" }>;

const activityStatus = (event: ChildActivityEvent): "running" | "completed" | "failed" =>
  event.itemStatus ?? "completed";

const activityFromItem = (
  item: WorkbenchItem,
  status: "running" | "completed" | "failed",
): DesktopRuntimeTranscriptEntry["activity"] => {
  switch (item.kind) {
    case "command":
      return { kind: "command", label: "Command", status };
    case "fileChange":
      return {
        kind: "file_change",
        label: "File changes",
        status,
        fileChangeCount: item.changes.length,
      };
    case "toolCall":
      return { kind: "tool", label: item.tool, status };
    case "reasoning":
      return { kind: "reasoning", label: "Reasoning", status };
    case "notice":
      return { kind: "notice", label: "Notice", status };
    default:
      return undefined;
  }
};

const roleFromItem = (item: WorkbenchItem | undefined): DesktopRuntimeTranscriptEntry["role"] => {
  if (item?.kind === "message") return item.role;
  if (item?.kind === "command" || item?.kind === "fileChange" || item?.kind === "toolCall")
    return "tool";
  return "system";
};

/** Reconcile one child item by stable provider identity while preserving source order. */
export const reconcileChildActivityTranscript = (
  transcript: ReadonlyArray<DesktopRuntimeTranscriptEntry>,
  event: ChildActivityEvent,
): ReadonlyArray<DesktopRuntimeTranscriptEntry> => {
  const activity =
    event.item === undefined ? undefined : activityFromItem(event.item, activityStatus(event));
  const entry: DesktopRuntimeTranscriptEntry = {
    ...(event.itemRef === undefined ? {} : { entryRef: event.itemRef }),
    role: roleFromItem(event.item),
    text: event.summary,
    ...(activity === undefined ? {} : { activity }),
  };
  const index =
    event.itemRef === undefined
      ? -1
      : transcript.findIndex((candidate) => candidate.entryRef === event.itemRef);
  const next =
    index === -1
      ? [...transcript, entry]
      : transcript.map((candidate, cursor) => (cursor === index ? entry : candidate));
  return next.slice(-128);
};
