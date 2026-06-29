import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import {
  AutopilotDecisionActIdle,
  AutopilotDecisionsLoaded,
} from '../model'
import type { Model } from '../model'
import { view } from './decisions'

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

describe('Autopilot decisions page', () => {
  test('renders closeout receipt state from the decision projection', () => {
    const model = {
      autopilotDecisionAct: AutopilotDecisionActIdle(),
      autopilotDecisions: AutopilotDecisionsLoaded({
        response: {
          decisions: [
            {
              closeoutReceipts: [
                {
                  action: 'accept',
                  closeoutRef:
                    'decision.closeout.accept.autopilot_work_order.test_1',
                  decidedAt: '2026-06-11T17:30:00.000Z',
                  outcome: 'applied',
                  receiptRefs: [
                    'receipt.review.accept.autopilot_work_order.test_1',
                  ],
                  resolvedState: 'accepted',
                },
              ],
              decision: {
                accountLeaseRefs: [],
                actionKind: 'approve_pr_draft',
                actionLabel: 'Approve PR draft',
                actionRef:
                  'decision_action.autopilot_work_order.test_1.approve_pr_draft',
                actionSubmissionRefs: [],
                actionSubmissionRequired: true,
                assignmentRefs: [],
                audience: 'customer',
                blockedReasonRefs: [],
                createdAtDisplay: 'just now',
                customerNextActionRef: 'next_action.review_recorded',
                directEffectPermitted: false,
                evidenceRefs: [],
                id: 'decision_action.autopilot_work_order.test_1.approve_pr_draft',
                missionRef: 'mission.autopilot_work_order.test_1',
                prerequisiteRefs: [],
                programRunRef: null,
                receiptRefs: [
                  'receipt.review.accept.autopilot_work_order.test_1',
                ],
                routeRefs: [],
                safeSummaryRef: 'summary.review_decision_recorded',
                sourceAuthorityRefs: [],
                status: 'completed',
                statusLabel: 'Completed',
                updatedAtDisplay: 'just now',
                workroomRefs: [],
              },
              work: {
                createdAt: '2026-06-11T16:00:00.000Z',
                state: 'accepted',
                taskRefs: ['task.public.test'],
                updatedAt: '2026-06-11T17:30:00.000Z',
                workOrderRef: 'autopilot_work_order.test_1',
              },
            },
          ],
          directEffectPermitted: false,
          generatedAt: '2026-06-11T17:30:00.000Z',
          pendingCount: 0,
        },
      }),
    } as Model

    const rendered = renderHtml(view(model))

    expect(rendered).toContain(
      'decision.closeout.accept.autopilot_work_order.test_1',
    )
    expect(rendered).toContain(
      'receipt.review.accept.autopilot_work_order.test_1',
    )
    expect(rendered).toContain('accepted')
    expect(rendered).toContain('applied')
  })
})
