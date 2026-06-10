import { Effect } from 'effect'

import { epochMillisToIsoTimestamp } from './runtime-primitives'
import {
  type LedgerStatement,
  createPayInStatements,
  markPayInFailedStatements,
  markPayInForwardingStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from './payments-ledger'

// The automated sweep worker (issue #4707; design:
// docs/payments/reliable-tips.md §3). On the worker cron: for each
// agent whose sweepable balance exceeds their threshold, attempt a
// Lightning payout of the excess to their REGISTERED offer - fee caps,
// a minimum, pending-sweep dedup, recent-failure backoff. Failures cost
// nothing: the funding debit refunds atomically and the next tick
// retries. Only a settled sweep makes credited value settled bitcoin.

export const TIPS_SWEEP_MIN_SAT = 100
export const TIPS_SWEEP_MAX_PER_TICK = 5
export const TIPS_SWEEP_FAILURE_BACKOFF_MINUTES = 30

export type SweepCandidate = Readonly<{
  actorRef: string
  balanceMsat: number
  sweepThresholdSat: number
  // Registered source only: the wallet claim ref identifies the offer's
  // provenance; the raw offer string is used for the payment and never
  // stored on ledger rows.
  walletClaimRef: string
  bolt12Offer: string
}>

export const sweepAmountSat = (
  candidate: Pick<SweepCandidate, 'balanceMsat' | 'sweepThresholdSat'>,
): number => {
  const excessMsat =
    candidate.balanceMsat - candidate.sweepThresholdSat * 1000
  const amountSat = Math.floor(excessMsat / 1000)
  return amountSat >= TIPS_SWEEP_MIN_SAT ? amountSat : 0
}

export type SweepPlan = Readonly<{
  payInId: string
  fundingLegId: string
  payoutLegId: string
  actorRef: string
  amountSat: number
  walletClaimRef: string
  idempotencyKey: string
}>

export const sweepCreateStatements = (
  plan: SweepPlan,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  const amountMsat = plan.amountSat * 1000

  return createPayInStatements(
    {
      contextRef: `sweep.${plan.actorRef}`,
      costMsat: amountMsat,
      genesisId: null,
      idempotencyKey: plan.idempotencyKey,
      legs: [
        {
          amountMsat,
          direction: 'in',
          externalRef: null,
          kind: 'balance',
          legId: plan.fundingLegId,
          partyRef: plan.actorRef,
        },
        {
          amountMsat,
          direction: 'out',
          externalRef: plan.walletClaimRef,
          kind: 'lightning',
          legId: plan.payoutLegId,
          partyRef: plan.actorRef,
        },
      ],
      payInId: plan.payInId,
      payInType: 'sweep',
      payerRef: plan.actorRef,
      rung: null,
    },
    nowIso,
  )
}

export type BufferPayResult =
  | Readonly<{ ok: true; paymentRef: string }>
  | Readonly<{ ok: false; reason: string }>

export type BufferPayFn = (
  input: Readonly<{ bolt12Offer: string; amountSat: number }>,
) => Promise<BufferPayResult>

export type SweepTickOutcome = Readonly<{
  attempted: number
  settled: number
  failed: number
  skippedReason: string | null
}>

export const selectSweepCandidates = async (
  db: D1Database,
  nowIso: string,
  limit: number = TIPS_SWEEP_MAX_PER_TICK,
): Promise<ReadonlyArray<SweepCandidate>> => {
  const backoffCutoff = epochMillisToIsoTimestamp(
    Date.parse(nowIso) - TIPS_SWEEP_FAILURE_BACKOFF_MINUTES * 60_000,
  )

  const result = await db
    .prepare(
      `SELECT b.actor_ref, b.balance_msat, b.sweep_threshold_sat,
              w.wallet_ref, w.bolt12_offer
         FROM agent_balances b
         JOIN forum_tip_recipient_wallets w
           ON w.actor_ref = b.actor_ref
          AND w.state = 'ready'
          AND w.archived_at IS NULL
          AND w.bolt12_offer IS NOT NULL
        WHERE b.sweep_enabled = 1
          AND b.balance_msat >= (b.sweep_threshold_sat + ?) * 1000
          AND NOT EXISTS (
            SELECT 1 FROM pay_ins p
             WHERE p.pay_in_type = 'sweep'
               AND p.payer_ref = b.actor_ref
               AND (
                 p.state IN ('pending', 'forwarding')
                 OR (p.state = 'failed' AND p.state_changed_at > ?)
               )
          )
        ORDER BY b.balance_msat DESC
        LIMIT ?`,
    )
    .bind(TIPS_SWEEP_MIN_SAT, backoffCutoff, limit)
    .all()

  return ((result.results ?? []) as Array<Record<string, unknown>>).map(
    row => ({
      actorRef: String(row.actor_ref),
      balanceMsat: Number(row.balance_msat),
      bolt12Offer: String(row.bolt12_offer),
      sweepThresholdSat: Number(row.sweep_threshold_sat),
      walletClaimRef: String(row.wallet_ref),
    }),
  )
}

export const runTipsSweepTick = async (
  db: D1Database,
  deps: Readonly<{
    payFromBuffer: BufferPayFn | null
    makeId: () => string
    nowIso: string
    maxPerTick?: number
  }>,
): Promise<SweepTickOutcome> => {
  if (deps.payFromBuffer === null) {
    return {
      attempted: 0,
      failed: 0,
      settled: 0,
      skippedReason: 'tips_buffer_unconfigured',
    }
  }

  const candidates = await selectSweepCandidates(
    db,
    deps.nowIso,
    deps.maxPerTick ?? TIPS_SWEEP_MAX_PER_TICK,
  )

  let settled = 0
  let failed = 0

  for (const candidate of candidates) {
    const amountSat = sweepAmountSat(candidate)
    if (amountSat <= 0) {
      continue
    }

    const plan: SweepPlan = {
      actorRef: candidate.actorRef,
      amountSat,
      fundingLegId: deps.makeId(),
      idempotencyKey: `sweep:${candidate.actorRef}:${deps.nowIso}`,
      payInId: deps.makeId(),
      payoutLegId: deps.makeId(),
      walletClaimRef: candidate.walletClaimRef,
    }

    await runLedgerStatements(db, [
      ...sweepCreateStatements(plan, deps.nowIso),
      ...markPayInForwardingStatements(plan.payInId, deps.nowIso),
    ])

    const payResult = await deps.payFromBuffer({
      amountSat,
      bolt12Offer: candidate.bolt12Offer,
    })

    if (payResult.ok) {
      await runLedgerStatements(db, [
        ...markPayInPaidStatements(
          { balancePayoutLegs: [], payInId: plan.payInId },
          deps.nowIso,
        ),
        {
          params: [payResult.paymentRef, plan.payoutLegId],
          sql: `UPDATE pay_in_legs
                SET external_ref = external_ref || '|' || ?
                WHERE id = ?`,
        },
      ])
      settled += 1
    } else {
      await runLedgerStatements(
        db,
        markPayInFailedStatements(
          {
            balanceFundingLegs: [
              {
                amountMsat: amountSat * 1000,
                legId: plan.fundingLegId,
                partyRef: candidate.actorRef,
                refundLegId: deps.makeId(),
              },
            ],
            failureReason: `buffer_pay_failed:${payResult.reason}`.slice(
              0,
              120,
            ),
            payInId: plan.payInId,
          },
          deps.nowIso,
        ),
      )
      failed += 1
    }
  }

  return {
    attempted: candidates.length,
    failed,
    settled,
    skippedReason: null,
  }
}

export const runTipsSweepScheduled = (
  db: D1Database,
  deps: Readonly<{
    payFromBuffer: BufferPayFn | null
    makeId: () => string
    nowIso: string
  }>,
): Effect.Effect<SweepTickOutcome, never> =>
  Effect.tryPromise({
    catch: () => 'sweep_tick_error' as const,
    try: () => runTipsSweepTick(db, deps),
  }).pipe(
    Effect.catch(reason =>
      Effect.succeed({
        attempted: 0,
        failed: 0,
        settled: 0,
        skippedReason: reason,
      } satisfies SweepTickOutcome),
    ),
  )
