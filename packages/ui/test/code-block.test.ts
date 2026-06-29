import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import { AiElements } from '../src/index'

// Minimal vnode → string renderer (mirrors component-class-migration.test.ts),
// sufficient to assert the rendered markup contract of the code block.
type VNodeLike = {
  sel?: string
  data?: { attrs?: Record<string, unknown>; class?: Record<string, boolean> }
  children?: ReadonlyArray<VNodeLike | string>
  text?: string
}

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const classNames = Object.entries(node.data?.class ?? {}).flatMap(
    ([className, enabled]) => (enabled ? [className] : []),
  )
  const entries = [
    ...Object.entries(attrs),
    ...(classNames.length > 0 ? [['class', classNames.join(' ')]] : []),
  ]
  return entries
    .map(([key, value]) =>
      value === '' ? ` ${key}=""` : ` ${key}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) return ''
  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child => (typeof child === 'string' ? child : renderHtml(child as Html)))
    .join('')
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

describe('ai-elements code block', () => {
  test('renders a highlighted, copyable, framed code surface', () => {
    const rendered = renderHtml(
      AiElements.codeBlock({
        props: {
          code: 'const total = 42',
          language: 'typescript',
          filename: 'sum.ts',
        },
      }),
    )

    // Base-contract markers for the family + new copy primitive.
    expect(rendered).toContain('ai-elements:code-block/CodeBlock')
    expect(rendered).toContain('ai-elements:code-block/CodeBlockHeader')
    expect(rendered).toContain('ai-elements:code-block/CodeBlockBody')
    expect(rendered).toContain('ai-elements:code-block/CodeBlockCopyButton')

    // Host hooks for the copy controller.
    expect(rendered).toContain('data-oa-code-block=""')
    expect(rendered).toContain('data-oa-code-copy=""')
    expect(rendered).toContain('data-oa-code-copy-label=""')

    // Pristine, byte-faithful source rides along for copy.
    expect(rendered).toContain('data-oa-code-source=""')
    expect(rendered).toContain('const total = 42')

    // Filename header + syntax tokens (keyword color + number color).
    expect(rendered).toContain('sum.ts')
    expect(rendered).toContain('text-[#7aa2ff]') // keyword `const`
    expect(rendered).toContain('text-[#4fd0ff]') // number `42`
  })

  test('omits the copy button when copy is disabled', () => {
    const rendered = renderHtml(
      AiElements.codeBlock({
        props: { code: 'x = 1', language: 'python' },
        copy: false,
      }),
    )
    expect(rendered).not.toContain('data-oa-code-copy=""')
  })

  test('renders a line-number gutter when requested', () => {
    const rendered = renderHtml(
      AiElements.codeBlock({
        props: { code: 'a\nb\nc', language: 'typescript' },
        showLineNumbers: true,
        copy: false,
      }),
    )
    // Gutter shows 1-based line numbers (text nodes render as <node>N</node>).
    expect(rendered).toContain('text-[#34507f]') // gutter class
    expect(rendered).toContain('>1</node>')
    expect(rendered).toContain('>3</node>')
  })
})
