/** Runs all local rollout filesystem work outside Electron's main process. */
import { parentPort as workerParentPort } from "node:worker_threads"
import { readFileSync } from "node:fs"
import path from "node:path"

import { findRecentCodexThread, readRecentCodexHistory, recentCodexSessionFiles } from "./codex-history.ts"
import { buildHistorySearchDocuments, buildMergedHistoryGraphs, readMergedHistoryCatalog, readMergedHistoryPage, searchMergedHistory, type MergedHistoryGraphs } from "./merged-history.ts"
import type { HistorySearchDocument } from "./history-search.ts"

type Request =
  | Readonly<{ kind: "list"; sessionsRoot: string; limit?: number }>
  | Readonly<{ kind: "detail"; sessionsRoot: string; id: string; messageLimit?: number }>
  | Readonly<{ kind: "history_catalog"; sessionsRoot: string; claudeRoot?: string | null }>
  | Readonly<{ kind: "history_page"; sessionsRoot: string; threadRef: string; offset?: number; limit?: number; claudeRoot?: string | null }>
  | Readonly<{ kind: "history_search"; sessionsRoot: string; claudeRoot?: string | null; query: string; limit?: number }>

const names = (sessionsRoot: string): ReadonlyMap<string, string> => {
  try {
    const entries = readFileSync(path.join(path.dirname(sessionsRoot), "session_index.jsonl"), "utf8")
      .split("\n")
      .flatMap(line => { try { return [JSON.parse(line) as { id?: unknown; thread_name?: unknown }] } catch { return [] } })
    const index = new Map<string, string>()
    for (const entry of entries) {
      if (typeof entry.id === "string" && typeof entry.thread_name === "string" && entry.thread_name.trim() !== "") index.set(entry.id, entry.thread_name.trim())
    }
    return index
  } catch { return new Map() }
}
let sessionsRoot = ""
let claudeRoot: string | null = null
let titleIndex: ReadonlyMap<string, string> = new Map()
let fileIndex: ReadonlyMap<string, string> = new Map()
let mergedGraphs: MergedHistoryGraphs | null = null
// Rebuildable bounded content-index cache (H4). Reset whenever a root changes.
let searchIndex: Readonly<{ documents: ReadonlyArray<HistorySearchDocument>; indexedSessions: number; truncated: boolean }> | null = null
const withIndexedTitle = <T extends { id: string; title: string }>(thread: T): T => ({ ...thread, title: titleIndex.get(thread.id) ?? thread.title })

type HistoryWorkerInput = Readonly<{ id: number; request: Request }>
type UtilityParentPort = Readonly<{
  on: (event: "message", listener: (event: Readonly<{ data: HistoryWorkerInput }>) => void) => void
  postMessage: (value: unknown) => void
}>

// Electron 43's Node worker_threads path traps inside V8 ThreadIsolation on
// this supported macOS runtime when the history result crosses MessagePort.
// The main process therefore hosts history in an Electron utility process.
// Keep the Node Worker adapter for the standalone build contract, but select
// the process-isolated port whenever Electron supplies one.
const utilityParentPort = (process as NodeJS.Process & { parentPort?: UtilityParentPort | null }).parentPort ?? null
const onMessage = (listener: (input: HistoryWorkerInput) => void): void => {
  if (utilityParentPort !== null) {
    utilityParentPort.on("message", event => listener(event.data))
    return
  }
  workerParentPort?.on("message", listener)
}
const postMessage = (value: unknown): void => {
  if (utilityParentPort !== null) utilityParentPort.postMessage(value)
  else workerParentPort?.postMessage(value)
}

onMessage(input => {
  try {
    const request = input.request
    const nextClaudeRoot = "claudeRoot" in request ? request.claudeRoot ?? null : claudeRoot
    if (sessionsRoot !== request.sessionsRoot || claudeRoot !== nextClaudeRoot) { sessionsRoot = request.sessionsRoot; claudeRoot = nextClaudeRoot; titleIndex = names(sessionsRoot); fileIndex = new Map(); mergedGraphs = null; searchIndex = null }
    if ((request.kind === "history_catalog" || request.kind === "history_page" || request.kind === "history_search") && mergedGraphs === null) mergedGraphs = buildMergedHistoryGraphs(sessionsRoot, claudeRoot)
    const result = request.kind === "history_catalog"
      ? readMergedHistoryCatalog(sessionsRoot, claudeRoot, mergedGraphs!)
      : request.kind === "history_page"
      ? readMergedHistoryPage({ codexRoot: sessionsRoot, claudeRoot, threadRef: request.threadRef, offset: request.offset, limit: request.limit }, mergedGraphs!)
      : request.kind === "history_search"
      ? (() => { if (searchIndex === null) searchIndex = buildHistorySearchDocuments(sessionsRoot, claudeRoot, mergedGraphs!); return searchMergedHistory({ query: request.query, limit: request.limit }, searchIndex) })()
      : request.kind === "list"
      ? (() => { fileIndex = recentCodexSessionFiles(request.sessionsRoot); return readRecentCodexHistory({ sessionsRoot: request.sessionsRoot, includeMessages: false, limit: request.limit }).map(withIndexedTitle) })()
      : (() => { const thread = findRecentCodexThread({ sessionsRoot: request.sessionsRoot, id: request.id, messageLimit: request.messageLimit, file: fileIndex.get(request.id) }); return thread === null ? null : withIndexedTitle(thread) })()
    postMessage({ id: input.id, ok: true, result })
  } catch { postMessage({ id: input.id, ok: false, result: null }) }
})
