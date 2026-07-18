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
import type { WorkbenchItem } from "../workbench-item-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"

export type ToolCardStatus = "running" | "ok" | "failed"

export type ToolCardModel = Readonly<{
  /** The started note's key — stable across the in-place completion update. */
  key: string
  /** Provider invocation identity when the harness exposes one. */
  itemRef?: string
  toolName: string
  timestamp: string
  status: ToolCardStatus
  /** Bounded raw args summary (usually redacted JSON) — expandable only. */
  argsSummary: string
  /** Bounded raw result / failure text; null while running. */
  resultSummary: string | null
  /**
   * Typed item payload (#8859) when the trace note carried one: the
   * structured command/fileChange/toolCall fields wave-2 cards render.
   * Completion payloads supersede started payloads for the same card.
   */
  item?: WorkbenchItem
}>

/**
 * Consecutive context-gathering tools (read/glob/grep — the OpenCode
 * CONTEXT_GROUP_TOOLS port, card-reconciliation pass EP250 #8712) collapse
 * into ONE "Gathered context — N reads, M searches" row with indented
 * member rows. Runs of a single call stay plain tool cards.
 */
export const contextGroupTools = ["Read", "Glob", "Grep"] as const

export type ContextGroupModel = Readonly<{
  /** Stable group key derived from the first member's started-note key. */
  key: string
  cards: ReadonlyArray<ToolCardModel>
  running: boolean
  failed: boolean
  reads: number
  searches: number
}>

export type TranscriptEntry =
  | Readonly<{ kind: "note"; note: DesktopNoteEntry }>
  | Readonly<{ kind: "question"; note: DesktopNoteEntry }>
  /** A runtime-capability card (EP250 wave-2 plan/child/queue) — the note
   * carries the typed `runtime` payload; the shell renders it. */
  | Readonly<{ kind: "runtime"; note: DesktopNoteEntry }>
  | Readonly<{ kind: "tool"; card: ToolCardModel }>
  | Readonly<{ kind: "context-group"; group: ContextGroupModel }>

export type ToolCardTranscriptEntry = Exclude<TranscriptEntry, Readonly<{ kind: "context-group" }>>

const isPrivateCodexCompatibilityNote = (note: DesktopNoteEntry): boolean =>
  note.role === "system" && note.text.startsWith("Codex compatibility notice:")

/** Typed trace facts for a note: typed meta first, text-parse fallback. */
export const toolTraceFromNote = (note: DesktopNoteEntry): DesktopToolTrace | null => {
  if (note.role !== "system") return null
  if (note.meta?.trace !== undefined) return note.meta.trace
  return parseFableLocalTraceNoteText(note.text)
}

const makeContextGroup = (cards: ReadonlyArray<ToolCardModel>): ContextGroupModel => ({
  key: `ctx-${cards[0]!.key}`,
  cards,
  running: cards.some((card) => card.status === "running"),
  failed: cards.some((card) => card.status === "failed"),
  reads: cards.filter((card) => card.toolName === "Read").length,
  searches: cards.filter((card) => card.toolName !== "Read").length,
})

/** "3 reads, 2 searches" — omits zero-count parts, singular/plural honest. */
export const contextGroupSummary = (group: ContextGroupModel): string =>
  [
    ...(group.reads > 0 ? [`${group.reads} read${group.reads === 1 ? "" : "s"}`] : []),
    ...(group.searches > 0 ? [`${group.searches} search${group.searches === 1 ? "" : "es"}`] : []),
  ].join(", ")

/**
 * Folds runs (length >= 2) of consecutive context-gathering tool cards into
 * single context-group entries. Any non-groupable entry (notes, questions,
 * other tools) breaks the run.
 */
export const groupContextEntries = (
  entries: ReadonlyArray<TranscriptEntry>,
): ReadonlyArray<TranscriptEntry> => {
  const out: Array<TranscriptEntry> = []
  let run: Array<ToolCardModel> = []
  const flush = (): void => {
    if (run.length >= 2) out.push({ kind: "context-group", group: makeContextGroup(run) })
    else for (const card of run) out.push({ kind: "tool", card })
    run = []
  }
  for (const entry of entries) {
    if (
      entry.kind === "tool" &&
      (contextGroupTools as ReadonlyArray<string>).includes(entry.card.toolName)
    ) {
      run.push(entry.card)
      continue
    }
    flush()
    out.push(entry)
  }
  flush()
  return out
}

/**
 * Projects notes into transcript entries, folding started + ok/failed trace
 * pairs into single updating tool cards and consecutive read/glob/grep runs
 * into context groups (EP250 card reconciliation).
 */
export const projectTranscriptEntries = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<TranscriptEntry> => groupContextEntries(projectToolCardEntries(notes))

/** The ungrouped per-invocation projection (one card per tool invocation). */
export const projectToolCardEntries = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<ToolCardTranscriptEntry> => {
  const entries: Array<ToolCardTranscriptEntry> = []
  // Open (still-running) cards by entries index, FIFO per toolName.
  const openByTool = new Map<string, Array<number>>()
  const openByRef = new Map<string, number>()
  for (const note of notes) {
    // Builds before the connection-diagnostics fix persisted compatibility
    // receipts as ordinary system notes. Keep those historical diagnostics
    // out of the conversation too; this exact product-owned prefix cannot
    // hide Guardian, rotation, failure, or user/assistant content.
    if (isPrivateCodexCompatibilityNote(note)) continue
    if (note.runtime !== undefined) {
      entries.push({ kind: "runtime", note })
      continue
    }
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
          ...(trace.itemRef === undefined ? {} : { itemRef: trace.itemRef }),
          toolName: trace.toolName,
          timestamp: note.timestamp,
          status: "running",
          argsSummary: trace.summary,
          resultSummary: null,
          ...(trace.item === undefined ? {} : { item: trace.item }),
        },
      })
      const queue = openByTool.get(trace.toolName) ?? []
      queue.push(index)
      openByTool.set(trace.toolName, queue)
      if (trace.itemRef !== undefined) openByRef.set(trace.itemRef, index)
      continue
    }
    if (trace.phase === "progress") {
      const index = trace.itemRef === undefined
        ? openByTool.get(trace.toolName)?.[0]
        : openByRef.get(trace.itemRef)
      if (index === undefined) {
        const nextIndex = entries.length
        entries.push({
          kind: "tool",
          card: {
            key: note.key,
            ...(trace.itemRef === undefined ? {} : { itemRef: trace.itemRef }),
            toolName: trace.toolName,
            timestamp: note.timestamp,
            status: "running",
            argsSummary: "",
            resultSummary: null,
            ...(trace.item === undefined ? {} : { item: trace.item }),
          },
        })
        const queue = openByTool.get(trace.toolName) ?? []
        queue.push(nextIndex)
        openByTool.set(trace.toolName, queue)
        if (trace.itemRef !== undefined) openByRef.set(trace.itemRef, nextIndex)
      } else {
        const open = entries[index]
        if (open?.kind === "tool") {
          entries[index] = {
            kind: "tool",
            card: {
              ...open.card,
              ...(trace.item === undefined ? {} : { item: trace.item }),
            },
          }
        }
      }
      continue
    }
    const queue = openByTool.get(trace.toolName) ?? []
    const exactIndex = trace.itemRef === undefined ? undefined : openByRef.get(trace.itemRef)
    const openIndex = exactIndex ?? queue.shift()
    if (exactIndex !== undefined) {
      const queueIndex = queue.indexOf(exactIndex)
      if (queueIndex !== -1) queue.splice(queueIndex, 1)
    }
    if (trace.itemRef !== undefined) openByRef.delete(trace.itemRef)
    const status: ToolCardStatus = trace.phase === "ok" ? "ok" : "failed"
    if (openIndex === undefined) {
      // A completion with no visible invocation (e.g. the page began after
      // the started line): render it as its own already-completed card.
      entries.push({
        kind: "tool",
        card: {
          key: note.key,
          ...(trace.itemRef === undefined ? {} : { itemRef: trace.itemRef }),
          toolName: trace.toolName,
          timestamp: note.timestamp,
          status,
          argsSummary: "",
          resultSummary: trace.summary === "" ? null : trace.summary,
          ...(trace.item === undefined ? {} : { item: trace.item }),
        },
      })
      continue
    }
    const open = entries[openIndex]
    if (open !== undefined && open.kind === "tool") {
      const item = trace.item ?? open.card.item
      entries[openIndex] = {
        kind: "tool",
        card: {
          ...open.card,
          status,
          resultSummary: trace.summary === "" ? null : trace.summary,
          ...(item === undefined ? {} : { item }),
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

export const truncate = (value: string, limit: number): string =>
  value.length <= limit ? value : `${value.slice(0, Math.max(0, limit - 1))}…`

/**
 * Opaque continuation/message blobs (EP250 owner directive: "spawn agent card
 * is still showing a fucking json object in the card tool thing, not good").
 * Base64-class payloads — long unbroken [A-Za-z0-9+/=_-] runs, e.g. the
 * Fernet-style `gAAAAB…` spawn-agent continuation — NEVER render in a card
 * body. They stay reachable behind the details/inspector affordance only.
 */
export const isOpaqueBlobValue = (value: string): boolean => {
  const text = value.trim()
  // Base64-class: long, unbroken, and letter+digit mixed — a long plain word
  // or path stays presentable (and paths/URLs carry ./: separators anyway).
  return text.length >= 64 && /^[A-Za-z0-9+/=_-]+$/.test(text) && /\d/.test(text) && /[A-Za-z]/.test(text)
}

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

/**
 * Compact `key: value` summary for unknown tools — string fields only, and
 * never an opaque base64-class blob (those stay behind details/inspector).
 */
export const compactArgSummary = (args: Readonly<Record<string, string>>): string => {
  const parts: Array<string> = []
  let used = 0
  for (const [key, value] of Object.entries(args)) {
    if (isOpaqueBlobValue(value)) continue
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

/** Bounded "*** Update File: path" extraction for apply_patch inputs. */
const patchFileSummary = (argsSummary: string): string => {
  const files = [...argsSummary.matchAll(/^\*{3} (?:Update|Add|Delete) File: (.+)$/gm)]
    .map((match) => (match[1] ?? "").trim())
    .filter((value) => value !== "")
  return files.slice(0, 3).join(" · ") + (files.length > 3 ? ` · +${files.length - 3} more` : "")
}

/**
 * The ONE humanization table — chat trace tool names AND Codex-history item
 * labels (EP250: historical tool/collab cards reuse this table; never fork a
 * second one). Opaque blob values never surface through any branch.
 */
export const humanizeToolInvocation = (
  toolName: string,
  argsSummary: string,
): HumanizedToolInvocation => {
  const args = readToolArgs(argsSummary)
  const detail = (value: string | undefined): string =>
    value === undefined || isOpaqueBlobValue(value)
      ? ""
      : truncate(value.replace(/\s+/g, " ").trim(), DETAIL_LIMIT)
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
    // --- Codex-history item labels (EP250 historical card humanization) ---
    case "exec":
    case "exec_command":
    case "write_stdin":
    case "shell":
    case "bash":
    case "command_execution": {
      const command = detail(args["description"]) !== ""
        ? detail(args["description"])
        : detail(args["cmd"]) !== "" ? detail(args["cmd"]) : detail(args["command"])
      return { title: "Terminal", detail: command !== "" ? command : compactArgSummary(args) }
    }
    case "spawn_agent": {
      const task = detail(args["task_name"]) !== ""
        ? detail(args["task_name"])
        : detail(args["agent"]) !== "" ? detail(args["agent"]) : detail(args["name"])
      const meta = [
        ...(detail(args["fork_turns"]) === "" ? [] : [`fork turns: ${detail(args["fork_turns"])}`]),
        ...(detail(args["model"]) === "" ? [] : [detail(args["model"])]),
      ].join(" · ")
      const primary = task === "" ? compactArgSummary(args) : task
      return {
        title: "Spawn agent",
        detail: primary === "" ? meta : meta === "" ? primary : `${primary} · ${meta}`,
      }
    }
    case "apply_patch": {
      const files = patchFileSummary(argsSummary)
      return { title: "Edited files", detail: files !== "" ? truncate(files, DETAIL_LIMIT) : compactArgSummary(args) }
    }
    case "read":
    case "read_file": {
      const target = detail(args["path"]) !== ""
        ? detail(args["path"])
        : detail(args["file_path"]) !== "" ? detail(args["file_path"]) : detail(args["target"])
      return { title: "Read file", detail: target !== "" ? target : compactArgSummary(args) }
    }
    case "list":
    case "glob":
    case "grep":
    case "find": {
      const query = detail(args["pattern"]) !== ""
        ? detail(args["pattern"])
        : detail(args["query"]) !== "" ? detail(args["query"]) : detail(args["path"])
      return { title: "Searched files", detail: query !== "" ? query : compactArgSummary(args) }
    }
    case "web_search":
      return { title: "Web search", detail: detail(args["query"]) !== "" ? detail(args["query"]) : compactArgSummary(args) }
    default: {
      // mcp__server__tool → the tool segment names the card.
      if (toolName.startsWith("mcp__")) {
        const segment = toolName.split("__").filter((part) => part !== "").at(-1) ?? toolName
        return { title: prettyToolName(segment), detail: compactArgSummary(args) }
      }
      return { title: prettyToolName(toolName), detail: compactArgSummary(args) }
    }
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
