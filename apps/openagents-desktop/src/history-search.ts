/**
 * Free-text history session search (#8712 H4) — the pure ranking core.
 *
 * This is a REBUILDABLE local cache, never authority. Per the audit's
 * History/Discovery/Memory split, an index is a cache: it ranks over session
 * TITLES (always available from the catalog) and a bounded per-session CONTENT
 * projection (item text), and it never changes the loss-accounted catalog/page
 * truth. A content match carries the exact matching item so the UI can open the
 * session windowed on it (reusing the bottom-anchored restore-to-item flow).
 *
 * Matching is deterministic substring matching over normalized text. There is
 * no ad-hoc intent/keyword ROUTING here (which the workspace contract forbids):
 * this is bounded full-text filtering of an owner-local corpus, ranked by match
 * quality then recency.
 */
import type { CodexHistorySearchResult, CodexHistorySource } from "./codex-history-contract.ts"

export type HistorySearchItem = Readonly<{ itemRef: string; sequence: number; text: string }>

export type HistorySearchDocument = Readonly<{
  threadRef: string
  rootThreadRef: string
  source: CodexHistorySource
  title: string
  updatedAt: string
  /** Bounded content projection for this session; empty until indexed. */
  items: ReadonlyArray<HistorySearchItem>
}>

const TITLE_MATCH_WEIGHT = 1_000_000
const CONTENT_MATCH_WEIGHT = 500_000

const normalize = (value: string): string => value.toLowerCase().replace(/\s+/gu, " ").trim()

/** Recency tiebreak in [0,1): newer sessions rank above equal-match older ones. */
const recencyScore = (updatedAt: string): number => {
  const parsed = Date.parse(updatedAt)
  if (!Number.isFinite(parsed)) return 0
  // Map epoch millis into a bounded fraction; monotonic in time, < each weight.
  return Math.min(0.999, Math.max(0, parsed / 4_102_444_800_000)) // ~year 2100
}

const snippetAround = (text: string, at: number, needleLength: number): string => {
  const start = Math.max(0, at - 80)
  const end = Math.min(text.length, at + needleLength + 80)
  const core = text.slice(start, end).replace(/\s+/gu, " ").trim()
  return `${start > 0 ? "…" : ""}${core}${end < text.length ? "…" : ""}`.slice(0, 240)
}

/**
 * Rank documents against a query. Title matches rank above content matches;
 * within a tier, more-recent sessions rank first. Content matches return the
 * FIRST matching item (source order) as the open-at target.
 */
export const searchHistoryDocuments = (
  docs: ReadonlyArray<HistorySearchDocument>,
  query: string,
  limit = 40,
): ReadonlyArray<CodexHistorySearchResult> => {
  const needle = normalize(query)
  if (needle === "") return []
  const results: Array<CodexHistorySearchResult> = []
  for (const doc of docs) {
    const title = doc.title
    const recency = recencyScore(doc.updatedAt)
    if (normalize(title).includes(needle)) {
      results.push({ threadRef: doc.threadRef, rootThreadRef: doc.rootThreadRef, source: doc.source, title: title.slice(0, 160), matchKind: "title", matchItemRef: null, matchSequence: null, snippet: title.slice(0, 240), updatedAt: doc.updatedAt, score: TITLE_MATCH_WEIGHT + recency })
      continue
    }
    let matched: HistorySearchItem | null = null
    let matchAt = -1
    for (const item of doc.items) {
      const at = normalize(item.text).indexOf(needle)
      if (at >= 0) { matched = item; matchAt = at; break }
    }
    if (matched !== null) {
      results.push({ threadRef: doc.threadRef, rootThreadRef: doc.rootThreadRef, source: doc.source, title: title.slice(0, 160), matchKind: "content", matchItemRef: matched.itemRef, matchSequence: matched.sequence, snippet: snippetAround(matched.text, matchAt, needle.length), updatedAt: doc.updatedAt, score: CONTENT_MATCH_WEIGHT + recency })
    }
  }
  return results.sort((a, b) => b.score - a.score).slice(0, Math.max(1, Math.min(limit, 100)))
}
