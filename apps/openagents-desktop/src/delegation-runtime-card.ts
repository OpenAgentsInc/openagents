import type {
  SafeMessageChainEntry,
  SafeTurnProjection,
} from "@openagentsinc/agent-runtime-schema";

import type { DesktopRuntimeCard } from "./chat-contract.ts";

export type DelegationRuntimeCard = Extract<DesktopRuntimeCard, { kind: "child" }>;
type DelegationTranscript = NonNullable<DelegationRuntimeCard["transcript"]>;

/** The stable child ref for one delegated kernel turn. */
export const AFS_DELEGATION_CHILD_REF = "codex" as const;

const DELEGATION_TRANSCRIPT_TEXT_LIMIT = 4_000;
const commandToolLabel = /(?:^|[_ -])(bash|command|exec|shell|terminal)(?:$|[_ -])/iu;

const activityStatus = (
  entry: SafeMessageChainEntry,
  cardState: SafeTurnProjection["cardState"],
): "running" | "completed" | "failed" => {
  if (entry.commandOutputByteCount !== undefined) return "completed";
  if (cardState === "failed" || cardState === "refused" || cardState === "cancelled")
    return "failed";
  if (cardState === "done") return "completed";
  return "running";
};

const transcriptActivity = (
  entry: SafeMessageChainEntry,
  cardState: SafeTurnProjection["cardState"],
): DelegationTranscript[number]["activity"] => {
  const status = activityStatus(entry, cardState);
  if (entry.role === "system") return { kind: "reasoning", label: "Reasoning", status };
  if (entry.role !== "tool") return undefined;
  const label = (entry.toolLabel ?? "Tool").slice(0, 120);
  if (entry.fileChangeCount !== undefined) {
    return {
      kind: "file_change",
      label,
      status,
      fileChangeCount: entry.fileChangeCount,
      ...(entry.commandOutputByteCount === undefined
        ? {}
        : { outputByteCount: entry.commandOutputByteCount }),
    };
  }
  if (commandToolLabel.test(label)) {
    return {
      kind: "command",
      label,
      status,
      ...(entry.commandOutputByteCount === undefined
        ? {}
        : { outputByteCount: entry.commandOutputByteCount }),
    };
  }
  return {
    kind: "tool",
    label,
    status,
    ...(entry.commandOutputByteCount === undefined
      ? {}
      : { outputByteCount: entry.commandOutputByteCount }),
  };
};

const transcriptText = (entry: SafeMessageChainEntry): string => {
  if (entry.text.trim() !== "") return entry.text.slice(0, DELEGATION_TRANSCRIPT_TEXT_LIMIT);
  const parts: Array<string> = [];
  if (entry.toolLabel !== undefined) parts.push(entry.toolLabel);
  if (entry.fileChangeCount !== undefined) {
    parts.push(`${entry.fileChangeCount} file${entry.fileChangeCount === 1 ? "" : "s"}`);
  }
  if (entry.commandOutputByteCount !== undefined)
    parts.push(`${entry.commandOutputByteCount} bytes`);
  return parts.join(" · ").slice(0, DELEGATION_TRANSCRIPT_TEXT_LIMIT);
};

const childStatus = (cardState: SafeTurnProjection["cardState"]): DelegationRuntimeCard["status"] =>
  cardState === "done"
    ? "completed"
    : cardState === "queued" || cardState === "running"
      ? "running"
      : "failed";

/** Convert one safe kernel projection into the runtime card used by live and reload paths. */
export const delegationCardFromProjection = (
  projection: SafeTurnProjection,
  title: string,
  detail: string,
): DelegationRuntimeCard => {
  const failureReason = projection.failureReason;
  const transcript = projection.messageChain.map((entry) => {
    const activity = transcriptActivity(entry, projection.cardState);
    return {
      entryRef: entry.entryRef.slice(0, 120),
      role: entry.role,
      text: transcriptText(entry),
      ...(activity === undefined ? {} : { activity }),
    };
  });
  return {
    kind: "child",
    turnRef: projection.requestRef.slice(0, 120),
    childRef: AFS_DELEGATION_CHILD_REF,
    status: childStatus(projection.cardState),
    title: title.slice(0, 400),
    detail: (failureReason ?? detail).slice(0, 400),
    transcript:
      failureReason === undefined
        ? transcript
        : [
            ...transcript,
            {
              entryRef: `${projection.requestRef}.failure`.slice(0, 120),
              role: "system" as const,
              text: failureReason.slice(0, DELEGATION_TRANSCRIPT_TEXT_LIMIT),
              activity: { kind: "notice" as const, label: "Failure", status: "failed" as const },
            },
          ],
    steered: null,
  };
};

/** Build the initial card after the host confirms a delegated turn start. */
export const seedDelegationCard = (
  delegationRef: string,
  objective: string,
): DelegationRuntimeCard => ({
  kind: "child",
  turnRef: delegationRef.slice(0, 120),
  childRef: AFS_DELEGATION_CHILD_REF,
  status: "running",
  title: "Codex subagent",
  detail: objective.slice(0, 400),
  transcript: [
    {
      entryRef: `${delegationRef}.prompt`.slice(0, 120),
      role: "user",
      text: objective.slice(0, DELEGATION_TRANSCRIPT_TEXT_LIMIT),
    },
  ],
  steered: null,
});
