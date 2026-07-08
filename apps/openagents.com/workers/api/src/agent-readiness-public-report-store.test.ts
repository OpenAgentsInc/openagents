import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { renderAgentReadinessFifteenStepAssessment, scanAgentReadinessDomain } from '@openagentsinc/agent-readiness'

import {
  AgentReadinessPublicReportValidationError,
  makeD1AgentReadinessPublicReportStore,
} from './agent-readiness-public-report-store'

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
  db.exec(migration('0310_agent_readiness_public_reports.sql'))
  return new SqliteD1(db) as unknown as D1Database
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

const makeAssessment = async (domain = 'broken-spa.example') => {
  const report = await scanAgentReadinessDomain(domain, {
    fetch: fixtureFetch,
    generatedAt: '2026-07-08T06:30:00.000Z',
    minRequestIntervalMs: 0,
  })
  return renderAgentReadinessFifteenStepAssessment(report, {
    generatedAt: '2026-07-08T07:00:00.000Z',
  })
}

const runtime = {
  makeToken: (() => {
    let n = 0
    return () => `rr_test_${(n += 1)}`
  })(),
  nowIso: () => '2026-07-08T07:00:00.000Z',
}

describe('agent-readiness-public-report-store (OB-3, #8560)', () => {
  test('creates a tokenized public report and reads back only public-safe fields', async () => {
    const db = makeDb()
    const store = makeD1AgentReadinessPublicReportStore(db)
    const assessment = await makeAssessment()

    const created = await store.createPublicReport(
      {
        pipelineRef: 'biz-pipe-001',
        sourceRef: 'apollo_agent_readiness_ecommerce',
        domain: assessment.domain,
        assessment,
      },
      runtime,
    )

    expect(created.reportToken).toBe('rr_test_1')
    expect(created.receiptRef).toBe('agent_readiness_report:rr_test_1')
    expect(created.domain).toBe(assessment.domain)
    expect(created.score).toBe(assessment.overallScore)
    expect(created.grade).toBe(assessment.overallGrade)

    const projection = await store.readPublicReportByToken(created.reportToken)
    expect(projection).not.toBeNull()
    expect(projection?.domain).toBe(assessment.domain)
    expect(projection?.assessment).toEqual(assessment)
    // Public projection never carries pipelineRef/sourceRef.
    expect(JSON.stringify(projection)).not.toContain('biz-pipe-001')
    expect(JSON.stringify(projection)).not.toContain('apollo_agent_readiness_ecommerce')
  })

  test('returns null for an unknown or malformed token', async () => {
    const db = makeDb()
    const store = makeD1AgentReadinessPublicReportStore(db)

    expect(await store.readPublicReportByToken('rr_does_not_exist')).toBeNull()
    expect(await store.readPublicReportByToken('../etc/passwd')).toBeNull()
  })

  test('recordReportClick increments the counter and returns the internal sourceRef for funnel attribution', async () => {
    const db = makeDb()
    const store = makeD1AgentReadinessPublicReportStore(db)
    const assessment = await makeAssessment()
    const created = await store.createPublicReport(
      {
        pipelineRef: 'biz-pipe-002',
        sourceRef: 'apollo_agent_readiness_legal',
        domain: assessment.domain,
        assessment,
      },
      runtime,
    )

    const first = await store.recordReportClick(created.reportToken, runtime)
    expect(first).toEqual({ sourceRef: 'apollo_agent_readiness_legal', clickCount: 1 })
    const second = await store.recordReportClick(created.reportToken, runtime)
    expect(second).toEqual({ sourceRef: 'apollo_agent_readiness_legal', clickCount: 2 })

    expect(await store.recordReportClick('rr_missing', runtime)).toBeNull()
  })

  test('rejects unsafe domain/ref input rather than storing it', async () => {
    const db = makeDb()
    const store = makeD1AgentReadinessPublicReportStore(db)
    const assessment = await makeAssessment()

    await expect(
      store.createPublicReport(
        {
          pipelineRef: 'biz-pipe-003',
          sourceRef: 'apollo_agent_readiness_legal',
          domain: 'not a domain!!',
          assessment,
        },
        runtime,
      ),
    ).rejects.toBeInstanceOf(AgentReadinessPublicReportValidationError)
  })
})
