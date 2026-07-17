import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Read-only, deliberately narrow projection of local Codex JSONL rollouts.
 * It never returns source paths, raw events, tool calls, or non-conversation
 * payloads. Unknown relationship metadata is excluded rather than risking a
 * noisy sub-agent sidebar.
 */
import { closeSync, openSync, readFileSync, readSync, readdirSync, statSync } from "node:fs"
import path from "node:path"

import type { DesktopMessage, DesktopThread } from "./chat-contract.ts"
import type { CodexHistoryAgent, CodexHistoryAgentPreview, CodexHistoryCatalog, CodexHistoryItem, CodexHistoryItemKind, CodexHistoryPage } from "./codex-history-contract.ts"
import { workbenchItemFromThreadItem, workbenchPlanItemFromEntries, type WorkbenchItem, type WorkbenchPlanEntryInput } from "./workbench-item-contract.ts"

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
export const codexAuthoredTitle = (value: string): string | null => {
  if (reservedTransportTitle(value)) return null
  const title = value.replace(/\s+/g, " ").trim().slice(0, 80)
  return title === "" ? null : title
}
const titleFor = (value: string): string => codexAuthoredTitle(value) ?? "Untitled Codex chat"
type CodexUserRecordKind = "authored" | "agent_metadata" | "plugin_metadata" | "environment_context"
const codexUserRecordKind = (role: string | null, value: string): CodexUserRecordKind => {
  if (role !== "user") return "authored"
  if (value.startsWith("# AGENTS.md instructions for ") && value.includes("\n<INSTRUCTIONS>\n")) return "agent_metadata"
  if (value.startsWith("<recommended_plugins>\n")) return "plugin_metadata"
  const trimmed = value.trim()
  return trimmed.startsWith("<environment_context>") && trimmed.endsWith("</environment_context>")
    ? "environment_context"
    : "authored"
}
const reservedTransportTitle = (value: string): boolean => {
  const trimmed = value.trimStart()
  return trimmed.startsWith("<environment_context>") ||
    trimmed.startsWith("<recommended_plugins>") ||
    trimmed.startsWith("# AGENTS.md instructions for ")
}

const filesUnder = (root: string, includeCompressed = false): string[] => {
  const visit = (directory: string): string[] => {
    try {
      return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
        const child = path.join(directory, entry.name)
        if (entry.isDirectory()) return visit(child)
        return entry.isFile() && (entry.name.endsWith(".jsonl") || (includeCompressed && entry.name.endsWith(".jsonl.zst"))) ? [child] : []
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
            // lastIndexOf, NOT /[^\n]*$/ — that regex is O(L^2) on a long
            // partial trailing line and can hang on a large head window.
            const raw = head.toString("utf8")
            return raw.slice(0, raw.lastIndexOf("\n") + 1)
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
    if (role === "user" && thread.title === undefined && codexUserRecordKind(role, text) === "authored") thread.title = titleFor(text)
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

// Provider-native historical projection (#8674). This deliberately coexists
// with the legacy local chat adapter above until all chat mutation call sites
// are moved off that adapter. It has no 24-hour or tail window.
type SessionIndexEntry = Readonly<{ file: string; id: string; parentId: string | null; createdAt: string; updatedAt: string; model: string | null; role: string | null; nickname:string|null; agentPath:string|null; sourceVersion:string|null; reasoning:string|null; cwd:string|null }>

const safeText = (value: unknown, limit = 20_000): string => typeof value === "string" ? value.slice(0, limit) : typeof value === "number" || typeof value === "boolean" ? String(value) : value === null || value === undefined ? "" : (()=>{try{return JSON.stringify(value).slice(0,limit)}catch{return ""}})()
const redactionPatterns = [
  /\b(?:sk|rk|pk|sess)-[A-Za-z0-9_-]{12,}\b/gu,
  /\bBearer\s+[A-Za-z0-9._~+\/-]{8,}=*/giu,
  /\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|authorization)\b\s*[:=]\s*[^\s,;]+/giu,
  /\b(?:[a-z]+\s+){11,23}[a-z]+\b/giu,
]
export const redactCodexHistoryText = (value: string): Readonly<{ text: string; redacted: boolean }> => {
  let text = value.slice(0, 20_000); let redacted = value.length > 20_000
  for (const pattern of redactionPatterns) text = text.replace(pattern, () => { redacted = true; return "[REDACTED]" })
  return { text, redacted }
}

const historyText = (file: string): string => file.endsWith(".zst") ? Buffer.from(Runtime.zstdDecompressSync(readFileSync(file))).toString("utf8") : readFileSync(file,"utf8")

/**
 * Stream complete JSONL lines from a rollout WITHOUT materializing the file
 * (2026-07-14 rc.10 incident, #8789: a real ~/.codex held a 4.5 GB rollout;
 * `readFileSync` ENOMEMed inside the catalog title scan, the whole graph
 * build threw, and the sidebar silently fell back to the 24-hour list under
 * an "all time" header). Reads fixed 8 MB chunks, carries the partial
 * trailing line as BYTES (multibyte characters never split across chunk
 * boundaries), and hands each complete non-empty line to `onLine` — return
 * `false` to stop early. `byteCap` bounds how far into the file the scan may
 * reach. A pathological line longer than `maxLineBytes` is flushed as one
 * (unparseable) line so memory stays bounded and the row still projects as an
 * accounted gap rather than crashing the process.
 */
const streamChunkBytes = 8 * 1024 * 1024
const maxLineBytes = 64 * 1024 * 1024
const streamRolloutLines = (file: string, onLine: (line: string) => boolean, byteCap?: number): void => {
  if (file.endsWith(".zst")) {
    // Archived .zst rollouts decompress whole-buffer today (they are written
    // compacted and orders of magnitude smaller than live rollouts); the cap
    // still bounds how much decoded text the scan walks.
    const text = historyText(file)
    const bounded = byteCap === undefined ? text : text.slice(0, byteCap)
    for (const line of bounded.split("\n")) { if (line !== "" && !onLine(line)) return }
    return
  }
  const size = statSync(file).size
  const cap = byteCap === undefined ? size : Math.min(size, byteCap)
  const descriptor = openSync(file, "r")
  try {
    const chunk = Buffer.alloc(Math.min(streamChunkBytes, Math.max(1, cap)))
    let position = 0
    let carry: Buffer = Buffer.alloc(0)
    while (position < cap) {
      const bytes = readSync(descriptor, chunk, 0, Math.min(chunk.length, cap - position), position)
      if (bytes <= 0) break
      position += bytes
      const merged = carry.length === 0 ? chunk.subarray(0, bytes) : Buffer.concat([carry, chunk.subarray(0, bytes)])
      const lastNewline = merged.lastIndexOf(0x0a)
      if (lastNewline < 0) {
        carry = maxLineBytes < merged.length ? Buffer.alloc(0) : Buffer.from(merged)
        if (carry.length === 0 && !onLine("[unparseable oversized line]")) return
        continue
      }
      const text = merged.subarray(0, lastNewline).toString("utf8")
      carry = Buffer.from(merged.subarray(lastNewline + 1))
      for (const line of text.split("\n")) { if (line !== "" && !onLine(line)) return }
    }
    // The final partial line is a complete record only at end-of-file.
    if (carry.length > 0 && position >= size) onLine(carry.toString("utf8"))
  } finally { closeSync(descriptor) }
}
const historyFiles = (sessionsRoot: string): string[] => {
  const active = filesUnder(sessionsRoot,true); const archived = filesUnder(path.join(path.dirname(sessionsRoot),"archived_sessions"),true)
  const seen = new Set<string>(); return [...active,...archived].filter(file => { const name=path.basename(file).replace(/\.zst$/u,""); if(seen.has(name))return false;seen.add(name);return true })
}
const firstLine = (file: string): RecordValue | null => {
  try {
    if (file.endsWith(".zst")) { const line=historyText(file).split("\n").find(Boolean); return line === undefined ? null : object(JSON.parse(line)) }
    const descriptor = openSync(file, "r")
    try {
      const buffer = Buffer.alloc(Math.min(1024 * 1024, statSync(file).size))
      readSync(descriptor, buffer, 0, buffer.length, 0)
      const line = buffer.toString("utf8").split("\n").find(Boolean)
      return line === undefined ? null : object(JSON.parse(line))
    } finally { closeSync(descriptor) }
  } catch { return null }
}

const indexAllSessions = (sessionsRoot: string): SessionIndexEntry[] => historyFiles(sessionsRoot).flatMap(file => {
  const envelope = firstLine(file); const payload = envelope === null ? null : object(envelope.payload)
  if (envelope === null || payload === null || envelope.type !== "session_meta") return []
  const id = string(payload.id) ?? string(payload.session_id) ?? string(payload.thread_id)
  if (id === null) return []
  const source = object(payload.source); const subagent = source === null ? null : object(source.subagent); const spawn = subagent === null ? null : object(subagent.thread_spawn)
  const parentId = string(payload.parent_thread_id) ?? string(payload.parentThreadId) ?? string(payload.parent_session_id) ?? (spawn === null ? null : string(spawn.parent_thread_id))
  const createdAt = iso(envelope.timestamp) ?? statSync(file).birthtime.toISOString()
  return [{ file, id, parentId, createdAt, updatedAt: statSync(file).mtime.toISOString(), model: string(payload.model), role: string(payload.agent_role) ?? string(payload.agent_type) ?? (spawn === null ? null : string(spawn.agent_role)), nickname:string(payload.agent_nickname)??(spawn===null?null:string(spawn.agent_nickname)),agentPath:string(payload.agent_path)??(spawn===null?null:string(spawn.agent_path)),sourceVersion:string(payload.multi_agent_version)??string(payload.history_mode),reasoning:string(payload.reasoning_effort),cwd:string(payload.cwd) }]
})

const titleIndex = (sessionsRoot: string): ReadonlyMap<string, string> => {
  try {
    const result = new Map<string, string>()
    for (const line of readFileSync(path.join(path.dirname(sessionsRoot), "session_index.jsonl"), "utf8").split("\n")) {
      if (line === "") continue
      try { const row = object(JSON.parse(line)); const id = row && string(row.id); const title = row && string(row.thread_name); if (id && title) result.set(id, title.slice(0, 160)) } catch { /* explicit malformed index omission */ }
    }
    return result
  } catch { return new Map() }
}

/**
 * Bounded head scan for a display title (#8789). The authored first user
 * message sits near the head of a rollout; scanning past this cap buys no
 * titles and previously (via a whole-file `readFileSync`) crashed the entire
 * catalog on multi-GB rollouts. Beyond-cap sessions fall back to the honest
 * "Untitled Codex chat" label instead of taking the catalog down with them.
 */
const titleScanByteCap = 8 * 1024 * 1024
const firstAuthoredTitle = (entry: SessionIndexEntry): string | null => {
  let title: string | null = null
  try {
    streamRolloutLines(entry.file, line => {
      try {
        const envelope = object(JSON.parse(line)); const payload = envelope === null ? null : object(envelope.payload)
        if (envelope?.type !== "response_item" || payload === null || string(payload.type) !== "message") return true
        const role = string(payload.role)
        const text = contentText(payload.content)
        if (role === "user" && text !== "" && codexUserRecordKind(role, text) === "authored") { title = titleFor(text); return false }
      } catch { /* malformed records cannot become display titles */ }
      return true
    }, titleScanByteCap)
  } catch { /* an unreadable file cannot cost the rest of the catalog */ }
  return title
}

const inferredStatus = (file: string): CodexHistoryAgent["status"] => {
  try {
    const text = file.endsWith(".zst") ? historyText(file).slice(-historyTailBytes) : (() => { const size=statSync(file).size; const descriptor=openSync(file,"r"); try { const buffer=Buffer.alloc(Math.min(historyTailBytes,size)); readSync(descriptor,buffer,0,buffer.length,size-buffer.length); return buffer.toString("utf8") } finally { closeSync(descriptor) } })()
    const rows=text.split("\n").flatMap(line=>{try{return [object(JSON.parse(line))]}catch{return []}}).filter((row):row is RecordValue=>row!==null)
    for(const row of rows.reverse()){const payload=object(row.payload);if(!payload)continue;const type=(string(payload.type)??string(row.type)??"").toLowerCase();const status=JSON.stringify(payload.status??"").toLowerCase();if(status.includes("not_found"))return "not_found";if(status.includes("errored"))return "errored";if(status.includes("interrupted"))return "interrupted";if(status.includes("shutdown"))return "shutdown";if(status.includes("waiting"))return "waiting";if(status.includes("running"))return "running";if(status.includes("completed"))return "completed";if(type==="turn_aborted")return "interrupted";if(type==="error")return "errored";if(type==="task_complete"||type==="turn_complete")return "completed";if(type==="task_started"||type==="turn_started")return "running"}
    return "completed"
  } catch { return "unknown" }
}

export type CodexHistoryGraph = Readonly<{ entries: SessionIndexEntry[]; agents: CodexHistoryAgent[] }>

export const buildCodexHistoryGraph = (sessionsRoot: string): CodexHistoryGraph => {
  const entries = indexAllSessions(sessionsRoot); const byId = new Map(entries.map(entry => [entry.id, entry])); const titles = titleIndex(sessionsRoot)
  const children = new Map<string, string[]>()
  for (const entry of entries) if (entry.parentId !== null) children.set(entry.parentId, [...(children.get(entry.parentId) ?? []), entry.id])
  const rootOf = (entry: SessionIndexEntry): string => { let current = entry; const seen = new Set<string>(); while (current.parentId !== null && !seen.has(current.id)) { seen.add(current.id); const parent = byId.get(current.parentId); if (!parent) break; current = parent } return current.id }
  const depthOf = (entry: SessionIndexEntry): number => { let depth = 0; let current = entry; const seen = new Set<string>(); while (current.parentId !== null && !seen.has(current.id)) { seen.add(current.id); const parent = byId.get(current.parentId); if (!parent) break; depth++; current = parent } return depth }
  const descendants = (id: string, seen = new Set<string>()): number => { if (seen.has(id)) return 0; seen.add(id); return (children.get(id) ?? []).reduce((sum, child) => sum + 1 + descendants(child, seen), 0) }
  const agents = entries.map(entry => {
    const indexedTitle = titles.get(entry.id)
    const title = indexedTitle !== undefined && !reservedTransportTitle(indexedTitle)
      ? indexedTitle
      : firstAuthoredTitle(entry) ?? (entry.parentId === null ? "Untitled Codex chat" : entry.nickname ?? entry.role ?? "Subagent")
    return ({
    threadRef: entry.id, parentThreadRef: entry.parentId, title,
    status: inferredStatus(entry.file), createdAt: entry.createdAt, updatedAt: entry.updatedAt, depth: depthOf(entry), descendantCount: descendants(entry.id), model: entry.model, role: entry.role, nickname:entry.nickname,agentPath:entry.agentPath,sourceVersion:entry.sourceVersion,reasoning:entry.reasoning,source:"codex" as const,
  })}).sort((a,b) => a.createdAt.localeCompare(b.createdAt))
  return { entries, agents: agents.map(agent => ({ ...agent, title: agent.title || rootOf(byId.get(agent.threadRef)!) })) }
}

export const readCodexHistoryCatalog = (sessionsRoot: string, graph = buildCodexHistoryGraph(sessionsRoot)): CodexHistoryCatalog => {
  const { agents } = graph
  return { roots: agents.filter(agent => agent.parentThreadRef === null).sort((a,b) => b.updatedAt.localeCompare(a.updatedAt)), agents }
}

const contentText = (value: unknown): string => Array.isArray(value) ? value.map(part => { const row = object(part); return row === null ? "" : safeText(row.text ?? row.value ?? row.content) }).filter(Boolean).join("\n") : safeText(value)
const planEntryStatus = (value: unknown): "pending" | "in_progress" | "completed" | null => {
  const raw = string(value)?.toLowerCase().replaceAll(/[\s-]+/g, "_") ?? null
  if (raw === "completed" || raw === "complete" || raw === "done") return "completed"
  if (raw === "in_progress" || raw === "inprogress" || raw === "active" || raw === "started") return "in_progress"
  if (raw === "pending" || raw === "todo" || raw === "not_started") return "pending"
  return null
}
/**
 * Tolerant structured-entry reader for history `plan`/`todo_list` rows (T8
 * #8865). Supports the canonical `{step,status}` shape plus common alternate
 * field names (`{content,status}`, `{text,completed}`); returns `[]` when the
 * raw value isn't a recognizable list so callers fall back to the flattened
 * prose summary instead of losing the row.
 */
export const planEntriesFromRecord = (value: unknown): ReadonlyArray<WorkbenchPlanEntryInput> => {
  if (!Array.isArray(value)) return []
  const out: Array<WorkbenchPlanEntryInput> = []
  for (const raw of value) {
    const row = object(raw)
    if (row === null) continue
    const step = string(row.step) ?? string(row.content) ?? string(row.text) ?? string(row.title)
    if (step === null) continue
    const status = planEntryStatus(row.status) ??
      (row.completed === true ? "completed" : row.completed === false ? "pending" : null)
    out.push({ step, status: status ?? "pending" })
  }
  return out
}
/** Typed plan sidecar (T8 #8865): structured entries when the raw record
 * carries a recognizable list, else the flattened prose text — never both,
 * so `DesktopPlanCard` shows one honest representation per row. */
export const planWorkbenchItemFromRow = (
  item: RecordValue,
  redact: (value: string) => string,
): WorkbenchItem | null => {
  const entries = planEntriesFromRecord(item.plan ?? item.todos ?? item.items)
  const prose = entries.length === 0 ? contentText(item.plan ?? item.content ?? item.text) : ""
  if (entries.length === 0 && prose.trim() === "") return null
  return workbenchPlanItemFromEntries(
    { source: "codex", entries, ...(prose.trim() === "" ? {} : { prose }) },
    redact,
  )
}
const field = (label: string, value: unknown) => { const text = redactCodexHistoryText(safeText(value)); return text.text === "" ? null : { label, value: text.text, redacted: text.redacted } }
const firstString = (value: unknown): string | null => Array.isArray(value) ? value.map(string).find((entry): entry is string => entry !== null) ?? null : string(value)
const isInjectedAgentMetadata = (role: string | null, value: string): boolean => codexUserRecordKind(role, value) === "agent_metadata"
const isInjectedPluginMetadata = (role: string | null, value: string): boolean => codexUserRecordKind(role, value) === "plugin_metadata"
/** Codex persists its per-turn execution envelope as a user-role message. It
 * is runtime scaffolding, not authored chat, so keep it on the filtered
 * context lane. Require one complete, top-level envelope: user prose that
 * merely mentions the tag remains visible. */
const isInjectedEnvironmentContext = (role: string | null, value: string): boolean => {
  return codexUserRecordKind(role, value) === "environment_context"
}
const agentMessageEnvelope = (value: string): Readonly<{ type: string | null; task: string | null; sender: string | null; payload: string | null }> => {
  const payloadMarker = "\nPayload:\n"
  const payloadIndex = value.indexOf(payloadMarker)
  const header = (payloadIndex < 0 ? value : value.slice(0,payloadIndex)).split("\n")
  const read = (name: string): string | null => header.find(line=>line.startsWith(`${name}: `))?.slice(name.length+2).trim() || null
  return { type:read("Message Type"),task:read("Task name"),sender:read("Sender"),payload:payloadIndex<0?null:value.slice(payloadIndex+payloadMarker.length).trim()||null }
}

/** Exported for direct unit coverage of the rollout row -> CodexHistoryItem
 * classification (T9 #8866); every reader above calls this internally. */
export const projectRow = (row: unknown, threadRef: string, sequence: number): CodexHistoryItem => {
  const envelope = object(row); const payload = envelope && object(envelope.payload); const envelopeType = envelope && string(envelope.type) || "invalid"; const timestamp = envelope && iso(envelope.timestamp) || new Date(0).toISOString()
  if (envelope === null || payload === null) return { itemRef: `${threadRef}:${sequence}`, threadRef, sequence, timestamp, kind: "gap", label: "Unreadable source record", summary: "This record could not be decoded.", status: "unsupported", fields: [], redacted: false, sourceType: envelopeType }
  const nestedPayload = object(payload.payload); let item = nestedPayload ?? payload; let itemType = string(item.type) ?? envelopeType
  if (envelopeType === "event_msg" && itemType === "item_completed") { const completed=object(item.item); if(completed){item=completed;itemType=string(item.type)??itemType} }
  let kind: CodexHistoryItemKind = "gap"; let label = itemType; let summary = ""; let status: string | null = string(item.status); const fields: Array<{label:string;value:string;redacted:boolean}> = []
  const push = (name: string, value: unknown) => { const next = field(name, value); if (next) fields.push(next) }
  if (envelopeType === "session_meta") { kind = "session"; label = "Session started"; summary = "Codex session metadata"; push("model", item.model); push("role", item.agent_role); push("source", typeof item.source === "string" ? item.source : object(item.source)?.type) }
  else if (envelopeType === "turn_context") { kind = "context"; label = "Turn context"; summary = "Execution context updated"; push("model", item.model); push("approval", item.approval_policy); push("sandbox", item.sandbox_policy) }
  else if (envelopeType === "world_state") { kind = "context"; label = "World state"; summary = "Recorded agent world-state snapshot"; push("agents", Array.isArray(item.agents) ? item.agents.length : item.agent_count); push("version", item.version) }
  else if (envelopeType === "compacted") { kind = "context"; label = "History compacted"; summary = "Codex persisted a compacted history boundary"; push("replacement items", Array.isArray(item.replacement_history) ? item.replacement_history.length : item.item_count) }
  else if (envelopeType === "inter_agent_communication_metadata") { kind="system_message";label="Agent communication metadata";summary="Inter-agent protocol handoff marker";push("trigger turn",item.trigger_turn) }
  else if (itemType === "message" || itemType === "agent_message") { const role = string(item.role) ?? (itemType === "agent_message" ? "assistant" : null); summary = contentText(item.content ?? item.text); const injectedAgentMetadata=isInjectedAgentMetadata(role,summary);const injectedPluginMetadata=isInjectedPluginMetadata(role,summary);const injectedEnvironmentContext=isInjectedEnvironmentContext(role,summary);kind = injectedAgentMetadata ? "metadata" : injectedPluginMetadata ? "system_message" : injectedEnvironmentContext ? "context" : itemType === "agent_message" ? "agent_message" : role === "user" ? "user_message" : role === "system" || role === "developer" ? "system_message" : "assistant_message"; label = injectedAgentMetadata ? "Agent metadata" : injectedPluginMetadata ? "Plugin metadata" : injectedEnvironmentContext ? "Execution environment" : itemType === "agent_message" ? "Agent message" : role === "user" ? "You" : role === "assistant" ? "Assistant" : `Message · ${role ?? "unknown"}`;if(itemType==="agent_message"){const envelope=agentMessageEnvelope(summary);push("message type",envelope.type);push("task",envelope.task);push("sender",envelope.sender??item.author);push("recipient",item.recipient);push("payload",envelope.payload)} }
  else if (itemType.includes("reasoning")) { kind = "reasoning"; label = "Reasoning summary"; summary = contentText(item.summary); if(summary==="")summary="[REDACTED: reasoning not persisted as summary]" }
  else if (itemType.includes("plan") || itemType === "todo_list") {
    kind = "plan"; label = "Plan"
    const structured = planEntriesFromRecord(item.plan ?? item.todos ?? item.items)
    summary = structured.length > 0 ? structured.map(entry => entry.step).join("; ") : contentText(item.plan ?? item.content ?? item.text)
  }
  else if (itemType.includes("collab") || itemType.includes("agent") || ["spawn_agent","send_input","wait","resume_agent","interrupt_agent","close_agent"].includes(itemType)) { const agentsState=object(item.agents_states); const operation=string(item.tool)??string(item.name)??itemType; const agentRef=string(item.new_thread_id)??string(item.agent_thread_id)??firstString(item.receiver_thread_ids)??string(item.agent_id)??string(item.receiver_thread_id)??(agentsState===null?null:Object.keys(agentsState)[0]??null); kind = "collaboration"; label = operation === "spawn_agent" ? "Subagent started" : operation.replaceAll("_"," "); summary = safeText(item.message ?? item.prompt ?? item.result ?? item.status ?? item.kind); push("agent", agentRef); push("operation", operation); push("activity", item.kind) }
  else if (itemType.includes("approval")) { kind = "approval"; label = "Approval"; summary = safeText(item.reason ?? item.message ?? item.status); push("decision", item.decision) }
  else if (itemType.includes("usage") || itemType.includes("token_count")) { kind = "usage"; label = "Usage"; summary = "Token usage update"; push("input", item.input_tokens); push("output", item.output_tokens); push("total", item.total_tokens) }
  else if (itemType.includes("output") || itemType.includes("result")) { kind = "tool_result"; label = string(item.name) ?? "Tool result"; summary = safeText(item.output ?? item.result ?? item.content); push("call", item.call_id); push("status", item.status); push("started", item.started_at ?? item.start_time); push("ended", item.completed_at ?? item.end_time); push("duration", item.duration_ms); push("output", item.output ?? item.result); push("files", item.files ?? item.affected_files); push("artifacts", item.artifacts ?? item.artifact_refs); push("error", item.error) }
  else if (itemType.includes("call") || itemType.includes("tool") || itemType.includes("shell") || itemType === "function_call" || itemType === "commandExecution" || itemType.includes("command_execution") || itemType === "apply_patch" || itemType === "applyPatch") { kind = "tool_call"; label = string(item.name) ?? itemType; summary = safeText(item.command ?? item.input ?? item.arguments); push("call", item.call_id); push("status", item.status); push("started", item.started_at ?? item.start_time); push("ended", item.completed_at ?? item.end_time); push("duration", item.duration_ms); push("input", item.arguments ?? item.input ?? item.command); push("files", item.files ?? item.affected_files); push("artifacts", item.artifacts ?? item.artifact_refs); push("error", item.error) }
  else if (itemType.includes("error")) { kind = "error"; label = "Error"; summary = safeText(item.message ?? item.error); status = "error" }
  // Long-tail honest rows (#8869, T12 epic #8857 wave 2): these ThreadItem
  // variants previously matched none of the branches above and fell through
  // to `kind: "gap"` (counted as a completeness loss even though the source
  // record decoded fine). Classified as `tool_call` so the typed-sidecar
  // attachment below (kind === "tool_call") carries the real `WorkbenchItem`
  // the renderer needs; the string label/summary stay the honest fallback.
  else if (itemType === "hookPrompt" || itemType === "hook_prompt") { kind = "tool_call"; label = "Hook prompt"; summary = contentText(item.fragments) }
  else if (itemType === "sleep") { kind = "tool_call"; label = "Sleep"; summary = `${typeof item.durationMs === "number" ? item.durationMs : 0}ms` }
  else if (itemType === "enteredReviewMode" || itemType === "entered_review_mode") { kind = "tool_call"; label = "Entered review"; summary = safeText(item.review) }
  else if (itemType === "exitedReviewMode" || itemType === "exited_review_mode") { kind = "tool_call"; label = "Exited review"; summary = safeText(item.review) }
  else if (itemType === "contextCompaction" || itemType === "context_compaction") { kind = "tool_call"; label = "Context compacted"; summary = "" }
  else if (envelopeType === "event_msg") { kind = itemType.includes("error") ? "error" : "lifecycle"; label = itemType; summary = safeText(item.message ?? item.text ?? item.status); push("event", itemType) }
  const redactedSummary = redactCodexHistoryText(summary || label); const redacted = redactedSummary.redacted || fields.some(item => item.redacted) || summary.startsWith("[REDACTED:")
  // Typed sidecar (#8859, extended #8863 for reasoning, T9 #8866 for
  // approval): tool-class, reasoning, and approval rows carry the structured
  // WorkbenchItem (command cwd/exit/duration/output tail, per-file diffs,
  // args/results, recorded approval decision) so renderers rebuild the same
  // typed card the live turn showed. Reasoning rows get the same treatment
  // so history renders the identical `DesktopReasoningDisclosure` component
  // the live turn used — but only when not redacted; a redacted row never
  // gets a typed item (honest absence, not a false completed summary). The
  // reader is tolerant — rows whose source shape has no typed projection
  // stay string-only.
  const typedSource = [string(item.name), string(item.tool), itemType].some(value => value === "apply_patch" || value === "applyPatch")
    ? { ...item, type: "apply_patch", patch: item.input ?? item.arguments ?? item.content }
    : item
  // T8 (#8865): plan/todo_list rows get the SAME typed sidecar (structured
  // entries or prose fallback) so history plans render through the identical
  // DesktopPlanCard the live turn and turn/plan/updated notification use,
  // instead of a bespoke single-entry reconstruction in the timeline renderer.
  const typedItem = kind === "tool_call" || kind === "tool_result" || kind === "approval" ||
      (kind === "reasoning" && !redacted)
    ? workbenchItemFromThreadItem(typedSource, "codex", value => redactCodexHistoryText(value).text)
    : kind === "plan"
      ? planWorkbenchItemFromRow(item, value => redactCodexHistoryText(value).text)
      : null
  return { itemRef: `${threadRef}:${sequence}`, threadRef, sequence, timestamp, kind, label: label.slice(0,160), summary: redactedSummary.text, status, fields: fields.map(({label,value}) => ({label,value})), redacted, sourceType: `${envelopeType}/${itemType}`.slice(0,160), ...(typedItem === null ? {} : { item: typedItem }) }
}

const projectionAccounting = (row: unknown): Readonly<{ gap: boolean; redaction: boolean }> => {
  const envelope=object(row);const payload=envelope&&object(envelope.payload);if(envelope===null||payload===null)return {gap:true,redaction:false}
  const envelopeType=string(envelope.type)??"invalid";const nestedPayload=object(payload.payload);let item=nestedPayload??payload;let itemType=string(item.type)??envelopeType
  if(envelopeType==="event_msg"&&itemType==="item_completed"){const completed=object(item.item);if(completed){item=completed;itemType=string(item.type)??itemType}}
  const supported=envelopeType==="session_meta"||envelopeType==="turn_context"||envelopeType==="world_state"||envelopeType==="compacted"||envelopeType==="event_msg"||itemType==="message"||itemType==="agent_message"||itemType.includes("reasoning")||itemType.includes("plan")||itemType==="todo_list"||itemType.includes("collab")||itemType.includes("agent")||["spawn_agent","send_input","wait","resume_agent","interrupt_agent","close_agent"].includes(itemType)||itemType.includes("approval")||itemType.includes("usage")||itemType.includes("token_count")||itemType.includes("output")||itemType.includes("result")||itemType.includes("call")||itemType.includes("tool")||itemType.includes("shell")||itemType==="function_call"||itemType==="commandExecution"||itemType.includes("command_execution")||itemType==="apply_patch"||itemType==="applyPatch"||itemType.includes("error")
    // Long-tail honest rows (issue 8869, T12 epic 8857 wave 2) — see projectRow.
    ||itemType==="hookPrompt"||itemType==="hook_prompt"||itemType==="sleep"||itemType==="enteredReviewMode"||itemType==="entered_review_mode"||itemType==="exitedReviewMode"||itemType==="exited_review_mode"||itemType==="contextCompaction"||itemType==="context_compaction"
  const redaction=itemType.includes("reasoning")&&contentText(item.summary)===""
  return {gap:!supported,redaction}
}

const tailHistoryRows = (file: string): ReadonlyArray<unknown> => {
  try {
    const text = file.endsWith(".zst") ? historyText(file).slice(-historyTailBytes) : (() => { const size=statSync(file).size; const descriptor=openSync(file,"r"); try { const buffer=Buffer.alloc(Math.min(historyTailBytes,size)); readSync(descriptor,buffer,0,buffer.length,size-buffer.length); return buffer.toString("utf8") } finally { closeSync(descriptor) } })()
    return text.split("\n").flatMap(line=>{try{return [JSON.parse(line)]}catch{return []}})
  } catch { return [] }
}

const previewableChildItem = (item: CodexHistoryItem): boolean =>
  ["user_message", "assistant_message", "agent_message", "reasoning", "plan", "collaboration", "tool_call", "tool_result", "approval", "error"].includes(item.kind) &&
  item.summary.trim() !== "" && !item.summary.startsWith("[REDACTED:")

const childPreview = (entry: SessionIndexEntry, agent: CodexHistoryAgent): CodexHistoryAgentPreview => {
  const rows = tailHistoryRows(entry.file)
  const latest = rows.map((row,index)=>projectRow(row,entry.id,index)).reverse().find(previewableChildItem)
  return {
    threadRef: entry.id,
    title: agent.title,
    status: agent.status,
    updatedAt: agent.updatedAt,
    latest: latest === undefined ? null : {
      label: latest.label,
      summary: latest.summary.slice(0,360),
      kind: latest.kind,
      timestamp: latest.timestamp,
    },
  }
}

const isSubagentLaunchItem = (item: CodexHistoryItem): boolean => {
  const operation = item.fields.find(field => field.label === "operation")?.value
  const activity = item.fields.find(field => field.label === "activity")?.value
  return operation === "spawn_agent" || operation === "collab_agent_spawn_begin" || operation === "collab_agent_spawn_end" || (operation === "sub_agent_activity" && activity === "started")
}

export const readCodexHistoryPage = (input: Readonly<{ sessionsRoot: string; threadRef: string; offset?: number; limit?: number }>, graph = buildCodexHistoryGraph(input.sessionsRoot)): CodexHistoryPage | null => {
  const entry = graph.entries.find(item => item.id === input.threadRef); if (!entry) return null
  // ONE streaming pass (bounded memory, #8789): whole-conversation accounting
  // and totals still walk EVERY line — only the requested window's parsed rows
  // are retained, so a multi-GB rollout no longer ENOMEMs the reader.
  const requestedOffset = Math.max(0, input.offset ?? 0); const limit = Math.max(1, Math.min(input.limit ?? 200, 500))
  const windowRows: unknown[] = []
  let total = 0; let gaps = 0; let redactions = 0
  try {
    streamRolloutLines(entry.file, line => {
      let parsed: unknown = null
      try { parsed = JSON.parse(line) } catch { parsed = null }
      const accounting = projectionAccounting(parsed)
      if (accounting.gap) gaps++
      if (accounting.redaction) redactions++
      if (total >= requestedOffset && windowRows.length < limit) windowRows.push(parsed)
      total++
      return true
    })
  } catch { return null }
  const offset = Math.min(requestedOffset, total)
  const entriesById=new Map(graph.entries.map(item=>[item.id,item])); const agentsById=new Map(graph.agents.map(item=>[item.threadRef,item])); const previewById=new Map<string,CodexHistoryAgentPreview>(); const items=windowRows.map((row,index)=>{const item=projectRow(row,entry.id,offset+index);if(item.kind!=="collaboration"||!isSubagentLaunchItem(item))return item;const agentRef=item.fields.find(field=>field.label==="agent")?.value;if(!agentRef)return item;const childEntry=entriesById.get(agentRef);const childAgent=agentsById.get(agentRef);if(childEntry===undefined||childAgent===undefined)return {...item,fields:[...item.fields,{label:"history",value:"Child history not recorded"}]};let relatedAgent=previewById.get(agentRef);if(relatedAgent===undefined){relatedAgent=childPreview(childEntry,childAgent);previewById.set(agentRef,relatedAgent)}return {...item,relatedAgent}})
  const root = (() => { let current = entry; const byId = new Map(graph.entries.map(item => [item.id,item])); const seen = new Set<string>(); while (current.parentId && !seen.has(current.id)) { seen.add(current.id); const parent = byId.get(current.parentId); if (!parent) break; current = parent } return current.id })()
  return { rootThreadRef: root, selectedThreadRef: entry.id, agents: graph.agents.filter(agent => { let id: string | null = agent.threadRef; const byId = new Map(graph.entries.map(item => [item.id,item])); while (id) { if (id === root) return true; id = byId.get(id)?.parentId ?? null } return false }), items, offset, limit, totalItems: total, hasPrevious: offset > 0, hasNext: offset + limit < total, completeness: { source: total, rendered: total - gaps - redactions, redactions, gaps, complete: true } }
}

/**
 * Bounded HEAD projection for the free-text search-index CACHE (#8788). The
 * loss-accounted paging authority stays `readCodexHistoryPage`; this exists so
 * building the content index over the most-recent sessions never streams a
 * whole multi-GB rollout just to index its first `maxItems` rows.
 */
export const searchIndexByteCap = 16 * 1024 * 1024
export const readCodexHistoryHeadItems = (file: string, threadRef: string, maxItems: number, byteCap = searchIndexByteCap): CodexHistoryItem[] => {
  const items: CodexHistoryItem[] = []
  let sequence = 0
  try {
    streamRolloutLines(file, line => {
      let parsed: unknown = null
      try { parsed = JSON.parse(line) } catch { parsed = null }
      items.push(projectRow(parsed, threadRef, sequence))
      sequence++
      return items.length < maxItems
    }, byteCap)
  } catch { return items }
  return items
}
