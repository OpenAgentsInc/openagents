import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const TREASURY_SERVICE_TOKEN_HEADER = 'x-treasury-service-token'

export type TreasuryRouteDependencies = Readonly<{
  serviceLabel?: string
  fetchTreasury?: ContainerPathFetch | undefined
  recordPayoutTransaction?:
    | ((input: {
        amountSat: number
        paymentRef: string | null
        settled: boolean
      }) => Promise<void>)
    | undefined
  requireAdminApiToken: (request: Request) => Promise<boolean>
}>

type TreasuryHealth = Readonly<{
  accessTokenConfigured: boolean
  mnemonicConfigured: boolean
  serviceTokenConfigured: boolean
}>

const TREASURY_AUTHORITY_BOUNDARY =
  'The campaign treasury pays bounded marketing rewards only. It is not the revenue node, the Forum tip payer, or Treasury settlement authority, and holding it grants no moderation, payout-policy, or registry authority.'

const TREASURY_POLICY_REFS = [
  'policy.public.treasury.bounded_campaign_rewards_only.v1',
  'policy.public.treasury.own_wallet_identity.v1',
] as const

const healthFromPayload = (payload: unknown): TreasuryHealth | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>

  return typeof record.mnemonicConfigured === 'boolean' &&
    typeof record.accessTokenConfigured === 'boolean' &&
    typeof record.serviceTokenConfigured === 'boolean'
    ? {
        accessTokenConfigured: record.accessTokenConfigured,
        mnemonicConfigured: record.mnemonicConfigured,
        serviceTokenConfigured: record.serviceTokenConfigured,
      }
    : null
}

const readTreasuryHealth = (
  fetchTreasury: ContainerPathFetch,
): Effect.Effect<TreasuryHealth | null> =>
  Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const response = await fetchTreasury('/healthz')

      if (!response.ok) {
        return null
      }

      return healthFromPayload(await response.json())
    },
  }).pipe(Effect.catch(() => Effect.succeed(null)))

const treasuryState = (health: TreasuryHealth | null): string => {
  if (health === null) {
    return 'unavailable'
  }

  return health.mnemonicConfigured &&
    health.accessTokenConfigured &&
    health.serviceTokenConfigured
    ? 'configured'
    : 'unconfigured'
}

export const handlePublicTreasuryLaunchStatusApi = (
  request: Request,
  dependencies: TreasuryRouteDependencies,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  if (dependencies.fetchTreasury === undefined) {
    return Effect.succeed(
      noStoreJsonResponse({
        authorityBoundary: TREASURY_AUTHORITY_BOUNDARY,
        policyRefs: TREASURY_POLICY_REFS,
        service: dependencies.serviceLabel ?? 'mdk_treasury',
        state: 'unprovisioned',
      }),
    )
  }

  return Effect.map(readTreasuryHealth(dependencies.fetchTreasury), health =>
    noStoreJsonResponse({
      authorityBoundary: TREASURY_AUTHORITY_BOUNDARY,
      configured:
        health === null
          ? null
          : {
              accessToken: health.accessTokenConfigured,
              mnemonic: health.mnemonicConfigured,
              serviceToken: health.serviceTokenConfigured,
            },
      policyRefs: TREASURY_POLICY_REFS,
      service: dependencies.serviceLabel ?? 'mdk_treasury',
      state: treasuryState(health),
    }),
  )
}

const readTreasuryBalance = (
  fetchTreasury: ContainerPathFetch,
): Effect.Effect<unknown> =>
  Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const response = await fetchTreasury('/balance')

      return response.ok ? await response.json() : null
    },
  }).pipe(Effect.catch(() => Effect.succeed(null)))

export const handleOperatorTreasuryStatusApi = (
  request: Request,
  dependencies: TreasuryRouteDependencies,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.tryPromise({
    catch: () => false,
    try: () => dependencies.requireAdminApiToken(request),
  }).pipe(
    Effect.catch(() => Effect.succeed(false)),
    Effect.flatMap(authorized => {
      if (!authorized) {
        return Effect.succeed(
          noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
        )
      }

      if (dependencies.fetchTreasury === undefined) {
        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'treasury_unprovisioned' },
            { status: 503 },
          ),
        )
      }

      const fetchTreasury = dependencies.fetchTreasury

      return Effect.flatMap(readTreasuryHealth(fetchTreasury), health =>
        Effect.map(
          health !== null && treasuryState(health) === 'configured'
            ? readTreasuryBalance(fetchTreasury)
            : Effect.succeed(null),
          balance =>
            noStoreJsonResponse({
              balance,
              configured:
                health === null
                  ? null
                  : {
                      accessToken: health.accessTokenConfigured,
                      mnemonic: health.mnemonicConfigured,
                      serviceToken: health.serviceTokenConfigured,
                    },
              service: dependencies.serviceLabel ?? 'mdk_treasury',
              state: treasuryState(health),
            }),
        ),
      )
    }),
  )
}

const readTreasuryFunding = (
  fetchTreasury: ContainerPathFetch,
): Effect.Effect<unknown> =>
  Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const response = await fetchTreasury('/offer')

      return response.ok ? await response.json() : null
    },
  }).pipe(Effect.catch(() => Effect.succeed(null)))

export const handleOperatorTreasuryFundingDestinationApi = (
  request: Request,
  dependencies: TreasuryRouteDependencies,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.tryPromise({
    catch: () => false,
    try: () => dependencies.requireAdminApiToken(request),
  }).pipe(
    Effect.catch(() => Effect.succeed(false)),
    Effect.flatMap(authorized => {
      if (!authorized) {
        return Effect.succeed(
          noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
        )
      }

      if (dependencies.fetchTreasury === undefined) {
        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'treasury_unprovisioned' },
            { status: 503 },
          ),
        )
      }

      return Effect.map(
        readTreasuryFunding(dependencies.fetchTreasury),
        funding =>
          funding === null
            ? noStoreJsonResponse(
                { error: 'treasury_funding_destination_unavailable' },
                { status: 503 },
              )
            : noStoreJsonResponse({ funding, service: 'mdk_treasury' }),
      )
    }),
  )
}

// Owner payout policy (2026-06-10): a payout that exceeds the treasury's
// spendable balance is not refused outright - it falls back to 10% of the
// current spendable amount (floored), so a depleted treasury pays out in a
// decaying series instead of stalling. Basis is maxSendableSat, the
// fee-buffered honest spendable figure.
export const TREASURY_FRACTIONAL_FALLBACK_DIVISOR = 10

export const treasuryPayoutPlan = (input: {
  intendedAmountSat: number
  maxSendableSat: number | null
}):
  | { kind: 'full'; paidAmountSat: number }
  | { kind: 'fractional_fallback_10pct'; paidAmountSat: number }
  | { kind: 'depleted' } => {
  if (input.maxSendableSat === null || input.maxSendableSat <= 0) {
    return { kind: 'depleted' }
  }

  if (input.maxSendableSat >= input.intendedAmountSat) {
    return { kind: 'full', paidAmountSat: input.intendedAmountSat }
  }

  const fallback = Math.floor(
    input.maxSendableSat / TREASURY_FRACTIONAL_FALLBACK_DIVISOR,
  )

  return fallback < 1
    ? { kind: 'depleted' }
    : { kind: 'fractional_fallback_10pct', paidAmountSat: fallback }
}

const balancePayload = (payload: unknown): { maxSendableSat: number | null } | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>
  const value = record.maxSendableSat

  return value === null || typeof value === 'number'
    ? { maxSendableSat: value as number | null }
    : null
}

export const handleOperatorTreasuryPayoutApi = (
  request: Request,
  dependencies: TreasuryRouteDependencies,
) => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  return Effect.tryPromise({
    catch: () => false,
    try: () => dependencies.requireAdminApiToken(request),
  }).pipe(
    Effect.catch(() => Effect.succeed(false)),
    Effect.flatMap(authorized => {
      if (!authorized) {
        return Effect.succeed(
          noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
        )
      }

      const fetchTreasury = dependencies.fetchTreasury

      if (fetchTreasury === undefined) {
        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'treasury_unprovisioned' },
            { status: 503 },
          ),
        )
      }

      return Effect.tryPromise({
        catch: () => null,
        try: async () => {
          let body: { amountSat?: unknown; destination?: unknown } = {}

          try {
            body = (await request.json()) as typeof body
          } catch {
            return noStoreJsonResponse(
              { error: 'invalid_json_body' },
              { status: 400 },
            )
          }

          const destination =
            typeof body.destination === 'string' ? body.destination.trim() : ''
          const intendedAmountSat = Number(body.amountSat)

          if (destination === '') {
            return noStoreJsonResponse(
              { error: 'destination_required' },
              { status: 400 },
            )
          }

          if (!Number.isInteger(intendedAmountSat) || intendedAmountSat <= 0) {
            return noStoreJsonResponse(
              { error: 'amount_sat_must_be_positive_integer' },
              { status: 400 },
            )
          }

          const balanceResponse = await fetchTreasury('/balance')

          if (!balanceResponse.ok) {
            return noStoreJsonResponse(
              { error: 'treasury_balance_unavailable' },
              { status: 503 },
            )
          }

          const balance = balancePayload(await balanceResponse.json())

          if (balance === null) {
            return noStoreJsonResponse(
              { error: 'treasury_balance_unavailable' },
              { status: 503 },
            )
          }

          const plan = treasuryPayoutPlan({
            intendedAmountSat,
            maxSendableSat: balance.maxSendableSat,
          })

          if (plan.kind === 'depleted') {
            return noStoreJsonResponse(
              {
                error: 'treasury_depleted',
                intendedAmountSat,
                maxSendableSat: balance.maxSendableSat,
              },
              { status: 409 },
            )
          }

          const payResponse = await fetchTreasury('/pay', {
            body: JSON.stringify({
              amountSat: plan.paidAmountSat,
              destination,
            }),
            method: 'POST',
          })
          const payResult = (await payResponse.json()) as Record<
            string,
            unknown
          >

          if (!payResponse.ok) {
            return noStoreJsonResponse(
              {
                error: 'treasury_pay_failed',
                intendedAmountSat,
                paidAmountSat: null,
                policyApplied: plan.kind,
                reason: payResult.error ?? null,
              },
              { status: 502 },
            )
          }

          try {
            await dependencies.recordPayoutTransaction?.({
              amountSat: plan.paidAmountSat,
              paymentRef:
                typeof payResult.paymentId === 'string'
                  ? payResult.paymentId
                  : null,
              settled: payResult.status === 'succeeded',
            })
          } catch {
            // The payment already happened; a ledger write failure must not
            // convert a successful payout into an error response.
          }

          return noStoreJsonResponse({
            intendedAmountSat,
            paidAmountSat: plan.paidAmountSat,
            paymentId: payResult.paymentId ?? null,
            policyApplied: plan.kind,
            status: payResult.status ?? null,
          })
        },
      }).pipe(
        Effect.catch(() =>
          Effect.succeed(
            noStoreJsonResponse(
              { error: 'treasury_payout_failed' },
              { status: 502 },
            ),
          ),
        ),
        Effect.map(response =>
          response === null
            ? noStoreJsonResponse(
                { error: 'treasury_payout_failed' },
                { status: 502 },
              )
            : response,
        ),
      )
    }),
  )
}
