/**
 * Read-only, deliberately narrow projection of local Codex JSONL rollouts.
 * It never returns source paths, raw events, tool calls, or non-conversation
 * payloads. Unknown relationship metadata is excluded rather than risking a
 * noisy sub-agent sidebar.
 */
import { closeSync, openSync, readSync, readdirSync, statSync } from "node:fs"
import path from "node:path"

import type { DesktopMessage, DesktopThread } from "./chat-contract.ts"

const windowMs = 24 * 60 * 60 * 1000
const recentMessageLimit = 5
const historyTailBytes = 96 * 1024
const summaryHeadBytes = 16 * 1024

type RecordValue = Record<string, unknown>
const object = (value: unknown): RecordValue | null =>
  typeof value === "object" && value !== null && !Array.isArray(value) ? value as RecordValue : null
const string = (value: unknown): string | null => typeof value === "string" && value.trim() !== "" ? value : null
const nested = (value: RecordValue, key: string): RecordValue | null => object(value[key])
const iso = (value: unknown): string | null => {
  const text = string(value)
  return text !== null && Number.isFinite(Date.parse(text)) ? text : null
}
const displayTime = (value: string): string => new Date(value).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
const titleFor = (value: string): string => value.replace(/\s+/g, " ").trim().slice(0, 80) || "Untitled Codex chat"

const filesUnder = (root: string): string[] => {
  const visit = (directory: string): string[] => {
    try {
      return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
        const child = path.join(directory, entry.name)
        if (entry.isDirectory()) return visit(child)
        return entry.isFile() && entry.name.endsWith(".jsonl") ? [child] : []
      })
    } catch { return [] }
  }
  return visit(root)
}

const changedSince = (file: string, cutoff: number): boolean => {
  try { return statSync(file).mtimeMs >= cutoff } catch { return false }
}

type MutableThread = {
  id: string
  createdAt: string
  updatedAt: string
  cwd?: string
  model?: string
  title?: string
  child: boolean
  notes: DesktopMessage[]
}

const firstMatch = (value: string, pattern: RegExp): string | null => pattern.exec(value)?.[1] ?? null
const partialSessionMeta = (file: string): MutableThread | null => {
  try {
    const descriptor = openSync(file, "r")
    let head: string
    try {
      const bytes = Buffer.alloc(Math.min(summaryHeadBytes, statSync(file).size))
      readSync(descriptor, bytes, 0, bytes.length, 0)
      head = bytes.toString("utf8")
    } finally { closeSync(descriptor) }
    const id = firstMatch(head, /"id"\s*:\s*"([^"]+)"/u)
    if (id === null) return null
    const modifiedAt = statSync(file).mtime.toISOString()
    const parent = firstMatch(head, /"parent_thread_id"\s*:\s*"([^"]+)"/u)
    return {
      id,
      createdAt: modifiedAt,
      updatedAt: modifiedAt,
      cwd: firstMatch(head, /"cwd"\s*:\s*"([^"]+)"/u) ?? undefined,
      child: parent !== null,
      notes: [],
    }
  } catch { return null }
}

/**
 * Large Codex rollouts can be tens of megabytes. History needs the opening
 * session metadata/title and trailing messages only, never the full trace.
 * Keeping this bounded also prevents a read-only sidebar refresh from
 * monopolizing the Electron main process or serializing an entire transcript.
 */
const boundedJsonLines = (file: string, includeMessages: boolean): unknown[] | null => {
  try {
    const size = statSync(file).size
    const text = !includeMessages
      ? (() => {
          const descriptor = openSync(file, "r")
          try {
            const head = Buffer.alloc(Math.min(summaryHeadBytes, size))
            readSync(descriptor, head, 0, head.length, 0)
            return head.toString("utf8").replace(/[^\n]*$/u, "")
          } finally { closeSync(descriptor) }
        })()
      : (() => {
          const descriptor = openSync(file, "r")
          try {
            const tailSize = Math.min(historyTailBytes, size)
            const tail = Buffer.alloc(tailSize)
            readSync(descriptor, tail, 0, tail.length, size - tail.length)
            // Discard the partial line at the beginning of the tail window.
            return tail.toString("utf8").replace(/^[^\n]*\n/u, "")
          } finally { closeSync(descriptor) }
        })()
    return text.split("\n").filter(Boolean).flatMap(line => {
      try { return [JSON.parse(line)] } catch { return [] }
    })
  } catch { return null }
}

const readOne = (file: string, includeMessages: boolean): MutableThread | null => {
  const rows = boundedJsonLines(file, includeMessages)
  if (rows === null) return null
  let thread: MutableThread | null = partialSessionMeta(file)
  for (const row of rows) {
    const envelope = object(row); const payload = envelope && nested(envelope, "payload")
    if (envelope === null || payload === null) continue
    const at = iso(envelope.timestamp) ?? new Date(0).toISOString()
    const payloadType = string(envelope.type)
    if (payloadType === "session_meta") {
      const id = string(payload.id) ?? string(payload.session_id) ?? string(payload.thread_id)
      if (id === null) continue
      const parent = string(payload.parent_thread_id) ?? string(payload.parentThreadId) ?? string(payload.parent_session_id)
      const source = string(payload.source)?.toLowerCase() ?? ""
      const modifiedAt = statSync(file).mtime.toISOString()
      thread = { id, createdAt: at, updatedAt: modifiedAt > at ? modifiedAt : at, cwd: string(payload.cwd) ?? undefined, model: string(payload.model) ?? undefined, child: parent !== null || source.includes("subagent") || source.includes("side"), notes: [] }
      continue
    }
    if (thread === null || payloadType !== "response_item") continue
    // Codex writes message items directly in payload today; older fixture
    // rollouts nest the item once more. Support both bounded shapes.
    const item = string(payload.type) === "message" ? payload : nested(payload, "payload")
    if (item === null || string(item.type) !== "message") continue
    const role = string(item.role)
    if (role !== "user" && role !== "assistant") continue
    const content = Array.isArray(item.content) ? item.content : []
    const text = content.map(part => {
      const entry = object(part)
      return entry === null ? null : string(entry.text) ?? string(entry.value)
    }).find((value): value is string => value !== null)
    if (text === undefined) continue
    thread.updatedAt = at
    if (role === "user" && thread.title === undefined) thread.title = titleFor(text)
    thread.notes.push({ key: `${thread.id}-${thread.notes.length}`, role, text: text.slice(0, 4_000), timestamp: displayTime(at) })
  }
  return thread
}

export const readRecentCodexHistory = (input: Readonly<{ sessionsRoot: string; now?: Date; includeMessages?: boolean; limit?: number }>): DesktopThread[] => {
  const cutoff = (input.now ?? new Date()).getTime() - windowMs
  return filesUnder(input.sessionsRoot)
    // Do not parse historic 10–100 MB rollouts just to learn that the file
    // itself has not changed in the selected time window.
    .filter(file => changedSince(file, cutoff))
    .map(file => readOne(file, input.includeMessages !== false))
    .filter((thread): thread is MutableThread => thread !== null && !thread.child && Date.parse(thread.updatedAt) >= cutoff)
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .slice(0, input.limit)
    .map(thread => ({ id: thread.id, title: thread.title ?? "Untitled Codex chat", createdAt: thread.createdAt, updatedAt: thread.updatedAt, cwd: thread.cwd, model: thread.model, notes: input.includeMessages === false ? [] : thread.notes.slice(-recentMessageLimit) }))
}

/** Cheap filename-only index for the persistent worker's selected-thread path. */
export const recentCodexSessionFiles = (sessionsRoot: string, now: Date = new Date()): ReadonlyMap<string, string> => {
  const cutoff = now.getTime() - windowMs
  const index = new Map<string, string>()
  for (const file of filesUnder(sessionsRoot)) {
    if (!changedSince(file, cutoff)) continue
    const id = /([0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12})\.jsonl$/iu.exec(file)?.[1]
    if (id !== undefined) index.set(id, file)
  }
  return index
}

export const findRecentCodexThread = (input: Readonly<{ sessionsRoot: string; id: string; now?: Date; messageLimit?: number; file?: string }>): DesktopThread | null => {
  const cutoff = (input.now ?? new Date()).getTime() - windowMs
  const file = input.file ?? filesUnder(input.sessionsRoot).filter(candidate => changedSince(candidate, cutoff)).find(candidate => {
    const summary = readOne(candidate, false)
    return summary?.id === input.id && !summary.child
  })
  const thread = file === undefined ? null : readOne(file, true)
  return thread === null || thread.child
    ? null
    : { id: thread.id, title: thread.title ?? "Untitled Codex chat", createdAt: thread.createdAt, updatedAt: thread.updatedAt, cwd: thread.cwd, model: thread.model, notes: thread.notes.slice(-(input.messageLimit ?? recentMessageLimit)) }
}
