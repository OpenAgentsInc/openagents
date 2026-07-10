/** Runs all local rollout filesystem work outside Electron's main process. */
import { parentPort, workerData } from "node:worker_threads"
import { readFileSync } from "node:fs"
import path from "node:path"

import { findRecentCodexThread, readRecentCodexHistory } from "./codex-history.ts"

type Request =
  | Readonly<{ kind: "list"; sessionsRoot: string; limit?: number }>
  | Readonly<{ kind: "detail"; sessionsRoot: string; id: string; messageLimit?: number }>

const request = workerData as Request
const names = (): ReadonlyMap<string, string> => {
  try {
    const entries = readFileSync(path.join(path.dirname(request.sessionsRoot), "session_index.jsonl"), "utf8")
      .split("\n")
      .flatMap(line => { try { return [JSON.parse(line) as { id?: unknown; thread_name?: unknown }] } catch { return [] } })
    const index = new Map<string, string>()
    for (const entry of entries) {
      if (typeof entry.id === "string" && typeof entry.thread_name === "string" && entry.thread_name.trim() !== "") index.set(entry.id, entry.thread_name.trim())
    }
    return index
  } catch { return new Map() }
}
const titleIndex = names()
const withIndexedTitle = <T extends { id: string; title: string }>(thread: T): T => ({ ...thread, title: titleIndex.get(thread.id) ?? thread.title })
const result = request.kind === "list"
  ? readRecentCodexHistory({ sessionsRoot: request.sessionsRoot, includeMessages: false, limit: request.limit }).map(withIndexedTitle)
  : (() => { const thread = findRecentCodexThread({ sessionsRoot: request.sessionsRoot, id: request.id, messageLimit: request.messageLimit }); return thread === null ? null : withIndexedTitle(thread) })()

parentPort?.postMessage({ ok: true, result })
