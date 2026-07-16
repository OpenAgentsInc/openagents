import { Button } from "#components/ui/button"
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "#components/ui/message-scroller"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload, type MarkdownBlock, type MarkdownInline } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
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
} from "@openagentsinc/ui/desktop-workbench"
import type { ReactElement, ReactNode } from "react"
import { Component, createElement, memo, useEffect, useMemo, useRef, useState } from "react"
import { ChevronRight, Folder } from "lucide-react"

import type { CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts"
import {
  workbenchItemSignature,
  workbenchPlanItemFromEntries,
  type WorkbenchCollabAgentStatus,
  type WorkbenchItem,
} from "../workbench-item-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"
import { parseChatMarkdown } from "./markdown.ts"
import { childInterruptable } from "./runtime-cards.ts"
import { humanizeToolInvocation, projectToolCardEntries } from "./tool-cards.ts"

const terminalStatuses = new Set([
  "canceled", "cancelled", "completed", "errored", "failed", "interrupted", "shutdown",
  "task_complete", "task_completed", "turn_aborted", "turn_canceled", "turn_cancelled",
  "turn_complete", "turn_completed", "turn_failed", "turn_interrupted",
])

const field = (item: CodexHistoryItem, name: string): string | null =>
  item.fields.find(entry => entry.label.toLocaleLowerCase() === name)?.value ?? null

const normalizedStatus = (item: CodexHistoryItem): string =>
  (item.status ?? item.label).trim().toLocaleLowerCase().replaceAll(/[ .-]+/g, "_")

const isTerminal = (item: CodexHistoryItem): boolean =>
  item.kind === "lifecycle" && terminalStatuses.has(normalizedStatus(item))

export type ReactTimelineRecord = Readonly<{
  key: string
  itemRef: string
  sequence: number
  kind: CodexHistoryItem["kind"] | "local_message" | "question"
  label: string
  body: string
  timestamp: string
  status: string | null
  redacted: boolean
  fields: CodexHistoryItem["fields"]
  resultRef: string | null
  resultBody: string | null
  resultStatus: string | null
  /**
   * Typed item payload (#8859) when the source note/history row carried one.
   * Wave-2 card lanes render this; the string label/body stay authoritative
   * until then.
   */
  item?: WorkbenchItem
  /**
   * Desktop-only interrupt wiring for a live delegate-child runtime note
   * (#8867 T10). NOT part of the shared `WorkbenchItem` contract — the
   * Interrupt affordance is a desktop-local capability (`runtime-cards.ts`'s
   * `childInterruptable`, the same predicate the compatibility renderer
   * uses), never assumed by the web host that also consumes `dispatchWorkbenchItem`.
   */
  runtimeChild?: Readonly<{ turnRef: string; childRef: string; interruptable: boolean }>
}>

const recordFromItem = (
  item: CodexHistoryItem,
  result: CodexHistoryItem | undefined,
): ReactTimelineRecord => {
  const args = item.kind === "tool_call" ? item.summary || field(item, "input") || "" : ""
  const humanized = item.kind === "tool_call" ? humanizeToolInvocation(item.label, args) : null
  const typedItem = result?.item ?? item.item
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
  }
}

/** Pure presentation over the bounded history contract; no provider parsing. */
export const projectReactTimelineRecords = (
  source: ReadonlyArray<CodexHistoryItem>,
): ReadonlyArray<ReactTimelineRecord> => {
  const byRef = new Map<string, CodexHistoryItem>()
  for (const item of source) byRef.set(item.itemRef, item)
  const items = [...byRef.values()].sort(
    (left, right) => left.sequence - right.sequence || left.itemRef.localeCompare(right.itemRef),
  )
  const newestTerminal = items.filter(isTerminal).at(-1)?.itemRef ?? null
  const resultByCall = new Map<string, CodexHistoryItem>()
  for (const item of items) {
    if (item.kind !== "tool_result") continue
    const callRef = field(item, "call")
    if (callRef !== null) resultByCall.set(callRef, item)
  }
  const consumed = new Set<string>()
  const records: Array<ReactTimelineRecord> = []
  for (const item of items) {
    // Transport/accounting scaffolding stays available in the bounded history
    // inspector; it is not a primary conversation row.
    if (["session", "context", "metadata", "usage"].includes(item.kind)) continue
    // Persisted reasoning can intentionally be only a redaction marker. Absence
    // is the honest primary presentation, not a false failure card.
    if (item.kind === "reasoning" && item.redacted) continue
    if (isTerminal(item) && item.itemRef !== newestTerminal) continue
    if (item.kind === "tool_call") {
      const callRef = field(item, "call")
      const result = callRef === null ? undefined : resultByCall.get(callRef)
      if (result !== undefined) consumed.add(result.itemRef)
      records.push(recordFromItem(item, result))
      continue
    }
    if (item.kind === "tool_result" && consumed.has(item.itemRef)) continue
    records.push(recordFromItem(item, undefined))
  }
  return records
}

export const projectLocalTimelineRecords = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<ReactTimelineRecord> => projectToolCardEntries(notes).flatMap((entry, index): ReadonlyArray<ReactTimelineRecord> => {
  if (entry.kind === "tool") {
    const humanized = humanizeToolInvocation(entry.card.toolName, entry.card.argsSummary)
    return [{
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
      resultStatus: entry.card.status === "failed" ? "failed" : entry.card.status === "ok" ? "completed" : null,
      ...(entry.card.item === undefined ? {} : { item: entry.card.item }),
    }]
  }
  // Delegate-child lifecycle (collabAgentToolCall/subAgentActivity, projected
  // by local-harness.ts as a `runtime: {kind:"child"}` note): route through
  // the SAME typed "agent" WorkbenchItem the DesktopAgentGroup card renders
  // (#8867 T10) instead of falling through to a flat system-notice line.
  // `queue` runtime notes are untouched (unchanged generic fallthrough
  // below); `plan` runtime notes get their own typed-item branch further
  // down (T8 #8865).
  if (entry.kind === "runtime" && entry.note.runtime?.kind === "child") {
    const runtime = entry.note.runtime
    const coarseStatus = runtime.status === "running" ? "in_progress" as const
      : runtime.status === "completed" ? "completed" as const : "failed" as const
    const childCollabStatus = runtime.status === "running" ? "running" as const
      : runtime.status === "completed" ? "completed" as const : "errored" as const
    return [{
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
        // harness starts emitting `child_*` FableLocalEvents.
        source: "codex",
        status: coarseStatus,
        children: [{
          threadRef: runtime.childRef,
          status: childCollabStatus,
          ...(runtime.title === "" ? {} : { nickname: runtime.title }),
        }],
      },
      runtimeChild: { turnRef: runtime.turnRef, childRef: runtime.childRef, interruptable: childInterruptable(runtime) },
    }]
  }
  const note = entry.note
  if (note.role === "system" && /^(Usage|Connected)\s*·/i.test(note.text)) return []
  // T8 (#8865): a live plan_updated note carries its full typed payload on
  // `note.runtime` (entries + optional prose) — read it DIRECTLY instead of
  // pattern-matching `note.text` (which is always the literal "Plan updated"
  // and never matched the old `/^Plan\s*·/` check, so this card silently
  // degraded to a generic system-message notice on this surface). The typed
  // `item` here is what makes `TimelineItem` dispatch through the SAME
  // `DesktopPlanCard` history rows and `turn/plan/updated` already use.
  if (note.runtime?.kind === "plan") {
    const runtimePlan = note.runtime
    return [{
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
    }]
  }
  const kind: ReactTimelineRecord["kind"] = note.question !== undefined
    ? "question"
    : note.role !== "system"
      ? "local_message"
      : /^Reasoning\s*·/i.test(note.text)
        ? "reasoning"
        : /^Approval\s*·/i.test(note.text)
          ? "approval"
          : /^Turn (completed|complete|canceled|cancelled)$/i.test(note.text)
            ? "lifecycle"
            : /^Turn (failed|interrupted)|error/i.test(note.text)
              ? "error"
              : /\s·\s(?:running|completed|failed|errored)$/i.test(note.text)
                ? "tool_call"
                : "system_message"
  const label = note.question !== undefined
    ? note.question.kind === "tool_approval" ? "Tool approval"
      : note.question.kind === "plan_review" ? "Plan review" : "Question"
    : note.role === "user" ? "You"
      : note.role === "assistant" ? "Assistant"
        : kind === "tool_call" ? note.text.split(" · ")[0] || "Tool"
          : kind === "reasoning" ? "Reasoning"
            : "System"
  return [{
    key: note.key,
    itemRef: note.key,
    sequence: index,
    kind,
    label,
    body: note.question?.questions[0]?.question ?? note.text.replace(/^(Reasoning|Approval)\s*·\s*/i, ""),
    timestamp: note.timestamp,
    status: note.question?.status ?? note.runtime?.kind ?? null,
    redacted: false,
    fields: [],
    resultRef: null,
    resultBody: null,
    resultStatus: null,
  }]
})

const Inline = ({ nodes }: { readonly nodes: ReadonlyArray<MarkdownInline> }): ReactNode => nodes.map((node, index) => {
  if (node.kind === "text") return <span key={index}>{node.text}</span>
  if (node.kind === "code") return <code key={index}>{node.text}</code>
  if (node.kind === "strong") return <strong key={index}><Inline nodes={node.children} /></strong>
  if (node.kind === "emphasis") return <em key={index}><Inline nodes={node.children} /></em>
  return null
})

const Blocks = ({ blocks }: { readonly blocks: ReadonlyArray<MarkdownBlock> }): ReactNode => blocks.map((block, index) => {
  if (block.kind === "paragraph") return <p key={index}><Inline nodes={block.children} /></p>
  if (block.kind === "heading") return createElement(`h${block.level}`, { key: index }, <Inline nodes={block.children} />)
  if (block.kind === "blockquote") return <blockquote key={index}><Blocks blocks={block.children} /></blockquote>
  if (block.kind === "list") {
    const Tag = block.ordered ? "ol" : "ul"
    return <Tag key={index}>{block.items.map((item, itemIndex) => <li key={itemIndex}><Blocks blocks={item} /></li>)}</Tag>
  }
  return null
})

export const SafeReactMarkdown = memo(({ value }: { readonly value: string }): ReactElement => {
  const segments = useMemo(() => parseChatMarkdown(value), [value])
  return <div className="oa-react-markdown">
    {segments.map((segment, index) => segment.kind === "markdown"
      ? <Blocks key={index} blocks={segment.blocks} />
      : segment.kind === "code"
        ? <pre key={index}><code data-language={segment.language}>{segment.code}</code></pre>
        : <hr key={index} />)}
  </div>
})

const isUserRecord = (record: ReactTimelineRecord): boolean =>
  record.kind === "user_message" || record.label === "You"

const isMessageRecord = (record: ReactTimelineRecord): boolean =>
  isUserRecord(record) || ["assistant_message", "agent_message", "local_message"].includes(record.kind)

const isWorkRecord = (record: ReactTimelineRecord): boolean =>
  ["reasoning", "tool_call", "tool_result", "approval", "collaboration"].includes(record.kind)

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
  "command", "fileChange", "toolCall", "reasoning", "approval",
  "meter", "compaction", "sleep", "review", "hook", "plan", "notice",
])

const compact = (value: string, limit = 180): string => {
  const normalized = value.replaceAll("\\n", " ").replaceAll(/\s+/g, " ").trim()
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit - 1)}…`
}

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
    case "spawnAgent": return "spawn"
    case "sendInput": return "send"
    case "resumeAgent": return "resume"
    case "wait": return "wait"
    case "closeAgent": return "close"
    default: return tool
  }
}

/** codex-history.ts's raw snake_case `operation` field -> the same short verb vocabulary. */
const historyOperationTag = (operation: string): string => {
  switch (operation) {
    case "spawn_agent": return "spawn"
    case "send_input": return "send"
    case "resume_agent": return "resume"
    case "wait": return "wait"
    case "close_agent": return "close"
    case "interrupt_agent": return "interrupt"
    default: return operation.replaceAll("_", " ")
  }
}

/** CollabAgentStatus -> the coarse icon/tone bucket DesktopAgentRow understands. */
const toDesktopAgentStatusFromCollab = (status: WorkbenchCollabAgentStatus): DesktopAgentStatus => {
  switch (status) {
    case "running": return "running"
    case "completed": return "completed"
    case "pendingInit": return "waiting"
    case "interrupted":
    case "errored":
    case "shutdown":
    case "notFound":
      return "failed"
  }
}

/** "pendingInit" -> "PENDING INIT"; "notFound" -> "NOT FOUND"; the rest just uppercase. */
const collabStatusLabel = (status: WorkbenchCollabAgentStatus): string =>
  status.replace(/([a-z])([A-Z])/g, "$1 $2").toUpperCase()

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => {})
}

export const TimelineItem = ({ record, report }: {
  readonly record: ReactTimelineRecord
  readonly report: IntentReporter
}): ReactElement => {
  if (record.kind === "lifecycle" && !["failed", "errored", "interrupted"].includes(record.status ?? "")) {
    return <span data-timeline-key={record.key} data-kind="lifecycle" hidden />
  }
  if (record.item !== undefined && dispatchableWorkbenchKinds.has(record.item.kind)) {
    return dispatchWorkbenchItem(record.item as WorkbenchDispatchItem, {
      itemKey: record.key,
      sequence: record.sequence,
    })
  }
  // Typed live delegate-child status (#8867 T10): a per-agent status row
  // built from collabAgentToolCall/subAgentActivity (via
  // `projectLocalTimelineRecords`'s runtime-child branch above). Placed
  // BEFORE the history-only `collaboration` string branch below so a record
  // that carries both (never true today, but future-proof) prefers the typed
  // path.
  if (record.item?.kind === "agent") {
    const agentItem = record.item
    const operation = agentItem.tool === undefined ? undefined : agentOperationTag(agentItem.tool)
    const agents: ReadonlyArray<DesktopAgentActivity> = agentItem.children !== undefined && agentItem.children.length > 0
      ? agentItem.children.map(child => ({
          agentKey: child.threadRef,
          detail: agentItem.activityKind !== undefined ? "" : (agentItem.prompt ?? ""),
          name: child.nickname ?? agentItem.agentPath ?? child.threadRef,
          role: agentItem.activityKind !== undefined ? "Subagent activity" : "Delegated agent",
          status: toDesktopAgentStatusFromCollab(child.status),
          statusLabel: collabStatusLabel(child.status),
          ...(agentItem.agentPath === undefined ? {} : { path: agentItem.agentPath }),
          ...(agentItem.activityKind === undefined ? {} : { activityKind: agentItem.activityKind }),
          ...(record.runtimeChild === undefined ? {} : {
            interruptable: record.runtimeChild.interruptable,
            onInterrupt: () => dispatch(report, "DesktopChildInterruptRequested", {
              turnRef: record.runtimeChild!.turnRef,
              childRef: record.runtimeChild!.childRef,
            }),
          }),
        }))
      : [{
          agentKey: record.key,
          detail: agentItem.prompt ?? "",
          name: agentItem.tool ?? "Agent",
          role: "agent",
          status: agentItem.status === "in_progress" ? "running" : agentItem.status === "completed" ? "completed" : "failed",
        }]
    return <DesktopAgentGroup agents={agents} itemKey={record.key} operation={operation} prompt={agentItem.prompt} />
  }
  if (record.kind === "collaboration") {
    const status = ["failed", "errored", "interrupted"].includes(record.status ?? "")
      ? "failed"
      : record.status === "running" || record.status === "in_progress"
        ? "running"
        : "completed"
    const agentRef = record.fields.find(entry => entry.label.toLocaleLowerCase() === "agent")?.value ?? record.itemRef
    // Nice-to-have enrichment (#8867 T10 §4): codex-history.ts already
    // captures the raw `operation`/`activity` fields on collab-like rows
    // (see `projectRow`'s collab branch); surface them the same way the live
    // typed path does, without a deeper history-graph restructure.
    const historyOperation = record.fields.find(entry => entry.label.toLocaleLowerCase() === "operation")?.value
    const historyActivity = record.fields.find(entry => entry.label.toLocaleLowerCase() === "activity")?.value
    const activityKind = historyActivity === "started" || historyActivity === "interacted" || historyActivity === "interrupted"
      ? historyActivity
      : undefined
    return <DesktopAgentGroup
      agents={[{
        agentKey: agentRef,
        detail: record.body,
        name: record.label,
        role: "Delegated agent",
        status,
        transcript: [{ label: "Activity", text: record.body }],
        ...(activityKind === undefined ? {} : { activityKind }),
      }]}
      itemKey={record.key}
      operation={historyOperation === undefined ? undefined : historyOperationTag(historyOperation)}
    />
  }
  if (isWorkRecord(record)) return <DesktopWorkEntry
    body={<>
      {record.redacted ? <p>Details unavailable.</p> : <pre><code>{record.body}</code></pre>}
      {record.resultBody === null ? null : <><strong>{record.resultStatus === "failed" ? "Result · failed" : "Result"}</strong><pre><code>{record.resultBody}</code></pre></>}
      <Button className="oa-react-item-details" type="button" variant="ghost" size="xs"
        onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}>Inspect event</Button>
    </>}
    itemKey={record.key}
    kind={record.kind}
    label={record.label}
    preview={compact(record.body || record.resultBody || record.label)}
    status={record.status ?? "completed"}
    statusLabel={["failed", "errored", "interrupted"].includes(record.status ?? "") ? "Failed" : record.status === "running" ? "Running" : "Done"}
  />

  // Fallback only: a "plan" record always carries a typed `item` today (the
  // live-note and history projectors above attach one whenever there is any
  // real content), so `dispatchableWorkbenchKinds` already handled it. This
  // stays for the edge case of a genuinely empty/untyped plan row.
  if (record.kind === "plan") return <DesktopPlanCard
    entries={[{
      step: record.body,
      status: record.status === "completed" ? "completed" : record.status === "running" || record.status === "in_progress" ? "in_progress" : "pending",
    }]}
    itemKey={record.key}
  />

  const danger = record.kind === "error" || record.kind === "gap" || ["failed", "errored", "interrupted"].includes(record.status ?? "")
  if (!isMessageRecord(record) || danger || record.redacted) return <DesktopTimelineNotice
    body={record.redacted ? "Message content unavailable." : record.body}
    danger={danger}
    itemKey={record.key}
    kind={record.kind}
    label={danger ? record.label : "Update"}
  />

  return <DesktopTimelineMessage itemKey={record.key} kind={record.kind} label={record.label} sequence={record.sequence} tone={isUserRecord(record) ? "user" : "assistant"}>
    <SafeReactMarkdown value={record.body} />
    {record.kind === "local_message" || record.kind === "question" ? null
      : <Button className="oa-react-item-details" type="button" variant="ghost" size="xs"
          onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}
          aria-label={`Show details for ${record.label}, item ${record.sequence + 1}`}>Details</Button>}
  </DesktopTimelineMessage>
}

class TimelineItemBoundary extends Component<Readonly<{
  record: ReactTimelineRecord
  report: IntentReporter
}>, Readonly<{ failed: boolean }>> {
  state = { failed: false }
  static getDerivedStateFromError(): Readonly<{ failed: boolean }> { return { failed: true } }
  render(): ReactElement {
    if (this.state.failed) return <article className="oa-react-timeline-item" data-timeline-key={this.props.record.key}
      data-kind="presentation_error" data-tone="danger" role="listitem" aria-label={`Item ${this.props.record.sequence + 1} unavailable`}>
      <header><strong>Item unavailable</strong><span>Presentation error</span></header>
      <p>This item could not be displayed. No completion state was inferred.</p>
    </article>
    return <TimelineItem record={this.props.record} report={this.props.report} />
  }
}

const sameTimelineRecord = (left: ReactTimelineRecord, right: ReactTimelineRecord): boolean =>
  left.key === right.key && left.itemRef === right.itemRef && left.sequence === right.sequence &&
  left.kind === right.kind && left.label === right.label && left.body === right.body &&
  left.timestamp === right.timestamp && left.status === right.status && left.redacted === right.redacted &&
  left.resultRef === right.resultRef && left.resultBody === right.resultBody && left.resultStatus === right.resultStatus &&
  // Scalar signature comparison — content changes flip it without
  // stringifying multi-kilobyte diffs on every memo check (#8859).
  workbenchItemSignature(left.item) === workbenchItemSignature(right.item) &&
  // Interrupt eligibility (#8867 T10) can flip (child_steered) without any
  // other field moving; compare it explicitly so the button disappears the
  // instant the runtime marks the child no-longer-interruptable.
  left.runtimeChild?.interruptable === right.runtimeChild?.interruptable &&
  left.runtimeChild?.turnRef === right.runtimeChild?.turnRef &&
  left.runtimeChild?.childRef === right.runtimeChild?.childRef &&
  left.fields.length === right.fields.length && left.fields.every((field, index) => {
    const candidate = right.fields[index]
    return candidate !== undefined && field.label === candidate.label && field.value === candidate.value
  })

const MemoTimelineItemBoundary = memo(TimelineItemBoundary, (left, right) =>
  left.report === right.report && sameTimelineRecord(left.record, right.record))

type TimelineProps = Readonly<{
  sessionKey: string
  records: ReadonlyArray<ReactTimelineRecord>
  loadedItemCount: number
  offset: number
  totalItems: number
  loadingEdge: "top" | "bottom" | null
  working?: boolean
  report: IntentReporter
}>

const WorkGroupDisclosure = ({ groupKey, folded, visible, running, report }: {
  readonly groupKey: string
  readonly folded: ReadonlyArray<ReactTimelineRecord>
  readonly visible: ReadonlyArray<ReactTimelineRecord>
  readonly running: boolean
  readonly report: IntentReporter
}): ReactElement => {
  const [expanded, setExpanded] = useState(false)
  const activityLabel = `${folded.length} ${folded.length === 1 ? "activity" : "activities"}`
  return <div className="oa-react-work-group" role="listitem" data-work-group={groupKey}>
    <button
      className="oa-react-work-group-summary"
      type="button"
      aria-expanded={expanded}
      aria-controls={`${groupKey}:details`}
      onClick={() => setExpanded(value => !value)}
    >
      <ChevronRight aria-hidden="true" data-icon-name="ChevronRight" data-expanded={expanded ? "true" : "false"} />
      <strong>{running ? `+${folded.length} previous` : "Worked"}</strong>
      <span>{activityLabel}</span>
    </button>
    {expanded ? <div id={`${groupKey}:details`} role="list">
      {folded.map(entry => <MemoTimelineItemBoundary key={entry.key} record={entry} report={report} />)}
    </div> : null}
    {visible.map(entry => <MemoTimelineItemBoundary key={entry.key} record={entry} report={report} />)}
  </div>
}

const TimelineRecords = ({ records, report }: {
  readonly records: ReadonlyArray<ReactTimelineRecord>
  readonly report: IntentReporter
}): ReactElement => {
  const output: Array<ReactElement> = []
  for (let index = 0; index < records.length;) {
    const record = records[index]!
    if (!isWorkRecord(record)) {
      output.push(<MessageScrollerItem key={record.key} messageId={record.key} scrollAnchor={isUserRecord(record)}>
        <MemoTimelineItemBoundary record={record} report={report} />
      </MessageScrollerItem>)
      index += 1
      continue
    }
    const group: Array<ReactTimelineRecord> = []
    while (index < records.length && isWorkRecord(records[index]!)) group.push(records[index++]!)
    if (group.length === 1) {
      output.push(<MessageScrollerItem key={group[0]!.key} messageId={group[0]!.key}>
        <MemoTimelineItemBoundary record={group[0]!} report={report} />
      </MessageScrollerItem>)
      continue
    }
    const running = group.some(entry => entry.status === "running")
    const visible = running ? group.slice(-1) : []
    const folded = running ? group.slice(0, -1) : group
    // The first work record is the stable group identity. Appending streaming
    // work must not remount the disclosure and discard the reader's choice.
    const groupKey = `work:${group[0]!.key}`
    output.push(<MessageScrollerItem key={groupKey} messageId={groupKey}>
      <WorkGroupDisclosure groupKey={groupKey} folded={folded} visible={visible} running={running} report={report} />
    </MessageScrollerItem>)
  }
  return <>{output}</>
}

const TimelineScroller = (props: TimelineProps): ReactElement => {
  const viewportRef = useRef<HTMLDivElement>(null)
  const requestedEdge = useRef<"top" | "bottom" | null>(null)
  const previousLoadingEdge = useRef(props.loadingEdge)
  useEffect(() => {
    if (previousLoadingEdge.current !== props.loadingEdge && props.loadingEdge === null) requestedEdge.current = null
    previousLoadingEdge.current = props.loadingEdge
  }, [props.loadingEdge])
  const onScroll = (): void => {
    const element = viewportRef.current
    if (element === null || props.loadingEdge !== null || requestedEdge.current !== null) return
    if (props.offset > 0 && element.scrollTop < element.clientHeight * 1.5) {
      requestedEdge.current = "top"
      dispatch(props.report, "HistoryOlderRequested")
      return
    }
    const windowEnd = props.offset + props.loadedItemCount
    if (windowEnd < props.totalItems && element.scrollHeight - element.scrollTop - element.clientHeight < element.clientHeight * 1.5) {
      requestedEdge.current = "bottom"
      dispatch(props.report, "HistoryNewerRequested")
    }
  }
  const releaseReaderIntent = (): void => {
    const element = viewportRef.current
    if (element === null) return
    const event = typeof WheelEvent === "function"
      ? new WheelEvent("wheel", { bubbles: true, deltaY: 0 })
      : new Event("wheel", { bubbles: true })
    element.dispatchEvent(event)
  }
  return <MessageScroller className="oa-react-timeline-region" aria-label="Conversation timeline">
    <MessageScrollerViewport ref={viewportRef} className="oa-react-timeline-scroll"
      data-timeline-session={props.sessionKey} onScroll={onScroll}
      onPointerDownCapture={releaseReaderIntent} onSelect={releaseReaderIntent}
      aria-label={`${props.records.length} loaded conversation items of ${props.totalItems}`}>
      <MessageScrollerContent className="oa-react-timeline-content" aria-busy={props.working === true}>
        {props.loadingEdge === "top"
          ? <p className="oa-react-timeline-loading" role="status">Fetching earlier items…</p>
          : props.offset > 0
            ? <p className="oa-react-timeline-position">Showing items {props.offset + 1}–{props.offset + props.loadedItemCount} of {props.totalItems}</p>
            : null}
        <TimelineRecords records={props.records} report={props.report} />
        {props.working ? <MessageScrollerItem messageId="working-indicator"><div className="oa-react-working" role="status" aria-label="Codex is working">
          <span>Working</span><i /><i /><i />
        </div></MessageScrollerItem> : null}
        {props.loadingEdge === "bottom" ? <p className="oa-react-timeline-loading" role="status">Fetching newer items…</p> : null}
      </MessageScrollerContent>
    </MessageScrollerViewport>
    <MessageScrollerButton className="oa-react-new-activity" behavior="auto" aria-label="Jump to latest" title="Jump to latest" />
  </MessageScroller>
}

export const ReactTimeline = (props: TimelineProps): ReactElement =>
  <MessageScrollerProvider key={props.sessionKey} autoScroll defaultScrollPosition="last-anchor" scrollPreviousItemPeek={64}>
    <TimelineScroller {...props} />
  </MessageScrollerProvider>

export const ConversationTimeline = ({ page, notes, loadingEdge, working, workingDirectory, report }: {
  readonly page: CodexHistoryPage | null
  readonly notes: ReadonlyArray<DesktopNoteEntry>
  readonly loadingEdge: "top" | "bottom" | null
  readonly working?: boolean
  readonly workingDirectory: string | null
  readonly report: IntentReporter
}): ReactElement => {
  if (page === null && notes.length === 0) return <section className="oa-react-timeline-empty" aria-label="Conversation">
    <div className="oa-react-empty-conversation">
      <h2>Start a conversation with Codex</h2>
      <div className="oa-react-empty-working-directory" aria-label={workingDirectory === null ? "Working directory unavailable" : `Working directory: ${workingDirectory}`}>
        <Folder aria-hidden="true" data-icon-name="Folder" />
        <code title={workingDirectory ?? undefined}>{workingDirectory ?? "Working directory unavailable"}</code>
        <Button type="button" variant="ghost" size="sm"
          aria-label="Change working directory" title="Change working directory"
          onClick={() => dispatch(report, "DesktopWorkspacePickerRequested")}>Change</Button>
      </div>
    </div>
  </section>
  const records = page === null ? projectLocalTimelineRecords(notes) : projectReactTimelineRecords(page.items)
  return <ReactTimeline sessionKey={page?.selectedThreadRef ?? "local"} records={records}
    loadedItemCount={page?.items.length ?? records.length} offset={page?.offset ?? 0}
    totalItems={page?.totalItems ?? records.length} loadingEdge={loadingEdge} working={working} report={report} />
}
