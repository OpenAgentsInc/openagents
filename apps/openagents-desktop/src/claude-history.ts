/**
 * Read-only, loss-accounted projection of the local Claude Code history tree
 * under `~/.claude/projects` (#8712 H3). It mirrors the Codex importer
 * (`codex-history.ts`): the same bounded, credential-redacted `CodexHistory*`
 * contracts, the same `source = rendered + redactions + gaps` completeness
 * equation, and the same parent/child agent graph — reconstructed for Claude's
 * DIFFERENT persistence model.
 *
 * Claude's model (per `docs/teardowns/2026-07-10-claude-subagents-rendering-analysis.md`):
 *   - a parent session is `<project>/<sessionId>.jsonl`;
 *   - an ordinary child is `<sessionId>/subagents/agent-<agentId>.jsonl`;
 *   - a workflow child is `<sessionId>/subagents/workflows/wf_<runId>/agent-<agentId>.jsonl`;
 *   - the parent/child edge is NOT `parentUuid` (that is the intra-transcript
 *     causal chain). It is the invoking `Agent` tool call's structured result:
 *     `assistant.tool_use(name=Agent,id) -> user.tool_result(tool_use_id)`
 *     -> top-level `toolUseResult.agentId` -> `agent-<agentId>.jsonl`.
 *
 * Every discovered child file becomes a node: a linked node when its structured
 * edge is recovered, or an explicit ORPHAN/gap node (the ~3% class) attached to
 * its session root and flagged, never silently hidden.
 *
 * Refs are namespaced `claude:<id>` so the merged catalog and page router never
 * confuse a Claude id with a Codex thread id. Raw prompts, thinking, tool
 * arguments, command output, credentials, and file contents never leave this
 * projection except through the same bounded/redacted item fields the Codex
 * importer already surfaces.
 */
import { closeSync, openSync, readFileSync, readSync, readdirSync, statSync } from "node:fs"
import path from "node:path"

import type { CodexHistoryAgent, CodexHistoryAgentPreview, CodexHistoryCatalog, CodexHistoryItem, CodexHistoryItemKind, CodexHistoryPage } from "./codex-history-contract.ts"
import { redactCodexHistoryText } from "./codex-history.ts"

const metadataHeadBytes = 128 * 1024
const historyTailBytes = 96 * 1024

type RecordValue = Record<string, unknown>
const object = (value: unknown): RecordValue | null =>
  typeof value === "object" && value !== null && !Array.isArray(value) ? value as RecordValue : null
const string = (value: unknown): string | null => typeof value === "string" && value.trim() !== "" ? value : null
const iso = (value: unknown): string | null => {
  const text = typeof value === "string" ? value : null
  return text !== null && Number.isFinite(Date.parse(text)) ? text : null
}
const safeText = (value: unknown, limit = 20_000): string =>
  typeof value === "string" ? value.slice(0, limit)
    : typeof value === "number" || typeof value === "boolean" ? String(value)
      : value === null || value === undefined ? ""
        : (() => { try { return JSON.stringify(value).slice(0, limit) } catch { return "" } })()

export const CLAUDE_REF_PREFIX = "claude:"
export const isClaudeThreadRef = (ref: string): boolean => ref.startsWith(CLAUDE_REF_PREFIX)
const ref = (id: string): string => `${CLAUDE_REF_PREFIX}${id}`

const jsonlFilesUnder = (root: string): string[] => {
  const visit = (directory: string): string[] => {
    try {
      return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
        const child = path.join(directory, entry.name)
        if (entry.isDirectory()) return visit(child)
        // The workflow journal is orchestration state, not a transcript node.
        return entry.isFile() && entry.name.endsWith(".jsonl") && entry.name !== "journal.jsonl" ? [child] : []
      })
    } catch { return [] }
  }
  return visit(root)
}

/** Read a bounded head window of a JSONL file as decoded records. */
const headRecords = (file: string, bytes = metadataHeadBytes): RecordValue[] => {
  try {
    const size = statSync(file).size
    const descriptor = openSync(file, "r")
    let text: string
    try {
      const buffer = Buffer.alloc(Math.min(bytes, size))
      readSync(descriptor, buffer, 0, buffer.length, 0)
      const raw = buffer.toString("utf8")
      // Drop a partial trailing line so we never JSON.parse a truncated record.
      // Use lastIndexOf, NOT /[^\n]*$/ — that regex is O(L^2) on a head whose
      // final partial line is tens of KB (a real Claude record) and can hang.
      text = buffer.length < size ? raw.slice(0, raw.lastIndexOf("\n") + 1) : raw
    } finally { closeSync(descriptor) }
    return text.split("\n").flatMap(line => { const row = object(safeJson(line)); return row === null ? [] : [row] })
  } catch { return [] }
}

const safeJson = (line: string): unknown => { try { return line.trim() === "" ? null : JSON.parse(line) } catch { return null } }

const tailText = (file: string): string => {
  try {
    const size = statSync(file).size
    const descriptor = openSync(file, "r")
    try {
      const buffer = Buffer.alloc(Math.min(historyTailBytes, size))
      readSync(descriptor, buffer, 0, buffer.length, size - buffer.length)
      return buffer.toString("utf8")
    } finally { closeSync(descriptor) }
  } catch { return "" }
}

// ---------------------------------------------------------------------------
// Discovery + graph reconstruction
// ---------------------------------------------------------------------------

type ClaudeFileKind = "root" | "child"
type ClaudeFileEntry = Readonly<{
  file: string
  kind: ClaudeFileKind
  /** Claude id: sessionId for a root, agentId for a child. */
  id: string
  /** Owning session id (from the path). */
  sessionId: string
  workflow: boolean
}>

const AGENT_FILE = /agent-([0-9A-Za-z_-]+)\.jsonl$/u

/** Classify a discovered file into a root/child node by its path shape. */
const classify = (root: string, file: string): ClaudeFileEntry | null => {
  const rel = path.relative(root, file)
  const parts = rel.split(path.sep)
  const subagentsIndex = parts.indexOf("subagents")
  if (subagentsIndex > 0) {
    const sessionId = parts[subagentsIndex - 1]!
    const match = AGENT_FILE.exec(path.basename(file))
    if (sessionId === undefined || match === null) return null
    return { file, kind: "child", id: match[1]!, sessionId, workflow: parts.includes("workflows") }
  }
  // Any transcript directly in a project dir is a parent session (Claude names
  // these `<sessionId>.jsonl`). Never an `agent-*` file (those live under
  // `subagents/`) and never the workflow journal (filtered at discovery).
  if (path.basename(file).startsWith("agent-")) return null
  const id = path.basename(file).replace(/\.jsonl$/u, "")
  return { file, kind: "root", id, sessionId: id, workflow: false }
}

type ClaudeNode = Readonly<{
  entry: ClaudeFileEntry
  threadRef: string
  parentThreadRef: string | null
  createdAt: string
  updatedAt: string
  model: string | null
  title: string
  version: string | null
  cwd: string | null
  gitBranch: string | null
  status: CodexHistoryAgent["status"]
  orphan: boolean
}>

export type ClaudeHistoryGraph = Readonly<{ nodes: ReadonlyArray<ClaudeNode>; byRef: ReadonlyMap<string, ClaudeNode> }>

/** First user-authored text in a bounded head — the session title source. */
const titleFrom = (records: ReadonlyArray<RecordValue>): string | null => {
  for (const record of records) {
    if (string(record.type) !== "user") continue
    if (record.isMeta === true) continue
    const message = object(record.message)
    if (message === null) continue
    const text = messageText(message.content)
    // Skip synthetic tool-result-only user turns and command wrappers.
    if (text === null) continue
    const trimmed = text.replace(/\s+/gu, " ").trim()
    if (trimmed === "" || trimmed.startsWith("<") || trimmed.startsWith("Caveat:")) continue
    return trimmed.slice(0, 80)
  }
  return null
}

/** Concatenated text of a Claude message content (string or block array). */
const messageText = (content: unknown): string | null => {
  if (typeof content === "string") return content
  if (!Array.isArray(content)) return null
  const parts = content.flatMap(part => {
    const block = object(part)
    if (block === null) return []
    const type = string(block.type)
    if (type === "text") { const t = string(block.text); return t === null ? [] : [t] }
    return []
  })
  return parts.length === 0 ? null : parts.join("\n")
}

const modelFrom = (records: ReadonlyArray<RecordValue>): string | null => {
  for (const record of records) {
    const message = object(record.message)
    const model = message === null ? null : string(message.model)
    if (model !== null && model !== "<synthetic>") return model
  }
  return null
}

const firstTimestamp = (records: ReadonlyArray<RecordValue>): string | null => {
  for (const record of records) { const at = iso(record.timestamp); if (at !== null) return at }
  return null
}

const inferredStatus = (file: string, orphan: boolean): CodexHistoryAgent["status"] => {
  const rows = tailText(file).split("\n").flatMap(line => { const row = object(safeJson(line)); return row === null ? [] : [row] })
  for (const row of rows.reverse()) {
    if (row.isApiErrorMessage === true) return "errored"
    const message = object(row.message)
    if (string(row.type) === "assistant" && message !== null) {
      const stop = string(message.stop_reason)
      if (stop === "stop_sequence" || stop === "end_turn" || stop === "tool_use" || stop === null) return "completed"
    }
    const result = object(row.toolUseResult)
    const status = result === null ? null : string(result.status)
    if (status === "async_launched") return "running"
    if (status === "completed") return "completed"
  }
  return orphan ? "unknown" : "completed"
}

export const buildClaudeHistoryGraph = (projectsRoot: string): ClaudeHistoryGraph => {
  const entries = jsonlFilesUnder(projectsRoot).flatMap(file => { const entry = classify(projectsRoot, file); return entry === null ? [] : [entry] })
  // De-duplicate ids (a resumed session can appear once); keep the newest file.
  const byId = new Map<string, ClaudeFileEntry>()
  for (const entry of entries) {
    const existing = byId.get(entry.id)
    if (existing === undefined) { byId.set(entry.id, entry); continue }
    try { if (statSync(entry.file).mtimeMs > statSync(existing.file).mtimeMs) byId.set(entry.id, entry) } catch { /* keep existing */ }
  }
  const unique = [...byId.values()]
  // Metadata-first (like the Codex importer): the catalog is built from bounded
  // HEAD reads and the filesystem path topology, NOT by full-scanning 100MB
  // parents for Agent edges (which does not scale to a multi-GB ~/.claude).
  // Every child attaches to its session ROOT by path; the precise per-Agent
  // parent/child edge and previews are recovered lazily when a session's page
  // is opened (readClaudeHistoryPage reads that one file anyway). A child whose
  // session root file is absent is an explicit rootless orphan (topology gap),
  // shown, never hidden.
  const rootIds = new Set(unique.filter(entry => entry.kind === "root").map(entry => entry.id))

  const nodes: ClaudeNode[] = unique.map(entry => {
    const head = headRecords(entry.file)
    const createdAt = firstTimestamp(head) ?? statSync(entry.file).birthtime.toISOString()
    const updatedAt = (() => { try { return statSync(entry.file).mtime.toISOString() } catch { return createdAt } })()
    const first = head[0] ?? {}
    const rootedInSession = entry.kind === "child" && rootIds.has(entry.sessionId)
    const orphan = entry.kind === "child" && !rootedInSession
    const parentId = entry.kind === "root" ? null : rootedInSession ? entry.sessionId : null
    return {
      entry,
      threadRef: ref(entry.id),
      parentThreadRef: parentId === null ? null : ref(parentId),
      createdAt,
      updatedAt,
      model: modelFrom(head),
      title: titleFrom(head) ?? (entry.kind === "root" ? "Untitled Claude chat" : entry.workflow ? "Workflow agent" : "Subagent"),
      version: string(first.version),
      cwd: string(first.cwd),
      gitBranch: string(first.gitBranch),
      status: inferredStatus(entry.file, orphan),
      orphan,
    }
  }).sort((a, b) => a.createdAt.localeCompare(b.createdAt))

  return { nodes, byRef: new Map(nodes.map(node => [node.threadRef, node])) }
}

const depthOf = (node: ClaudeNode, byRef: ReadonlyMap<string, ClaudeNode>): number => {
  let depth = 0; let current: ClaudeNode | undefined = node; const seen = new Set<string>()
  while (current !== undefined && current.parentThreadRef !== null && !seen.has(current.threadRef)) { seen.add(current.threadRef); const parent = byRef.get(current.parentThreadRef); if (parent === undefined) break; depth++; current = parent }
  return depth
}
const rootOf = (node: ClaudeNode, byRef: ReadonlyMap<string, ClaudeNode>): string => {
  let current = node; const seen = new Set<string>()
  while (current.parentThreadRef !== null && !seen.has(current.threadRef)) { seen.add(current.threadRef); const parent = byRef.get(current.parentThreadRef); if (parent === undefined) break; current = parent }
  return current.threadRef
}

const toAgent = (node: ClaudeNode, graph: ClaudeHistoryGraph, children: ReadonlyMap<string, string[]>): CodexHistoryAgent => {
  const descendants = (ref_: string, seen = new Set<string>()): number => { if (seen.has(ref_)) return 0; seen.add(ref_); return (children.get(ref_) ?? []).reduce((sum, child) => sum + 1 + descendants(child, seen), 0) }
  return {
    threadRef: node.threadRef,
    parentThreadRef: node.parentThreadRef,
    title: node.title.slice(0, 160),
    status: node.status,
    createdAt: node.createdAt,
    updatedAt: node.updatedAt,
    depth: depthOf(node, graph.byRef),
    descendantCount: descendants(node.threadRef),
    model: node.model,
    role: node.entry.kind === "root" ? null : node.entry.workflow ? "workflow" : "subagent",
    nickname: null,
    agentPath: node.cwd,
    sourceVersion: node.version,
    reasoning: null,
    source: "claude",
    ...(node.orphan ? { orphan: true } : {}),
  }
}

const childIndex = (graph: ClaudeHistoryGraph): ReadonlyMap<string, string[]> => {
  const children = new Map<string, string[]>()
  for (const node of graph.nodes) if (node.parentThreadRef !== null) children.set(node.parentThreadRef, [...(children.get(node.parentThreadRef) ?? []), node.threadRef])
  return children
}

export const readClaudeHistoryCatalog = (projectsRoot: string, graph = buildClaudeHistoryGraph(projectsRoot)): CodexHistoryCatalog => {
  const children = childIndex(graph)
  const agents = graph.nodes.map(node => toAgent(node, graph, children))
  return { roots: agents.filter(agent => agent.parentThreadRef === null).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)), agents }
}

/** Orphan/topology-gap accounting for the loss-accounting oracle. */
export const claudeHistoryTopology = (graph: ClaudeHistoryGraph): Readonly<{ childFiles: number; linked: number; orphans: number }> => {
  const childNodes = graph.nodes.filter(node => node.entry.kind === "child")
  const orphans = childNodes.filter(node => node.orphan).length
  return { childFiles: childNodes.length, linked: childNodes.length - orphans, orphans }
}

// ---------------------------------------------------------------------------
// Page projection — one item per JSONL record (loss-accounted like Codex)
// ---------------------------------------------------------------------------

const AGENT_TOOL_NAMES = new Set(["Agent", "Task"])

const contentBlocks = (content: unknown): RecordValue[] => Array.isArray(content) ? content.flatMap(part => { const block = object(part); return block === null ? [] : [block] }) : []
const toolResultText = (content: unknown): string => typeof content === "string" ? content : Array.isArray(content) ? content.map(part => { const block = object(part); return block === null ? "" : safeText(block.text ?? block.content) }).filter(Boolean).join("\n") : safeText(content)

const projectClaudeRow = (row: unknown, threadRef: string, sequence: number): CodexHistoryItem => {
  const record = object(row)
  const timestamp = record === null ? new Date(0).toISOString() : iso(record.timestamp) ?? new Date(0).toISOString()
  const base = { itemRef: `${threadRef}:${sequence}`, threadRef, sequence, timestamp }
  if (record === null) return { ...base, kind: "gap", label: "Unreadable source record", summary: "This record could not be decoded.", status: "unsupported", fields: [], redacted: false, sourceType: "invalid" }
  const recordType = string(record.type) ?? "unknown"
  const fields: Array<{ label: string; value: string; redacted: boolean }> = []
  const push = (label: string, value: unknown): void => { const text = redactCodexHistoryText(safeText(value)); if (text.text !== "") fields.push({ label, value: text.text, redacted: text.redacted }) }

  let kind: CodexHistoryItemKind = "gap"; let label = recordType; let summary = ""; let status: string | null = null; let toolUseId: string | null = null

  if (recordType === "summary") { kind = "context"; label = "History compacted"; summary = safeText(record.summary) }
  else if (recordType === "system") { kind = "system_message"; label = "System"; summary = messageText((object(record.message)?.content) ?? record.content) ?? safeText(record.content ?? record.text); if (record.isApiErrorMessage === true) { kind = "error"; label = "API error"; status = "error" } }
  else if (recordType === "user" || recordType === "assistant") {
    const message = object(record.message)
    const content = message === null ? record.content : message.content
    const blocks = contentBlocks(content)
    const toolUse = blocks.find(block => string(block.type) === "tool_use")
    const toolResult = blocks.find(block => string(block.type) === "tool_result")
    const thinking = blocks.find(block => string(block.type) === "thinking")
    const text = messageText(content)
    if (toolUse !== undefined && recordType === "assistant") {
      const name = string(toolUse.name) ?? "tool"
      toolUseId = string(toolUse.id)
      const isAgent = AGENT_TOOL_NAMES.has(name)
      kind = isAgent ? "collaboration" : "tool_call"
      label = isAgent ? "Spawn agent" : name
      const input = object(toolUse.input)
      summary = isAgent ? safeText(input?.description ?? input?.prompt ?? input?.subagent_type) : safeText(toolUse.input)
      if (isAgent) { push("agent type", input?.subagent_type); push("background", input?.run_in_background); push("isolation", input?.isolation); push("model", input?.model) }
      else { push("input", toolUse.input) }
      push("call", toolUseId)
    } else if (toolResult !== undefined && recordType === "user") {
      kind = "tool_result"; label = "Tool result"
      const result = object(record.toolUseResult)
      summary = toolResultText(toolResult.content) || safeText(result?.stdout ?? result?.content)
      status = toolResult.is_error === true ? "error" : "completed"
      push("call", toolResult.tool_use_id)
      if (result !== null) { push("agent", result.agentId); push("status", result.status); push("files", result.totalToolUseCount ?? undefined) }
    } else if (thinking !== undefined && text === null) {
      kind = "reasoning"; label = "Reasoning summary"; summary = safeText(thinking.thinking)
    } else if (text !== null) {
      const injectedAgents = recordType === "user" && text.startsWith("# AGENTS.md instructions for ")
      kind = injectedAgents ? "metadata" : recordType === "assistant" ? "assistant_message" : "user_message"
      label = injectedAgents ? "Agent metadata" : recordType === "assistant" ? "Assistant" : "You"
      summary = text
    } else if (blocks.some(block => string(block.type) === "image")) {
      kind = recordType === "assistant" ? "assistant_message" : "user_message"; label = recordType === "assistant" ? "Assistant" : "You"; summary = "[image content]"
    }
  }

  const redactedSummary = redactCodexHistoryText(summary || label)
  const redacted = redactedSummary.redacted || fields.some(field => field.redacted)
  return { ...base, kind, label: label.slice(0, 160), summary: redactedSummary.text, status, fields: fields.map(({ label, value }) => ({ label, value })), redacted, sourceType: `${recordType}${toolUseId === null ? "" : "/tool_use"}`.slice(0, 160) }
}

/** Supported/gap accounting mirrors `projectClaudeRow` without projecting. */
const projectionAccounting = (row: unknown): Readonly<{ gap: boolean }> => {
  const record = object(row)
  if (record === null) return { gap: true }
  const recordType = string(record.type) ?? "unknown"
  if (recordType === "summary" || recordType === "system") return { gap: false }
  if (recordType !== "user" && recordType !== "assistant") return { gap: true }
  const message = object(record.message)
  const content = message === null ? record.content : message.content
  const blocks = contentBlocks(content)
  const supported = blocks.some(block => ["tool_use", "tool_result", "thinking", "text", "image"].includes(string(block.type) ?? "")) || typeof content === "string"
  return { gap: !supported }
}

const previewable = (item: CodexHistoryItem): boolean =>
  ["user_message", "assistant_message", "reasoning", "tool_call", "tool_result", "collaboration", "error"].includes(item.kind) && item.summary.trim() !== "" && !item.summary.startsWith("[REDACTED:")

const childPreview = (node: ClaudeNode, agent: CodexHistoryAgent): CodexHistoryAgentPreview => {
  const rows = tailText(node.entry.file).split("\n").flatMap(line => { const row = safeJson(line); return row === null ? [] : [row] })
  const latest = rows.map((row, index) => projectClaudeRow(row, node.threadRef, index)).reverse().find(previewable)
  return { threadRef: node.threadRef, title: agent.title, status: agent.status, updatedAt: agent.updatedAt, latest: latest === undefined ? null : { label: latest.label, summary: latest.summary.slice(0, 360), kind: latest.kind, timestamp: latest.timestamp } }
}

export const readClaudeHistoryPage = (input: Readonly<{ projectsRoot: string; threadRef: string; offset?: number; limit?: number }>, graph = buildClaudeHistoryGraph(input.projectsRoot)): CodexHistoryPage | null => {
  const node = graph.byRef.get(input.threadRef)
  if (node === undefined) return null
  let text: string
  try { text = readFileSync(node.entry.file, "utf8") } catch { return null }
  const rows = text.split("\n").filter(line => line !== "").map(safeJson)
  const offset = Math.max(0, Math.min(input.offset ?? 0, rows.length))
  const limit = Math.max(1, Math.min(input.limit ?? 200, 500))

  // Resolve Agent tool_use ids to child agentIds for related-agent previews.
  const toolUseToChild = new Map<string, string>()
  for (const row of rows) { const record = object(row); const result = record === null ? null : object(record.toolUseResult); const child = result === null ? null : string(result.agentId); const call = result === null ? null : string(result.tool_use_id) ?? string(record?.parentToolUseID); if (child !== null && call !== null) toolUseToChild.set(call, child) }

  const children = childIndex(graph)
  const previewByRef = new Map<string, CodexHistoryAgentPreview>()
  const items = rows.slice(offset, offset + limit).map((row, index) => {
    const item = projectClaudeRow(row, node.threadRef, offset + index)
    if (item.kind !== "collaboration") return item
    const call = item.fields.find(field => field.label === "call")?.value
    const childId = call === undefined ? undefined : toolUseToChild.get(call)
    if (childId === undefined) return item
    const childNode = graph.byRef.get(ref(childId))
    if (childNode === undefined) return { ...item, fields: [...item.fields, { label: "history", value: "Child history not recorded" }] }
    let preview = previewByRef.get(childNode.threadRef)
    if (preview === undefined) { preview = childPreview(childNode, toAgent(childNode, graph, children)); previewByRef.set(childNode.threadRef, preview) }
    return { ...item, relatedAgent: preview }
  })

  // Whole-conversation loss accounting. Gaps = records that project to no
  // supported kind. Inline credential scrubbing (Bearer/sk- patterns) keeps the
  // record RENDERED with an inline [REDACTED] marker — exactly as the Codex
  // importer does — so `redactions` here counts only omitted content, of which
  // this projection produces none. source = rendered + redactions + gaps holds.
  const gaps = rows.reduce((sum: number, row) => sum + (projectionAccounting(row).gap ? 1 : 0), 0)
  const rootRef = rootOf(node, graph.byRef)
  const subtree = graph.nodes.filter(candidate => { let ref_: string | null = candidate.threadRef; const seen = new Set<string>(); while (ref_ !== null && !seen.has(ref_)) { if (ref_ === rootRef) return true; seen.add(ref_); ref_ = graph.byRef.get(ref_)?.parentThreadRef ?? null } return false })

  return {
    rootThreadRef: rootRef,
    selectedThreadRef: node.threadRef,
    agents: subtree.map(candidate => toAgent(candidate, graph, children)),
    items,
    offset,
    limit,
    totalItems: rows.length,
    hasPrevious: offset > 0,
    hasNext: offset + limit < rows.length,
    completeness: { source: rows.length, rendered: rows.length - gaps, redactions: 0, gaps, complete: true },
  }
}
