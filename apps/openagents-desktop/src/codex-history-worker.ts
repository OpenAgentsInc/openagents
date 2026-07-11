/** Runs all local rollout filesystem work outside Electron's main process. */
import { parentPort } from "node:worker_threads"
import { readFileSync } from "node:fs"
import path from "node:path"

import { buildCodexHistoryGraph, findRecentCodexThread, readCodexHistoryCatalog, readCodexHistoryPage, readRecentCodexHistory, recentCodexSessionFiles, type CodexHistoryGraph } from "./codex-history.ts"

type Request =
  | Readonly<{ kind: "list"; sessionsRoot: string; limit?: number }>
  | Readonly<{ kind: "detail"; sessionsRoot: string; id: string; messageLimit?: number }>
  | Readonly<{ kind: "history_catalog"; sessionsRoot: string }>
  | Readonly<{ kind: "history_page"; sessionsRoot: string; threadRef: string; offset?: number; limit?: number }>

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
let titleIndex: ReadonlyMap<string, string> = new Map()
let fileIndex: ReadonlyMap<string, string> = new Map()
let historyGraph: CodexHistoryGraph | null = null
const withIndexedTitle = <T extends { id: string; title: string }>(thread: T): T => ({ ...thread, title: titleIndex.get(thread.id) ?? thread.title })
parentPort?.on("message", (input: Readonly<{ id: number; request: Request }>) => {
  try {
    const request = input.request
    if (sessionsRoot !== request.sessionsRoot) { sessionsRoot = request.sessionsRoot; titleIndex = names(sessionsRoot); fileIndex = new Map(); historyGraph = null }
    if ((request.kind === "history_catalog" || request.kind === "history_page") && historyGraph === null) historyGraph = buildCodexHistoryGraph(request.sessionsRoot)
    const result = request.kind === "history_catalog"
      ? readCodexHistoryCatalog(request.sessionsRoot, historyGraph!)
      : request.kind === "history_page"
      ? readCodexHistoryPage(request, historyGraph!)
      : request.kind === "list"
      ? (() => { fileIndex = recentCodexSessionFiles(request.sessionsRoot); return readRecentCodexHistory({ sessionsRoot: request.sessionsRoot, includeMessages: false, limit: request.limit }).map(withIndexedTitle) })()
      : (() => { const thread = findRecentCodexThread({ sessionsRoot: request.sessionsRoot, id: request.id, messageLimit: request.messageLimit, file: fileIndex.get(request.id) }); return thread === null ? null : withIndexedTitle(thread) })()
    parentPort?.postMessage({ id: input.id, ok: true, result })
  } catch { parentPort?.postMessage({ id: input.id, ok: false, result: null }) }
})
