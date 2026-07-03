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

const isResumableCatalogEntry = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): boolean => {
  if (hasLiveThreadSource(entry)) return true
  if (entry.harnessKind !== "codex") return true
  const threadRef = entry.threadRef ?? entry.sessionRef
  return isCodexAppServerThreadId(threadRef)
}

const threadIdForDisplay = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): string =>
  isResumableCatalogEntry(entry)
    ? entry.threadRef ?? entry.sessionRef
    : entry.catalogEntryId

export const sessionCatalogEntryToThreadSummary = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): KhalaCodeDesktopCodexThreadSummary => {
  const threadId = threadIdForDisplay(entry)
  const resumable = isResumableCatalogEntry(entry)
  return {
    id: threadId,
    sessionId: entry.sessionRef,
    title: resumable || entry.title !== "Codex session" ? entry.title : "Stored Codex session",
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
      : "Stored local Codex session metadata does not include a current app-server UUID thread id.",
  }
}
