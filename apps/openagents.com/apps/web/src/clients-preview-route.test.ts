import {
  decisionRequestFixture,
  sessionListFixture,
} from '@openagentsinc/autopilot-control-protocol/fixtures'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import * as ClientsPreview from './page/clientsPreview'

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

describe('clients preview route', () => {
  test('renders autopilot sessions and a decision card from protocol fixtures', () => {
    const rendered = renderHtml(ClientsPreview.view())

    expect(rendered).toContain('Sessions')
    expect(rendered).toContain('Decision')
    expect(rendered).toContain('data-autopilot-session-list=""')
    expect(rendered).toContain(sessionListFixture[0]?.sessionRef)
    expect(rendered).toContain(sessionListFixture[1]?.sessionRef)
    expect(rendered).toContain(
      `data-autopilot-decision-id="${decisionRequestFixture.requestId}"`,
    )
    expect(rendered).toContain(decisionRequestFixture.actionRef)
  })
})
