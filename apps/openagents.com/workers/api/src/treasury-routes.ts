import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  isLightningAddress,
  resolveLightningAddressInvoice,
} from './lnurl-pay'
import type { XClaimRewardTreasuryDispatchStats } from './x-claim-reward-treasury-dispatcher'

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
  readRewardDispatchStats?: () => Promise<XClaimRewardTreasuryDispatchStats>
  requireAdminApiToken: (request: Request) => Promise<boolean>
  // LNURL-pay resolver used to turn a Lightning Address destination into a
  // payable BOLT11 (#5078). Defaults to the real resolver; injectable for tests.
  resolveLightningAddress?: (
    address: string,
    amountSat: number,
  ) => Promise<{ ok: true; bolt11: string } | { ok: false; reason: string }>
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

const readRewardDispatchStats = (
  dependencies: TreasuryRouteDependencies,
): Effect.Effect<XClaimRewardTreasuryDispatchStats | null> => {
  const reader = dependencies.readRewardDispatchStats

  return reader === undefined
    ? Effect.succeed(null)
    : Effect.tryPromise({
        catch: () => null,
        try: () => reader(),
      }).pipe(Effect.catch(() => Effect.succeed(null)))
}

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
        Effect.all({
          balance:
            health !== null && treasuryState(health) === 'configured'
              ? readTreasuryBalance(fetchTreasury)
              : Effect.succeed(null),
          rewardDispatch: readRewardDispatchStats(dependencies),
        }).pipe(
          Effect.map(({ balance, rewardDispatch }) =>
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
              ...(rewardDispatch === null ? {} : { rewardDispatch }),
              service: dependencies.serviceLabel ?? 'mdk_treasury',
              state: treasuryState(health),
            }),
          ),
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

// Policy-applying payout core (issue #4703): the ONE path that moves
// money out of the treasury, shared by the operator route and the
// gated in-worker Artanis spend action. Applies the owner's
// full-or-10%-fractional policy and records the transaction.
export type TreasuryPayoutExecution =
  | Readonly<{
      kind: 'paid'
      intendedAmountSat: number
      paidAmountSat: number
      policyApplied: string
      paymentRef: string
      status: string
      // Which destination rail actually settled: the primary `destination`
      // (online BOLT 12 / MDK) or the `fallbackDestination` (e.g. the
      // recipient's static Lightning Address held on file) (#5078).
      paidVia: 'primary' | 'fallback'
    }>
  | Readonly<{
      kind: 'refused'
      reason:
        | 'treasury_unconfigured'
        | 'treasury_depleted'
        | 'treasury_balance_unavailable'
        | 'treasury_pay_failed'
      intendedAmountSat: number
      policyApplied: string | null
    }>

export const executeTreasuryPayout = async (
  dependencies: TreasuryRouteDependencies,
  input: Readonly<{
    destination: string
    amountSat: number
    // Optional payout fallback (e.g. the recipient's static Lightning Address
    // hosted by their offline Spark backup wallet's LSP). When the primary
    // destination fails, we retry once against this address. Both are normal
    // Lightning sends on the treasury's pay path — no Spark SDK on our side.
    fallbackDestination?: string | null
  }>,
): Promise<TreasuryPayoutExecution> => {
  const fetchTreasury = dependencies.fetchTreasury
  const intendedAmountSat = Math.floor(input.amountSat)
  const fallbackDestination =
    typeof input.fallbackDestination === 'string' &&
    input.fallbackDestination.trim() !== '' &&
    input.fallbackDestination.trim() !== input.destination.trim()
      ? input.fallbackDestination.trim()
      : null

  if (fetchTreasury === undefined) {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: null,
      reason: 'treasury_unconfigured',
    }
  }

  const balanceResponse = await fetchTreasury('/balance')
  if (!balanceResponse.ok) {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: null,
      reason: 'treasury_balance_unavailable',
    }
  }
  const balance = balancePayload(await balanceResponse.json())
  if (balance === null) {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: null,
      reason: 'treasury_balance_unavailable',
    }
  }
  const plan = treasuryPayoutPlan({
    intendedAmountSat,
    maxSendableSat: balance.maxSendableSat ?? 0,
  })

  if (plan.kind === 'depleted') {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: 'depleted',
      reason: 'treasury_depleted',
    }
  }

  const attemptPay = async (
    destination: string,
  ): Promise<Record<string, unknown> | null> => {
    // A Lightning Address can't be paid by MDK directly; resolve it to a BOLT11
    // via LNURL-pay first (#5078).
    let sendDestination = destination
    if (isLightningAddress(destination)) {
      const resolve =
        dependencies.resolveLightningAddress ?? resolveLightningAddressInvoice
      const resolved = await resolve(destination, plan.paidAmountSat)
      if (!resolved.ok) {
        return null
      }
      sendDestination = resolved.bolt11
    }
    const payResponse = await fetchTreasury('/pay', {
      body: JSON.stringify({
        amountSat: plan.paidAmountSat,
        destination: sendDestination,
      }),
      method: 'POST',
    })
    const payResult = (await payResponse.json()) as Record<string, unknown>
    if (!payResponse.ok || payResult.status !== 'succeeded') {
      return null
    }
    return payResult
  }

  let paidVia: 'primary' | 'fallback' = 'primary'
  let payResult = await attemptPay(input.destination)
  if (payResult === null && fallbackDestination !== null) {
    // Retry once against the fallback destination (e.g. the recipient's static
    // Lightning Address). Still a normal Lightning send on the pay path.
    paidVia = 'fallback'
    payResult = await attemptPay(fallbackDestination)
  }

  if (payResult === null) {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: plan.kind,
      reason: 'treasury_pay_failed',
    }
  }

  const paymentRef = `payment.treasury.${String(payResult.paymentId ?? '').slice(0, 12)}`
  await dependencies.recordPayoutTransaction?.({
    amountSat: plan.paidAmountSat,
    paymentRef,
    settled: true,
  })

  return {
    intendedAmountSat,
    kind: 'paid',
    paidAmountSat: plan.paidAmountSat,
    paymentRef,
    policyApplied: plan.kind,
    status: 'succeeded',
    paidVia,
  }
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
          let body: {
            amountSat?: unknown
            destination?: unknown
            fallbackDestination?: unknown
          } = {}

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
          const fallbackDestination =
            typeof body.fallbackDestination === 'string' &&
            body.fallbackDestination.trim() !== '' &&
            body.fallbackDestination.trim() !== destination
              ? body.fallbackDestination.trim()
              : null
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

          const attemptPay = async (
            payDestination: string,
          ): Promise<{
            ok: boolean
            payResult: Record<string, unknown>
          }> => {
            // MDK pays BOLT11/BOLT12, not a Lightning Address. If the destination
            // is a lud16 address (e.g. a Spark-hosted offline-receive address),
            // resolve it to a BOLT11 for this amount via LNURL-pay first (#5078).
            let sendDestination = payDestination
            if (isLightningAddress(payDestination)) {
              const resolve =
                dependencies.resolveLightningAddress ??
                resolveLightningAddressInvoice
              const resolved = await resolve(payDestination, plan.paidAmountSat)
              if (!resolved.ok) {
                return {
                  ok: false,
                  payResult: {
                    error: `lightning_address_resolution_failed:${resolved.reason}`,
                  },
                }
              }
              sendDestination = resolved.bolt11
            }
            const payResponse = await fetchTreasury('/pay', {
              body: JSON.stringify({
                amountSat: plan.paidAmountSat,
                destination: sendDestination,
              }),
              method: 'POST',
            })
            const payResult = (await payResponse.json()) as Record<
              string,
              unknown
            >
            return { ok: payResponse.ok, payResult }
          }

          let paidVia: 'primary' | 'fallback' = 'primary'
          let attempt = await attemptPay(destination)
          if (!attempt.ok && fallbackDestination !== null) {
            // Retry once against the recipient's payout fallback (e.g. their
            // static Lightning Address). Still a normal Lightning send.
            paidVia = 'fallback'
            attempt = await attemptPay(fallbackDestination)
          }

          const payResult = attempt.payResult

          if (!attempt.ok) {
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
            paidVia,
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
