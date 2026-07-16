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
 *   - command (T4 #8861) / toolCall (T7 #8864): wired to their typed cards.
 *   - fileChange (T5 #8862) / reasoning (T6 #8863) / approval (T9 #8866) /
 *     meter (T11 #8868) / compaction, sleep, review, hook (T12 #8869):
 *     render through the existing generic `DesktopWorkEntry` shell for now.
 *     Each Wave-2 lane replaces its own branch with the polished, typed
 *     card — never another lane's branch.
 */
import type { ReactElement } from "react"

import { DesktopAgentGroup, type DesktopAgentActivity, type DesktopAgentStatus } from "./agent-group.tsx"
import type { DesktopApprovalDecision } from "./approval-card.tsx"
import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopCommandCard } from "./command-card.tsx"
import { ContextMeter } from "./context-meter.tsx"
import { DesktopFileChangeCard } from "./file-change-card.tsx"
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
  outputCapReached?: boolean
  commandSource?: "agent" | "userShell" | "unifiedExecStartup" | "unifiedExecInteraction"
}>

export type WorkbenchFileChangeEntryDispatch = Readonly<{
  path: string
  kind: "add" | "delete" | "update"
  adds?: number
  dels?: number
  diff?: string
  diffCapReached?: boolean
}>

export type WorkbenchFileChangeDispatchItem = Readonly<{
  kind: "fileChange"
  source: WorkbenchDispatchSource
  status: WorkbenchDispatchStatus
  scope?: "item" | "turn"
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
  /**
   * mcp: connector/app-context badge (`McpToolCallAppContext`). Not yet
   * projected by `workbench-item-contract.ts` (#8859) — declared here
   * additively so the card renders it the moment a producer starts setting
   * it. See the T7 (#8864) issue comment for the wiring decision.
   */
  appContext?: string
  /**
   * mcp: latest `item/mcpToolCall/progress` tick
   * (`McpToolCallProgressNotification.message`) while the call is still
   * running. Not yet emitted by `codex-app-server-turn.ts` — declared here
   * additively for the same reason as `appContext` above.
   */
  progressMessage?: string
}>

/** Wire vocabulary mirror of `WorkbenchCollabAgentStatus` (workbench-item-contract.ts). */
export type WorkbenchCollabAgentStatusDispatch =
  | "pendingInit" | "running" | "interrupted" | "completed" | "errored" | "shutdown" | "notFound"

/** Wire vocabulary mirror of `WorkbenchSubAgentActivityKind`. */
export type WorkbenchSubAgentActivityKindDispatch = "started" | "interacted" | "interrupted"

export type WorkbenchAgentChildDispatch = Readonly<{
  threadRef: string
  status: WorkbenchCollabAgentStatusDispatch
  nickname?: string
}>

export type WorkbenchAgentDispatchItem = Readonly<{
  kind: "agent"
  source: WorkbenchDispatchSource
  tool?: string
  prompt?: string
  status: WorkbenchDispatchStatus
  childRefs?: ReadonlyArray<string>
  /** Per-child status (#8867 T10) — one row per collab agent or the single subAgentActivity target. */
  children?: ReadonlyArray<WorkbenchAgentChildDispatch>
  /** Present when this item projects a subAgentActivity ping rather than a collabAgentToolCall. */
  activityKind?: WorkbenchSubAgentActivityKindDispatch
  agentPath?: string
}>

export type WorkbenchPlanDispatchItem = Readonly<{
  kind: "plan"
  source: WorkbenchDispatchSource
  entries: ReadonlyArray<DesktopPlanEntry>
  /**
   * Free-form plan narrative (T8 #8865 unification). The `plan` ThreadItem
   * (collaboration-mode write-ups) carries this instead of structured
   * entries; `turn/plan/updated` and history rows carry entries instead. A
   * plan item may carry either, or both.
   */
  prose?: string
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

/** collabAgentToolCall.tool -> the short operation verb `DesktopAgentGroup` brackets as `[SPAWN]`/`[SEND]`/etc. */
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

/** CollabAgentStatus -> the coarse icon/tone bucket `DesktopAgentRow` already understands. */
const toDesktopAgentStatusFromCollab = (status: WorkbenchCollabAgentStatusDispatch): DesktopAgentStatus => {
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
const collabStatusLabel = (status: WorkbenchCollabAgentStatusDispatch): string =>
  status.replace(/([a-z])([A-Z])/g, "$1 $2").toUpperCase()

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
 * component-audit gap list) — but only message/plan/agent/notice/command/
 * toolCall render through their intended real card today; the rest render
 * the generic `DesktopWorkEntry` shell until their Wave-2 lane lands (see
 * the module-level status list above).
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
      return <DesktopPlanCard entries={planItem.entries} itemKey={context.itemKey} prose={planItem.prose} />
    }

    case "agent": {
      const agentItem: WorkbenchAgentDispatchItem = item
      const operation = agentItem.tool === undefined ? undefined : agentOperationTag(agentItem.tool)
      const agents: ReadonlyArray<DesktopAgentActivity> = agentItem.children !== undefined && agentItem.children.length > 0
        ? agentItem.children.map(child => ({
            agentKey: child.threadRef,
            detail: agentItem.activityKind !== undefined ? "" : (agentItem.prompt ?? ""),
            name: child.nickname ?? child.threadRef,
            role: agentItem.activityKind !== undefined ? "Subagent activity" : "Delegated agent",
            status: toDesktopAgentStatusFromCollab(child.status),
            statusLabel: collabStatusLabel(child.status),
            ...(agentItem.agentPath === undefined ? {} : { path: agentItem.agentPath }),
            ...(agentItem.activityKind === undefined ? {} : { activityKind: agentItem.activityKind }),
          }))
        : [{
            agentKey: context.itemKey,
            detail: agentItem.prompt ?? "",
            name: agentItem.tool ?? "Agent",
            role: "agent",
            status: toAgentStatus(agentItem.status),
            ...(agentItem.agentPath === undefined ? {} : { path: agentItem.agentPath }),
            ...(agentItem.activityKind === undefined ? {} : { activityKind: agentItem.activityKind }),
          }]
      return <DesktopAgentGroup
        agents={agents}
        itemKey={context.itemKey}
        operation={operation}
        prompt={agentItem.prompt}
      />
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

    case "command": {
      const commandItem: WorkbenchCommandDispatchItem = item
      return <DesktopCommandCard
        command={commandItem.command}
        commandSource={commandItem.commandSource}
        cwd={commandItem.cwd}
        durationMs={commandItem.durationMs}
        exitCode={commandItem.exitCode}
        itemKey={context.itemKey}
        output={commandItem.outputTail}
        outputCapReached={commandItem.outputCapReached}
        status={toActivityStatus(commandItem.status)}
      />
    }

    case "fileChange": {
      const fileChangeItem: WorkbenchFileChangeDispatchItem = item
      return <DesktopFileChangeCard
        changes={fileChangeItem.changes.map(change => ({
          ...(change.adds === undefined ? {} : { additions: change.adds }),
          ...(change.dels === undefined ? {} : { deletions: change.dels }),
          ...(change.diff === undefined ? {} : { diff: change.diff }),
          ...(change.diffCapReached === undefined ? {} : { diffCapReached: change.diffCapReached }),
          kind: change.kind,
          path: change.path,
        }))}
        itemKey={context.itemKey}
        {...(fileChangeItem.scope === undefined ? {} : { scope: fileChangeItem.scope })}
        status={toActivityStatus(fileChangeItem.status)}
      />
    }

    // T7 (#8864): the structured payload (args table, result/error, duration,
    // web query+resultCount, image path) drives `DesktopToolCallCard`'s own
    // per-`callKind` title/summary/meta/body computation — see
    // `tool-call-card.tsx`. `appContext`/`progressMessage` ride through
    // untouched for forward-compatibility; no current producer sets them.
    case "toolCall": {
      const toolCallItem: WorkbenchToolCallDispatchItem = item
      const toolKind: DesktopToolKind = toolCallItem.callKind
      return <DesktopToolCallCard
        appContext={toolCallItem.appContext}
        args={toolCallItem.args}
        durationMs={toolCallItem.durationMs}
        errorMessage={toolCallItem.errorMessage}
        itemKey={context.itemKey}
        namespace={toolCallItem.namespace}
        path={toolCallItem.path}
        progressMessage={toolCallItem.progressMessage}
        query={toolCallItem.query}
        resultCount={toolCallItem.resultCount}
        resultSnippet={toolCallItem.resultSnippet}
        server={toolCallItem.server}
        status={toActivityStatus(toolCallItem.status)}
        tool={toolCallItem.tool}
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

    // Historical/inspector rendering of a past usage snapshot (T11 #8868).
    // The LIVE meter does not mount through this per-record dispatch path —
    // a meter is a persistent header/rail widget, not one chat message — so
    // this branch exists only so a "meter" item that appears in history
    // (e.g. a rollout-replayed snapshot) still renders honestly instead of
    // falling back to the generic work-entry shell. See
    // `./context-meter.tsx` and `./header.tsx` for the live mount point.
    case "meter": {
      const meterItem: WorkbenchMeterDispatchItem = item
      return <ContextMeter
        historical
        itemKey={context.itemKey}
        usage={{
          ...(meterItem.inputTokens === undefined ? {} : { inputTokens: meterItem.inputTokens }),
          ...(meterItem.cachedInputTokens === undefined ? {} : { cachedInputTokens: meterItem.cachedInputTokens }),
          ...(meterItem.outputTokens === undefined ? {} : { outputTokens: meterItem.outputTokens }),
          ...(meterItem.reasoningTokens === undefined ? {} : { reasoningTokens: meterItem.reasoningTokens }),
          ...(meterItem.totalTokens === undefined ? {} : { totalTokens: meterItem.totalTokens }),
        }}
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
