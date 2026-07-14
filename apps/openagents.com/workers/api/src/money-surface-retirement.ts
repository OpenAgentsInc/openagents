export const MONEY_SURFACE_RETIREMENT_SCHEMA_VERSION =
  'openagents.money_surface_retired.v1' as const
export const MONEY_SURFACE_RETIRED_AT = '2026-07-14' as const

const retiredExactPaths = new Set([
  '/api/operator/artanis/spend-decision',
  '/api/operator/buy-mode/results/settle',
  '/api/public/partner-payouts',
  '/api/public/site-referral-payouts',
  '/api/public/treasury',
  '/api/public/treasury/launch-status',
  '/api/treasury/donate',
  '/api/sites',
  '/treasury',
  '/treasury/donate',
])

const retiredPrefixes = [
  '/api/admin/credits/',
  '/api/agent/sites',
  '/api/agents/search/payments/',
  '/api/billing/',
  '/api/mobile/credits/',
  '/api/omni/operator/billing/',
  '/api/onboarding/billing/',
  '/api/operator/business/sarah-checkout',
  '/api/operator/crm/sales/checkout',
  '/api/operator/partners/payout-ledger/',
  '/api/operator/sites',
  '/api/operator/sites/referrals/payout-ledger/',
  '/api/operator/tips-buffer/',
  '/api/operator/treasury/',
  '/api/public/autopilot/labor-products',
  '/api/public/labor-earnings',
  '/api/public/marketplace/',
  '/api/public/markets/',
  '/api/sites/',
  '/checkout',
  '/r/site/',
  '/v1/inference/privacy/paid-privacy/',
  '/v1/khala-code/plans/purchases',
] as const

const forumMoneyPath =
  /^\/api\/forum\/(?:moderation\/tip-earnings|tip-leaderboards|paid-actions\/|tip-recipient-wallets\/|actors\/[^/]+\/tip-earnings|posts\/[^/]+\/(?:direct-tips|tips\/ladder)|direct-tips\/|pylons\/[^/]+\/tips\/ladder|receipts\/[^/]+\/settlement-claims)/

const pylonMoneyPath =
  /^\/api\/pylons\/[^/]+\/(?:wallet-readiness|payout-target-admission|spark-payout-target|assignments\/[^/]+\/(?:payment-receipts|settlement-status))$/

export const isRetiredMoneySurfaceRequest = (
  _method: string,
  pathname: string,
): boolean => {
  return (
    retiredExactPaths.has(pathname) ||
    retiredPrefixes.some(prefix => pathname.startsWith(prefix)) ||
    forumMoneyPath.test(pathname) ||
    pylonMoneyPath.test(pathname)
  )
}

export const moneySurfaceRetiredResponse = (): Response =>
  Response.json(
    {
      schemaVersion: MONEY_SURFACE_RETIREMENT_SCHEMA_VERSION,
      ok: false,
      error: {
        code: 'money_surface_retired',
        retryable: false,
      },
      retiredAt: MONEY_SURFACE_RETIRED_AT,
      capability: {
        payments: false,
        credits: false,
        wallets: false,
        payouts: false,
        settlements: false,
        paidCapacityAvailable: false,
        freeFallbackAllowed: false,
      },
    },
    {
      headers: {
        'Cache-Control': 'no-store',
      },
      status: 410,
    },
  )
