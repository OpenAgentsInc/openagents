import type { KhalaCodeDesktopCodexThreadSummary } from "./codex-threads"

export type KhalaCodeDesktopSessionHarnessKind = "claude" | "codex"
export type KhalaCodeDesktopSessionCatalogScope = "app" | "all_home"

export type KhalaCodeDesktopSessionExactTotals = {
  readonly cachedInputTokens?: number | undefined
  readonly inputTokens?: number | undefined
  readonly outputTokens?: number | undefined
  readonly reasoningOutputTokens?: number | undefined
  readonly totalTokens: number
  readonly source: string
}

export type KhalaCodeDesktopSessionCatalogEntry = {
  readonly catalogEntryId: string
  readonly harnessKind: KhalaCodeDesktopSessionHarnessKind
  readonly sessionRef: string
  readonly threadRef: string | null
  readonly desktopSessionRef: string | null
  readonly lastTurnRef: string | null
  readonly title: string
  readonly preview: string
  readonly cwd: string | null
  readonly projectLabel: string
  readonly status: string
  readonly statusLabel: string
  readonly source: string
  readonly createdAt: number | null
  readonly updatedAt: number | null
  readonly recencyAt: number | null
  readonly exactTotals?: KhalaCodeDesktopSessionExactTotals | undefined
}

export type KhalaCodeDesktopSessionCatalogResult = {
  readonly ok: true
  readonly schemaVersion: "khala-code-desktop.session-catalog.v1"
  readonly scope: KhalaCodeDesktopSessionCatalogScope
  readonly entries: readonly KhalaCodeDesktopSessionCatalogEntry[]
  readonly diagnostics: readonly string[]
}

const harnessLabel = (kind: KhalaCodeDesktopSessionHarnessKind): string =>
  kind === "codex" ? "Codex" : "Claude"

const codexUuidIdPattern = /^(?:urn:uuid:)?(?:[0-9a-f]{32}|[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12})$/iu

export const isCodexAppServerThreadId = (value: string): boolean =>
  codexUuidIdPattern.test(value)

const hasLiveThreadSource = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): boolean =>
  entry.source === "codex_app_server_thread_list" ||
  entry.source === "codex_app_server_thread_projection" ||
  entry.source === "appServer" ||
  entry.source === "claude_sdk_list_sessions" ||
  entry.source === "claude_thread_projection"

/**
 * A catalog entry is only safe to offer as a clickable, resumable sidebar
 * row when either (a) it was confirmed live by the harness's own
 * `listThreads()` (a real, currently-listable rollout/session), or (b) it
 * carries a UUID-shaped thread/session id, which is the only id format the
 * Codex app-server and the Claude Agent SDK ever assign to a real resumable
 * thread. Entries sourced only from this desktop's lightweight local
 * bookkeeping store (`codex-sessions.json` / `claude-sessions.json`) can
 * reference a thread that was never actually persisted as a resumable
 * rollout (e.g. an interrupted first turn, or a non-UUID id from an
 * external test harness) - those must be marked non-resumable for BOTH
 * harnesses so the sidebar never offers a "session" that fails with
 * "couldn't be opened" the moment the user clicks it.
 */
const isResumableCatalogEntry = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): boolean => {
  if (hasLiveThreadSource(entry)) return true
  const threadRef = entry.threadRef ?? entry.sessionRef
  return isCodexAppServerThreadId(threadRef)
}

const threadIdForDisplay = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): string =>
  isResumableCatalogEntry(entry)
    ? entry.threadRef ?? entry.sessionRef
    : entry.catalogEntryId

const genericFallbackTitle = (
  kind: KhalaCodeDesktopSessionHarnessKind,
): string => `${harnessLabel(kind)} session`

export const sessionCatalogEntryToThreadSummary = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): KhalaCodeDesktopCodexThreadSummary => {
  const threadId = threadIdForDisplay(entry)
  const resumable = isResumableCatalogEntry(entry)
  const fallbackTitle = genericFallbackTitle(entry.harnessKind)
  return {
    id: threadId,
    sessionId: entry.sessionRef,
    title: resumable || entry.title !== fallbackTitle ? entry.title : `Stored ${fallbackTitle}`,
    preview: entry.preview,
    cwd: entry.cwd,
    projectLabel: entry.projectLabel,
    status: entry.status,
    statusLabel: resumable ? entry.statusLabel : "stored local record",
    modelProvider: entry.harnessKind,
    source: entry.source,
    forkedFromId: null,
    parentThreadId: null,
    createdAt: entry.createdAt,
    updatedAt: entry.updatedAt,
    recencyAt: entry.recencyAt,
    badges: [harnessLabel(entry.harnessKind)],
    resumable,
    unavailableReason: resumable
      ? null
      : `Stored local ${harnessLabel(entry.harnessKind)} session metadata does not include a current app-server UUID thread id.`,
  }
}
