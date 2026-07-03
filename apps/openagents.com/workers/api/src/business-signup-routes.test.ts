import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  type BusinessSignupRuntime,
  handleBusinessSignupApi,
  readBusinessSignupRequest,
} from './business-signup-routes'

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

  async run(): Promise<{ success: true }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true }
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

// 0191 creates business_signup_requests; 0216 adds the referral columns
// (referral_code, referral_attribution_id) that the insert now writes, plus the
// consume-once binding table. node:sqlite keeps foreign_keys OFF, so 0216's
// references to referral_attributions / site_referral_sources are inert here.
const SCHEMA = [
  '0191_business_signup_requests.sql',
  '0216_business_signup_referral_attribution.sql',
  '0270_business_funnel_events.sql',
].map(migration)

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  for (const sql of SCHEMA) {
    db.exec(sql)
  }
  return new SqliteD1(db) as unknown as D1Database
}

let counter = 0

const runtime: BusinessSignupRuntime = {
  makeId: (prefix: string) => `${prefix}_${(counter += 1)}`,
  nowIso: () => '2026-06-16T12:00:00.000Z',
  expiresAtFromNow: () => '2026-07-16T12:00:00.000Z',
}

const run = (request: Request, db: D1Database) =>
  Effect.runPromise(handleBusinessSignupApi(request, db, runtime))

beforeEach(() => {
  counter = 0
})

describe('business signup routes', () => {
  test('stores Slack opt-in form posts as manual invite pending', async () => {
    const db = makeDb()
    const body = new URLSearchParams({
      businessName: '  Acme Co.  ',
      contactEmail: 'LEAD@Example.com',
      website: 'https://example.com',
      phone: '+1 555 000 0000',
      helpWith: 'Need a launch workflow.',
      requestSlackChannel: 'yes',
    })

    const response = await run(
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'POST',
        headers: {
          'content-type': 'application/x-www-form-urlencoded',
        },
        body,
      }),
      db,
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('content-type')).toContain('text/html')
    const html = await response.text()
    expect(html).toContain('Request received')
    expect(html).toContain('Slack Connect still requires your workspace')
    expect(html).toContain('business_signup_1')

    const record = await readBusinessSignupRequest(db, 'business_signup_1')
    expect(record).toMatchObject({
      businessName: 'Acme Co.',
      contactEmail: 'lead@example.com',
      website: 'https://example.com/',
      phone: '+1 555 000 0000',
      helpWith: 'Need a launch workflow.',
      requestSlackChannel: true,
      slackConnectStatus: 'manual_invite_pending',
      sourceRoute: '/business',
    })
  })

  test('JSON response is public-safe and does not echo contact details', async () => {
    const db = makeDb()
    const response = await run(
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          businessName: 'Acme Co.',
          contactEmail: 'lead@example.com',
          phone: '+1 555 000 0000',
          requestSlackChannel: true,
        }),
      }),
      db,
    )

    expect(response.status).toBe(201)
    const text = await response.text()
    expect(text).toContain('manual_invite_pending')
    expect(text).not.toContain('lead@example.com')
    expect(text).not.toContain('+1 555')
    expect(JSON.parse(text)).toMatchObject({
      request: {
        id: 'business_signup_1',
        requestedSlackChannel: true,
        slackConnectStatus: 'manual_invite_pending',
        nextAction: 'operator_manual_slack_connect_invite',
      },
    })
  })

  test('records a signup-stage funnel event with coarse source attribution', async () => {
    const db = makeDb()
    const response = await run(
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          businessName: 'Acme Co.',
          contactEmail: 'lead@example.com',
          phone: '+1 555 000 0000',
          requestSlackChannel: false,
          sourceAttribution: 'AI-search',
        }),
      }),
      db,
    )

    expect(response.status).toBe(201)

    const row = await db
      .prepare(
        `SELECT event_ref, stage, source_kind, source_ref
           FROM business_funnel_events
          WHERE event_ref = ?`,
      )
      .bind('business_signup:business_signup_1')
      .first<{
        event_ref: string
        stage: string
        source_kind: string
        source_ref: string
      }>()

    expect(row).toEqual({
      event_ref: 'business_signup:business_signup_1',
      stage: 'signup',
      source_kind: 'ai_search',
      source_ref: '/business',
    })
  })

  test('rejects missing email', async () => {
    const db = makeDb()
    const response = await run(
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          businessName: 'Acme Co.',
          phone: '+1 555 000 0000',
        }),
      }),
      db,
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({
      error: 'business_signup_validation_error',
      reason: 'contactEmail is required and must be a valid email',
    })
  })

  test('only accepts POST', async () => {
    const response = await run(
      new Request('https://openagents.com/api/public/business-signup', {
        method: 'GET',
      }),
      makeDb(),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST')
  })
})
