import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION } from './public-projection-staleness'
import {
  SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS,
  SITE_REFERRAL_PAYOUT_PERCENT_BPS,
  SITE_REFERRAL_PAYOUT_POLICY_REF,
} from './site-referral-payout-ledger'
import {
  SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
  aggregateSiteReferralPayoutPublicProjection,
} from './site-referral-payout-public-projection'
import { handleSiteReferralPayoutsPublicApi } from './site-referral-payout-public-routes'

const NOW_ISO = '2026-06-19T18:00:00.000Z'

describe('aggregateSiteReferralPayoutPublicProjection', () => {
  test('empty ledger is an honest zero-settled projection with the wiring present', () => {
    const projection = aggregateSiteReferralPayoutPublicProjection([])

    expect(projection.kind).toBe('site_referral_payouts_public')
    expect(projection.publicSafe).toBe(true)
    expect(projection.ledgerWiredInSource).toBe(true)
    expect(projection.totalCurrentPayouts).toBe(0)
    expect(projection.settledCount).toBe(0)
    expect(projection.settledSats).toBe(0)
    expect(projection.schemaVersion).toBe(
      SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
    )
    expect(projection.staleness.composition).toBe('live_at_read')
    expect(projection.staleness.maxStalenessSeconds).toBe(0)
    expect(projection.staleness.contractVersion).toBe(
      PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
    )
    expect(projection.policy.policyRef).toBe(SITE_REFERRAL_PAYOUT_POLICY_REF)
    expect(projection.policy.percentBps).toBe(SITE_REFERRAL_PAYOUT_PERCENT_BPS)
    expect(projection.policy.maxEventSats).toBe(
      SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS,
    )
    // Every state present even at zero — no missing buckets.
    expect(projection.stateTotals.map(total => total.state).sort()).toEqual(
      [
        'approved',
        'dispatched',
        'eligible',
        'failed',
        'refused',
        'reversed',
        'settled',
      ].sort(),
    )
    expect(
      projection.stateTotals.every(
        total => total.count === 0 && total.totalSats === 0,
      ),
    ).toBe(true)
  })

  test('counts and sums current entries per state without settling anything', () => {
    const projection = aggregateSiteReferralPayoutPublicProjection([
      { amountSats: 50, state: 'eligible' },
      { amountSats: 50, state: 'eligible' },
      { amountSats: 30, state: 'approved' },
      { amountSats: 0, state: 'refused' },
    ])

    const eligible = projection.stateTotals.find(
      total => total.state === 'eligible',
    )
    const approved = projection.stateTotals.find(
      total => total.state === 'approved',
    )

    expect(projection.totalCurrentPayouts).toBe(4)
    expect(eligible).toEqual({ count: 2, state: 'eligible', totalSats: 100 })
    expect(approved).toEqual({ count: 1, state: 'approved', totalSats: 30 })
    // No real money has moved: settled stays zero even with eligibility present.
    expect(projection.settledCount).toBe(0)
    expect(projection.settledSats).toBe(0)
  })

  test('a settled entry surfaces in the real settled figures', () => {
    const projection = aggregateSiteReferralPayoutPublicProjection([
      { amountSats: 25, state: 'settled' },
      { amountSats: -25, state: 'reversed' },
    ])

    expect(projection.settledCount).toBe(1)
    expect(projection.settledSats).toBe(25)
    const reversed = projection.stateTotals.find(
      total => total.state === 'reversed',
    )
    expect(reversed).toEqual({ count: 1, state: 'reversed', totalSats: -25 })
  })

  test('no per-row identifier fields or payment material appear in the payload', () => {
    // Identifiers and payment material from a real ledger entry must never reach
    // the projection. The descriptive caveat/policy ref vocabulary (e.g.
    // "...no_referrer_or_referred_identifiers") is intentionally allowed; what
    // must be absent are the actual id FIELD keys and any payment-material
    // tokens that would only appear if a raw entry leaked.
    const projection = aggregateSiteReferralPayoutPublicProjection([
      { amountSats: 50, state: 'eligible' },
    ])
    const serialized = JSON.stringify(projection).toLowerCase()

    for (const bannedFieldKey of [
      '"referreruserid"',
      '"referreduserid"',
      '"referralattributionid"',
      '"referralsourceid"',
      '"referralinviteid"',
      '"payoutref"',
      '"qualifyingeventref"',
      '"idempotencykey"',
      '"evidencerefs"',
    ]) {
      expect(serialized).not.toContain(bannedFieldKey)
    }

    for (const bannedMaterial of ['lnbc', 'lntb', 'preimage', 'xprv']) {
      expect(serialized).not.toContain(bannedMaterial)
    }
  })
})

describe('handleSiteReferralPayoutsPublicApi', () => {
  test('GET returns the live-at-read projection with generatedAt', async () => {
    const response = await Effect.runPromise(
      handleSiteReferralPayoutsPublicApi(
        new Request('https://openagents.com/api/public/site-referral-payouts'),
        {
          nowIso: () => NOW_ISO,
          readCurrentStates: async () => [
            { amountSats: 50, state: 'eligible' },
          ],
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.generatedAt).toBe(NOW_ISO)
    expect(body.kind).toBe('site_referral_payouts_public')
    expect(body.settledCount).toBe(0)
    expect(body.totalCurrentPayouts).toBe(1)
    expect((body.staleness as Record<string, unknown>).composition).toBe(
      'live_at_read',
    )
  })

  test('non-GET is rejected', async () => {
    const response = await Effect.runPromise(
      handleSiteReferralPayoutsPublicApi(
        new Request(
          'https://openagents.com/api/public/site-referral-payouts',
          { method: 'POST' },
        ),
        { nowIso: () => NOW_ISO, readCurrentStates: async () => [] },
      ),
    )

    expect(response.status).toBe(405)
  })
})
