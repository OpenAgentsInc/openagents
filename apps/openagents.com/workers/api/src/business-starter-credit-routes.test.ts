import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { makeD1BusinessPipelineStore } from './business-pipeline-queue'
import {
  SALES_STARTER_CREDIT_ATTRIBUTION_KIND,
  makeD1BusinessStarterCreditStore,
} from './business-starter-credit'
import { makeOperatorBusinessStarterCreditRoutes } from './business-starter-credit-routes'
import { readAgentBalance } from './payments-ledger'
import { paymentsLedgerDbFromD1 } from './test/payments-ledger-sqlite'

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const ledgerSchema = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN ('tip','sweep','buffer_funding','reward','adjustment','usd_credit_grant')
  ),
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('pending', 'forwarding', 'paid', 'failed')
  ),
  failure_reason TEXT,
  rung TEXT CHECK (rung IN ('credited', 'direct_bolt12') OR rung IS NULL),
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  public_receipt_ref TEXT,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL
);
CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL,
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  kind TEXT NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);
`

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const NOW = '2026-07-04T12:00:00.000Z'
const ACCOUNT_REF = 'agent:sales_starter_001'
const PIPELINE_REF = 'biz-pipe-2026w27-credit-001'

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(ledgerSchema)
  db.exec(migration('0278_business_commitment_ledger.sql'))
  db.exec(migration('0294_business_pipeline_queue.sql'))
  db.exec('ALTER TABLE business_pipeline_rows ADD COLUMN business_signup_request_id TEXT;')
  db.exec(migration('0299_business_pipeline_partner_routing.sql'))
  db.exec(migration('0314_business_pipeline_subject_ref.sql'))
  db.exec(migration('0295_business_starter_credit_grants.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const makeStores = (db: D1Database) => {
  const pipelineStore = makeD1BusinessPipelineStore(db)
  return {
    // CFG-4 (#8519): the credits ledger handle shares the same underlying
    // SQLite database as the D1 shim in tests.
    ledger: paymentsLedgerDbFromD1(db as never),
    pipelineStore,
    starterCreditStore: makeD1BusinessStarterCreditStore(
      db,
      paymentsLedgerDbFromD1(db as never),
      pipelineStore,
    ),
  }
}

const seedPipeline = async (
  db: D1Database,
  pipelineRef = PIPELINE_REF,
): Promise<void> => {
  const { pipelineStore } = makeStores(db)
  await pipelineStore.createPipelineRow(
    {
      ownerRole: 'operator',
      pipelineRef,
      quotedBand: {
        label: 'agent-ready quick win',
        maxUsdCents: 500_000,
        minUsdCents: 150_000,
      },
      receiptRefs: ['receipt.business.scope_completed.20260704.001'],
      sourceRef: 'apollo_agent_readiness_ecommerce',
      stage: 'scope_completed',
      vertical: 'e-commerce',
    },
    {
      makeId: prefix => `${prefix}_test`,
      nowIso: () => NOW,
    },
  )
}

const authedRoutes = (db: D1Database) =>
  makeOperatorBusinessStarterCreditRoutes({
    makeStore: () => makeStores(db).starterCreditStore,
    requireAdminApiToken: async request =>
      request.headers.get('authorization') === 'Bearer test-admin',
  })

const operatorRequest = (path: string, init: RequestInit = {}) =>
  new Request(`https://openagents.com${path}`, {
    ...init,
    headers: {
      authorization: 'Bearer test-admin',
      'content-type': 'application/json',
      ...(init.headers ?? {}),
    },
  })

const runRoute = async (db: D1Database, request: Request): Promise<Response> => {
  const routed = authedRoutes(db).routeOperatorBusinessStarterCreditRequest(
    request,
    {} as never,
    {} as ExecutionContext,
  )
  if (routed === undefined) throw new Error('route did not match')
  return Effect.runPromise(routed)
}

// CFG-4 (#8519): pay_ins is credits-domain — read it through the ledger
// handle, exactly as production does.
const payInCount = async (db: D1Database): Promise<number> => {
  const rows = await paymentsLedgerDbFromD1(db as never).query(
    `SELECT COUNT(*) AS count FROM pay_ins`,
  )
  return Number(rows[0]?.count ?? 0)
}

describe('business starter credit routes', () => {
  test('grants non-transferable sales starter credit through the normal USD-origin ledger', async () => {
    const db = makeDb()
    await seedPipeline(db)

    const response = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: ACCOUNT_REF,
            amountUsdCents: 10_000,
            grantRef: 'sales-starter-grant-001',
            windowRef: 'sales_starter_credit.2026-07',
          }),
          method: 'POST',
        },
      ),
    )
    const body = await response.json() as {
      grant: {
        amountMsat: number
        attributionKind: string
        creditReceiptRef: string
        transferPolicy: string
      }
      pipelineReceiptRefs: ReadonlyArray<string>
    }

    expect(response.status).toBe(201)
    expect(body.grant).toMatchObject({
      attributionKind: SALES_STARTER_CREDIT_ATTRIBUTION_KIND,
      creditReceiptRef:
        'receipt.inference.usd_credit_grant.sales-starter-grant-001',
      transferPolicy: 'non_transferable',
    })
    expect(body.pipelineReceiptRefs).toContain(body.grant.creditReceiptRef)

    const balance = await readAgentBalance(makeStores(db).ledger, ACCOUNT_REF)
    expect(balance?.balanceMsat).toBe(body.grant.amountMsat)
    expect(balance?.usdCreditMsat).toBe(body.grant.amountMsat)
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)

    const payInRows = await makeStores(db).ledger.query(
      `SELECT pay_in_type, payer_ref, context_ref, public_receipt_ref
           FROM pay_ins
          WHERE public_receipt_ref = ?`,
      [body.grant.creditReceiptRef],
    )
    const payIn = payInRows[0]

    expect(payIn).toMatchObject({
      pay_in_type: 'usd_credit_grant',
      payer_ref: ACCOUNT_REF,
      public_receipt_ref: body.grant.creditReceiptRef,
    })
    expect(String(payIn?.context_ref)).toContain(SALES_STARTER_CREDIT_ATTRIBUTION_KIND)
  })

  // CFG-4 (#8519): the old KS-8.7 test asserting that a wired `mirror` saw
  // the pay_ins/pay_in_legs refs from `createGrant` was DELETED here — the
  // fail-soft D1→Postgres mirror machinery for the credits tables is gone;
  // Postgres (the ledger handle) is the sole authority the grant writes to.

  test('refuses amount and window cap exceedance without minting credit', async () => {
    const db = makeDb()
    await seedPipeline(db)

    const tooLarge = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: ACCOUNT_REF,
            amountUsdCents: 10_001,
            grantRef: 'sales-starter-grant-too-large',
          }),
          method: 'POST',
        },
      ),
    )
    expect(tooLarge.status).toBe(409)
    expect(await tooLarge.json()).toMatchObject({
      error: 'business_starter_credit_refused',
      reason: 'amount_cap_exceeded',
    })
    expect(await payInCount(db)).toBe(0)

    const first = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: ACCOUNT_REF,
            grantRef: 'sales-starter-grant-window-1',
            windowGrantCap: 1,
            windowRef: 'sales_starter_credit.2026-07-cap',
          }),
          method: 'POST',
        },
      ),
    )
    expect(first.status).toBe(201)

    const secondPipelineRef = 'biz-pipe-2026w27-credit-002'
    await seedPipeline(db, secondPipelineRef)
    const second = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${secondPipelineRef}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: 'agent:sales_starter_002',
            grantRef: 'sales-starter-grant-window-2',
            windowGrantCap: 1,
            windowRef: 'sales_starter_credit.2026-07-cap',
          }),
          method: 'POST',
        },
      ),
    )
    expect(second.status).toBe(409)
    expect(await second.json()).toMatchObject({
      error: 'business_starter_credit_refused',
      reason: 'window_cap_exceeded',
    })
  })

  test('links redemption receipts back through the pipeline receipt list', async () => {
    const db = makeDb()
    await seedPipeline(db)

    await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: ACCOUNT_REF,
            grantRef: 'sales-starter-grant-redemption',
          }),
          method: 'POST',
        },
      ),
    )

    const redemptionReceiptRef = 'receipt.inference.charge.redemption_001'
    const redemption = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-redemptions`,
        {
          body: JSON.stringify({
            grantRef: 'sales-starter-grant-redemption',
            redemptionReceiptRef,
          }),
          method: 'POST',
        },
      ),
    )
    expect(redemption.status).toBe(200)
    expect(await redemption.json()).toMatchObject({
      grant: {
        redemptionReceiptRefs: [redemptionReceiptRef],
      },
    })

    const pipeline = await makeStores(db).pipelineStore.readPipelineRow(PIPELINE_REF)
    expect(pipeline?.receiptRefs).toEqual(
      expect.arrayContaining([
        'receipt.inference.usd_credit_grant.sales-starter-grant-redemption',
        redemptionReceiptRef,
      ]),
    )
  })

  test('requires admin auth and rejects private account refs', async () => {
    const db = makeDb()
    await seedPipeline(db)

    const unauthorized = await runRoute(
      db,
      new Request(
        `https://openagents.com/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        { method: 'POST' },
      ),
    )
    expect(unauthorized.status).toBe(401)

    const unsafe = await runRoute(
      db,
      operatorRequest(
        `/api/operator/business/pipeline/${PIPELINE_REF}/starter-credit-grants`,
        {
          body: JSON.stringify({
            accountRef: 'agent:lead@example.com',
            grantRef: 'sales-starter-grant-private',
          }),
          method: 'POST',
        },
      ),
    )
    expect(unsafe.status).toBe(400)
    expect(await unsafe.json()).toMatchObject({
      error: 'business_starter_credit_validation_error',
    })
  })
})
