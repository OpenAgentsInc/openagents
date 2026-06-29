import { describe, expect, test } from 'vitest'

import { assertXClaimRewardSettlementEvidenceRefs } from './x-claim-reward-settlement-evidence'

describe('X claim reward settlement evidence pre-persistence gate', () => {
  test('accepts a well-formed public settlement evidence ref', () => {
    const gate = assertXClaimRewardSettlementEvidenceRefs([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
    ])

    expect(gate.ok).toBe(true)
    expect(gate.blockingReasonRefs).toEqual([])
    expect(gate.acceptedRefs).toEqual([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
    ])
    expect(gate.checks.every(check => check.ok)).toBe(true)
  })

  test('trims and dedupes accepted refs', () => {
    const gate = assertXClaimRewardSettlementEvidenceRefs([
      '  settlement_evidence.public.mdk_campaign_wallet.send_confirmed  ',
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
    ])

    expect(gate.ok).toBe(true)
    expect(gate.acceptedRefs).toEqual([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
    ])
  })

  test('blocks an empty ref list (whitespace only)', () => {
    const gate = assertXClaimRewardSettlementEvidenceRefs(['   ', ''])

    expect(gate.ok).toBe(false)
    expect(gate.acceptedRefs).toEqual([])
    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_settlement_requires_evidence',
    )
  })

  test('blocks refs that carry no public settlement evidence ref', () => {
    const gate = assertXClaimRewardSettlementEvidenceRefs([
      'receipt.public.x_claim.1',
    ])

    expect(gate.ok).toBe(false)
    expect(gate.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_settlement_missing_public_ref',
    )
  })

  test('blocks leaked payment material in any submitted ref', () => {
    const invoiceLeak = assertXClaimRewardSettlementEvidenceRefs([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
      'lnbc1000n1psomeinvoice',
    ])
    const offerLeak = assertXClaimRewardSettlementEvidenceRefs([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
      'lno1qqqqqqqqqqqqqqqqqqqqqqqqqqqqq',
    ])
    const addressLeak = assertXClaimRewardSettlementEvidenceRefs([
      'settlement_evidence.public.owner@getalby.com',
    ])
    const preimageLeak = assertXClaimRewardSettlementEvidenceRefs([
      'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
      'a'.repeat(64),
    ])

    for (const gate of [invoiceLeak, offerLeak, addressLeak, preimageLeak]) {
      expect(gate.ok).toBe(false)
      expect(gate.acceptedRefs).toEqual([])
      expect(gate.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_settlement_payment_material_leaked',
      )
    }
  })

  test('never echoes payment material in its serialized output', () => {
    const serialized = JSON.stringify(
      assertXClaimRewardSettlementEvidenceRefs([
        'settlement_evidence.public.mdk_campaign_wallet.send_confirmed',
        'lnbc1000n1psomeinvoice',
      ]),
    )

    expect(serialized).not.toContain('lnbc')
  })
})
