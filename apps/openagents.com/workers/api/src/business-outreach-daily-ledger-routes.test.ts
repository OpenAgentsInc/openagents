import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  ADMIN_OPS_DAILY_SALES_LEDGER_PATH,
  type AdminCaller,
  makeDailySalesLedgerRoutes,
} from './business-outreach-daily-ledger-routes'

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
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0278_business_commitment_ledger.sql'))
  db.exec(migration('0294_business_pipeline_queue.sql'))
  db.exec('ALTER TABLE business_pipeline_rows ADD COLUMN business_signup_request_id TEXT;')
  db.exec(migration('0299_business_pipeline_partner_routing.sql'))
  db.exec(migration('0314_business_pipeline_subject_ref.sql'))
  db.exec(migration('0296_business_outreach_sequences.sql'))
  db.exec(migration('0026_email_ledger.sql'))
  db.exec(migration('0063_email_campaign_records.sql'))
  db.exec(migration('0218_crm_contacts.sql'))
  db.exec(migration('0270_business_funnel_events.sql'))
  db.exec(migration('0310_crm_command_batches_and_replies.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

type Env = Readonly<{
  OPENAGENTS_DB: D1Database
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }>
}>

type FakeSqlClient = Readonly<{
  query: (
    text: string,
    params: ReadonlyArray<string>,
  ) => Promise<ReadonlyArray<Record<string, unknown>>>
  end: () => Promise<void>
}>

const makeRoutes = (
  db: D1Database,
  adminUserId: string | undefined,
  options: Readonly<{
    khalaSyncBinding?: (env: Env) => Readonly<{ connectionString: string }> | undefined
    makeSqlClient?: (connectionString: string) => Promise<FakeSqlClient>
  }> = {},
) =>
  makeDailySalesLedgerRoutes<Env>({
    db: env => env.OPENAGENTS_DB,
    khalaSyncBinding: options.khalaSyncBinding,
    makeSqlClient: options.makeSqlClient,
    nowIso: () => '2026-07-08T12:00:00.000Z',
    requireAdminCaller: async (): Promise<AdminCaller | undefined> =>
      adminUserId === undefined ? undefined : { userId: adminUserId },
  })

const fakeCtx = {} as ExecutionContext

describe('daily sales ledger routes — auth (fail closed)', () => {
  test('401s without an admin caller', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, undefined)

    const response = await routes.handleDailySalesLedgerApi(
      new Request(`https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}`),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(401)
  })
})

describe('daily sales ledger routes', () => {
  test('rejects non-GET methods', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleDailySalesLedgerApi(
      new Request(`https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}`, {
        method: 'POST',
      }),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(405)
  })

  test('defaults to the trailing-7-day window and returns a zero-filled ledger with no data', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleDailySalesLedgerApi(
      new Request(`https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}`),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: true
      ledger: { since: string; until: string; digestLine: string; deliverabilityDays: Array<unknown> }
    }
    expect(body.ok).toBe(true)
    expect(body.ledger.since).toBe('2026-07-02')
    expect(body.ledger.until).toBe('2026-07-08')
    expect(body.ledger.deliverabilityDays).toHaveLength(7)
    expect(body.ledger.digestLine).toContain('2026-07-08 sales ledger:')
  })

  test('honors explicit since/until query params', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleDailySalesLedgerApi(
      new Request(
        `https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}?since=2026-06-01&until=2026-06-02`,
      ),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { ledger: { since: string; until: string } }
    expect(body.ledger.since).toBe('2026-06-01')
    expect(body.ledger.until).toBe('2026-06-02')
  })

  test('reports conversations not_measured when KHALA_SYNC_DB is absent', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner', {
      khalaSyncBinding: e => e.KHALA_SYNC_DB,
    })

    const response = await routes.handleDailySalesLedgerApi(
      new Request(
        `https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}?since=2026-07-08&until=2026-07-08`,
      ),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ledger: {
        engagementDays: Array<{ conversations: { status: string; reasonRef?: string } }>
      }
    }
    expect(body.ledger.engagementDays[0]?.conversations).toEqual({
      reasonRef: 'reason.ob6.khala_sync_db_binding_absent',
      status: 'not_measured',
    })
  })

  test('measures conversations through the khala-sync binding when present', async () => {
    const db = makeDb()
    const env: Env = {
      KHALA_SYNC_DB: { connectionString: 'postgres://khala-sync-test' },
      OPENAGENTS_DB: db,
    }
    const seenConnectionStrings: Array<string> = []
    const routes = makeRoutes(db, 'user_owner', {
      khalaSyncBinding: e => e.KHALA_SYNC_DB,
      makeSqlClient: async connectionString => {
        seenConnectionStrings.push(connectionString)
        return {
          end: async () => {},
          query: async () => [{ count: 4, day: '2026-07-08' }],
        }
      },
    })

    const response = await routes.handleDailySalesLedgerApi(
      new Request(
        `https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}?since=2026-07-08&until=2026-07-08`,
      ),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ledger: {
        digestLine: string
        engagementDays: Array<{ conversations: { status: string; count?: number } }>
      }
    }
    expect(seenConnectionStrings).toEqual(['postgres://khala-sync-test'])
    expect(body.ledger.engagementDays[0]?.conversations).toEqual({
      count: 4,
      status: 'measured',
    })
    expect(body.ledger.digestLine).toContain('conversations 4')
  })

  test('400s on an invalid window instead of throwing', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleDailySalesLedgerApi(
      new Request(
        `https://openagents.com${ADMIN_OPS_DAILY_SALES_LEDGER_PATH}?since=2026-06-10&until=2026-06-01`,
      ),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { ok: false; messageSafe: string }
    expect(body.ok).toBe(false)
  })
})
