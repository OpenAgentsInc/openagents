import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makePublicSiteReferralPayoutReceiptRoutes } from './public-site-referral-payout-receipt-routes'
import type {
  PublicSiteReferralPayoutReceiptProjection,
  SiteReferralPayoutReceiptStore,
} from './site-referral-payout-receipts'

const projection = (
  receiptRef: string,
): PublicSiteReferralPayoutReceiptProjection => ({
  amountSats: 125,
  attributionLinked: true,
  authorityBoundary:
    'Public proof only. This referral payout receipt read grants no attribution, invite, checkout, spend, refund, payout, settlement, wallet, provider, or registry authority.',
  caveatRefs: ['caveat.site_referral_payout.settlement_evidence_required'],
  evidenceRefs: [
    'evidence.site_referral_payout.adapter.hosted_mdk',
    receiptRef,
  ],
  generatedAt: '2026-06-20T00:01:00.000Z',
  policyRefs: ['policy.site_referral_payout.v1'],
  qualifyingEventKind: 'inference_paid_request',
  receiptRef,
  resolution: {
    settlementRail: 'hosted_mdk',
    state: 'settled',
    status: 'ok',
  },
  schemaVersion: 'openagents.site_referral_payout_receipt.v1',
  sourceRefs: [
    `route:/api/public/site-referral-payout-receipts/${receiptRef}`,
    'ledger.site_referral_payout_ledger_entries.state',
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: [
      'site_referral_payout_eligibility_recorded',
      'site_referral_payout_state_transition_recorded',
    ],
  },
})

const storeFor = (
  receipt: PublicSiteReferralPayoutReceiptProjection | null,
): SiteReferralPayoutReceiptStore => ({
  readSiteReferralPayoutReceipt: () => Promise.resolve(receipt),
})

const route = async (
  store: SiteReferralPayoutReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const routes = makePublicSiteReferralPayoutReceiptRoutes<{
    store: SiteReferralPayoutReceiptStore
  }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-20T00:01:00.000Z',
  })
  const response = routes.routePublicSiteReferralPayoutReceiptRequest(
    new Request(
      `https://openagents.com/api/public/site-referral-payout-receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('site referral payout receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public Site referral payout receipt routes', () => {
  test('serves settled referral payout receipts with staleness metadata', async () => {
    const receiptRef = 'receipt.site_referral_payout.hosted_mdk.abc123'
    const response = await route(storeFor(projection(receiptRef)), receiptRef)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      attributionLinked: true,
      generatedAt: '2026-06-20T00:01:00.000Z',
      qualifyingEventKind: 'inference_paid_request',
      receiptRef,
      resolution: {
        settlementRail: 'hosted_mdk',
        state: 'settled',
        status: 'ok',
      },
      schemaVersion: 'openagents.site_referral_payout_receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    const serialized = JSON.stringify(body).toLowerCase()
    for (const bannedFieldKey of [
      '"payoutref"',
      '"referreruserid"',
      '"referreduserid"',
      '"referralattributionid"',
      '"referralinviteid"',
      '"referralsourceid"',
      '"idempotencykey"',
      '"qualifyingeventref"',
      '"currententryid"',
    ]) {
      expect(serialized).not.toContain(bannedFieldKey)
    }
    for (const bannedMaterial of [
      'lnbc',
      'lntb',
      'payment_hash',
      'preimage',
      'private_key',
      'secret',
      'xprv',
    ]) {
      expect(serialized).not.toContain(bannedMaterial)
    }
  })

  test('returns 404 when the receipt ref does not resolve', async () => {
    const response = await route(
      storeFor(null),
      'receipt.site_referral_payout.hosted_mdk.nope',
    )

    expect(response.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor(null),
      'receipt.site_referral_payout.hosted_mdk.abc123',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
