// Session-stream concision + markdown rendering for the Autopilot desktop
// transcript (owner report, 2026-06-19: the Codex session stream was dumping
// raw, super-verbose text — unrendered `## headings`, full hashes,
// "tokens used: 51342", line-by-line wrapped raw markdown — instead of a clean
// readable transcript).
//
// This module is PURE (no DOM, no innerHTML, no side effects) so the surfacing
// rules and the markdown→Foldkit mapping stay unit-testable without a runtime.
// All rendering goes through `foldkit/html` `h.*` typed builders (the desktop's
// "Foldkit over everything — NO hand-DOM" mandate), so nothing is injected as
// raw HTML; markdown is parsed into a small token tree and emitted as safe `h.*`
// nodes.
//
// Surfacing design ported (ideas only, not code) from `projects/repos/opencode`:
//   - Assistant text is the meaningful content → render as markdown.
//   - Tool actions/results are shown as a concise one-line summary, never a raw
//     dump; large output is truncated with an ellipsis marker.
//   - Token/usage noise is kept OUT of the transcript (it belongs in a footer,
//     not the scrollback).
//   - Pure lifecycle frames (thread/turn started/completed) and provider/mode
//     metadata are suppressed from the readable transcript.
//   - Consecutive identical lines are de-duplicated.

import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { Message } from "./message.js"
import type { SessionEventRow } from "../shared/rpc.js"

// `html` is a factory parameterized by the Message set (same pattern as view.ts:
// `const h = html<Message>()`). These views emit no events, so any Message set
// works; we use the desktop's `Message` for type compatibility with the caller.
const h = html<Message>()
const cls = (value: string): Attribute<Message> => h.Class(value)

// ── Classification ───────────────────────────────────────────────────────────
//
// The Pylon node projects each composer event as a `messageText` summary the
// codex-composer produced, e.g. `agent: <text>`, `thinking: <text>`,
// `running: <cmd>`, `completed: <cmd> exit 0`, `tokens used: 51342`,
// `external session: …`, `control session mode: …`. We classify by that shape
// so the transcript can surface the meaningful kinds and suppress the noise.

export type StreamLineKind =
  | "assistant" // the agent's answer text (rendered as markdown)
  | "reasoning" // the agent's thinking summary (rendered as markdown, dimmed)
  | "tool" // a tool action / result one-liner (command, file change, search…)
  | "error" // a turn/tool error
  | "noise" // token counters, lifecycle frames, provider/mode metadata, refs

export type ClassifiedStreamLine = {
  readonly kind: StreamLineKind
  // The presentational text with any leading `agent:`/`thinking:` label and the
  // duplicated giant-hash refs stripped. Markdown-bearing for assistant/reasoning.
  readonly text: string
}

// Drop the codex-composer summary label prefix ("agent: ", "thinking: ").
const stripLabel = (text: string, label: string): string =>
  text.startsWith(label) ? text.slice(label.length) : text

// A 24+ hex char run is one of our `*.<sha>` refs (session/digest/workspace).
// Those are machine refs, never useful in a readable transcript.
const LONG_HEX = /\b[a-f0-9]{24,}\b/i

const NOISE_PREFIXES = [
  "tokens used",
  "thinking tokens:",
  "output tokens:",
  "token count",
  "thread started",
  "thread ended",
  "turn started",
  "turn completed",
  "task started",
  "task complete",
  "external session:",
  "control session mode:",
  "agent message", // empty-text placeholder summary
] as const

export function classifyStreamLine(detail: string): ClassifiedStreamLine {
  const raw = detail.trim()
  if (raw.length === 0) return { kind: "noise", text: "" }
  const lower = raw.toLowerCase()

  for (const prefix of NOISE_PREFIXES) {
    if (lower === prefix || lower.startsWith(`${prefix} `) || lower.startsWith(`${prefix}:`)) {
      return { kind: "noise", text: "" }
    }
  }
  // Bare ref lines (e.g. `digest.pylon.…<sha>`) carry no reader value.
  if (LONG_HEX.test(raw) && !raw.includes(" ")) return { kind: "noise", text: "" }

  if (lower.startsWith("error:") || lower.startsWith("turn failed:")) {
    return { kind: "error", text: raw }
  }
  if (raw.startsWith("agent:")) {
    return { kind: "assistant", text: stripLabel(raw, "agent:").trim() }
  }
  if (raw.startsWith("thinking:")) {
    return { kind: "reasoning", text: stripLabel(raw, "thinking:").trim() }
  }
  // Tool-shaped one-liners the composer emits:
  //   running/completed/failed: <cmd> exit N | <status>: <file changes> |
  //   web search: … | <status>: <server>.<tool> | todo list 2/3
  if (
    /^(running|completed|failed|started|queued|cancelled)[: ]/i.test(raw) ||
    /^web search:/i.test(raw) ||
    /^todo list/i.test(raw) ||
    /^create |^update |^delete |^add |^modify /i.test(raw)
  ) {
    return { kind: "tool", text: raw }
  }
  // Otherwise treat as assistant prose (the most common unlabeled content).
  return { kind: "assistant", text: raw }
}

// ── Event → concise transcript projection ────────────────────────────────────
//
// Reduce the raw event tail to the readable lines worth showing, de-duplicating
// consecutive identical content and dropping noise. Token/usage stays out of the
// transcript (the pane header/footer can surface it separately).

export type ConciseStreamLine = ClassifiedStreamLine & { readonly eventIndex: number }

export function conciseStreamLines(
  events: ReadonlyArray<SessionEventRow>,
): ReadonlyArray<ConciseStreamLine> {
  const out: ConciseStreamLine[] = []
  let lastKey = ""
  for (const event of events) {
    const source = (event.full && event.full.trim().length > 0 ? event.full : event.detail) ?? ""
    const classified = classifyStreamLine(source)
    if (classified.kind === "noise" || classified.text.length === 0) continue
    const key = `${classified.kind}:${classified.text}`
    if (key === lastKey) continue // collapse repeated identical lines
    lastKey = key
    out.push({ ...classified, eventIndex: event.eventIndex })
  }
  return out
}

// ── Minimal markdown → Foldkit renderer ──────────────────────────────────────
//
// Deliberately small + dependency-free: covers the constructs the owner saw
// rendered raw — headings, bold/italic, inline code, fenced code blocks, and
// bullet/numbered lists — and degrades any unknown syntax to plain text. It is
// streaming-friendly in the sense that it renders whatever text exists so far on
// every frame (an unterminated fence is shown as a code block in progress).
//
// NOTE: text is always passed as string children to `h.*`, which Foldkit escapes
// — there is no innerHTML path, so this is XSS-safe by construction.

const TRUNCATE_TOOL_CHARS = 240

type InlineToken =
  | { t: "text"; v: string }
  | { t: "code"; v: string }
  | { t: "strong"; v: string }
  | { t: "em"; v: string }

// Tokenize a single line of inline markdown. Order matters: code spans win over
// emphasis so backticked `**x**` stays literal.
function tokenizeInline(line: string): InlineToken[] {
  const tokens: InlineToken[] = []
  let rest = line
  const push = (token: InlineToken) => {
    if (token.t === "text" && token.v.length === 0) return
    tokens.push(token)
  }
  // Eagerly match the earliest of: `code`, **strong**, *em*/_em_.
  const patterns: Array<{ re: RegExp; make: (m: RegExpExecArray) => InlineToken }> = [
    { re: /`([^`]+)`/, make: (m) => ({ t: "code", v: m[1]! }) },
    { re: /\*\*([^*]+)\*\*/, make: (m) => ({ t: "strong", v: m[1]! }) },
    { re: /\*([^*]+)\*/, make: (m) => ({ t: "em", v: m[1]! }) },
    { re: /_([^_]+)_/, make: (m) => ({ t: "em", v: m[1]! }) },
  ]
  // Bounded loop: each iteration consumes at least one char of `rest`.
  for (let guard = 0; rest.length > 0 && guard < 5000; guard++) {
    let best: { index: number; len: number; token: InlineToken } | null = null
    for (const { re, make } of patterns) {
      const m = re.exec(rest)
      if (m && (best === null || m.index < best.index)) {
        best = { index: m.index, len: m[0].length, token: make(m) }
      }
    }
    if (best === null) {
      push({ t: "text", v: rest })
      break
    }
    if (best.index > 0) push({ t: "text", v: rest.slice(0, best.index) })
    push(best.token)
    rest = rest.slice(best.index + best.len)
  }
  return tokens
}

function renderInline(line: string): Html[] {
  return tokenizeInline(line).map((token) => {
    switch (token.t) {
      case "code":
        return h.code([cls("md-code")], [token.v])
      case "strong":
        return h.strong([], [token.v])
      case "em":
        return h.em([], [token.v])
      default:
        return h.span([], [token.v])
    }
  })
}

// Block-level: split into paragraphs, headings, fenced code, and lists.
export function renderMarkdown(source: string): Html {
  const lines = source.replace(/\r\n/g, "\n").split("\n")
  const blocks: Html[] = []
  let i = 0
  while (i < lines.length) {
    const line = lines[i]!
    const trimmed = line.trim()

    // Fenced code block.
    if (trimmed.startsWith("```")) {
      const code: string[] = []
      i++
      while (i < lines.length && !lines[i]!.trim().startsWith("```")) {
        code.push(lines[i]!)
        i++
      }
      if (i < lines.length) i++ // consume closing fence
      blocks.push(h.pre([cls("md-pre")], [h.code([], [code.join("\n")])]))
      continue
    }

    // Blank line.
    if (trimmed.length === 0) {
      i++
      continue
    }

    // Heading (#, ##, ### …) → a single styled line, never raw `##`.
    const heading = /^(#{1,6})\s+(.*)$/.exec(trimmed)
    if (heading) {
      const level = Math.min(heading[1]!.length, 3)
      const content = heading[2]!
      const tag = level === 1 ? h.h1 : level === 2 ? h.h2 : h.h3
      blocks.push(tag([cls(`md-h md-h${level}`)], renderInline(content)))
      i++
      continue
    }

    // Lists (unordered `-`/`*`/`+`, ordered `1.`).
    if (/^([-*+]\s+|\d+[.)]\s+)/.test(trimmed)) {
      const ordered = /^\d+[.)]\s+/.test(trimmed)
      const items: Html[] = []
      while (i < lines.length) {
        const li = lines[i]!.trim()
        const m = /^(?:[-*+]\s+|\d+[.)]\s+)(.*)$/.exec(li)
        if (!m) break
        items.push(h.li([], renderInline(m[1]!)))
        i++
      }
      blocks.push(h.ul([cls(ordered ? "md-ol" : "md-ul")], items))
      continue
    }

    // Paragraph: gather contiguous non-blank, non-block lines.
    const para: string[] = [trimmed]
    i++
    while (i < lines.length) {
      const next = lines[i]!.trim()
      if (
        next.length === 0 ||
        next.startsWith("```") ||
        /^#{1,6}\s+/.test(next) ||
        /^([-*+]\s+|\d+[.)]\s+)/.test(next)
      ) {
        break
      }
      para.push(next)
      i++
    }
    blocks.push(h.p([cls("md-p")], renderInline(para.join(" "))))
  }

  if (blocks.length === 0) return h.empty
  return h.div([cls("md")], blocks)
}

// A concise tool line: collapse whitespace and truncate long output.
export function renderToolLine(text: string): Html {
  const oneLine = text.replace(/\s+/g, " ").trim()
  const clipped =
    oneLine.length > TRUNCATE_TOOL_CHARS ? `${oneLine.slice(0, TRUNCATE_TOOL_CHARS)}…` : oneLine
  return h.div([cls("stream-tool")], [h.code([], [clipped])])
}

// ── The concise transcript view ──────────────────────────────────────────────
//
// Renders the classified, de-noised lines: assistant/reasoning as markdown,
// tool actions as concise one-liners, errors highlighted. Used by the composer
// transcript + session-detail timeline in place of the old raw label dump.

// Returns null when there is no readable content yet, so the caller can fall
// back to the raw lifecycle timeline (rather than relying on h.empty identity).
export function conciseTranscript(events: ReadonlyArray<SessionEventRow>): Html | null {
  const lines = conciseStreamLines(events)
  if (lines.length === 0) return null
  return h.div(
    [cls("stream-transcript")],
    lines.map((line) => {
      switch (line.kind) {
        case "assistant":
          return h.div([cls("stream-assistant")], [renderMarkdown(line.text)])
        case "reasoning":
          return h.div([cls("stream-reasoning")], [renderMarkdown(line.text)])
        case "error":
          return h.div([cls("stream-error")], [h.code([], [line.text])])
        case "tool":
          return renderToolLine(line.text)
        default:
          return h.empty
      }
    }),
  )
}
