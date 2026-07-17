/**
 * Merged Codex + Claude history surface (#8712 H3/H4). It presents both
 * providers through ONE catalog/page/search API without changing either
 * importer's loss accounting. Claude refs are `claude:`-namespaced, so page and
 * search routing is unambiguous; Codex refs stay bare. Import and search are
 * additive: a session from either source projects its own whole-conversation
 * completeness equation, untouched.
 */
import path from "node:path"

import type { CodexHistoryCatalog, CodexHistoryPage, CodexHistorySearchResponse } from "./codex-history-contract.ts"
import { buildCodexHistoryGraph, readCodexHistoryCatalog, readCodexHistoryHeadItems, readCodexHistoryPage, type CodexHistoryGraph } from "./codex-history.ts"
import { buildClaudeHistoryGraph, isClaudeThreadRef, readClaudeHistoryCatalog, readClaudeHistoryPage, type ClaudeHistoryGraph } from "./claude-history.ts"
import { searchHistoryDocuments, type HistorySearchDocument } from "./history-search.ts"

export type MergedHistoryGraphs = Readonly<{ codex: CodexHistoryGraph | null; claude: ClaudeHistoryGraph | null }>

const safe = <T>(build: () => T): T | null => { try { return build() } catch { return null } }

export const buildMergedHistoryGraphs = (codexRoot: string, claudeRoot: string | null): MergedHistoryGraphs => ({
  codex: codexRoot === "" ? null : safe(() => buildCodexHistoryGraph(codexRoot)),
  claude: claudeRoot === null || claudeRoot === "" ? null : safe(() => buildClaudeHistoryGraph(claudeRoot)),
})

const emptyCatalog: CodexHistoryCatalog = { roots: [], agents: [] }

export const readMergedHistoryCatalog = (codexRoot: string, claudeRoot: string | null, graphs?: MergedHistoryGraphs): CodexHistoryCatalog => {
  const built = graphs ?? buildMergedHistoryGraphs(codexRoot, claudeRoot)
  const codex = built.codex === null ? emptyCatalog : readCodexHistoryCatalog(codexRoot, built.codex)
  const claude = built.claude === null || claudeRoot === null ? emptyCatalog : readClaudeHistoryCatalog(claudeRoot, built.claude)
  return {
    roots: [...codex.roots, ...claude.roots].sort((a, b) => b.createdAt.localeCompare(a.createdAt) || a.threadRef.localeCompare(b.threadRef)).slice(0, 10_000),
    agents: [...codex.agents, ...claude.agents].slice(0, 10_000),
  }
}

export const readMergedHistoryPage = (input: Readonly<{ codexRoot: string; claudeRoot: string | null; threadRef: string; offset?: number; limit?: number }>, graphs?: MergedHistoryGraphs): CodexHistoryPage | null => {
  const built = graphs ?? buildMergedHistoryGraphs(input.codexRoot, input.claudeRoot)
  if (isClaudeThreadRef(input.threadRef)) {
    return built.claude === null || input.claudeRoot === null ? null : readClaudeHistoryPage({ projectsRoot: input.claudeRoot, threadRef: input.threadRef, offset: input.offset, limit: input.limit }, built.claude)
  }
  return built.codex === null ? null : readCodexHistoryPage({ sessionsRoot: input.codexRoot, threadRef: input.threadRef, offset: input.offset, limit: input.limit }, built.codex)
}

const CONTENT_SESSION_BUDGET = 60
const CONTENT_ITEM_CAP = 300

/**
 * Build the bounded content-index documents once (a rebuildable cache). Titles
 * cover EVERY root; content is projected only for the most-recently active
 * `CONTENT_SESSION_BUDGET` roots (bounded pages) so sidebar presentation order
 * cannot change search-index coverage or make first search block on the archive.
 */
export const buildHistorySearchDocuments = (
  codexRoot: string,
  claudeRoot: string | null,
  graphs: MergedHistoryGraphs,
): Readonly<{ documents: ReadonlyArray<HistorySearchDocument>; indexedSessions: number; truncated: boolean }> => {
  const catalog = readMergedHistoryCatalog(codexRoot, claudeRoot, graphs)
  const roots = catalog.roots
  const contentBudgetRefs = new Set([...roots]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || left.threadRef.localeCompare(right.threadRef))
    .slice(0, CONTENT_SESSION_BUDGET)
    .map(root => root.threadRef))
  // Codex file/cwd lookups for the bounded HEAD read (#8788/#8789: indexing
  // content through the whole-file page reader crashed or crawled on multi-GB
  // rollouts) and the searchable workspace label.
  const codexEntries = new Map((graphs.codex?.entries ?? []).map(entry => [entry.id, entry]))
  const documents = roots.map(root => {
    const indexed = contentBudgetRefs.has(root.threadRef)
    const codexEntry = root.source === "codex" ? codexEntries.get(root.threadRef) : undefined
    const rawItems = !indexed
      ? []
      : codexEntry !== undefined
        ? readCodexHistoryHeadItems(codexEntry.file, root.threadRef, CONTENT_ITEM_CAP)
        : readMergedHistoryPage({ codexRoot, claudeRoot, threadRef: root.threadRef, offset: 0, limit: CONTENT_ITEM_CAP }, graphs)?.items ?? []
    const items = rawItems
      .filter(item => item.summary.trim() !== "" && !item.summary.startsWith("[REDACTED:"))
      .map(item => ({ itemRef: item.itemRef, sequence: item.sequence, text: item.summary }))
    const cwd = codexEntry?.cwd ?? null
    return { threadRef: root.threadRef, rootThreadRef: root.threadRef, source: root.source, title: root.title, updatedAt: root.updatedAt, workspaceLabel: cwd === null ? null : path.basename(cwd), items }
  })
  return { documents, indexedSessions: contentBudgetRefs.size, truncated: roots.length > contentBudgetRefs.size }
}

export const searchMergedHistory = (
  input: Readonly<{ query: string; limit?: number }>,
  index: Readonly<{ documents: ReadonlyArray<HistorySearchDocument>; indexedSessions: number; truncated: boolean }>,
): CodexHistorySearchResponse => ({
  query: input.query.slice(0, 200),
  results: searchHistoryDocuments(index.documents, input.query, input.limit ?? 40),
  indexedSessions: index.indexedSessions,
  truncated: index.truncated,
})
