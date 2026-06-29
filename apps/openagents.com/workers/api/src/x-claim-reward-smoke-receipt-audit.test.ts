import { describe, expect, test } from 'vitest'

import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import {
  auditXClaimRewardSmokeReceipt,
  buildXClaimRewardSmokeTransitionRequest,
} from './x-claim-reward-smoke-receipt-audit'

const settledReward = (
  overrides: Partial<XClaimRewardRecord> = {},
): XClaimRewardRecord => ({
  agentUserId: 'user_agent_1',
  amountSats: 1000,
  challengeId: 'x_challenge_1',
  claimId: 'agent_claim_1',
  createdAt: '2026-06-10T10:00:00.000Z',
  evidenceRefs: [
    'receipt.public.x_claim.1',
    'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
  ],
  id: 'x_claim_reward_1',
  ownerUserId: 'user_owner_1',
  receiptRef: 'x_claim_reward_receipt_x_claim_reward_1',
  state: 'settled',
  stateReasonRef: null,
  treasuryPaymentId: 'payment_secret_1',
  updatedAt: '2026-06-10T12:00:00.000Z',
  xAccountRef: 'x_account.public.owner_1',
  ...overrides,
})

describe('X claim reward smoke receipt audit', () => {
  test('passes a clean settled reward with public settlement evidence', () => {
    const audit = auditXClaimRewardSmokeReceipt(settledReward())

    expect(audit.ok).toBe(true)
    expect(audit.violationReasonRefs).toEqual([])
    expect(audit.checks.every(check => check.ok)).toBe(true)
    expect(audit.transitionReceiptSummary.settlementEvidenceRefs).toEqual([
      'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
    ])
  })

  test('blocks when the reward has not reached settled', () => {
    const audit = auditXClaimRewardSmokeReceipt(
      settledReward({ state: 'dispatched' }),
    )

    expect(audit.ok).toBe(false)
    expect(audit.violationReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_unexpected_state',
    )
  })

  test('blocks when the amount is not the bounded campaign reward', () => {
    const audit = auditXClaimRewardSmokeReceipt(
      settledReward({ amountSats: 5000 }),
    )

    expect(audit.violationReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_amount_mismatch',
    )
  })

  test('blocks a malformed receipt ref', () => {
    const audit = auditXClaimRewardSmokeReceipt(
      settledReward({ receiptRef: 'lnbc10n1pjmadeup' }),
    )

    expect(audit.violationReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_receipt_ref_malformed',
    )
  })

  test('blocks when no public settlement evidence ref is present', () => {
    const audit = auditXClaimRewardSmokeReceipt(
      settledReward({ evidenceRefs: ['receipt.public.x_claim.1'] }),
    )

    expect(audit.violationReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_settlement_evidence_missing',
    )
    expect(audit.transitionReceiptSummary.settlementEvidenceRefs).toEqual([])
  })

  test('blocks a leaked lightning invoice in evidence refs', () => {
    const audit = auditXClaimRewardSmokeReceipt(
      settledReward({
        evidenceRefs: [
          'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
          'lnbc1000n1psomeinvoicepayload',
        ],
      }),
    )

    expect(audit.violationReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_payment_material_leaked',
    )
  })

  test('blocks a leaked BOLT12 offer, lightning address, or preimage', () => {
    const offerLeak = auditXClaimRewardSmokeReceipt(
      settledReward({
        evidenceRefs: [
          'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
          'lno1qqqqqqqqqqqqqqqqqqqqqqqqqqqqq',
        ],
      }),
    )
    const addressLeak = auditXClaimRewardSmokeReceipt(
      settledReward({
        stateReasonRef: 'paid owner@getalby.com',
      }),
    )
    const preimageLeak = auditXClaimRewardSmokeReceipt(
      settledReward({
        evidenceRefs: [
          'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
          'a'.repeat(64),
        ],
      }),
    )

    expect(offerLeak.ok).toBe(false)
    expect(addressLeak.ok).toBe(false)
    expect(preimageLeak.ok).toBe(false)
  })

  test('transition receipt summary carries no payment id or destination', () => {
    const serialized = JSON.stringify(
      auditXClaimRewardSmokeReceipt(settledReward()).transitionReceiptSummary,
    )

    expect(serialized).not.toContain('payment_secret_1')
    expect(serialized).not.toContain('lnbc')
  })
})

describe('X claim reward smoke transition request', () => {
  test('builds a public-safe green-flip proposal from a clean settled reward', () => {
    const proposal = buildXClaimRewardSmokeTransitionRequest(settledReward())

    expect(proposal.ready).toBe(true)
    expect(proposal.blockingReasonRefs).toEqual([])
    expect(proposal.transitionRequest).toEqual({
      evidenceRefs: [
        'x_claim_reward_receipt_x_claim_reward_1',
        'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
      ],
      promiseId: 'agents.x_claim_reward.v1',
      toState: 'green',
    })
  })

  test('withholds a proposal until the post-settlement audit passes', () => {
    const proposal = buildXClaimRewardSmokeTransitionRequest(
      settledReward({ state: 'dispatched' }),
    )

    expect(proposal.ready).toBe(false)
    expect(proposal.transitionRequest).toBeNull()
    expect(proposal.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_unexpected_state',
    )
  })

  test('refuses to propose when payment material leaks into evidence', () => {
    const proposal = buildXClaimRewardSmokeTransitionRequest(
      settledReward({
        evidenceRefs: [
          'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
          'lnbc1000n1psomeinvoicepayload',
        ],
      }),
    )

    expect(proposal.ready).toBe(false)
    expect(proposal.transitionRequest).toBeNull()
    expect(proposal.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_payment_material_leaked',
    )
  })

  test('the proposed transition request leaks no payment material', () => {
    const serialized = JSON.stringify(
      buildXClaimRewardSmokeTransitionRequest(settledReward()),
    )

    expect(serialized).not.toContain('payment_secret_1')
    expect(serialized).not.toContain('lnbc')
  })
})
