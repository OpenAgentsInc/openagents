import { Schema as S } from 'effect'

import type { PaymentsLedgerDb } from './payments-ledger-db'

// Agent credit ledger (issue #4705).
// Design contract: docs/payments/reliable-tips.md. Every paid attempt is
// one pay_ins row created atomically (one Postgres transaction) with the
// legs that fund it and the legs that say where value goes. Balances move
// only by increment/decrement; balance-touching legs store the resulting
// balance via a sequential-statement subquery inside the same
// transaction; FAILED refunds funding debits atomically; retries chain
// through genesis/successor with a set-if-null optimistic lock.
//
// CFG-4 (#8519): this domain is Cloud SQL Postgres-authoritative. The
// D1 batch executor, the fail-soft dual-write mirrors, and the
// `KHALA_SYNC_*` read flags for these tables are GONE — `PaymentsLedgerDb`
// (`payments-ledger-db.ts`) is the only store.

export const PayInType = S.Literals([
  'tip',
  'sweep',
  'buffer_funding',
  'reward',
  'adjustment',
  // USD-purchased, inference-spendable credit grant (#5497). Funds the msat
  // `agent_balances` ledger from a card (Stripe) USD purchase. The granted msat
  // is tracked as USD-origin (`agent_balances.usd_credit_msat`) so the RL-3
  // asset boundary keeps it inference-spendable but NOT Bitcoin-withdrawable.
  'usd_credit_grant',
])
export type PayInType = typeof PayInType.Type

export const PayInState = S.Literals([
  'pending',
  'forwarding',
  'paid',
  'failed',
])
export type PayInState = typeof PayInState.Type

export const PayInRung = S.Literals([
  'credited',
  'direct_bolt12',
  'direct_lightning',
])
export type PayInRung = typeof PayInRung.Type

export const PayInLegDirection = S.Literals(['in', 'out'])
export type PayInLegDirection = typeof PayInLegDirection.Type

export const PayInLegKind = S.Literals(['balance', 'lightning'])
export type PayInLegKind = typeof PayInLegKind.Type

const allowedTransitions: Readonly<Record<PayInState, readonly PayInState[]>> =
  {
    failed: [],
    forwarding: ['paid', 'failed'],
    paid: [],
    pending: ['forwarding', 'paid', 'failed'],
  }

export const payInTransitionAllowed = (
  from: PayInState,
  to: PayInState,
): boolean => allowedTransitions[from].includes(to)

export type LedgerStatement = Readonly<{
  sql: string
  params: ReadonlyArray<string | number | null>
}>

export type PayInLegPlan = Readonly<{
  legId: string
  direction: PayInLegDirection
  kind: PayInLegKind
  partyRef: string
  amountMsat: number
  externalRef: string | null
}>

export type PayInPlan = Readonly<{
  payInId: string
  payInType: PayInType
  payerRef: string
  costMsat: number
  rung: PayInRung | null
  contextRef: string | null
  idempotencyKey: string
  publicReceiptRef: string | null
  genesisId: string | null
  legs: ReadonlyArray<PayInLegPlan>
}>

export class PayInPlanError extends Error {
  override readonly name = 'PayInPlanError'
}

const assertPlanInvariants = (plan: PayInPlan): void => {
  if (plan.costMsat <= 0 || !Number.isInteger(plan.costMsat)) {
    throw new PayInPlanError('pay-in cost must be a positive integer msat')
  }

  for (const leg of plan.legs) {
    if (leg.amountMsat <= 0 || !Number.isInteger(leg.amountMsat)) {
      throw new PayInPlanError('leg amounts must be positive integer msats')
    }
  }

  const inMsat = plan.legs
    .filter(leg => leg.direction === 'in')
    .reduce((sum, leg) => sum + leg.amountMsat, 0)

  if (inMsat !== plan.costMsat) {
    throw new PayInPlanError('funding legs must cover the pay-in cost exactly')
  }

  const outMsat = plan.legs
    .filter(leg => leg.direction === 'out')
    .reduce((sum, leg) => sum + leg.amountMsat, 0)

  if (outMsat > plan.costMsat) {
    throw new PayInPlanError('payout legs may not exceed the pay-in cost')
  }
}

const ensureBalanceRowStatement = (
  partyRef: string,
  nowIso: string,
): LedgerStatement => ({
  params: [partyRef, nowIso, nowIso],
  sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
        VALUES (?, 0, ?, ?)
        ON CONFLICT (actor_ref) DO NOTHING`,
})

const balanceDebitStatement = (
  partyRef: string,
  amountMsat: number,
  nowIso: string,
): LedgerStatement => ({
  // The CHECK (balance_msat >= 0) constraint aborts the whole batch on
  // insufficient funds - atomic insufficient-balance failure by design.
  params: [amountMsat, nowIso, partyRef],
  sql: `UPDATE agent_balances
        SET balance_msat = balance_msat - ?, updated_at = ?
        WHERE actor_ref = ?`,
})

const balanceCreditStatement = (
  partyRef: string,
  amountMsat: number,
  nowIso: string,
): LedgerStatement => ({
  params: [amountMsat, nowIso, partyRef],
  sql: `UPDATE agent_balances
        SET balance_msat = balance_msat + ?, updated_at = ?
        WHERE actor_ref = ?`,
})

const insertLegStatement = (
  payInId: string,
  leg: PayInLegPlan,
  nowIso: string,
  options?: Readonly<{ refundOfLegId?: string }>,
): LedgerStatement =>
  leg.kind === 'balance'
    ? {
        // Sequential statements inside one D1 batch share the
        // transaction, so this subquery reads the balance as updated by
        // the preceding debit/credit statement - the resulting balance
        // is captured atomically.
        params: [
          leg.legId,
          payInId,
          leg.direction,
          leg.kind,
          leg.partyRef,
          leg.amountMsat,
          leg.partyRef,
          leg.externalRef,
          options?.refundOfLegId ?? null,
          nowIso,
        ],
        sql: `INSERT INTO pay_in_legs
              (id, pay_in_id, direction, kind, party_ref, amount_msat,
               resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
              VALUES (?, ?, ?, ?, ?, ?,
                      (SELECT balance_msat FROM agent_balances WHERE actor_ref = ?),
                      ?, ?, ?)`,
      }
    : {
        params: [
          leg.legId,
          payInId,
          leg.direction,
          leg.kind,
          leg.partyRef,
          leg.amountMsat,
          leg.externalRef,
          options?.refundOfLegId ?? null,
          nowIso,
        ],
        sql: `INSERT INTO pay_in_legs
              (id, pay_in_id, direction, kind, party_ref, amount_msat,
               resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
              VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      }

// Creating a pay-in atomically: insert the row, debit every balance
// funding leg (constraint-guarded), and record every leg with its
// resulting balance. Balance payout legs are NOT credited here - credits
// land when the pay-in is marked paid, mirroring SN's lifecycle.
export const createPayInStatements = (
  plan: PayInPlan,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  assertPlanInvariants(plan)

  const statements: LedgerStatement[] = [
    {
      params: [
        plan.payInId,
        plan.payInType,
        plan.payerRef,
        plan.costMsat,
        plan.rung,
        plan.contextRef,
        plan.idempotencyKey,
        plan.publicReceiptRef,
        plan.genesisId,
        nowIso,
        nowIso,
      ],
      sql: `INSERT INTO pay_ins
            (id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
             idempotency_key, public_receipt_ref, genesis_id, created_at,
             state_changed_at)
            VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)`,
    },
  ]

  for (const leg of plan.legs) {
    if (leg.kind === 'balance' && leg.direction === 'in') {
      statements.push(ensureBalanceRowStatement(leg.partyRef, nowIso))
      statements.push(
        balanceDebitStatement(leg.partyRef, leg.amountMsat, nowIso),
      )
      statements.push(insertLegStatement(plan.payInId, leg, nowIso))
    } else if (leg.kind === 'balance' && leg.direction === 'out') {
      // Payout credit deferred to paid-time; record the intent leg with
      // no resulting balance yet.
      statements.push({
        params: [
          leg.legId,
          plan.payInId,
          leg.direction,
          leg.kind,
          leg.partyRef,
          leg.amountMsat,
          leg.externalRef,
          nowIso,
        ],
        sql: `INSERT INTO pay_in_legs
              (id, pay_in_id, direction, kind, party_ref, amount_msat,
               resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
              VALUES (?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?)`,
      })
    } else {
      statements.push(insertLegStatement(plan.payInId, leg, nowIso))
    }
  }

  return statements
}

// Marking paid atomically: state transition guarded in SQL (WHERE state
// IN pending/forwarding), then credit every balance payout leg and
// stamp its resulting balance.
export const markPayInPaidStatements = (
  input: Readonly<{
    payInId: string
    balancePayoutLegs: ReadonlyArray<
      Readonly<{ legId: string; partyRef: string; amountMsat: number }>
    >
    rung?: PayInRung
  }>,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  const statements: LedgerStatement[] = [
    {
      params: [nowIso, input.rung ?? null, input.payInId],
      sql: `UPDATE pay_ins
            SET state = 'paid', state_changed_at = ?, rung = COALESCE(?, rung)
            WHERE id = ? AND state IN ('pending', 'forwarding')`,
    },
  ]

  for (const leg of input.balancePayoutLegs) {
    statements.push(ensureBalanceRowStatement(leg.partyRef, nowIso))
    statements.push(
      balanceCreditStatement(leg.partyRef, leg.amountMsat, nowIso),
    )
    statements.push({
      params: [leg.partyRef, leg.legId],
      sql: `UPDATE pay_in_legs
            SET resulting_balance_msat =
              (SELECT balance_msat FROM agent_balances WHERE actor_ref = ?)
            WHERE id = ?`,
    })
  }

  return statements
}

// Marking failed atomically: state transition plus a compensating refund
// leg for every balance funding debit. The refund credits the payer back
// and records itself as a leg pointing at the leg it reverses.
export const markPayInFailedStatements = (
  input: Readonly<{
    payInId: string
    failureReason: string
    balanceFundingLegs: ReadonlyArray<
      Readonly<{
        legId: string
        refundLegId: string
        partyRef: string
        amountMsat: number
      }>
    >
  }>,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  const statements: LedgerStatement[] = [
    {
      params: [input.failureReason, nowIso, input.payInId],
      sql: `UPDATE pay_ins
            SET state = 'failed', failure_reason = ?, state_changed_at = ?
            WHERE id = ? AND state IN ('pending', 'forwarding')`,
    },
  ]

  for (const leg of input.balanceFundingLegs) {
    statements.push(
      balanceCreditStatement(leg.partyRef, leg.amountMsat, nowIso),
    )
    statements.push(
      insertLegStatement(
        input.payInId,
        {
          amountMsat: leg.amountMsat,
          direction: 'out',
          externalRef: 'refund',
          kind: 'balance',
          legId: leg.refundLegId,
          partyRef: leg.partyRef,
        },
        nowIso,
        { refundOfLegId: leg.legId },
      ),
    )
  }

  return statements
}

export const markPayInForwardingStatements = (
  payInId: string,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => [
  {
    params: [nowIso, payInId],
    sql: `UPDATE pay_ins
          SET state = 'forwarding', state_changed_at = ?
          WHERE id = ? AND state = 'pending'`,
  },
]

// Retry chain: the successor lock is set-if-null, and the cloned pay-in
// is inserted with INSERT ... SELECT guarded on the lock having been won
// by THIS retry - a lost race inserts zero rows, so no attempt can ever
// be retried twice (SN's genesisId/successorId discipline).
export const retryPayInStatements = (
  input: Readonly<{
    previousPayInId: string
    newPlan: PayInPlan
  }>,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  assertPlanInvariants(input.newPlan)

  const lock: LedgerStatement = {
    params: [input.newPlan.payInId, input.previousPayInId],
    sql: `UPDATE pay_ins
          SET successor_id = ?
          WHERE id = ? AND successor_id IS NULL AND state = 'failed'`,
  }

  const guardedInsert: LedgerStatement = {
    params: [
      input.newPlan.payInId,
      input.newPlan.payInType,
      input.newPlan.payerRef,
      input.newPlan.costMsat,
      input.newPlan.rung,
      input.newPlan.contextRef,
      input.newPlan.idempotencyKey,
      input.newPlan.publicReceiptRef,
      input.newPlan.genesisId,
      nowIso,
      nowIso,
      input.previousPayInId,
      input.newPlan.payInId,
    ],
    sql: `INSERT INTO pay_ins
          (id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
           idempotency_key, public_receipt_ref, genesis_id, created_at,
           state_changed_at)
          SELECT ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?
          WHERE EXISTS (
            SELECT 1 FROM pay_ins WHERE id = ? AND successor_id = ?
          )`,
  }

  const statements: LedgerStatement[] = [lock, guardedInsert]

  for (const leg of input.newPlan.legs) {
    if (leg.kind === 'balance' && leg.direction === 'in') {
      statements.push(ensureBalanceRowStatement(leg.partyRef, nowIso))
      statements.push({
        params: [leg.amountMsat, nowIso, leg.partyRef, input.newPlan.payInId],
        sql: `UPDATE agent_balances
              SET balance_msat = balance_msat - ?, updated_at = ?
              WHERE actor_ref = ?
                AND EXISTS (SELECT 1 FROM pay_ins WHERE id = ?)`,
      })
      statements.push({
        params: [
          leg.legId,
          input.newPlan.payInId,
          leg.direction,
          leg.kind,
          leg.partyRef,
          leg.amountMsat,
          leg.partyRef,
          leg.externalRef,
          nowIso,
          input.newPlan.payInId,
        ],
        sql: `INSERT INTO pay_in_legs
              (id, pay_in_id, direction, kind, party_ref, amount_msat,
               resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
              SELECT ?, ?, ?, ?, ?, ?,
                     (SELECT balance_msat FROM agent_balances WHERE actor_ref = ?),
                     ?, NULL, ?
              WHERE EXISTS (SELECT 1 FROM pay_ins WHERE id = ?)`,
      })
    } else {
      statements.push({
        params: [
          leg.legId,
          input.newPlan.payInId,
          leg.direction,
          leg.kind,
          leg.partyRef,
          leg.amountMsat,
          leg.externalRef,
          nowIso,
          input.newPlan.payInId,
        ],
        sql: `INSERT INTO pay_in_legs
              (id, pay_in_id, direction, kind, party_ref, amount_msat,
               resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
              SELECT ?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?
              WHERE EXISTS (SELECT 1 FROM pay_ins WHERE id = ?)`,
      })
    }
  }

  return statements
}

export type AgentBalanceRow = Readonly<{
  actorRef: string
  // Inference-spendable balance: total minus escrow-held. USD-funded credit
  // (#5497) IS included here — a card purchase funds inference.
  availableMsat: number
  balanceMsat: number
  heldMsat: number
  // USD-origin portion of `balance_msat` (#5497, RL-3). Inference-spendable but
  // NOT Bitcoin-withdrawable; the sweep subtracts it.
  usdCreditMsat: number
  // Bitcoin-withdrawable balance: available minus the USD-origin portion,
  // floored at 0. This is the ONLY balance the Lightning sweep may pay out as
  // real Bitcoin (RL-3 asset boundary). A USD-purchased credit never leaks here.
  bitcoinWithdrawableMsat: number
  sweepEnabled: boolean
  sweepThresholdSat: number
  sendCreditsBelowSat: number
  receiveCreditsBelowSat: number
}>

export const decodeAgentBalanceRow = (row: {
  actor_ref: unknown
  balance_msat: unknown
  held_msat?: unknown
  usd_credit_msat?: unknown
  sweep_enabled: unknown
  sweep_threshold_sat: unknown
  send_credits_below_sat: unknown
  receive_credits_below_sat: unknown
}): AgentBalanceRow => {
  const availableMsat = Math.max(
    0,
    Number(row.balance_msat) - Number(row.held_msat ?? 0),
  )
  const usdCreditMsat = Math.max(0, Number(row.usd_credit_msat ?? 0))
  return {
    availableMsat,
    actorRef: String(row.actor_ref),
    balanceMsat: Number(row.balance_msat),
    bitcoinWithdrawableMsat: Math.max(0, availableMsat - usdCreditMsat),
    heldMsat: Number(row.held_msat ?? 0),
    receiveCreditsBelowSat: Number(row.receive_credits_below_sat),
    sendCreditsBelowSat: Number(row.send_credits_below_sat),
    sweepEnabled: Number(row.sweep_enabled) === 1,
    sweepThresholdSat: Number(row.sweep_threshold_sat),
    usdCreditMsat,
  }
}

export const readAgentBalance = async (
  db: PaymentsLedgerDb,
  actorRef: string,
): Promise<AgentBalanceRow | null> => {
  const rows = await db.query(
    `SELECT actor_ref, balance_msat, held_msat, usd_credit_msat,
            sweep_enabled, sweep_threshold_sat,
            send_credits_below_sat, receive_credits_below_sat
     FROM agent_balances WHERE actor_ref = ?`,
    [actorRef],
  )

  const row = rows[0]
  return row === undefined ? null : decodeAgentBalanceRow(row as never)
}

/**
 * Execute ledger statements as ONE atomic Postgres transaction (CFG-4,
 * #8519). All-or-nothing: a CHECK violation (insufficient balance) or a
 * UNIQUE violation (idempotency replay) aborts the whole transaction,
 * exactly the semantics the D1 batch had. There is no mirror and no D1
 * branch — Postgres is the sole authority for this domain.
 */
export const runLedgerStatements = async (
  db: PaymentsLedgerDb,
  statements: ReadonlyArray<LedgerStatement>,
): Promise<void> => {
  await db.batch(statements)
}

export const sumAgentBalancesMsat = async (
  db: PaymentsLedgerDb,
): Promise<number> => {
  const rows = await db.query(
    'SELECT COALESCE(SUM(balance_msat), 0) AS total FROM agent_balances',
  )

  return Number((rows[0] as { total?: unknown } | undefined)?.total ?? 0)
}
