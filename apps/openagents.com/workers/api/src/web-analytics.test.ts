import { ANALYTICS_SCHEMA_VERSION } from '@openagentsinc/analytics'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  WEB_ANALYTICS_ADMIN_PATH,
  WEB_ANALYTICS_INGEST_PATH,
  makeWebAnalyticsRoutes,
} from './web-analytics'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>) {
    this.bound = values
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
  }

  async run(): Promise<{ success: true; results: never[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

const makeDatabase = () => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(`
    CREATE TABLE web_analytics_events (
      event_id TEXT PRIMARY KEY,
      event_name TEXT NOT NULL,
      client_kind TEXT NOT NULL,
      route_id TEXT NOT NULL,
      occurred_at TEXT NOT NULL,
      received_at TEXT NOT NULL,
      schema_version TEXT NOT NULL
    );
  `)
  const db = {
    prepare: (sql: string) => new SqliteD1Statement(raw, sql),
    batch: async (statements: ReadonlyArray<SqliteD1Statement>) =>
      Promise.all(statements.map(statement => statement.run())),
  } as unknown as D1Database
  return { db, raw }
}

type TestSession = Readonly<{ user: Readonly<{ email: string }> }>
type TestEnv = Readonly<{ WEB_ANALYTICS_ENABLED?: string | undefined }>

const context = {} as ExecutionContext

const requestBatch = {
  schemaVersion: ANALYTICS_SCHEMA_VERSION,
  events: [
    {
      schemaVersion: ANALYTICS_SCHEMA_VERSION,
      eventId: 'event.test.00000001',
      name: 'page_view',
      client: 'web',
      routeId: '/',
      occurredAt: '2026-07-25T18:00:00.000Z',
    },
    {
      schemaVersion: ANALYTICS_SCHEMA_VERSION,
      eventId: 'event.test.00000002',
      name: 'github_view',
      client: 'web',
      routeId: '/',
      occurredAt: '2026-07-25T18:00:01.000Z',
    },
  ],
}

const makeRoutes = (
  db: D1Database,
  session: TestSession | undefined = undefined,
) =>
  makeWebAnalyticsRoutes<TestSession, TestEnv>({
    appendRefreshedSessionCookies: response => response,
    db: () => db,
    isOpenAgentsAdminEmail: email => email === 'admin@openagents.com',
    requireBrowserSession: async () => session,
    nowIso: () => '2026-07-25T18:05:00.000Z',
  })

describe('web analytics routes', () => {
  test('is default-off and requires the exact same origin', async () => {
    const { db } = makeDatabase()
    const routes = makeRoutes(db)
    const disabled = await routes.handleIngest(
      new Request(`https://openagents.com${WEB_ANALYTICS_INGEST_PATH}`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          origin: 'https://openagents.com',
        },
        body: JSON.stringify(requestBatch),
      }),
      {},
    )
    const crossOrigin = await routes.handleIngest(
      new Request(`https://openagents.com${WEB_ANALYTICS_INGEST_PATH}`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          origin: 'https://example.com',
        },
        body: JSON.stringify(requestBatch),
      }),
      { WEB_ANALYTICS_ENABLED: 'true' },
    )
    expect(disabled.status).toBe(404)
    expect(crossOrigin.status).toBe(403)
  })

  test('stores a bounded batch idempotently and returns aggregates', async () => {
    const { db, raw } = makeDatabase()
    const routes = makeRoutes(db, {
      user: { email: 'admin@openagents.com' },
    })
    const env = { WEB_ANALYTICS_ENABLED: 'true' }
    const ingest = () =>
      routes.handleIngest(
        new Request(`https://openagents.com${WEB_ANALYTICS_INGEST_PATH}`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            origin: 'https://openagents.com',
          },
          body: JSON.stringify(requestBatch),
        }),
        env,
      )

    expect((await ingest()).status).toBe(204)
    expect((await ingest()).status).toBe(204)
    expect(
      (
        raw
          .prepare('SELECT COUNT(*) AS count FROM web_analytics_events')
          .get() as { count: number }
      ).count,
    ).toBe(2)

    const response = await routes.handleAdminSummary(
      new Request(
        `https://openagents.com${WEB_ANALYTICS_ADMIN_PATH}?window=24h`,
      ),
      env,
      context,
    )
    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      pageViews: 1,
      totalNamedEvents: 1,
      namedEvents: [{ name: 'github_view', count: 1 }],
      topPages: [{ routeId: '/', pageViews: 1 }],
    })
  })

  test('enforces browser-session admin authorization', async () => {
    const { db } = makeDatabase()
    const env = { WEB_ANALYTICS_ENABLED: 'true' }
    const request = new Request(
      `https://openagents.com${WEB_ANALYTICS_ADMIN_PATH}`,
    )
    const unauthorized = await makeRoutes(db).handleAdminSummary(
      request,
      env,
      context,
    )
    const forbidden = await makeRoutes(db, {
      user: { email: 'reader@example.com' },
    }).handleAdminSummary(request, env, context)

    expect(unauthorized.status).toBe(401)
    expect(forbidden.status).toBe(403)
  })
})
