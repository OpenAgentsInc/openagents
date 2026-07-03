import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makePublicAcceptedOutcomeSettlementRoutes } from './public-accepted-outcome-settlement-routes'

/**
 * Minimal fake D1 that answers the single `SELECT * FROM
 * omni_accepted_outcome_economics WHERE id = ?` read used by
 * readOmniAcceptedOutcomeEconomicsById. It returns a row for the configured id
 * and null otherwise.
 */
const economicsRow = (id: string) => ({
  accepted_outcome_contract_id: 'omni_accepted_outcome_contract_1',
  accepted_value_cents: 5000,
  archived_at: null,
  artifact_cost_cents: 100,
  buyer_price_asset: 'usd',
  buyer_price_cents: 5000,
  created_at: '2026-06-20T00:00:00.000Z',
  credits_charged: 0,
  funding_mode: 'credit_funded',
  gross_margin_cents: 4400,
  id,
  idempotency_key: `idem-${id}`,
  internal_caveat_ref: null,
  metadata_json: JSON.stringify({
    contributors: { platformId: 'platform.oa', runnerId: 'runner.alice' },
  }),
  no_settlement_implication: 1,
  provider_cost_cents: 300,
  public_caveat_ref: 'caveat.no_settlement',
  retry_cost_cents: 0,
  review_cost_cents: 100,
  review_minutes: 5,
  runner_cost_cents: 100,
  sats_charged: 0,
  total_cost_cents: 600,
  updated_at: '2026-06-20T00:00:00.000Z',
  work_kind: 'coding',
  workroom_id: 'omni_workroom_coding_1',
})

const fakeDb = (knownId: string | null): D1Database => {
  const prepared = {
    bind: (boundId: string) => ({
      first: async () =>
        knownId !== null && boundId === knownId ? economicsRow(boundId) : null,
    }),
  }
  return {
    prepare: () => prepared,
  } as unknown as D1Database
}

const route = async (db: D1Database, economicsId: string, init?: RequestInit) => {
  const routes = makePublicAcceptedOutcomeSettlementRoutes<{ db: D1Database }>({
    db: env => env.db,
    nowIso: () => '2026-06-20T12:00:00.000Z',
  })
  const response = routes.routePublicAcceptedOutcomeSettlementRequest(
    new Request(
      `https://openagents.com/api/public/accepted-outcome/settlement/${encodeURIComponent(
        economicsId,
      )}`,
      init,
    ),
    { db },
  )
  if (response === undefined) {
    throw new Error('settlement route did not match')
  }
  return Effect.runPromise(response)
}

describe('public accepted-outcome settlement routes', () => {
  test('serves the eight-state settlement projection with no private figures', async () => {
    const response = await route(fakeDb('omni_outcome_1'), 'omni_outcome_1')
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    // Public-projection staleness declaration (epic #4751): generatedAt plus a
    // live-at-read contract that is never older than the request.
    expect(body.generatedAt).toBe('2026-06-20T12:00:00.000Z')
    expect(body.staleness.composition).toBe('live_at_read')
    expect(body.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(body.staleness.maxStalenessSeconds).toBe(0)
    expect(body.settlement.settlementComplete).toBe(true)
    expect(body.settlement.settlementMachine.transitions).toHaveLength(8)
    expect(
      body.settlement.settlementMachine.transitions.map((t: any) => t.stateId),
    ).toEqual([
      'authorized',
      'paid',
      'accepted',
      'pending_payout',
      'dispatched',
      'confirmed',
      'reconciled',
      'margin',
    ])
    expect(
      new Set(
        body.settlement.settlementMachine.transitions.map(
          (t: any) => t.evidenceRef,
        ),
      ).size,
    ).toBe(8)
    // No internal monetary figures leak.
    expect(JSON.stringify(body)).not.toMatch(
      /amountCents|accruedMarginCents|grossMarginCents|4400|5000/,
    )
    // INERT: no money moved.
    expect(
      body.settlement.settlementMachine.transitions.every(
        (t: any) => t.movedMoney === false,
      ),
    ).toBe(true)
  })

  test('serves the BF-8.6 make-good demo as an inert no-custody projection', async () => {
    const response = await route(
      fakeDb('bf_8_6_make_good_demo_001'),
      'bf_8_6_make_good_demo_001',
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.settlement.settlementComplete).toBe(true)
    expect(body.settlement.settlementMachine.dispatchArmed).toBe(false)
    expect(
      body.settlement.settlementMachine.transitions.map(
        (transition: any) => transition.stateId,
      ),
    ).toEqual([
      'authorized',
      'paid',
      'accepted',
      'pending_payout',
      'dispatched',
      'confirmed',
      'reconciled',
      'margin',
    ])
    expect(
      body.settlement.settlementMachine.transitions.every(
        (transition: any) => transition.movedMoney === false,
      ),
    ).toBe(true)
    expect(JSON.stringify(body)).not.toMatch(
      /wallet|preimage|private|client|adapterArmed\":true|realBitcoinMoved\":true/i,
    )
  })

  test('returns 404 for an unknown outcome', async () => {
    const response = await route(fakeDb(null), 'missing_outcome')
    expect(response.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(fakeDb('omni_outcome_1'), 'omni_outcome_1', {
      method: 'POST',
    })
    expect(response.status).toBe(405)
  })
})
