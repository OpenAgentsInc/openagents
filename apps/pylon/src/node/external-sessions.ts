// #4951 external agent sessions — surface a host Claude Code / Codex session
// (the MAIN conversation, Pylon-managed or not) into Autopilot by tailing its
// JSONL log. Pure parsing here; the node poller (index.ts) does the IO + merges
// the results into session.list / session.events. Read-only.
//
// Each raw line is normalized to a concise one-line activity (+ a phase), so the
// existing timeline renders it compactly; detail-on-expand is layered later.

import { createHash } from "node:crypto"
import { openSync, readSync, closeSync, fstatSync, readdirSync, statSync } from "node:fs"
import { join } from "node:path"
import { expandClaudeMessage } from "./claude-blocks.js"

export type ExternalAgentKind = "claude" | "codex"

export type ExternalEvent = {
  observedAt: string
  phase: string
  messageText: string
  // Full untruncated content for tap-to-expand (agent text, tool input, result).
  messageFull?: string
}

export type ExternalSession = {
  sessionRef: string
  aliasSessionRefs?: string[]
  agentKind: ExternalAgentKind
  parentRef: string | null
  state: string
  title: string
  latestActivity: string
  events: ExternalEvent[]
}

function clip(value: string, max = 200): string {
  const oneLine = value.replace(/\s+/g, " ").trim()
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine
}

export function stableExternalSessionRef(prefix: string, value: string): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

// Read only the last `maxBytes` of a (possibly huge) file, dropping the first
// partial line. Bounds the per-poll cost for multi-MB session logs.
export function tailLines(path: string, maxBytes = 262_144): string[] {
  const fd = openSync(path, "r")
  try {
    const size = fstatSync(fd).size
    const start = size > maxBytes ? size - maxBytes : 0
    const length = size - start
    const buf = Buffer.alloc(length)
    readSync(fd, buf, 0, length, start)
    const text = buf.toString("utf8")
    const lines = text.split("\n").filter((l) => l.trim().length > 0)
    return start > 0 && lines.length > 0 ? lines.slice(1) : lines
  } finally {
    closeSync(fd)
  }
}

// Concise summary of a Claude tool_use block.
function summarizeToolUse(name: string, input: unknown): string {
  const rec = (input ?? {}) as Record<string, unknown>
  const str = (k: string): string => (typeof rec[k] === "string" ? (rec[k] as string) : "")
  switch (name) {
    case "Bash":
      return `Bash: ${clip(str("command"), 160)}`
    case "Read":
      return `Read ${clip(str("file_path"), 120)}`
    case "Edit":
    case "Write":
    case "NotebookEdit":
      return `${name} ${clip(str("file_path"), 120)}`
    case "Grep":
      return `Grep ${clip(str("pattern"), 80)}`
    case "Glob":
      return `Glob ${clip(str("pattern"), 80)}`
    case "Task":
    case "Agent":
      return `→ sub-agent: ${clip(str("description") || str("prompt"), 120)}`
    case "TodoWrite":
      return "TodoWrite (plan update)"
    default: {
      const preview = clip(JSON.stringify(rec), 100)
      return `${name}: ${preview}`
    }
  }
}

// Normalize one Claude JSONL object. Returns an event, a {title}, or null (noise).
export function normalizeClaudeLine(raw: unknown): ExternalEvent | { title: string } | null {
  if (raw === null || typeof raw !== "object") return null
  const o = raw as Record<string, any>
  const type = typeof o.type === "string" ? o.type : ""
  const observedAt = typeof o.timestamp === "string" ? o.timestamp : new Date().toISOString()

  if (type === "ai-title" && typeof o.aiTitle === "string") return { title: o.aiTitle }
  if (type === "summary" && typeof o.summary === "string") return { title: o.summary }

  if (type === "user" || type === "assistant") {
    const message = o.message ?? {}
    const role = type === "user" ? "you" : "agent"
    const phase = type === "user" ? "user" : "agent_message"
    const content = message.content
    const full = (v: string) => (v.length > 8000 ? v.slice(0, 8000) : v)
    if (typeof content === "string") {
      if (content.trim().length === 0) return null
      return { observedAt, phase, messageText: `${role}: ${clip(content)}`, messageFull: full(content) }
    }
    if (Array.isArray(content)) {
      // One event per block would be noisy; collapse to the most salient block,
      // but carry the FULL content on messageFull for tap-to-expand.
      for (const block of content) {
        if (block?.type === "tool_use" && typeof block.name === "string") {
          return {
            observedAt,
            phase: "tool_use",
            messageText: summarizeToolUse(block.name, block.input),
            messageFull: full(`${block.name}\n${JSON.stringify(block.input ?? {}, null, 2)}`),
          }
        }
      }
      for (const block of content) {
        if (block?.type === "tool_result") {
          const c = block.content
          const text = typeof c === "string" ? c : Array.isArray(c) ? (c.find((b: any) => b?.type === "text")?.text ?? "") : ""
          return { observedAt, phase: "tool_result", messageText: `result: ${clip(String(text), 160)}`, messageFull: full(String(text)) }
        }
      }
      const textBlock = content.find((b: any) => b?.type === "text" && typeof b.text === "string")
      if (textBlock && textBlock.text.trim().length > 0) {
        return { observedAt, phase, messageText: `${role}: ${clip(textBlock.text)}`, messageFull: full(textBlock.text) }
      }
      const thinking = content.find((b: any) => b?.type === "thinking" && typeof b.thinking === "string")
      if (thinking) return { observedAt, phase: "reasoning", messageText: "thinking…", messageFull: full(thinking.thinking) }
    }
    return null
  }

  return null // file-history-snapshot, last-prompt, mode, permission-mode, etc.
}

// Build an external session from a Claude session file's tail.
export function buildClaudeSession(input: {
  sessionId: string
  lines: string[]
  mtimeMs: number
  nowMs: number
  parentRef?: string | null
  maxEvents?: number
}): ExternalSession {
  const events: ExternalEvent[] = []
  let title = ""
  for (const line of input.lines) {
    let raw: unknown
    try {
      raw = JSON.parse(line)
    } catch {
      continue
    }
    // Titles via the line normalizer; message bodies via expandClaudeMessage so
    // EVERY content block becomes a discrete event (#4951 "all data").
    const titled = normalizeClaudeLine(raw)
    if (titled !== null && "title" in titled) {
      title = titled.title
      continue
    }
    events.push(...expandClaudeMessage(raw))
  }
  const tail = events.slice(-(input.maxEvents ?? 100))
  // Recent write ⇒ running; otherwise idle.
  const state = input.nowMs - input.mtimeMs < 90_000 ? "running" : "idle"
  return {
    sessionRef: `claude:${input.sessionId}`,
    aliasSessionRefs: [
      stableExternalSessionRef("session.pylon.claude_composer", input.sessionId),
    ],
    agentKind: "claude",
    parentRef: input.parentRef ?? null,
    state,
    title: title || input.sessionId,
    latestActivity: tail.length > 0 ? tail[tail.length - 1].messageText : title || "(no activity)",
    events: tail,
  }
}

// Scan ~/.claude/projects for recently-active session logs (+ their sub-agents,
// linked via parentRef) and build external sessions from each file's tail.
// Bounded by recency (maxAgeMs) and count (maxSessions) so the poll stays cheap.
export function scanClaudeSessions(opts: {
  projectsRoot: string
  nowMs: number
  maxAgeMs: number
  maxSessions: number
}): ExternalSession[] {
  type Candidate = { sessionId: string; path: string; mtimeMs: number; parentRef: string | null }
  const candidates: Candidate[] = []
  let projects: string[] = []
  try {
    projects = readdirSync(opts.projectsRoot)
  } catch {
    return []
  }
  for (const project of projects) {
    const projectDir = join(opts.projectsRoot, project)
    let entries: string[] = []
    try {
      entries = readdirSync(projectDir)
    } catch {
      continue
    }
    for (const entry of entries) {
      if (!entry.endsWith(".jsonl")) continue
      const path = join(projectDir, entry)
      let st
      try {
        st = statSync(path)
      } catch {
        continue
      }
      if (!st.isFile() || opts.nowMs - st.mtimeMs > opts.maxAgeMs) continue
      const sessionId = entry.replace(/\.jsonl$/, "")
      candidates.push({ sessionId, path, mtimeMs: st.mtimeMs, parentRef: null })
      // Sub-agents live under <sessionId>/subagents/agent-*.jsonl.
      const subDir = join(projectDir, sessionId, "subagents")
      let subs: string[] = []
      try {
        subs = readdirSync(subDir)
      } catch {
        subs = []
      }
      for (const sub of subs) {
        if (!sub.endsWith(".jsonl")) continue
        const subPath = join(subDir, sub)
        try {
          const sst = statSync(subPath)
          // Only nest RECENTLY-active sub-agents — a parent chat stays "recent"
          // for its whole life, but long-finished Task sub-agents are stale and
          // shouldn't keep showing under it.
          if (opts.nowMs - sst.mtimeMs > opts.maxAgeMs) continue
          candidates.push({
            sessionId: `${sessionId}.${sub.replace(/\.jsonl$/, "")}`,
            path: subPath,
            mtimeMs: sst.mtimeMs,
            parentRef: `claude:${sessionId}`,
          })
        } catch {
          // skip
        }
      }
    }
  }
  candidates.sort((a, b) => b.mtimeMs - a.mtimeMs)
  const out: ExternalSession[] = []
  for (const c of candidates.slice(0, opts.maxSessions)) {
    try {
      out.push(
        buildClaudeSession({
          sessionId: c.sessionId,
          lines: tailLines(c.path),
          mtimeMs: c.mtimeMs,
          nowMs: opts.nowMs,
          parentRef: c.parentRef,
        }),
      )
    } catch {
      // unreadable file -> skip
    }
  }
  return out
}

// Map an external session into a session.list entry (the loose shape the
// clients consume). state is coerced into the SessionState enum so the desktop's
// strict decode passes; the real activity is on latestActivity.
export function toSessionListEntry(s: ExternalSession, nowIso: string): Record<string, unknown> {
  return {
    sessionRef: s.sessionRef,
    ...(s.title.trim().length === 0 ? {} : { title: s.title }),
    adapter: s.agentKind === "claude" ? "claude_agent" : "codex",
    state: s.state === "running" ? "running" : "completed",
    accountRefHash: null,
    latestActivity: s.latestActivity,
    ...(s.parentRef ? { parentRef: s.parentRef } : {}),
    agentKind: s.agentKind,
    pylonManaged: false,
    artifactRef: null,
    resultRef: null,
    errorClass: null,
    eventCount: s.events.length,
    updatedAt: nowIso,
  }
}

// Map external events into the session.events recentEvents row shape.
export function toEventRows(s: ExternalSession): Array<{
  eventIndex: number
  observedAt: string
  phase: string
  state: string
  messageText: string
  messageFull?: string
}> {
  const state = s.state === "running" ? "running" : "completed"
  return s.events.map((e, i) => ({
    eventIndex: i,
    observedAt: e.observedAt,
    phase: e.phase,
    state,
    messageText: e.messageText,
    ...(e.messageFull ? { messageFull: e.messageFull } : {}),
  }))
}
