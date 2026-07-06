import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from './http/responses'
import { readAgentBalance, runLedgerStatements } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { currentIsoTimestamp } from './runtime-primitives'

// Agent-scoped balance surface (issue #4712): a registered agent reads
// its own sweepable balance, thresholds, and recent ledger activity, and
// tunes its preferences. Public-safe fields only - amounts, states,
// rungs, refs - never destinations, offers, or payment material.
//
// CFG-4 (#8519): every table this surface touches (`agent_balances`,
// `pay_ins`, `pay_in_legs`) is credits-domain and therefore Cloud SQL
// Postgres-authoritative — the routes take the `PaymentsLedgerDb` handle
// directly. The old D1/treasury-authority/mirror routing for these tables is
// deleted.

export type AgentBalanceAuth = (
  request: Request,
) => Promise<Readonly<{ actorRef: string }> | undefined>

const AgentBalancePreferencesBody = S.Struct({
  receiveCreditsBelowSat: S.optionalKey(S.Number),
  sendCreditsBelowSat: S.optionalKey(S.Number),
  sweepEnabled: S.optionalKey(S.Boolean),
  sweepThresholdSat: S.optionalKey(S.Number),
})

const boundedPref = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Math.floor(value)))

export const handleAgentBalanceApi = (
  request: Request,
  deps: Readonly<{ ledgerDb: PaymentsLedgerDb; authenticate: AgentBalanceAuth }>,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const balance = yield* Effect.promise(() =>
      readAgentBalance(deps.ledgerDb, session.actorRef),
    )

    const recent = yield* Effect.promise(async () => {
      try {
        return await deps.ledgerDb.query(
          `SELECT id, pay_in_type, state, rung, cost_msat, context_ref,
                  failure_reason, state_changed_at
             FROM pay_ins
            WHERE payer_ref = ?
               OR id IN (
                 SELECT pay_in_id FROM pay_in_legs
                  WHERE party_ref = ? AND direction = 'out'
               )
            ORDER BY created_at DESC
            LIMIT 20`,
          [session.actorRef, session.actorRef],
        )
      } catch {
        return []
      }
    })

    return noStoreJsonResponse({
      balance:
        balance === null
          ? {
              availableMsat: 0,
              balanceMsat: 0,
              heldMsat: 0,
              receiveCreditsBelowSat: 10,
              sendCreditsBelowSat: 10,
              sweepEnabled: true,
              sweepThresholdSat: 210,
            }
          : {
              availableMsat: balance.availableMsat,
              balanceMsat: balance.balanceMsat,
              heldMsat: balance.heldMsat,
              receiveCreditsBelowSat: balance.receiveCreditsBelowSat,
              sendCreditsBelowSat: balance.sendCreditsBelowSat,
              sweepEnabled: balance.sweepEnabled,
              sweepThresholdSat: balance.sweepThresholdSat,
            },
      recentActivity: recent.map(row => ({
        contextRef: row.context_ref,
        // Postgres returns bigint columns as strings; decode explicitly.
        costMsat: Number(row.cost_msat),
        failureReason: row.failure_reason,
        payInId: row.id,
        payInType: row.pay_in_type,
        rung: row.rung,
        state: row.state,
        stateChangedAt: row.state_changed_at,
      })),
    })
  })

export const handleAgentBalancePreferencesApi = (
  request: Request,
  deps: Readonly<{ ledgerDb: PaymentsLedgerDb; authenticate: AgentBalanceAuth }>,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = yield* Effect.promise(async () => {
      try {
        return S.decodeUnknownSync(AgentBalancePreferencesBody)(
          await request.json(),
        )
      } catch {
        return undefined
      }
    })

    if (body === undefined) {
      return noStoreJsonResponse({ error: 'bad_request' }, { status: 400 })
    }

    const nowIso = currentIsoTimestamp()
    const updates: string[] = []
    const params: Array<string | number> = []

    if (body.sweepEnabled !== undefined) {
      updates.push('sweep_enabled = ?')
      params.push(body.sweepEnabled ? 1 : 0)
    }
    if (body.sweepThresholdSat !== undefined) {
      updates.push('sweep_threshold_sat = ?')
      params.push(boundedPref(body.sweepThresholdSat, 100, 1_000_000))
    }
    if (body.sendCreditsBelowSat !== undefined) {
      updates.push('send_credits_below_sat = ?')
      params.push(boundedPref(body.sendCreditsBelowSat, 0, 10_000))
    }
    if (body.receiveCreditsBelowSat !== undefined) {
      updates.push('receive_credits_below_sat = ?')
      params.push(boundedPref(body.receiveCreditsBelowSat, 0, 10_000))
    }

    if (updates.length === 0) {
      return noStoreJsonResponse({ error: 'no_preferences' }, { status: 400 })
    }

    // CFG-4 (#8519): one atomic Postgres transaction on the credits ledger.
    // The old KS-8.8 fail-soft Postgres mirror is gone — Postgres IS the
    // authority now.
    yield* Effect.promise(() =>
      runLedgerStatements(deps.ledgerDb, [
        {
          params: [session.actorRef, nowIso, nowIso],
          sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
             VALUES (?, 0, ?, ?)
             ON CONFLICT (actor_ref) DO NOTHING`,
        },
        {
          params: [...params, nowIso, session.actorRef],
          sql: `UPDATE agent_balances SET ${updates.join(', ')}, updated_at = ?
             WHERE actor_ref = ?`,
        },
      ]),
    )

    const balance = yield* Effect.promise(() =>
      readAgentBalance(deps.ledgerDb, session.actorRef),
    )

    return noStoreJsonResponse({ preferences: balance })
  })
