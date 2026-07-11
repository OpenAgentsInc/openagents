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

// Provider-native historical projection (#8674). This deliberately coexists
// with the legacy local chat adapter above until all chat mutation call sites
// are moved off that adapter. It has no 24-hour or tail window.
type SessionIndexEntry = Readonly<{ file: string; id: string; parentId: string | null; createdAt: string; updatedAt: string; model: string | null; role: string | null; nickname:string|null; agentPath:string|null; sourceVersion:string|null; reasoning:string|null }>

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

const historyText = (file: string): string => file.endsWith(".zst") ? Buffer.from(Bun.zstdDecompressSync(readFileSync(file))).toString("utf8") : readFileSync(file,"utf8")
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
  return [{ file, id, parentId, createdAt, updatedAt: statSync(file).mtime.toISOString(), model: string(payload.model), role: string(payload.agent_role) ?? string(payload.agent_type) ?? (spawn === null ? null : string(spawn.agent_role)), nickname:string(payload.agent_nickname)??(spawn===null?null:string(spawn.agent_nickname)),agentPath:string(payload.agent_path)??(spawn===null?null:string(spawn.agent_path)),sourceVersion:string(payload.multi_agent_version)??string(payload.history_mode),reasoning:string(payload.reasoning_effort) }]
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
  const agents = entries.map(entry => ({
    threadRef: entry.id, parentThreadRef: entry.parentId, title: titles.get(entry.id) ?? (entry.parentId === null ? "Untitled Codex chat" : entry.nickname ?? entry.role ?? "Subagent"),
    status: inferredStatus(entry.file), createdAt: entry.createdAt, updatedAt: entry.updatedAt, depth: depthOf(entry), descendantCount: descendants(entry.id), model: entry.model, role: entry.role, nickname:entry.nickname,agentPath:entry.agentPath,sourceVersion:entry.sourceVersion,reasoning:entry.reasoning,
  })).sort((a,b) => a.createdAt.localeCompare(b.createdAt))
  return { entries, agents: agents.map(agent => ({ ...agent, title: agent.title || rootOf(byId.get(agent.threadRef)!) })) }
}

export const readCodexHistoryCatalog = (sessionsRoot: string, graph = buildCodexHistoryGraph(sessionsRoot)): CodexHistoryCatalog => {
  const { agents } = graph
  return { roots: agents.filter(agent => agent.parentThreadRef === null).sort((a,b) => b.updatedAt.localeCompare(a.updatedAt)), agents }
}

const contentText = (value: unknown): string => Array.isArray(value) ? value.map(part => { const row = object(part); return row === null ? "" : safeText(row.text ?? row.value ?? row.content) }).filter(Boolean).join("\n") : safeText(value)
const field = (label: string, value: unknown) => { const text = redactCodexHistoryText(safeText(value)); return text.text === "" ? null : { label, value: text.text, redacted: text.redacted } }
const firstString = (value: unknown): string | null => Array.isArray(value) ? value.map(string).find((entry): entry is string => entry !== null) ?? null : string(value)

const projectRow = (row: unknown, threadRef: string, sequence: number): CodexHistoryItem => {
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
  else if (itemType === "message" || itemType === "agent_message") { const role = string(item.role) ?? (itemType === "agent_message" ? "assistant" : null); kind = role === "user" ? "user_message" : role === "system" || role === "developer" ? "system_message" : "assistant_message"; label = role === "user" ? "You" : role === "assistant" ? "Assistant" : `Message · ${role ?? "unknown"}`; summary = contentText(item.content ?? item.text) }
  else if (itemType.includes("reasoning")) { kind = "reasoning"; label = "Reasoning summary"; summary = contentText(item.summary); if(summary==="")summary="[REDACTED: reasoning not persisted as summary]" }
  else if (itemType.includes("plan") || itemType === "todo_list") { kind = "plan"; label = "Plan"; summary = contentText(item.plan ?? item.content ?? item.text) }
  else if (itemType.includes("collab") || itemType.includes("agent") || ["spawn_agent","send_input","wait","resume_agent","interrupt_agent","close_agent"].includes(itemType)) { const agentsState=object(item.agents_states); const operation=string(item.tool)??string(item.name)??itemType; const agentRef=string(item.new_thread_id)??string(item.agent_thread_id)??firstString(item.receiver_thread_ids)??string(item.agent_id)??string(item.receiver_thread_id)??(agentsState===null?null:Object.keys(agentsState)[0]??null); kind = "collaboration"; label = operation === "spawn_agent" ? "Subagent started" : operation.replaceAll("_"," "); summary = safeText(item.message ?? item.prompt ?? item.result ?? item.status ?? item.kind); push("agent", agentRef); push("operation", operation); push("activity", item.kind) }
  else if (itemType.includes("approval")) { kind = "approval"; label = "Approval"; summary = safeText(item.reason ?? item.message ?? item.status); push("decision", item.decision) }
  else if (itemType.includes("usage") || itemType.includes("token_count")) { kind = "usage"; label = "Usage"; summary = "Token usage update"; push("input", item.input_tokens); push("output", item.output_tokens); push("total", item.total_tokens) }
  else if (itemType.includes("output") || itemType.includes("result")) { kind = "tool_result"; label = string(item.name) ?? "Tool result"; summary = safeText(item.output ?? item.result ?? item.content); push("call", item.call_id); push("status", item.status); push("started", item.started_at ?? item.start_time); push("ended", item.completed_at ?? item.end_time); push("duration", item.duration_ms); push("output", item.output ?? item.result); push("files", item.files ?? item.affected_files); push("artifacts", item.artifacts ?? item.artifact_refs); push("error", item.error) }
  else if (itemType.includes("call") || itemType.includes("tool") || itemType.includes("shell") || itemType === "function_call" || itemType.includes("command_execution")) { kind = "tool_call"; label = string(item.name) ?? itemType; summary = safeText(item.command ?? item.input ?? item.arguments); push("call", item.call_id); push("status", item.status); push("started", item.started_at ?? item.start_time); push("ended", item.completed_at ?? item.end_time); push("duration", item.duration_ms); push("input", item.arguments ?? item.input ?? item.command); push("files", item.files ?? item.affected_files); push("artifacts", item.artifacts ?? item.artifact_refs); push("error", item.error) }
  else if (itemType.includes("error")) { kind = "error"; label = "Error"; summary = safeText(item.message ?? item.error); status = "error" }
  else if (envelopeType === "event_msg") { kind = itemType.includes("error") ? "error" : "lifecycle"; label = itemType; summary = safeText(item.message ?? item.text ?? item.status); push("event", itemType) }
  const redactedSummary = redactCodexHistoryText(summary || label); const redacted = redactedSummary.redacted || fields.some(item => item.redacted) || summary.startsWith("[REDACTED:")
  return { itemRef: `${threadRef}:${sequence}`, threadRef, sequence, timestamp, kind, label: label.slice(0,160), summary: redactedSummary.text, status, fields: fields.map(({label,value}) => ({label,value})), redacted, sourceType: `${envelopeType}/${itemType}`.slice(0,160) }
}

const projectionAccounting = (row: unknown): Readonly<{ gap: boolean; redaction: boolean }> => {
  const envelope=object(row);const payload=envelope&&object(envelope.payload);if(envelope===null||payload===null)return {gap:true,redaction:false}
  const envelopeType=string(envelope.type)??"invalid";const nestedPayload=object(payload.payload);let item=nestedPayload??payload;let itemType=string(item.type)??envelopeType
  if(envelopeType==="event_msg"&&itemType==="item_completed"){const completed=object(item.item);if(completed){item=completed;itemType=string(item.type)??itemType}}
  const supported=envelopeType==="session_meta"||envelopeType==="turn_context"||envelopeType==="world_state"||envelopeType==="compacted"||envelopeType==="event_msg"||itemType==="message"||itemType==="agent_message"||itemType.includes("reasoning")||itemType.includes("plan")||itemType==="todo_list"||itemType.includes("collab")||itemType.includes("agent")||["spawn_agent","send_input","wait","resume_agent","interrupt_agent","close_agent"].includes(itemType)||itemType.includes("approval")||itemType.includes("usage")||itemType.includes("token_count")||itemType.includes("output")||itemType.includes("result")||itemType.includes("call")||itemType.includes("tool")||itemType.includes("shell")||itemType==="function_call"||itemType.includes("command_execution")||itemType.includes("error")
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
  ["user_message", "assistant_message", "reasoning", "plan", "collaboration", "tool_call", "tool_result", "approval", "error"].includes(item.kind) &&
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
  const rows = historyText(entry.file).split("\n").filter(line => line !== "").map(line => { try { return JSON.parse(line) } catch { return null } })
  const offset = Math.max(0, Math.min(input.offset ?? 0, rows.length)); const limit = Math.max(1, Math.min(input.limit ?? 200, 500)); const entriesById=new Map(graph.entries.map(item=>[item.id,item])); const agentsById=new Map(graph.agents.map(item=>[item.threadRef,item])); const previewById=new Map<string,CodexHistoryAgentPreview>(); const items=rows.slice(offset,offset+limit).map((row,index)=>{const item=projectRow(row,entry.id,offset+index);if(item.kind!=="collaboration"||!isSubagentLaunchItem(item))return item;const agentRef=item.fields.find(field=>field.label==="agent")?.value;if(!agentRef)return item;const childEntry=entriesById.get(agentRef);const childAgent=agentsById.get(agentRef);if(childEntry===undefined||childAgent===undefined)return {...item,fields:[...item.fields,{label:"history",value:"Child history not recorded"}]};let relatedAgent=previewById.get(agentRef);if(relatedAgent===undefined){relatedAgent=childPreview(childEntry,childAgent);previewById.set(agentRef,relatedAgent)}return {...item,relatedAgent}})
  const root = (() => { let current = entry; const byId = new Map(graph.entries.map(item => [item.id,item])); const seen = new Set<string>(); while (current.parentId && !seen.has(current.id)) { seen.add(current.id); const parent = byId.get(current.parentId); if (!parent) break; current = parent } return current.id })()
  const accounting=rows.reduce((total,row)=>{const next=projectionAccounting(row);return {gaps:total.gaps+(next.gap?1:0),redactions:total.redactions+(next.redaction?1:0)}},{gaps:0,redactions:0})
  return { rootThreadRef: root, selectedThreadRef: entry.id, agents: graph.agents.filter(agent => { let id: string | null = agent.threadRef; const byId = new Map(graph.entries.map(item => [item.id,item])); while (id) { if (id === root) return true; id = byId.get(id)?.parentId ?? null } return false }), items, offset, limit, totalItems: rows.length, hasPrevious: offset > 0, hasNext: offset + limit < rows.length, completeness: { source: rows.length, rendered: rows.length - accounting.gaps - accounting.redactions, redactions:accounting.redactions, gaps:accounting.gaps, complete: true } }
}
