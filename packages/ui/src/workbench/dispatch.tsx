/**
 * Per-variant dispatch table (#8860, epic #8857 Wave 1).
 *
 * Maps a harness-neutral workbench item's `kind` to the shared component
 * that renders it, so each Wave-2 lane (T4-T12) can extend exactly ONE
 * branch of `dispatchWorkbenchItem` plus its own new module, without any two
 * lanes touching the same file.
 *
 * `WorkbenchDispatchItem` is a STRUCTURAL mirror of the `WorkbenchItem`
 * Effect Schema union defined in
 * `apps/openagents-desktop/src/workbench-item-contract.ts` (#8859). It is
 * declared locally rather than imported so `@openagentsinc/ui` — a shared,
 * host-agnostic package — never depends on an application package; any real
 * `WorkbenchItem` value from that contract satisfies this shape structurally
 * and can be passed straight through.
 *
 * Wiring status (component map, `docs/fable/autopilot-ui-component-audit.md`
 * §5):
 *   - message / plan / agent / notice: real shared components (already
 *     wired upstream of this table).
 *   - command (T4 #8861) / fileChange (T5 #8862) / reasoning (T6 #8863) /
 *     toolCall (T7 #8864) / approval (T9 #8866) / meter (T11 #8868) /
 *     compaction, sleep, review, hook (T12 #8869): render through the
 *     existing generic `DesktopWorkEntry` (or `DesktopToolCallCard` for
 *     toolCall) shell for now. Each Wave-2 lane replaces its own branch with
 *     the polished, typed card — never another lane's branch.
 */
import type { ReactElement } from "react"

import { DesktopAgentGroup, type DesktopAgentActivity, type DesktopAgentStatus } from "./agent-group.tsx"
import type { DesktopApprovalDecision } from "./approval-card.tsx"
import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopPlanCard, type DesktopPlanEntry } from "./plan-card.tsx"
import { DesktopTimelineMessage } from "./message.tsx"
import { DesktopTimelineNotice } from "./notice.tsx"
import { DesktopToolCallCard, type DesktopToolKind } from "./tool-call-card.tsx"
import { DesktopWorkEntry } from "./work-entry.tsx"

/** Which harness observed the item — mirrors `WorkbenchItemSource`. */
export type WorkbenchDispatchSource = "codex" | "claude" | "local"

/** Normalized lifecycle status — mirrors `WorkbenchItemStatus`. */
export type WorkbenchDispatchStatus = "in_progress" | "completed" | "failed" | "declined"

export type WorkbenchMessageDispatchItem = Readonly<{
  kind: "message"
  source: WorkbenchDispatchSource
  role: "user" | "assistant" | "system"
  text: string
  phase?: string
  citation?: string
}>

export type WorkbenchReasoningDispatchItem = Readonly<{
  kind: "reasoning"
  source: WorkbenchDispatchSource
  summary: string
}>

export type WorkbenchCommandDispatchItem = Readonly<{
  kind: "command"
  source: WorkbenchDispatchSource
  command: string
  cwd?: string
  status: WorkbenchDispatchStatus
  exitCode?: number | null
  durationMs?: number
  outputTail?: string
  commandSource?: "agent" | "userShell" | "unifiedExecStartup" | "unifiedExecInteraction"
}>

export type WorkbenchFileChangeEntryDispatch = Readonly<{
  path: string
  kind: "add" | "delete" | "update"
  adds?: number
  dels?: number
  diff?: string
}>

export type WorkbenchFileChangeDispatchItem = Readonly<{
  kind: "fileChange"
  source: WorkbenchDispatchSource
  status: WorkbenchDispatchStatus
  changes: ReadonlyArray<WorkbenchFileChangeEntryDispatch>
}>

export type WorkbenchToolCallDispatchItem = Readonly<{
  kind: "toolCall"
  source: WorkbenchDispatchSource
  callKind: "mcp" | "dynamic" | "web" | "image"
  tool: string
  server?: string
  namespace?: string
  args: ReadonlyArray<Readonly<{ key: string; value: string }>>
  resultSnippet?: string
  errorMessage?: string
  durationMs?: number
  status: WorkbenchDispatchStatus
  query?: string
  resultCount?: number
  path?: string
}>

export type WorkbenchAgentDispatchItem = Readonly<{
  kind: "agent"
  source: WorkbenchDispatchSource
  tool?: string
  prompt?: string
  status: WorkbenchDispatchStatus
  childRefs?: ReadonlyArray<string>
}>

export type WorkbenchPlanDispatchItem = Readonly<{
  kind: "plan"
  source: WorkbenchDispatchSource
  entries: ReadonlyArray<DesktopPlanEntry>
}>

export type WorkbenchApprovalDispatchItem = Readonly<{
  kind: "approval"
  source: WorkbenchDispatchSource
  status: WorkbenchDispatchStatus
  decision?: string
  detail?: string
}>

export type WorkbenchMeterDispatchItem = Readonly<{
  kind: "meter"
  source: WorkbenchDispatchSource
  inputTokens?: number
  cachedInputTokens?: number
  outputTokens?: number
  reasoningTokens?: number
  totalTokens?: number
}>

export type WorkbenchNoticeDispatchItem = Readonly<{
  kind: "notice"
  source: WorkbenchDispatchSource
  severity?: "info" | "warning" | "error"
  text: string
}>

export type WorkbenchCompactionDispatchItem = Readonly<{ kind: "compaction"; source: WorkbenchDispatchSource }>

export type WorkbenchSleepDispatchItem = Readonly<{
  kind: "sleep"
  source: WorkbenchDispatchSource
  durationMs: number
}>

export type WorkbenchReviewDispatchItem = Readonly<{
  kind: "review"
  source: WorkbenchDispatchSource
  phase: "entered" | "exited"
  review: string
}>

export type WorkbenchHookDispatchItem = Readonly<{
  kind: "hook"
  source: WorkbenchDispatchSource
  text: string
}>

export type WorkbenchDispatchItem =
  | WorkbenchMessageDispatchItem
  | WorkbenchReasoningDispatchItem
  | WorkbenchCommandDispatchItem
  | WorkbenchFileChangeDispatchItem
  | WorkbenchToolCallDispatchItem
  | WorkbenchAgentDispatchItem
  | WorkbenchPlanDispatchItem
  | WorkbenchApprovalDispatchItem
  | WorkbenchMeterDispatchItem
  | WorkbenchNoticeDispatchItem
  | WorkbenchCompactionDispatchItem
  | WorkbenchSleepDispatchItem
  | WorkbenchReviewDispatchItem
  | WorkbenchHookDispatchItem

export type WorkbenchDispatchContext = Readonly<{
  /** Stable per-item key; carried onto `data-timeline-key` by every card. */
  itemKey: string
  /** Position in the timeline, used for the message aria-label only. */
  sequence?: number
}>

const toActivityStatus = (status: WorkbenchDispatchStatus): DesktopActivityStatus => {
  switch (status) {
    case "in_progress": return "running"
    case "completed": return "completed"
    case "failed": return "failed"
    case "declined": return "failed"
  }
}

const toAgentStatus = (status: WorkbenchDispatchStatus): DesktopAgentStatus => {
  switch (status) {
    case "in_progress": return "running"
    case "completed": return "completed"
    case "failed": return "failed"
    case "declined": return "failed"
  }
}

const toApprovalDecision = (item: WorkbenchApprovalDispatchItem): DesktopApprovalDecision => {
  if (item.decision === "approved") return "approved"
  if (item.decision === "denied" || item.status === "declined") return "denied"
  if (item.status === "completed") return "approved"
  return "pending"
}

const messageLabel = (role: WorkbenchMessageDispatchItem["role"]): string =>
  role === "user" ? "You" : role === "system" ? "System" : "Assistant"

/**
 * Renders one `WorkbenchDispatchItem` through its shared component. Every
 * branch is handled — nothing silently drops (design-spec §8 rule 7 /
 * component-audit gap list) — but only message/plan/agent/notice render
 * through their intended real card today; the rest render the generic
 * `DesktopWorkEntry`/`DesktopToolCallCard` shell until their Wave-2 lane
 * lands (see the module-level status list above).
 */
export const dispatchWorkbenchItem = (
  item: WorkbenchDispatchItem,
  context: WorkbenchDispatchContext,
): ReactElement => {
  switch (item.kind) {
    case "message":
      return <DesktopTimelineMessage
        itemKey={context.itemKey}
        kind={`${item.role}_message`}
        label={messageLabel(item.role)}
        sequence={context.sequence ?? 0}
        tone={item.role === "user" ? "user" : "assistant"}
      ><p>{item.text}</p></DesktopTimelineMessage>

    case "plan": {
      const planItem: WorkbenchPlanDispatchItem = item
      return <DesktopPlanCard entries={planItem.entries} itemKey={context.itemKey} />
    }

    case "agent": {
      const agentItem: WorkbenchAgentDispatchItem = item
      const agents: ReadonlyArray<DesktopAgentActivity> = [{
        agentKey: context.itemKey,
        detail: agentItem.prompt ?? "",
        name: agentItem.tool ?? "Agent",
        role: "agent",
        status: toAgentStatus(agentItem.status),
      }]
      return <DesktopAgentGroup agents={agents} itemKey={context.itemKey} />
    }

    case "notice":
      return <DesktopTimelineNotice
        body={item.text}
        danger={item.severity === "error"}
        itemKey={context.itemKey}
        kind="notice"
      />

    // TODO(T6 #8863): replace with a streaming `ReasoningDisclosure` card
    // (ghost-text delta -> collapsed summary) instead of this generic shell.
    case "reasoning":
      return <DesktopWorkEntry
        body={item.summary}
        itemKey={context.itemKey}
        kind="reasoning"
        label="Reasoning"
        preview={item.summary}
        status="completed"
      />

    // TODO(T4 #8861): replace with `DesktopCommandCard` wired to cwd/exit
    // code/duration and the live `commandExecution/outputDelta` stream.
    case "command": {
      const commandItem: WorkbenchCommandDispatchItem = item
      return <DesktopWorkEntry
        body={commandItem.outputTail ?? ""}
        itemKey={context.itemKey}
        kind="command"
        label="Command"
        preview={commandItem.command}
        status={toActivityStatus(commandItem.status)}
      />
    }

    // TODO(T5 #8862): replace with `DesktopFileChangeCard` wired to per-file
    // diffs (`changes[].diff`) and the running `turn/diff/updated` summary.
    case "fileChange": {
      const fileChangeItem: WorkbenchFileChangeDispatchItem = item
      return <DesktopWorkEntry
        body={fileChangeItem.changes.map(change => change.path).join(", ")}
        itemKey={context.itemKey}
        kind="fileChange"
        label="File changes"
        preview={`${fileChangeItem.changes.length} ${fileChangeItem.changes.length === 1 ? "file" : "files"}`}
        status={toActivityStatus(fileChangeItem.status)}
      />
    }

    // TODO(T7 #8864): wire args table / result snippet / duration / query
    // per `callKind`; today this passes the bounded fields straight through
    // the existing generic tool-call shell.
    case "toolCall": {
      const toolCallItem: WorkbenchToolCallDispatchItem = item
      const toolKind: DesktopToolKind = toolCallItem.callKind
      return <DesktopToolCallCard
        body={toolCallItem.resultSnippet ?? toolCallItem.errorMessage ?? ""}
        itemKey={context.itemKey}
        label={toolCallItem.tool}
        meta={toolCallItem.server ?? toolCallItem.namespace}
        status={toActivityStatus(toolCallItem.status)}
        summary={toolCallItem.query ?? toolCallItem.tool}
        toolKind={toolKind}
      />
    }

    // TODO(T9 #8866): replace with `DesktopApprovalCard` wired to the real
    // interactive decision flow (`onDecision`) and a resource description.
    case "approval": {
      const approvalItem: WorkbenchApprovalDispatchItem = item
      return <DesktopWorkEntry
        body={approvalItem.detail ?? ""}
        itemKey={context.itemKey}
        kind="approval"
        label="Approval"
        preview={approvalItem.detail ?? ""}
        status={toActivityStatus(approvalItem.status)}
        statusLabel={toApprovalDecision(approvalItem) === "approved" ? "Approved" : toApprovalDecision(approvalItem) === "denied" ? "Denied" : "Pending"}
      />
    }

    // TODO(T11 #8868): replace with a `ContextMeter` quantized block bar
    // over `thread/tokenUsage/updated` + `AccountRateLimitsUpdated`.
    case "meter": {
      const meterItem: WorkbenchMeterDispatchItem = item
      return <DesktopWorkEntry
        body=""
        itemKey={context.itemKey}
        kind="meter"
        label="Usage"
        preview={meterItem.totalTokens === undefined ? "" : `${meterItem.totalTokens} tokens`}
        status="completed"
      />
    }

    // TODO(T12 #8869): replace compaction/sleep/review/hook with honest
    // mono ledger rows instead of the generic work-entry shell.
    case "compaction":
      return <DesktopWorkEntry
        body=""
        itemKey={context.itemKey}
        kind="compaction"
        label="Context compacted"
        preview=""
        status="completed"
      />

    case "sleep": {
      const sleepItem: WorkbenchSleepDispatchItem = item
      return <DesktopWorkEntry
        body=""
        itemKey={context.itemKey}
        kind="sleep"
        label="Sleep"
        preview={`${sleepItem.durationMs}ms`}
        status="completed"
      />
    }

    case "review": {
      const reviewItem: WorkbenchReviewDispatchItem = item
      return <DesktopWorkEntry
        body={reviewItem.review}
        itemKey={context.itemKey}
        kind="review"
        label={reviewItem.phase === "entered" ? "Entered review" : "Exited review"}
        preview={reviewItem.review}
        status="completed"
      />
    }

    case "hook": {
      const hookItem: WorkbenchHookDispatchItem = item
      return <DesktopWorkEntry
        body={hookItem.text}
        itemKey={context.itemKey}
        kind="hook"
        label="Hook"
        preview={hookItem.text}
        status="completed"
      />
    }
  }
}
