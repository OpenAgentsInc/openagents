import { describe, expect, it } from 'vitest'

import type { AutopilotWorkReviewDecisionRecord } from './autopilot-work-routes'
import {
  buildAutopilotDecisionCloseoutReceipt,
  type AutopilotDecisionCloseoutReceipt,
} from './autopilot-decision-closeout'
import { createAutopilotDecisionCloseoutLedger } from './autopilot-decision-closeout-ledger'

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

describe('createAutopilotDecisionCloseoutLedger', () => {
  it('starts empty', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    expect(ledger.list()).toEqual([])
    expect(ledger.summary().count).toBe(0)
    expect(ledger.get('decision.closeout.accept.wo_1')).toBeUndefined()
  })

  it('records and reads back the first closeout for a decision', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const receipt = build()

    expect(ledger.append(receipt)).toEqual({ accepted: true, deduped: false })
    expect(ledger.get(receipt.closeoutRef)).toEqual(receipt)
    expect(ledger.list()).toHaveLength(1)
    expect(ledger.summary().count).toBe(1)
  })

  it('rejects a receipt that fails validation', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const tampered = { ...build(), outcome: 'applied', resolvedState: 'rejected' }

    expect(ledger.append(tampered)).toEqual({
      accepted: false,
      reason: 'invalid',
    })
    expect(ledger.list()).toEqual([])
  })

  it('converges an idempotent replay (applied then duplicate) without growing', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const applied = build({ idempotent: false })
    // Same decision resolved again: same closeoutRef, duplicate outcome, later ts.
    const replay = build({
      idempotent: true,
      decidedAt: '2026-06-20T02:00:00.000Z',
    })

    expect(applied.closeoutRef).toBe(replay.closeoutRef)
    expect(applied.line).not.toBe(replay.line)

    expect(ledger.append(applied)).toEqual({ accepted: true, deduped: false })
    expect(ledger.append(replay)).toEqual({ accepted: true, deduped: true })

    // The ledger held exactly one closeout, keeping the canonical applied one.
    expect(ledger.list()).toHaveLength(1)
    expect(ledger.get(applied.closeoutRef)?.outcome).toBe('applied')
    expect(ledger.summary().byOutcome).toEqual({ applied: 1, duplicate: 0 })
  })

  it('refuses a conflicting second closeout for the same closeoutRef', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const applied = build()
    // Same closeoutRef (same action + work order) but a different actor —
    // two parties must not disagree about who closed the decision.
    const conflicting = build({
      reviewDecision: reviewDecision({ actorAgentUserId: 'user_imposter_2' }),
    })
    expect(conflicting.closeoutRef).toBe(applied.closeoutRef)

    expect(ledger.append(applied)).toEqual({ accepted: true, deduped: false })
    const result = ledger.append(conflicting)
    expect(result.accepted).toBe(false)
    if (result.accepted === false && result.reason === 'conflict') {
      expect(result.existing.actorAgentUserId).toBe('user_owner_1')
    } else {
      throw new Error('expected a conflict result')
    }
    expect(ledger.list()).toHaveLength(1)
  })

  it('serves audit slices by work order, actor, and outcome', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const accept = build()
    const reject = build({
      decisionRef: 'decision_action.wo_2.approve_pr_draft',
      workOrderRef: 'wo_2',
      reviewDecision: reviewDecision({
        action: 'reject',
        actorAgentUserId: 'user_owner_2',
        decisionRefs: [],
        rejectionRefs: ['decision.queue.reject.wo_2'],
      }),
    })

    ledger.append(accept)
    ledger.append(reject)

    expect(ledger.byWorkOrder('wo_1')).toEqual([accept])
    expect(ledger.byActor('user_owner_2')).toEqual([reject])
    expect(ledger.byOutcome('applied')).toHaveLength(2)
    expect(ledger.byOutcome('duplicate')).toEqual([])
  })

  it('summarizes counts by outcome and action', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    ledger.append(build())
    ledger.append(
      build({
        decisionRef: 'decision_action.wo_3.approve_pr_draft',
        workOrderRef: 'wo_3',
        reviewDecision: reviewDecision({
          action: 'request_changes',
          decisionRefs: [],
          revisionRequestRefs: ['decision.queue.request_changes.wo_3'],
        }),
      }),
    )

    expect(ledger.summary()).toEqual({
      count: 2,
      byOutcome: { applied: 2, duplicate: 0 },
      byAction: { accept: 1, reject: 0, request_changes: 1 },
    })
  })

  it('returns mutation-safe snapshots', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    ledger.append(build())

    const snapshot = ledger.list()
    // Mutating the returned array must not affect the ledger's own store.
    snapshot.push(build({ workOrderRef: 'wo_9' }))
    expect(ledger.list()).toHaveLength(1)

    // Each read hands back a fresh receipt object and a fresh refs array.
    const first = ledger.get('decision.closeout.accept.wo_1')
    const second = ledger.get('decision.closeout.accept.wo_1')
    expect(first).toEqual(second)
    expect(first).not.toBe(second)
    expect(first?.receiptRefs).not.toBe(second?.receiptRefs)
  })

  it('keeps ledgers isolated from one another', () => {
    const a = createAutopilotDecisionCloseoutLedger()
    const b = createAutopilotDecisionCloseoutLedger()

    a.append(build())

    expect(a.list()).toHaveLength(1)
    expect(b.list()).toHaveLength(0)
  })
})
