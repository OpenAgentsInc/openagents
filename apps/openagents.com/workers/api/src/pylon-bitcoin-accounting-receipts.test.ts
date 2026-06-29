import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonBitcoinAccountingReceiptProjection,
  PylonBitcoinAccountingReceiptUnsafe,
  examplePylonBitcoinAccountingReceipt,
  projectPylonBitcoinAccountingReceipt,
  pylonBitcoinAccountingProjectionHasPrivateMaterial,
} from './pylon-bitcoin-accounting-receipts'

const nowIso = '2026-06-06T21:55:00.000Z'

describe('Pylon Bitcoin accounting receipts', () => {
  test('projects settled accepted-work accounting with public-safe bitcoin amount', () => {
    const publicProjection = projectPylonBitcoinAccountingReceipt(
      examplePylonBitcoinAccountingReceipt(),
      'public',
      nowIso,
    )
    const operatorProjection = projectPylonBitcoinAccountingReceipt(
      examplePylonBitcoinAccountingReceipt(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonBitcoinAccountingReceiptProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      bitcoinAmountDisplay: '0.00001500 bitcoin (1,500 sats)',
      bitcoinAmountSats: 1500,
      buyerPaymentEvidencePresent: true,
      buyerPaymentEvidenceRefs: [],
      payoutConfirmationClaimAllowed: true,
      payoutDispatchClaimAllowed: true,
      payoutEligibilityClaimAllowed: true,
      payoutVerificationClaimAllowed: true,
      providerRef: 'provider.pylon_public_demo',
      rewardIntentClaimAllowed: true,
      settlementClaimAllowed: true,
      state: 'settled',
      stateLabel: 'Settled',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(operatorProjection.buyerPaymentEvidenceRefs).toEqual([
      'buyer_payment_evidence.site_order_otec',
    ])
    expect(JSON.stringify(publicProjection)).not.toContain(
      '2026-06-06T21:50:00.000Z',
    )
    expect(pylonBitcoinAccountingProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('hides bitcoin amount without a public-safe amount receipt', () => {
    const projection = projectPylonBitcoinAccountingReceipt({
      ...examplePylonBitcoinAccountingReceipt(),
      amountReceiptRefs: [],
    }, 'public', nowIso)

    expect(projection.bitcoinAmountDisplay).toBeNull()
    expect(projection.bitcoinAmountSats).toBeNull()
  })

  test('keeps reward intent, payout eligibility, payout dispatch, verification, and settlement separate', () => {
    const base = examplePylonBitcoinAccountingReceipt()
    const rewardIntent = projectPylonBitcoinAccountingReceipt({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      state: 'accepted_work_reward_intent',
    }, 'customer', nowIso)
    const eligible = projectPylonBitcoinAccountingReceipt({
      ...base,
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      state: 'payout_eligible',
    }, 'customer', nowIso)
    const dispatched = projectPylonBitcoinAccountingReceipt({
      ...base,
      payoutConfirmationRefs: [],
      payoutVerificationRefs: [],
      settlementRefs: [],
      state: 'payout_dispatched',
    }, 'customer', nowIso)

    expect(rewardIntent.rewardIntentClaimAllowed).toBe(true)
    expect(rewardIntent.payoutEligibilityClaimAllowed).toBe(false)
    expect(rewardIntent.payoutDispatchClaimAllowed).toBe(false)
    expect(rewardIntent.settlementClaimAllowed).toBe(false)
    expect(eligible.rewardIntentClaimAllowed).toBe(true)
    expect(eligible.payoutEligibilityClaimAllowed).toBe(true)
    expect(eligible.payoutDispatchClaimAllowed).toBe(false)
    expect(eligible.settlementClaimAllowed).toBe(false)
    expect(dispatched.payoutDispatchClaimAllowed).toBe(true)
    expect(dispatched.payoutVerificationClaimAllowed).toBe(false)
    expect(dispatched.settlementClaimAllowed).toBe(false)
  })

  test('requires evidence as accounting state advances', () => {
    const base = examplePylonBitcoinAccountingReceipt()

    expect(() =>
      projectPylonBitcoinAccountingReceipt({
        ...base,
        rewardIntentRefs: [],
        state: 'accepted_work_reward_intent',
      }, 'operator', nowIso),
    ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
    expect(() =>
      projectPylonBitcoinAccountingReceipt({
        ...base,
        payoutEligibilityRefs: [],
        state: 'payout_eligible',
      }, 'operator', nowIso),
    ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
    expect(() =>
      projectPylonBitcoinAccountingReceipt({
        ...base,
        payoutDispatchRefs: [],
        state: 'payout_dispatched',
      }, 'operator', nowIso),
    ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
    expect(() =>
      projectPylonBitcoinAccountingReceipt({
        ...base,
        payoutVerificationRefs: [],
        state: 'payout_verified',
      }, 'operator', nowIso),
    ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
    expect(() =>
      projectPylonBitcoinAccountingReceipt({
        ...base,
        settlementRefs: [],
        state: 'settled',
      }, 'operator', nowIso),
    ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
  })

  test('rejects raw invoices, preimages, payment hashes, wallet state, payout targets, private keys, mnemonics, provider tokens, and customer data', () => {
    const base = examplePylonBitcoinAccountingReceipt()

    for (const record of [
      { ...base, evidenceRefs: ['invoice.lnbc123'] },
      { ...base, evidenceRefs: ['payment_preimage.secret'] },
      { ...base, evidenceRefs: ['payment_hash.raw_hash'] },
      { ...base, settlementRefs: ['wallet_state.local_node'] },
      { ...base, payoutDispatchRefs: ['payout_target.raw_node'] },
      { ...base, evidenceRefs: ['private_key.local'] },
      { ...base, evidenceRefs: ['mnemonic.local'] },
      { ...base, evidenceRefs: ['provider_token.codex'] },
      { ...base, caveatRefs: ['customer_email_ben@example.com'] },
    ]) {
      expect(() =>
        projectPylonBitcoinAccountingReceipt(record, 'operator', nowIso),
      ).toThrow(PylonBitcoinAccountingReceiptUnsafe)
    }
  })
})
