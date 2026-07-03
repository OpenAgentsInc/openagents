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

const resumableThreadIdFor = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): string =>
  entry.threadRef ?? entry.sessionRef

export const sessionCatalogEntryToThreadSummary = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
): KhalaCodeDesktopCodexThreadSummary => {
  const threadId = resumableThreadIdFor(entry)
  return {
    id: threadId,
    sessionId: entry.sessionRef,
    title: entry.title,
    preview: entry.preview,
    cwd: entry.cwd,
    projectLabel: entry.projectLabel,
    status: entry.status,
    statusLabel: entry.statusLabel,
    modelProvider: entry.harnessKind,
    source: entry.source,
    forkedFromId: null,
    parentThreadId: null,
    createdAt: entry.createdAt,
    updatedAt: entry.updatedAt,
    recencyAt: entry.recencyAt,
    badges: [harnessLabel(entry.harnessKind)],
  }
}
