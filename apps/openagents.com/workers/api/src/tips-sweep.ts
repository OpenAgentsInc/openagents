import { Effect } from 'effect'

import {
  type LedgerStatement,
  createPayInStatements,
  markPayInFailedStatements,
  markPayInForwardingStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from './payments-ledger'
import { epochMillisToIsoTimestamp } from './runtime-primitives'

// The automated sweep worker (issue #4707; design:
// docs/payments/reliable-tips.md §3). On the worker cron: for each
// agent whose sweepable balance exceeds their threshold, attempt a
// Lightning payout of the excess to their registered destination - fee caps,
// a minimum, pending-sweep dedup, recent-failure backoff. Failures cost
// nothing: the funding debit refunds atomically and the next tick
// retries. Only a settled sweep makes credited value settled bitcoin.

export const TIPS_SWEEP_MIN_SAT = 100
export const TIPS_SWEEP_MAX_PER_TICK = 5
export const TIPS_SWEEP_FAILURE_BACKOFF_MINUTES = 30

export type SweepCandidate = Readonly<{
  actorRef: string
  // Available balance, excluding escrow-held claims.
  balanceMsat: number
  sweepThresholdSat: number
  // Registered source only: the wallet claim ref identifies the destination's
  // provenance; the raw destination string is used for payment and never
  // stored on ledger rows.
  walletClaimRef: string
  payoutDestination: string
}>

export const sweepAmountSat = (
  candidate: Pick<SweepCandidate, 'balanceMsat' | 'sweepThresholdSat'>,
): number => {
  const excessMsat = candidate.balanceMsat - candidate.sweepThresholdSat * 1000
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
      publicReceiptRef: null,
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
  | Readonly<{ ok: false; pending: true; paymentId: string }>
  | Readonly<{ ok: false; pending?: false; reason: string }>

export type BufferPayFn = (
  input: Readonly<{
    destination: string
    amountSat: number
  }>,
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

  // RL-3 asset boundary (#5497): the sweep is the live Bitcoin-withdrawal path,
  // so it pays out only the Bitcoin-WITHDRAWABLE balance:
  //   balance_msat - held_msat - usd_credit_msat
  // USD-purchased credit (`usd_credit_msat`) is inference-spendable but NEVER a
  // Bitcoin liability, so it is subtracted from the sweepable amount here and
  // from the threshold gate below. A USD credit can never leak into a sweep.
  const result = await db
    .prepare(
      `SELECT b.actor_ref,
              b.balance_msat - COALESCE(b.held_msat, 0)
                - COALESCE(b.usd_credit_msat, 0) AS available_balance_msat,
              b.sweep_threshold_sat,
              w.wallet_ref, w.lightning_address, w.bolt12_offer
         FROM agent_balances b
         JOIN forum_tip_recipient_wallets w
           ON w.actor_ref = b.actor_ref
          AND w.state = 'ready'
          AND w.archived_at IS NULL
          AND (w.lightning_address IS NOT NULL OR w.bolt12_offer IS NOT NULL)
        WHERE b.sweep_enabled = 1
          AND b.balance_msat - COALESCE(b.held_msat, 0)
                - COALESCE(b.usd_credit_msat, 0)
                >= (b.sweep_threshold_sat + ?) * 1000
          AND NOT EXISTS (
            SELECT 1 FROM pay_ins p
             WHERE p.pay_in_type = 'sweep'
               AND p.payer_ref = b.actor_ref
               AND (
                 p.state IN ('pending', 'forwarding')
                 OR (p.state = 'failed' AND p.state_changed_at > ?)
               )
          )
        ORDER BY available_balance_msat DESC
        LIMIT ?`,
    )
    .bind(TIPS_SWEEP_MIN_SAT, backoffCutoff, limit)
    .all()

  return ((result.results ?? []) as Array<Record<string, unknown>>).map(row => {
    const lightningAddress =
      typeof row.lightning_address === 'string' &&
      row.lightning_address.trim() !== ''
        ? row.lightning_address.trim()
        : null
    return {
      actorRef: String(row.actor_ref),
      balanceMsat: Number(row.available_balance_msat ?? row.balance_msat),
      payoutDestination: lightningAddress ?? String(row.bolt12_offer),
      sweepThresholdSat: Number(row.sweep_threshold_sat),
      walletClaimRef: String(row.wallet_ref),
    }
  })
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
      destination: candidate.payoutDestination,
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
    } else if (payResult.pending === true) {
      // #4710: a pending buffer payment HOLDS the debit in forwarding.
      // The reconciliation pass polls the buffer until the outcome is
      // known; refunding now risks paying the recipient twice.
      await runLedgerStatements(db, [
        {
          params: [`pending:${payResult.paymentId}`, plan.payoutLegId],
          sql: `UPDATE pay_in_legs
                SET external_ref = external_ref || '|' || ?
                WHERE id = ?`,
        },
      ])
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

// Backing invariant (issue #4708): sum of all agent balances must not
// exceed the buffer wallet's balance. Checked every tick; a violation
// raises (captured by the scheduled observer) instead of passing
// silently.
export const checkTipsBufferBackingInvariant = async (
  db: D1Database,
  fetchBufferBalance: () => Promise<number | null>,
): Promise<
  Readonly<{
    ok: boolean
    agentBalancesSat: number
    bufferBalanceSat: number | null
  }>
> => {
  const row = await db
    .prepare(
      'SELECT COALESCE(SUM(balance_msat), 0) AS total FROM agent_balances',
    )
    .first()
  const agentBalancesSat = Math.ceil(
    Number((row as { total?: unknown } | null)?.total ?? 0) / 1000,
  )
  const bufferBalanceSat = await fetchBufferBalance()

  const ok =
    bufferBalanceSat === null
      ? agentBalancesSat === 0
      : agentBalancesSat <= bufferBalanceSat

  if (!ok) {
    throw new Error(
      `tips_buffer_backing_violated: agent balances ${agentBalancesSat} sat exceed buffer ${bufferBalanceSat ?? 'unconfigured'} sat`,
    )
  }

  return { agentBalancesSat, bufferBalanceSat, ok }
}

// #4710: reconcile forwarding pay-ins whose buffer payment outcome was
// unknown at send time. completed -> paid (ref stamped); failed ->
// refund, and for ladder tips also pay the credited fallback so the tip
// still never fails; still-pending -> wait for the next tick.
export const reconcileForwardingBufferPayments = async (
  db: D1Database,
  deps: Readonly<{
    fetchBufferPaymentStatus: (
      paymentId: string,
    ) => Promise<'succeeded' | 'failed' | 'pending'>
    makeId: () => string
    nowIso: string
  }>,
): Promise<
  Readonly<{ settled: number; refunded: number; waiting: number }>
> => {
  const rows = ((
    await db
      .prepare(
        `SELECT p.id AS pay_in_id, p.pay_in_type, p.payer_ref, p.cost_msat,
                  p.context_ref, p.idempotency_key, p.public_receipt_ref,
                  l.id AS leg_id, l.external_ref,
                  fin.id AS funding_leg_id, fin.party_ref AS funding_party_ref
             FROM pay_ins p
             JOIN pay_in_legs l
               ON l.pay_in_id = p.id AND l.kind = 'lightning'
              AND l.direction = 'out' AND l.external_ref LIKE '%|pending:%'
        LEFT JOIN pay_in_legs fin
               ON fin.pay_in_id = p.id AND fin.kind = 'balance'
              AND fin.direction = 'in' AND fin.refund_of_leg_id IS NULL
            WHERE p.state = 'forwarding'
            LIMIT 10`,
      )
      .all()
  ).results ?? []) as Array<Record<string, unknown>>

  let settled = 0
  let refunded = 0
  let waiting = 0

  for (const row of rows) {
    const externalRef = String(row.external_ref)
    const paymentId = externalRef.slice(
      externalRef.indexOf('|pending:') + '|pending:'.length,
    )
    const status = await deps.fetchBufferPaymentStatus(paymentId)

    if (status === 'pending') {
      waiting += 1
      continue
    }

    const payInId = String(row.pay_in_id)
    if (status === 'succeeded') {
      await runLedgerStatements(db, [
        ...markPayInPaidStatements(
          { balancePayoutLegs: [], payInId },
          deps.nowIso,
        ),
        {
          params: [
            `payment.tips_buffer.${paymentId.slice(0, 12)}`,
            String(row.leg_id),
          ],
          sql: `UPDATE pay_in_legs
                SET external_ref = external_ref || '|' || ?
                WHERE id = ?`,
        },
      ])
      settled += 1
      continue
    }

    // failed: refund the funding debit...
    const fundingLegId = row.funding_leg_id
    const fundingPartyRef = row.funding_party_ref
    if (fundingLegId !== null && fundingPartyRef !== null) {
      await runLedgerStatements(
        db,
        markPayInFailedStatements(
          {
            balanceFundingLegs: [
              {
                amountMsat: Number(row.cost_msat),
                legId: String(fundingLegId),
                partyRef: String(fundingPartyRef),
                refundLegId: deps.makeId(),
              },
            ],
            failureReason: 'buffer_pay_failed_after_forwarding',
            payInId,
          },
          deps.nowIso,
        ),
      )
      refunded += 1

      // ...and for ladder tips, the recipient still gets paid: the
      // credited fallback rides the same ledger, so the tip never fails.
      if (
        String(row.pay_in_type) === 'tip' &&
        String(row.context_ref ?? '').startsWith('forum.post.')
      ) {
        const recipientRow = (await db
          .prepare(
            `SELECT party_ref FROM pay_in_legs
              WHERE pay_in_id = ? AND kind = 'lightning' AND direction = 'out'`,
          )
          .bind(payInId)
          .first()) as { party_ref: string } | null
        if (recipientRow !== null) {
          const fallbackPayInId = deps.makeId()
          const fundingLeg = deps.makeId()
          const payoutLeg = deps.makeId()
          const amountMsat = Number(row.cost_msat)
          const postId = String(row.context_ref).replace('forum.post.', '')
          const { creditedTipStatements } = await import('./tip-ladder')
          await runLedgerStatements(
            db,
            creditedTipStatements(
              {
                amountSat: Math.floor(amountMsat / 1000),
                fundingLegId: fundingLeg,
                idempotencyKey: `${String(row.idempotency_key)}:reconciled_fallback`,
                ladderReason: 'direct_attempt_failed',
                payInId: fallbackPayInId,
                payoutLegId: payoutLeg,
                postId,
                publicReceiptRef:
                  typeof row.public_receipt_ref === 'string'
                    ? row.public_receipt_ref
                    : null,
                recipientRef: recipientRow.party_ref,
                senderRef: String(row.payer_ref),
              },
              deps.nowIso,
            ),
          )
        }
      }
    }
  }

  return { refunded, settled, waiting }
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
