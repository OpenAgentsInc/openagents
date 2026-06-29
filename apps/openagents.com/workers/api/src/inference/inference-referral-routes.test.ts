import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { hostedMdkDirectPayoutDisabledGate } from '../mdk-payout-mode-gate'
import { makeInferenceReferralRoutes } from './inference-referral-routes'

// Reuse the same real-sqlite D1 shim used by the accrual test.
type Row = Record<string, unknown>
class Stmt {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): Stmt {
    this.bound = values.map(v => (v === undefined ? null : v))
    return this
  }
  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Row>(): Promise<{ results: T[] }> {
    return { results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[] }
  }
  async run(): Promise<{ success: true; results: [] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}
class D1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): Stmt {
    return new Stmt(this.db, sql)
  }
  async batch(statements: ReadonlyArray<Stmt>): Promise<Array<{ success: true }>> {
    for (const s of statements) await s.run()
    return statements.map(() => ({ success: true as const }))
  }
}

const SCHEMA = `
CREATE TABLE site_referral_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL, payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE, referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL, referral_invite_id TEXT,
  referrer_user_id TEXT NOT NULL, referred_user_id TEXT,
  qualifying_event_ref TEXT NOT NULL, qualifying_event_kind TEXT NOT NULL,
  qualifying_amount_sats INTEGER NOT NULL DEFAULT 0, amount_sats INTEGER NOT NULL,
  period_key TEXT NOT NULL, state TEXT NOT NULL, state_reason_ref TEXT,
  previous_entry_id TEXT, reversal_of_entry_id TEXT,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]', policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL, archived_at TEXT
);
`

type Env = { OPENAGENTS_DB: D1Database }
type Session = { user: { email: string; userId: string } }

const makeEnv = (): Env => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return { OPENAGENTS_DB: new D1(raw) as unknown as D1Database }
}

const baseDeps = (
  overrides: Partial<{
    session: Session | undefined
    admin: boolean
  }> = {},
) =>
  makeInferenceReferralRoutes<Session, Env>({
    appendRefreshedSessionCookies: response => response,
    dispatchDependencies: {
      adapter: {
        adapterKind: 'test',
        dispatch: async () => ({ receiptRef: 'receipt.test' }),
      },
      nowIso: () => '2026-06-19T12:00:00.000Z',
      readReadiness: async () => hostedMdkDirectPayoutDisabledGate(),
    },
    requireAdminApiToken: async () => overrides.admin ?? false,
    requireBrowserSession: async () =>
      'session' in overrides
        ? overrides.session
        : { user: { email: 'a@b.c', userId: 'ref-1' } },
  })

const ctx = {} as ExecutionContext

const run = (effect: Effect.Effect<Response> | undefined) =>
  effect === undefined ? Promise.resolve(undefined) : Effect.runPromise(effect)

describe('inference referral routes', () => {
  test('GET dashboard returns the signed-in referrer rollup', async () => {
    const routes = baseDeps()
    const response = await run(
      routes.routeInferenceReferralRequest(
        new Request('https://x/api/inference/referral/dashboard'),
        makeEnv(),
        ctx,
      ),
    )
    expect(response?.status).toBe(200)
    const body = (await response?.json()) as {
      inferenceReferralDashboard: { referrerUserId: string }
    }
    expect(body.inferenceReferralDashboard.referrerUserId).toBe('ref-1')
  })

  test('GET dashboard 401 without a session', async () => {
    const routes = baseDeps({ session: undefined })
    const response = await run(
      routes.routeInferenceReferralRequest(
        new Request('https://x/api/inference/referral/dashboard'),
        makeEnv(),
        ctx,
      ),
    )
    expect(response?.status).toBe(401)
  })

  test('POST dispatch 401 without admin token', async () => {
    const routes = baseDeps({ admin: false })
    const response = await run(
      routes.routeInferenceReferralRequest(
        new Request(
          'https://x/api/operator/inference/referral/payout/p1/dispatch',
          { body: JSON.stringify({ revenueAsset: 'bitcoin' }), method: 'POST' },
        ),
        makeEnv(),
        ctx,
      ),
    )
    expect(response?.status).toBe(401)
  })

  test('POST dispatch (admin) REFUSES under the owner-armed gate', async () => {
    const routes = baseDeps({ admin: true })
    // Seed one eligible payout so the dispatch reaches the readiness gate.
    const env = makeEnv()
    await env.OPENAGENTS_DB.prepare(
      `INSERT INTO site_referral_payout_ledger_entries
        (id, payout_ref, idempotency_key, referral_attribution_id, referral_source_id,
         referrer_user_id, qualifying_event_ref, qualifying_event_kind,
         qualifying_amount_sats, amount_sats, period_key, state, created_at)
       VALUES ('e1','p1','k1','a1','s1','ref-1','q1','inference_paid_request',500,25,'inference-2026-06','eligible','2026-06-19T12:00:00.000Z')`,
    )
      .bind()
      .run()

    const response = await run(
      routes.routeInferenceReferralRequest(
        new Request(
          'https://x/api/operator/inference/referral/payout/p1/dispatch',
          { body: JSON.stringify({ revenueAsset: 'bitcoin' }), method: 'POST' },
        ),
        env,
        ctx,
      ),
    )
    expect(response?.status).toBe(200)
    const body = (await response?.json()) as {
      dispatch: { _tag: string; reasonRef?: string }
    }
    expect(body.dispatch._tag).toBe('refused')
    expect(body.dispatch.reasonRef).toContain('payout_target_not_ready')
  })

  test('non-matching path returns undefined (falls through)', () => {
    const routes = baseDeps()
    expect(
      routes.routeInferenceReferralRequest(
        new Request('https://x/api/other'),
        makeEnv(),
        ctx,
      ),
    ).toBeUndefined()
  })
})
