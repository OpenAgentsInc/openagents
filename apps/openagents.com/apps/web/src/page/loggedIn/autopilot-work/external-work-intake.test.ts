import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeExternalWorkIntakeInput,
  projectForgeExternalWorkIntake,
} from './external-work-intake'

const baseInput = {
  generatedAt: '2026-06-18T01:40:00.000Z',
  snapshotRef: 'external-work-intake-snapshot.public.work_1',
  versionRef: 'external-work-intake-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge external work intake projection', () => {
  test('projects admitted external intake as refs-only non-authoritative state', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          acceptancePolicyRefs: ['acceptance-policy.public.review_required'],
          accountRefs: ['account.public.requester'],
          adapterPreferenceRefs: ['adapter-preference.public.pylon'],
          admissionReceiptRefs: ['admission-receipt.public.work_1'],
          apiParityRefs: ['api-parity.public.work_intake'],
          budgetRefs: ['budget.public.work_1'],
          budgetRequired: true,
          capabilityRefs: ['capability.public.repo_write'],
          channel: 'ui',
          dataClassificationRefs: ['data-classification.public_safe'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.work_1'],
          intakeRef: 'intake.public.work_1',
          policyRefs: ['policy.public.intake.ui'],
          requesterRefs: ['requester.public.user_1'],
          reviewPolicyRefs: ['review-policy.public.requester_required'],
          routingReceiptRefs: ['routing-receipt.public.work_1'],
          scopeRefs: ['scope.public.repository'],
          status: 'admitted',
          statusReceiptRefs: ['status-receipt.public.admitted'],
          verificationRefs: ['verification.public.bun_test'],
          workKind: 'coding_task',
          workOrderRefs: ['work-order.public.work_1'],
        },
      ],
    })

    expect(view.status).toBe('admitted')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      admitted: 1,
      delivered: 0,
      pending: 0,
      rejected: 0,
      routed: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      adapterSelectionAuthority: false,
      admissionAuthority: false,
      budgetReserveAuthority: false,
      deploymentAuthority: false,
      enqueueWorkAuthority: false,
      paymentAuthority: false,
      publicClaimAuthority: false,
      rejectionAuthority: false,
      settlementAuthority: false,
      startExecutionAuthority: false,
      workOrderCreateAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing intake state as empty', () => {
    const view = projectForgeExternalWorkIntake({
      generatedAt: '2026-06-18T01:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('renders rejection receipt state without readiness blockers', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.requester'],
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.rejected'],
          intakeRef: 'intake.public.rejected',
          rejectionReceiptRefs: ['rejection-receipt.public.scope_denied'],
          requesterRefs: ['requester.public.user_1'],
          status: 'rejected',
          statusReceiptRefs: ['status-receipt.public.rejected'],
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('rejected')
    expect(view.blockerRefs).toEqual([])
    expect(view.entries[0]?.rejectionReceiptRefs).toEqual([
      'rejection-receipt.public.scope_denied',
    ])
  })

  test('blocks missing identity and account refs', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.missing_identity'],
          intakeRef: 'intake.public.missing_identity',
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:intake-identity-missing:intake.public.missing_identity',
    )
  })

  test('blocks required budget without budget or payment refs', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.requester'],
          budgetRequired: true,
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.no_budget'],
          intakeRef: 'intake.public.no_budget',
          requesterRefs: ['requester.public.user_1'],
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:budget-or-payment-ref-missing:intake.public.no_budget',
    )
  })

  test('blocks payment refs without admission and routing receipts', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.requester'],
          capabilityRefs: ['capability.public.repo_write'],
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.payment'],
          intakeRef: 'intake.public.payment',
          paymentRefs: ['payment.public.reserved'],
          requesterRefs: ['requester.public.user_1'],
          status: 'admitted',
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:payment-without-admission-routing:intake.public.payment',
    )
  })

  test('blocks adapter preferences without routing and policy refs', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.requester'],
          adapterPreferenceRefs: ['adapter-preference.public.pylon'],
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.adapter'],
          intakeRef: 'intake.public.adapter',
          requesterRefs: ['requester.public.user_1'],
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:adapter-routing-policy-missing:intake.public.adapter',
    )
  })

  test('blocks browser intake without API parity refs', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      entries: [
        {
          accountRefs: ['account.public.requester'],
          channel: 'ui',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.ui'],
          intakeRef: 'intake.public.ui',
          requesterRefs: ['requester.public.user_1'],
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:browser-action-api-parity-missing:intake.public.ui',
    )
  })

  test('blocks populated intake entries without snapshot refs', () => {
    const view = projectForgeExternalWorkIntake({
      entries: [
        {
          accountRefs: ['account.public.requester'],
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.no_snapshot'],
          intakeRef: 'intake.public.no_snapshot',
          requesterRefs: ['requester.public.user_1'],
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
      generatedAt: '2026-06-18T01:40:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.no_snapshot:missing-external-work-intake-snapshot-ref',
    )
  })

  test('omits unsafe private intake material before projection', () => {
    const view = projectForgeExternalWorkIntake({
      ...baseInput,
      blockerRefs: [
        'intake-blocker.public.safe',
        'raw intake /Users/christopher/intake.json',
      ],
      entries: [
        {
          acceptancePolicyRefs: ['acceptance-policy.public.safe'],
          accountRefs: ['account.public.safe'],
          adapterPreferenceRefs: ['adapter-preference.public.safe'],
          admissionReceiptRefs: ['admission-receipt.public.safe'],
          apiParityRefs: ['api-parity.public.safe'],
          blockerRefs: ['entry-intake-blocker.public.safe'],
          budgetRefs: ['budget.public.safe'],
          capabilityRefs: ['capability.public.safe'],
          channel: 'ui',
          dataClassificationRefs: [
            'data-classification.public_safe',
            'raw request /Users/christopher/private.md',
          ],
          deliveryReceiptRefs: ['delivery-receipt.public.safe'],
          expirationRefs: ['expiration.public.safe'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.safe'],
          intakeRef: 'intake.public.safe',
          paymentRefs: ['payment.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          requestRefs: ['request.public.safe', 'provider payload sk-private'],
          requesterRefs: ['requester.public.safe'],
          reviewPolicyRefs: ['review-policy.public.safe'],
          routingReceiptRefs: ['routing-receipt.public.safe'],
          scopeRefs: ['scope.public.safe'],
          status: 'admitted',
          statusReceiptRefs: ['status-receipt.public.safe'],
          verificationRefs: ['verification.public.safe'],
          workKind: 'coding_task',
          workOrderRefs: ['work-order.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.requestRefs).toEqual(['request.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-external-work-intake-blocker:work.public.work_1:unsafe-external-work-intake-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw intake')
    expect(payload).not.toContain('raw request')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      externalWorkIntake: {
        entries: [
          {
            accountRefs: ['account.public.work_2'],
            channel: 'api',
            freshness: 'fresh',
            idempotencyRefs: ['idempotency.public.work_2'],
            intakeRef: 'intake.public.work_2',
            requesterRefs: ['requester.public.work_2'],
            status: 'pending',
            workKind: 'coding_task',
          },
        ],
        snapshotRef: 'external-work-intake-snapshot.public.work_2',
        versionRef: 'external-work-intake-version.public.v2',
      },
      generatedAt: '2026-06-18T01:41:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeExternalWorkIntakeInput(work)).toEqual({
      entries: [
        {
          accountRefs: ['account.public.work_2'],
          channel: 'api',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.work_2'],
          intakeRef: 'intake.public.work_2',
          requesterRefs: ['requester.public.work_2'],
          status: 'pending',
          workKind: 'coding_task',
        },
      ],
      generatedAt: '2026-06-18T01:41:00.000Z',
      snapshotRef: 'external-work-intake-snapshot.public.work_2',
      versionRef: 'external-work-intake-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
