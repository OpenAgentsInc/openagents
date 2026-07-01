import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import { AiElements } from '../src/index'

type VNodeLike = {
  sel?: string
  data?: {
    attrs?: Record<string, unknown>
    class?: Record<string, boolean>
    props?: Record<string, unknown>
    style?: Record<string, string>
  }
  children?: ReadonlyArray<VNodeLike | string>
  text?: string
}

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const styles = node.data?.style ?? {}
  const classNames = Object.entries(node.data?.class ?? {}).flatMap(
    ([className, enabled]) => (enabled ? [className] : []),
  )
  const entries = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(Object.keys(styles).length === 0
      ? []
      : [
          [
            'style',
            Object.entries(styles)
              .map(([key, value]) => `${key}:${value}`)
              .join(';'),
          ],
        ]),
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

describe('ai-elements shimmer', () => {
  test('renders an accessible shimmer status primitive', () => {
    const rendered = renderHtml(
      AiElements.shimmer({
        props: {
          as: 'p',
          children: 'Thinking',
          duration: 1.4,
          spread: 2,
        },
      }),
    )

    expect(rendered).toContain('data-ui-base="ai-elements:shimmer/Shimmer"')
    expect(rendered).toContain('class="oa-ai-shimmer"')
    expect(rendered).toContain('role="status"')
    expect(rendered).toContain('aria-live="polite"')
    expect(rendered).toContain('--oa-ai-shimmer-duration:1.4s')
    expect(rendered).toContain('--oa-ai-shimmer-spread:16.00ch')
    expect(rendered).toContain('Thinking')
  })

  test('ships token-backed CSS with reduced-motion fallback', async () => {
    const css = await Bun.file(
      new URL('../src/ai-elements/shimmer.css', import.meta.url),
    ).text()

    expect(css).toContain('.oa-ai-shimmer')
    expect(css).toContain('@keyframes oa-ai-shimmer-sweep')
    expect(css).toContain('var(--oa-color-khala-energy-text-strong, #cdeeff)')
    expect(css).toContain('@media (prefers-reduced-motion: reduce)')
  })
})
