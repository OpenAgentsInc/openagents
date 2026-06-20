import { describe, expect, test } from 'vitest'

import { type PartnerAgreement } from './partner-attribution-policy'
import {
  type CreatePartnerPayoutEligibilityInput,
  type PartnerPayoutLedgerEntry,
  createPartnerPayoutEligibility,
} from './partner-payout-ledger'
import {
  type PartnerAgreementReader,
  recordPartnerPayoutForPaidEvent,
} from './partner-payout-feed'
import { buildStripeCheckoutPartnerPayoutEvent } from './stripe-billing'

// Locks the Stripe-checkout -> partner-payout-feed contract that the billing
// fulfillment path depends on (autopilot_sites.partner_payout_ledger.v1). The
// builder is pure, so we exercise it without a live Stripe client or D1, then
// run the produced event through the same feed the wiring calls.

const checkout = {
  amountCents: 10000,
  nowIso: '2026-06-20T12:00:00.000Z',
  sessionId: 'cs_test_123',
  userId: 'github:buyer',
}

const coveringAgreement: PartnerAgreement = {
  agreementRef: 'partner_agreement_acme',
  effectiveFromIso: '2026-01-01T00:00:00.000Z',
  effectiveUntilIso: null,
  partnerRef: 'design_partner_acme',
  partnerUserId: 'github:acme_agency',
  role: 'design_partner',
}

const stubReader =
  (agreements: ReadonlyArray<PartnerAgreement>): PartnerAgreementReader =>
  () =>
    Promise.resolve(agreements)

const capturingCreate = (
  captured: Array<CreatePartnerPayoutEligibilityInput>,
): typeof createPartnerPayoutEligibility =>
  (_db, input) => {
    captured.push(input)

    const entry: PartnerPayoutLedgerEntry = {
      amount: 2000,
      archivedAt: null,
      asset: input.asset,
      beneficiaryUserId: input.beneficiaryUserId ?? null,
      caveatRefs: [],
      createdAt: input.nowIso,
      evidenceRefs: [input.qualifyingEventRef],
      id: 'partner_payout_entry_test',
      idempotencyKey: input.idempotencyKey,
      partnerRef: input.partnerRef,
      partnerRole: input.partnerRole,
      partnerUserId: input.partnerUserId,
      payoutRef:
        input.payoutRef ??
        `partner_payout_${input.partnerRole}_${input.partnerRef}`,
      periodKey: input.periodKey,
      policyRefs: [],
      previousEntryId: null,
      qualifyingAmount: input.qualifyingAmount,
      qualifyingEventKind: input.qualifyingEventKind,
      qualifyingEventRef: input.qualifyingEventRef,
      reversalOfEntryId: null,
      state: 'eligible',
      stateReasonRef: null,
    }

    return Promise.resolve(entry)
  }

const unusedDb = {} as D1Database

describe('buildStripeCheckoutPartnerPayoutEvent', () => {
  test('maps a fulfilled USD checkout onto a USD qualifying event', () => {
    const event = buildStripeCheckoutPartnerPayoutEvent(checkout)

    expect(event).toEqual({
      asset: 'usd',
      customerUserId: 'github:buyer',
      eventIso: '2026-06-20T12:00:00.000Z',
      idempotencyKey: 'partner_payout.stripe_checkout.cs_test_123',
      periodKey: '2026-06',
      qualifyingAmount: 10000,
      qualifyingEventKind: 'stripe_credit_purchase',
      qualifyingEventRef: 'evidence.stripe_checkout_paid.cs_test_123',
    })
  })

  test('derives a deterministic per-session idempotency key', () => {
    const a = buildStripeCheckoutPartnerPayoutEvent(checkout)
    const b = buildStripeCheckoutPartnerPayoutEvent(checkout)

    expect(a.idempotencyKey).toBe(b.idempotencyKey)
  })
})

describe('stripe checkout -> partner payout feed', () => {
  test('records nothing when no explicit agreement covers the buyer', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(
      unusedDb,
      buildStripeCheckoutPartnerPayoutEvent(checkout),
      { createEligibility: capturingCreate(captured), readAgreements: stubReader([]) },
    )

    expect(result._tag).toBe('no_active_agreement')
    expect(captured).toHaveLength(0)
  })

  test('records a USD eligibility when an explicit agreement covers the buyer', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(
      unusedDb,
      buildStripeCheckoutPartnerPayoutEvent(checkout),
      {
        createEligibility: capturingCreate(captured),
        readAgreements: stubReader([coveringAgreement]),
      },
    )

    expect(result).toMatchObject({
      _tag: 'recorded',
      agreementRef: 'partner_agreement_acme',
      policyRef: 'policy.partner_attribution.v1',
    })
    expect(captured[0]).toMatchObject({
      asset: 'usd',
      beneficiaryUserId: 'github:buyer',
      evidenceRefs: ['partner_agreement_acme'],
      idempotencyKey: 'partner_payout.stripe_checkout.cs_test_123',
      partnerRef: 'design_partner_acme',
      partnerRole: 'design_partner',
      qualifyingAmount: 10000,
      qualifyingEventRef: 'evidence.stripe_checkout_paid.cs_test_123',
    })
  })

  test('records nothing when the buyer is the partner (self-attribution)', async () => {
    const captured: Array<CreatePartnerPayoutEligibilityInput> = []
    const result = await recordPartnerPayoutForPaidEvent(
      unusedDb,
      buildStripeCheckoutPartnerPayoutEvent({
        ...checkout,
        userId: 'github:acme_agency',
      }),
      {
        createEligibility: capturingCreate(captured),
        readAgreements: stubReader([coveringAgreement]),
      },
    )

    expect(result._tag).toBe('self_attribution')
    expect(captured).toHaveLength(0)
  })
})
