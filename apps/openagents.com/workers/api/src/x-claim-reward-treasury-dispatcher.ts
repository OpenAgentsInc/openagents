import { Effect } from 'effect'

import type {
  XClaimRewardRecord,
  XClaimRewardState,
} from './agent-owner-claim-routes'
import { X_CLAIM_REWARD_AMOUNT_SATS } from './agent-owner-claim-routes'
import type { ContainerPathFetch } from './http/container-fetch'
import { parseJsonStringArray } from './json-boundary'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

const DEFAULT_PER_RUN_REWARD_CAP = 1
const DEFAULT_DAILY_SATS_CAP = 5000
const DEFAULT_PAYMENT_TIMEOUT_SECS = 50
const LIQUIDITY_BUFFER_FLOOR_SATS = 10
const LIQUIDITY_BUFFER_RETRY_MULTIPLIER = 1.1

const DispatchStartedReasonRef =
  'reason.public.x_claim_reward_treasury_dispatch_started'
const PaymentPendingReasonRef =
  'reason.public.x_claim_reward_treasury_payment_pending'
const RecipientWalletMissingReasonRef =
  'reason.public.x_claim_reward_recipient_wallet_missing'
const TreasuryUnavailableReasonRef =
  'reason.public.x_claim_reward_treasury_unavailable'
const TreasuryBalanceUnavailableReasonRef =
  'reason.public.x_claim_reward_treasury_balance_unavailable'
const TreasuryLiquidityInsufficientReasonRef =
  'reason.public.x_claim_reward_treasury_liquidity_insufficient'
const TreasuryPaymentFailedReasonRef =
  'reason.public.x_claim_reward_treasury_payment_failed'
const TreasuryPayFailedReasonRef =
  'reason.public.x_claim_reward_treasury_pay_failed'
const DispatchDisabledReasonRef =
  'reason.public.x_claim_reward_treasury_dispatch_disabled'
const NoApprovedRewardReasonRef =
  'reason.public.x_claim_reward_no_approved_reward'
const DailyCapReachedReasonRef =
  'reason.public.x_claim_reward_treasury_daily_cap_reached'

type XClaimRewardRow = Readonly<{
  agent_user_id: string | null
  amount_sats: number
  challenge_id: string
  claim_id: string
  created_at: string
  evidence_refs_json: string
  id: string
  owner_user_id: string
  receipt_ref: string
  state: XClaimRewardState
  state_reason_ref: string | null
  treasury_payment_id: string | null
  updated_at: string
  x_account_ref: string
}>

export type XClaimRewardTreasuryDispatchConfig = Readonly<{
  dailySatsCap: number
  enabled: boolean
  liquidityBufferSats: number
  nowIso: string
  paymentTimeoutSecs: number
  perRunRewardCap: number
}>

export type XClaimRewardTreasuryDispatchStats = Readonly<{
  dailySatsCap: number
  enabled: boolean
  liquidityBufferSats: number
  pendingPaymentCount: number
  perRunRewardCap: number
  requestedDispatchCount: number
  todayReservedSats: number
}>

export type XClaimRewardTreasuryDispatchSummary = Readonly<{
  failed: number
  pending: number
  polled: number
  requested: number
  settled: number
  skippedReasonRefs: ReadonlyArray<string>
  stats: XClaimRewardTreasuryDispatchStats
}>

export type XClaimRewardTreasuryDispatchStore = Readonly<{
  attachTreasuryPayment: (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    paymentId: string
    rewardId: string
    stateReasonRef: string | null
  }) => Promise<XClaimRewardRecord | undefined>
  claimDispatchRequestedReward: (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    rewardId: string
    stateReasonRef: string
  }) => Promise<XClaimRewardRecord | undefined>
  countTodayReservedSats: (dayStartIso: string) => Promise<number>
  failReward: (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    reasonRef: string
    rewardId: string
  }) => Promise<XClaimRewardRecord | undefined>
  listDispatchRequestedRewards: (
    limit: number,
  ) => Promise<ReadonlyArray<XClaimRewardRecord>>
  listPendingTreasuryPaymentRewards: (
    limit: number,
  ) => Promise<ReadonlyArray<XClaimRewardRecord>>
  readDispatchStats: (
    dayStartIso: string,
    config: XClaimRewardTreasuryDispatchConfig,
  ) => Promise<XClaimRewardTreasuryDispatchStats>
  settleReward: (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    rewardId: string
  }) => Promise<XClaimRewardRecord | undefined>
}>

export type XClaimRewardTreasuryClient = Readonly<{
  pay: (input: {
    amountSat: number
    destination: string
    timeoutSecs: number
  }) => Promise<TreasuryPaymentResult>
  readBalance: () => Promise<TreasuryBalanceResult>
  readPayment: (paymentId: string) => Promise<TreasuryPaymentResult>
}>

type TreasuryBalanceResult =
  | Readonly<{ kind: 'ok'; maxSendableSat: number | null }>
  | Readonly<{ kind: 'unavailable'; reasonRef: string }>

type TreasuryPaymentResult =
  | Readonly<{ kind: 'succeeded'; paymentId: string }>
  | Readonly<{ kind: 'pending'; paymentId: string }>
  | Readonly<{ kind: 'failed'; paymentId: string | null; reasonRef: string }>

export type XClaimRewardTreasuryDispatcherDependencies = Readonly<{
  config: XClaimRewardTreasuryDispatchConfig
  store: XClaimRewardTreasuryDispatchStore
  treasury: XClaimRewardTreasuryClient | null
  resolveRecipient: (
    reward: XClaimRewardRecord,
  ) => Promise<{ destination: string; destinationSourceRef: string } | null>
}>

type DispatchAccumulator = Readonly<{
  failed: number
  pending: number
  polled: number
  requested: number
  settled: number
  skippedReasonRefs: ReadonlyArray<string>
  todayReservedSats: number
}>

const rowToReward = (row: XClaimRewardRow): XClaimRewardRecord => ({
  agentUserId: row.agent_user_id,
  amountSats: Number(row.amount_sats),
  challengeId: row.challenge_id,
  claimId: row.claim_id,
  createdAt: row.created_at,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  id: row.id,
  ownerUserId: row.owner_user_id,
  receiptRef: row.receipt_ref,
  state: row.state,
  stateReasonRef: row.state_reason_ref,
  treasuryPaymentId: row.treasury_payment_id,
  updatedAt: row.updated_at,
  xAccountRef: row.x_account_ref,
})

const normalizePositiveInteger = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback
}

const defaultLiquidityBufferSats = (): number =>
  Math.ceil(
    Math.max(
      LIQUIDITY_BUFFER_FLOOR_SATS,
      X_CLAIM_REWARD_AMOUNT_SATS * 0.01,
    ) * LIQUIDITY_BUFFER_RETRY_MULTIPLIER,
  )

export const xClaimRewardDispatchDayStartIso = (nowIso: string): string => {
  if (!Number.isFinite(Date.parse(nowIso))) {
    return nowIso
  }

  return `${nowIso.slice(0, 10)}T00:00:00.000Z`
}

export const readXClaimRewardTreasuryDispatchConfig = (
  env: Readonly<{
    TREASURY_DISPATCH_DAILY_SATS_CAP?: string | undefined
    TREASURY_DISPATCH_ENABLED?: string | undefined
    TREASURY_DISPATCH_LIQUIDITY_BUFFER_SATS?: string | undefined
    TREASURY_DISPATCH_PAYMENT_TIMEOUT_SECS?: string | undefined
    TREASURY_DISPATCH_PER_RUN_REWARD_CAP?: string | undefined
  }>,
  nowIso: string,
): XClaimRewardTreasuryDispatchConfig => ({
  dailySatsCap: normalizePositiveInteger(
    env.TREASURY_DISPATCH_DAILY_SATS_CAP,
    DEFAULT_DAILY_SATS_CAP,
  ),
  enabled: env.TREASURY_DISPATCH_ENABLED === 'true',
  liquidityBufferSats: normalizePositiveInteger(
    env.TREASURY_DISPATCH_LIQUIDITY_BUFFER_SATS,
    defaultLiquidityBufferSats(),
  ),
  nowIso,
  paymentTimeoutSecs: Math.min(
    normalizePositiveInteger(
      env.TREASURY_DISPATCH_PAYMENT_TIMEOUT_SECS,
      DEFAULT_PAYMENT_TIMEOUT_SECS,
    ),
    DEFAULT_PAYMENT_TIMEOUT_SECS,
  ),
  perRunRewardCap: normalizePositiveInteger(
    env.TREASURY_DISPATCH_PER_RUN_REWARD_CAP,
    DEFAULT_PER_RUN_REWARD_CAP,
  ),
})

/**
 * KS-8.8 (#8319): D1 stays the sole dispatch authority. On a
 * `TreasuryDatabase` seam handle each reward state transition additionally
 * read-back-mirrors the resolved ledger row into Postgres fail-soft. The
 * dispatch-decision scans (dispatch_requested / pending-payment) have NO
 * Postgres twin — the dispatcher reads exactly one store until the
 * epic-gated cutover, so the mirror can never double-dispatch a reward.
 */
export const makeD1XClaimRewardTreasuryDispatchStore = (
  database: TreasuryDatabase,
): XClaimRewardTreasuryDispatchStore => {
  const db = treasuryAuthorityDb(database)
  const mirrorReward = (rewardId: string) =>
    mirrorTreasuryRows(database, 'x_claim_reward_ledger', 'id', [rewardId])
  const readRewardById = async (
    rewardId: string,
  ): Promise<XClaimRewardRecord | undefined> => {
    const row = await db
      .prepare(`SELECT * FROM x_claim_reward_ledger WHERE id = ? LIMIT 1`)
      .bind(rewardId)
      .first<XClaimRewardRow>()

    return row === null ? undefined : rowToReward(row)
  }

  const countTodayReservedSats = async (dayStartIso: string): Promise<number> => {
    const row = await db
      .prepare(
        `SELECT COALESCE(SUM(amount_sats), 0) AS reserved_sats
           FROM x_claim_reward_ledger
          WHERE updated_at >= ?
            AND state IN ('dispatched', 'settled')
            AND treasury_payment_id IS NOT NULL`,
      )
      .bind(dayStartIso)
      .first<{ reserved_sats: number }>()

    return row === null ? 0 : Number(row.reserved_sats)
  }

  const updateReward = async (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    reasonRef: string | null
    rewardId: string
    state: XClaimRewardState
  }): Promise<XClaimRewardRecord | undefined> => {
    await db
      .prepare(
        `UPDATE x_claim_reward_ledger
            SET state = ?,
                state_reason_ref = ?,
                evidence_refs_json = ?,
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(
        input.state,
        input.reasonRef,
        JSON.stringify(input.evidenceRefs),
        input.nowIso,
        input.rewardId,
      )
      .run()

    await mirrorReward(input.rewardId)
    return readRewardById(input.rewardId)
  }

  return {
    attachTreasuryPayment: async input => {
      const result = await db
        .prepare(
          `UPDATE x_claim_reward_ledger
              SET treasury_payment_id = ?,
                  state_reason_ref = ?,
                  evidence_refs_json = ?,
                  updated_at = ?
            WHERE id = ?
              AND state = 'dispatched'
              AND treasury_payment_id IS NULL`,
        )
        .bind(
          input.paymentId,
          input.stateReasonRef,
          JSON.stringify(input.evidenceRefs),
          input.nowIso,
          input.rewardId,
        )
        .run()

      if (result.success && result.meta.changes >= 1) {
        await mirrorReward(input.rewardId)
        return readRewardById(input.rewardId)
      }
      return undefined
    },
    claimDispatchRequestedReward: async input => {
      const result = await db
        .prepare(
          `UPDATE x_claim_reward_ledger
              SET state = 'dispatched',
                  state_reason_ref = ?,
                  evidence_refs_json = ?,
                  updated_at = ?
            WHERE id = ?
              AND state = 'dispatch_requested'
              AND treasury_payment_id IS NULL`,
        )
        .bind(
          input.stateReasonRef,
          JSON.stringify(input.evidenceRefs),
          input.nowIso,
          input.rewardId,
        )
        .run()

      if (result.success && result.meta.changes >= 1) {
        await mirrorReward(input.rewardId)
        return readRewardById(input.rewardId)
      }
      return undefined
    },
    countTodayReservedSats,
    failReward: input =>
      updateReward({
        evidenceRefs: input.evidenceRefs,
        nowIso: input.nowIso,
        reasonRef: input.reasonRef,
        rewardId: input.rewardId,
        state: 'failed',
      }),
    listDispatchRequestedRewards: async limit => {
      const rows = await db
        .prepare(
          `SELECT *
             FROM x_claim_reward_ledger
            WHERE state = 'dispatch_requested'
            ORDER BY updated_at ASC
            LIMIT ?`,
        )
        .bind(Math.max(0, Math.floor(limit)))
        .all<XClaimRewardRow>()

      return rows.results.map(rowToReward)
    },
    listPendingTreasuryPaymentRewards: async limit => {
      const rows = await db
        .prepare(
          `SELECT *
             FROM x_claim_reward_ledger
            WHERE state = 'dispatched'
              AND treasury_payment_id IS NOT NULL
            ORDER BY updated_at ASC
            LIMIT ?`,
        )
        .bind(Math.max(0, Math.floor(limit)))
        .all<XClaimRewardRow>()

      return rows.results.map(rowToReward)
    },
    readDispatchStats: async (dayStartIso, config) => {
      const requested = await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM x_claim_reward_ledger
            WHERE state = 'dispatch_requested'`,
        )
        .first<{ count: number }>()
      const pending = await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM x_claim_reward_ledger
            WHERE state = 'dispatched'
              AND treasury_payment_id IS NOT NULL`,
        )
        .first<{ count: number }>()
      const todayReservedSats = await countTodayReservedSats(dayStartIso)

      return {
        dailySatsCap: config.dailySatsCap,
        enabled: config.enabled,
        liquidityBufferSats: config.liquidityBufferSats,
        pendingPaymentCount: pending === null ? 0 : Number(pending.count),
        perRunRewardCap: config.perRunRewardCap,
        requestedDispatchCount:
          requested === null ? 0 : Number(requested.count),
        todayReservedSats,
      }
    },
    settleReward: input =>
      updateReward({
        evidenceRefs: input.evidenceRefs,
        nowIso: input.nowIso,
        reasonRef: null,
        rewardId: input.rewardId,
        state: 'settled',
      }),
  }
}

const jsonRecord = async (response: {
  json: () => Promise<unknown>
}): Promise<Record<string, unknown>> => {
  try {
    const body = await response.json()

    return typeof body === 'object' && body !== null
      ? (body as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

const paymentResultFromPayload = (
  response: { ok: boolean; status: number; json: () => Promise<unknown> },
  payload: Record<string, unknown>,
): TreasuryPaymentResult => {
  const status = typeof payload.status === 'string' ? payload.status : ''
  const paymentId =
    typeof payload.paymentId === 'string' && payload.paymentId.trim() !== ''
      ? payload.paymentId.trim()
      : null

  if (response.ok && status === 'succeeded' && paymentId !== null) {
    return { kind: 'succeeded', paymentId }
  }

  if (response.ok && status === 'pending' && paymentId !== null) {
    return { kind: 'pending', paymentId }
  }

  return {
    kind: 'failed',
    paymentId,
    reasonRef:
      status === 'failed'
        ? TreasuryPaymentFailedReasonRef
        : TreasuryPayFailedReasonRef,
  }
}

export const makeXClaimRewardTreasuryClient = (
  fetchTreasury: ContainerPathFetch | undefined,
): XClaimRewardTreasuryClient | null =>
  fetchTreasury === undefined
    ? null
    : {
        pay: async input => {
          try {
            const response = await fetchTreasury('/pay', {
              body: JSON.stringify({
                amountSat: input.amountSat,
                destination: input.destination,
                timeoutSecs: input.timeoutSecs,
              }),
              method: 'POST',
            })

            return paymentResultFromPayload(response, await jsonRecord(response))
          } catch {
            return {
              kind: 'failed',
              paymentId: null,
              reasonRef: TreasuryUnavailableReasonRef,
            }
          }
        },
        readBalance: async () => {
          try {
            const response = await fetchTreasury('/balance')

            if (!response.ok) {
              return {
                kind: 'unavailable',
                reasonRef: TreasuryBalanceUnavailableReasonRef,
              }
            }

            const payload = await jsonRecord(response)
            const maxSendableSat = payload.maxSendableSat

            return maxSendableSat === null || typeof maxSendableSat === 'number'
              ? { kind: 'ok', maxSendableSat }
              : {
                  kind: 'unavailable',
                  reasonRef: TreasuryBalanceUnavailableReasonRef,
                }
          } catch {
            return {
              kind: 'unavailable',
              reasonRef: TreasuryUnavailableReasonRef,
            }
          }
        },
        readPayment: async paymentId => {
          try {
            const response = await fetchTreasury(
              `/payments/${encodeURIComponent(paymentId)}`,
            )

            return paymentResultFromPayload(response, await jsonRecord(response))
          } catch {
            return {
              kind: 'failed',
              paymentId,
              reasonRef: TreasuryUnavailableReasonRef,
            }
          }
        },
      }

export const makeD1XClaimRewardRecipientResolver =
  (database: TreasuryDatabase) =>
  async (
    reward: XClaimRewardRecord,
  ): Promise<{ destination: string; destinationSourceRef: string } | null> => {
    if (reward.agentUserId === null) {
      return null
    }

    const row = await treasuryAuthorityDb(database)
      .prepare(
        `SELECT wallet_ref, bolt12_offer
           FROM forum_tip_recipient_wallets
          WHERE actor_ref = ?
            AND state = 'ready'
            AND archived_at IS NULL
            AND bolt12_offer IS NOT NULL
          LIMIT 1`,
      )
      .bind(`agent:${reward.agentUserId}`)
      .first<{
        wallet_ref: string
        bolt12_offer: string | null
      }>()

    return row === null
      ? null
      : {
          destination: row.bolt12_offer!,
          destinationSourceRef: row.wallet_ref,
        }
  }

const dispatchAttemptRef = (rewardId: string): string =>
  `dispatch_attempt.public.mdk_treasury.x_claim_reward_${rewardId}`

const settlementEvidenceRef = (rewardId: string): string =>
  `settlement_evidence.public.mdk_treasury.x_claim_reward_${rewardId}`

const settledReceiptRef = (rewardId: string): string =>
  `receipt.public.x_claim_reward.settled.${rewardId}`

const uniqueRefs = (
  refs: ReadonlyArray<string>,
  extraRefs: ReadonlyArray<string>,
): ReadonlyArray<string> => Array.from(new Set([...refs, ...extraRefs]))

const processPendingReward = async (
  dependencies: XClaimRewardTreasuryDispatcherDependencies,
  accumulator: DispatchAccumulator,
  reward: XClaimRewardRecord,
): Promise<DispatchAccumulator> => {
  if (reward.treasuryPaymentId === null || dependencies.treasury === null) {
    return {
      ...accumulator,
      skippedReasonRefs: uniqueRefs(accumulator.skippedReasonRefs, [
        TreasuryUnavailableReasonRef,
      ]),
    }
  }

  const payment = await dependencies.treasury.readPayment(reward.treasuryPaymentId)

  if (payment.kind === 'succeeded') {
    await dependencies.store.settleReward({
      evidenceRefs: uniqueRefs(reward.evidenceRefs, [
        settledReceiptRef(reward.id),
        settlementEvidenceRef(reward.id),
      ]),
      nowIso: dependencies.config.nowIso,
      rewardId: reward.id,
    })

    return {
      ...accumulator,
      polled: accumulator.polled + 1,
      settled: accumulator.settled + 1,
    }
  }

  if (payment.kind === 'failed' && payment.reasonRef !== TreasuryUnavailableReasonRef) {
    await dependencies.store.failReward({
      evidenceRefs: reward.evidenceRefs,
      nowIso: dependencies.config.nowIso,
      reasonRef: payment.reasonRef,
      rewardId: reward.id,
    })

    return {
      ...accumulator,
      failed: accumulator.failed + 1,
      polled: accumulator.polled + 1,
    }
  }

  return {
    ...accumulator,
    pending: accumulator.pending + 1,
    polled: accumulator.polled + 1,
  }
}

const processDispatchRequestedReward = async (
  dependencies: XClaimRewardTreasuryDispatcherDependencies,
  accumulator: DispatchAccumulator,
  reward: XClaimRewardRecord,
): Promise<DispatchAccumulator> => {
  if (accumulator.todayReservedSats + reward.amountSats > dependencies.config.dailySatsCap) {
    return {
      ...accumulator,
      skippedReasonRefs: uniqueRefs(accumulator.skippedReasonRefs, [
        'reason.public.x_claim_reward_treasury_daily_cap_reached',
      ]),
    }
  }

  const recipient = await dependencies.resolveRecipient(reward)

  if (recipient === null) {
    await dependencies.store.failReward({
      evidenceRefs: reward.evidenceRefs,
      nowIso: dependencies.config.nowIso,
      reasonRef: RecipientWalletMissingReasonRef,
      rewardId: reward.id,
    })

    return { ...accumulator, failed: accumulator.failed + 1 }
  }

  const claimed = await dependencies.store.claimDispatchRequestedReward({
    evidenceRefs: uniqueRefs(reward.evidenceRefs, [
      dispatchAttemptRef(reward.id),
      recipient.destinationSourceRef,
    ]),
    nowIso: dependencies.config.nowIso,
    rewardId: reward.id,
    stateReasonRef: DispatchStartedReasonRef,
  })

  if (claimed === undefined) {
    return accumulator
  }

  if (dependencies.treasury === null) {
    await dependencies.store.failReward({
      evidenceRefs: claimed.evidenceRefs,
      nowIso: dependencies.config.nowIso,
      reasonRef: TreasuryUnavailableReasonRef,
      rewardId: reward.id,
    })

    return {
      ...accumulator,
      failed: accumulator.failed + 1,
      requested: accumulator.requested + 1,
    }
  }

  const payment = await dependencies.treasury.pay({
    amountSat: X_CLAIM_REWARD_AMOUNT_SATS,
    destination: recipient.destination,
    timeoutSecs: dependencies.config.paymentTimeoutSecs,
  })

  const withPayment =
    payment.paymentId === null
      ? claimed
      : ((await dependencies.store.attachTreasuryPayment({
          evidenceRefs: claimed.evidenceRefs,
          nowIso: dependencies.config.nowIso,
          paymentId: payment.paymentId,
          rewardId: reward.id,
          stateReasonRef:
            payment.kind === 'pending' ? PaymentPendingReasonRef : claimed.stateReasonRef,
        })) ?? claimed)

  if (payment.kind === 'succeeded') {
    await dependencies.store.settleReward({
      evidenceRefs: uniqueRefs(withPayment.evidenceRefs, [
        settledReceiptRef(reward.id),
        settlementEvidenceRef(reward.id),
      ]),
      nowIso: dependencies.config.nowIso,
      rewardId: reward.id,
    })

    return {
      ...accumulator,
      requested: accumulator.requested + 1,
      settled: accumulator.settled + 1,
      todayReservedSats: accumulator.todayReservedSats + reward.amountSats,
    }
  }

  if (payment.kind === 'pending') {
    return {
      ...accumulator,
      pending: accumulator.pending + 1,
      requested: accumulator.requested + 1,
      todayReservedSats: accumulator.todayReservedSats + reward.amountSats,
    }
  }

  await dependencies.store.failReward({
    evidenceRefs: withPayment.evidenceRefs,
    nowIso: dependencies.config.nowIso,
    reasonRef: payment.reasonRef,
    rewardId: reward.id,
  })

  return {
    ...accumulator,
    failed: accumulator.failed + 1,
    requested: accumulator.requested + 1,
  }
}

const processSequentially = async <A>(
  items: ReadonlyArray<A>,
  accumulator: DispatchAccumulator,
  fn: (accumulator: DispatchAccumulator, item: A) => Promise<DispatchAccumulator>,
): Promise<DispatchAccumulator> => {
  const [head, ...tail] = items

  return head === undefined
    ? accumulator
    : processSequentially(tail, await fn(accumulator, head), fn)
}

export const runXClaimRewardTreasuryDispatch = async (
  dependencies: XClaimRewardTreasuryDispatcherDependencies,
): Promise<XClaimRewardTreasuryDispatchSummary> => {
  const dayStartIso = xClaimRewardDispatchDayStartIso(dependencies.config.nowIso)
  const stats = await dependencies.store.readDispatchStats(
    dayStartIso,
    dependencies.config,
  )
  const disabledSummary = {
    failed: 0,
    pending: 0,
    polled: 0,
    requested: 0,
    settled: 0,
    skippedReasonRefs: [],
    stats,
  } satisfies XClaimRewardTreasuryDispatchSummary

  if (!dependencies.config.enabled) {
    return disabledSummary
  }

  const pendingRewards =
    await dependencies.store.listPendingTreasuryPaymentRewards(
      dependencies.config.perRunRewardCap,
    )
  const afterPolling = await processSequentially(
    pendingRewards,
    {
      failed: 0,
      pending: 0,
      polled: 0,
      requested: 0,
      settled: 0,
      skippedReasonRefs: [],
      todayReservedSats: await dependencies.store.countTodayReservedSats(
        dayStartIso,
      ),
    },
    (accumulator, reward) =>
      processPendingReward(dependencies, accumulator, reward),
  )

  if (dependencies.treasury === null) {
    return {
      ...afterPolling,
      skippedReasonRefs: uniqueRefs(afterPolling.skippedReasonRefs, [
        TreasuryUnavailableReasonRef,
      ]),
      stats: await dependencies.store.readDispatchStats(
        dayStartIso,
        dependencies.config,
      ),
    }
  }

  const balance = await dependencies.treasury.readBalance()

  if (balance.kind !== 'ok') {
    return {
      ...afterPolling,
      skippedReasonRefs: uniqueRefs(afterPolling.skippedReasonRefs, [
        balance.reasonRef,
      ]),
      stats: await dependencies.store.readDispatchStats(
        dayStartIso,
        dependencies.config,
      ),
    }
  }

  const threshold =
    X_CLAIM_REWARD_AMOUNT_SATS + dependencies.config.liquidityBufferSats

  if ((balance.maxSendableSat ?? 0) < threshold) {
    return {
      ...afterPolling,
      skippedReasonRefs: uniqueRefs(afterPolling.skippedReasonRefs, [
        TreasuryLiquidityInsufficientReasonRef,
      ]),
      stats: await dependencies.store.readDispatchStats(
        dayStartIso,
        dependencies.config,
      ),
    }
  }

  const remainingSats =
    dependencies.config.dailySatsCap - afterPolling.todayReservedSats
  const newRewardLimit = Math.min(
    dependencies.config.perRunRewardCap,
    Math.floor(remainingSats / X_CLAIM_REWARD_AMOUNT_SATS),
  )

  if (newRewardLimit < 1) {
    return {
      ...afterPolling,
      skippedReasonRefs: uniqueRefs(afterPolling.skippedReasonRefs, [
        'reason.public.x_claim_reward_treasury_daily_cap_reached',
      ]),
      stats: await dependencies.store.readDispatchStats(
        dayStartIso,
        dependencies.config,
      ),
    }
  }

  const requestedRewards =
    await dependencies.store.listDispatchRequestedRewards(newRewardLimit)
  const afterDispatch = await processSequentially(
    requestedRewards,
    afterPolling,
    (accumulator, reward) =>
      processDispatchRequestedReward(dependencies, accumulator, reward),
  )

  return {
    failed: afterDispatch.failed,
    pending: afterDispatch.pending,
    polled: afterDispatch.polled,
    requested: afterDispatch.requested,
    settled: afterDispatch.settled,
    skippedReasonRefs: afterDispatch.skippedReasonRefs,
    stats: await dependencies.store.readDispatchStats(
      dayStartIso,
      dependencies.config,
    ),
  }
}

export type XClaimRewardSmokePreflightCheck = Readonly<{
  name: string
  ok: boolean
  reasonRef: string | null
}>

export type XClaimRewardSmokePreflightReport = Readonly<{
  blockingReasonRefs: ReadonlyArray<string>
  checks: ReadonlyArray<XClaimRewardSmokePreflightCheck>
  ready: boolean
}>

export type XClaimRewardSmokePreflightInput = Readonly<{
  balanceMaxSendableSat: number | null
  stats: XClaimRewardTreasuryDispatchStats
}>

/**
 * Evaluates whether the bounded treasury wallet and ledger are in a clean state
 * to run the first live single-reward dispatch smoke. This is a pure,
 * public-safe gate: it moves no funds and emits no payment material, only
 * aggregate readiness checks the operator confirms before enabling the live run.
 */
export const evaluateXClaimRewardSmokePreflight = (
  input: XClaimRewardSmokePreflightInput,
): XClaimRewardSmokePreflightReport => {
  const { balanceMaxSendableSat, stats } = input
  const liquidityThreshold =
    X_CLAIM_REWARD_AMOUNT_SATS + stats.liquidityBufferSats
  const dailyHeadroomSats = stats.dailySatsCap - stats.todayReservedSats

  const checks: ReadonlyArray<XClaimRewardSmokePreflightCheck> = [
    {
      name: 'dispatch_flag_enabled',
      ok: stats.enabled,
      reasonRef: stats.enabled ? null : DispatchDisabledReasonRef,
    },
    {
      name: 'per_run_cap_allows_one',
      ok: stats.perRunRewardCap >= 1,
      reasonRef: stats.perRunRewardCap >= 1 ? null : DailyCapReachedReasonRef,
    },
    {
      name: 'exactly_one_approved_reward',
      ok: stats.requestedDispatchCount === 1,
      reasonRef:
        stats.requestedDispatchCount === 1 ? null : NoApprovedRewardReasonRef,
    },
    {
      name: 'no_pending_payment_in_flight',
      ok: stats.pendingPaymentCount === 0,
      reasonRef:
        stats.pendingPaymentCount === 0 ? null : PaymentPendingReasonRef,
    },
    {
      name: 'daily_cap_headroom',
      ok: dailyHeadroomSats >= X_CLAIM_REWARD_AMOUNT_SATS,
      reasonRef:
        dailyHeadroomSats >= X_CLAIM_REWARD_AMOUNT_SATS
          ? null
          : DailyCapReachedReasonRef,
    },
    {
      name: 'treasury_liquidity_sufficient',
      ok:
        balanceMaxSendableSat !== null &&
        balanceMaxSendableSat >= liquidityThreshold,
      reasonRef:
        balanceMaxSendableSat === null
          ? TreasuryBalanceUnavailableReasonRef
          : balanceMaxSendableSat >= liquidityThreshold
            ? null
            : TreasuryLiquidityInsufficientReasonRef,
    },
  ]

  const blockingReasonRefs = Array.from(
    new Set(
      checks
        .filter(check => !check.ok && check.reasonRef !== null)
        .map(check => check.reasonRef as string),
    ),
  )

  return {
    blockingReasonRefs,
    checks,
    ready: blockingReasonRefs.length === 0,
  }
}

export const runXClaimRewardTreasuryDispatchScheduled = (
  db: TreasuryDatabase,
  input: Readonly<{
    config: XClaimRewardTreasuryDispatchConfig
    fetchTreasury: ContainerPathFetch | undefined
  }>,
): Effect.Effect<XClaimRewardTreasuryDispatchSummary> =>
  Effect.promise(() =>
    runXClaimRewardTreasuryDispatch({
      config: input.config,
      resolveRecipient: makeD1XClaimRewardRecipientResolver(db),
      store: makeD1XClaimRewardTreasuryDispatchStore(db),
      treasury: makeXClaimRewardTreasuryClient(input.fetchTreasury),
    }),
  )
