import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT,
  firstDollarEvidenceBundleRef,
  readFirstDollarEvidenceBundle,
  recordRevenueEventProvenance,
} from './revenue-event-provenance'
import { makePublicFirstDollarEvidenceRoutes } from './revenue-event-provenance-routes'

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

  async run(): Promise<D1Result> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true } as D1Result
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
  db.exec(migration('0293_revenue_event_provenance.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

describe('revenue event provenance', () => {
  test('records a public-safe first-dollar evidence bundle', async () => {
    const db = makeDb()
    const eventRef = 'revenue_event.khala_code.paid_plan.purchase_123'
    const bundleRef = firstDollarEvidenceBundleRef('khala_code', eventRef)

    const record = await recordRevenueEventProvenance(db, {
      amountCents: 1900,
      amountSats: null,
      caveatRefs: ['caveat.revenue.first_dollar.owner_signoff_required'],
      demandProvenance: 'external',
      eventRef,
      evidenceBundleRef: bundleRef,
      idempotencyKey: 'revenue-event-test-1',
      ledgerRowRef: 'khala_code_paid_plan_purchase_123',
      ledgerTable: 'khala_code_paid_plan_payment_intents',
      paymentState: 'fulfilled',
      productRef: 'khala_code',
      publicEvidenceRefs: [
        'receipt.inference.privacy_entitlement.khala_code_paid_plan_123',
      ],
      receiptRef:
        'receipt.inference.privacy_entitlement.khala_code_paid_plan_123',
      recordedAt: '2026-07-04T18:00:00.000Z',
      revenueSurfaceRef: 'khala_code.paid_plan',
      sourceRefs: ['route:/v1/khala-code/plans/purchases'],
    })

    expect(record).toMatchObject({
      demandProvenance: 'external',
      paymentState: 'fulfilled',
      amountCents: 1900,
    })

    const bundle = await readFirstDollarEvidenceBundle(
      db,
      bundleRef,
      '2026-07-04T18:01:00.000Z',
    )

    expect(bundle).toMatchObject({
      schemaVersion:
        'openagents.revenue_loop.first_dollar_evidence_bundle.v1',
      bundleRef,
      provenance: {
        label: 'external',
        rule: 'no_external_dollar_no_demand_claim',
      },
      revenueEvent: {
        amountCents: 1900,
        ledgerTable: 'khala_code_paid_plan_payment_intents',
        paymentState: 'fulfilled',
        productRef: 'khala_code',
      },
    })
    expect(bundle?.registryEvidenceRefs).toEqual(
      expect.arrayContaining([
        `receipt:${record.receiptRef}`,
        `ledger:revenue_event_provenance:${eventRef}`,
      ]),
    )
    expect(JSON.stringify(bundle)).not.toMatch(
      /checkout_url=|customer@example|lnbc1|payment_hash=|preimage=|sk-live|wallet_secret/i,
    )
  })

  test('serves bundles from the public route and rejects mutations', async () => {
    const db = makeDb()
    const eventRef = 'revenue_event.qa_swarm.first_engagement.qa_123'
    const bundleRef = firstDollarEvidenceBundleRef('qa_swarm', eventRef)
    await recordRevenueEventProvenance(db, {
      amountCents: 300000,
      amountSats: null,
      caveatRefs: ['caveat.revenue.qa_swarm.first_paid_delivery_not_claimed'],
      demandProvenance: 'external',
      eventRef,
      evidenceBundleRef: bundleRef,
      idempotencyKey: 'revenue-event-test-2',
      ledgerRowRef: 'receipt.qa_swarm.first_engagement.qa_123',
      ledgerTable: 'qa_swarm_first_engagements',
      paymentState: 'payment_evidence_recorded',
      productRef: 'qa_swarm',
      publicEvidenceRefs: ['receipt.qa_swarm.first_engagement.qa_123'],
      receiptRef: 'receipt.qa_swarm.first_engagement.qa_123',
      recordedAt: '2026-07-04T18:00:00.000Z',
      revenueSurfaceRef: 'qa_swarm.swarm_audit_first_engagement',
      sourceRefs: ['route:/api/operator/qa-swarm/first-engagements'],
    })

    const routes = makePublicFirstDollarEvidenceRoutes<{
      db: D1Database
    }>({
      makeDb: env => env.db,
      nowIso: () => '2026-07-04T18:01:00.000Z',
    })
    const request = new Request(
      `https://openagents.com${FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT}/${encodeURIComponent(
        bundleRef,
      )}`,
    )
    const response = await Effect.runPromise(
      routes.routePublicFirstDollarEvidenceRequest(request, { db })!,
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      bundle: { bundleRef: string; revenueEvent: { productRef: string } }
    }
    expect(body.bundle.bundleRef).toBe(bundleRef)
    expect(body.bundle.revenueEvent.productRef).toBe('qa_swarm')

    const mutation = await Effect.runPromise(
      routes.routePublicFirstDollarEvidenceRequest(
        new Request(request.url, { method: 'POST' }),
        { db },
      )!,
    )
    expect(mutation.status).toBe(405)
  })

  test('rejects unsafe refs instead of storing public evidence leaks', async () => {
    const db = makeDb()
    await expect(
      recordRevenueEventProvenance(db, {
        amountCents: 100,
        amountSats: null,
        caveatRefs: [],
        demandProvenance: 'external',
        eventRef: 'revenue_event.bad',
        evidenceBundleRef: 'evidence.revenue.first_dollar.bad',
        idempotencyKey: 'revenue-event-test-3',
        ledgerRowRef: 'row.bad',
        ledgerTable: 'qa_swarm_first_engagements',
        paymentState: 'payment_evidence_recorded',
        productRef: 'qa_swarm',
        publicEvidenceRefs: ['lnbc1rawinvoice'],
        receiptRef: 'receipt.qa_swarm.first_engagement.bad',
        recordedAt: '2026-07-04T18:00:00.000Z',
        revenueSurfaceRef: 'qa_swarm.swarm_audit_first_engagement',
        sourceRefs: [],
      }),
    ).rejects.toThrow('unsafe_revenue_evidence_ref')
  })
})
