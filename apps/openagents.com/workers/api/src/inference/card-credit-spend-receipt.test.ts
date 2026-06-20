import { describe, expect, test } from 'vitest'

import {
  type CardCreditSpendReceiptInput,
  assembleCardCreditSpendReceipt,
  cardCreditPurchaseEvidenceRef,
  cardCreditPurchaseLedgerKey,
  cardCreditSpendReceiptRef,
} from './card-credit-spend-receipt'
import { cardCreditGrantContextRef } from './card-credit-provenance'
import { inferenceChargeReceiptRef } from './metering-hook'
import { usdCreditGrantReceiptRef } from './usd-credit-bridge'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

// A consistent chain: $5.00 purchased, $5.00 granted (so granted msat is the
// exact floor conversion), and a partial inference spend that fits the grant.
const validInput = (
  overrides: Partial<{
    purchase: Partial<CardCreditSpendReceiptInput['purchase']>
    grant: Partial<CardCreditSpendReceiptInput['grant']>
    spend: Partial<CardCreditSpendReceiptInput['spend']>
  }> = {},
): CardCreditSpendReceiptInput => ({
  grant: {
    grantRef: 'req_grant_1',
    grantedCents: 500,
    grantedMsat: usdCentsToMsatFloor(500),
    ...overrides.grant,
  },
  purchase: {
    purchasedCents: 500,
    sessionId: 'cs_test_123',
    ...overrides.purchase,
  },
  spend: {
    requestId: 'chatcmpl_abc',
    servedModel: 'gemini-3.5-flash',
    spentMsat: 1_000,
    totalTokens: 42,
    ...overrides.spend,
  },
})

describe('cardCreditSpendReceiptRef and leg refs', () => {
  test('the chain refs match the refs the real ledger legs already emit', () => {
    expect(cardCreditSpendReceiptRef('cs_test_123')).toBe(
      'receipt.inference.card_credit_spend.cs_test_123',
    )
    expect(cardCreditPurchaseLedgerKey('cs_test_123')).toBe(
      'billing:stripe-checkout:cs_test_123',
    )
    expect(cardCreditPurchaseEvidenceRef('cs_test_123')).toBe(
      'evidence.stripe_checkout_paid.cs_test_123',
    )
  })
})

describe('assembleCardCreditSpendReceipt', () => {
  test('assembles a dereferenceable three-hop chain from real leg refs', () => {
    const result = assembleCardCreditSpendReceipt(validInput())
    expect(result.ok).toBe(true)
    if (!result.ok) return

    expect(result.receipt.receiptRef).toBe(
      cardCreditSpendReceiptRef('cs_test_123'),
    )
    expect(result.receipt.chain.map(step => step.step)).toEqual([
      'card_to_credit',
      'credit_to_msat',
      'msat_to_inference',
    ])
    // Each hop dereferences the SAME ref its real ledger write emits.
    expect(result.receipt.chain[0]?.receiptRef).toBe(
      cardCreditPurchaseLedgerKey('cs_test_123'),
    )
    expect(result.receipt.chain[0]?.evidenceRef).toBe(
      cardCreditPurchaseEvidenceRef('cs_test_123'),
    )
    expect(result.receipt.chain[1]?.receiptRef).toBe(
      usdCreditGrantReceiptRef('req_grant_1'),
    )
    expect(result.receipt.chain[2]?.receiptRef).toBe(
      inferenceChargeReceiptRef('chatcmpl_abc'),
    )
  })

  test('conservation residual is granted minus spent msat', () => {
    const result = assembleCardCreditSpendReceipt(validInput())
    if (!result.ok) throw new Error('expected ok')
    const granted = usdCentsToMsatFloor(500)
    expect(result.receipt.conservation.grantedMsat).toBe(granted)
    expect(result.receipt.conservation.spentMsat).toBe(1_000)
    expect(result.receipt.conservation.residualMsat).toBe(granted - 1_000)
  })

  test('rejects a blank leg ref', () => {
    const result = assembleCardCreditSpendReceipt(
      validInput({ spend: { requestId: '  ' } }),
    )
    expect(result).toMatchObject({ ok: false, reason: 'missing_ref' })
  })

  test('rejects a non-positive or fractional amount', () => {
    expect(
      assembleCardCreditSpendReceipt(validInput({ spend: { spentMsat: 0 } })),
    ).toMatchObject({ ok: false, reason: 'nonpositive_amount' })
    expect(
      assembleCardCreditSpendReceipt(
        validInput({ grant: { grantedMsat: 1.5 } }),
      ),
    ).toMatchObject({ ok: false, reason: 'nonpositive_amount' })
  })

  test('rejects a grant that exceeds the purchase', () => {
    const result = assembleCardCreditSpendReceipt(
      validInput({
        grant: { grantedCents: 600, grantedMsat: usdCentsToMsatFloor(600) },
      }),
    )
    expect(result).toMatchObject({ ok: false, reason: 'grant_exceeds_purchase' })
  })

  test('rejects a grant msat that does not match the shared conversion', () => {
    const result = assembleCardCreditSpendReceipt(
      validInput({ grant: { grantedMsat: usdCentsToMsatFloor(500) + 1 } }),
    )
    expect(result).toMatchObject({
      ok: false,
      reason: 'grant_conversion_mismatch',
    })
  })

  test('rejects a spend that exceeds the granted credit', () => {
    const result = assembleCardCreditSpendReceipt(
      validInput({ spend: { spentMsat: usdCentsToMsatFloor(500) + 1 } }),
    )
    expect(result).toMatchObject({ ok: false, reason: 'spend_exceeds_grant' })
  })

  test('allows a spend that exactly consumes the grant (zero residual)', () => {
    const granted = usdCentsToMsatFloor(500)
    const result = assembleCardCreditSpendReceipt(
      validInput({ spend: { spentMsat: granted } }),
    )
    if (!result.ok) throw new Error('expected ok')
    expect(result.receipt.conservation.residualMsat).toBe(0)
  })

  test('binds the grant to the purchase when the grant context_ref proves the session', () => {
    const contextRef = cardCreditGrantContextRef('cs_test_123')
    if (contextRef === undefined) throw new Error('expected a context ref')
    const result = assembleCardCreditSpendReceipt(
      validInput({ grant: { contextRef } }),
    )
    if (!result.ok) throw new Error('expected ok')
    // The credit_to_msat step now carries the dereferenceable provenance ref.
    expect(result.receipt.chain[1]?.evidenceRef).toBe(contextRef)
  })

  test('rejects a grant whose context_ref names a different session', () => {
    const contextRef = cardCreditGrantContextRef('cs_other_999')
    if (contextRef === undefined) throw new Error('expected a context ref')
    const result = assembleCardCreditSpendReceipt(
      validInput({ grant: { contextRef } }),
    )
    expect(result).toMatchObject({ ok: false, reason: 'provenance_mismatch' })
  })

  test('rejects a generic (non-card) grant context_ref as unprovable provenance', () => {
    const result = assembleCardCreditSpendReceipt(
      validInput({ grant: { contextRef: 'inference:usd-credit:user_42' } }),
    )
    expect(result).toMatchObject({ ok: false, reason: 'provenance_mismatch' })
  })

  test('omitting the grant context_ref leaves provenance caller-asserted (no evidence ref)', () => {
    const result = assembleCardCreditSpendReceipt(validInput())
    if (!result.ok) throw new Error('expected ok')
    expect(result.receipt.chain[1]?.evidenceRef).toBeUndefined()
  })
})
