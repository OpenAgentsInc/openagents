import { describe, expect, test } from 'vitest'

import {
  PARTNER_ATTRIBUTION_ELIGIBLE_ROLES,
  PARTNER_ATTRIBUTION_POLICY_REF,
  type PartnerAgreement,
  type PartnerAgreementSeed,
  assessPartnerAgreementSeed,
  decidePartnerAttribution,
} from './partner-attribution-policy'

const EVENT_ISO = '2026-06-20T06:20:00.000Z'

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

describe('decidePartnerAttribution', () => {
  test('no candidate agreements -> no_active_agreement (no inferred fallback)', () => {
    expect(
      decidePartnerAttribution(
        { customerUserId: 'user_customer', eventIso: EVENT_ISO },
        [],
      ),
    ).toEqual({ _tag: 'no_active_agreement' })
  })

  test('an active explicit agreement is attributed with the policy ref', () => {
    const decision = decidePartnerAttribution(
      { customerUserId: 'user_customer', eventIso: EVENT_ISO },
      [agreement({ role: 'affiliate' })],
    )

    expect(decision).toEqual({
      _tag: 'attributed',
      agreementRef: 'agreement.partner_a.v1',
      partnerRef: 'partner.partner_a',
      partnerRole: 'affiliate',
      partnerUserId: 'user_partner_a',
      policyRef: PARTNER_ATTRIBUTION_POLICY_REF,
    })
  })

  test('referral role is excluded here to prevent cross-rail double-pay', () => {
    expect(PARTNER_ATTRIBUTION_ELIGIBLE_ROLES).not.toContain('referral')
    expect(
      decidePartnerAttribution(
        { customerUserId: 'user_customer', eventIso: EVENT_ISO },
        [agreement({ role: 'referral' as PartnerAgreement['role'] })],
      ),
    ).toEqual({ _tag: 'no_active_agreement' })
  })

  test('design_partner outranks affiliate when both cover the customer', () => {
    const decision = decidePartnerAttribution(
      { customerUserId: 'user_customer', eventIso: EVENT_ISO },
      [
        agreement({
          agreementRef: 'agreement.affiliate.v1',
          partnerRef: 'partner.affiliate',
          partnerUserId: 'user_affiliate',
          role: 'affiliate',
        }),
        agreement({
          agreementRef: 'agreement.design_partner.v1',
          partnerRef: 'partner.design',
          partnerUserId: 'user_design',
          role: 'design_partner',
        }),
      ],
    )

    expect(decision).toMatchObject({
      _tag: 'attributed',
      partnerRole: 'design_partner',
      partnerRef: 'partner.design',
    })
  })

  test('within a role, the earliest-effective agreement wins deterministically', () => {
    const decision = decidePartnerAttribution(
      { customerUserId: 'user_customer', eventIso: EVENT_ISO },
      [
        agreement({
          agreementRef: 'agreement.late.v1',
          effectiveFromIso: '2026-05-01T00:00:00.000Z',
          partnerRef: 'partner.late',
          partnerUserId: 'user_late',
          role: 'affiliate',
        }),
        agreement({
          agreementRef: 'agreement.early.v1',
          effectiveFromIso: '2026-02-01T00:00:00.000Z',
          partnerRef: 'partner.early',
          partnerUserId: 'user_early',
          role: 'affiliate',
        }),
      ],
    )

    expect(decision).toMatchObject({
      _tag: 'attributed',
      partnerRef: 'partner.early',
    })
  })

  test('an agreement effective after the event does not attribute', () => {
    expect(
      decidePartnerAttribution(
        { customerUserId: 'user_customer', eventIso: EVENT_ISO },
        [
          agreement({
            effectiveFromIso: '2026-07-01T00:00:00.000Z',
            role: 'affiliate',
          }),
        ],
      ),
    ).toEqual({ _tag: 'no_active_agreement' })
  })

  test('an expired agreement (event at/after effectiveUntil) does not attribute', () => {
    expect(
      decidePartnerAttribution(
        { customerUserId: 'user_customer', eventIso: EVENT_ISO },
        [
          agreement({
            effectiveUntilIso: '2026-06-01T00:00:00.000Z',
            role: 'design_partner',
          }),
        ],
      ),
    ).toEqual({ _tag: 'no_active_agreement' })
  })

  test('a partner cannot be attributed payout for their own purchase', () => {
    expect(
      decidePartnerAttribution(
        { customerUserId: 'user_partner_a', eventIso: EVENT_ISO },
        [agreement({ role: 'design_partner' })],
      ),
    ).toEqual({ _tag: 'self_attribution', partnerRef: 'partner.partner_a' })
  })
})

describe('assessPartnerAgreementSeed', () => {
  const seed = (
    overrides: Partial<PartnerAgreementSeed> &
      Pick<PartnerAgreementSeed, 'role'>,
  ): PartnerAgreementSeed => ({
    customerUserId: 'user_customer',
    effectiveFromIso: '2026-01-01T00:00:00.000Z',
    effectiveUntilIso: null,
    partnerUserId: 'user_partner_a',
    ...overrides,
  })

  test('an attributable role with a consistent open-ended window is seedable', () => {
    expect(assessPartnerAgreementSeed(seed({ role: 'design_partner' }))).toEqual(
      { _tag: 'seedable' },
    )
  })

  test('a non-attributable role (referral) is rejected at the write boundary', () => {
    expect(
      assessPartnerAgreementSeed(
        seed({ role: 'referral' as PartnerAgreementSeed['role'] }),
      ),
    ).toMatchObject({ _tag: 'rejected' })
  })

  test('a self-agreement (partner == customer) is rejected', () => {
    expect(
      assessPartnerAgreementSeed(
        seed({ customerUserId: 'user_partner_a', role: 'affiliate' }),
      ),
    ).toMatchObject({ _tag: 'rejected' })
  })

  test('an inverted window (until <= from) is rejected', () => {
    expect(
      assessPartnerAgreementSeed(
        seed({
          effectiveFromIso: '2026-06-01T00:00:00.000Z',
          effectiveUntilIso: '2026-01-01T00:00:00.000Z',
          role: 'affiliate',
        }),
      ),
    ).toMatchObject({ _tag: 'rejected' })
  })

  test('an unparseable effectiveFromIso is rejected', () => {
    expect(
      assessPartnerAgreementSeed(
        seed({ effectiveFromIso: 'not-a-date', role: 'design_partner' }),
      ),
    ).toMatchObject({ _tag: 'rejected' })
  })
})
