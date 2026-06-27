import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { PylonCodexAssignmentStatusRoute } from '../route'
import * as StatusPage from './pylonCodexAssignmentStatus'

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

describe('Pylon Codex assignment status page', () => {
  test('renders the assignment ref and owner-scoped closeout commands', () => {
    const route = PylonCodexAssignmentStatusRoute({
      assignmentRef: 'assignment.public.khala_coding.chatcmpl_example',
    })
    const rendered = renderHtml(StatusPage.view(route, { _tag: 'LoggedOut' }))

    expect(rendered).toContain('Pylon Codex assignment')
    expect(rendered).toContain(
      'assignment.public.khala_coding.chatcmpl_example',
    )
    expect(rendered).toContain('pylon khala status --assignment-ref')
    expect(rendered).toContain('pylon khala proof')
    expect(rendered).toContain('proofChecklist.blockerRefs')
    expect(rendered).toContain('pylon-codex-own-capacity')
    expect(rendered).not.toMatch(/rawEventsJson|safe_metadata_json|bearer/i)
  })

  test('uses an assignment-specific document title', () => {
    expect(
      StatusPage.title(
        PylonCodexAssignmentStatusRoute({
          assignmentRef: 'assignment.public.khala_coding.chatcmpl_example',
        }),
      ),
    ).toBe(
      'Pylon Codex assignment assignment.public.khala_coding.chatcmpl_example - OpenAgents',
    )
  })
})
