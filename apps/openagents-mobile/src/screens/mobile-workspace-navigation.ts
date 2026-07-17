import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"

import type { MobileControllerDirectory } from "../coding/mobile-controller-directory"
import type { MobileConversationThreadSummary } from "../conversation/mobile-conversation"

export const MOBILE_WORKSPACE_MAX_ROWS = 200
export const MOBILE_WORKSPACE_MAX_PROJECT_FILTERS = 12
export const MOBILE_WORKSPACE_MAX_SEARCH = 160

export type MobileWorkspaceStatusFilter =
  | "all"
  | "active"
  | "attention"
  | "idle"
  | "archived"

export type MobileWorkspaceRow = Readonly<{
  rowId: string
  kind: "conversation" | "coding_session" | "attention"
  threadRef: string
  sessionRef: string | null
  repositoryRef: string | null
  projectRef: string | null
  title: string
  projectLabel: string
  worktreeLabel: string | null
  activityAt: string
  recencyLabel: string
  state: "synced" | "active" | "idle" | "attention" | "recovery" | "archived"
  stateLabel: string
  selected: boolean
  attentionTarget: Readonly<{
    attentionRef: string
    threadRef: string
    turnRef: string
  }> | null
}>

export type MobileWorkspaceProjectFilter = Readonly<{
  id: string
  label: string
}>

export type MobileWorkspaceNavigationProjection = Readonly<{
  rows: ReadonlyArray<MobileWorkspaceRow>
  totalRowCount: number
  hiddenRowCount: number
  projectFilters: ReadonlyArray<MobileWorkspaceProjectFilter>
  selectedProjectAvailable: boolean
}>

const compactIdentity = (value: string): string => {
  const segment = value.split(/[/:]/u).filter(Boolean).at(-1) ?? value
  return segment.length <= 28 ? segment : `${segment.slice(0, 12)}…${segment.slice(-8)}`
}

const recencyLabel = (timestamp: string | null, now: Date): string => {
  if (timestamp === null) return "No messages"
  const parsed = new Date(timestamp)
  if (!Number.isFinite(parsed.getTime())) return "Activity unknown"
  const delta = Math.max(0, now.getTime() - parsed.getTime())
  if (delta < 60_000) return "Now"
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m`
  if (delta < 86_400_000) return `${Math.floor(delta / 3_600_000)}h`
  if (delta < 604_800_000) return `${Math.floor(delta / 86_400_000)}d`
  return parsed.toLocaleDateString("en-US", { month: "short", day: "numeric" })
}

const searchText = (row: MobileWorkspaceRow): string => [
  row.title,
  row.projectLabel,
  row.worktreeLabel,
  row.stateLabel,
  row.threadRef,
  row.sessionRef,
].filter((value): value is string => value !== null).join(" ").toLocaleLowerCase()

const statusMatches = (row: MobileWorkspaceRow, filter: MobileWorkspaceStatusFilter): boolean => {
  if (filter === "all") return row.state !== "archived"
  if (filter === "active") return row.state === "active"
  if (filter === "attention") return row.state === "attention" || row.state === "recovery"
  if (filter === "idle") return row.state === "idle" || row.state === "synced"
  return row.state === "archived"
}

const codingState = (
  state: MobileControllerDirectory["recent"][number]["state"],
  attention: MobileControllerDirectory["recent"][number]["attention"],
): Pick<MobileWorkspaceRow, "state" | "stateLabel"> => {
  if (attention === "needs_recovery") return { state: "recovery", stateLabel: "Needs recovery" }
  if (state === "active") return { state: "active", stateLabel: "Running" }
  return { state: "idle", stateLabel: "Ready" }
}

/**
 * Bounded local projection over data that is already confirmed for this owner.
 * It joins by exact refs but keeps those refs out of primary presentation.
 */
export const projectMobileWorkspaceNavigation = (input: Readonly<{
  threads: ReadonlyArray<MobileConversationThreadSummary>
  archivedThreads: ReadonlyArray<MobileConversationThreadSummary>
  directory: MobileControllerDirectory | null
  attention: ConfirmedRuntimeAttentionSnapshot | null
  activeThreadRef: string | null
  search: string
  status: MobileWorkspaceStatusFilter
  projectRef: string | null
  now?: Date
}>): MobileWorkspaceNavigationProjection => {
  const now = input.now ?? new Date()
  const activeThreads = new Map(input.threads.map(thread => [thread.threadRef, thread] as const))
  const sessions = input.directory?.authority === "confirmed" ? input.directory.recent : []
  const sessionThreadRefs = new Set(sessions.map(session => session.threadRef))
  const rows: Array<MobileWorkspaceRow> = []

  for (const session of sessions) {
    const thread = activeThreads.get(session.threadRef)
    const state = codingState(session.state, session.attention)
    rows.push({
      rowId: `session:${session.sessionRef}`,
      kind: "coding_session",
      threadRef: session.threadRef,
      sessionRef: session.sessionRef,
      repositoryRef: session.repositoryRef,
      projectRef: session.projectRef,
      title: thread?.title.trim() || session.repositoryName,
      projectLabel: session.repositoryName,
      worktreeLabel: compactIdentity(session.worktreeRef),
      activityAt: session.lastActiveAt,
      recencyLabel: recencyLabel(session.lastActiveAt, now),
      ...state,
      selected: input.activeThreadRef === session.threadRef,
      attentionTarget: null,
    })
  }

  for (const thread of input.threads) {
    if (sessionThreadRefs.has(thread.threadRef)) continue
    rows.push({
      rowId: `thread:${thread.threadRef}`,
      kind: "conversation",
      threadRef: thread.threadRef,
      sessionRef: null,
      repositoryRef: null,
      projectRef: null,
      title: thread.title,
      projectLabel: "Chats",
      worktreeLabel: null,
      activityAt: thread.lastMessageAt ?? thread.updatedAt,
      recencyLabel: recencyLabel(thread.lastMessageAt ?? thread.updatedAt, now),
      state: "synced",
      stateLabel: "Synced",
      selected: input.activeThreadRef === thread.threadRef,
      attentionTarget: null,
    })
  }

  for (const thread of input.archivedThreads) {
    rows.push({
      rowId: `archived:${thread.threadRef}`,
      kind: "conversation",
      threadRef: thread.threadRef,
      sessionRef: null,
      repositoryRef: null,
      projectRef: null,
      title: thread.title,
      projectLabel: "Chats",
      worktreeLabel: null,
      activityAt: thread.lastMessageAt ?? thread.updatedAt,
      recencyLabel: recencyLabel(thread.lastMessageAt ?? thread.updatedAt, now),
      state: "archived",
      stateLabel: "Archived",
      selected: false,
      attentionTarget: null,
    })
  }

  if (input.attention?.issues.length === 0) {
    for (const item of input.attention.pending) {
      const session = sessions.find(value => value.threadRef === item.threadRef)
      const thread = activeThreads.get(item.threadRef)
      rows.push({
        rowId: `attention:${item.attentionRef}`,
        kind: "attention",
        threadRef: item.threadRef,
        sessionRef: session?.sessionRef ?? null,
        repositoryRef: session?.repositoryRef ?? null,
        projectRef: session?.projectRef ?? null,
        title: thread?.title.trim() || session?.repositoryName || "Pending request",
        projectLabel: session?.repositoryName ?? "Chats",
        worktreeLabel: session === undefined ? null : compactIdentity(session.worktreeRef),
        activityAt: item.requestedAt,
        recencyLabel: "Needs you",
        state: "attention",
        stateLabel: item.kind === "provider_question"
          ? "Question"
          : item.kind === "tool_approval"
            ? "Approval"
            : "Plan review",
        selected: false,
        attentionTarget: {
          attentionRef: item.attentionRef,
          threadRef: item.threadRef,
          turnRef: item.turnRef,
        },
      })
    }
  }

  const projects = new Map<string, MobileWorkspaceProjectFilter>()
  for (const session of sessions) {
    if (!projects.has(session.projectRef)) {
      projects.set(session.projectRef, { id: session.projectRef, label: session.repositoryName })
    }
  }
  const projectFilters = [...projects.values()]
    .sort((left, right) => left.label.localeCompare(right.label) || left.id.localeCompare(right.id))
    .slice(0, MOBILE_WORKSPACE_MAX_PROJECT_FILTERS)
  const query = input.search.trim().slice(0, MOBILE_WORKSPACE_MAX_SEARCH).toLocaleLowerCase()
  const selectedProjectAvailable = input.projectRef === null || projects.has(input.projectRef)
  const filtered = rows
    .filter(row => statusMatches(row, input.status))
    .filter(row => input.projectRef === null || row.projectRef === input.projectRef)
    .filter(row => query === "" || searchText(row).includes(query))
    .sort((left, right) => {
      const attentionOrder = Number(right.state === "attention" || right.state === "recovery") -
        Number(left.state === "attention" || left.state === "recovery")
      return attentionOrder || right.activityAt.localeCompare(left.activityAt) ||
        left.rowId.localeCompare(right.rowId)
    })
  const visible = filtered.slice(0, MOBILE_WORKSPACE_MAX_ROWS)
  return {
    rows: visible,
    totalRowCount: filtered.length,
    hiddenRowCount: Math.max(0, filtered.length - visible.length),
    projectFilters,
    selectedProjectAvailable,
  }
}
