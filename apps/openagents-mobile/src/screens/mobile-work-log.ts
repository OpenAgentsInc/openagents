import {
  Badge,
  Button,
  CodeBlock,
  CopyButton,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"
import type {
  ConfirmedAgentRun,
  ConfirmedAgentTimelineEvent,
  ConfirmedAgentTimelineItem,
} from "@openagentsinc/khala-sync-client"

import type { MobileAccessibilityProfile } from "./khala-core"

export const MOBILE_WORK_LOG_MAX_ITEMS = 100
export const MOBILE_WORK_LOG_COLLAPSED_ITEMS = 5

export type MobileWorkStatus = "running" | "success" | "failure" | "canceled" | "neutral"

export interface MobileWorkItem {
  readonly itemRef: string
  readonly kind: "connection" | "reasoning" | "tool" | "plan" | "usage" | "status" | "error"
  readonly summary: string
  readonly detail: string | null
  readonly fullDetail: string | null
  readonly status: MobileWorkStatus
  readonly createdAt: string
}

export interface MobileWorkGroup {
  readonly groupRef: string
  readonly runRef: string
  readonly summary: string
  readonly status: MobileWorkStatus
  readonly identityLabel: string
  readonly elapsedLabel: string | null
  readonly createdAt: string
  readonly totalItemCount: number
  readonly omittedItemCount: number
  readonly items: ReadonlyArray<MobileWorkItem>
}

type OwnerConversationToolCopy = Readonly<{
  running: string
  success: string
  failure: string
}>

/**
 * Closed, user-facing names for Sarah's admitted tool brokers. The runtime
 * event remains the source of truth; this table only keeps internal broker
 * names and receipt refs out of the conversation UI.
 */
const ownerConversationToolCopy: Readonly<Record<string, OwnerConversationToolCopy>> = {
  codex_workers_capacity: {
    running: "Checking Codex capacity…",
    success: "Codex capacity checked",
    failure: "Couldn't check Codex capacity",
  },
  codex_workers_start: {
    running: "Starting Codex workers…",
    success: "Codex worker request completed",
    failure: "Couldn't start Codex workers",
  },
  codex_workers_status: {
    running: "Checking Codex workers…",
    success: "Codex worker status loaded",
    failure: "Couldn't check Codex workers",
  },
  full_auto_status: {
    running: "Checking Full Auto…",
    success: "Full Auto status loaded",
    failure: "Couldn't check Full Auto",
  },
  full_auto_control: {
    running: "Sending the Full Auto command…",
    success: "Full Auto command sent",
    failure: "The Full Auto command failed",
  },
  sarah_harness_status: {
    running: "Inspecting Sarah's harness…",
    success: "Sarah's harness inspected",
    failure: "Couldn't inspect Sarah's harness",
  },
  sarah_harness_review_history: {
    running: "Reviewing Sarah's conversation history…",
    success: "Sarah's conversation history reviewed",
    failure: "Couldn't review Sarah's conversation history",
  },
}

const ownerConversationActivityItems = (
  group: MobileWorkGroup,
): ReadonlyArray<MobileWorkItem> =>
  group.items.filter(item => item.kind === "tool" || item.kind === "error")

/** True when a work group contains something Sarah should disclose in chat. */
export const hasOwnerConversationActivity = (group: MobileWorkGroup): boolean =>
  ownerConversationActivityItems(group).length > 0

const ownerConversationActivityCopy = (item: MobileWorkItem): Readonly<{
  label: string
  evidence: string
}> => {
  if (item.kind === "error") {
    return {
      label: "Sarah hit a problem while working",
      evidence: "Runtime error received",
    }
  }
  const copy = ownerConversationToolCopy[item.summary]
  if (copy === undefined) {
    return {
      label: item.status === "running"
        ? "Sarah is using a tool…"
        : item.status === "success"
          ? "Sarah finished using a tool"
          : "Sarah's tool call failed",
      evidence: item.status === "running" ? "Tool call in progress" : "Tool result received",
    }
  }
  return {
    label: item.status === "running"
      ? copy.running
      : item.status === "success"
        ? copy.success
        : copy.failure,
    evidence: item.status === "running" ? "Using an OpenAgents tool" : "Tool result received",
  }
}

/**
 * Sarah's work appears as terse conversation activity, not the coding work-log
 * card. Each row is backed by a confirmed tool.call/tool.result/tool.error
 * event and updates in place when the matching tool call settles.
 */
export const renderOwnerConversationActivity = (
  group: MobileWorkGroup,
): ReadonlyArray<View> => ownerConversationActivityItems(group).slice(-6).map(item => {
  const copy = ownerConversationActivityCopy(item)
  const failed = item.status === "failure"
  return Stack({
    key: `${group.groupRef}-${item.itemRef}-owner-activity`,
    direction: "column",
    gap: "0",
    style: { width: "full" },
    a11y: { role: "region", label: `${copy.label}. ${copy.evidence}.` },
  }, [
    Text({
      key: `${group.groupRef}-${item.itemRef}-owner-activity-label`,
      content: copy.label,
      variant: "body",
      color: failed ? "danger" : item.status === "running" ? "accent" : "textPrimary",
      weight: "medium",
    }),
    Text({
      key: `${group.groupRef}-${item.itemRef}-owner-activity-evidence`,
      content: copy.evidence,
      variant: "caption",
      color: failed ? "danger" : "textMuted",
    }),
  ])
})

const runtimeLabels: Readonly<Record<NonNullable<ConfirmedAgentRun["runtime"]>, string>> = {
  opencode_codex: "OpenCode Codex",
  codex: "Codex",
  claude_code: "Claude Code",
  openagents_native: "OpenAgents",
}

const backendLabels: Readonly<Record<NonNullable<ConfirmedAgentRun["backend"]>, string>> = {
  gcloud_vm: "managed computer",
  pylon: "Pylon",
  hosted: "hosted runtime",
}

const titleCase = (value: string): string => value
  .replaceAll("_", " ")
  .replace(/\b\w/g, letter => letter.toUpperCase())

const compactDetail = (value: string): string => {
  const compact = value.replace(/\s+/g, " ").trim()
  return compact.length <= 120 ? compact : `${compact.slice(0, 119)}…`
}

const boundedDetail = (value: string): string =>
  value.length <= 20_000 ? value : `${value.slice(0, 19_999)}…`

const elapsedMillis = (run: ConfirmedAgentRun): number | null => {
  const start = Date.parse(run.startedAt ?? run.createdAt)
  const terminal = run.status === "completed"
    ? run.completedAt
    : run.status === "failed"
      ? run.failedAt
      : run.status === "canceled"
        ? run.canceledAt
        : run.updatedAt
  const end = Date.parse(terminal ?? run.updatedAt)
  return Number.isFinite(start) && Number.isFinite(end) ? Math.max(0, end - start) : null
}

const formatElapsed = (milliseconds: number): string => {
  const seconds = Math.max(0, Math.round(milliseconds / 1_000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  if (minutes < 60) return remainingSeconds === 0 ? `${minutes}m` : `${minutes}m ${remainingSeconds}s`
  const hours = Math.floor(minutes / 60)
  const remainingMinutes = minutes % 60
  return remainingMinutes === 0 ? `${hours}h` : `${hours}h ${remainingMinutes}m`
}

const groupStatus = (run: ConfirmedAgentRun): MobileWorkStatus => {
  switch (run.status) {
    case "queued":
    case "running":
    case "waiting_for_input": return "running"
    case "completed": return "success"
    case "failed": return "failure"
    case "canceled": return "canceled"
  }
}

const groupSummary = (run: ConfirmedAgentRun, elapsed: string | null): string => {
  switch (run.status) {
    case "queued": return "Waiting to start"
    case "running": return elapsed === null ? "Working" : `Working · ${elapsed}`
    case "waiting_for_input": return elapsed === null ? "Waiting for input" : `Worked for ${elapsed} · waiting for input`
    case "completed": return elapsed === null ? "Work completed" : `Worked for ${elapsed}`
    case "failed": return elapsed === null ? "Work failed" : `Work failed after ${elapsed}`
    case "canceled": return elapsed === null ? "Work stopped" : `Stopped after ${elapsed}`
  }
}

const identityLabel = (run: ConfirmedAgentRun): string => {
  const runtime = run.runtime === undefined ? "Agent" : runtimeLabels[run.runtime]
  return run.backend === undefined ? runtime : `${runtime} · ${backendLabels[run.backend]}`
}

const modelLabels: Readonly<Record<string, string>> = {
  "gemma-4-31b-it": "Gemma 4 31B",
  "gemma-4-26b-a4b-it": "Gemma 4 26B",
  "gemini-3.5-flash": "Gemini 3.5 Flash",
}

const providerLabels: Readonly<Record<string, string>> = {
  "google-ai-studio": "Google AI Studio",
  "openagents-khala": "OpenAgents hosted",
}

const readableRef = (value: string): string =>
  value
    .replace(/^model\./u, "")
    .replaceAll("_", " ")
    .replaceAll("-", " ")
    .replace(/\b\w/gu, letter => letter.toUpperCase())

/** Human-readable projection of server-authored runtime identity. Model prose
 * never participates in this label. */
export const mobileTimelineSourceIdentityLabel = (
  source: NonNullable<ConfirmedAgentTimelineEvent["source"]>,
): string => {
  const model = source.modelRef === undefined
    ? null
    : modelLabels[source.modelRef] ?? readableRef(source.modelRef)
  const provider = source.providerRef === undefined
    ? null
    : providerLabels[source.providerRef] ?? readableRef(source.providerRef)
  if (model !== null && provider !== null) return `${model} · ${provider}`
  if (model !== null) return model
  if (provider !== null) return provider
  return source.lane === "hosted_khala" ? "OpenAgents hosted" : readableRef(source.lane)
}

const observedIdentityLabel = (
  run: ConfirmedAgentRun,
  events: ReadonlyArray<ConfirmedAgentTimelineEvent>,
): string => {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const observed = events[index]?.source
    if (observed !== undefined) return mobileTimelineSourceIdentityLabel(observed)
  }
  return identityLabel(run)
}

const workStatusForItem = (item: ConfirmedAgentTimelineItem): MobileWorkStatus => {
  switch (item.kind) {
    case "tool": return item.status === "called" ? "running" : item.status === "completed" ? "success" : "failure"
    case "plan": return item.status === "running" || item.status === "pending"
      ? "running"
      : item.status === "failed" || item.status === "error"
        ? "failure"
        : "success"
    case "terminal": return item.status === "completed" ? "success" : item.status === "failed" ? "failure" : "canceled"
    case "interrupted": return "canceled"
    case "error": return "failure"
    case "connected": return "success"
    case "reconnect": return "running"
    case "stale": return "failure"
    case "reasoning": return "neutral"
    case "heartbeat":
    case "usage": return "neutral"
    case "text":
    case "question":
    case "approval": return "neutral"
  }
}

const itemFromEvent = (event: ConfirmedAgentTimelineEvent): MobileWorkItem | null => {
  const item = event.item
  if (item == null) return null
  const common = {
    createdAt: event.createdAt,
    status: workStatusForItem(item),
  }
  switch (item.kind) {
    case "text":
    case "question":
    case "approval": return null
    case "plan":
      if (item.interactionRef !== undefined) return null
      return {
        ...common,
        itemRef: `plan:${item.stepRef}`,
        kind: "plan",
        summary: event.summary.trim() === "" ? "Plan step" : compactDetail(event.summary),
        detail: titleCase(item.status),
        fullDetail: null,
      }
    case "reasoning": {
      const fullDetail = boundedDetail(item.text)
      return {
        ...common,
        itemRef: `reasoning:${item.messageRef}`,
        kind: "reasoning",
        summary: "Reasoning",
        detail: compactDetail(fullDetail),
        fullDetail,
      }
    }
    case "connected": return {
      ...common,
      itemRef: `connection:${item.turnRef}`,
      kind: "connection",
      summary: "Connected",
      detail: item.lane,
      fullDetail: null,
    }
    case "tool": return {
      ...common,
      itemRef: `tool:${item.toolCallRef}`,
      kind: "tool",
      summary: item.toolName,
      detail: item.status === "called" ? "Running" : item.status === "completed" ? "Completed" : "Failed",
      fullDetail: null,
    }
    case "usage": {
      const parts = [
        item.inputTokens === undefined ? null : `${item.inputTokens.toLocaleString("en-US")} input`,
        item.outputTokens === undefined ? null : `${item.outputTokens.toLocaleString("en-US")} output`,
        item.totalTokens === undefined ? null : `${item.totalTokens.toLocaleString("en-US")} total`,
      ].filter((part): part is string => part !== null)
      return {
        ...common,
        itemRef: `usage:${event.runRef}`,
        kind: "usage",
        summary: "Token usage",
        detail: parts.length === 0 ? "Unreported" : parts.join(" · "),
        fullDetail: null,
      }
    }
    case "terminal": return {
      ...common,
      itemRef: `terminal:${event.runRef}`,
      kind: "status",
      summary: item.status === "completed" ? "Turn completed" : item.status === "failed" ? "Turn failed" : "Turn canceled",
      detail: null,
      fullDetail: null,
    }
    case "interrupted": return {
      ...common,
      itemRef: `interrupted:${event.eventRef}`,
      kind: "status",
      summary: "Turn interrupted",
      detail: null,
      fullDetail: null,
    }
    case "heartbeat":
    case "reconnect":
    case "stale": {
      const fullDetail = boundedDetail(item.detail)
      return {
        ...common,
        itemRef: `${item.kind}:${event.eventRef}`,
        kind: item.kind === "heartbeat" ? "connection" : "status",
        summary: item.kind === "heartbeat" ? "Provider active" : item.kind === "reconnect" ? "Reconnected" : "Connection stale",
        detail: compactDetail(fullDetail),
        fullDetail,
      }
    }
    case "error": {
      const fullDetail = boundedDetail(item.messageSafe)
      return {
        ...common,
        itemRef: `error:${event.eventRef}`,
        kind: "error",
        summary: "Runtime error",
        detail: compactDetail(fullDetail),
        fullDetail,
      }
    }
  }
}

const mergeItem = (previous: MobileWorkItem, next: MobileWorkItem): MobileWorkItem => {
  if (previous.kind !== "reasoning" || next.kind !== "reasoning") return next
  const fullDetail = boundedDetail(`${previous.fullDetail ?? ""}${next.fullDetail ?? ""}`)
  return {
    ...next,
    detail: compactDetail(fullDetail),
    fullDetail,
  }
}

export const projectMobileWorkGroup = (
  run: ConfirmedAgentRun | null,
  events: ReadonlyArray<ConfirmedAgentTimelineEvent>,
): MobileWorkGroup | null => {
  if (run === null) return null
  const orderedRefs: Array<string> = []
  const byRef = new Map<string, MobileWorkItem>()
  for (const event of events) {
    const item = itemFromEvent(event)
    if (item === null) continue
    const previous = byRef.get(item.itemRef)
    if (previous === undefined) orderedRefs.push(item.itemRef)
    byRef.set(item.itemRef, previous === undefined ? item : mergeItem(previous, item))
  }
  const allItems = orderedRefs.flatMap(ref => {
    const item = byRef.get(ref)
    return item === undefined ? [] : [item]
  })
  if (allItems.length === 0) return null
  const items = allItems.slice(-MOBILE_WORK_LOG_MAX_ITEMS)
  const elapsed = elapsedMillis(run)
  const elapsedLabel = elapsed === null ? null : formatElapsed(elapsed)
  return {
    groupRef: `work:${run.runRef}`,
    runRef: run.runRef,
    summary: groupSummary(run, elapsedLabel),
    status: groupStatus(run),
    identityLabel: observedIdentityLabel(run, events),
    elapsedLabel,
    createdAt: allItems[0]?.createdAt ?? run.createdAt,
    totalItemCount: allItems.length,
    omittedItemCount: Math.max(0, allItems.length - items.length),
    items,
  }
}

const statusLabel = (status: MobileWorkStatus): string => {
  switch (status) {
    case "running": return "Running"
    case "success": return "Done"
    case "failure": return "Failed"
    case "canceled": return "Stopped"
    case "neutral": return "Info"
  }
}

const statusTone = (status: MobileWorkStatus): "info" | "success" | "danger" | "neutral" =>
  status === "running" ? "info" : status === "success" ? "success" : status === "failure" ? "danger" : "neutral"

export const renderMobileWorkLog = (
  group: MobileWorkGroup,
  expanded: boolean,
  expandedItemRefs: Readonly<Record<string, boolean>>,
  accessibility: MobileAccessibilityProfile,
): View => {
  const hiddenBoundedCount = Math.max(0, group.items.length - MOBILE_WORK_LOG_COLLAPSED_ITEMS)
  const visibleItems = expanded ? group.items : group.items.slice(-MOBILE_WORK_LOG_COLLAPSED_ITEMS)
  return Stack({
    key: `${group.groupRef}-surface`,
    direction: "column",
    gap: "1",
    padding: "1",
    style: { width: "full" },
    a11y: { role: "region", label: `${group.identityLabel} work log, ${group.summary}` },
  }, [
    Stack({
      key: `${group.groupRef}-header`,
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full" },
    }, [
      Badge({
        key: `${group.groupRef}-status`,
        label: statusLabel(group.status),
        tone: statusTone(group.status),
      }),
      Text({
        key: `${group.groupRef}-summary`,
        content: group.summary,
        variant: "body",
        color: "textPrimary",
        weight: "medium",
        style: { flex: 1 },
      }),
    ]),
    Text({
      key: `${group.groupRef}-identity`,
      content: `${group.identityLabel} · ${group.totalItemCount} ${group.totalItemCount === 1 ? "activity" : "activities"}`,
      variant: "caption",
      color: "textMuted",
    }),
    ...(hiddenBoundedCount === 0 ? [] : [Button({
      key: `${group.groupRef}-toggle`,
      label: expanded ? "Show fewer activities" : `+${hiddenBoundedCount} previous ${hiddenBoundedCount === 1 ? "activity" : "activities"}`,
      variant: "ghost",
      onPress: IntentRef("WorkGroupToggled", StaticPayload({ groupRef: group.groupRef })),
      a11y: {
        label: expanded ? "Show fewer work activities" : `Show ${hiddenBoundedCount} previous work activities`,
        expanded,
      },
      style: { width: "full", minHeight: accessibility.minTouchTarget },
    })]),
    ...visibleItems.flatMap(item => {
      const itemExpanded = expandedItemRefs[item.itemRef] === true
      const canExpand = item.fullDetail !== null
      const rowLabel = `${statusLabel(item.status)} · ${item.summary}${item.detail === null ? "" : ` · ${item.detail}`}`
      const row = canExpand
        ? Button({
            key: `${group.groupRef}-${item.itemRef}`,
            label: rowLabel,
            variant: "ghost",
            onPress: IntentRef("WorkItemToggled", StaticPayload({ itemRef: item.itemRef })),
            a11y: {
              label: `${item.summary}. ${item.detail ?? statusLabel(item.status)}. ${statusLabel(item.status)}`,
              expanded: itemExpanded,
            },
            style: { width: "full", minHeight: accessibility.minTouchTarget },
          })
        : Stack({
            key: `${group.groupRef}-${item.itemRef}`,
            direction: "row",
            gap: "2",
            align: "center",
            style: { width: "full", minHeight: accessibility.minTouchTarget },
            a11y: {
              role: "group",
              label: `${item.summary}. ${item.detail ?? statusLabel(item.status)}. ${statusLabel(item.status)}`,
            },
          }, [
            Badge({
              key: `${group.groupRef}-${item.itemRef}-status`,
              label: statusLabel(item.status),
              tone: statusTone(item.status),
            }),
            Text({
              key: `${group.groupRef}-${item.itemRef}-label`,
              content: `${item.summary}${item.detail === null ? "" : ` · ${item.detail}`}`,
              variant: "caption",
              color: item.status === "failure" ? "danger" : "textPrimary",
              style: { flex: 1 },
            }),
          ])
      return [
        row,
        ...(itemExpanded && item.fullDetail !== null
          ? [Stack({
              key: `${group.groupRef}-${item.itemRef}-detail`,
              direction: "column",
              gap: "1",
              padding: "2",
              style: { width: "full", backgroundColor: "surfaceRaised", borderRadius: "md" },
              a11y: { role: "region", label: `${item.summary} full detail` },
            }, [
              CodeBlock({
                key: `${group.groupRef}-${item.itemRef}-detail-text`,
                lines: item.fullDetail.split("\n").slice(0, 400).map(line => ({
                  tokens: [{ kind: "plain" as const, text: line.slice(0, 1_000) }],
                })),
                style: { width: "full", borderRadius: "md", padding: "2" },
              }),
              CopyButton({
                key: `${group.groupRef}-${item.itemRef}-copy`,
                content: item.fullDetail,
                label: "Copy detail",
                accessibilityLabel: `Copy ${item.summary} detail`,
                size: "sm",
                variant: "ghost",
              }),
            ])]
          : []),
      ]
    }),
    ...(group.omittedItemCount === 0 ? [] : [Text({
      key: `${group.groupRef}-safety-bound`,
      content: `${group.omittedItemCount} older ${group.omittedItemCount === 1 ? "activity" : "activities"} withheld by the mobile safety bound`,
      variant: "caption",
      color: "textMuted",
    })]),
  ])
}
