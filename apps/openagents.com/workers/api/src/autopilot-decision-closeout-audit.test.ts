import { describe, expect, it } from 'vitest'

import { reconcileAutopilotDecisionCloseoutCoverage } from './autopilot-decision-closeout-audit'
import { buildAutopilotDecisionCloseoutReceipt } from './autopilot-decision-closeout'
import { createAutopilotDecisionCloseoutLedger } from './autopilot-decision-closeout-ledger'
import type { AutopilotWorkReviewAction } from './autopilot-work-routes'

const reviewDecision = (
  action: AutopilotWorkReviewAction,
  workOrderRef: string,
) => ({
  action,
  actorAgentCredentialId: 'cred_1',
  actorAgentUserId: 'owner_1',
  decisionRefs: action === 'accept' ? [`decision.queue.accept.${workOrderRef}`] : [],
  idempotencyKeyHash: 'hash_1',
  recordedAt: '2026-06-20T00:00:00.000Z',
  rejectionRefs: action === 'reject' ? [`decision.queue.reject.${workOrderRef}`] : [],
  revisionRequestRefs:
    action === 'request_changes' ? [`decision.queue.request_changes.${workOrderRef}`] : [],
})

const resolved = (action: AutopilotWorkReviewAction, workOrderRef: string) => ({
  action,
  decisionRef: `decision_action.${workOrderRef}.approve_pr_draft`,
  workOrderRef,
})

const recordCloseout = (
  ledger: ReturnType<typeof createAutopilotDecisionCloseoutLedger>,
  action: AutopilotWorkReviewAction,
  workOrderRef: string,
  idempotent = false,
) =>
  ledger.append(
    buildAutopilotDecisionCloseoutReceipt({
      decidedAt: '2026-06-20T00:00:00.000Z',
      decisionRef: `decision_action.${workOrderRef}.approve_pr_draft`,
      idempotent,
      reviewDecision: reviewDecision(action, workOrderRef),
      workOrderRef,
    }),
  )

describe('reconcileAutopilotDecisionCloseoutCoverage', () => {
  it('reports complete coverage with no resolved decisions and an empty ledger', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    const coverage = reconcileAutopilotDecisionCloseoutCoverage({ ledger, resolved: [] })

    expect(coverage).toEqual({ complete: true, covered: [], missing: [], orphans: [] })
  })

  it('marks a resolved decision with a present closeout as covered (with outcome)', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    recordCloseout(ledger, 'accept', 'wo_1')

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({
      ledger,
      resolved: [resolved('accept', 'wo_1')],
    })

    expect(coverage.complete).toBe(true)
    expect(coverage.missing).toEqual([])
    expect(coverage.orphans).toEqual([])
    expect(coverage.covered).toEqual([
      {
        action: 'accept',
        closeoutRef: 'decision.closeout.accept.wo_1',
        decisionRef: 'decision_action.wo_1.approve_pr_draft',
        outcome: 'applied',
        workOrderRef: 'wo_1',
      },
    ])
  })

  it('flags a resolved decision without a closeout as a gap (not complete)', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({
      ledger,
      resolved: [resolved('reject', 'wo_2')],
    })

    expect(coverage.complete).toBe(false)
    expect(coverage.covered).toEqual([])
    expect(coverage.missing).toEqual([
      {
        action: 'reject',
        closeoutRef: 'decision.closeout.reject.wo_2',
        decisionRef: 'decision_action.wo_2.approve_pr_draft',
        workOrderRef: 'wo_2',
      },
    ])
    expect(coverage.missing[0]).not.toHaveProperty('outcome')
  })

  it('reports a ledger closeout with no resolved decision as an orphan', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    recordCloseout(ledger, 'accept', 'wo_orphan')

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({ ledger, resolved: [] })

    expect(coverage.complete).toBe(true)
    expect(coverage.orphans).toEqual(['decision.closeout.accept.wo_orphan'])
  })

  it('separates covered, missing, and orphan across a mixed batch (sorted)', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    recordCloseout(ledger, 'accept', 'wo_b')
    recordCloseout(ledger, 'request_changes', 'wo_z')

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({
      ledger,
      resolved: [resolved('accept', 'wo_b'), resolved('reject', 'wo_a')],
    })

    expect(coverage.complete).toBe(false)
    expect(coverage.covered.map(e => e.closeoutRef)).toEqual(['decision.closeout.accept.wo_b'])
    expect(coverage.missing.map(e => e.closeoutRef)).toEqual(['decision.closeout.reject.wo_a'])
    expect(coverage.orphans).toEqual(['decision.closeout.request_changes.wo_z'])
  })

  it('collapses an idempotent replay (same work order + action) to one expected key', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    recordCloseout(ledger, 'accept', 'wo_3')
    // Replay produces the SAME closeoutRef; ledger dedups it.
    recordCloseout(ledger, 'accept', 'wo_3', true)

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({
      ledger,
      resolved: [resolved('accept', 'wo_3'), resolved('accept', 'wo_3')],
    })

    expect(coverage.complete).toBe(true)
    expect(coverage.covered).toHaveLength(1)
    expect(coverage.missing).toEqual([])
    expect(coverage.orphans).toEqual([])
  })

  it('treats different actions on the same work order as distinct closeout keys', () => {
    const ledger = createAutopilotDecisionCloseoutLedger()
    recordCloseout(ledger, 'accept', 'wo_4')

    const coverage = reconcileAutopilotDecisionCloseoutCoverage({
      ledger,
      resolved: [resolved('accept', 'wo_4'), resolved('reject', 'wo_4')],
    })

    expect(coverage.covered.map(e => e.closeoutRef)).toEqual(['decision.closeout.accept.wo_4'])
    expect(coverage.missing.map(e => e.closeoutRef)).toEqual(['decision.closeout.reject.wo_4'])
    expect(coverage.complete).toBe(false)
  })
})
