import { describe, expect, test } from 'vitest'

import {
  MONEY_SURFACE_RETIREMENT_SCHEMA_VERSION,
  isRetiredMoneySurfaceRequest,
  moneySurfaceRetiredResponse,
} from './money-surface-retirement'

describe('VP-1 money-surface ingress retirement', () => {
  test.each([
    ['POST', '/api/billing/checkout'],
    ['GET', '/api/admin/credits/balance'],
    ['POST', '/api/agents/search/payments/redeem'],
    ['POST', '/api/forum/posts/post-1/direct-tips'],
    ['GET', '/api/forum/tip-leaderboards'],
    ['POST', '/api/sites/site-1/commerce/checkout-intents'],
    ['POST', '/api/pylons/pylon-1/spark-payout-target'],
    ['POST', '/api/operator/treasury/payout'],
    ['GET', '/api/public/treasury'],
    ['POST', '/api/public/labor-earnings/payout'],
    ['GET', '/api/public/markets/open-markets'],
    ['GET', '/api/sites'],
    ['POST', '/api/sites/builder-sessions'],
    ['POST', '/api/sites/site-1/forms/contact/submit'],
    ['POST', '/api/agent/sites/site-1/previews'],
    ['GET', '/api/operator/sites/site-1'],
    ['GET', '/r/site/source-1'],
    ['POST', '/v1/khala-code/plans/purchases'],
  ])('retires %s %s before auth or handler dispatch', (method, pathname) => {
    expect(isRetiredMoneySurfaceRequest(method, pathname)).toBe(true)
  })

  test.each([
    ['POST', '/api/billing/stripe/webhook'],
    ['POST', '/api/forum/paid-actions/mdk/webhooks'],
    ['POST', '/api/sites/site-1/commerce/mdk/webhooks'],
    ['GET', '/treasury'],
  ])('retires terminal callbacks and the treasury UI after the recovery archive: %s %s', (method, pathname) => {
    expect(isRetiredMoneySurfaceRequest(method, pathname)).toBe(true)
  })

  test.each([
    ['GET', '/api/public/billing/stripe-checkout-receipts/receipt-1'],
    ['GET', '/api/public/partner-payout-receipts/receipt-1'],
    ['GET', '/api/public/settled-feed'],
    ['POST', '/api/pylons/pylon-1/fleet-runs/claim'],
    ['GET', '/api/forum/posts'],
  ])('does not hide retained history or non-money MVP traffic: %s %s', (method, pathname) => {
    expect(isRetiredMoneySurfaceRequest(method, pathname)).toBe(false)
  })

  test('returns one auth-independent fail-closed 410 contract', async () => {
    const response = moneySurfaceRetiredResponse()
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(410)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body).toEqual({
      schemaVersion: MONEY_SURFACE_RETIREMENT_SCHEMA_VERSION,
      ok: false,
      error: {
        code: 'money_surface_retired',
        retryable: false,
      },
      retiredAt: '2026-07-14',
      capability: {
        payments: false,
        credits: false,
        wallets: false,
        payouts: false,
        settlements: false,
        paidCapacityAvailable: false,
        freeFallbackAllowed: false,
      },
    })
  })
})
