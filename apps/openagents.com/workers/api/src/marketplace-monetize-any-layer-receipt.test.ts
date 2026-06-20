import { describe, expect, test } from 'vitest'

import type { AccrueMonetizeLayerReferralResult } from './marketplace-monetize-any-layer-accrual'
import {
  type LayerMonetizationDefinition,
  buildLayerMonetizationDefinition,
  planLayerMonetizationAccrual,
} from './marketplace-monetize-any-layer'
import {
  MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA,
  MONETIZE_LAYER_RESALE_RECEIPT_MISSING_REF,
  buildMonetizeLayerResaleReceipt,
  monetizeLayerResaleReceiptProjection,
  monetizeLayerResaleReceiptRef,
} from './marketplace-monetize-any-layer-receipt'
import type { SiteReferralPayoutLedgerEntry } from './site-referral-payout-ledger'

const okDefinition = (
  overrides: Partial<Parameters<typeof buildLayerMonetizationDefinition>[0]> = {},
): LayerMonetizationDefinition => {
  const result = buildLayerMonetizationDefinition({
    offerId: 'offer_inference_resale',
    sellerRef: 'agent:seller',
    layer: 'inference',
    monetizationKind: 'agentic_work',
    unitPriceMsat: 1000,
    priceAsset: 'bitcoin',
    referralBps: 500,
    referrerRef: 'agent:referrer',
    createdAt: '2026-06-19T00:00:00.000Z',
    ...overrides,
  })
  if (!result.ok) {
    throw new Error(`expected ok definition: ${result.error.reason}`)
  }
  return result.definition
}

const disabledResult = (
  definition: LayerMonetizationDefinition,
  meteredSpendMsat: number,
): AccrueMonetizeLayerReferralResult => ({
  _tag: 'disabled',
  plan: planLayerMonetizationAccrual({ definition, meteredSpendMsat }),
})

const recordedEntry = (
  overrides: Partial<SiteReferralPayoutLedgerEntry> = {},
): SiteReferralPayoutLedgerEntry => ({
  amountSats: 5,
  archivedAt: null,
  caveatRefs: [],
  createdAt: '2026-06-19T00:00:00.000Z',
  evidenceRefs: [],
  id: 'entry_1',
  idempotencyKey: 'idem_1',
  payoutRef: 'payout.monetize_inference.evt_1',
  periodKey: '2026-06',
  policyRefs: [],
  previousEntryId: null,
  qualifyingAmountSats: 100,
  qualifyingEventKind: 'monetize_any_layer.agentic_work',
  qualifyingEventRef: 'event.monetize_inference.evt_1',
  referredUserId: 'user_referee',
  referralAttributionId: 'attr_1',
  referralInviteId: null,
  referralSourceId: 'src_1',
  referrerUserId: 'user_referrer',
  reversalOfEntryId: null,
  state: 'eligible',
  stateReasonRef: null,
  ...overrides,
})

describe('monetize-any-layer resale receipt (#5518)', () => {
  test('binds offer + spend + disabled outcome into one inert, unsettled receipt', () => {
    const definition = okDefinition()
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: 'evt_1',
      result: disabledResult(definition, 200_000),
    })
    expect(built.ok).toBe(true)
    if (!built.ok) return
    const { receipt } = built
    expect(receipt.schema).toBe(MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA)
    expect(receipt.promiseState).toBe('planned')
    expect(receipt.inert).toBe(true)
    expect(receipt.settled).toBe(false)
    expect(receipt.receiptRef).toBe(
      monetizeLayerResaleReceiptRef('inference', 'evt_1'),
    )
    expect(receipt.meteredSpendMsat).toBe(200_000)
    // 200_000 msat -> 200 whole sats fed to the sat-denominated ledger.
    expect(receipt.qualifyingAmountSats).toBe(200)
    expect(receipt.authorized).toBe(true)
    expect(receipt.referralOutcome).toEqual({ _tag: 'disabled' })
    // The receipt advances but does NOT clear the resale-receipt blocker.
    expect(receipt.unclearedBlockerRefs).toContain(
      MONETIZE_LAYER_RESALE_RECEIPT_MISSING_REF,
    )
  })

  test('surfaces the recorded ledger row refs + state for an accrued event', () => {
    const definition = okDefinition()
    const result: AccrueMonetizeLayerReferralResult = {
      _tag: 'accrued',
      plan: planLayerMonetizationAccrual({ definition, meteredSpendMsat: 100_000 }),
      accrual: { _tag: 'recorded', entry: recordedEntry() },
    }
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: 'evt_1',
      result,
    })
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.receipt.referralOutcome).toEqual({
      _tag: 'recorded',
      payoutRef: 'payout.monetize_inference.evt_1',
      qualifyingEventRef: 'event.monetize_inference.evt_1',
      ledgerState: 'eligible',
      referralAccrualSats: 5,
    })
    // Still inert + unsettled: an eligibility row is not a settled payout.
    expect(built.receipt.settled).toBe(false)
  })

  test('carries an unauthorized outcome for a blocked (subscription resale) plan', () => {
    const definition = okDefinition({
      monetizationKind: 'subscription_capacity_resale',
    })
    const plan = planLayerMonetizationAccrual({
      definition,
      meteredSpendMsat: 100_000,
      accountAuthMode: 'subscription',
    })
    expect(plan.authorized).toBe(false)
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: 'evt_blocked',
      result: { _tag: 'unauthorized', plan },
    })
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.receipt.authorized).toBe(false)
    expect(built.receipt.referralOutcome._tag).toBe('unauthorized')
    if (built.receipt.referralOutcome._tag === 'unauthorized') {
      expect(built.receipt.referralOutcome.blockerRefs.length).toBeGreaterThan(0)
    }
  })

  test('rejects an empty event id', () => {
    const definition = okDefinition()
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: '   ',
      result: disabledResult(definition, 100_000),
    })
    expect(built.ok).toBe(false)
  })

  test('rejects a result whose plan describes a different offer', () => {
    const definition = okDefinition()
    const other = okDefinition({ layer: 'sandbox', sellerRef: 'agent:other' })
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: 'evt_1',
      result: disabledResult(other, 100_000),
    })
    expect(built.ok).toBe(false)
  })

  test('projection drops amounts/keys and keeps the outcome tag + posture', () => {
    const definition = okDefinition()
    const built = buildMonetizeLayerResaleReceipt({
      definition,
      eventId: 'evt_1',
      result: disabledResult(definition, 200_000),
    })
    expect(built.ok).toBe(true)
    if (!built.ok) return
    const projection = monetizeLayerResaleReceiptProjection(built.receipt)
    expect(projection.referralOutcomeTag).toBe('disabled')
    expect(projection.settled).toBe(false)
    expect(projection.qualifyingAmountSats).toBe(200)
    expect(projection).not.toHaveProperty('meteredSpendMsat')
  })
})
