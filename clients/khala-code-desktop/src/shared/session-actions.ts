import type {
  KhalaCodeDesktopCodexThreadSummary,
} from "./codex-threads"
import type {
  KhalaCodeDesktopMessage,
} from "./rpc"

export type KhalaCodeSessionActionKind =
  | "fork"
  | "share"
  | "unshare"
  | "archive"
  | "unarchive"
  | "restore_closed_tab"
  | "previous_session"
  | "next_session"
  | "previous_message"
  | "next_message"

export type KhalaCodeSessionActionCommandId =
  | "session.fork"
  | "session.share"
  | "session.unshare"
  | "session.archive"
  | "session.unarchive"
  | "session.restore_closed"
  | "session.previous"
  | "session.next"
  | "message.previous"
  | "message.next"

export type KhalaCodeSessionRuntimeBoundary =
  | "codex_app_server"
  | "khala_owned_server"
  | "pylon"
  | "local_only"

export type KhalaCodeSessionActionIntent = Readonly<{
  action: KhalaCodeSessionActionKind
  commandId: KhalaCodeSessionActionCommandId
  enabled: boolean
  reason: string
  runtimeBoundary: KhalaCodeSessionRuntimeBoundary
  threadId?: string
  targetThreadId?: string
}>

export type KhalaCodeClosedSessionTab = Readonly<{
  closedAt: number
  threadId: string
}>

export type KhalaCodeSessionActionProjection = Readonly<{
  activeThreadId: string | null
  activeThreadTitle: string | null
  closedTabs: readonly KhalaCodeClosedSessionTab[]
  intents: readonly KhalaCodeSessionActionIntent[]
  messageCount: number
  sessionCount: number
}>

export type KhalaCodeSessionActionProjectionInput = Readonly<{
  activeThreadId?: string | null
  closedTabs?: readonly KhalaCodeClosedSessionTab[]
  messages?: readonly KhalaCodeDesktopMessage[]
  sessions?: readonly KhalaCodeDesktopCodexThreadSummary[]
}>

export const KHALA_CODE_CLOSED_SESSION_TABS_STORAGE_KEY =
  "khala-code-desktop.closed-session-tabs.v1"

const CLOSED_SESSION_TAB_LIMIT = 12

const actionCommandId = (
  action: KhalaCodeSessionActionKind,
): KhalaCodeSessionActionCommandId => {
  switch (action) {
    case "fork":
      return "session.fork"
    case "share":
      return "session.share"
    case "unshare":
      return "session.unshare"
    case "archive":
      return "session.archive"
    case "unarchive":
      return "session.unarchive"
    case "restore_closed_tab":
      return "session.restore_closed"
    case "previous_session":
      return "session.previous"
    case "next_session":
      return "session.next"
    case "previous_message":
      return "message.previous"
    case "next_message":
      return "message.next"
  }
}

const isArchivedThread = (
  thread: KhalaCodeDesktopCodexThreadSummary | null,
): boolean =>
  thread !== null &&
  (thread.status === "archived" || thread.badges.includes("archived"))

const isThreadSupported = (
  thread: KhalaCodeDesktopCodexThreadSummary | null,
): boolean =>
  thread !== null && thread.resumable !== false

const neighborThreadId = (
  sessions: readonly KhalaCodeDesktopCodexThreadSummary[],
  activeThreadId: string | null,
  offset: -1 | 1,
): string | null => {
  if (activeThreadId === null) return null
  const activeIndex = sessions.findIndex(session => session.id === activeThreadId)
  if (activeIndex < 0) return null
  const target = sessions[activeIndex + offset]
  return target !== undefined && target.resumable !== false ? target.id : null
}

const intent = (
  input: Omit<KhalaCodeSessionActionIntent, "commandId">,
): KhalaCodeSessionActionIntent => ({
  ...input,
  commandId: actionCommandId(input.action),
})

export const projectKhalaCodeSessionActionIntents = (
  input: KhalaCodeSessionActionProjectionInput,
): KhalaCodeSessionActionProjection => {
  const activeThreadId = input.activeThreadId ?? null
  const sessions = input.sessions ?? []
  const messages = input.messages ?? []
  const activeThread = activeThreadId === null
    ? null
    : sessions.find(session => session.id === activeThreadId) ?? null
  const supported = isThreadSupported(activeThread)
  const archived = isArchivedThread(activeThread)
  const previousThreadId = neighborThreadId(sessions, activeThreadId, -1)
  const nextThreadId = neighborThreadId(sessions, activeThreadId, 1)
  const restoreThreadId = (input.closedTabs ?? [])[0]?.threadId ?? null
  const activeReason = activeThreadId === null
    ? "No active session"
    : activeThread?.unavailableReason ?? "Active session is not resumable in this runtime"
  const messageReason = messages.length > 1
    ? "Ready"
    : "Need at least two visible messages"

  return {
    activeThreadId,
    activeThreadTitle: activeThread?.title ?? null,
    closedTabs: input.closedTabs ?? [],
    messageCount: messages.length,
    sessionCount: sessions.length,
    intents: [
      intent({
        action: "fork",
        enabled: supported,
        reason: supported ? "Fork through the active Codex-compatible runtime" : activeReason,
        runtimeBoundary: "codex_app_server",
        ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
      }),
      intent({
        action: "share",
        enabled: false,
        reason: "Sharing requires an explicit safe backing path; private local transcripts and files stay local.",
        runtimeBoundary: "khala_owned_server",
        ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
      }),
      intent({
        action: "unshare",
        enabled: false,
        reason: "No Khala/Pylon share record is attached to this local session.",
        runtimeBoundary: "khala_owned_server",
        ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
      }),
      intent({
        action: "archive",
        enabled: supported && !archived,
        reason: !supported ? activeReason : archived ? "Session is already archived" : "Archive the active session",
        runtimeBoundary: "codex_app_server",
        ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
      }),
      intent({
        action: "unarchive",
        enabled: supported && archived,
        reason: !supported ? activeReason : archived ? "Restore the archived session" : "Active session is not archived",
        runtimeBoundary: "codex_app_server",
        ...(activeThreadId === null ? {} : { threadId: activeThreadId }),
      }),
      intent({
        action: "restore_closed_tab",
        enabled: (input.closedTabs ?? []).length > 0,
        reason: (input.closedTabs ?? []).length > 0 ? "Restore the most recently closed session tab" : "No closed sessions recorded",
        runtimeBoundary: "local_only",
        ...(restoreThreadId === null ? {} : { targetThreadId: restoreThreadId }),
      }),
      intent({
        action: "previous_session",
        enabled: previousThreadId !== null,
        reason: previousThreadId === null ? "No previous resumable session" : "Open the previous session",
        runtimeBoundary: "local_only",
        ...(previousThreadId === null ? {} : { targetThreadId: previousThreadId }),
      }),
      intent({
        action: "next_session",
        enabled: nextThreadId !== null,
        reason: nextThreadId === null ? "No next resumable session" : "Open the next session",
        runtimeBoundary: "local_only",
        ...(nextThreadId === null ? {} : { targetThreadId: nextThreadId }),
      }),
      intent({
        action: "previous_message",
        enabled: messages.length > 1,
        reason: messageReason,
        runtimeBoundary: "local_only",
      }),
      intent({
        action: "next_message",
        enabled: messages.length > 1,
        reason: messageReason,
        runtimeBoundary: "local_only",
      }),
    ],
  }
}

export const khalaCodeSessionActionIntentFor = (
  projection: KhalaCodeSessionActionProjection,
  action: KhalaCodeSessionActionKind,
): KhalaCodeSessionActionIntent | null =>
  projection.intents.find(intent => intent.action === action) ?? null

export const khalaCodeSessionNavigationTarget = (
  projection: KhalaCodeSessionActionProjection,
  action: "next_session" | "previous_session" | "restore_closed_tab",
): string | null =>
  khalaCodeSessionActionIntentFor(projection, action)?.targetThreadId ?? null

const parseClosedSessionTabs = (value: string | null): readonly KhalaCodeClosedSessionTab[] => {
  if (value === null) return []
  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) return []
    return parsed.flatMap(item => {
      if (item === null || typeof item !== "object" || Array.isArray(item)) return []
      const record = item as Record<string, unknown>
      return typeof record.threadId === "string" &&
        record.threadId.length > 0 &&
        typeof record.closedAt === "number" &&
        Number.isFinite(record.closedAt)
        ? [{ closedAt: record.closedAt, threadId: record.threadId }]
        : []
    })
  } catch {
    return []
  }
}

export const readKhalaCodeClosedSessionTabs = (
  storage: Pick<Storage, "getItem">,
): readonly KhalaCodeClosedSessionTab[] =>
  parseClosedSessionTabs(storage.getItem(KHALA_CODE_CLOSED_SESSION_TABS_STORAGE_KEY))

export const writeKhalaCodeClosedSessionTabs = (
  storage: Pick<Storage, "setItem">,
  tabs: readonly KhalaCodeClosedSessionTab[],
): void => {
  storage.setItem(
    KHALA_CODE_CLOSED_SESSION_TABS_STORAGE_KEY,
    JSON.stringify(tabs.slice(0, CLOSED_SESSION_TAB_LIMIT)),
  )
}

export const recordKhalaCodeClosedSessionTab = (
  storage: Pick<Storage, "getItem" | "setItem">,
  threadId: string,
  now: number = Date.now(),
): readonly KhalaCodeClosedSessionTab[] => {
  const next = [
    { closedAt: now, threadId },
    ...readKhalaCodeClosedSessionTabs(storage).filter(tab => tab.threadId !== threadId),
  ].slice(0, CLOSED_SESSION_TAB_LIMIT)
  writeKhalaCodeClosedSessionTabs(storage, next)
  return next
}

export const removeKhalaCodeClosedSessionTab = (
  storage: Pick<Storage, "getItem" | "setItem">,
  threadId: string,
): readonly KhalaCodeClosedSessionTab[] => {
  const next = readKhalaCodeClosedSessionTabs(storage)
    .filter(tab => tab.threadId !== threadId)
  writeKhalaCodeClosedSessionTabs(storage, next)
  return next
}
