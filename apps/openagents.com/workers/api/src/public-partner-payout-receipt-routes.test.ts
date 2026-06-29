import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  PartnerPayoutReceiptStore,
  PublicPartnerPayoutReceiptProjection,
} from './partner-payout-receipts'
import { makePublicPartnerPayoutReceiptRoutes } from './public-partner-payout-receipt-routes'

const projection = (
  receiptRef: string,
): PublicPartnerPayoutReceiptProjection => ({
  amount: 250,
  asset: 'sats',
  authorityBoundary:
    'Public proof only. This partner payout receipt read grants no partner attribution, eligibility, payout, settlement, withdrawal, wallet, provider, spend, revenue, registry, or public-claim authority.',
  caveatRefs: ['caveat.partner_payout.settlement_evidence_required'],
  evidenceRefs: ['evidence.partner_payout.adapter.hosted_mdk', receiptRef],
  generatedAt: '2026-06-20T00:01:00.000Z',
  policyRefs: ['policy.partner_payout.v1'],
  qualifyingEventKind: 'stripe_credit_purchase',
  receiptRef,
  resolution: {
    settlementRail: 'hosted_mdk',
    state: 'settled',
    status: 'ok',
  },
  schemaVersion: 'openagents.partner_payout_receipt.v1',
  sourceRefs: [
    `route:/api/public/partner-payout-receipts/${receiptRef}`,
    'ledger.partner_payout_ledger_entries.state',
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: [
      'partner_payout_eligibility_recorded',
      'partner_payout_state_transition_recorded',
    ],
  },
})

const storeFor = (
  receipt: PublicPartnerPayoutReceiptProjection | null,
): PartnerPayoutReceiptStore => ({
  readPartnerPayoutReceipt: () => Promise.resolve(receipt),
})

const route = async (
  store: PartnerPayoutReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const routes = makePublicPartnerPayoutReceiptRoutes<{
    store: PartnerPayoutReceiptStore
  }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-20T00:01:00.000Z',
  })
  const response = routes.routePublicPartnerPayoutReceiptRequest(
    new Request(
      `https://openagents.com/api/public/partner-payout-receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('partner payout receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public partner payout receipt routes', () => {
  test('serves settled partner payout receipts with staleness metadata', async () => {
    const receiptRef = 'receipt.partner_payout.hosted_mdk.abc123'
    const response = await route(storeFor(projection(receiptRef)), receiptRef)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      asset: 'sats',
      generatedAt: '2026-06-20T00:01:00.000Z',
      qualifyingEventKind: 'stripe_credit_purchase',
      receiptRef,
      resolution: {
        settlementRail: 'hosted_mdk',
        state: 'settled',
        status: 'ok',
      },
      schemaVersion: 'openagents.partner_payout_receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    const serialized = JSON.stringify(body).toLowerCase()
    for (const bannedFieldKey of [
      '"beneficiaryuserid"',
      '"currententryid"',
      '"idempotencykey"',
      '"partnerref"',
      '"partneruserid"',
      '"payoutref"',
      '"qualifyingeventref"',
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
      'receipt.partner_payout.hosted_mdk.nope',
    )

    expect(response.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor(null),
      'receipt.partner_payout.hosted_mdk.abc123',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
