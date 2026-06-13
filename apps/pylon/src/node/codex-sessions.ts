// #4951 Codex external sessions — mirror the Claude JSONL tailer for
// ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl.

import { readdirSync, statSync } from "node:fs"
import { join } from "node:path"
import { tailLines, type ExternalEvent, type ExternalSession } from "./external-sessions"

function clip(value: string, max = 200): string {
  const oneLine = value.replace(/\s+/g, " ").trim()
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine
}

function textFromContent(content: unknown, blockType: string): string {
  if (typeof content === "string") return content
  if (!Array.isArray(content)) return ""
  const block = content.find((b: any) => b?.type === blockType && typeof b.text === "string")
  return block?.text ?? ""
}

function asRecord(value: unknown): Record<string, unknown> {
  if (value !== null && typeof value === "object") return value as Record<string, unknown>
  if (typeof value !== "string") return {}
  try {
    const parsed = JSON.parse(value)
    return parsed !== null && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

function summarizeTool(name: string, input: unknown): string {
  const rec = asRecord(input)
  const str = (k: string): string => (typeof rec[k] === "string" ? (rec[k] as string) : "")
  if (name === "exec_command") return `${name}: ${clip(str("cmd"), 160)}`
  if (name === "apply_patch" && typeof input === "string") return `${name}: ${clip(input, 160)}`
  const preview = typeof input === "string" ? clip(input, 160) : clip(JSON.stringify(rec), 160)
  return preview.length > 0 ? `${name}: ${preview}` : `${name}: ...`
}

// Normalize one Codex rollout JSONL object into a compact timeline event.
export function normalizeCodexLine(raw: unknown): ExternalEvent | null {
  if (raw === null || typeof raw !== "object") return null
  const o = raw as Record<string, any>
  const type = typeof o.type === "string" ? o.type : ""
  const payload = o.payload && typeof o.payload === "object" ? (o.payload as Record<string, any>) : {}
  const payloadType = typeof payload.type === "string" ? payload.type : ""
  const observedAt = typeof o.timestamp === "string" ? o.timestamp : new Date().toISOString()

  if (type === "event_msg") {
    if (payloadType === "agent_message" && typeof payload.message === "string" && payload.message.trim().length > 0) {
      return { observedAt, phase: "agent_message", messageText: `agent: ${clip(payload.message)}` }
    }
    if (payloadType === "user_message" && typeof payload.message === "string" && payload.message.trim().length > 0) {
      return { observedAt, phase: "user", messageText: `you: ${clip(payload.message)}` }
    }
    if (payloadType === "patch_apply_end") {
      const text = typeof payload.stdout === "string" && payload.stdout.trim().length > 0 ? payload.stdout : String(payload.status ?? "")
      return { observedAt, phase: "tool_result", messageText: `result: ${clip(text, 160)}` }
    }
    if (payloadType === "task_complete" && typeof payload.last_agent_message === "string" && payload.last_agent_message.trim().length > 0) {
      return { observedAt, phase: "agent_message", messageText: `agent: ${clip(payload.last_agent_message)}` }
    }
    return null
  }

  if (type !== "response_item") return null

  if (payloadType === "reasoning") return { observedAt, phase: "reasoning", messageText: "reasoning" }

  if (payloadType === "message") {
    const role = typeof payload.role === "string" ? payload.role : ""
    if (role === "assistant") {
      const text = textFromContent(payload.content, "output_text")
      return text.trim().length > 0 ? { observedAt, phase: "agent_message", messageText: `agent: ${clip(text)}` } : null
    }
    if (role === "user") {
      const text = textFromContent(payload.content, "input_text")
      return text.trim().length > 0 ? { observedAt, phase: "user", messageText: `you: ${clip(text)}` } : null
    }
    return null
  }

  if (payloadType === "function_call" || payloadType === "custom_tool_call") {
    const name = typeof payload.name === "string" ? payload.name : "tool"
    return { observedAt, phase: "tool_use", messageText: summarizeTool(name, payload.arguments ?? payload.input) }
  }

  if (payloadType === "function_call_output" || payloadType === "custom_tool_call_output") {
    const output = typeof payload.output === "string" ? payload.output : JSON.stringify(payload)
    return { observedAt, phase: "tool_result", messageText: `result: ${clip(output, 160)}` }
  }

  return null
}

export function buildCodexSession(input: {
  sessionId: string
  lines: string[]
  mtimeMs: number
  nowMs: number
  parentRef?: string | null
  maxEvents?: number
}): ExternalSession {
  const events: ExternalEvent[] = []
  for (const line of input.lines) {
    let parsed: ExternalEvent | null
    try {
      parsed = normalizeCodexLine(JSON.parse(line))
    } catch {
      parsed = null
    }
    if (parsed !== null) events.push(parsed)
  }
  const tail = events.slice(-(input.maxEvents ?? 100))
  const state = input.nowMs - input.mtimeMs < 90_000 ? "running" : "idle"
  return {
    sessionRef: `codex:${input.sessionId}`,
    agentKind: "codex",
    parentRef: input.parentRef ?? null,
    state,
    title: input.sessionId,
    latestActivity: tail.length > 0 ? tail[tail.length - 1].messageText : "(no activity)",
    events: tail,
  }
}

export function scanCodexSessions(opts: {
  sessionsRoot: string
  nowMs: number
  maxAgeMs: number
  maxSessions: number
}): ExternalSession[] {
  type Candidate = { sessionId: string; path: string; mtimeMs: number }
  const candidates: Candidate[] = []
  let years: string[] = []
  try {
    years = readdirSync(opts.sessionsRoot)
  } catch {
    return []
  }

  for (const year of years) {
    const yearDir = join(opts.sessionsRoot, year)
    let months: string[] = []
    try {
      months = readdirSync(yearDir)
    } catch {
      continue
    }
    for (const month of months) {
      const monthDir = join(yearDir, month)
      let days: string[] = []
      try {
        days = readdirSync(monthDir)
      } catch {
        continue
      }
      for (const day of days) {
        const dayDir = join(monthDir, day)
        let entries: string[] = []
        try {
          entries = readdirSync(dayDir)
        } catch {
          continue
        }
        for (const entry of entries) {
          if (!entry.startsWith("rollout-") || !entry.endsWith(".jsonl")) continue
          const path = join(dayDir, entry)
          let st
          try {
            st = statSync(path)
          } catch {
            continue
          }
          if (!st.isFile() || opts.nowMs - st.mtimeMs > opts.maxAgeMs) continue
          candidates.push({ sessionId: entry.replace(/\.jsonl$/, ""), path, mtimeMs: st.mtimeMs })
        }
      }
    }
  }

  candidates.sort((a, b) => b.mtimeMs - a.mtimeMs)
  const out: ExternalSession[] = []
  for (const c of candidates.slice(0, opts.maxSessions)) {
    try {
      out.push(buildCodexSession({ sessionId: c.sessionId, lines: tailLines(c.path), mtimeMs: c.mtimeMs, nowMs: opts.nowMs }))
    } catch {
      // unreadable file -> skip
    }
  }
  return out
}
