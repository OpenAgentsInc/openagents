import { Schema as S } from 'effect'
import type { BillingDomainMirror } from './billing'

import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
  type TreasuryDomainTable,
} from './treasury-domain-store'

// Agent credit ledger (issue #4705).
// Design contract: docs/payments/reliable-tips.md. Every paid attempt is
// one pay_ins row created atomically (one D1 batch = one transaction)
// with the legs that fund it and the legs that say where value goes.
// Balances move only by increment/decrement; balance-touching legs store
// the resulting balance via a sequential-statement subquery inside the
// same batch; FAILED refunds funding debits atomically; retries chain
// through genesis/successor with a set-if-null optimistic lock.

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
  /**
   * KS-8.7 (#8318): the pay-in this statement touches, when it touches one.
   * `runLedgerStatements` uses these annotations to drive the fail-soft
   * Postgres mirror (pay_ins row + its legs, read back from D1 after the
   * batch).
   */
  payInId?: string | undefined
  /**
   * KS-8.8 (#8319): rows this statement touches in a treasury-domain
   * table, for the fail-soft Postgres dual-write mirror. Populated by the
   * statement BUILDERS (which know the row keys); consumed by
   * `runLedgerStatements` AFTER the authoritative D1 batch commits. Keys
   * only — the mirror reads the resolved rows back from D1, so it can
   * never invent an amount or a state.
   */
  mirror?: Readonly<{
    table: TreasuryDomainTable
    keyColumn: string
    keys: ReadonlyArray<string>
  }>
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

const balanceMirror = (partyRef: string) =>
  ({
    keyColumn: 'actor_ref',
    keys: [partyRef],
    table: 'agent_balances',
  }) as const

const ensureBalanceRowStatement = (
  partyRef: string,
  nowIso: string,
): LedgerStatement => ({
  mirror: balanceMirror(partyRef),
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
  mirror: balanceMirror(partyRef),
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
  mirror: balanceMirror(partyRef),
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
        payInId,
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
        payInId,
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
      payInId: plan.payInId,
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
        payInId: plan.payInId,
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
      payInId: input.payInId,
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
      payInId: input.payInId,
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
      payInId: input.payInId,
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
    payInId,
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
    payInId: input.previousPayInId,
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
    payInId: input.newPlan.payInId,
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
        mirror: balanceMirror(leg.partyRef),
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
        payInId: input.newPlan.payInId,
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
        payInId: input.newPlan.payInId,
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
  db: D1Database,
  actorRef: string,
): Promise<AgentBalanceRow | null> => {
  const row = await db
    .prepare(
      `SELECT actor_ref, balance_msat, held_msat, usd_credit_msat,
              sweep_enabled, sweep_threshold_sat,
              send_credits_below_sat, receive_credits_below_sat
       FROM agent_balances WHERE actor_ref = ?`,
    )
    .bind(actorRef)
    .first()

  return row === null ? null : decodeAgentBalanceRow(row as never)
}

/**
 * Execute ledger statements as ONE atomic D1 batch (the authority), then
 * mirror every builder-annotated treasury-domain row to Postgres
 * fail-soft (KS-8.8 #8319). The mirror runs only AFTER the batch commits
 * and reads the resolved rows back from D1 — a Postgres outage never
 * fails the ledger write, and the mirror can never invent an amount.
 * Call sites passing a bare D1Database behave exactly as before.
 */
export const runLedgerStatements = async (
  db: TreasuryDatabase,
  statements: ReadonlyArray<LedgerStatement>,
  /**
   * KS-8.7 (#8318): optional fail-soft Postgres mirror
   * (`billingDomainMirrorFromEnv`). After the D1 batch commits, every
   * annotated pay-in (row + its legs) is read back and converge-copied to
   * Cloud SQL. Callers without the env in reach omit it — those writes are
   * converged by the backfill sweeps (documented in the RUNBOOK coverage
   * list) until the decommission lane rehomes them.
   */
  mirror?: BillingDomainMirror | undefined,
): Promise<void> => {
  const authority = treasuryAuthorityDb(db)
  await authority.batch(
    statements.map(statement =>
      authority.prepare(statement.sql).bind(...statement.params),
    ),
  )

  if (mirror !== undefined) {
    const payInIds = [
      ...new Set(
        statements
          .map(statement => statement.payInId)
          .filter((id): id is string => id !== undefined),
      ),
    ]
    if (payInIds.length > 0) {
      await mirror(
        authority,
        payInIds.flatMap(payInId => [
          { key: { id: payInId }, table: 'pay_ins' as const },
          { key: { pay_in_id: payInId }, table: 'pay_in_legs' as const },
        ]),
      )
    }
  }

  // Group annotated mirrors by (table, keyColumn); dedupe keys so one
  // batch touching the same balance row many times mirrors it once.
  const groups = new Map<
    string,
    {
      table: TreasuryDomainTable
      keyColumn: string
      keys: Set<string>
    }
  >()
  for (const statement of statements) {
    if (statement.mirror === undefined) continue
    const groupKey = `${statement.mirror.table}:${statement.mirror.keyColumn}`
    const group = groups.get(groupKey) ?? {
      keyColumn: statement.mirror.keyColumn,
      keys: new Set<string>(),
      table: statement.mirror.table,
    }
    for (const key of statement.mirror.keys) group.keys.add(key)
    groups.set(groupKey, group)
  }
  for (const group of groups.values()) {
    await mirrorTreasuryRows(db, group.table, group.keyColumn, [...group.keys])
  }
}

export const sumAgentBalancesMsat = async (db: D1Database): Promise<number> => {
  const row = await db
    .prepare(
      'SELECT COALESCE(SUM(balance_msat), 0) AS total FROM agent_balances',
    )
    .first()

  return Number((row as { total?: unknown } | null)?.total ?? 0)
}
