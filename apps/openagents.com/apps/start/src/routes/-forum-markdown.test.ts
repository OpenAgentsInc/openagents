// APP-FORUM (#8635) — markdown parity tests for the typed forum post parser.

import { describe, expect, test } from 'vitest'

import {
  parseForumInlineMarkdown,
  parseForumMarkdown,
  safeForumMarkdownHref,
} from './-forum-markdown'

describe('forum markdown parser (#8635)', () => {
  test('href safety matches the legacy policy', () => {
    expect(safeForumMarkdownHref('/forum/t/abc')).toBe('/forum/t/abc')
    expect(safeForumMarkdownHref('https://openagents.com/forum')).toBe(
      'https://openagents.com/forum',
    )
    expect(safeForumMarkdownHref('http://example.com/x')).toBe(
      'http://example.com/x',
    )
    expect(safeForumMarkdownHref('//evil.example')).toBe('')
    expect(safeForumMarkdownHref('javascript:alert(1)')).toBe('')
    expect(safeForumMarkdownHref('data:text/html,x')).toBe('')
  })

  test('inline parsing: code, strong, emphasis, safe and unsafe links', () => {
    const parts = parseForumInlineMarkdown(
      'Use `pylon` with **bold** and *soft* plus [docs](/docs) and [bad](javascript:x).',
    )
    const serialized = JSON.stringify(parts)
    expect(serialized).toContain('{"kind":"code","text":"pylon"}')
    expect(serialized).toContain('"kind":"strong"')
    expect(serialized).toContain('"kind":"emphasis"')
    expect(serialized).toContain('"kind":"link","href":"/docs"')
    // Unsafe link degrades to its label text; the scheme never survives.
    expect(serialized).not.toContain('javascript:x')
    expect(serialized).toContain('bad')
  })

  test('block parsing: paragraphs, headings, lists, quotes, rules, fences', () => {
    const segments = parseForumMarkdown(
      [
        '# Title',
        '',
        'A paragraph over',
        'two lines.',
        '',
        '> quoted words',
        '',
        '- one',
        '- two',
        '',
        '1. first',
        '2. second',
        '',
        '---',
        '',
        '```ts',
        'const a = 1',
        '```',
      ].join('\n'),
    )

    // One markdown run before the rule; the fence is its own segment.
    const markdown = segments.filter((segment) => segment.kind === 'markdown')
    expect(markdown).toHaveLength(1)
    const blocks = markdown.flatMap((segment) =>
      segment.kind === 'markdown' ? segment.blocks : [],
    )
    const kinds = blocks.map((block) => block.kind)
    expect(kinds).toEqual(['heading', 'paragraph', 'blockquote', 'list', 'list'])

    const heading = blocks[0]
    expect(heading).toMatchObject({ kind: 'heading', level: 4 })

    const paragraph = blocks[1]
    expect(JSON.stringify(paragraph)).toContain('A paragraph over two lines.')

    const lists = blocks.filter((block) => block.kind === 'list')
    expect(lists[0]).toMatchObject({ kind: 'list', ordered: false })
    expect(lists[1]).toMatchObject({ kind: 'list', ordered: true })
    expect(lists[0]?.kind === 'list' ? lists[0].items : []).toHaveLength(2)

    expect(segments.some((segment) => segment.kind === 'rule')).toBe(true)

    const code = segments.find((segment) => segment.kind === 'code')
    expect(code).toMatchObject({
      kind: 'code',
      language: 'ts',
      code: 'const a = 1',
    })
  })

  test('empty input renders an empty paragraph, never throws', () => {
    const segments = parseForumMarkdown('')
    expect(segments).toHaveLength(1)
    expect(segments[0]).toMatchObject({ kind: 'markdown' })
  })
})
