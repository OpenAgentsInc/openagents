/**
 * Typed tool-call cards (EP250, #8712).
 *
 * Owner statement (verbatim): "why don't you go improve the UI of those tool
 * calls so it's not just JSON stuff? Like I thought we had some custom
 * components that showed things properly, not these JSON blobs."
 *
 * This module projects the flat transcript note list into typed entries:
 * ordinary chat notes, interactive question cards, and ONE tool card per tool
 * invocation. A `started` trace note opens a card; the matching `ok`/`failed`
 * trace note UPDATES that card in place (status chip + result line) instead
 * of rendering a second SYSTEM row. The bounded raw args/result stay
 * reachable behind a compact expand affordance — never the default rendering.
 *
 * Pairing honesty: the trace notes carry no tool-invocation id across the
 * note boundary (the fable-local events are toolName + summary only), so
 * completion notes pair with the OLDEST still-open card of the same toolName
 * in note order (FIFO). That matches how the SDK emits tool_use/tool_result
 * sequences today; if typed invocation ids ever ride the events, key on them
 * instead.
 *
 * Presentation vocabulary (icons, humanized titles) follows the Codex
 * history workspace (`./history-workspace.ts`, #8674) — the in-repo precedent
 * for "custom components that showed things properly".
 */
import type { IconName } from "@effect-native/core"

import type { DesktopToolTrace } from "../chat-contract.ts"
import { parseFableLocalTraceNoteText } from "../fable-local-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"

export type ToolCardStatus = "running" | "ok" | "failed"

export type ToolCardModel = Readonly<{
  /** The started note's key — stable across the in-place completion update. */
  key: string
  toolName: string
  timestamp: string
  status: ToolCardStatus
  /** Bounded raw args summary (usually redacted JSON) — expandable only. */
  argsSummary: string
  /** Bounded raw result / failure text; null while running. */
  resultSummary: string | null
}>

export type TranscriptEntry =
  | Readonly<{ kind: "note"; note: DesktopNoteEntry }>
  | Readonly<{ kind: "question"; note: DesktopNoteEntry }>
  | Readonly<{ kind: "tool"; card: ToolCardModel }>

/** Typed trace facts for a note: typed meta first, text-parse fallback. */
export const toolTraceFromNote = (note: DesktopNoteEntry): DesktopToolTrace | null => {
  if (note.role !== "system") return null
  if (note.meta?.trace !== undefined) return note.meta.trace
  return parseFableLocalTraceNoteText(note.text)
}

/**
 * Projects notes into transcript entries, folding started + ok/failed trace
 * pairs into single updating tool cards.
 */
export const projectTranscriptEntries = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<TranscriptEntry> => {
  const entries: Array<TranscriptEntry> = []
  // Open (still-running) cards by entries index, FIFO per toolName.
  const openByTool = new Map<string, Array<number>>()
  for (const note of notes) {
    if (note.question !== undefined) {
      entries.push({ kind: "question", note })
      continue
    }
    const trace = toolTraceFromNote(note)
    if (trace === null) {
      entries.push({ kind: "note", note })
      continue
    }
    if (trace.phase === "started") {
      const index = entries.length
      entries.push({
        kind: "tool",
        card: {
          key: note.key,
          toolName: trace.toolName,
          timestamp: note.timestamp,
          status: "running",
          argsSummary: trace.summary,
          resultSummary: null,
        },
      })
      const queue = openByTool.get(trace.toolName) ?? []
      queue.push(index)
      openByTool.set(trace.toolName, queue)
      continue
    }
    const queue = openByTool.get(trace.toolName) ?? []
    const openIndex = queue.shift()
    const status: ToolCardStatus = trace.phase === "ok" ? "ok" : "failed"
    if (openIndex === undefined) {
      // A completion with no visible invocation (e.g. the page began after
      // the started line): render it as its own already-completed card.
      entries.push({
        kind: "tool",
        card: {
          key: note.key,
          toolName: trace.toolName,
          timestamp: note.timestamp,
          status,
          argsSummary: "",
          resultSummary: trace.summary === "" ? null : trace.summary,
        },
      })
      continue
    }
    const open = entries[openIndex]
    if (open !== undefined && open.kind === "tool") {
      entries[openIndex] = {
        kind: "tool",
        card: {
          ...open.card,
          status,
          resultSummary: trace.summary === "" ? null : trace.summary,
        },
      }
    }
  }
  return entries
}

// ---------------------------------------------------------------------------
// Humanization — one primary line per known tool, bounded compact fallback
// for unknown tools. Never a raw JSON dump as the default rendering.
// ---------------------------------------------------------------------------

const DETAIL_LIMIT = 160
const UNKNOWN_ARGS_LIMIT = 140

const truncate = (value: string, limit: number): string =>
  value.length <= limit ? value : `${value.slice(0, Math.max(0, limit - 1))}…`

/**
 * Bounded-tolerant arg reader: trace summaries are redacted JSON truncated at
 * the summary limit, so strict JSON.parse can fail mid-string. Fall back to
 * extracting `"key":"value"` string pairs from the bounded text.
 */
export const readToolArgs = (argsSummary: string): Readonly<Record<string, string>> => {
  const text = argsSummary.trim()
  if (text === "") return {}
  try {
    const parsed: unknown = JSON.parse(text)
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      const out: Record<string, string> = {}
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value === "string") out[key] = value
        else if (typeof value === "number" || typeof value === "boolean") out[key] = String(value)
      }
      return out
    }
  } catch {
    // Truncated JSON: extract complete string fields only.
    const out: Record<string, string> = {}
    for (const match of text.matchAll(/"([A-Za-z0-9_]+)"\s*:\s*"((?:[^"\\]|\\.)*)"/g)) {
      const key = match[1]
      const raw = match[2]
      if (key === undefined || raw === undefined) continue
      try {
        out[key] = JSON.parse(`"${raw}"`) as string
      } catch {
        out[key] = raw
      }
    }
    return out
  }
  return {}
}

/** Compact `key: value` summary for unknown tools — string fields only. */
export const compactArgSummary = (args: Readonly<Record<string, string>>): string => {
  const parts: Array<string> = []
  let used = 0
  for (const [key, value] of Object.entries(args)) {
    const part = `${key}: ${truncate(value.replace(/\s+/g, " ").trim(), 60)}`
    if (used + part.length > UNKNOWN_ARGS_LIMIT) break
    parts.push(part)
    used += part.length + 3
  }
  return parts.join(" · ")
}

export type HumanizedToolInvocation = Readonly<{
  title: string
  detail: string
}>

const prettyToolName = (toolName: string): string =>
  toolName.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase())

/** The humanization table — one primary line per known tool. */
export const humanizeToolInvocation = (
  toolName: string,
  argsSummary: string,
): HumanizedToolInvocation => {
  const args = readToolArgs(argsSummary)
  const detail = (value: string | undefined): string =>
    value === undefined ? "" : truncate(value.replace(/\s+/g, " ").trim(), DETAIL_LIMIT)
  switch (toolName) {
    case "mcp__codex__delegate":
      return { title: "Delegate to Codex", detail: detail(args["task"]) }
    case "Agent": {
      const description = detail(args["description"])
      const model = args["model"]
      return {
        title: "Agent",
        detail: model === undefined || model === ""
          ? description
          : description === "" ? model : `${description} · ${model}`,
      }
    }
    case "Bash":
      return {
        title: "Bash",
        detail: detail(args["description"]) !== ""
          ? detail(args["description"])
          : detail(args["command"]),
      }
    case "Read":
      return { title: "Read", detail: detail(args["file_path"]) }
    case "Write":
      return { title: "Write", detail: detail(args["file_path"]) }
    case "Edit":
      return { title: "Edit", detail: detail(args["file_path"]) }
    case "Glob":
      return { title: "Glob", detail: detail(args["pattern"]) }
    case "Grep":
      return { title: "Grep", detail: detail(args["pattern"]) }
    case "ToolSearch":
      return { title: "Tool search", detail: detail(args["query"]) }
    case "WebSearch":
      return { title: "Web search", detail: detail(args["query"]) }
    case "WebFetch":
      return { title: "Web fetch", detail: detail(args["url"]) }
    default:
      return { title: prettyToolName(toolName), detail: compactArgSummary(args) }
  }
}

/** Bounded first-line result snippet for a successful invocation. */
export const toolResultSnippet = (resultSummary: string): string => {
  const firstLine = resultSummary.split("\n").find((line) => line.trim() !== "") ?? ""
  return truncate(firstLine.trim(), 200)
}

/** Same icon vocabulary the Codex history workspace uses for tool items. */
export const toolCardIcon = (toolName: string): IconName => {
  if (toolName === "Bash") return "Terminal"
  if (["Edit", "Write", "NotebookEdit"].includes(toolName)) return "Code"
  if (["Read", "Glob", "Grep"].includes(toolName)) return "Folder"
  if (toolName === "Agent" || toolName.startsWith("mcp__codex__")) return "Agent"
  return "Tools"
}
