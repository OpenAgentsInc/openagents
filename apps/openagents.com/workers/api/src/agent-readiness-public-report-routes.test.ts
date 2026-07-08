import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { scanAgentReadinessDomain } from '@openagentsinc/agent-readiness'

import {
  makeD1AgentReadinessPublicReportStore,
} from './agent-readiness-public-report-store'
import {
  handlePublicAgentReadinessReportApi,
  makeOperatorAgentReadinessReportRoutes,
} from './agent-readiness-public-report-routes'
import {
  makeD1BusinessPipelineStore,
  type BusinessPipelineRuntime,
} from './business-pipeline-queue'

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
  db.exec(migration('0191_business_signup_requests.sql'))
  db.exec(migration('0270_business_funnel_events.sql'))
  db.exec(migration('0278_business_commitment_ledger.sql'))
  db.exec(migration('0294_business_pipeline_queue.sql'))
  db.exec(migration('0296_business_outreach_sequences.sql'))
  db.exec(migration('0299_business_pipeline_partner_routing.sql'))
  db.exec(migration('0297_business_source_attribution.sql'))
  db.exec(migration('0310_agent_readiness_public_reports.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const runtime: BusinessPipelineRuntime = {
  makeId: (prefix: string) => `${prefix}_test`,
  nowIso: () => '2026-07-08T12:00:00.000Z',
}

const spaShell = `<!doctype html><html><head><title>SPA</title></head><body><div id="root"></div><script src="/assets/app.js"></script></body></html>`

const fixtureFetch: typeof fetch = (async input => {
  const url = new URL(String(input))
  const notFound = () =>
    new Response('not found', { status: 404, headers: { 'content-type': 'text/plain' } })
  const html = () =>
    new Response(spaShell, { status: 200, headers: { 'content-type': 'text/html' } })
  switch (url.pathname) {
    case '/':
    case '/robots.txt':
    case '/.well-known/mcp.json':
    case '/.well-known/mcp/manifest.json':
    case '/mcp/manifest.json':
    case '/.well-known/ai-catalog.json':
      return html()
    default:
      return notFound()
  }
}) as typeof fetch

const scanReport = async (domain = 'broken-spa.example') =>
  scanAgentReadinessDomain(domain, {
    fetch: fixtureFetch,
    generatedAt: '2026-07-08T06:30:00.000Z',
    minRequestIntervalMs: 0,
  })

const seedPipelineRow = async (db: D1Database, pipelineRef: string, sourceRef: string) => {
  await makeD1BusinessPipelineStore(db).createPipelineRow(
    {
      ownerRole: 'operator',
      pipelineRef,
      sourceRef,
      vertical: 'ecommerce',
    },
    runtime,
  )
}

const operatorRoutes = (db: D1Database) =>
  makeOperatorAgentReadinessReportRoutes({
    makePipelineStore: () => makeD1BusinessPipelineStore(db),
    makeReportStore: () => makeD1AgentReadinessPublicReportStore(db),
    nowIso: runtime.nowIso,
    publicBaseUrl: 'https://openagents.com',
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

const runOperatorRoute = async (db: D1Database, request: Request): Promise<Response> => {
  const routed = operatorRoutes(db).routeOperatorAgentReadinessReportRequest(
    request,
    {} as never,
    {} as ExecutionContext,
  )
  if (routed === undefined) throw new Error('route did not match')
  return Effect.runPromise(routed)
}

describe('agent-readiness public report routes (OB-3, #8560)', () => {
  test('operator create -> public GET returns only public-safe fields and increments click counters', async () => {
    const db = makeDb()
    await seedPipelineRow(db, 'biz-pipe-100', 'apollo_agent_readiness_ecommerce')
    const report = await scanReport()

    const createResponse = await runOperatorRoute(
      db,
      operatorRequest('/api/operator/agent-readiness/reports', {
        method: 'POST',
        body: JSON.stringify({ pipelineRef: 'biz-pipe-100', report }),
      }),
    )
    expect(createResponse.status).toBe(201)
    const created = (await createResponse.json()) as {
      report: { reportToken: string; url: string; domain: string; score: number }
    }
    expect(created.report.domain).toBe(report.domain)
    expect(created.report.url).toBe(
      `https://openagents.com/api/public/agent-readiness/reports/${created.report.reportToken}`,
    )

    // The pipeline row now carries the report receipt ref (BF-9.2).
    const pipelineRow = await makeD1BusinessPipelineStore(db).readPipelineRow('biz-pipe-100')
    expect(pipelineRow?.receiptRefs).toContain(
      `agent_readiness_report:${created.report.reportToken}`,
    )

    const publicRequest = new Request(
      `https://openagents.com/api/public/agent-readiness/reports/${created.report.reportToken}`,
    )
    const publicResponse = await Effect.runPromise(
      handlePublicAgentReadinessReportApi(publicRequest, { OPENAGENTS_DB: db }),
    )
    expect(publicResponse.status).toBe(200)
    const publicBody = (await publicResponse.json()) as Record<string, unknown>
    expect(publicBody.domain).toBe(report.domain)
    expect(publicBody).not.toHaveProperty('pipelineRef')
    expect(publicBody).not.toHaveProperty('sourceRef')
    expect(JSON.stringify(publicBody)).not.toContain('biz-pipe-100')
    expect(JSON.stringify(publicBody)).not.toContain('apollo_agent_readiness_ecommerce')

    // The GET wired a real funnel click into the existing LG-6 counters.
    const funnelRows = await db
      .prepare(
        `SELECT stage, source_kind, source_ref FROM business_funnel_events WHERE stage = 'visit'`,
      )
      .all<{ stage: string; source_kind: string; source_ref: string }>()
    expect(funnelRows.results).toHaveLength(1)
    expect(funnelRows.results[0]?.source_ref).toBe('apollo_agent_readiness_ecommerce')
    expect(funnelRows.results[0]?.source_kind).toBe('outbound')
  })

  test('operator create returns 404 for an unknown pipeline row and does not create a public report', async () => {
    const db = makeDb()
    const report = await scanReport()
    const response = await runOperatorRoute(
      db,
      operatorRequest('/api/operator/agent-readiness/reports', {
        method: 'POST',
        body: JSON.stringify({ pipelineRef: 'biz-pipe-missing', report }),
      }),
    )
    expect(response.status).toBe(404)
  })

  test('operator create requires the admin bearer token', async () => {
    const db = makeDb()
    await seedPipelineRow(db, 'biz-pipe-101', 'apollo_agent_readiness_legal')
    const report = await scanReport()
    const response = await runOperatorRoute(
      db,
      new Request('https://openagents.com/api/operator/agent-readiness/reports', {
        method: 'POST',
        body: JSON.stringify({ pipelineRef: 'biz-pipe-101', report }),
        headers: { 'content-type': 'application/json' },
      }),
    )
    expect(response.status).toBe(401)
  })

  test('public GET 404s for an unknown token without touching funnel counters', async () => {
    const db = makeDb()
    const response = await Effect.runPromise(
      handlePublicAgentReadinessReportApi(
        new Request('https://openagents.com/api/public/agent-readiness/reports/rr_missing'),
        { OPENAGENTS_DB: db },
      ),
    )
    expect(response.status).toBe(404)
    const funnelRows = await db
      .prepare(`SELECT COUNT(*) AS count FROM business_funnel_events`)
      .first<{ count: number }>()
    expect(Number(funnelRows?.count ?? 0)).toBe(0)
  })
})
