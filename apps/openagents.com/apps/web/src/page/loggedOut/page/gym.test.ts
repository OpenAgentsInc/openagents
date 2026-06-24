import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { initGymModel, runGymFixture } from '../gym/flow'
import * as Gym from './gym'

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

describe('public Gym page', () => {
  test('renders public fixture controls and locked economics', () => {
    const rendered = renderHtml(Gym.view(initGymModel()))

    expect(rendered).toContain('data-gym-page')
    expect(rendered).toContain('data-gym-no-spend-banner')
    expect(rendered).toContain('Provider fan-out')
    expect(rendered).toContain('Program signature modules')
    expect(rendered).toContain('fixture only - no spend')
    expect(rendered).toContain('data-gym-run')
  })

  test('renders the report viewer payload after a fixture run', () => {
    const rendered = renderHtml(Gym.view(runGymFixture(initGymModel())))

    expect(rendered).toContain('data-gym-result')
    expect(rendered).toContain('openagents.gym.fixture_report.v1')
    expect(rendered).toContain('Mean cost')
    expect(rendered).toContain('$0.00')
  })
})
