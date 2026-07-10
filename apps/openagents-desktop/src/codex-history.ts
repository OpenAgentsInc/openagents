/**
 * Read-only, deliberately narrow projection of local Codex JSONL rollouts.
 * It never returns source paths, raw events, tool calls, or non-conversation
 * payloads. Unknown relationship metadata is excluded rather than risking a
 * noisy sub-agent sidebar.
 */
import { readdirSync, readFileSync } from "node:fs"
import path from "node:path"

import type { DesktopMessage, DesktopThread } from "./chat-contract.ts"

const windowMs = 24 * 60 * 60 * 1000
const recentMessageLimit = 12

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

const readOne = (file: string): MutableThread | null => {
  let rows: unknown[]
  try { rows = readFileSync(file, "utf8").split("\n").filter(Boolean).map(line => JSON.parse(line)) } catch { return null }
  let thread: MutableThread | null = null
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
      thread = { id, createdAt: at, updatedAt: at, cwd: string(payload.cwd) ?? undefined, model: string(payload.model) ?? undefined, child: parent !== null || source.includes("subagent") || source.includes("side"), notes: [] }
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

export const readRecentCodexHistory = (input: Readonly<{ sessionsRoot: string; now?: Date }>): DesktopThread[] => {
  const cutoff = (input.now ?? new Date()).getTime() - windowMs
  return filesUnder(input.sessionsRoot)
    .map(readOne)
    .filter((thread): thread is MutableThread => thread !== null && !thread.child && Date.parse(thread.updatedAt) >= cutoff)
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .map(thread => ({ id: thread.id, title: thread.title ?? "Untitled Codex chat", createdAt: thread.createdAt, updatedAt: thread.updatedAt, cwd: thread.cwd, model: thread.model, notes: thread.notes.slice(-recentMessageLimit) }))
}

export const findRecentCodexThread = (input: Readonly<{ sessionsRoot: string; id: string; now?: Date }>): DesktopThread | null =>
  readRecentCodexHistory(input).find(thread => thread.id === input.id) ?? null
