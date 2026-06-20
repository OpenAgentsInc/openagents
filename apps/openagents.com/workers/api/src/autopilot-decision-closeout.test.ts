import { describe, expect, it } from 'vitest'

import type { AutopilotWorkReviewDecisionRecord } from './autopilot-work-routes'
import {
  buildAutopilotDecisionCloseoutReceipt,
  validateAutopilotDecisionCloseoutReceipt,
  type AutopilotDecisionCloseoutReceipt,
} from './autopilot-decision-closeout'

const reviewDecision = (
  overrides: Partial<AutopilotWorkReviewDecisionRecord> = {},
): AutopilotWorkReviewDecisionRecord => ({
  action: 'accept',
  actorAgentCredentialId: 'cred_123',
  actorAgentUserId: 'user_owner_1',
  decisionRefs: ['decision.queue.accept.wo_1'],
  idempotencyKeyHash: 'abc123hash',
  recordedAt: '2026-06-20T00:00:00.000Z',
  rejectionRefs: [],
  revisionRequestRefs: [],
  ...overrides,
})

const build = (
  overrides: Partial<
    Parameters<typeof buildAutopilotDecisionCloseoutReceipt>[0]
  > = {},
): AutopilotDecisionCloseoutReceipt =>
  buildAutopilotDecisionCloseoutReceipt({
    decisionRef: 'decision_action.wo_1.approve_pr_draft',
    workOrderRef: 'wo_1',
    reviewDecision: reviewDecision(),
    idempotent: false,
    decidedAt: '2026-06-20T01:00:00.000Z',
    ...overrides,
  })

describe('buildAutopilotDecisionCloseoutReceipt', () => {
  it('builds an applied closeout for a first accept resolution', () => {
    const receipt = build()

    expect(receipt.kind).toBe('autopilot_decision_closeout_receipt')
    expect(receipt.outcome).toBe('applied')
    expect(receipt.action).toBe('accept')
    expect(receipt.resolvedState).toBe('accepted')
    expect(receipt.closeoutRef).toBe('decision.closeout.accept.wo_1')
    expect(receipt.hasAnswer).toBe(false)
    expect(validateAutopilotDecisionCloseoutReceipt(receipt)).toBe(true)
  })

  it('classifies an idempotent replay as a duplicate closeout', () => {
    const receipt = build({ idempotent: true })

    expect(receipt.outcome).toBe('duplicate')
    expect(validateAutopilotDecisionCloseoutReceipt(receipt)).toBe(true)
  })

  it('keeps the closeoutRef stable across applied and duplicate (exactly-once key)', () => {
    expect(build({ idempotent: false }).closeoutRef).toBe(
      build({ idempotent: true }).closeoutRef,
    )
  })

  it('maps reject and request_changes to their resolved states', () => {
    expect(build({ reviewDecision: reviewDecision({ action: 'reject' }) }).resolvedState).toBe(
      'rejected',
    )
    expect(
      build({
        reviewDecision: reviewDecision({ action: 'request_changes' }),
      }).resolvedState,
    ).toBe('revision_required')
  })

  it('captures and normalizes public-safe receipt refs only', () => {
    const receipt = build({
      reviewDecision: reviewDecision({
        action: 'reject',
        decisionRefs: [],
        rejectionRefs: [
          'decision.queue.reject.wo_1',
          '  decision.queue.reject.wo_1  ',
          'has spaces are unsafe',
        ],
      }),
    })

    expect(receipt.receiptRefs).toContain('receipt.review.reject.wo_1')
    expect(receipt.receiptRefs).toContain('decision.queue.reject.wo_1')
    expect(receipt.receiptRefs).not.toContain('has spaces are unsafe')
    // sorted + deduped
    expect([...receipt.receiptRefs]).toEqual([...receipt.receiptRefs].sort())
    expect(new Set(receipt.receiptRefs).size).toBe(receipt.receiptRefs.length)
  })
})

describe('validateAutopilotDecisionCloseoutReceipt', () => {
  it('rejects a non-object', () => {
    expect(validateAutopilotDecisionCloseoutReceipt(null)).toBe(false)
    expect(validateAutopilotDecisionCloseoutReceipt('x')).toBe(false)
  })

  it('rejects a tampered outcome (line no longer reconstructs)', () => {
    const tampered = { ...build(), outcome: 'duplicate' as const }
    expect(validateAutopilotDecisionCloseoutReceipt(tampered)).toBe(false)
  })

  it('rejects an action/resolvedState mismatch', () => {
    const tampered = { ...build(), resolvedState: 'rejected' as const }
    expect(validateAutopilotDecisionCloseoutReceipt(tampered)).toBe(false)
  })

  it('rejects an unknown action', () => {
    const tampered = { ...build(), action: 'approve' }
    expect(validateAutopilotDecisionCloseoutReceipt(tampered)).toBe(false)
  })

  it('rejects a tampered line', () => {
    const tampered = { ...build(), line: 'forged' }
    expect(validateAutopilotDecisionCloseoutReceipt(tampered)).toBe(false)
  })
})
