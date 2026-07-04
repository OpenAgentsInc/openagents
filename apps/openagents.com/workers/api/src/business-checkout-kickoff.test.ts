import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { insertBusinessSignupRequest } from './business-signup-routes'
import { provisionBusinessCheckoutKickoff } from './business-checkout-kickoff'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<D1Result<T>> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as Array<T>

    return { results, success: true } as D1Result<T>
  }

  async run(): Promise<D1Result> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true } as D1Result
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  batch<T = unknown>(
    statements: ReadonlyArray<D1PreparedStatement>,
  ): Promise<Array<D1Result<T>>> {
    return Promise.all(statements.map(statement => statement.run<T>()))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0))
  }

  exec(sql: string): Promise<D1ExecResult> {
    this.db.exec(sql)
    return Promise.resolve({ count: 0, duration: 0 })
  }

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(`
    CREATE TABLE users (id TEXT PRIMARY KEY);
    CREATE TABLE teams (
      id TEXT PRIMARY KEY,
      status TEXT NOT NULL DEFAULT 'active',
      archived_at TEXT
    );
    CREATE TABLE team_memberships (
      team_id TEXT NOT NULL,
      user_id TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'active'
    );
    CREATE TABLE team_projects (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'active',
      archived_at TEXT
    );
    CREATE TABLE stripe_checkout_sessions (session_id TEXT PRIMARY KEY);
  `)
  for (const name of [
    '0091_omni_accepted_outcome_contracts.sql',
    '0190_prefilled_workspaces.sql',
    '0191_business_signup_requests.sql',
    '0192_prefilled_workspace_invite_engagement.sql',
    '0195_private_prefilled_workspace_access.sql',
    '0216_business_signup_referral_attribution.sql',
    '0270_business_funnel_events.sql',
    '0271_business_signup_fulfillment.sql',
    '0272_business_checkout_kickoffs.sql',
    '0278_business_commitment_ledger.sql',
    '0294_business_pipeline_queue.sql',
    '0297_business_source_attribution.sql',
  ]) {
    db.exec(migration(name))
  }

  return new SqliteD1(db) as unknown as D1Database
}

describe('business checkout kickoff', () => {
  test('settled checkout provisions workspace and service promise idempotently', async () => {
    const db = makeDb()
    await db
      .prepare("INSERT INTO users (id) VALUES ('github:buyer_1')")
      .run()
    await db
      .prepare(
        "INSERT INTO stripe_checkout_sessions (session_id) VALUES ('cs_test_business_001')",
      )
      .run()
    const signup = await insertBusinessSignupRequest(
      db,
      {
        businessName: 'Vertical prospect',
        contactEmail: 'buyer@example.com',
        helpWith: 'Need a first deliverable.',
        phone: '+1 555 0100',
        referralCode: null,
        requestSlackChannel: false,
        sourceRef: 'direct',
        website: 'https://example.com',
      },
      {
        expiresAtFromNow: () => '2026-08-02T00:00:00.000Z',
        makeId: prefix => `${prefix}_001`,
        nowIso: () => '2026-07-02T00:00:00.000Z',
      },
    )

    const first = await provisionBusinessCheckoutKickoff(db, {
      checkoutSessionId: 'cs_test_business_001',
      creditGrantCents: 8000,
      setupFeeCents: 2000,
      signupId: signup.id,
      totalAmountCents: 10000,
      userId: 'github:buyer_1',
    })
    const second = await provisionBusinessCheckoutKickoff(db, {
      checkoutSessionId: 'cs_test_business_001',
      creditGrantCents: 8000,
      setupFeeCents: 2000,
      signupId: signup.id,
      totalAmountCents: 10000,
      userId: 'github:buyer_1',
    })

    expect(second).toEqual(first)

    const workspaceCount = await db
      .prepare('SELECT COUNT(*) AS count FROM prefilled_workspaces')
      .first<{ count: number }>()
    const contractCount = await db
      .prepare('SELECT COUNT(*) AS count FROM omni_accepted_outcome_contracts')
      .first<{ count: number }>()
    const kickoff = await db
      .prepare(
        `SELECT credit_grant_cents,
                setup_fee_cents,
                public_receipt_ref
           FROM business_checkout_kickoffs
          WHERE checkout_session_id = ?`,
      )
      .bind('cs_test_business_001')
      .first<{
        credit_grant_cents: number
        public_receipt_ref: string
        setup_fee_cents: number
      }>()

    expect(workspaceCount?.count).toBe(1)
    expect(contractCount?.count).toBe(1)
    expect(kickoff).toMatchObject({
      credit_grant_cents: 8000,
      public_receipt_ref:
        'receipt.business.checkout_kickoff.cs_test_business_001',
      setup_fee_cents: 2000,
    })
  })
})
