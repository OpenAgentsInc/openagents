import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import * as DemoLegal from './page/demoLegal'
import { DemoLegalRoute, DemoRoute, urlToAppRoute } from './route'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

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

describe('demo legal route', () => {
  test('parses the public /demo/legal path (not the bare /demo demo app)', () => {
    expect(urlToAppRoute(appUrl('/demo/legal'))).toEqual(DemoLegalRoute())
    // The bare /demo still resolves to the existing demo app route.
    expect(urlToAppRoute(appUrl('/demo'))).toEqual(DemoRoute())
  })

  test('keeps unauthenticated users on the legal demo page without an auth bootstrap', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/demo/legal'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'DemoLegal' },
    })
    // Public route: no auth bootstrap command is dispatched.
    expect(commands).toHaveLength(0)
  })

  test('renders the legal MVP components: command bar, quick actions, and the cards', () => {
    const rendered = renderHtml(DemoLegal.view({ _tag: 'LoggedOut' }))

    // Honest, demo-framed value prop (not "legal AI dashboard").
    expect(rendered).toContain(
      'Stay in strategic counsel mode — Forge prepares the work surface.',
    )

    // Command bar with the example request.
    expect(rendered).toContain(
      'I need an NDA for a Texas startup talking to a vendor.',
    )

    // Quick-action chips.
    expect(rendered).toContain('Find a form')
    expect(rendered).toContain('Prepare a consult')
    expect(rendered).toContain('Review this draft')

    // NDA draft card (draft only).
    expect(rendered).toContain('NDA draft')
    expect(rendered).toContain('Draft only')

    // Lawyer-facing review checklist card.
    expect(rendered).toContain('Lawyer review checklist')

    // Time-entry card pending approval + follow-up task.
    expect(rendered).toContain('Time entry')
    expect(rendered).toContain('Pending approval')

    // Matter workspace panel + daily brief.
    expect(rendered).toContain('Matter workspace')
    expect(rendered).toContain('Daily brief')

    // Cards carry 3D anchors for the three-effect htmlOverlay projection.
    expect(rendered).toContain('data-anchor="command-bar"')
    expect(rendered).toContain('data-anchor="nda-draft"')

    // Honest demo framing — not a live legal product.
    expect(rendered).toContain('Demo only — not a live legal product.')
  })
})
