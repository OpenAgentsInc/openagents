import { Button } from "#components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "#components/ui/tooltip";
import { ReactBootSequence } from "./react-boot-sequence.tsx";
import type { BootSequenceAgentLine } from "./boot-sequence.ts";
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "#components/ui/message-scroller";
import {
  ComponentValueBinding,
  IntentRef,
  type IntentError,
  type IntentReporter,
  type JsonPayload,
  type MarkdownBlock,
  type MarkdownInline,
} from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import {
  DesktopAgentGroup,
  DesktopPlanCard,
  DesktopTimelineMessage,
  DesktopTimelineNotice,
  DesktopWorkEntry,
  dispatchWorkbenchItem,
  type DesktopAgentActivity,
  type DesktopAgentStatus,
  type WorkbenchDispatchItem,
} from "@openagentsinc/ui/desktop-workbench";
import type { ReactElement, ReactNode, RefObject } from "react";
import {
  Component,
  createElement,
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { CheckIcon, ChevronRight, CopyIcon, Folder, FolderPen } from "lucide-react";

import { localDelegateAgentRef } from "../live-agent-graph-local.ts";
import type { CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts";
import {
  workbenchItemSignature,
  workbenchPlanItemFromEntries,
  type WorkbenchCollabAgentStatus,
  type WorkbenchItem,
} from "../workbench-item-contract.ts";
import type { DesktopNoteEntry } from "./shell.ts";
import { parseChatMarkdown } from "./markdown.ts";
import { childInterruptable } from "./runtime-cards.ts";
import { humanizeToolInvocation, projectToolCardEntries } from "./tool-cards.ts";

const terminalStatuses = new Set([
  "canceled",
  "cancelled",
  "completed",
  "errored",
  "failed",
  "interrupted",
  "shutdown",
  "task_complete",
  "task_completed",
  "turn_aborted",
  "turn_canceled",
  "turn_cancelled",
  "turn_complete",
  "turn_completed",
  "turn_failed",
  "turn_interrupted",
]);

const shortTimestampFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
});
const longTimestampFormatter = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "long",
  year: "numeric",
});
const wallClockTimestamp = /^\d{1,2}:\d{2}(?:\s?[ap]m)?$/iu;

export const formatReactTimelineTimestamp = (
  value: string,
): Readonly<{
  short: string;
  tooltip: string;
}> => {
  const timestamp = value.trim();
  if (timestamp === "" || timestamp === "--:--") return { short: "", tooltip: "" };
  if (wallClockTimestamp.test(timestamp)) return { short: timestamp, tooltip: timestamp };
  const date = new Date(timestamp);
  if (!Number.isFinite(date.getTime()) || date.getTime() === 0) return { short: "", tooltip: "" };
  return {
    short: shortTimestampFormatter.format(date),
    tooltip: `${shortTimestampFormatter.format(date)}, ${longTimestampFormatter.format(date)}`,
  };
};

const field = (item: CodexHistoryItem, name: string): string | null =>
  item.fields.find((entry) => entry.label.toLocaleLowerCase() === name)?.value ?? null;

const normalizedStatus = (item: CodexHistoryItem): string =>
  (item.status ?? item.label)
    .trim()
    .toLocaleLowerCase()
    .replaceAll(/[ .-]+/g, "_");

const isTerminal = (item: CodexHistoryItem): boolean =>
  item.kind === "lifecycle" && terminalStatuses.has(normalizedStatus(item));

export type ReactTimelineRecord = Readonly<{
  key: string;
  itemRef: string;
  sequence: number;
  kind: CodexHistoryItem["kind"] | "local_message" | "question";
  label: string;
  body: string;
  timestamp: string;
  status: string | null;
  redacted: boolean;
  fields: CodexHistoryItem["fields"];
  resultRef: string | null;
  resultBody: string | null;
  resultStatus: string | null;
  /** Terminal assistant response in its user-turn segment; commentary stays visually quiet. */
  showAssistantMeta?: boolean;
  /**
   * Typed item payload (#8859) when the source note/history row carried one.
   * Wave-2 card lanes render this; the string label/body stay authoritative
   * until then.
   */
  item?: WorkbenchItem;
  /**
   * Desktop-only interrupt wiring for a live delegate-child runtime note
   * (#8867 T10). NOT part of the shared `WorkbenchItem` contract — the
   * Interrupt affordance is a desktop-local capability (`runtime-cards.ts`'s
   * `childInterruptable`, the same predicate the compatibility renderer
   * uses), never assumed by the web host that also consumes `dispatchWorkbenchItem`.
   */
  runtimeChild?: Readonly<{ turnRef: string; childRef: string; interruptable: boolean }>;
}>;

const recordFromItem = (
  item: CodexHistoryItem,
  result: CodexHistoryItem | undefined,
): ReactTimelineRecord => {
  const args = item.kind === "tool_call" ? item.summary || field(item, "input") || "" : "";
  const humanized = item.kind === "tool_call" ? humanizeToolInvocation(item.label, args) : null;
  const typedItem = result?.item ?? item.item;
  return {
    key: item.itemRef,
    itemRef: item.itemRef,
    sequence: item.sequence,
    kind: item.kind,
    label: humanized?.title ?? item.label,
    body: humanized?.detail || item.summary || "No display text",
    timestamp: item.timestamp,
    status: result?.status ?? item.status,
    redacted: item.redacted || result?.redacted === true,
    fields: item.fields,
    resultRef: result?.itemRef ?? null,
    resultBody: result?.summary ?? null,
    resultStatus: result?.status ?? null,
    ...(typedItem === undefined ? {} : { item: typedItem }),
  };
};

/** Pure presentation over the bounded history contract; no provider parsing. */
export const projectReactTimelineRecords = (
  source: ReadonlyArray<CodexHistoryItem>,
): ReadonlyArray<ReactTimelineRecord> => {
  const byRef = new Map<string, CodexHistoryItem>();
  for (const item of source) byRef.set(item.itemRef, item);
  const items = [...byRef.values()].sort(
    (left, right) => left.sequence - right.sequence || left.itemRef.localeCompare(right.itemRef),
  );
  const newestTerminal = items.filter(isTerminal).at(-1)?.itemRef ?? null;
  const resultByCall = new Map<string, CodexHistoryItem>();
  for (const item of items) {
    if (item.kind !== "tool_result") continue;
    const callRef = field(item, "call");
    if (callRef !== null) resultByCall.set(callRef, item);
  }
  const consumed = new Set<string>();
  const records: Array<ReactTimelineRecord> = [];
  for (const item of items) {
    // Transport/accounting scaffolding stays available in the bounded history
    // inspector; it is not a primary conversation row.
    if (["session", "context", "metadata", "usage"].includes(item.kind)) continue;
    // Persisted reasoning can intentionally be only a redaction marker. Absence
    // is the honest primary presentation, not a false failure card.
    if (item.kind === "reasoning" && item.redacted) continue;
    if (isTerminal(item) && item.itemRef !== newestTerminal) continue;
    if (item.kind === "tool_call") {
      const callRef = field(item, "call");
      const result = callRef === null ? undefined : resultByCall.get(callRef);
      if (result !== undefined) consumed.add(result.itemRef);
      records.push(recordFromItem(item, result));
      continue;
    }
    if (item.kind === "tool_result" && consumed.has(item.itemRef)) continue;
    records.push(recordFromItem(item, undefined));
  }
  return records;
};

export const projectLocalTimelineRecords = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<ReactTimelineRecord> =>
  projectToolCardEntries(notes).flatMap((entry, index): ReadonlyArray<ReactTimelineRecord> => {
    if (entry.kind === "tool") {
      const humanized = humanizeToolInvocation(entry.card.toolName, entry.card.argsSummary);
      return [
        {
          key: entry.card.key,
          itemRef: entry.card.key,
          sequence: index,
          kind: "tool_call" as const,
          label: humanized.title,
          body: humanized.detail || entry.card.argsSummary || entry.card.toolName,
          timestamp: entry.card.timestamp,
          status: entry.card.status === "ok" ? "completed" : entry.card.status,
          redacted: false,
          fields: [],
          resultRef: entry.card.resultSummary === null ? null : `${entry.card.key}:result`,
          resultBody: entry.card.resultSummary,
          resultStatus:
            entry.card.status === "failed"
              ? "failed"
              : entry.card.status === "ok"
                ? "completed"
                : null,
          ...(entry.card.item === undefined ? {} : { item: entry.card.item }),
        },
      ];
    }
    // Delegate-child lifecycle (collabAgentToolCall/subAgentActivity, projected
    // by local-harness.ts as a `runtime: {kind:"child"}` note): route through
    // the SAME typed "agent" WorkbenchItem the DesktopAgentGroup card renders
    // (#8867 T10) instead of falling through to a flat system-notice line.
    // `queue` runtime notes are untouched (unchanged generic fallthrough
    // below); `plan` runtime notes get their own typed-item branch further
    // down (T8 #8865).
    if (entry.kind === "runtime" && entry.note.runtime?.kind === "child") {
      const runtime = entry.note.runtime;
      const coarseStatus =
        runtime.status === "running"
          ? ("in_progress" as const)
          : runtime.status === "completed"
            ? ("completed" as const)
            : ("failed" as const);
      const childCollabStatus =
        runtime.status === "running"
          ? ("running" as const)
          : runtime.status === "completed"
            ? ("completed" as const)
            : ("errored" as const);
      return [
        {
          key: entry.note.key,
          itemRef: entry.note.key,
          sequence: index,
          kind: "collaboration" as const,
          label: "Delegated agent",
          body: runtime.detail || runtime.title,
          timestamp: entry.note.timestamp,
          status: runtime.status,
          redacted: false,
          fields: [],
          resultRef: null,
          resultBody: null,
          resultStatus: null,
          item: {
            kind: "agent",
            // Delegate-child events ride the codex-app-server collab wire today
            // (`collabAgentToolCall`/`subAgentActivity`); revisit if another
            // harness starts emitting `child_*` ClaudeLocalEvents.
            source: "codex",
            status: coarseStatus,
            children: [
              {
                threadRef: runtime.childRef,
                status: childCollabStatus,
                ...(runtime.title === "" ? {} : { nickname: runtime.title }),
                // Show the subagent's detail (its objective while running, its
                // bounded failure reason once errored) inline on the card, so an
                // errored delegate reads "ERRORED — <reason>" without a click.
                ...(runtime.detail === "" ? {} : { detail: runtime.detail }),
              },
            ],
          },
          runtimeChild: {
            turnRef: runtime.turnRef,
            childRef: runtime.childRef,
            interruptable: childInterruptable(runtime),
          },
        },
      ];
    }
    const note = entry.note;
    if (note.role === "system" && /^(Usage|Connected)\s*·/i.test(note.text)) return [];
    // T8 (#8865): a live plan_updated note carries its full typed payload on
    // `note.runtime` (entries + optional prose) — read it DIRECTLY instead of
    // pattern-matching `note.text` (which is always the literal "Plan updated"
    // and never matched the old `/^Plan\s*·/` check, so this card silently
    // degraded to a generic system-message notice on this surface). The typed
    // `item` here is what makes `TimelineItem` dispatch through the SAME
    // `DesktopPlanCard` history rows and `turn/plan/updated` already use.
    if (note.runtime?.kind === "plan") {
      const runtimePlan = note.runtime;
      return [
        {
          key: note.key,
          itemRef: note.key,
          sequence: index,
          kind: "plan" as const,
          label: "Plan",
          body: runtimePlan.prose ?? note.text,
          timestamp: note.timestamp,
          status: null,
          redacted: false,
          fields: [],
          resultRef: null,
          resultBody: null,
          resultStatus: null,
          item: workbenchPlanItemFromEntries({
            source: "local",
            entries: runtimePlan.entries,
            ...(runtimePlan.prose === undefined ? {} : { prose: runtimePlan.prose }),
          }),
        },
      ];
    }
    const kind: ReactTimelineRecord["kind"] =
      note.question !== undefined
        ? "question"
        : note.role !== "system"
          ? "local_message"
          : /^Reasoning\s*·/i.test(note.text)
            ? "reasoning"
            : /^Approval\s*·/i.test(note.text)
              ? "approval"
              : /^Spec revalidation\s*·/i.test(note.text)
                ? "system_message"
                : /^Turn (completed|complete|canceled|cancelled)$/i.test(note.text)
                  ? "lifecycle"
                  : /^Turn (failed|interrupted)|error/i.test(note.text)
                    ? "error"
                    : /\s·\s(?:running|completed|failed|errored)$/i.test(note.text)
                      ? "tool_call"
                      : "system_message";
    const label =
      note.question !== undefined
        ? note.question.kind === "tool_approval"
          ? "Tool approval"
          : note.question.kind === "plan_review"
            ? "Plan review"
            : "Question"
        : note.role === "user"
          ? "You"
          : note.role === "assistant"
            ? "Assistant"
            : kind === "tool_call"
              ? note.text.split(" · ")[0] || "Tool"
              : kind === "reasoning"
                ? "Reasoning"
                : "System";
    const body =
      note.question?.questions[0]?.question ??
      note.text.replace(/^(Reasoning|Approval)\s*·\s*/i, "");
    return [
      {
        key: note.key,
        itemRef: note.key,
        sequence: index,
        kind,
        label,
        body,
        timestamp: note.timestamp,
        status: note.question?.status ?? note.runtime?.kind ?? null,
        redacted: false,
        fields: [],
        resultRef: null,
        resultBody: null,
        resultStatus: null,
        // Older persisted local/Claude notes predate the typed reasoning payload.
        // The existing bounded legacy classifier above has already selected the
        // semantic route; adapt that body into the same typed presentation used by
        // current live and history records so restarts do not restore old chrome.
        ...(kind === "reasoning"
          ? {
              item: {
                kind: "reasoning" as const,
                source: "local" as const,
                summary: body,
                status: "completed" as const,
              },
            }
          : {}),
      },
    ];
  });

const Inline = ({ nodes }: { readonly nodes: ReadonlyArray<MarkdownInline> }): ReactNode =>
  nodes.map((node, index) => {
    if (node.kind === "text") return <span key={index}>{node.text}</span>;
    if (node.kind === "code") return <code key={index}>{node.text}</code>;
    if (node.kind === "strong")
      return (
        <strong key={index}>
          <Inline nodes={node.children} />
        </strong>
      );
    if (node.kind === "emphasis")
      return (
        <em key={index}>
          <Inline nodes={node.children} />
        </em>
      );
    return null;
  });

const Blocks = ({ blocks }: { readonly blocks: ReadonlyArray<MarkdownBlock> }): ReactNode =>
  blocks.map((block, index) => {
    if (block.kind === "paragraph")
      return (
        <p key={index}>
          <Inline nodes={block.children} />
        </p>
      );
    if (block.kind === "heading")
      return createElement(`h${block.level}`, { key: index }, <Inline nodes={block.children} />);
    if (block.kind === "blockquote")
      return (
        <blockquote key={index}>
          <Blocks blocks={block.children} />
        </blockquote>
      );
    if (block.kind === "list") {
      const Tag = block.ordered ? "ol" : "ul";
      return (
        <Tag key={index}>
          {block.items.map((item, itemIndex) => (
            <li key={itemIndex}>
              <Blocks blocks={item} />
            </li>
          ))}
        </Tag>
      );
    }
    return null;
  });

export const SafeReactMarkdown = memo(({ value }: { readonly value: string }): ReactElement => {
  const segments = useMemo(() => parseChatMarkdown(value), [value]);
  return (
    <div className="oa-react-markdown">
      {segments.map((segment, index) =>
        segment.kind === "markdown" ? (
          <Blocks key={index} blocks={segment.blocks} />
        ) : segment.kind === "code" ? (
          <pre key={index}>
            <code data-language={segment.language}>{segment.code}</code>
          </pre>
        ) : (
          <hr key={index} />
        ),
      )}
    </div>
  );
});

const isUserRecord = (record: ReactTimelineRecord): boolean =>
  record.kind === "user_message" || record.label === "You";

const isMessageRecord = (record: ReactTimelineRecord): boolean =>
  isUserRecord(record) ||
  ["assistant_message", "agent_message", "local_message"].includes(record.kind);

const isWorkRecord = (record: ReactTimelineRecord): boolean =>
  ["reasoning", "tool_call", "tool_result", "approval", "collaboration"].includes(record.kind);

// Reasoning is authored transcript content, even after its streaming lifecycle
// completes. Keep it on the primary timeline instead of letting the settled
// work disclosure absorb it alongside commands and tool activity.
const isFoldableWorkRecord = (record: ReactTimelineRecord): boolean =>
  isWorkRecord(record) && record.kind !== "reasoning";

/**
 * `WorkbenchItem` kinds (#8859) rendered through `dispatchWorkbenchItem`
 * (`dispatch.tsx`, #8860) instead of a bespoke branch below. Most of these
 * still resolve to the generic `DesktopWorkEntry`/`DesktopToolCallCard` shell
 * until their own Wave-2 lane (T4-T12, epic #8857) lands its polished card —
 * a no-op today, but it means that lane ships by editing ONLY its own
 * `dispatch.tsx` branch, with zero further changes here.
 *
 * `plan` (T8 #8865) is the first Wave-2 kind to graduate: every plan source
 * (live `turn/plan/updated` / the `plan` ThreadItem, and history
 * `plan`/`todo_list` rows) now projects into one typed `WorkbenchPlanItem`
 * carried on `record.item`, so it dispatches here through the SAME real
 * `DesktopPlanCard` instead of the bespoke single-entry reconstruction below
 * (kept only as a fallback for a record whose source carried neither
 * structured entries nor prose). `notice` joined this set in T12 (#8869): a
 * typed `notice` item now carries `severity`, which only
 * `dispatchWorkbenchItem`'s restyled `DesktopTimelineNotice` call honors —
 * the generic string-kind branch below has no severity to read. `message`
 * and `agent` keep their existing bespoke branches below unchanged (not part
 * of Wave 2's scope).
 */
const dispatchableWorkbenchKinds: ReadonlySet<WorkbenchItem["kind"]> = new Set([
  "command",
  "fileChange",
  "toolCall",
  "reasoning",
  "approval",
  "meter",
  "compaction",
  "sleep",
  "review",
  "hook",
  "plan",
  "notice",
]);

const compact = (value: string, limit = 180): string => {
  const normalized = value.replaceAll("\\n", " ").replaceAll(/\s+/g, " ").trim();
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit - 1)}…`;
};

// ---------------------------------------------------------------------------
// Agent status/operation formatting (#8867 T10). Mirrors
// `packages/ui/src/workbench/dispatch.tsx`'s private helpers of the same
// name for the shared `dispatchWorkbenchItem` "agent" branch; kept local
// here because this file's history-branch enrichment reads the raw
// snake_case `operation` string codex-history.ts already captures
// (`spawn_agent`/`send_input`/...), a different vocabulary than the live
// camelCase `collabAgentToolCall.tool`.
// ---------------------------------------------------------------------------

/** collabAgentToolCall.tool -> the short operation verb DesktopAgentGroup brackets as [SPAWN]/[SEND]/etc. */
const agentOperationTag = (tool: string): string => {
  switch (tool) {
    case "spawnAgent":
      return "spawn";
    case "sendInput":
      return "send";
    case "resumeAgent":
      return "resume";
    case "wait":
      return "wait";
    case "closeAgent":
      return "close";
    default:
      return tool;
  }
};

/** codex-history.ts's raw snake_case `operation` field -> the same short verb vocabulary. */
const historyOperationTag = (operation: string): string => {
  switch (operation) {
    case "spawn_agent":
      return "spawn";
    case "send_input":
      return "send";
    case "resume_agent":
      return "resume";
    case "wait":
      return "wait";
    case "close_agent":
      return "close";
    case "interrupt_agent":
      return "interrupt";
    default:
      return operation.replaceAll("_", " ");
  }
};

/** CollabAgentStatus -> the coarse icon/tone bucket DesktopAgentRow understands. */
const toDesktopAgentStatusFromCollab = (status: WorkbenchCollabAgentStatus): DesktopAgentStatus => {
  switch (status) {
    case "running":
      return "running";
    case "completed":
      return "completed";
    case "pendingInit":
      return "waiting";
    case "interrupted":
    case "errored":
    case "shutdown":
    case "notFound":
      return "failed";
  }
};

/** "pendingInit" -> "PENDING INIT"; "notFound" -> "NOT FOUND"; the rest just uppercase. */
const collabStatusLabel = (status: WorkbenchCollabAgentStatus): string =>
  status.replace(/([a-z])([A-Z])/g, "$1 $2").toUpperCase();

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(
    report(
      payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()),
      payload,
    ) as Effect.Effect<void, IntentError>,
  ).catch(() => {});
};

const MESSAGE_COPY_FEEDBACK_MS = 1000;
const MAX_COLLAPSED_USER_MESSAGE_LINES = 8;
const MAX_COLLAPSED_USER_MESSAGE_LENGTH = 600;

/** T3 parity rule: long prompts stay scannable without hiding ordinary messages. */
export const shouldCollapseUserMessage = (text: string): boolean => {
  if (text.trim() === "") return false;
  return (
    text.length > MAX_COLLAPSED_USER_MESSAGE_LENGTH ||
    text.split("\n").length > MAX_COLLAPSED_USER_MESSAGE_LINES
  );
};

/**
 * Expanding transcript details above a reader's viewport must not move the
 * content they were reading. At the live edge the message-scroller remains
 * the sole follow authority; away from it we keep the toggled row's bottom
 * at the same visual position, matching T3's work-fold behavior.
 */
const togglePreservingReaderPosition = (row: HTMLElement | null, update: () => void): void => {
  const viewport = row?.closest<HTMLElement>('[data-slot="message-scroller-viewport"]') ?? null;
  if (row === null || viewport === null) {
    update();
    return;
  }
  const wasAtEnd = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= 1;
  const bottomBefore = row.getBoundingClientRect().bottom;
  update();
  if (wasAtEnd) return;
  row.ownerDocument.defaultView?.setTimeout(() => {
    const delta = row.getBoundingClientRect().bottom - bottomBefore;
    if (Number.isFinite(delta) && Math.abs(delta) >= 0.5) viewport.scrollTop += delta;
  }, 0);
};

/**
 * Copy `text` to the clipboard, tolerating renderers where the async Clipboard
 * API is unavailable or denied. Electron renderers loaded from a non-secure
 * custom protocol can leave `navigator.clipboard` undefined or reject
 * `writeText`, so we fall back to a hidden textarea plus `execCommand("copy")`
 * inside the click's user-gesture. Resolves to whether the copy succeeded.
 */
const copyMessageTextToClipboard = async (text: string): Promise<boolean> => {
  try {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText !== undefined) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to the execCommand path; a swallowed async-API failure must
    // not stop the synchronous fallback below.
  }
  if (typeof document === "undefined") return false;
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);
  try {
    textarea.focus();
    textarea.select();
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    document.body.removeChild(textarea);
  }
};

const MessageCopyButton = memo(({ text }: Readonly<{ text: string }>): ReactElement => {
  const [isCopied, setIsCopied] = useState(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (resetTimer.current !== null) clearTimeout(resetTimer.current);
    },
    [],
  );

  const copyToClipboard = (): void => {
    void copyMessageTextToClipboard(text).then((copied) => {
      if (!copied) return;
      setIsCopied(true);
      if (resetTimer.current !== null) clearTimeout(resetTimer.current);
      resetTimer.current = setTimeout(() => setIsCopied(false), MESSAGE_COPY_FEEDBACK_MS);
    });
  };

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              aria-label="Copy message"
              disabled={isCopied}
              onClick={copyToClipboard}
              type="button"
              size="icon-xs"
              variant="ghost"
              className="text-muted-foreground hover:text-foreground"
            />
          }
        >
          {isCopied ? (
            <CheckIcon className="size-3 text-primary" />
          ) : (
            <CopyIcon className="size-3" />
          )}
        </TooltipTrigger>
        <TooltipContent>
          <p>Copy to clipboard</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
});

/**
 * T3 Code's exact user-row composition (bubble followed by a hover/focus-only
 * metadata row), ported from apps/web/src/components/chat/MessagesTimeline.tsx
 * at fdca15471d92e95e4ec5501f45dbf3ce81f8d991. The one visual substitution is
 * the owner-requested OpenAgents blue surface token.
 */
const UserTimelineRow = ({
  record,
  report,
}: Readonly<{
  record: ReactTimelineRecord;
  report: IntentReporter;
}>): ReactElement => {
  const timestamp = formatReactTimelineTimestamp(record.timestamp);
  const [expanded, setExpanded] = useState(false);
  const rowRef = useRef<HTMLElement>(null);
  const collapsible = shouldCollapseUserMessage(record.body);
  const collapsed = collapsible && !expanded;
  return (
    <article
      ref={rowRef}
      aria-label={`${record.label}. Item ${record.sequence + 1}`}
      className="oa-react-timeline-item oa-react-user-message-row"
      data-kind={record.kind}
      data-timeline-key={record.key}
      data-tone="user"
      role="listitem"
    >
      <div data-slot="user-message-bubble" className="oa-react-user-message-bubble">
        <div
          className="oa-react-user-message-body"
          data-user-message-collapsible={collapsible ? "true" : "false"}
          data-user-message-collapsed={collapsed ? "true" : "false"}
        >
          <SafeReactMarkdown value={record.body} />
        </div>
        {collapsible ? (
          <Button
            className="oa-react-user-message-toggle"
            type="button"
            variant="ghost"
            size="xs"
            aria-expanded={expanded}
            onClick={() =>
              togglePreservingReaderPosition(rowRef.current, () => setExpanded((value) => !value))
            }
          >
            {expanded ? "Show less" : "Show full message"}
          </Button>
        ) : null}
      </div>
      <div data-slot="user-message-actions" className="oa-react-user-message-actions">
        <div className="flex shrink-0 items-center gap-2">
          {timestamp.short === "" ? null : (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger
                  render={<p className="text-muted-foreground text-xs tabular-nums" />}
                >
                  {timestamp.short}
                </TooltipTrigger>
                <TooltipContent>{timestamp.tooltip}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          <div className="flex items-center gap-0.5">
            {record.kind === "local_message" || record.kind === "question" ? null : (
              <Button
                type="button"
                variant="ghost"
                size="xs"
                onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}
                aria-label={`Show details for ${record.label}, item ${record.sequence + 1}`}
              >
                Details
              </Button>
            )}
            <MessageCopyButton text={record.body} />
          </div>
        </div>
      </div>
    </article>
  );
};

export const TimelineItem = ({
  record,
  report,
}: {
  readonly record: ReactTimelineRecord;
  readonly report: IntentReporter;
}): ReactElement => {
  if (
    record.kind === "lifecycle" &&
    !["failed", "errored", "interrupted"].includes(record.status ?? "")
  ) {
    return <span data-timeline-key={record.key} data-kind="lifecycle" hidden />;
  }
  if (record.item !== undefined && dispatchableWorkbenchKinds.has(record.item.kind)) {
    return dispatchWorkbenchItem(record.item as WorkbenchDispatchItem, {
      itemKey: record.key,
      renderMarkdown: (value) => <SafeReactMarkdown value={value} />,
      sequence: record.sequence,
    });
  }
  // Typed live delegate-child status (#8867 T10): a per-agent status row
  // built from collabAgentToolCall/subAgentActivity (via
  // `projectLocalTimelineRecords`'s runtime-child branch above). Placed
  // BEFORE the history-only `collaboration` string branch below so a record
  // that carries both (never true today, but future-proof) prefers the typed
  // path.
  if (record.item?.kind === "agent") {
    const agentItem = record.item;
    const operation = agentItem.tool === undefined ? undefined : agentOperationTag(agentItem.tool);
    const agents: ReadonlyArray<DesktopAgentActivity> =
      agentItem.children !== undefined && agentItem.children.length > 0
        ? agentItem.children.map((child) => ({
            agentKey: child.threadRef,
            detail: child.detail ?? (agentItem.activityKind !== undefined ? "" : (agentItem.prompt ?? "")),
            name: child.nickname ?? agentItem.agentPath ?? child.threadRef,
            role: agentItem.activityKind !== undefined ? "Subagent activity" : "Delegated agent",
            status: toDesktopAgentStatusFromCollab(child.status),
            statusLabel: collabStatusLabel(child.status),
            ...(agentItem.agentPath === undefined ? {} : { path: agentItem.agentPath }),
            ...(agentItem.activityKind === undefined
              ? {}
              : { activityKind: agentItem.activityKind }),
            ...(record.runtimeChild === undefined
              ? {}
              : {
                  interruptable: record.runtimeChild.interruptable,
                  onInterrupt: () =>
                    dispatch(report, "DesktopChildInterruptRequested", {
                      turnRef: record.runtimeChild!.turnRef,
                      childRef: record.runtimeChild!.childRef,
                    }),
                  // Clicking the card opens the subagent's message chain in the
                  // right pane (AFS-04 follow-up): the same inspect_agent intent the
                  // Foldkit child card dispatches, keyed by the canonical delegate ref.
                  onInspect: () =>
                    dispatch(report, "DesktopAgentAction", {
                      kind: "inspect_agent",
                      agentRef: localDelegateAgentRef(
                        record.runtimeChild!.turnRef,
                        record.runtimeChild!.childRef,
                      ),
                    }),
                }),
          }))
        : [
            {
              agentKey: record.key,
              detail: agentItem.prompt ?? "",
              name: agentItem.tool ?? "Agent",
              role: "agent",
              status:
                agentItem.status === "in_progress"
                  ? "running"
                  : agentItem.status === "completed"
                    ? "completed"
                    : "failed",
            },
          ];
    return (
      <DesktopAgentGroup
        agents={agents}
        itemKey={record.key}
        operation={operation}
        prompt={agentItem.prompt}
      />
    );
  }
  if (record.kind === "collaboration") {
    const status = ["failed", "errored", "interrupted"].includes(record.status ?? "")
      ? "failed"
      : record.status === "running" || record.status === "in_progress"
        ? "running"
        : "completed";
    const agentRef =
      record.fields.find((entry) => entry.label.toLocaleLowerCase() === "agent")?.value ??
      record.itemRef;
    // Nice-to-have enrichment (#8867 T10 §4): codex-history.ts already
    // captures the raw `operation`/`activity` fields on collab-like rows
    // (see `projectRow`'s collab branch); surface them the same way the live
    // typed path does, without a deeper history-graph restructure.
    const historyOperation = record.fields.find(
      (entry) => entry.label.toLocaleLowerCase() === "operation",
    )?.value;
    const historyActivity = record.fields.find(
      (entry) => entry.label.toLocaleLowerCase() === "activity",
    )?.value;
    const activityKind =
      historyActivity === "started" ||
      historyActivity === "interacted" ||
      historyActivity === "interrupted"
        ? historyActivity
        : undefined;
    return (
      <DesktopAgentGroup
        agents={[
          {
            agentKey: agentRef,
            detail: record.body,
            name: record.label,
            role: "Delegated agent",
            status,
            transcript: [{ label: "Activity", text: record.body }],
            ...(activityKind === undefined ? {} : { activityKind }),
          },
        ]}
        itemKey={record.key}
        operation={
          historyOperation === undefined ? undefined : historyOperationTag(historyOperation)
        }
      />
    );
  }
  if (isWorkRecord(record))
    return (
      <DesktopWorkEntry
        body={
          <>
            {record.redacted ? (
              <p>Details unavailable.</p>
            ) : (
              <pre>
                <code>{record.body}</code>
              </pre>
            )}
            {record.resultBody === null ? null : (
              <>
                <strong>{record.resultStatus === "failed" ? "Result · failed" : "Result"}</strong>
                <pre>
                  <code>{record.resultBody}</code>
                </pre>
              </>
            )}
            <Button
              className="oa-react-item-details"
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}
            >
              Inspect event
            </Button>
          </>
        }
        itemKey={record.key}
        kind={record.kind}
        label={record.label}
        preview={compact(record.body || record.resultBody || record.label)}
        status={record.status ?? "completed"}
        statusLabel={
          ["failed", "errored", "interrupted"].includes(record.status ?? "")
            ? "Failed"
            : record.status === "running"
              ? "Running"
              : "Done"
        }
      />
    );

  // Fallback only: a "plan" record always carries a typed `item` today (the
  // live-note and history projectors above attach one whenever there is any
  // real content), so `dispatchableWorkbenchKinds` already handled it. This
  // stays for the edge case of a genuinely empty/untyped plan row.
  if (record.kind === "plan")
    return (
      <DesktopPlanCard
        entries={[
          {
            step: record.body,
            status:
              record.status === "completed"
                ? "completed"
                : record.status === "running" || record.status === "in_progress"
                  ? "in_progress"
                  : "pending",
          },
        ]}
        itemKey={record.key}
      />
    );

  const danger =
    record.kind === "error" ||
    record.kind === "gap" ||
    ["failed", "errored", "interrupted"].includes(record.status ?? "");
  if (!isMessageRecord(record) || danger || record.redacted)
    return (
      <DesktopTimelineNotice
        body={record.redacted ? "Message content unavailable." : record.body}
        danger={danger}
        itemKey={record.key}
        kind={record.kind}
        label={danger ? record.label : "Update"}
      />
    );

  if (isUserRecord(record)) return <UserTimelineRow record={record} report={report} />;

  const timestamp = formatReactTimelineTimestamp(record.timestamp);
  const streaming = record.status === "running" || record.status === "in_progress";
  return (
    <DesktopTimelineMessage
      itemKey={record.key}
      kind={record.kind}
      label={record.label}
      sequence={record.sequence}
      tone="assistant"
    >
      <div className="oa-react-assistant-message-body">
        <SafeReactMarkdown value={record.body} />
      </div>
      {record.showAssistantMeta === true ? (
        <div className="oa-react-assistant-message-actions" data-slot="assistant-message-actions">
          {!streaming && record.body.trim() !== "" ? (
            <MessageCopyButton text={record.body} />
          ) : null}
          {timestamp.short === "" || streaming ? null : (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger
                  render={<p className="text-muted-foreground text-xs tabular-nums" />}
                >
                  {timestamp.short}
                </TooltipTrigger>
                <TooltipContent>{timestamp.tooltip}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          {record.kind === "local_message" || record.kind === "question" ? null : (
            <Button
              className="oa-react-item-details"
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}
              aria-label={`Show details for ${record.label}, item ${record.sequence + 1}`}
            >
              Details
            </Button>
          )}
        </div>
      ) : null}
    </DesktopTimelineMessage>
  );
};

class TimelineItemBoundary extends Component<
  Readonly<{
    record: ReactTimelineRecord;
    report: IntentReporter;
  }>,
  Readonly<{ failed: boolean }>
> {
  state = { failed: false };
  static getDerivedStateFromError(): Readonly<{ failed: boolean }> {
    return { failed: true };
  }
  render(): ReactElement {
    if (this.state.failed)
      return (
        <article
          className="oa-react-timeline-item"
          data-timeline-key={this.props.record.key}
          data-kind="presentation_error"
          data-tone="danger"
          role="listitem"
          aria-label={`Item ${this.props.record.sequence + 1} unavailable`}
        >
          <header>
            <strong>Item unavailable</strong>
            <span>Presentation error</span>
          </header>
          <p>This item could not be displayed. No completion state was inferred.</p>
        </article>
      );
    return <TimelineItem record={this.props.record} report={this.props.report} />;
  }
}

const sameTimelineRecord = (left: ReactTimelineRecord, right: ReactTimelineRecord): boolean =>
  left.key === right.key &&
  left.itemRef === right.itemRef &&
  left.sequence === right.sequence &&
  left.kind === right.kind &&
  left.label === right.label &&
  left.body === right.body &&
  left.timestamp === right.timestamp &&
  left.status === right.status &&
  left.redacted === right.redacted &&
  left.resultRef === right.resultRef &&
  left.resultBody === right.resultBody &&
  left.resultStatus === right.resultStatus &&
  left.showAssistantMeta === right.showAssistantMeta &&
  // Scalar signature comparison — content changes flip it without
  // stringifying multi-kilobyte diffs on every memo check (#8859).
  workbenchItemSignature(left.item) === workbenchItemSignature(right.item) &&
  // Interrupt eligibility (#8867 T10) can flip (child_steered) without any
  // other field moving; compare it explicitly so the button disappears the
  // instant the runtime marks the child no-longer-interruptable.
  left.runtimeChild?.interruptable === right.runtimeChild?.interruptable &&
  left.runtimeChild?.turnRef === right.runtimeChild?.turnRef &&
  left.runtimeChild?.childRef === right.runtimeChild?.childRef &&
  left.fields.length === right.fields.length &&
  left.fields.every((field, index) => {
    const candidate = right.fields[index];
    return (
      candidate !== undefined && field.label === candidate.label && field.value === candidate.value
    );
  });

const MemoTimelineItemBoundary = memo(
  TimelineItemBoundary,
  (left, right) => left.report === right.report && sameTimelineRecord(left.record, right.record),
);

type TimelineProps = Readonly<{
  sessionKey: string;
  records: ReadonlyArray<ReactTimelineRecord>;
  loadedItemCount: number;
  offset: number;
  totalItems: number;
  loadingEdge: "top" | "bottom" | null;
  working?: boolean;
  waitingForAnswer?: boolean;
  agentName?: string;
  report: IntentReporter;
}>;

export type ReactTimelineTurn = Readonly<{
  id: string;
  userPreview: string;
  assistantPreview: string | null;
}>;

export type ReactTimelineTurnFold = Readonly<{
  id: string;
  turnId: string;
  label: string;
  expanded: boolean;
  hiddenCount: number;
}>;

type TimelineDisplayRow =
  | Readonly<{ kind: "record"; id: string; record: ReactTimelineRecord }>
  | Readonly<{ kind: "work-group"; id: string; records: ReadonlyArray<ReactTimelineRecord> }>
  | Readonly<{ kind: "turn-fold"; id: string; fold: ReactTimelineTurnFold }>;

const parseTimelineTimestamp = (value: string): number | null => {
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : null;
};

/** T3's compact elapsed-time vocabulary, kept stable for fold labels. */
export const formatReactTimelineDuration = (durationMs: number): string => {
  if (!Number.isFinite(durationMs) || durationMs < 0) return "0ms";
  if (durationMs < 1_000) return `${Math.max(1, Math.round(durationMs))}ms`;
  if (durationMs < 10_000) return `${Math.round(durationMs / 100) / 10}s`;
  if (durationMs < 60_000) return `${Math.round(durationMs / 1_000)}s`;
  const minutes = Math.floor(durationMs / 60_000);
  const seconds = Math.round((durationMs % 60_000) / 1_000);
  return seconds === 60
    ? `${minutes + 1}m`
    : seconds === 0
      ? `${minutes}m`
      : `${minutes}m ${seconds}s`;
};

const turnFoldLabel = (records: ReadonlyArray<ReactTimelineRecord>): string => {
  const times = records.flatMap((record) => {
    const timestamp = parseTimelineTimestamp(record.timestamp);
    return timestamp === null ? [] : [timestamp];
  });
  const duration =
    times.length < 2 ? null : formatReactTimelineDuration(Math.max(...times) - Math.min(...times));
  const stopped = records.some((record) =>
    ["canceled", "cancelled", "interrupted", "turn_interrupted"].includes(record.status ?? ""),
  );
  if (stopped)
    return duration === null ? "You stopped this response" : `You stopped after ${duration}`;
  return duration === null ? "Worked" : `Worked for ${duration}`;
};

const groupDisplayWorkRows = (
  rows: ReadonlyArray<TimelineDisplayRow>,
): ReadonlyArray<TimelineDisplayRow> => {
  const result: TimelineDisplayRow[] = [];
  for (let index = 0; index < rows.length; ) {
    const row = rows[index]!;
    if (row.kind !== "record" || !isFoldableWorkRecord(row.record)) {
      result.push(row);
      index += 1;
      continue;
    }
    const records: ReactTimelineRecord[] = [];
    while (index < rows.length) {
      const candidate = rows[index]!;
      if (candidate.kind !== "record" || !isFoldableWorkRecord(candidate.record)) break;
      records.push(candidate.record);
      index += 1;
    }
    result.push(
      records.length === 1
        ? { kind: "record", id: records[0]!.key, record: records[0]! }
        : { kind: "work-group", id: `work:${records[0]!.key}`, records },
    );
  }
  return result;
};

const isTurnFoldableActivity = (record: ReactTimelineRecord): boolean =>
  (isMessageRecord(record) && !isUserRecord(record)) ||
  ["reasoning", "tool_call", "tool_result", "collaboration"].includes(record.kind);

/**
 * Settled user turns keep the authored prompt and terminal answer visible and
 * fold intermediate commentary/tool activity into one duration row. The live
 * turn never folds. This is presentation-only; every source record remains in
 * the bounded history authority and returns under the same stable key.
 */
export const deriveReactTimelineDisplayRows = (
  records: ReadonlyArray<ReactTimelineRecord>,
  expandedTurnIds: ReadonlySet<string>,
  working: boolean,
): ReadonlyArray<TimelineDisplayRow> => {
  const rows: TimelineDisplayRow[] = [];
  const userIndexes = records.flatMap((record, index) => (isUserRecord(record) ? [index] : []));
  const firstUserIndex = userIndexes[0] ?? records.length;
  for (const record of records.slice(0, firstUserIndex))
    rows.push({ kind: "record", id: record.key, record });

  userIndexes.forEach((start, turnIndex) => {
    const end = userIndexes[turnIndex + 1] ?? records.length;
    const segment = records.slice(start, end);
    const user = segment[0];
    if (user === undefined) return;
    rows.push({ kind: "record", id: user.key, record: user });

    const latestLiveTurn = working && turnIndex === userIndexes.length - 1;
    let terminalIndex = -1;
    for (let index = 1; index < segment.length; index += 1) {
      const record = segment[index]!;
      if (isMessageRecord(record) && !isUserRecord(record) && record.body.trim() !== "")
        terminalIndex = index;
    }
    const hiddenIndexes = new Set(
      segment.flatMap((record, index) =>
        index > 0 && index !== terminalIndex && isTurnFoldableActivity(record) ? [index] : [],
      ),
    );
    const hidden = segment.filter((_, index) => hiddenIndexes.has(index));
    if (latestLiveTurn || hidden.length === 0) {
      for (const record of segment.slice(1)) rows.push({ kind: "record", id: record.key, record });
      return;
    }

    const turnId = user.key;
    const expanded = expandedTurnIds.has(turnId);
    rows.push({
      kind: "turn-fold",
      id: `turn-fold:${turnId}`,
      fold: {
        id: `turn-fold:${turnId}`,
        turnId,
        label: turnFoldLabel(segment),
        expanded,
        hiddenCount: hidden.length,
      },
    });
    for (let index = 1; index < segment.length; index += 1) {
      const record = segment[index]!;
      if (hiddenIndexes.has(index) && !expanded) continue;
      rows.push({ kind: "record", id: record.key, record });
    }
  });
  return groupDisplayWorkRows(rows);
};

const minimapPreview = (value: string): string => {
  const compacted = value.replaceAll(/\s+/gu, " ").trim();
  return compacted.length <= 72 ? compacted : `${compacted.slice(0, 69)}…`;
};

/** One minimap stop per authored user turn, with the terminal answer as context. */
export const deriveReactTimelineTurns = (
  records: ReadonlyArray<ReactTimelineRecord>,
): ReadonlyArray<ReactTimelineTurn> => {
  const turns: ReactTimelineTurn[] = [];
  let active: ReactTimelineTurn | null = null;
  for (const record of records) {
    if (isUserRecord(record)) {
      active = {
        id: record.key,
        userPreview: minimapPreview(record.body) || "User message",
        assistantPreview: null,
      };
      turns.push(active);
      continue;
    }
    if (
      active !== null &&
      isMessageRecord(record) &&
      !isUserRecord(record) &&
      record.body.trim() !== ""
    ) {
      active = { ...active, assistantPreview: minimapPreview(record.body) };
      turns[turns.length - 1] = active;
    }
  }
  return turns;
};

/** T3 shows metadata only on the final assistant chunk before the next user turn. */
export const deriveAssistantMetaKeys = (
  records: ReadonlyArray<ReactTimelineRecord>,
): ReadonlySet<string> => {
  const result = new Set<string>();
  let latestAssistantKey: string | null = null;
  for (const record of records) {
    if (isUserRecord(record)) {
      if (latestAssistantKey !== null) result.add(latestAssistantKey);
      latestAssistantKey = null;
      continue;
    }
    if (isMessageRecord(record)) latestAssistantKey = record.key;
  }
  if (latestAssistantKey !== null) result.add(latestAssistantKey);
  return result;
};

const TimelineMinimap = ({
  turns,
  viewportRef,
  releaseReaderIntent,
  onSelectVirtualTurn,
}: {
  readonly turns: ReadonlyArray<ReactTimelineTurn>;
  readonly viewportRef: RefObject<HTMLDivElement | null>;
  readonly releaseReaderIntent: () => void;
  readonly onSelectVirtualTurn: (turnId: string) => void;
}): ReactElement | null => {
  if (turns.length < 2) return null;
  const select = (turn: ReactTimelineTurn): void => {
    const viewport = viewportRef.current;
    if (viewport === null) return;
    const target = [...viewport.querySelectorAll<HTMLElement>("[data-message-id]")].find(
      (element) => element.dataset.messageId === turn.id,
    );
    releaseReaderIntent();
    if (target === undefined) {
      onSelectVirtualTurn(turn.id);
      return;
    }
    const reducedMotion =
      globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    target.scrollIntoView?.({ block: "start", behavior: reducedMotion ? "auto" : "smooth" });
  };
  return (
    <nav className="oa-react-timeline-minimap" aria-label="Conversation turns">
      <ol>
        {turns.map((turn, index) => (
          <li key={turn.id}>
            <button
              type="button"
              data-in-view="false"
              data-turn-id={turn.id}
              aria-label={`Jump to turn ${index + 1}: ${turn.userPreview}`}
              title={`${turn.userPreview}${turn.assistantPreview === null ? "" : `\n${turn.assistantPreview}`}`}
              onClick={() => select(turn)}
            >
              <span aria-hidden="true" />
            </button>
          </li>
        ))}
      </ol>
    </nav>
  );
};

const TurnFoldDisclosure = ({
  fold,
  onToggle,
}: {
  readonly fold: ReactTimelineTurnFold;
  readonly onToggle: (turnId: string, row: HTMLElement | null) => void;
}): ReactElement => {
  const rowRef = useRef<HTMLDivElement>(null);
  return (
    <div ref={rowRef} className="oa-react-turn-fold" data-turn-fold={fold.turnId}>
      <button
        type="button"
        aria-expanded={fold.expanded}
        onClick={() => onToggle(fold.turnId, rowRef.current)}
      >
        <span>{fold.label}</span>
        <small>
          {fold.hiddenCount} {fold.hiddenCount === 1 ? "activity" : "activities"}
        </small>
        <ChevronRight
          aria-hidden="true"
          data-icon-name="ChevronRight"
          data-expanded={fold.expanded ? "true" : "false"}
        />
      </button>
    </div>
  );
};

const WorkGroupDisclosure = ({
  groupKey,
  folded,
  visible,
  running,
  report,
}: {
  readonly groupKey: string;
  readonly folded: ReadonlyArray<ReactTimelineRecord>;
  readonly visible: ReadonlyArray<ReactTimelineRecord>;
  readonly running: boolean;
  readonly report: IntentReporter;
}): ReactElement => {
  const [expanded, setExpanded] = useState(false);
  const groupRef = useRef<HTMLDivElement>(null);
  const activityLabel = `${folded.length} ${folded.length === 1 ? "activity" : "activities"}`;
  return (
    <div ref={groupRef} className="oa-react-work-group" role="listitem" data-work-group={groupKey}>
      <button
        className="oa-react-work-group-summary"
        type="button"
        aria-expanded={expanded}
        aria-controls={`${groupKey}:details`}
        onClick={() =>
          togglePreservingReaderPosition(groupRef.current, () => setExpanded((value) => !value))
        }
      >
        <ChevronRight
          aria-hidden="true"
          data-icon-name="ChevronRight"
          data-expanded={expanded ? "true" : "false"}
        />
        <strong>{running ? `+${folded.length} previous` : "Worked"}</strong>
        <span>{activityLabel}</span>
      </button>
      {expanded ? (
        <div id={`${groupKey}:details`} role="list">
          {folded.map((entry) => (
            <MemoTimelineItemBoundary key={entry.key} record={entry} report={report} />
          ))}
        </div>
      ) : null}
      {visible.map((entry) => (
        <MemoTimelineItemBoundary key={entry.key} record={entry} report={report} />
      ))}
    </div>
  );
};

const TimelineDisplayRowContent = ({
  row,
  assistantMetaKeys,
  report,
  onToggleTurn,
}: {
  readonly row: TimelineDisplayRow;
  readonly assistantMetaKeys: ReadonlySet<string>;
  readonly report: IntentReporter;
  readonly onToggleTurn: (turnId: string, row: HTMLElement | null) => void;
}): ReactElement => {
  const presented = (record: ReactTimelineRecord): ReactTimelineRecord =>
    isMessageRecord(record) && !isUserRecord(record)
      ? { ...record, showAssistantMeta: assistantMetaKeys.has(record.key) }
      : record;
  if (row.kind === "turn-fold")
    return <TurnFoldDisclosure fold={row.fold} onToggle={onToggleTurn} />;
  if (row.kind === "record")
    return <MemoTimelineItemBoundary record={presented(row.record)} report={report} />;
  const running = row.records.some((entry) => entry.status === "running");
  const visible = running ? row.records.slice(-1) : [];
  const folded = running ? row.records.slice(0, -1) : row.records;
  return (
    <WorkGroupDisclosure
      groupKey={row.id}
      folded={folded}
      visible={visible}
      running={running}
      report={report}
    />
  );
};

const TIMELINE_VIRTUALIZE_AFTER = 80;
const TIMELINE_ESTIMATED_ROW_HEIGHT = 116;
const TIMELINE_OVERSCAN_PX = 640;

type TimelineVirtualWindow = Readonly<{
  start: number;
  end: number;
  top: number;
  bottom: number;
}>;

/** Pure measured-window calculation used by runtime and the 500-row proof. */
export const resolveReactTimelineVirtualWindow = (
  input: Readonly<{
    rowIds: ReadonlyArray<string>;
    measuredHeights: ReadonlyMap<string, number>;
    scrollTop: number;
    viewportHeight: number;
  }>,
): TimelineVirtualWindow => {
  const count = input.rowIds.length;
  if (count <= TIMELINE_VIRTUALIZE_AFTER) return { start: 0, end: count, top: 0, bottom: 0 };
  const sizes = input.rowIds.map(
    (id) => input.measuredHeights.get(id) ?? TIMELINE_ESTIMATED_ROW_HEIGHT,
  );
  const offsets: number[] = [0];
  for (const size of sizes) offsets.push(offsets[offsets.length - 1]! + size);
  const total = offsets[offsets.length - 1] ?? 0;
  if (input.viewportHeight <= 0) {
    const start = Math.max(0, count - 40);
    return { start, end: count, top: offsets[start] ?? 0, bottom: 0 };
  }
  const windowStart = Math.max(0, input.scrollTop - TIMELINE_OVERSCAN_PX);
  const windowEnd = Math.min(total, input.scrollTop + input.viewportHeight + TIMELINE_OVERSCAN_PX);
  let start = 0;
  while (start < count && (offsets[start + 1] ?? total) < windowStart) start += 1;
  let end = start;
  while (end < count && (offsets[end] ?? total) <= windowEnd) end += 1;
  end = Math.max(start + 1, Math.min(count, end));
  return {
    start,
    end,
    top: offsets[start] ?? 0,
    bottom: Math.max(0, total - (offsets[end] ?? total)),
  };
};

const MeasuredTimelineRow = ({
  row,
  index,
  virtualized,
  onMeasure,
  children,
}: {
  readonly row: TimelineDisplayRow;
  readonly index: number;
  readonly virtualized: boolean;
  readonly onMeasure: (id: string, index: number, height: number) => void;
  readonly children: ReactNode;
}): ReactElement => {
  const measureRef = useCallback(
    (element: HTMLDivElement | null) => {
      if (element === null || !virtualized) return;
      const measure = (): void => {
        const height = element.getBoundingClientRect().height;
        if (Number.isFinite(height) && height > 0) onMeasure(row.id, index, height);
      };
      measure();
      const observer = new ResizeObserver(measure);
      observer.observe(element);
      return () => observer.disconnect();
    },
    [index, onMeasure, row.id, virtualized],
  );
  return (
    <MessageScrollerItem
      ref={measureRef}
      messageId={row.id}
      data-virtual-index={virtualized ? index : undefined}
    >
      {children}
    </MessageScrollerItem>
  );
};

const TimelineDisplayRows = ({
  rows,
  window,
  assistantMetaKeys,
  report,
  onToggleTurn,
  onMeasure,
}: {
  readonly rows: ReadonlyArray<TimelineDisplayRow>;
  readonly window: TimelineVirtualWindow;
  readonly assistantMetaKeys: ReadonlySet<string>;
  readonly report: IntentReporter;
  readonly onToggleTurn: (turnId: string, row: HTMLElement | null) => void;
  readonly onMeasure: (id: string, index: number, height: number) => void;
}): ReactElement => {
  const virtualized = rows.length > TIMELINE_VIRTUALIZE_AFTER;
  const visible = rows.slice(window.start, window.end);
  return (
    <>
      {window.top > 0 ? (
        <div
          className="oa-react-timeline-virtual-spacer"
          data-virtual-spacer="top"
          style={{ height: window.top }}
        />
      ) : null}
      {visible.map((row, relativeIndex) => (
        <MeasuredTimelineRow
          key={row.id}
          row={row}
          index={window.start + relativeIndex}
          virtualized={virtualized}
          onMeasure={onMeasure}
        >
          <TimelineDisplayRowContent
            row={row}
            assistantMetaKeys={assistantMetaKeys}
            report={report}
            onToggleTurn={onToggleTurn}
          />
        </MeasuredTimelineRow>
      ))}
      {window.bottom > 0 ? (
        <div
          className="oa-react-timeline-virtual-spacer"
          data-virtual-spacer="bottom"
          style={{ height: window.bottom }}
        />
      ) : null}
    </>
  );
};

const TimelineScroller = (props: TimelineProps): ReactElement => {
  const viewportRef = useRef<HTMLDivElement>(null);
  const turns = useMemo(() => deriveReactTimelineTurns(props.records), [props.records]);
  const assistantMetaKeys = useMemo(() => deriveAssistantMetaKeys(props.records), [props.records]);
  const [expandedTurnIds, setExpandedTurnIds] = useState<ReadonlySet<string>>(() => new Set());
  const [readerMode, setReaderMode] = useState<"following" | "anchored" | "free">("following");
  const readerModeRef = useRef(readerMode);
  const updateReaderMode = useCallback((mode: "following" | "anchored" | "free"): void => {
    readerModeRef.current = mode;
    setReaderMode(mode);
  }, []);
  const [scrollMetrics, setScrollMetrics] = useState({ top: 0, height: 0 });
  const measuredHeights = useRef(new Map<string, number>());
  const [measurementVersion, setMeasurementVersion] = useState(0);
  const displayRows = useMemo(
    () => deriveReactTimelineDisplayRows(props.records, expandedTurnIds, props.working === true),
    [expandedTurnIds, props.records, props.working],
  );
  const virtualWindow = useMemo(
    () =>
      resolveReactTimelineVirtualWindow({
        rowIds: displayRows.map((row) => row.id),
        measuredHeights: measuredHeights.current,
        scrollTop: scrollMetrics.top,
        viewportHeight: scrollMetrics.height,
      }),
    [displayRows, measurementVersion, scrollMetrics.height, scrollMetrics.top],
  );
  const virtualStartRef = useRef(virtualWindow.start);
  virtualStartRef.current = virtualWindow.start;
  const requestedEdge = useRef<"top" | "bottom" | null>(null);
  const previousLoadingEdge = useRef(props.loadingEdge);
  useEffect(() => {
    if (previousLoadingEdge.current !== props.loadingEdge && props.loadingEdge === null)
      requestedEdge.current = null;
    previousLoadingEdge.current = props.loadingEdge;
  }, [props.loadingEdge]);
  useEffect(() => {
    const element = viewportRef.current;
    if (element === null) return;
    const measure = (): void =>
      setScrollMetrics((current) => {
        const next = { top: element.scrollTop, height: element.clientHeight };
        return current.top === next.top && current.height === next.height ? current : next;
      });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const updateMinimapViewportState = (element: HTMLElement): void => {
    for (const button of element.parentElement?.querySelectorAll<HTMLButtonElement>(
      ".oa-react-timeline-minimap button",
    ) ?? []) {
      const turnId = button.dataset.turnId;
      const target =
        turnId === undefined
          ? null
          : ([...element.querySelectorAll<HTMLElement>("[data-message-id]")].find(
              (candidate) => candidate.dataset.messageId === turnId,
            ) ?? null);
      if (target === null) {
        button.dataset.inView = "false";
        continue;
      }
      const viewportRect = element.getBoundingClientRect();
      const rowRect = target.getBoundingClientRect();
      button.dataset.inView =
        rowRect.bottom > viewportRect.top && rowRect.top < viewportRect.bottom ? "true" : "false";
    }
  };

  const onScroll = (): void => {
    const element = viewportRef.current;
    if (element === null) return;
    setScrollMetrics((current) => {
      const next = { top: element.scrollTop, height: element.clientHeight };
      return current.top === next.top && current.height === next.height ? current : next;
    });
    updateMinimapViewportState(element);
    if (
      element.scrollHeight - element.scrollTop - element.clientHeight <= 2 &&
      readerModeRef.current !== "anchored"
    )
      updateReaderMode("following");
    if (props.loadingEdge !== null || requestedEdge.current !== null) return;
    if (props.offset > 0 && element.scrollTop < element.clientHeight * 1.5) {
      requestedEdge.current = "top";
      dispatch(props.report, "HistoryOlderRequested");
      return;
    }
    const windowEnd = props.offset + props.loadedItemCount;
    if (
      windowEnd < props.totalItems &&
      element.scrollHeight - element.scrollTop - element.clientHeight < element.clientHeight * 1.5
    ) {
      requestedEdge.current = "bottom";
      dispatch(props.report, "HistoryNewerRequested");
    }
  };
  const releaseReaderIntent = useCallback((): void => {
    const element = viewportRef.current;
    if (element === null) return;
    updateReaderMode("free");
  }, [updateReaderMode]);

  const onToggleTurn = useCallback((turnId: string, row: HTMLElement | null): void => {
    togglePreservingReaderPosition(row, () =>
      setExpandedTurnIds((existing) => {
        const next = new Set(existing);
        if (next.has(turnId)) next.delete(turnId);
        else next.add(turnId);
        return next;
      }),
    );
  }, []);

  const onMeasure = useCallback(
    (id: string, index: number, height: number): void => {
      const previous = measuredHeights.current.get(id) ?? TIMELINE_ESTIMATED_ROW_HEIGHT;
      if (Math.abs(previous - height) < 0.5) return;
      measuredHeights.current.set(id, height);
      const viewport = viewportRef.current;
      if (viewport !== null && index < virtualStartRef.current && readerMode === "free")
        viewport.scrollTop += height - previous;
      setMeasurementVersion((version) => version + 1);
    },
    [readerMode],
  );

  const scrollToVirtualTurn = useCallback(
    (turnId: string): void => {
      const viewport = viewportRef.current;
      const index = displayRows.findIndex((row) => row.id === turnId);
      if (viewport === null || index < 0) return;
      let top = 0;
      for (let cursor = 0; cursor < index; cursor += 1) {
        const row = displayRows[cursor]!;
        top += measuredHeights.current.get(row.id) ?? TIMELINE_ESTIMATED_ROW_HEIGHT;
      }
      viewport.scrollTo({ top: Math.max(0, top - 24), behavior: "auto" });
    },
    [displayRows],
  );

  // Owner directive 2026-07-19: the timeline sticks to the bottom like a normal
  // chat (see MessageScrollerProvider defaultScrollPosition="end" below). A
  // newly-authored turn no longer anchors itself to the TOP of the viewport;
  // new content keeps the conversation pinned to the latest message while the
  // reader stays in "following" mode, and manual scroll-up still yields to
  // "free" mode without being yanked back.
  return (
    <MessageScroller
      className="oa-react-timeline-region"
      aria-label="Conversation timeline"
      data-reader-mode={readerMode}
    >
      <MessageScrollerViewport
        ref={viewportRef}
        className="oa-react-timeline-scroll"
        data-timeline-session={props.sessionKey}
        onScroll={onScroll}
        onPointerDownCapture={releaseReaderIntent}
        onWheelCapture={releaseReaderIntent}
        onSelect={releaseReaderIntent}
        aria-label={`${props.records.length} loaded conversation items of ${props.totalItems}`}
      >
        <MessageScrollerContent
          className="oa-react-timeline-content"
          aria-busy={props.working === true}
        >
          {props.loadingEdge === "top" ? (
            <p className="oa-react-timeline-loading" role="status">
              Fetching earlier items…
            </p>
          ) : props.offset > 0 ? (
            <p className="oa-react-timeline-position">
              Showing items {props.offset + 1}–{props.offset + props.loadedItemCount} of{" "}
              {props.totalItems}
            </p>
          ) : null}
          <TimelineDisplayRows
            rows={displayRows}
            window={virtualWindow}
            assistantMetaKeys={assistantMetaKeys}
            report={props.report}
            onToggleTurn={onToggleTurn}
            onMeasure={onMeasure}
          />
          {props.waitingForAnswer ? (
            <MessageScrollerItem messageId="waiting-for-answer-indicator">
              <div className="oa-react-waiting" role="status" aria-label="Waiting for your answer">
                <span>Waiting for your answer</span>
              </div>
            </MessageScrollerItem>
          ) : null}
          {props.working ? (
            <MessageScrollerItem messageId="working-indicator">
              <div
                className="oa-react-working"
                role="status"
                aria-label={`${props.agentName ?? "Codex"} is working`}
              >
                <span>Working</span>
                <i />
                <i />
                <i />
              </div>
            </MessageScrollerItem>
          ) : null}
          {props.loadingEdge === "bottom" ? (
            <p className="oa-react-timeline-loading" role="status">
              Fetching newer items…
            </p>
          ) : null}
        </MessageScrollerContent>
      </MessageScrollerViewport>
      <MessageScrollerButton
        className="oa-react-new-activity"
        behavior="auto"
        aria-label="Jump to latest"
        title="Jump to latest"
        onClick={() => updateReaderMode("following")}
      />
      <TimelineMinimap
        turns={turns}
        viewportRef={viewportRef}
        releaseReaderIntent={releaseReaderIntent}
        onSelectVirtualTurn={scrollToVirtualTurn}
      />
    </MessageScroller>
  );
};

export const ReactTimeline = (props: TimelineProps): ReactElement => (
  <MessageScrollerProvider key={props.sessionKey} autoScroll defaultScrollPosition="end">
    <TimelineScroller {...props} />
  </MessageScrollerProvider>
);

export const ConversationTimeline = ({
  page,
  notes,
  loadingEdge,
  working,
  waitingForAnswer,
  workingDirectory,
  agentName,
  bootSequenceAgents,
  report,
}: {
  readonly page: CodexHistoryPage | null;
  readonly notes: ReadonlyArray<DesktopNoteEntry>;
  readonly loadingEdge: "top" | "bottom" | null;
  readonly working?: boolean;
  readonly waitingForAnswer?: boolean;
  readonly workingDirectory: string | null;
  readonly agentName: string;
  readonly bootSequenceAgents?: ReadonlyArray<BootSequenceAgentLine>;
  readonly report: IntentReporter;
}): ReactElement => {
  // Owner directive 2026-07-19: an empty conversation shows the Boot Sequence —
  // a terminal-style scan of which agents are available — instead of a blank
  // region. It reflects live discovery state and updates as lanes resolve.
  if (page === null && notes.length === 0)
    return (
      <section className="oa-react-timeline-empty" aria-label="Conversation">
        {bootSequenceAgents === undefined || bootSequenceAgents.length === 0 ? null : (
          <ReactBootSequence agents={bootSequenceAgents} />
        )}
      </section>
    );
  const records =
    page === null ? projectLocalTimelineRecords(notes) : projectReactTimelineRecords(page.items);
  return (
    <ReactTimeline
      sessionKey={page?.selectedThreadRef ?? "local"}
      records={records}
      loadedItemCount={page?.items.length ?? records.length}
      offset={page?.offset ?? 0}
      totalItems={page?.totalItems ?? records.length}
      loadingEdge={loadingEdge}
      working={working}
      waitingForAnswer={waitingForAnswer}
      agentName={agentName}
      report={report}
    />
  );
};
