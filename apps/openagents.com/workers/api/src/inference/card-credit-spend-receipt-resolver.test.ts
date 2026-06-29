import { describe, expect, test } from 'vitest'

import {
  type CardCreditPurchaseLeg,
  type CreditToMsatGrantLeg,
  type InferenceSpendLeg,
  cardCreditSpendReceiptRef,
} from './card-credit-spend-receipt'
import {
  type CardCreditSpendLegReaders,
  resolveCardCreditSpendReceipt,
} from './card-credit-spend-receipt-resolver'
import { cardCreditGrantContextRef } from './card-credit-provenance'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

const SESSION = 'cs_test_resolver_1'

// cardCreditGrantContextRef returns string | undefined (undefined for an
// invalid session id); these fixtures use valid ids, so narrow once here.
const grantContextRef = (sessionId: string): string => {
  const ref = cardCreditGrantContextRef(sessionId)
  if (ref === undefined) {
    throw new Error(`invalid test session id: ${sessionId}`)
  }
  return ref
}

const purchaseLeg: CardCreditPurchaseLeg = {
  purchasedCents: 500,
  sessionId: SESSION,
}

const grantLeg: CreditToMsatGrantLeg = {
  contextRef: grantContextRef(SESSION),
  grantRef: 'req_grant_1',
  grantedCents: 500,
  grantedMsat: usdCentsToMsatFloor(500),
}

const spendLeg: InferenceSpendLeg = {
  requestId: 'chatcmpl_abc',
  servedModel: 'gemini-3.5-flash',
  spentMsat: 1_000,
  totalTokens: 42,
}

// Build readers from optional fixed legs; a missing leg resolves to undefined
// (that hop has not settled yet).
const readers = (
  legs: Partial<{
    purchase: CardCreditPurchaseLeg
    grant: CreditToMsatGrantLeg
    spend: InferenceSpendLeg
  }> = {},
): CardCreditSpendLegReaders => ({
  readGrantLeg: () => Promise.resolve(legs.grant),
  readPurchaseLeg: () => Promise.resolve(legs.purchase),
  readSpendLeg: () => Promise.resolve(legs.spend),
})

describe('resolveCardCreditSpendReceipt', () => {
  test('a blank session id resolves to blank_session, reading no legs', async () => {
    let reads = 0
    const result = await resolveCardCreditSpendReceipt('   ', {
      readGrantLeg: () => {
        reads += 1
        return Promise.resolve(grantLeg)
      },
      readPurchaseLeg: () => {
        reads += 1
        return Promise.resolve(purchaseLeg)
      },
      readSpendLeg: () => {
        reads += 1
        return Promise.resolve(spendLeg)
      },
    })
    expect(result).toEqual({ ok: false, status: 'blank_session' })
    expect(reads).toBe(0)
  })

  test('resolves the full dereferenceable receipt when all three legs settled', async () => {
    const result = await resolveCardCreditSpendReceipt(
      SESSION,
      readers({ grant: grantLeg, purchase: purchaseLeg, spend: spendLeg }),
    )
    expect(result.ok).toBe(true)
    if (!result.ok) {
      throw new Error('expected ok resolution')
    }
    expect(result.receipt.receiptRef).toBe(cardCreditSpendReceiptRef(SESSION))
    expect(result.receipt.sessionId).toBe(SESSION)
    expect(result.receipt.conservation.residualMsat).toBe(
      grantLeg.grantedMsat - spendLeg.spentMsat,
    )
    // Provenance evidence flows through the resolver to the credit_to_msat hop.
    const grantStep = result.receipt.chain.find(
      (step) => step.step === 'credit_to_msat',
    )
    expect(grantStep?.evidenceRef).toBe(grantContextRef(SESSION))
  })

  test('reports pending:purchase before any card purchase has settled', async () => {
    const result = await resolveCardCreditSpendReceipt(SESSION, readers({}))
    expect(result).toEqual({ missing: 'purchase', ok: false, status: 'pending' })
  })

  test('reports pending:grant when the bridge has not granted msat yet', async () => {
    const result = await resolveCardCreditSpendReceipt(
      SESSION,
      readers({ purchase: purchaseLeg }),
    )
    expect(result).toEqual({ missing: 'grant', ok: false, status: 'pending' })
  })

  test('reports pending:spend when no metered inference has settled yet', async () => {
    const result = await resolveCardCreditSpendReceipt(
      SESSION,
      readers({ grant: grantLeg, purchase: purchaseLeg }),
    )
    expect(result).toEqual({ missing: 'spend', ok: false, status: 'pending' })
  })

  test('short-circuits: a missing grant never reads the spend leg', async () => {
    let spendReads = 0
    const result = await resolveCardCreditSpendReceipt(SESSION, {
      readGrantLeg: () => Promise.resolve(undefined),
      readPurchaseLeg: () => Promise.resolve(purchaseLeg),
      readSpendLeg: () => {
        spendReads += 1
        return Promise.resolve(spendLeg)
      },
    })
    expect(result).toEqual({ missing: 'grant', ok: false, status: 'pending' })
    expect(spendReads).toBe(0)
  })

  test('a complete-but-inconsistent chain resolves to invalid, not ok', async () => {
    // Spend exceeds the granted msat: legs are all present but the chain lies.
    const result = await resolveCardCreditSpendReceipt(
      SESSION,
      readers({
        grant: grantLeg,
        purchase: purchaseLeg,
        spend: { ...spendLeg, spentMsat: grantLeg.grantedMsat + 1 },
      }),
    )
    expect(result.ok).toBe(false)
    if (result.ok || result.status !== 'invalid') {
      throw new Error('expected invalid resolution')
    }
    expect(result.reason).toBe('spend_exceeds_grant')
  })

  test('a grant whose context_ref binds a different session is invalid', async () => {
    const result = await resolveCardCreditSpendReceipt(
      SESSION,
      readers({
        grant: { ...grantLeg, contextRef: grantContextRef('cs_other') },
        purchase: purchaseLeg,
        spend: spendLeg,
      }),
    )
    expect(result.ok).toBe(false)
    if (result.ok || result.status !== 'invalid') {
      throw new Error('expected invalid resolution')
    }
    expect(result.reason).toBe('provenance_mismatch')
  })
})
