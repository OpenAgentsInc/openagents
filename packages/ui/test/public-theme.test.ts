import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import {
  publicLandingThemeScript,
  publicLandingThemeSelector,
  publicLandingThemeShell,
  publicLandingThemeStorageKey,
} from '../src'

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

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe('public landing theme primitives', () => {
  test('renders a scoped shell with stable public landing markers', () => {
    const rendered = renderHtml(
      publicLandingThemeShell({
        preference: 'system',
        children: [publicLandingThemeSelector({ preference: 'system' })],
      }),
    )

    expect(rendered).toContain('data-ui-family="public/theme-shells"')
    expect(rendered).toContain('data-public-landing-shell=""')
    expect(rendered).toContain('data-public-landing-theme="dark"')
    expect(rendered).toContain(
      'data-public-landing-theme-preference="system"',
    )
    expect(rendered).toContain('bg-public-landing-page')
    expect(rendered).toContain('text-public-landing-text')
  })

  test('renders a labeled theme selector using the public landing token set', () => {
    const rendered = renderHtml(
      publicLandingThemeSelector({
        preference: 'light',
        label: 'Public landing theme',
      }),
    )

    expect(rendered).toContain('data-ui-family="public/theme-selectors"')
    expect(rendered).toContain('data-public-landing-theme-select=""')
    expect(rendered).toContain('aria-label="Public landing theme"')
    expect(rendered).toContain('bg-public-landing-surface')
    expect(rendered).toContain('Light')
    expect(rendered).toContain('Dark')
  })

  test('keeps the runtime script scoped to landing shells', () => {
    const script = publicLandingThemeScript()

    expect(script).toContain(publicLandingThemeStorageKey)
    expect(script).toContain("window.matchMedia('(prefers-color-scheme: dark)')")
    expect(script).toContain('[data-public-landing-shell]')
    expect(script).toContain('[data-public-landing-theme-select]')
    expect(script).toContain('data-public-landing-theme')
    expect(script).not.toContain('data-forum-theme')
    expect(script).not.toContain('document.documentElement.setAttribute')
  })
})
