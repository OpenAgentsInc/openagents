import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { aiElementBase } from './base'
import {
  parseMarkdownBlocks,
  parseMarkdownInline,
  type MarkdownBlock as Block,
  type MarkdownInlinePart,
} from './markdown'

export {
  isSafeMarkdownHref,
  parseMarkdownBlocks,
  parseMarkdownInline,
  type MarkdownBlock,
  type MarkdownInlinePart,
} from './markdown'

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
export const responseListClass = 'm-0 pl-4 [&>li+li]:mt-1'
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

const renderInlineParts = <Message>(
  parts: ReadonlyArray<MarkdownInlinePart>,
): ReadonlyArray<Html | string> => {
  const h = html<Message>()
  const nodes: Array<Html | string> = []
  for (const part of parts) {
    switch (part.kind) {
      case 'text':
        nodes.push(part.text)
        break
      case 'code':
        nodes.push(
          h.code([h.Class(responseInlineCodeClass)], [part.text]),
        )
        break
      case 'link':
        nodes.push(
          h.a(
            [
              h.Href(part.href),
              h.Target('_blank'),
              h.Rel('noopener noreferrer'),
              h.Class(responseLinkClass),
            ],
            renderInlineParts<Message>(part.children),
          ),
        )
        break
      case 'strong':
        nodes.push(
          h.strong(
            [h.Class(responseStrongClass)],
            renderInlineParts<Message>(part.children),
          ),
        )
        break
      case 'emphasis':
        nodes.push(
          h.em(
            [h.Class(responseEmphasisClass)],
            renderInlineParts<Message>(part.children),
          ),
        )
        break
    }
  }
  return nodes
}

const renderInline = <Message>(text: string): ReadonlyArray<Html | string> =>
  renderInlineParts<Message>(parseMarkdownInline(text))

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
