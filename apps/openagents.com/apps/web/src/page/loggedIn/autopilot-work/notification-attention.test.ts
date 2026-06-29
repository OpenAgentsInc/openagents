import { describe, expect, test } from 'vitest'

import {
  buildForgeNotificationAttentionInput,
  projectForgeNotificationAttention,
} from './notification-attention'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T22:30:00.000Z',
  snapshotRef: 'attention-snapshot.public.work_1',
  versionRef: 'attention-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge notification/attention projection', () => {
  test('projects actionable attention as refs-only non-authoritative state', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          actionRefs: ['action.public.review_required'],
          attentionRef: 'attention.public.review_required',
          channelRefs: ['channel.public.web'],
          decisionRefs: ['decision.public.review_required'],
          deliveryRefs: ['delivery.public.web.review_required'],
          dedupeRefs: ['dedupe.public.review_required'],
          freshness: 'fresh',
          notificationRefs: ['notification.public.review_required'],
          policyRefs: ['policy.public.attention.review'],
          resolutionRefs: [],
          severity: 'warning',
          state: 'waiting',
        },
      ],
    })

    expect(view.status).toBe('attention')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      active: 0,
      critical: 0,
      delivered: 1,
      total: 1,
      waiting: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      approvalRequestAuthority: false,
      attentionResolutionAuthority: false,
      decisionActionAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      notificationSendAuthority: false,
      notificationSubscriptionAuthority: false,
      providerAuthority: false,
      publicClaimAuthority: false,
      runStateMutationAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing notification/attention state as empty', () => {
    const view = projectForgeNotificationAttention({
      generatedAt: '2026-06-17T22:30:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.attention).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale attention evidence', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          attentionRef: 'attention.public.stale',
          deliveryRefs: ['delivery.public.stale'],
          freshness: 'stale',
          notificationRefs: ['notification.public.stale'],
          policyRefs: ['policy.public.attention'],
          severity: 'warning',
          state: 'active',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:stale-attention-evidence:attention.public.stale',
    )
  })

  test('blocks active notifications without policy or delivery refs', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          attentionRef: 'attention.public.no_delivery',
          freshness: 'fresh',
          notificationRefs: ['notification.public.no_delivery'],
          severity: 'critical',
          state: 'active',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:attention-policy-missing:attention.public.no_delivery',
    )
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:notification-delivery-missing:attention.public.no_delivery',
    )
  })

  test('blocks waiting attention without decision or action refs', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          attentionRef: 'attention.public.waiting',
          deliveryRefs: ['delivery.public.waiting'],
          freshness: 'fresh',
          notificationRefs: ['notification.public.waiting'],
          policyRefs: ['policy.public.attention'],
          severity: 'warning',
          state: 'waiting',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:decision-action-ref-missing:attention.public.waiting',
    )
  })

  test('blocks resolved or invalidated attention without closeout refs', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          attentionRef: 'attention.public.resolved',
          freshness: 'fresh',
          severity: 'info',
          state: 'resolved',
        },
        {
          attentionRef: 'attention.public.invalidated',
          freshness: 'fresh',
          severity: 'info',
          state: 'invalidated',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:resolution-ref-missing:attention.public.resolved',
    )
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:invalidation-ref-missing:attention.public.invalidated',
    )
  })

  test('omits unsafe private notification material before projection', () => {
    const view = projectForgeNotificationAttention({
      ...baseInput,
      attention: [
        {
          actionRefs: ['action.public.safe'],
          attentionRef: 'attention.public.safe',
          blockerRefs: ['entry-blocker.public.safe', 'raw notification sk-private'],
          channelRefs: ['channel.public.safe'],
          decisionRefs: ['decision.public.safe', 'private notification token'],
          deliveryRefs: ['delivery.public.safe'],
          dedupeRefs: ['dedupe.public.safe'],
          freshness: 'fresh',
          notificationRefs: [
            'notification.public.safe',
            'notification body /Users/christopher/message.md',
          ],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          severity: 'warning',
          state: 'waiting',
        },
      ],
      blockerRefs: [
        'attention-blocker.public.safe',
        'attention message /Users/christopher/attention.md',
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.attention[0]?.notificationRefs).toEqual(['notification.public.safe'])
    expect(view.attention[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.attention[0]?.decisionRefs).toEqual(['decision.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-notification-attention-blocker:work.public.work_1:unsafe-notification-attention-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw notification')
    expect(payload).not.toContain('private notification')
    expect(payload).not.toContain('notification body')
    expect(payload).not.toContain('attention message')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T22:31:00.000Z',
      notificationAttention: {
        attention: [
          {
            attentionRef: 'attention.public.work_2',
            deliveryRefs: ['delivery.public.work_2'],
            freshness: 'fresh',
            notificationRefs: ['notification.public.work_2'],
            policyRefs: ['policy.public.work_2'],
            resolutionRefs: ['resolution.public.work_2'],
            severity: 'info',
            state: 'resolved',
          },
        ],
        snapshotRef: 'attention-snapshot.public.work_2',
        versionRef: 'attention-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeNotificationAttentionInput(work)).toEqual({
      attention: [
        {
          attentionRef: 'attention.public.work_2',
          deliveryRefs: ['delivery.public.work_2'],
          freshness: 'fresh',
          notificationRefs: ['notification.public.work_2'],
          policyRefs: ['policy.public.work_2'],
          resolutionRefs: ['resolution.public.work_2'],
          severity: 'info',
          state: 'resolved',
        },
      ],
      generatedAt: '2026-06-17T22:31:00.000Z',
      snapshotRef: 'attention-snapshot.public.work_2',
      versionRef: 'attention-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
