import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PARTNER_PAYOUT_POLICY_REF,
  PARTNER_PAYOUT_ROLE_POLICY,
} from './partner-payout-ledger'
import {
  PARTNER_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
  aggregatePartnerPayoutPublicProjection,
} from './partner-payout-public-projection'
import { handlePartnerPayoutsPublicApi } from './partner-payout-public-routes'
import { PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION } from './public-projection-staleness'

const NOW_ISO = '2026-06-20T06:20:00.000Z'

describe('aggregatePartnerPayoutPublicProjection', () => {
  test('empty ledger is an honest zero-settled public projection', () => {
    const projection = aggregatePartnerPayoutPublicProjection([])

    expect(projection.kind).toBe('partner_payouts_public')
    expect(projection.publicSafe).toBe(true)
    expect(projection.ledgerWiredInSource).toBe(true)
    expect(projection.operatorRoutesWiredInSource).toBe(true)
    expect(projection.partnerProjectionApiWiredInSource).toBe(true)
    expect(projection.totalCurrentPayouts).toBe(0)
    expect(projection.settledCount).toBe(0)
    expect(projection.settledSats).toBe(0)
    expect(projection.schemaVersion).toBe(
      PARTNER_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
    )
    expect(projection.staleness.composition).toBe('live_at_read')
    expect(projection.staleness.maxStalenessSeconds).toBe(0)
    expect(projection.staleness.contractVersion).toBe(
      PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
    )
    expect(projection.policy.policyRef).toBe(PARTNER_PAYOUT_POLICY_REF)
    expect(
      projection.policy.rolePolicies.find(
        role => role.partnerRole === 'referral',
      )?.percentBps,
    ).toBe(PARTNER_PAYOUT_ROLE_POLICY.referral.percentBps)
    expect(projection.blockerRefs).toEqual([
      'blocker.product_promises.partner_first_real_payout_pending',
    ])
    expect(
      projection.blockerRefs.includes(
        'blocker.product_promises.partner_payout_settlement_not_wired',
      ),
    ).toBe(false)
    expect(
      projection.blockerRefs.includes(
        'blocker.product_promises.partner_projection_api_missing',
      ),
    ).toBe(false)
    expect(
      projection.blockerRefs.includes(
        'blocker.product_promises.partner_attribution_policy_missing',
      ),
    ).toBe(false)
  })

  test('counts current states by state, role, and asset', () => {
    const projection = aggregatePartnerPayoutPublicProjection([
      {
        amount: 10,
        asset: 'sats',
        partnerRole: 'referral',
        state: 'eligible',
      },
      {
        amount: 20,
        asset: 'sats',
        partnerRole: 'referral',
        state: 'settled',
      },
      {
        amount: 150,
        asset: 'usd',
        partnerRole: 'design_partner',
        state: 'approved',
      },
      {
        amount: -5,
        asset: 'credits',
        partnerRole: 'affiliate',
        state: 'reversed',
      },
    ])

    expect(projection.totalCurrentPayouts).toBe(4)
    expect(
      projection.stateTotals.find(total => total.state === 'settled'),
    ).toEqual({ count: 1, state: 'settled', totalAmount: 20 })
    expect(
      projection.roleTotals.find(total => total.partnerRole === 'referral'),
    ).toEqual({ count: 2, partnerRole: 'referral', totalAmount: 30 })
    expect(
      projection.assetTotals.find(total => total.asset === 'sats'),
    ).toEqual({
      asset: 'sats',
      count: 2,
      settledAmount: 20,
      totalAmount: 30,
    })
    expect(projection.settledCount).toBe(1)
    expect(projection.settledSats).toBe(20)
  })

  test('no per-row identifiers or payment material appear in the payload', () => {
    const projection = aggregatePartnerPayoutPublicProjection([
      {
        amount: 50,
        asset: 'sats',
        partnerRole: 'affiliate',
        state: 'eligible',
      },
    ])
    const serialized = JSON.stringify(projection).toLowerCase()

    for (const bannedFieldKey of [
      '"partnerref"',
      '"partneruserid"',
      '"beneficiaryuserid"',
      '"payoutref"',
      '"qualifyingeventref"',
      '"idempotencykey"',
      '"evidencerefs"',
      '"currententryid"',
    ]) {
      expect(serialized).not.toContain(bannedFieldKey)
    }

    for (const bannedMaterial of ['lnbc', 'lntb', 'preimage', 'xprv']) {
      expect(serialized).not.toContain(bannedMaterial)
    }
  })
})

describe('handlePartnerPayoutsPublicApi', () => {
  test('GET returns the live-at-read projection with generatedAt', async () => {
    const response = await Effect.runPromise(
      handlePartnerPayoutsPublicApi(
        new Request('https://openagents.com/api/public/partner-payouts'),
        {
          nowIso: () => NOW_ISO,
          readCurrentStates: async () => [
            {
              amount: 25,
              asset: 'sats',
              partnerRole: 'referral',
              state: 'eligible',
            },
          ],
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.generatedAt).toBe(NOW_ISO)
    expect(body.kind).toBe('partner_payouts_public')
    expect(body.settledCount).toBe(0)
    expect(body.totalCurrentPayouts).toBe(1)
    expect((body.staleness as Record<string, unknown>).composition).toBe(
      'live_at_read',
    )
  })

  test('non-GET is rejected', async () => {
    const response = await Effect.runPromise(
      handlePartnerPayoutsPublicApi(
        new Request('https://openagents.com/api/public/partner-payouts', {
          method: 'POST',
        }),
        { nowIso: () => NOW_ISO, readCurrentStates: async () => [] },
      ),
    )

    expect(response.status).toBe(405)
  })
})
