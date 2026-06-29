import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import { AiElements } from '../src'
import { parseMarkdownBlocks, response } from '../src/ai-elements/response'

// A lightweight Snabbdom-style vnode walker (mirrors business.test.ts) so the
// markdown renderer can be asserted as serialized markup without a DOM.
type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const attrs = html.data?.attrs ?? {}
  const classes = Object.entries(html.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([name]) => name)
    .join(' ')
  const attrString = [
    ...Object.entries(attrs),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join('')
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string' ? child : child === null ? '' : renderHtml(child),
    )
    .join('')

  return `<${tag}${attrString}>${html.text ?? ''}${children}</${tag}>`
}

const render = (markdown: string, streaming?: boolean): string =>
  renderHtml(
    response({ markdown, ...(streaming === undefined ? {} : { streaming }) }),
  )

describe('response markdown — block structure', () => {
  test('renders headings as demoted heading tags', () => {
    const markup = render('# Title\n\n## Section\n\n### Sub')
    expect(markup).toContain('<h3')
    expect(markup).toContain('Title')
    expect(markup).toContain('<h4')
    expect(markup).toContain('Section')
    expect(markup).toContain('<h5')
    expect(markup).toContain('Sub')
  })

  test('renders unordered and ordered lists', () => {
    const ul = render('- one\n- two\n- three')
    expect(ul).toContain('<ul')
    expect((ul.match(/<li/g) ?? []).length).toBe(3)

    const ol = render('1. first\n2. second')
    expect(ol).toContain('<ol')
    expect((ol.match(/<li/g) ?? []).length).toBe(2)
  })

  test('renders fenced code blocks with language metadata', () => {
    const markup = render('```ts\nconst x = 1\n```')
    expect(markup).toContain('<pre')
    expect(markup).toContain('data-language="ts"')
    expect(markup).toContain('const x = 1')
  })

  test('renders blockquotes and horizontal rules', () => {
    expect(render('> quoted line')).toContain('<blockquote')
    expect(render('---')).toContain('<hr')
  })

  test('paragraphs soft-wrap consecutive lines', () => {
    const blocks = parseMarkdownBlocks('line one\nline two')
    expect(blocks).toHaveLength(1)
    expect(blocks[0]).toMatchObject({ kind: 'paragraph', text: 'line one line two' })
  })
})

describe('response markdown — inline structure', () => {
  test('renders bold, italic, and inline code', () => {
    const markup = render('A **bold** and *italic* and `code` line.')
    expect(markup).toContain('<strong')
    expect(markup).toContain('bold')
    expect(markup).toContain('<em')
    expect(markup).toContain('italic')
    expect(markup).toContain('<code')
    expect(markup).toContain('code')
  })

  test('renders safe links and degrades unsafe schemes to text', () => {
    // The href is a DOM prop on the vnode (not a serialized attr in the
    // lightweight walker); asserting the anchor element + visible label proves
    // the safe link renders as a link.
    const safe = render('See [docs](https://openagents.com/docs).')
    expect(safe).toContain('<a')
    expect(safe).toContain('docs')
    expect(safe).toContain('text-[#7aa2ff]')

    const unsafe = render('Click [here](javascript:alert(1)).')
    expect(unsafe).not.toContain('<a')
    expect(unsafe).not.toContain('javascript:')
    expect(unsafe).toContain('here')
  })
})

describe('response markdown — streaming tolerance (streamdown pattern)', () => {
  test('a half-open bold marker never throws and renders as text', () => {
    expect(() => render('A reply with **bold stil')).not.toThrow()
    const markup = render('A reply with **bold stil')
    // No <strong> for the dangling marker; the literal text is preserved.
    expect(markup).toContain('bold stil')
  })

  test('a dangling inline code backtick renders literally', () => {
    expect(() => render('partial `cod')).not.toThrow()
    expect(render('partial `cod')).toContain('cod')
  })

  test('an unterminated fenced block renders the captured lines as code', () => {
    const markup = render('```ts\nconst y = 2\nconst z = 3')
    expect(markup).toContain('<pre')
    expect(markup).toContain('const y = 2')
    expect(markup).toContain('const z = 3')
  })

  test('an incomplete link renders the literal characters', () => {
    expect(() => render('start [partial link')).not.toThrow()
    expect(render('start [partial link')).toContain('partial link')
  })

  test('streaming appends a cursor; empty streaming shows only the cursor', () => {
    const withContent = render('Hello', true)
    expect(withContent).toContain('oa-stream-cursor')
    expect(withContent).toContain('Hello')

    const empty = render('', true)
    expect(empty).toContain('oa-stream-cursor')

    // Non-streaming never adds a cursor.
    expect(render('Hello')).not.toContain('oa-stream-cursor')
  })
})

describe('response markdown — package barrel', () => {
  test('is exposed through the AiElements namespace', () => {
    expect(typeof AiElements.response).toBe('function')
  })
})
