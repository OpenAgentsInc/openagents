import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from '../../../domain/session'
import { ProRoute } from '../../../route'
import {
  ProAgentDashboardLoaded,
  init as initLoggedIn,
} from '../model'
import { view } from './pro'

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

describe('/pro page', () => {
  test('renders live and retained dashboard rows from fixture projection rows', () => {
    const model = initLoggedIn(
      ProRoute(),
      authBootstrapFromSession({
        email: 'operator@example.com',
        name: 'Operator',
        userId: 'user.public.operator',
      }),
    )
    const loaded = {
      ...model,
      proAgentDashboard: ProAgentDashboardLoaded({
        response: {
          generatedAt: '2026-07-01T12:05:00.000Z',
          liveEntries: [
            {
              acknowledgedAt: '2026-07-01T12:00:00.000Z',
              agentLabel: 'codex_sdk',
              id: 'runner.public.codex.1',
              lastAssistantMessage: 'status.public.runner.working',
              prompt: 'task.public.t10_2',
              state: 'working',
              stateHistory: [
                {
                  at: '2026-07-01T11:59:00.000Z',
                  label: 'Waiting',
                  state: 'waiting',
                },
                {
                  at: '2026-07-01T12:00:00.000Z',
                  label: 'Working',
                  state: 'working',
                },
              ],
              stateStartedAt: '2026-07-01T12:00:00.000Z',
              toolName: 'status.list',
              unread: true,
              updatedAt: '2026-07-01T12:02:00.000Z',
              worktreeLabel: 'issue-7878',
            },
          ],
          retainedEntries: [
            {
              acknowledgedAt: '2026-07-01T12:04:30.000Z',
              agentLabel: 'codex_sdk',
              id: 'runner.public.codex.1',
              lastAssistantMessage: 'Done from runner status spine.',
              prompt: 'assignment.public.issue_7878',
              state: 'done',
              stateHistory: [
                {
                  at: '2026-07-01T12:04:00.000Z',
                  label: 'Done',
                  state: 'done',
                },
              ],
              stateStartedAt: '2026-07-01T12:04:00.000Z',
              toolName: 'codex_sdk',
              unread: false,
              updatedAt: '2026-07-01T12:04:30.000Z',
              worktreeLabel: 'issue-7878',
            },
          ],
          diffComments: [],
        },
      }),
    }

    const rendered = renderHtml(view(loaded))

    expect(rendered).toContain('Agent operations')
    expect(rendered).toContain('codex_sdk')
    expect(rendered).toContain('2026-07-01T12:00:00.000Z')
    expect(rendered).toContain('status.public.runner.working')
    expect(rendered).toContain('retained')
    expect(rendered).not.toContain('Codex lane 1')
    expect(rendered).not.toContain('future-runner-fixture')
  })
})
