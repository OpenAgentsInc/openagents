import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  recordBusinessFunnelEvent,
  type BusinessFunnelRuntime,
} from './business-funnel-dashboard'
import { handlePublicBusinessFunnelDashboardApi } from './business-funnel-dashboard-routes'

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

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0270_business_funnel_events.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

let counter = 0

const runtime: BusinessFunnelRuntime = {
  makeId: (prefix: string) => `${prefix}_${(counter += 1)}`,
  nowIso: () => '2026-07-02T17:00:00.000Z',
}

describe('business funnel dashboard', () => {
  test('aggregates exact stage rows by coarse source without private fields', async () => {
    const db = makeDb()

    await recordBusinessFunnelEvent(
      db,
      {
        eventRef: 'visit:content:2026-07-02',
        stage: 'visit',
        sourceKind: 'content',
        sourceRef: 'source.public.business.content',
        occurredAt: '2026-07-02T16:00:00.000Z',
      },
      runtime,
    )
    await recordBusinessFunnelEvent(
      db,
      {
        eventRef: 'signup:business_signup_1',
        stage: 'signup',
        sourceKind: 'ai_search',
        sourceRef: 'source.public.ai_search',
        occurredAt: '2026-07-02T16:05:00.000Z',
      },
      runtime,
    )
    await recordBusinessFunnelEvent(
      db,
      {
        eventRef: 'signup:business_signup_1',
        stage: 'signup',
        sourceKind: 'ai_search',
        sourceRef: 'source.public.ai_search',
        occurredAt: '2026-07-02T16:05:00.000Z',
      },
      runtime,
    )

    const response = await Effect.runPromise(
      handlePublicBusinessFunnelDashboardApi(
        new Request('https://openagents.com/api/public/business/funnel-dashboard'),
        db,
        runtime,
      ),
    )
    const body = await response.json() as {
      totals: { eventCount: number }
      stages: ReadonlyArray<{
        stage: string
        count: number
        sourceBreakdown: ReadonlyArray<{ sourceKind: string; count: number }>
      }>
      privacyBoundary: { aggregateOnly: boolean; excludes: ReadonlyArray<string> }
      staleness: { composition: string; rebuildsOn: ReadonlyArray<string> }
    }
    const json = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body.totals.eventCount).toBe(2)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      rebuildsOn: ['business_funnel_events.insert'],
    })
    expect(body.privacyBoundary.aggregateOnly).toBe(true)
    expect(body.privacyBoundary.excludes).toContain('contact_email')
    expect(json).not.toContain('lead@example.com')
    expect(json).not.toContain('555')

    const signup = body.stages.find(stage => stage.stage === 'signup')
    expect(signup).toMatchObject({ count: 1 })
    expect(
      signup?.sourceBreakdown.find(source => source.sourceKind === 'ai_search'),
    ).toEqual({ sourceKind: 'ai_search', count: 1 })
  })
})
