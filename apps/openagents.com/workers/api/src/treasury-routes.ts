import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isRecord } from './json-boundary'
import { isLightningAddress, resolveLightningAddressInvoice } from './lnurl-pay'
import { currentIsoTimestamp } from './runtime-primitives'
import type {
  TreasuryTransactionRecord,
  TreasuryTransactionStore,
} from './treasury-page-routes'
import type { XClaimRewardTreasuryDispatchStats } from './x-claim-reward-treasury-dispatcher'

export const TREASURY_SERVICE_TOKEN_HEADER = 'x-treasury-service-token'

export type TreasuryRouteDependencies = Readonly<{
  serviceLabel?: string
  fetchTreasury?: ContainerPathFetch | undefined
  fetchTipsBuffer?: ContainerPathFetch | undefined
  transactionStore?: TreasuryTransactionStore | undefined
  // Bounds operator payout calls into the MDK container so a stuck daemon send
  // returns a classified diagnostic instead of hanging the operator request.
  payRequestTimeoutMs?: number | undefined
  recordPayoutTransaction?:
    | ((input: {
        amountSat: number
        failureReasonRef?: string | null
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

type TreasuryPaymentStatus = 'pending' | 'succeeded' | 'failed'

type TreasuryTransactionWallet = 'treasury' | 'tips_buffer'

const transactionWallet = (
  record: TreasuryTransactionRecord,
): TreasuryTransactionWallet =>
  record.id.startsWith('tips_buffer_payout_') ? 'tips_buffer' : 'treasury'

const paymentIdForContainerLookup = (paymentRef: string): string | null =>
  paymentRef.startsWith('payment.treasury.') ||
  paymentRef.startsWith('payment.tips_buffer.')
    ? null
    : paymentRef

const paymentStatusFromPayload = (payload: unknown): TreasuryPaymentStatus => {
  if (typeof payload !== 'object' || payload === null) {
    return 'pending'
  }

  const status = (payload as Record<string, unknown>).status

  return status === 'succeeded' || status === 'failed' ? status : 'pending'
}

const readTreasuryPaymentStatus = (
  fetchPaymentStatus: ContainerPathFetch,
  paymentId: string,
): Promise<TreasuryPaymentStatus> =>
  fetchPaymentStatus(`/payments/${encodeURIComponent(paymentId)}`)
    .then(response => (response.ok ? response.json() : null))
    .then(paymentStatusFromPayload)
    .catch(() => 'pending' as const)

export type TreasuryTransactionReconcileResult = Readonly<{
  amountSat: number
  paymentStatus: TreasuryPaymentStatus
  previousState: TreasuryTransactionRecord['state']
  reconciledState: TreasuryTransactionRecord['state']
  transactionId: string
  updated: boolean
  wallet: TreasuryTransactionWallet
}>

type TreasuryTransactionReconcileBlocked = Readonly<{
  error: string
  kind: 'blocked'
  status: number
}>

type TreasuryTransactionReconcileDependencies = Readonly<{
  fetchTipsBuffer?: ContainerPathFetch | undefined
  fetchTreasury?: ContainerPathFetch | undefined
  transactionStore: TreasuryTransactionStore
}>

const reconcileTreasuryTransactionRecord = async (
  dependencies: TreasuryTransactionReconcileDependencies,
  record: TreasuryTransactionRecord,
): Promise<
  TreasuryTransactionReconcileResult | TreasuryTransactionReconcileBlocked
> => {
  const wallet = transactionWallet(record)
  const fetchPaymentStatus =
    wallet === 'tips_buffer'
      ? dependencies.fetchTipsBuffer
      : dependencies.fetchTreasury

  if (fetchPaymentStatus === undefined) {
    return {
      error: 'treasury_payment_status_unavailable',
      kind: 'blocked',
      status: 503,
    }
  }

  if (record.paymentRef === null) {
    return {
      error: 'treasury_payment_ref_missing',
      kind: 'blocked',
      status: 409,
    }
  }

  const paymentId = paymentIdForContainerLookup(record.paymentRef)

  if (paymentId === null) {
    return {
      error: 'treasury_payment_ref_not_reconcilable',
      kind: 'blocked',
      status: 409,
    }
  }

  const paymentStatus = await readTreasuryPaymentStatus(
    fetchPaymentStatus,
    paymentId,
  )

  if (record.state === 'pending' && paymentStatus === 'succeeded') {
    await dependencies.transactionStore.settle({
      amountSat: record.amountSat,
      id: record.id,
      settledAt: currentIsoTimestamp(),
    })
  }

  if (record.state === 'pending' && paymentStatus === 'failed') {
    await dependencies.transactionStore.fail({ id: record.id })
  }

  const reconciledState =
    record.state !== 'pending'
      ? record.state
      : paymentStatus === 'succeeded'
        ? 'settled'
        : paymentStatus === 'failed'
          ? 'failed'
          : 'pending'

  return {
    amountSat: record.amountSat,
    paymentStatus,
    previousState: record.state,
    reconciledState,
    transactionId: record.id,
    updated: record.state !== reconciledState,
    wallet,
  }
}

const isTreasuryTransactionReconcileBlocked = (
  result:
    | TreasuryTransactionReconcileResult
    | TreasuryTransactionReconcileBlocked,
): result is TreasuryTransactionReconcileBlocked => 'kind' in result

export const reconcilePendingTreasuryTransactions = async (
  dependencies: TreasuryTransactionReconcileDependencies &
    Readonly<{ limit?: number | undefined }>,
) => {
  const records = await dependencies.transactionStore.listPendingOutbound(
    dependencies.limit ?? 20,
  )
  const results = await Promise.all(
    records.map(record =>
      reconcileTreasuryTransactionRecord(dependencies, record),
    ),
  )
  const reconciledResults = results.filter(
    (result): result is TreasuryTransactionReconcileResult =>
      !isTreasuryTransactionReconcileBlocked(result),
  )

  return {
    blocked: results.filter(isTreasuryTransactionReconcileBlocked).length,
    checked: results.length,
    failed: results.filter(
      result =>
        !isTreasuryTransactionReconcileBlocked(result) &&
        result.reconciledState === 'failed',
    ).length,
    pending: results.filter(
      result =>
        !isTreasuryTransactionReconcileBlocked(result) &&
        result.reconciledState === 'pending',
    ).length,
    settled: reconciledResults.filter(
      result => result.reconciledState === 'settled',
    ).length,
    updated: reconciledResults.filter(result => result.updated).length,
  }
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

export const handleOperatorTreasuryTransactionReconcileApi = (
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

      const store = dependencies.transactionStore

      if (store === undefined) {
        return Effect.succeed(
          noStoreJsonResponse(
            { error: 'treasury_transaction_store_unavailable' },
            { status: 503 },
          ),
        )
      }

      return Effect.tryPromise({
        catch: () => null,
        try: async () => {
          let body: { transactionId?: unknown } = {}

          try {
            body = (await request.json()) as typeof body
          } catch {
            return noStoreJsonResponse(
              { error: 'invalid_json_body' },
              { status: 400 },
            )
          }

          const transactionId =
            typeof body.transactionId === 'string'
              ? body.transactionId.trim()
              : ''

          if (transactionId === '') {
            return noStoreJsonResponse(
              { error: 'transaction_id_required' },
              { status: 400 },
            )
          }

          const record = await store.read(transactionId)

          if (record === undefined) {
            return noStoreJsonResponse(
              { error: 'treasury_transaction_not_found' },
              { status: 404 },
            )
          }

          if (record.direction !== 'out') {
            return noStoreJsonResponse(
              { error: 'treasury_transaction_not_outbound' },
              { status: 400 },
            )
          }

          const result = await reconcileTreasuryTransactionRecord(
            {
              fetchTipsBuffer: dependencies.fetchTipsBuffer,
              fetchTreasury: dependencies.fetchTreasury,
              transactionStore: store,
            },
            record,
          )

          return isTreasuryTransactionReconcileBlocked(result)
            ? noStoreJsonResponse(
                { error: result.error },
                { status: result.status },
              )
            : noStoreJsonResponse(result)
        },
      }).pipe(
        Effect.catch(() =>
          Effect.succeed(
            noStoreJsonResponse(
              { error: 'treasury_transaction_reconcile_failed' },
              { status: 502 },
            ),
          ),
        ),
        Effect.map(response =>
          response === null
            ? noStoreJsonResponse(
                { error: 'treasury_transaction_reconcile_failed' },
                { status: 502 },
              )
            : response,
        ),
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

const DEFAULT_TREASURY_PAY_REQUEST_TIMEOUT_MS = 65_000

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

type TreasuryBalancePayload = Readonly<{ maxSendableSat: number | null }>

type TreasuryBalanceRead =
  | Readonly<{ balance: TreasuryBalancePayload; kind: 'ok' }>
  | Readonly<{ kind: 'unavailable' }>

const balancePayload = (payload: unknown): TreasuryBalancePayload | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>
  const value = record.maxSendableSat

  return value === null || typeof value === 'number'
    ? { maxSendableSat: value as number | null }
    : null
}

const waitForTreasurySendabilityRetry = (): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, 250))

const readTreasuryBalancePayload = async (
  fetchTreasury: ContainerPathFetch,
): Promise<TreasuryBalanceRead> => {
  try {
    const balanceResponse = await fetchTreasury('/balance')

    if (!balanceResponse.ok) {
      return { kind: 'unavailable' }
    }

    const balance = balancePayload(await balanceResponse.json())

    return balance === null
      ? { kind: 'unavailable' }
      : { balance, kind: 'ok' }
  } catch {
    return { kind: 'unavailable' }
  }
}

const readTreasuryBalanceWithSendabilityRetry = async (
  fetchTreasury: ContainerPathFetch,
  remainingAttempts = 3,
): Promise<TreasuryBalanceRead> => {
  const read = await readTreasuryBalancePayload(fetchTreasury)

  if (
    read.kind !== 'ok' ||
    read.balance.maxSendableSat !== null ||
    remainingAttempts <= 1
  ) {
    return read
  }

  await waitForTreasurySendabilityRetry()

  return readTreasuryBalanceWithSendabilityRetry(
    fetchTreasury,
    remainingAttempts - 1,
  )
}

const safeReasonSegment = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, '_')
    .replace(/^_+|_+$/gu, '')
    .slice(0, 80)

const treasuryPayoutFailureReasonRef = (detail: string | null): string => {
  if (detail === null || detail.trim() === '') {
    return 'reason.public.treasury_payout.failed'
  }

  const normalized = detail.trim().toLowerCase()
  const lightningPrefix = 'lightning_address_resolution_failed:'

  if (normalized.startsWith(lightningPrefix)) {
    const resolvedReason = safeReasonSegment(
      normalized.slice(lightningPrefix.length),
    )

    return resolvedReason === ''
      ? 'reason.public.treasury_payout.lightning_address_resolution_failed'
      : `reason.public.treasury_payout.lightning_address_resolution_failed.${resolvedReason}`
  }

  if (
    normalized.includes('treasury_insufficient_spendable_balance') ||
    normalized.includes('insufficient')
  ) {
    return 'reason.public.treasury_payout.insufficient_spendable_balance'
  }

  if (normalized.includes('self_pay')) {
    return 'reason.public.treasury_payout.self_pay_refused'
  }

  if (normalized.includes('timeout') || normalized.includes('timed out')) {
    return 'reason.public.treasury_payout.timeout'
  }

  if (normalized.includes('liquidity')) {
    return 'reason.public.treasury_payout.liquidity'
  }

  if (normalized.includes('route')) {
    return 'reason.public.treasury_payout.no_route'
  }

  if (normalized.includes('invoice')) {
    return 'reason.public.treasury_payout.invoice_rejected'
  }

  return 'reason.public.treasury_payout.failed'
}

const publicTreasuryPayoutReasonRef = (value: unknown): string | null =>
  typeof value === 'string' &&
  /^reason\.public\.treasury_payout\.[a-z0-9_.:-]{1,180}$/u.test(value)
    ? value
    : null

const safeTreasuryPayoutDiagnosticString = (value: unknown): string | null =>
  typeof value === 'string' && /^[a-z0-9_.:-]{1,120}$/u.test(value)
    ? value
    : null

const safeTreasuryPayoutDiagnosticNumber = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

const safeTreasuryPayoutDiagnosticBoolean = (value: unknown): boolean | null =>
  typeof value === 'boolean' ? value : null

const treasuryPayoutFailureReasonRefFromPayResult = (
  payResult: Record<string, unknown>,
): string => {
  const containerReasonRef = publicTreasuryPayoutReasonRef(payResult.reasonRef)
  const genericError =
    typeof payResult.error === 'string' &&
    payResult.error.endsWith('_pay_failed')
  const detail =
    containerReasonRef !== null
      ? containerReasonRef
      : genericError && typeof payResult.reason === 'string'
        ? payResult.reason
        : typeof payResult.error === 'string'
          ? payResult.error
          : typeof payResult.reason === 'string'
            ? payResult.reason
            : typeof payResult.message === 'string'
              ? payResult.message
              : typeof payResult.code === 'string'
                ? payResult.code
                : null

  return containerReasonRef ?? treasuryPayoutFailureReasonRef(detail)
}

const treasuryPayoutDiagnosticPayload = (
  payResult: Record<string, unknown>,
) => ({
  balanceChanged: safeTreasuryPayoutDiagnosticBoolean(
    payResult.balanceChanged,
  ),
  balanceSatAfter: safeTreasuryPayoutDiagnosticNumber(
    payResult.balanceSatAfter,
  ),
  balanceSatBefore: safeTreasuryPayoutDiagnosticNumber(
    payResult.balanceSatBefore,
  ),
  containerStatus: safeTreasuryPayoutDiagnosticString(payResult.status),
  destinationKind: safeTreasuryPayoutDiagnosticString(payResult.destinationKind),
  errorCode: safeTreasuryPayoutDiagnosticString(payResult.errorCode),
  errorName: safeTreasuryPayoutDiagnosticString(payResult.errorName),
  failureStage: safeTreasuryPayoutDiagnosticString(payResult.failureStage),
  feeBudgetMsatAfter: safeTreasuryPayoutDiagnosticNumber(
    payResult.feeBudgetMsatAfter,
  ),
  feeBudgetMsatBefore: safeTreasuryPayoutDiagnosticNumber(
    payResult.feeBudgetMsatBefore,
  ),
  messageFingerprint: safeTreasuryPayoutDiagnosticString(
    payResult.messageFingerprint,
  ),
  paymentIdPresent: safeTreasuryPayoutDiagnosticBoolean(
    payResult.paymentIdPresent,
  ),
  payResponseStatus: safeTreasuryPayoutDiagnosticNumber(
    payResult.payResponseStatus,
  ),
  preflightBalanceMaxSendableSat: safeTreasuryPayoutDiagnosticNumber(
    payResult.preflightBalanceMaxSendableSat,
  ),
  preflightMaxSendableSat: safeTreasuryPayoutDiagnosticNumber(
    payResult.preflightMaxSendableSat,
  ),
  reasonClass: safeTreasuryPayoutDiagnosticString(payResult.reasonClass),
  resolvedDestinationKind: safeTreasuryPayoutDiagnosticString(
    payResult.resolvedDestinationKind,
  ),
  resultReturned: safeTreasuryPayoutDiagnosticBoolean(payResult.resultReturned),
  sourceDestinationKind: safeTreasuryPayoutDiagnosticString(
    payResult.sourceDestinationKind,
  ),
  timeoutSecs: safeTreasuryPayoutDiagnosticNumber(payResult.timeoutSecs),
})

const treasuryPayoutAttemptTrace = (input: {
  attempt: 'primary' | 'fallback'
  ok: boolean
  payResult: Record<string, unknown>
}) => ({
  attempt: input.attempt,
  diagnostics: treasuryPayoutDiagnosticPayload(input.payResult),
  outcome: input.ok ? 'accepted' : 'failed',
  reasonRef: input.ok
    ? null
    : treasuryPayoutFailureReasonRefFromPayResult(input.payResult),
})

const treasuryPayRequestTimeoutMs = (
  dependencies: TreasuryRouteDependencies,
): number => {
  const configured = dependencies.payRequestTimeoutMs

  return typeof configured === 'number' &&
    Number.isFinite(configured) &&
    configured > 0
    ? Math.floor(configured)
    : DEFAULT_TREASURY_PAY_REQUEST_TIMEOUT_MS
}

const treasuryPayTimeoutSignal = (timeoutMs: number): AbortSignal | undefined =>
  typeof AbortSignal.timeout === 'function'
    ? AbortSignal.timeout(timeoutMs)
    : undefined

const treasuryPayRequestTimeout = (
  timeoutMs: number,
): Promise<{ kind: 'timeout' }> =>
  new Promise(resolve => {
    setTimeout(() => resolve({ kind: 'timeout' }), timeoutMs)
  })

const treasuryPayRequestErrorName = (error: unknown): string | null =>
  error instanceof Error ? safeReasonSegment(error.name) : null

const treasuryPayRequestFailurePayload = (input: {
  error?: unknown
  resolvedDestinationKind: string | null
  sourceDestinationKind: string | null
  timeoutMs: number
  timedOut: boolean
}): Record<string, unknown> => ({
  error: input.timedOut
    ? 'treasury_pay_request_timeout'
    : 'treasury_pay_request_failed',
  errorName:
    input.error === undefined ? null : treasuryPayRequestErrorName(input.error),
  failureStage: input.timedOut ? 'pay_request_timeout' : 'pay_request_fetch',
  reasonRef: input.timedOut
    ? 'reason.public.treasury_payout.timeout'
    : 'reason.public.treasury_payout.failed',
  resolvedDestinationKind: input.resolvedDestinationKind,
  resultReturned: false,
  sourceDestinationKind: input.sourceDestinationKind,
  timeoutSecs: Math.ceil(input.timeoutMs / 1000),
})

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

  const balanceRead = await readTreasuryBalanceWithSendabilityRetry(
    fetchTreasury,
  )
  if (balanceRead.kind === 'unavailable') {
    return {
      intendedAmountSat,
      kind: 'refused',
      policyApplied: null,
      reason: 'treasury_balance_unavailable',
    }
  }
  const balance = balanceRead.balance
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

          const balanceRead = await readTreasuryBalanceWithSendabilityRetry(
            fetchTreasury,
          )

          if (balanceRead.kind === 'unavailable') {
            return noStoreJsonResponse(
              { error: 'treasury_balance_unavailable' },
              { status: 503 },
            )
          }

          const balance = balanceRead.balance

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
            const sourceDestinationKind = isLightningAddress(payDestination)
              ? 'lightning_address'
              : null
            let resolvedDestinationKind: string | null = null
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
                    failureStage: 'lightning_address_resolution',
                    sourceDestinationKind,
                  },
                }
              }
              sendDestination = resolved.bolt11
              resolvedDestinationKind = 'bolt11'
            }
            const timeoutMs = treasuryPayRequestTimeoutMs(dependencies)
            const payRequest = fetchTreasury('/pay', {
              body: JSON.stringify({
                amountSat: plan.paidAmountSat,
                destination: sendDestination,
              }),
              method: 'POST',
              signal: treasuryPayTimeoutSignal(timeoutMs),
            })
              .then(async payResponse => {
                let payResult: Record<string, unknown>

                try {
                  const parsed = await payResponse.json()
                  payResult = isRecord(parsed) ? parsed : {}
                } catch {
                  payResult = {
                    error: 'treasury_pay_response_invalid_json',
                    failureStage: 'pay_response_invalid_json',
                  }
                }

                return { kind: 'response' as const, payResponse, payResult }
              })
              .catch(error => ({ error, kind: 'error' as const }))

            const payOutcome = await Promise.race([
              payRequest,
              treasuryPayRequestTimeout(timeoutMs),
            ])

            if (payOutcome.kind === 'timeout') {
              return {
                ok: false,
                payResult: treasuryPayRequestFailurePayload({
                  resolvedDestinationKind,
                  sourceDestinationKind,
                  timedOut: true,
                  timeoutMs,
                }),
              }
            }

            if (payOutcome.kind === 'error') {
              return {
                ok: false,
                payResult: treasuryPayRequestFailurePayload({
                  error: payOutcome.error,
                  resolvedDestinationKind,
                  sourceDestinationKind,
                  timedOut: false,
                  timeoutMs,
                }),
              }
            }
            const resultReturned =
              typeof payOutcome.payResult.resultReturned === 'boolean'
                ? payOutcome.payResult.resultReturned
                : true

            return {
              ok:
                payOutcome.payResponse.ok &&
                payOutcome.payResult.status === 'succeeded',
              payResult: {
                ...payOutcome.payResult,
                payResponseStatus: payOutcome.payResponse.status,
                resolvedDestinationKind:
                  resolvedDestinationKind ??
                  safeTreasuryPayoutDiagnosticString(
                    payOutcome.payResult.destinationKind,
                  ),
                resultReturned,
                sourceDestinationKind,
              },
            }
          }

          const attempts: Array<ReturnType<typeof treasuryPayoutAttemptTrace>> =
            []
          let paidVia: 'primary' | 'fallback' = 'primary'
          let attempt = await attemptPay(destination)
          attempts.push(
            treasuryPayoutAttemptTrace({
              attempt: 'primary',
              ok: attempt.ok,
              payResult: attempt.payResult,
            }),
          )
          if (!attempt.ok && fallbackDestination !== null) {
            // Retry once against the recipient's payout fallback (e.g. their
            // static Lightning Address). Still a normal Lightning send.
            paidVia = 'fallback'
            attempt = await attemptPay(fallbackDestination)
            attempts.push(
              treasuryPayoutAttemptTrace({
                attempt: 'fallback',
                ok: attempt.ok,
                payResult: attempt.payResult,
              }),
            )
          }

          const payResult = attempt.payResult

          if (!attempt.ok) {
            // Surface the underlying failure (resolution vs. the daemon's pay
            // error) so an opaque "treasury_pay_failed" doesn't hide whether it
            // was LNURL resolution, no route, recipient offline, or liquidity.
            // Persist and return only a public-safe classification, never raw
            // daemon text, payment material, invoices, hashes, or destinations.
            const diagnostics = treasuryPayoutDiagnosticPayload(payResult)
            const failureReasonRef =
              treasuryPayoutFailureReasonRefFromPayResult(payResult)

            try {
              await dependencies.recordPayoutTransaction?.({
                amountSat: plan.paidAmountSat,
                failureReasonRef,
                paymentRef: null,
                settled: false,
              })
            } catch {
              // The response still reports the failure even if D1 is briefly
              // unavailable; the operator can retry diagnostics without spend.
            }

            return noStoreJsonResponse(
              {
                error: 'treasury_pay_failed',
                intendedAmountSat,
                paidAmountSat: null,
                paidVia,
                policyApplied: plan.kind,
                reason: failureReasonRef,
                reasonRef: failureReasonRef,
                diagnostics,
                attempts,
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
            attempts,
            diagnostics: treasuryPayoutDiagnosticPayload(payResult),
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
