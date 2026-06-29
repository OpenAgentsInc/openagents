import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import * as ArtanisTraceTree from './artanisTraceTree'

type VNodeLike = Readonly<{
  children?: ReadonlyArray<string | VNodeLike | null>
  data?: {
    attrs?: Record<string, unknown>
    class?: Record<string, boolean>
    props?: Record<string, unknown>
  }
  sel?: string
  text?: string
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
  if (html === null || !isVNodeLike(html)) return ''
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
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

describe('Artanis RLM trace tree page', () => {
  test('renders the FRLM conductor tree with Blueprint governance refs', () => {
    const rendered = renderHtml(
      ArtanisTraceTree.view({ _tag: 'LoggedOut' }),
    )

    expect(rendered).toContain('Artanis execution tree')
    expect(rendered).toContain('FrlmConductor')
    expect(rendered).toContain('SubQuery.Submit')
    expect(rendered).toContain('SubQuery.Return')
    expect(rendered).toContain('Run.Done')
    expect(rendered).toContain('program_signature.frlm_conductor.v1')
    expect(rendered).toContain('program_signature.rlm_leaf_executor.v1')
    expect(rendered).toContain('/api/operator/rlm/traces')
    expect(rendered).toContain('No direct execution authority')
  })

  test('keeps private trace material out of the public visualizer', () => {
    const rendered = renderHtml(
      ArtanisTraceTree.view({ _tag: 'LoggedOut' }),
    )

    expect(rendered).not.toMatch(
      /raw_prompt|raw_trace|rawEvents|trajectory_json|bearer|api[_-]?key|sk-[a-z0-9]/i,
    )
  })

  test('uses a route-specific document title', () => {
    expect(ArtanisTraceTree.title()).toBe('Artanis RLM traces - OpenAgents')
  })
})
