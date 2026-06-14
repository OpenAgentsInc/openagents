import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { DemoRoute } from '../../route'
import { SelectedTrainingSceneNode } from './message'
import { init } from './model'
import { update } from './update'
import { view } from './view'

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

describe('demo view', () => {
  test('renders the fullscreen training scene at demo root', () => {
    const rendered = renderHtml(view(init(DemoRoute())))

    expect(rendered).toContain('data-component="demo-training-fullscreen"')
    expect(rendered).toContain('<oa-training-run')
    expect(rendered).toContain('Training Live')
    expect(rendered).toContain('CS336 A1 public run')
    expect(rendered).toContain('Active windows')
    expect(rendered).toContain('href="/demo2"')
  })

  test('changes overlay data when a training node is selected', () => {
    const [model] = update(
      init(DemoRoute()),
      SelectedTrainingSceneNode({ nodeId: 'freivalds' }),
    )
    const rendered = renderHtml(view(model))

    expect(rendered).toContain('data-selected-training-node="freivalds"')
    expect(rendered).toContain('Freivalds')
    expect(rendered).toContain('Challenge refs')
  })
})
