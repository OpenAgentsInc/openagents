/**
 * Chat markdown -> typed Effect Native content (#8712, EP250 owner directive:
 * "The markdown isn't rendered as markdown, so fix our fucking markdown
 * rendering.").
 *
 * The vendored Effect Native catalog ships the typed `Markdown` view (block +
 * inline model, issue #35) and `CodeBlock` (issue #36) but intentionally NO
 * parser: apps parse to the typed model and no arbitrary HTML ever enters the
 * view tree. This module ports the bounded parser behavior already proven by
 * the forum surface (`apps/openagents.com/apps/start/src/routes/
 * -forum-markdown.ts`), adapted for assistant chat bodies:
 *
 * - full ATX heading levels 1–6 (no forum h4–h6 downshift),
 * - bold/italics, inline code, fenced code, ordered/unordered lists,
 *   blockquotes, horizontal rules, paragraphs,
 * - links render as SAFE TEXT (`label (href)`) — never a clickable anchor in
 *   the transcript, and never raw HTML (everything is text nodes; the DOM
 *   renderer writes textContent only).
 *
 * Streaming-safe by construction: an unterminated `**`/`_` renders literally
 * as plain text until the closing marker arrives, and an unterminated fence
 * renders as a growing code block — re-parsing per append never throws.
 */
import {
  CodeBlock,
  Divider,
  Markdown,
  type MarkdownBlock,
  type MarkdownInline,
  type View,
} from "@effect-native/core"

export type ChatMarkdownSegment =
  | Readonly<{ kind: "markdown"; blocks: ReadonlyArray<MarkdownBlock> }>
  | Readonly<{ kind: "code"; language: string | undefined; code: string }>
  | Readonly<{ kind: "rule" }>

// --- Inline parsing ----------------------------------------------------------

const text = (value: string): MarkdownInline => ({ kind: "text", text: value })

const parseEmphasis = (value: string): ReadonlyArray<MarkdownInline> => {
  const parts: MarkdownInline[] = []
  const pattern = /(\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\s][^*]*)\*|_([^_\s][^_]*)_)/g
  let cursor = 0
  for (const match of value.matchAll(pattern)) {
    const index = match.index ?? 0
    if (index > cursor) parts.push(text(value.slice(cursor, index)))
    const strongBody = match[2] ?? match[3]
    const emphasisBody = match[4] ?? match[5]
    if (strongBody !== undefined) {
      parts.push({ kind: "strong", children: [text(strongBody)] })
    } else if (emphasisBody !== undefined) {
      parts.push({ kind: "emphasis", children: [text(emphasisBody)] })
    }
    cursor = index + match[0].length
  }
  if (cursor < value.length) parts.push(text(value.slice(cursor)))
  return parts
}

const parseWithoutLinks = (value: string): ReadonlyArray<MarkdownInline> => {
  const segments = value.split("`")
  // An unterminated backtick renders literally (streaming-safe).
  if (segments.length % 2 === 0) return parseEmphasis(value)
  const parts: MarkdownInline[] = []
  segments.forEach((segment, index) => {
    if (index % 2 === 1) {
      parts.push({ kind: "code", text: segment })
    } else if (segment !== "") {
      parts.push(...parseEmphasis(segment))
    }
  })
  return parts
}

/**
 * Links-as-safe-text (owner statement 4): `[label](href)` becomes the label
 * plus the href in parentheses, all plain text — the transcript never carries
 * a clickable navigation affordance or an attacker-controlled scheme.
 */
export const parseChatInlineMarkdown = (value: string): ReadonlyArray<MarkdownInline> => {
  const source = String(value ?? "")
  const linkPattern = /\[([^\]\n]+)\]\(([^)\s]+)\)/g
  const parts: MarkdownInline[] = []
  let cursor = 0
  for (const match of source.matchAll(linkPattern)) {
    const index = match.index ?? 0
    parts.push(...parseWithoutLinks(source.slice(cursor, index)))
    const label = match[1] !== undefined && match[1] !== "" ? match[1] : (match[2] ?? "Link")
    const href = (match[2] ?? "").trim()
    parts.push(...parseWithoutLinks(label))
    if (href !== "") parts.push(text(` (${href})`))
    cursor = index + match[0].length
  }
  parts.push(...parseWithoutLinks(source.slice(cursor)))
  return parts
}

// --- Block parsing -------------------------------------------------------------

const isFenceLine = (line: string): boolean => {
  const trimmed = line.trim()
  return trimmed.startsWith("```") || trimmed.startsWith("~~~")
}

const isBoundary = (line: string): boolean => {
  const trimmed = line.trim()
  return (
    trimmed === "" ||
    isFenceLine(line) ||
    /^#{1,6}\s+/.test(trimmed) ||
    /^[-*_]{3,}$/.test(trimmed) ||
    /^\s*>/.test(line) ||
    /^\s*(?:[-*+]\s+|\d+[.)]\s+)/.test(line)
  )
}

const paragraph = (value: string): MarkdownBlock => ({
  kind: "paragraph",
  children: [...parseChatInlineMarkdown(value)],
})

const headingLevel = (depth: number): 1 | 2 | 3 | 4 | 5 | 6 =>
  Math.min(Math.max(depth, 1), 6) as 1 | 2 | 3 | 4 | 5 | 6

export const parseChatMarkdown = (value: string): ReadonlyArray<ChatMarkdownSegment> => {
  const lines = String(value ?? "").replace(/\r\n?/g, "\n").split("\n")
  const segments: ChatMarkdownSegment[] = []
  let pendingBlocks: MarkdownBlock[] = []

  const flushBlocks = (): void => {
    if (pendingBlocks.length > 0) {
      segments.push({ kind: "markdown", blocks: pendingBlocks })
      pendingBlocks = []
    }
  }

  let index = 0
  while (index < lines.length) {
    const line = lines[index] ?? ""
    const trimmed = line.trim()

    if (trimmed === "") {
      index += 1
      continue
    }

    if (isFenceLine(line)) {
      const fence = trimmed.slice(0, 3)
      const language = trimmed.slice(3).trim()
      const codeLines: string[] = []
      index += 1
      // An unterminated fence consumes to the end — a growing code block
      // while the stream is still appending, never a crash.
      while (index < lines.length && !(lines[index] ?? "").trim().startsWith(fence)) {
        codeLines.push(lines[index] ?? "")
        index += 1
      }
      if (index < lines.length) index += 1
      flushBlocks()
      segments.push({
        kind: "code",
        language: language === "" ? undefined : language.slice(0, 40),
        code: codeLines.join("\n"),
      })
      continue
    }

    const heading = /^(#{1,6})\s+(.+)$/.exec(trimmed)
    if (heading !== null) {
      pendingBlocks.push({
        kind: "heading",
        level: headingLevel((heading[1] ?? "").length),
        children: [...parseChatInlineMarkdown(heading[2] ?? "")],
      })
      index += 1
      continue
    }

    if (/^[-*_]{3,}$/.test(trimmed)) {
      flushBlocks()
      segments.push({ kind: "rule" })
      index += 1
      continue
    }

    if (/^\s*>/.test(line)) {
      const quoteLines: string[] = []
      while (index < lines.length && /^\s*>/.test(lines[index] ?? "")) {
        quoteLines.push((lines[index] ?? "").replace(/^\s*>\s?/, "").trim())
        index += 1
      }
      pendingBlocks.push({
        kind: "blockquote",
        children: [paragraph(quoteLines.join(" "))],
      })
      continue
    }

    const ordered = /^\s*\d+[.)]\s+/.test(line)
    const unordered = /^\s*[-*+]\s+/.test(line)
    if (ordered || unordered) {
      const pattern = ordered ? /^\s*\d+[.)]\s+(.+)$/ : /^\s*[-*+]\s+(.+)$/
      const items: MarkdownBlock[][] = []
      while (index < lines.length) {
        const current = lines[index] ?? ""
        const match = pattern.exec(current)
        if (match === null) {
          if (current.trim() === "") {
            let lookahead = index + 1
            while (lookahead < lines.length && (lines[lookahead] ?? "").trim() === "") {
              lookahead += 1
            }
            if (lookahead < lines.length && pattern.test(lines[lookahead] ?? "")) {
              index = lookahead
              continue
            }
          }
          break
        }
        items.push([paragraph(match[1] ?? "")])
        index += 1
      }
      pendingBlocks.push({ kind: "list", ordered, items })
      continue
    }

    const paragraphLines: string[] = []
    while (index < lines.length && !isBoundary(lines[index] ?? "")) {
      paragraphLines.push((lines[index] ?? "").trim())
      index += 1
    }
    pendingBlocks.push(paragraph(paragraphLines.join(" ")))
  }

  flushBlocks()
  return segments.length === 0
    ? [{ kind: "markdown", blocks: [paragraph("")] }]
    : segments
}

/**
 * Lowers parsed segments to catalog views: `Markdown` for block groups,
 * `CodeBlock` (plain tokens — the chat lane runs no highlighter) for fenced
 * code, `Divider` for rules. Pure; safe to re-run on every stream append.
 */
export const chatMarkdownBody = (keyPrefix: string, value: string): ReadonlyArray<View> =>
  parseChatMarkdown(value).map((segment, index) =>
    segment.kind === "markdown"
      ? Markdown({ key: `${keyPrefix}-md-${index}`, blocks: segment.blocks })
      : segment.kind === "code"
        ? CodeBlock({
            key: `${keyPrefix}-code-${index}`,
            ...(segment.language === undefined ? {} : { language: segment.language }),
            lines: segment.code.split("\n").map(line => ({ tokens: [{ kind: "plain" as const, text: line }] })),
          })
        : Divider({ key: `${keyPrefix}-rule-${index}` }))
