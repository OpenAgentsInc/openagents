import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { aiElementBase } from './base'

const MODULE_ID = 'response'

// A streaming-tolerant Markdown renderer, the centralized AI Element every chat
// surface uses to render assistant prose. There is no raw-HTML escape hatch in
// Foldkit's virtual DOM (text nodes auto-escape), so this parses Markdown into a
// tree of typed Foldkit `Html` nodes — never an HTML string. That is the safe,
// auditable path (AGENTS.md bans innerHTML); a malformed model reply can only
// ever produce text nodes, never injected markup.
//
// STREAMING DISCIPLINE (the Vercel "streamdown" pattern): the input may be an
// INCOMPLETE markdown fragment mid-stream — a half-open `**`, a dangling `` ` ``,
// an unterminated ```fence```, a list item still being typed. The parser must
// never throw and must never flash broken syntax: dangling inline markers render
// as plain text up to the cursor, and an unterminated fenced block renders the
// lines captured so far as a code block. Each delta re-parses the whole
// accumulated text (cheap for chat-length replies), so the tree is always a
// correct render of "the markdown so far".
//
// Scope is deliberately the common chat subset — bold, italic, inline code,
// links, headings, unordered/ordered lists, blockquotes, fenced + indented code,
// horizontal rules, paragraphs. Not a full CommonMark engine (no tables, nested
// blockquotes, reference links, HTML passthrough); those degrade to readable
// plain text rather than rendering wrong.

// CLASSES — dark-only, mono-first, matching DESIGN.md tokens. Headings are mono
// off-white; links are Khala-blue; code is framed in the panel surface; list
// markers are subtle. No light theme, no decorative gradients.
export const responseClass =
  'grid gap-2 text-[0.8125rem] leading-[1.5] text-[#f1efe8] [overflow-wrap:anywhere]'
export const responseHeadingClass = 'm-0 font-medium text-[#f1efe8] leading-[1.3]'
export const responseH1Class = clsx(responseHeadingClass, 'text-[1.05rem]')
export const responseH2Class = clsx(responseHeadingClass, 'text-[0.95rem]')
export const responseH3Class = clsx(
  responseHeadingClass,
  'text-[0.875rem] uppercase tracking-[0.04em] text-white/80',
)
export const responseParagraphClass = 'm-0 text-[0.8125rem] leading-[1.5]'
export const responseListClass = 'm-0 grid gap-1 pl-4'
export const responseOrderedListClass = clsx(responseListClass, 'list-decimal')
export const responseUnorderedListClass = clsx(responseListClass, 'list-disc')
export const responseListItemClass =
  'text-[0.8125rem] leading-[1.45] marker:text-white/35'
export const responseInlineCodeClass =
  'border border-[#222] bg-[#0a0a0a] px-1 py-px font-mono text-[0.75rem] text-[#f1efe8]'
export const responseCodeBlockClass =
  'm-0 overflow-x-auto border border-[#222] bg-[#010102] px-3 py-2.5 font-mono text-[0.75rem] leading-[1.45] text-[#f1efe8]'
export const responseLinkClass =
  'text-[#7aa2ff] underline decoration-[#3a7bff]/40 underline-offset-2 hover:text-[#a8c2ff]'
export const responseBlockquoteClass =
  'm-0 border-l border-[#3a7bff]/30 pl-3 text-white/70 italic'
export const responseRuleClass = 'm-0 h-px w-full border-0 bg-[#222]'
export const responseStrongClass = 'font-semibold text-white/90'
export const responseEmphasisClass = 'italic'

// INLINE PARSING ----------------------------------------------------------

// A bounded inline tokenizer over a single line of text. Handles, in priority
// order, inline code (`` `code` ``), links (`[text](url)`), bold (`**` / `__`),
// and italic (`*` / `_`). A dangling/unterminated marker renders as the literal
// text from the marker to the end — the streaming-tolerant contract.
//
// Returns a list of Foldkit inline nodes (text + element nodes). Links are
// rendered with an explicit safe-href guard so a model-supplied `javascript:`
// URL degrades to plain text.
const isSafeHref = (href: string): boolean => {
  const trimmed = href.trim()
  if (trimmed === '') {
    return false
  }
  // Allow relative, anchor, and the http(s)/mailto schemes only. Everything else
  // (javascript:, data:, vbscript:, …) renders as plain text, never a link.
  return (
    /^https?:\/\//i.test(trimmed) ||
    /^mailto:/i.test(trimmed) ||
    trimmed.startsWith('/') ||
    trimmed.startsWith('#')
  )
}

const renderInline = <Message>(text: string): ReadonlyArray<Html | string> => {
  const h = html<Message>()
  const nodes: Array<Html | string> = []
  let buffer = ''
  let index = 0

  const flush = (): void => {
    if (buffer !== '') {
      nodes.push(buffer)
      buffer = ''
    }
  }

  while (index < text.length) {
    const rest = text.slice(index)

    // Inline code: `…`. Highest priority — its content is never further parsed.
    if (rest.startsWith('`')) {
      const end = rest.indexOf('`', 1)
      if (end > 0) {
        flush()
        nodes.push(
          h.code([h.Class(responseInlineCodeClass)], [rest.slice(1, end)]),
        )
        index += end + 1
        continue
      }
      // Dangling backtick mid-stream: emit the literal char, keep scanning.
      buffer += '`'
      index += 1
      continue
    }

    // Link: [text](href).
    if (rest.startsWith('[')) {
      const labelEnd = rest.indexOf(']')
      if (labelEnd > 0 && rest[labelEnd + 1] === '(') {
        const hrefEnd = rest.indexOf(')', labelEnd + 2)
        if (hrefEnd > labelEnd + 1) {
          const label = rest.slice(1, labelEnd)
          const href = rest.slice(labelEnd + 2, hrefEnd)
          flush()
          if (isSafeHref(href)) {
            nodes.push(
              h.a(
                [
                  h.Href(href),
                  h.Target('_blank'),
                  h.Rel('noopener noreferrer'),
                  h.Class(responseLinkClass),
                ],
                renderInline<Message>(label),
              ),
            )
          } else {
            // Unsafe scheme: render the visible label as plain inline text.
            nodes.push(...renderInline<Message>(label))
          }
          index += hrefEnd + 1
          continue
        }
      }
      // Incomplete link mid-stream: literal `[`, keep scanning.
      buffer += '['
      index += 1
      continue
    }

    // Bold: ** … ** or __ … __.
    const boldMarker = rest.startsWith('**') ? '**' : rest.startsWith('__') ? '__' : ''
    if (boldMarker !== '') {
      const end = rest.indexOf(boldMarker, boldMarker.length)
      if (end > 0) {
        flush()
        nodes.push(
          h.strong(
            [h.Class(responseStrongClass)],
            renderInline<Message>(rest.slice(boldMarker.length, end)),
          ),
        )
        index += end + boldMarker.length
        continue
      }
      // Dangling bold marker mid-stream: literal text, keep scanning.
      buffer += boldMarker
      index += boldMarker.length
      continue
    }

    // Italic: * … * or _ … _ (single marker, not part of a bold pair).
    const italicMarker =
      rest.startsWith('*') ? '*' : rest.startsWith('_') ? '_' : ''
    if (italicMarker !== '') {
      const end = rest.indexOf(italicMarker, 1)
      if (end > 0) {
        flush()
        nodes.push(
          h.em(
            [h.Class(responseEmphasisClass)],
            renderInline<Message>(rest.slice(1, end)),
          ),
        )
        index += end + 1
        continue
      }
      buffer += italicMarker
      index += 1
      continue
    }

    buffer += text[index]
    index += 1
  }

  flush()
  return nodes
}

// BLOCK PARSING -----------------------------------------------------------

type Block =
  | Readonly<{ kind: 'heading'; level: 1 | 2 | 3; text: string }>
  | Readonly<{ kind: 'paragraph'; text: string }>
  | Readonly<{ kind: 'unordered-list'; items: ReadonlyArray<string> }>
  | Readonly<{ kind: 'ordered-list'; items: ReadonlyArray<string> }>
  | Readonly<{ kind: 'blockquote'; text: string }>
  | Readonly<{ kind: 'code'; language: string | undefined; code: string }>
  | Readonly<{ kind: 'rule' }>

const headingMatch = (line: string): { level: 1 | 2 | 3; text: string } | undefined => {
  const match = /^(#{1,6})\s+(.*)$/.exec(line)
  if (match === null) {
    return undefined
  }
  const hashes = match[1] ?? ''
  const level = (hashes.length >= 3 ? 3 : hashes.length) as 1 | 2 | 3
  return { level, text: (match[2] ?? '').trim() }
}

const unorderedItemMatch = (line: string): string | undefined => {
  const match = /^\s*[-*+]\s+(.*)$/.exec(line)
  return match === null ? undefined : (match[1] ?? '')
}

const orderedItemMatch = (line: string): string | undefined => {
  const match = /^\s*\d+[.)]\s+(.*)$/.exec(line)
  return match === null ? undefined : (match[1] ?? '')
}

const isRule = (line: string): boolean => /^\s*([-*_])(\s*\1){2,}\s*$/.test(line)

const isBlank = (line: string): boolean => line.trim() === ''

// Parse the accumulated markdown text into a list of blocks. Streaming-tolerant:
// an unterminated ``` fence captures every line seen so far as a code block; a
// list still being typed is a valid list; trailing blank lines are dropped.
export const parseMarkdownBlocks = (text: string): ReadonlyArray<Block> => {
  const lines = text.replace(/\r\n/g, '\n').split('\n')
  const blocks: Array<Block> = []
  let lineIndex = 0

  while (lineIndex < lines.length) {
    const line = lines[lineIndex] ?? ''

    if (isBlank(line)) {
      lineIndex += 1
      continue
    }

    // Fenced code block: ```lang … ``` . An unterminated fence (mid-stream)
    // captures the rest of the input as code rather than flashing the raw fence.
    const fenceMatch = /^\s*```(.*)$/.exec(line)
    if (fenceMatch !== null) {
      const language = (fenceMatch[1] ?? '').trim()
      const codeLines: Array<string> = []
      lineIndex += 1
      let closed = false
      while (lineIndex < lines.length) {
        const codeLine = lines[lineIndex] ?? ''
        if (/^\s*```\s*$/.test(codeLine)) {
          closed = true
          lineIndex += 1
          break
        }
        codeLines.push(codeLine)
        lineIndex += 1
      }
      // `closed` is informational; whether or not the fence closed, the captured
      // lines render as a code block (the streaming-tolerant contract).
      void closed
      blocks.push({
        kind: 'code',
        language: language === '' ? undefined : language,
        code: codeLines.join('\n'),
      })
      continue
    }

    // Indented code block (4+ leading spaces), collected greedily.
    if (/^ {4}\S/.test(line)) {
      const codeLines: Array<string> = []
      while (lineIndex < lines.length && /^ {4}/.test(lines[lineIndex] ?? '')) {
        codeLines.push((lines[lineIndex] ?? '').slice(4))
        lineIndex += 1
      }
      blocks.push({ kind: 'code', language: undefined, code: codeLines.join('\n') })
      continue
    }

    if (isRule(line)) {
      blocks.push({ kind: 'rule' })
      lineIndex += 1
      continue
    }

    const heading = headingMatch(line)
    if (heading !== undefined) {
      blocks.push({ kind: 'heading', level: heading.level, text: heading.text })
      lineIndex += 1
      continue
    }

    // Unordered list: consecutive `- ` / `* ` / `+ ` lines.
    if (unorderedItemMatch(line) !== undefined) {
      const items: Array<string> = []
      while (lineIndex < lines.length) {
        const item = unorderedItemMatch(lines[lineIndex] ?? '')
        if (item === undefined) {
          break
        }
        items.push(item)
        lineIndex += 1
      }
      blocks.push({ kind: 'unordered-list', items })
      continue
    }

    // Ordered list: consecutive `1. ` / `2) ` lines.
    if (orderedItemMatch(line) !== undefined) {
      const items: Array<string> = []
      while (lineIndex < lines.length) {
        const item = orderedItemMatch(lines[lineIndex] ?? '')
        if (item === undefined) {
          break
        }
        items.push(item)
        lineIndex += 1
      }
      blocks.push({ kind: 'ordered-list', items })
      continue
    }

    // Blockquote: consecutive `> ` lines, joined into one quote.
    if (/^\s*>\s?/.test(line)) {
      const quoteLines: Array<string> = []
      while (lineIndex < lines.length && /^\s*>\s?/.test(lines[lineIndex] ?? '')) {
        quoteLines.push((lines[lineIndex] ?? '').replace(/^\s*>\s?/, ''))
        lineIndex += 1
      }
      blocks.push({ kind: 'blockquote', text: quoteLines.join('\n') })
      continue
    }

    // Paragraph: consecutive non-blank lines that don't start a new block, soft
    // wrapped into one paragraph.
    const paragraphLines: Array<string> = []
    while (lineIndex < lines.length) {
      const candidate = lines[lineIndex] ?? ''
      if (
        isBlank(candidate) ||
        /^\s*```/.test(candidate) ||
        headingMatch(candidate) !== undefined ||
        unorderedItemMatch(candidate) !== undefined ||
        orderedItemMatch(candidate) !== undefined ||
        /^\s*>\s?/.test(candidate) ||
        isRule(candidate)
      ) {
        break
      }
      paragraphLines.push(candidate.trim())
      lineIndex += 1
    }
    if (paragraphLines.length > 0) {
      blocks.push({ kind: 'paragraph', text: paragraphLines.join(' ') })
    }
  }

  return blocks
}

const headingClassFor = (level: 1 | 2 | 3): string =>
  level === 1 ? responseH1Class : level === 2 ? responseH2Class : responseH3Class

const headingTagFor = (level: 1 | 2 | 3): 'h3' | 'h4' | 'h5' =>
  // Keep heading levels demoted relative to the page h1 so the assistant prose
  // never out-ranks the HUD heading in the document outline.
  level === 1 ? 'h3' : level === 2 ? 'h4' : 'h5'

const renderBlock = <Message>(block: Block): Html => {
  const h = html<Message>()

  switch (block.kind) {
    case 'heading':
      return h[headingTagFor(block.level)](
        [h.Class(headingClassFor(block.level))],
        renderInline<Message>(block.text),
      )
    case 'paragraph':
      return h.p(
        [h.Class(responseParagraphClass)],
        renderInline<Message>(block.text),
      )
    case 'unordered-list':
      return h.ul(
        [h.Class(responseUnorderedListClass)],
        block.items.map(item =>
          h.li([h.Class(responseListItemClass)], renderInline<Message>(item)),
        ),
      )
    case 'ordered-list':
      return h.ol(
        [h.Class(responseOrderedListClass)],
        block.items.map(item =>
          h.li([h.Class(responseListItemClass)], renderInline<Message>(item)),
        ),
      )
    case 'blockquote':
      return h.blockquote(
        [h.Class(responseBlockquoteClass)],
        renderInline<Message>(block.text),
      )
    case 'code':
      return h.pre(
        [
          aiElementBase<Message>(MODULE_ID, 'ResponseCode'),
          h.Class(responseCodeBlockClass),
          ...(block.language === undefined
            ? []
            : [h.DataAttribute('language', block.language)]),
        ],
        [h.code([], [block.code])],
      )
    case 'rule':
      return h.hr([h.Class(responseRuleClass)])
  }
}

// Render a complete markdown body (or the markdown-so-far during streaming) into
// a single dark-only, mono response surface. `streaming` appends a blinking
// cursor affordance to the last block so the live reply visibly "types".
export const response = <Message>(input: {
  markdown: string
  streaming?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const blocks = parseMarkdownBlocks(input.markdown)
  const rendered = blocks.map(block => renderBlock<Message>(block))
  const children =
    input.streaming === true
      ? [...rendered, streamingCursor<Message>()]
      : rendered

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Response'),
      h.Class(responseClass),
    ],
    children.length === 0
      ? // An empty streaming reply (no content yet) shows the cursor alone so the
        // turn reads as "starting", never blank.
        input.streaming === true
        ? [streamingCursor<Message>()]
        : []
      : children,
  )
}

// The blinking type cursor. Honors reduced-motion (the CSS holds it static).
export const responseStreamingCursorClass =
  'oa-stream-cursor inline-block h-[1em] w-[0.5ch] translate-y-[0.1em] bg-[#4fd0ff] align-baseline'

export const streamingCursor = <Message>(): Html => {
  const h = html<Message>()

  return h.span(
    [
      aiElementBase<Message>(MODULE_ID, 'ResponseCursor'),
      h.AriaHidden(true),
      h.Class(responseStreamingCursorClass),
    ],
    [],
  )
}
