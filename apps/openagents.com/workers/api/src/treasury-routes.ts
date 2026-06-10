import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const TREASURY_SERVICE_TOKEN_HEADER = 'x-treasury-service-token'

export type TreasuryRouteDependencies = Readonly<{
  fetchTreasury?: ContainerPathFetch | undefined
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
        service: 'mdk_treasury',
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
      service: 'mdk_treasury',
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
              service: 'mdk_treasury',
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
