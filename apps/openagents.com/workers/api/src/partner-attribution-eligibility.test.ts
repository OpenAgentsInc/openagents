import { describe, expect, test } from 'vitest'

import { PARTNER_ATTRIBUTION_POLICY_REF } from './partner-attribution-policy'
import { type PartnerAgreement } from './partner-attribution-policy'
import { resolvePartnerPayoutEligibilityInput } from './partner-attribution-eligibility'
import { type PartnerQualifyingPaidEvent } from './partner-attribution-eligibility'

const EVENT_ISO = '2026-06-20T06:20:00.000Z'

const event = (
  overrides: Partial<PartnerQualifyingPaidEvent> = {},
): PartnerQualifyingPaidEvent => ({
  asset: 'usd',
  customerUserId: 'user_customer',
  eventIso: EVENT_ISO,
  idempotencyKey: 'partner_payout:evt_123',
  periodKey: '2026-06',
  qualifyingAmount: 10000,
  qualifyingEventKind: 'stripe.invoice.paid',
  qualifyingEventRef: 'evt.partner_payout.evt_123',
  ...overrides,
})

const agreement = (
  overrides: Partial<PartnerAgreement> & Pick<PartnerAgreement, 'role'>,
): PartnerAgreement => ({
  agreementRef: 'agreement.partner_a.v1',
  effectiveFromIso: '2026-01-01T00:00:00.000Z',
  effectiveUntilIso: null,
  partnerRef: 'partner.partner_a',
  partnerUserId: 'user_partner_a',
  ...overrides,
})

describe('resolvePartnerPayoutEligibilityInput', () => {
  test('no candidate agreements -> no_active_agreement (records nothing)', () => {
    expect(resolvePartnerPayoutEligibilityInput(event(), [])).toEqual({
      _tag: 'no_active_agreement',
    })
  })

  test('an active explicit agreement -> ledger-ready eligibility input', () => {
    const result = resolvePartnerPayoutEligibilityInput(event(), [
      agreement({ role: 'affiliate' }),
    ])

    expect(result).toEqual({
      _tag: 'eligible',
      agreementRef: 'agreement.partner_a.v1',
      policyRef: PARTNER_ATTRIBUTION_POLICY_REF,
      input: {
        asset: 'usd',
        beneficiaryUserId: 'user_customer',
        evidenceRefs: ['agreement.partner_a.v1'],
        idempotencyKey: 'partner_payout:evt_123',
        nowIso: EVENT_ISO,
        partnerRef: 'partner.partner_a',
        partnerRole: 'affiliate',
        partnerUserId: 'user_partner_a',
        periodKey: '2026-06',
        policyRefs: [PARTNER_ATTRIBUTION_POLICY_REF],
        qualifyingAmount: 10000,
        qualifyingEventKind: 'stripe.invoice.paid',
        qualifyingEventRef: 'evt.partner_payout.evt_123',
      },
    })
  })

  test('persists the attribution basis: agreement ref as evidence, policy ref', () => {
    const result = resolvePartnerPayoutEligibilityInput(event(), [
      agreement({ role: 'design_partner' }),
    ])

    if (result._tag !== 'eligible') {
      throw new Error('expected eligible')
    }

    expect(result.input.evidenceRefs).toEqual(['agreement.partner_a.v1'])
    expect(result.input.policyRefs).toEqual([PARTNER_ATTRIBUTION_POLICY_REF])
  })

  test('carries the qualifying event fields and asset through unchanged', () => {
    const result = resolvePartnerPayoutEligibilityInput(
      event({ asset: 'sats', qualifyingAmount: 250000, periodKey: '2026-07' }),
      [agreement({ role: 'design_partner' })],
    )

    expect(result._tag).toBe('eligible')

    if (result._tag !== 'eligible') {
      throw new Error('expected eligible')
    }

    expect(result.input.asset).toBe('sats')
    expect(result.input.qualifyingAmount).toBe(250000)
    expect(result.input.periodKey).toBe('2026-07')
    expect(result.input.partnerRole).toBe('design_partner')
  })

  test('design_partner outranks affiliate -> exactly one credited partner', () => {
    const result = resolvePartnerPayoutEligibilityInput(event(), [
      agreement({
        agreementRef: 'agreement.affiliate.v1',
        partnerRef: 'partner.affiliate',
        partnerUserId: 'user_affiliate',
        role: 'affiliate',
      }),
      agreement({
        agreementRef: 'agreement.design.v1',
        partnerRef: 'partner.design',
        partnerUserId: 'user_design',
        role: 'design_partner',
      }),
    ])

    expect(result._tag).toBe('eligible')

    if (result._tag !== 'eligible') {
      throw new Error('expected eligible')
    }

    expect(result.input.partnerRef).toBe('partner.design')
    expect(result.input.partnerRole).toBe('design_partner')
    expect(result.agreementRef).toBe('agreement.design.v1')
  })

  test('self-attribution short-circuits and records nothing', () => {
    const result = resolvePartnerPayoutEligibilityInput(
      event({ customerUserId: 'user_partner_a' }),
      [agreement({ role: 'affiliate' })],
    )

    expect(result).toEqual({
      _tag: 'self_attribution',
      partnerRef: 'partner.partner_a',
    })
  })

  test('an agreement that ended before the event records nothing', () => {
    const result = resolvePartnerPayoutEligibilityInput(event(), [
      agreement({
        effectiveUntilIso: '2026-06-01T00:00:00.000Z',
        role: 'affiliate',
      }),
    ])

    expect(result).toEqual({ _tag: 'no_active_agreement' })
  })
})
