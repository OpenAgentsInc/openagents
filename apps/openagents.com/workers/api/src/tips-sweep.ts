import { Effect, Schema as S } from 'effect'

import {
  type LedgerStatement,
  createPayInStatements,
  markPayInFailedStatements,
  markPayInForwardingStatements,
  markPayInPaidStatements,
  runLedgerStatements,
  sumAgentBalancesMsat,
} from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { epochMillisToIsoTimestamp } from './runtime-primitives'
import {
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

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

export class TipsBufferBackingViolation extends S.TaggedErrorClass<TipsBufferBackingViolation>()(
  'TipsBufferBackingViolation',
  {
    agentBalancesSat: S.Number,
    bufferBalanceSat: S.NullOr(S.Number),
    message: S.String,
  },
) {}

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
  database: TreasuryDatabase,
  ledgerDb: PaymentsLedgerDb,
  nowIso: string,
  limit: number = TIPS_SWEEP_MAX_PER_TICK,
): Promise<ReadonlyArray<SweepCandidate>> => {
  const backoffCutoff = epochMillisToIsoTimestamp(
    Date.parse(nowIso) - TIPS_SWEEP_FAILURE_BACKOFF_MINUTES * 60_000,
  )

  // CFG-4 (#8519): `agent_balances`/`pay_ins` are Cloud SQL
  // Postgres-authoritative, while the recipient wallet registry
  // (`forum_tip_recipient_wallets`) stays on the D1 treasury authority. The
  // old single D1 JOIN is therefore split into two single-store reads (a
  // NON-ATOMIC cross-store seam): the ledger scan below picks the sweepable
  // balances, then the D1 wallet lookup filters to ready destinations. Both
  // sides are read-only and the sweep itself re-verifies funds atomically
  // (the funding debit is CHECK-guarded inside the ledger transaction), so
  // a row changing between the two reads costs nothing.
  //
  // RL-3 asset boundary (#5497): the sweep is the live Bitcoin-withdrawal path,
  // so it pays out only the Bitcoin-WITHDRAWABLE balance:
  //   balance_msat - held_msat - usd_credit_msat
  // USD-purchased credit (`usd_credit_msat`) is inference-spendable but NEVER a
  // Bitcoin liability, so it is subtracted from the sweepable amount here and
  // from the threshold gate below. A USD credit can never leak into a sweep.
  //
  // The balance scan is unbounded by design: the WHERE clause already
  // restricts it to sweep-enabled agents above threshold without pending
  // sweeps — a naturally small set. The per-tick `limit` applies after the
  // wallet-readiness filter, exactly as the old JOIN's LIMIT did.
  const balanceRows = await ledgerDb.query(
    `SELECT b.actor_ref,
            b.balance_msat - COALESCE(b.held_msat, 0)
              - COALESCE(b.usd_credit_msat, 0) AS available_balance_msat,
            b.sweep_threshold_sat
       FROM agent_balances b
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
      ORDER BY available_balance_msat DESC`,
    [TIPS_SWEEP_MIN_SAT, backoffCutoff],
  )

  if (balanceRows.length === 0) {
    return []
  }

  // Ready payout destinations from the D1 treasury authority, chunked for
  // the D1 bound-parameter limit.
  const db = treasuryAuthorityDb(database)
  const actorRefs = balanceRows.map(row => String(row.actor_ref))
  const walletsByActorRef = new Map<string, Record<string, unknown>>()
  const WALLET_PARAM_CHUNK = 90
  for (let index = 0; index < actorRefs.length; index += WALLET_PARAM_CHUNK) {
    const chunk = actorRefs.slice(index, index + WALLET_PARAM_CHUNK)
    const walletResult = await db
      .prepare(
        `SELECT w.actor_ref, w.wallet_ref, w.lightning_address, w.bolt12_offer
           FROM forum_tip_recipient_wallets w
          WHERE w.actor_ref IN (${chunk.map(() => '?').join(', ')})
            AND w.state = 'ready'
            AND w.archived_at IS NULL
            AND (w.lightning_address IS NOT NULL OR w.bolt12_offer IS NOT NULL)`,
      )
      .bind(...chunk)
      .all()

    for (const row of (walletResult.results ?? []) as Array<
      Record<string, unknown>
    >) {
      if (!walletsByActorRef.has(String(row.actor_ref))) {
        walletsByActorRef.set(String(row.actor_ref), row)
      }
    }
  }

  const candidates: SweepCandidate[] = []
  for (const row of balanceRows) {
    if (candidates.length >= limit) {
      break
    }
    const wallet = walletsByActorRef.get(String(row.actor_ref))
    if (wallet === undefined) {
      continue
    }
    const lightningAddress =
      typeof wallet.lightning_address === 'string' &&
      wallet.lightning_address.trim() !== ''
        ? wallet.lightning_address.trim()
        : null
    candidates.push({
      actorRef: String(row.actor_ref),
      // Postgres returns bigint msat columns as strings; Number(...) decodes.
      balanceMsat: Number(row.available_balance_msat),
      payoutDestination: lightningAddress ?? String(wallet.bolt12_offer),
      sweepThresholdSat: Number(row.sweep_threshold_sat),
      walletClaimRef: String(wallet.wallet_ref),
    })
  }

  return candidates
}

export const runTipsSweepTick = async (
  // Wallet-registry reads (forum_tip_recipient_wallets) stay on the D1
  // treasury seam; all credits-table reads/writes ride `deps.ledgerDb`.
  db: TreasuryDatabase,
  deps: Readonly<{
    /** CFG-4 (#8519): the Postgres-authoritative credits ledger handle. */
    ledgerDb: PaymentsLedgerDb
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
    deps.ledgerDb,
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

    await runLedgerStatements(deps.ledgerDb, [
      ...sweepCreateStatements(plan, deps.nowIso),
      ...markPayInForwardingStatements(plan.payInId, deps.nowIso),
    ])

    const payResult = await deps.payFromBuffer({
      amountSat,
      destination: candidate.payoutDestination,
    })

    if (payResult.ok) {
      await runLedgerStatements(deps.ledgerDb, [
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
      await runLedgerStatements(deps.ledgerDb, [
        {
          params: [`pending:${payResult.paymentId}`, plan.payoutLegId],
          sql: `UPDATE pay_in_legs
                SET external_ref = external_ref || '|' || ?
                WHERE id = ?`,
        },
      ])
    } else {
      await runLedgerStatements(
        deps.ledgerDb,
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
  // CFG-4 (#8519): `agent_balances` is Postgres-authoritative; the SUM
  // reads the ledger handle directly. The old KS-8.8 flag-routed
  // treasuryRead (D1 authority + compare probe) is gone with the D1 rows.
  ledgerDb: PaymentsLedgerDb,
  fetchBufferBalance: () => Promise<number | null>,
): Promise<
  Readonly<{
    ok: boolean
    agentBalancesSat: number
    bufferBalanceSat: number | null
  }>
> => {
  const totalMsat = await sumAgentBalancesMsat(ledgerDb)
  const agentBalancesSat = Math.ceil(totalMsat / 1000)
  const bufferBalanceSat = await fetchBufferBalance()

  const ok =
    bufferBalanceSat === null
      ? agentBalancesSat === 0
      : agentBalancesSat <= bufferBalanceSat

  if (!ok) {
    throw new TipsBufferBackingViolation({
      agentBalancesSat,
      bufferBalanceSat,
      message: `tips_buffer_backing_violated: agent balances ${agentBalancesSat} sat exceed buffer ${bufferBalanceSat ?? 'unconfigured'} sat`,
    })
  }

  return { agentBalancesSat, bufferBalanceSat, ok }
}

// #4710: reconcile forwarding pay-ins whose buffer payment outcome was
// unknown at send time. completed -> paid (ref stamped); failed ->
// refund, and for ladder tips also pay the credited fallback so the tip
// still never fails; still-pending -> wait for the next tick.
export const reconcileForwardingBufferPayments = async (
  // CFG-4 (#8519): every table this reconcile touches (pay_ins,
  // pay_in_legs, agent_balances) is a credits table, so the whole pass
  // runs on the Postgres-authoritative ledger handle — no treasury/D1
  // seam remains here.
  ledgerDb: PaymentsLedgerDb,
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
  const rows = (await ledgerDb.query(
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
  )) as Array<Record<string, unknown>>

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
      await runLedgerStatements(ledgerDb, [
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
        ledgerDb,
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
        const recipientRows = await ledgerDb.query(
          `SELECT party_ref FROM pay_in_legs
            WHERE pay_in_id = ? AND kind = 'lightning' AND direction = 'out'`,
          [payInId],
        )
        const recipientRow =
          (recipientRows[0] as { party_ref: string } | undefined) ?? null
        if (recipientRow !== null) {
          const fallbackPayInId = deps.makeId()
          const fundingLeg = deps.makeId()
          const payoutLeg = deps.makeId()
          const amountMsat = Number(row.cost_msat)
          const postId = String(row.context_ref).replace('forum.post.', '')
          const { creditedTipStatements } = await import('./tip-ladder')
          await runLedgerStatements(
            ledgerDb,
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
  db: TreasuryDatabase,
  deps: Readonly<{
    /** CFG-4 (#8519): the Postgres-authoritative credits ledger handle. */
    ledgerDb: PaymentsLedgerDb
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
