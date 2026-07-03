// #4951 Codex external sessions — mirror the Claude JSONL tailer for
// ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl.

import { readdirSync, statSync } from "node:fs"
import { join } from "node:path"
import {
  stableExternalSessionRef,
  tailLines,
  type ExternalEvent,
  type ExternalSession,
} from "./external-sessions.js"

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

function reasoningSummaryText(summary: unknown): string {
  if (typeof summary === "string") return summary.replace(/\s+/g, " ").trim()
  if (!Array.isArray(summary)) return ""
  return summary
    .map((entry) => {
      if (typeof entry === "string") return entry
      if (entry !== null && typeof entry === "object" && typeof (entry as { text?: unknown }).text === "string") {
        return (entry as { text: string }).text
      }
      return ""
    })
    .join(" ")
    .replace(/\s+/g, " ")
    .trim()
}

function reasoningText(payload: Record<string, any>): string {
  for (const key of ["text", "message", "content", "thinking"]) {
    const value = payload[key]
    if (typeof value === "string" && value.trim().length > 0) {
      return value.replace(/\s+/g, " ").trim()
    }
    if (Array.isArray(value)) {
      const text = value
        .map((entry) =>
          entry !== null &&
          typeof entry === "object" &&
          typeof (entry as { text?: unknown }).text === "string"
            ? (entry as { text: string }).text
            : "",
        )
        .join(" ")
        .replace(/\s+/g, " ")
        .trim()
      if (text.length > 0) return text
    }
  }
  return reasoningSummaryText(payload.summary)
}

function numberOrZero(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function tokenUsageMessage(info: unknown): string | null {
  if (info === null || typeof info !== "object") return null
  const record = info as Record<string, unknown>
  const usage =
    record.last_token_usage !== null && typeof record.last_token_usage === "object"
      ? record.last_token_usage as Record<string, unknown>
      : record.total_token_usage !== null && typeof record.total_token_usage === "object"
        ? record.total_token_usage as Record<string, unknown>
        : record
  const output = numberOrZero(usage.output_tokens)
  const reasoning = numberOrZero(usage.reasoning_output_tokens)
  return output === 0 && reasoning === 0
    ? null
    : `thinking tokens: ${reasoning}; output tokens: ${output}`
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

function rolloutIdFromSessionId(sessionId: string): string | null {
  const match = /([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i.exec(sessionId)
  return match?.[1] ?? null
}

function titleFromCodexRaw(raw: unknown): string | null {
  if (raw === null || typeof raw !== "object") return null
  const o = raw as Record<string, any>
  const type = typeof o.type === "string" ? o.type : ""
  const payload = o.payload && typeof o.payload === "object" ? (o.payload as Record<string, any>) : {}
  const payloadType = typeof payload.type === "string" ? payload.type : ""

  if (
    type === "event_msg" &&
    payloadType === "user_message" &&
    typeof payload.message === "string" &&
    payload.message.trim().length > 0
  ) {
    return clip(payload.message, 80)
  }

  if (type === "response_item" && payloadType === "message") {
    const role = typeof payload.role === "string" ? payload.role : ""
    const text = role === "user"
      ? textFromContent(payload.content, "input_text")
      : role === "assistant"
        ? textFromContent(payload.content, "output_text")
        : ""
    return text.trim().length > 0 ? clip(text, 80) : null
  }

  return null
}

function fallbackCodexTitle(sessionId: string): string {
  return `Codex session ${sessionId.replace(/^rollout-/, "").slice(0, 24)}`
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
    if (payloadType === "task_started") {
      return { observedAt, phase: "started", messageText: "task started" }
    }
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
    if (payloadType === "task_complete") {
      return { observedAt, phase: "completed", messageText: "task complete" }
    }
    if (payloadType === "token_count") {
      const message = tokenUsageMessage(payload.info)
      return message === null ? null : { observedAt, phase: "reasoning", messageText: message }
    }
    return null
  }

  if (type !== "response_item") return null

  if (payloadType === "reasoning") {
    const text = reasoningText(payload)
    if (text.length === 0 && typeof payload.encrypted_content === "string") return null
    return {
      observedAt,
      phase: "reasoning",
      messageText: text.length > 0 ? `thinking: ${clip(text, 1800)}` : "thinking…",
      ...(text.length > 0 ? { messageFull: `thinking: ${text.slice(0, 8000)}` } : {}),
    }
  }

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
  sourceRef?: string
  parentRef?: string | null
  maxEvents?: number
}): ExternalSession {
  const events: ExternalEvent[] = []
  let rolloutSessionId: string | null = rolloutIdFromSessionId(input.sessionId)
  let title: string | null = null
  for (const line of input.lines) {
    let raw: unknown
    let parsed: ExternalEvent | null
    try {
      raw = JSON.parse(line)
      if (
        rolloutSessionId === null &&
        raw !== null &&
        typeof raw === "object" &&
        (raw as { type?: unknown }).type === "session_meta"
      ) {
        const payload = (raw as { payload?: unknown }).payload
        const id =
          payload !== null && typeof payload === "object"
            ? (payload as { id?: unknown }).id
            : null
        if (typeof id === "string" && id.trim() !== "") rolloutSessionId = id
      }
      if (title === null) title = titleFromCodexRaw(raw)
      parsed = normalizeCodexLine(raw)
    } catch {
      parsed = null
    }
    if (parsed !== null) events.push(parsed)
  }
  const tail = events.slice(-(input.maxEvents ?? 100))
  const state = input.nowMs - input.mtimeMs < 90_000 ? "running" : "idle"
  const sessionRef = stableExternalSessionRef(
    "session.pylon.codex_external",
    input.sourceRef ?? input.sessionId,
  )
  const aliasSessionRefs = [
    `codex:${input.sessionId}`,
    ...(rolloutSessionId === null
      ? []
      : [stableExternalSessionRef("session.pylon.codex_composer", rolloutSessionId)]),
  ]
  return {
    sessionRef,
    aliasSessionRefs,
    agentKind: "codex",
    parentRef: input.parentRef ?? null,
    state,
    title: title ?? fallbackCodexTitle(input.sessionId),
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
      out.push(buildCodexSession({
        sessionId: c.sessionId,
        lines: tailLines(c.path),
        mtimeMs: c.mtimeMs,
        nowMs: opts.nowMs,
        sourceRef: c.path,
      }))
    } catch {
      // unreadable file -> skip
    }
  }
  return out
}
