// APP-FORUM (#8635) — forum post markdown -> typed Effect Native content.
//
// The Effect Native catalog intentionally ships no markdown parser: apps parse
// to the typed `MarkdownBlock`/`MarkdownInline` model (see the catalog note on
// the Markdown component) and no arbitrary HTML ever enters the view tree.
// This module ports the legacy Foldkit forum page's bounded markdown feature
// set (apps/web/src/page/forum.ts renderMarkdown): paragraphs, ATX headings,
// fenced code, blockquotes, ordered/unordered lists, horizontal rules, inline
// code/strong/emphasis, and links with the same safe-href policy.
//
// Fenced code and rules have no `MarkdownBlock` representation, so the parser
// emits typed segments the page lowers to `CodeBlock` and `Divider` catalog
// views — still catalog-only composition, no local primitives.

import type { MarkdownBlock, MarkdownInline } from '@effect-native/core'

export type ForumMarkdownSegment =
  | Readonly<{ kind: 'markdown'; blocks: ReadonlyArray<MarkdownBlock> }>
  | Readonly<{ kind: 'code'; language: string | undefined; code: string }>
  | Readonly<{ kind: 'rule' }>

// --- Safe hrefs (legacy parity: same-origin paths or http(s) URLs only) ------

export const safeForumMarkdownHref = (href: string): string => {
  const trimmed = href.trim()
  if (trimmed.startsWith('/') && !trimmed.startsWith('//')) return trimmed
  try {
    const parsed = new URL(trimmed)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
      ? parsed.href
      : ''
  } catch {
    return ''
  }
}

// --- Inline parsing -----------------------------------------------------------

const text = (value: string): MarkdownInline => ({ kind: 'text', text: value })

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
      parts.push({ kind: 'strong', children: [text(strongBody)] })
    } else if (emphasisBody !== undefined) {
      parts.push({ kind: 'emphasis', children: [text(emphasisBody)] })
    }
    cursor = index + match[0].length
  }
  if (cursor < value.length) parts.push(text(value.slice(cursor)))
  return parts
}

const parseWithoutLinks = (value: string): ReadonlyArray<MarkdownInline> => {
  const segments = value.split('`')
  if (segments.length % 2 === 0) return parseEmphasis(value)
  const parts: MarkdownInline[] = []
  segments.forEach((segment, index) => {
    if (index % 2 === 1) {
      parts.push({ kind: 'code', text: segment })
    } else if (segment !== '') {
      parts.push(...parseEmphasis(segment))
    }
  })
  return parts
}

export const parseForumInlineMarkdown = (
  value: string,
): ReadonlyArray<MarkdownInline> => {
  const source = String(value ?? '')
  const linkPattern = /\[([^\]\n]+)\]\(([^)\s]+)\)/g
  const parts: MarkdownInline[] = []
  let cursor = 0
  for (const match of source.matchAll(linkPattern)) {
    const index = match.index ?? 0
    parts.push(...parseWithoutLinks(source.slice(cursor, index)))
    const label = match[1] !== undefined && match[1] !== '' ? match[1] : (match[2] ?? 'Link')
    const href = safeForumMarkdownHref(match[2] ?? '')
    if (href === '') {
      parts.push(...parseWithoutLinks(label))
    } else {
      parts.push({
        kind: 'link',
        href,
        children: [...parseWithoutLinks(label)],
      })
    }
    cursor = index + match[0].length
  }
  parts.push(...parseWithoutLinks(source.slice(cursor)))
  return parts
}

// --- Block parsing -------------------------------------------------------------

const isFenceLine = (line: string): boolean => {
  const trimmed = line.trim()
  return trimmed.startsWith('```') || trimmed.startsWith('~~~')
}

const isBoundary = (line: string): boolean => {
  const trimmed = line.trim()
  return (
    trimmed === '' ||
    isFenceLine(line) ||
    /^#{1,6}\s+/.test(trimmed) ||
    /^[-*_]{3,}$/.test(trimmed) ||
    /^\s*>/.test(line) ||
    /^\s*(?:[-*+]\s+|\d+[.)]\s+)/.test(line)
  )
}

const paragraph = (value: string): MarkdownBlock => ({
  kind: 'paragraph',
  children: [...parseForumInlineMarkdown(value)],
})

const headingLevel = (depth: number): 1 | 2 | 3 | 4 | 5 | 6 =>
  // Legacy parity: forum headings render small (h4-h6) inside post bodies.
  Math.min(depth + 3, 6) as 4 | 5 | 6

export const parseForumMarkdown = (
  value: string,
): ReadonlyArray<ForumMarkdownSegment> => {
  const lines = String(value ?? '').replace(/\r\n?/g, '\n').split('\n')
  const segments: ForumMarkdownSegment[] = []
  let pendingBlocks: MarkdownBlock[] = []

  const flushBlocks = (): void => {
    if (pendingBlocks.length > 0) {
      segments.push({ kind: 'markdown', blocks: pendingBlocks })
      pendingBlocks = []
    }
  }

  let index = 0
  while (index < lines.length) {
    const line = lines[index] ?? ''
    const trimmed = line.trim()

    if (trimmed === '') {
      index += 1
      continue
    }

    if (isFenceLine(line)) {
      const fence = trimmed.slice(0, 3)
      const language = trimmed.slice(3).trim()
      const codeLines: string[] = []
      index += 1
      while (index < lines.length && !(lines[index] ?? '').trim().startsWith(fence)) {
        codeLines.push(lines[index] ?? '')
        index += 1
      }
      if (index < lines.length) index += 1
      flushBlocks()
      segments.push({
        kind: 'code',
        language: language === '' ? undefined : language,
        code: codeLines.join('\n'),
      })
      continue
    }

    const heading = /^(#{1,6})\s+(.+)$/.exec(trimmed)
    if (heading !== null) {
      pendingBlocks.push({
        kind: 'heading',
        level: headingLevel((heading[1] ?? '').length),
        children: [...parseForumInlineMarkdown(heading[2] ?? '')],
      })
      index += 1
      continue
    }

    if (/^[-*_]{3,}$/.test(trimmed)) {
      flushBlocks()
      segments.push({ kind: 'rule' })
      index += 1
      continue
    }

    if (/^\s*>/.test(line)) {
      const quoteLines: string[] = []
      while (index < lines.length && /^\s*>/.test(lines[index] ?? '')) {
        quoteLines.push((lines[index] ?? '').replace(/^\s*>\s?/, '').trim())
        index += 1
      }
      pendingBlocks.push({
        kind: 'blockquote',
        children: [paragraph(quoteLines.join(' '))],
      })
      continue
    }

    const ordered = /^\s*\d+[.)]\s+/.test(line)
    const unordered = /^\s*[-*+]\s+/.test(line)
    if (ordered || unordered) {
      const pattern = ordered
        ? /^\s*\d+[.)]\s+(.+)$/
        : /^\s*[-*+]\s+(.+)$/
      const items: MarkdownBlock[][] = []
      while (index < lines.length) {
        const current = lines[index] ?? ''
        const match = pattern.exec(current)
        if (match === null) {
          // Allow blank separator lines between items of the same list.
          if (current.trim() === '') {
            let lookahead = index + 1
            while (lookahead < lines.length && (lines[lookahead] ?? '').trim() === '') {
              lookahead += 1
            }
            if (lookahead < lines.length && pattern.test(lines[lookahead] ?? '')) {
              index = lookahead
              continue
            }
          }
          break
        }
        items.push([paragraph(match[1] ?? '')])
        index += 1
      }
      pendingBlocks.push({ kind: 'list', ordered, items })
      continue
    }

    const paragraphLines: string[] = []
    while (index < lines.length && !isBoundary(lines[index] ?? '')) {
      paragraphLines.push((lines[index] ?? '').trim())
      index += 1
    }
    pendingBlocks.push(paragraph(paragraphLines.join(' ')))
  }

  flushBlocks()
  return segments.length === 0
    ? [{ kind: 'markdown', blocks: [paragraph('')] }]
    : segments
}

// The former EN-2 origin-resolution workaround (absolutizeMarkdown*Hrefs) is
// gone: effect-native v28 (issue #71, vendored at the pinned commit in
// packages/effect-native-vendor.json) admits same-origin rooted paths and
// #fragment refs on markdown link hrefs directly, so parsed trees enter the
// Markdown component without baking in a serving origin.
