import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  makeD1BusinessPipelineStore,
  type BusinessPipelineRuntime,
} from './business-pipeline-queue'
import { makeOperatorBusinessPipelineRoutes } from './business-pipeline-routes'

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
  db.exec(migration('0314_business_pipeline_subject_ref.sql'))
  db.exec(migration('0297_business_source_attribution.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const insertSignup = async (
  db: D1Database,
  sourceRef = 'apollo_agent_readiness_ecommerce',
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO business_signup_requests (
        id,
        business_name,
        contact_email,
        phone,
        request_slack_channel,
        slack_connect_status,
        source_route,
        source_ref,
        created_at,
        updated_at
      ) VALUES (?, ?, ?, ?, 0, 'not_requested', '/business', ?, ?, ?)`,
    )
    .bind(
      'business_signup_001',
      'Example Prospect',
      'lead@example.com',
      '+1 555 0100',
      sourceRef,
      runtime.nowIso(),
      runtime.nowIso(),
    )
    .run()
}

const runtime: BusinessPipelineRuntime = {
  makeId: (prefix: string) => `${prefix}_test`,
  nowIso: () => '2026-07-04T12:00:00.000Z',
}

const authedRoutes = (db: D1Database) =>
  makeOperatorBusinessPipelineRoutes({
    makeStore: () => makeD1BusinessPipelineStore(db),
    nowIso: runtime.nowIso,
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
  const routed = authedRoutes(db).routeOperatorBusinessPipelineRequest(
    request,
    {} as never,
    {} as ExecutionContext,
  )
  if (routed === undefined) throw new Error('route did not match')
  return Effect.runPromise(routed)
}

const waveProspects = (
  prefix: string,
  count: number,
  vertical: string,
): ReadonlyArray<Record<string, unknown>> =>
  Array.from({ length: count }, (_, index) => {
    const n = String(index + 1).padStart(3, '0')
    return {
      pipelineRef: `biz-pipe-${prefix}-${n}`,
      quotedBandLabel: 'audit first',
      quotedMaxUsdCents: 500_000,
      quotedMinUsdCents: 150_000,
      subjectRef: `prospect.${prefix}.${n}`,
      vertical,
    }
  })

describe('business pipeline queue routes', () => {
  test('ingests Apollo waves at volume with source attribution, suppression, and idempotent replays', async () => {
    const db = makeDb()
    await db
      .prepare(
        `INSERT INTO business_outreach_suppressions (
          suppression_ref,
          subject_ref,
          reason,
          source_ref,
          created_at
        ) VALUES (?, ?, 'existing_customer', ?, ?)`,
      )
      .bind(
        'business.outreach.suppression.agency_050',
        'prospect.agency.050',
        'crm.suppression.20260708',
        runtime.nowIso(),
      )
      .run()

    const agencyWave = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/apollo-waves', {
        body: JSON.stringify({
          prospects: waveProspects('agency', 100, 'agency'),
          segmentRef: 'segment.apollo.agencies_seo',
          sourceRef: 'apollo_agent_readiness_agency',
          waveRef: 'apollo.wave.agencies_seo.20260708a',
        }),
        method: 'POST',
      }),
    )
    const agencyBody = await agencyWave.json() as {
      wave: {
        acceptedCount: number
        duplicateCount: number
        duplicates: ReadonlyArray<string>
        inserted: ReadonlyArray<{
          pipelineRef: string
          sourceRef: string
          subjectRef: string | null
        }>
        suppressed: ReadonlyArray<{ subjectRef: string }>
        suppressedCount: number
      }
    }
    expect(agencyWave.status, JSON.stringify(agencyBody)).toBe(201)
    expect(agencyBody.wave).toMatchObject({
      acceptedCount: 99,
      duplicateCount: 0,
      suppressed: [{ subjectRef: 'prospect.agency.050' }],
      suppressedCount: 1,
    })
    expect(
      agencyBody.wave.inserted.every(
        row => row.sourceRef === 'apollo_agent_readiness_agency',
      ),
    ).toBe(true)
    expect(agencyBody.wave.inserted).toContainEqual(
      expect.objectContaining({
        pipelineRef: 'biz-pipe-agency-001',
        subjectRef: 'prospect.agency.001',
      }),
    )
    expect(
      agencyBody.wave.inserted.some(
        row => row.subjectRef === 'prospect.agency.050',
      ),
    ).toBe(false)

    const legalWave = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/apollo-waves', {
        body: JSON.stringify({
          prospects: waveProspects('legal', 100, 'regulated legal'),
          segmentRef: 'segment.apollo.legal_small_firm',
          sourceRef: 'apollo_model_custody',
          waveRef: 'apollo.wave.legal_small_firm.20260708a',
        }),
        method: 'POST',
      }),
    )
    expect(legalWave.status).toBe(201)
    expect(await legalWave.json()).toMatchObject({
      wave: {
        acceptedCount: 100,
        duplicateCount: 0,
        sourceRef: 'apollo_model_custody',
        suppressedCount: 0,
      },
    })

    const replay = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/apollo-waves', {
        body: JSON.stringify({
          prospects: waveProspects('agency', 100, 'agency'),
          segmentRef: 'segment.apollo.agencies_seo',
          sourceRef: 'apollo_agent_readiness_agency',
          waveRef: 'apollo.wave.agencies_seo.20260708a',
        }),
        method: 'POST',
      }),
    )
    expect(replay.status).toBe(201)
    const replayBody = await replay.json() as {
      wave: {
        acceptedCount: number
        duplicateCount: number
        duplicates: ReadonlyArray<string>
        suppressedCount: number
      }
    }
    expect(replayBody).toMatchObject({
      wave: {
        acceptedCount: 0,
        duplicateCount: 99,
        suppressedCount: 1,
      },
    })
    expect(replayBody.wave.duplicates).toContain('biz-pipe-agency-001')
    expect(replayBody.wave.duplicates).not.toContain('biz-pipe-agency-050')

    const sameSubjectSecondWave = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/apollo-waves', {
        body: JSON.stringify({
          prospects: [{
            pipelineRef: 'biz-pipe-agency-followup-001',
            quotedBandLabel: 'audit first',
            quotedMaxUsdCents: 500_000,
            quotedMinUsdCents: 150_000,
            subjectRef: 'prospect.agency.001',
            vertical: 'agency',
          }],
          segmentRef: 'segment.apollo.agencies_seo',
          sourceRef: 'apollo_agent_readiness_agency',
          waveRef: 'apollo.wave.agencies_seo.20260708b',
        }),
        method: 'POST',
      }),
    )
    expect(sameSubjectSecondWave.status).toBe(201)
    expect(await sameSubjectSecondWave.json()).toMatchObject({
      wave: {
        acceptedCount: 0,
        duplicateCount: 1,
        duplicates: ['biz-pipe-agency-followup-001'],
        suppressedCount: 0,
      },
    })

    const list = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline'),
    )
    const listBody = await list.json() as {
      rows: ReadonlyArray<{ pipelineRef: string; subjectRef: string | null }>
    }
    expect(
      listBody.rows.filter(row => row.subjectRef === 'prospect.agency.001'),
    ).toEqual([
      expect.objectContaining({
        pipelineRef: 'biz-pipe-agency-001',
        subjectRef: 'prospect.agency.001',
      }),
    ])

    const metrics = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/metrics'),
    )
    const metricsBody = await metrics.json() as {
      commitmentCoverage: { totalPipelineRowCount: number }
      sourceRefBreakdown: ReadonlyArray<{ rowCount: number; sourceRef: string }>
    }
    expect(metricsBody.commitmentCoverage.totalPipelineRowCount).toBe(199)
    expect(metricsBody.sourceRefBreakdown).toContainEqual(
      expect.objectContaining({
        rowCount: 99,
        sourceRef: 'apollo_agent_readiness_agency',
      }),
    )
    expect(metricsBody.sourceRefBreakdown).toContainEqual(
      expect.objectContaining({
        rowCount: 100,
        sourceRef: 'apollo_model_custody',
      }),
    )
    expect(JSON.stringify(metricsBody)).not.toMatch(
      /lead@example\.com|apollo payload|private\.example\.com|prospect\.agency\.050/i,
    )
  })

  test('creates, advances, and measures opaque pipeline rows with linked commitments', async () => {
    const db = makeDb()
    await insertSignup(db)

    const create = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline', {
        body: JSON.stringify({
          ownerRole: 'operator',
          businessSignupRequestId: 'business_signup_001',
          partnerRouteFlag: true,
          pipelineRef: 'biz-pipe-2026w27-001',
          quotedBandLabel: 'agent-ready quick win',
          quotedMaxUsdCents: 500_000,
          quotedMinUsdCents: 150_000,
          receiptRefs: ['receipt.business.intake.20260704.001'],
          sourceRef: 'apollo_agent_readiness_ecommerce',
          vertical: 'e-commerce',
        }),
        method: 'POST',
      }),
    )
    expect(create.status).toBe(201)
    expect(await create.json()).toMatchObject({
      row: {
        businessSignupRequestId: 'business_signup_001',
        partnerRoute: {
          approvalReceiptRef: null,
          state: 'candidate',
        },
        partnerRouteFlag: true,
        pipelineRef: 'biz-pipe-2026w27-001',
        provenanceLabel: 'partner',
        stage: 'intake_received',
      },
    })
    const linkedSignup = await db
      .prepare(
        `SELECT linked_pipeline_ref, source_ref
           FROM business_signup_requests
          WHERE id = ?`,
      )
      .bind('business_signup_001')
      .first<Row>()
    expect(linkedSignup).toMatchObject({
      linked_pipeline_ref: 'biz-pipe-2026w27-001',
      source_ref: 'apollo_agent_readiness_ecommerce',
    })

    const advance = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-001/advance',
        {
          body: JSON.stringify({
            nextActionDueAt: '2026-07-05',
            receiptRef: 'receipt.business.scope_scheduled.20260704.001',
            stage: 'scope_scheduled',
          }),
          method: 'POST',
        },
      ),
    )
    expect(advance.status).toBe(200)
    expect(await advance.json()).toMatchObject({
      row: {
        receiptRefs: [
          'receipt.business.intake.20260704.001',
          'receipt.business.scope_scheduled.20260704.001',
        ],
        stage: 'scope_scheduled',
      },
    })

    const missingApproval = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-001/partner-route',
        {
          body: JSON.stringify({
            budgetRangeRef: 'budget_range.partner.offer_001',
            dueWindowRef: 'due_window.partner.offer_001',
            offerRef: 'overflow_offer.partner_001',
            peerRef: 'peer.partner_agency_001',
            privacyTierRef: 'privacy_tier.standard',
            scopeSummaryRef: 'scope_summary.partner.offer_001',
            state: 'offered',
          }),
          method: 'POST',
        },
      ),
    )
    expect(missingApproval.status).toBe(400)
    expect(await missingApproval.json()).toMatchObject({
      error: 'business_pipeline_validation_error',
      reason: expect.stringContaining('approvalReceiptRef'),
    })

    const offer = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-001/partner-route',
        {
          body: JSON.stringify({
            approvalReceiptRef: 'receipt.operator.partner_route_approval.001',
            budgetRangeRef: 'budget_range.partner.offer_001',
            dueWindowRef: 'due_window.partner.offer_001',
            offerRef: 'overflow_offer.partner_001',
            peerRef: 'peer.partner_agency_001',
            privacyTierRef: 'privacy_tier.standard',
            scopeSummaryRef: 'scope_summary.partner.offer_001',
            state: 'offered',
          }),
          method: 'POST',
        },
      ),
    )
    expect(offer.status).toBe(200)
    expect(await offer.json()).toMatchObject({
      row: {
        partnerRoute: {
          approvalReceiptRef: 'receipt.operator.partner_route_approval.001',
          budgetRangeRef: 'budget_range.partner.offer_001',
          dueWindowRef: 'due_window.partner.offer_001',
          offerRef: 'overflow_offer.partner_001',
          peerRef: 'peer.partner_agency_001',
          privacyTierRef: 'privacy_tier.standard',
          scopeSummaryRef: 'scope_summary.partner.offer_001',
          state: 'offered',
        },
        partnerRouteFlag: true,
        provenanceLabel: 'partner',
        receiptRefs: [
          'receipt.business.intake.20260704.001',
          'receipt.business.scope_scheduled.20260704.001',
          'receipt.operator.partner_route_approval.001',
        ],
      },
    })

    const accepted = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-001/partner-route',
        {
          body: JSON.stringify({
            state: 'accepted',
          }),
          method: 'POST',
        },
      ),
    )
    expect(accepted.status).toBe(200)
    expect(await accepted.json()).toMatchObject({
      row: {
        partnerRoute: {
          approvalReceiptRef: 'receipt.operator.partner_route_approval.001',
          peerRef: 'peer.partner_agency_001',
          state: 'accepted',
        },
      },
    })

    const commitment = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-001/commitments',
        {
          body: JSON.stringify({
            commitmentKind: 'send',
            commitmentRef: 'business.commitment.send.report_001.20260704',
            dueAt: '2026-07-05T17:00:00.000Z',
            evidenceRefs: ['receipt.business.scope_scheduled.20260704.001'],
            ownerRef: 'owner.business.ops',
            promisedObjectRef: 'send.business.agent_readiness_report_001',
            sourceRefs: ['github:OpenAgentsInc/openagents#8263'],
          }),
          method: 'POST',
        },
      ),
    )
    expect(commitment.status).toBe(201)
    expect(await commitment.json()).toMatchObject({
      commitment: {
        commitmentRef: 'business.commitment.send.report_001.20260704',
        pipelineRef: 'biz-pipe-2026w27-001',
      },
    })

    const metrics = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/metrics'),
    )
    const body = await metrics.json() as {
      commitmentCoverage: {
        linkedPipelineRowCount: number
        missingCommitmentDefects: ReadonlyArray<string>
      }
      qualifiedPipeline: {
        maxUsdCents: number
        minUsdCents: number
        status: string
        targetUsdCents: number
      }
      provenanceBreakdown: ReadonlyArray<{
        provenanceLabel: string
        qualifiedPipeline: {
          maxUsdCents: number
          minUsdCents: number
          status: string
        }
        rowCount: number
      }>
      rates: { intakeToScopeRate: { status: string } }
      sourceRefBreakdown: ReadonlyArray<{
        qualifiedPipeline: { maxUsdCents: number; minUsdCents: number }
        rowCount: number
        sourceRef: string
      }>
    }

    expect(metrics.status).toBe(200)
    expect(body.qualifiedPipeline).toMatchObject({
      maxUsdCents: 500_000,
      minUsdCents: 150_000,
      status: 'measured',
      targetUsdCents: 2_500_000,
    })
    expect(body.provenanceBreakdown).toContainEqual({
      provenanceLabel: 'partner',
      qualifiedPipeline: expect.objectContaining({
        maxUsdCents: 500_000,
        minUsdCents: 150_000,
        status: 'measured',
      }),
      rowCount: 1,
    })
    expect(body.rates.intakeToScopeRate.status).toBe('measured')
    expect(body.commitmentCoverage).toMatchObject({
      linkedPipelineRowCount: 1,
      missingCommitmentDefects: [],
    })
    expect(body.sourceRefBreakdown).toContainEqual({
      qualifiedPipeline: expect.objectContaining({
        maxUsdCents: 500_000,
        minUsdCents: 150_000,
      }),
      rates: expect.any(Object),
      rowCount: 1,
      sourceRef: 'apollo_agent_readiness_ecommerce',
      stageCounts: expect.any(Array),
    })
    expect(JSON.stringify(body)).not.toMatch(
      /lead@example\.com|private\.example\.com|raw call note|customer name/i,
    )
  })

  test('accepts quoted Reactor Assessment rows from the RX-8 model-custody source ref', async () => {
    const db = makeDb()

    const create = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline', {
        body: JSON.stringify({
          ownerRole: 'operator',
          pipelineRef: 'biz-pipe-2026w27-model-custody-001',
          quotedBandLabel: 'Reactor Assessment',
          quotedMaxUsdCents: 1_500_000,
          quotedMinUsdCents: 750_000,
          receiptRefs: ['receipt.business.model_custody_intake.001'],
          sourceRef: 'apollo_model_custody',
          vertical: 'regulated legal',
        }),
        method: 'POST',
      }),
    )
    expect(create.status).toBe(201)
    expect(await create.json()).toMatchObject({
      row: {
        pipelineRef: 'biz-pipe-2026w27-model-custody-001',
        provenanceLabel: 'direct',
        quotedBand: {
          label: 'Reactor Assessment',
          maxUsdCents: 1_500_000,
          minUsdCents: 750_000,
        },
        sourceRef: 'apollo_model_custody',
      },
    })

    const advance = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-model-custody-001/advance',
        {
          body: JSON.stringify({
            receiptRef: 'receipt.business.model_custody_scope_scheduled.001',
            stage: 'scope_scheduled',
          }),
          method: 'POST',
        },
      ),
    )
    expect(advance.status).toBe(200)

    const metrics = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/metrics'),
    )
    const body = await metrics.json() as {
      qualifiedPipeline: {
        maxUsdCents: number
        minUsdCents: number
        status: string
      }
      sourceRefBreakdown: ReadonlyArray<{
        qualifiedPipeline: {
          maxUsdCents: number
          minUsdCents: number
          status: string
        }
        rowCount: number
        sourceRef: string
      }>
    }

    expect(body.qualifiedPipeline).toMatchObject({
      maxUsdCents: 1_500_000,
      minUsdCents: 750_000,
      status: 'measured',
    })
    expect(body.sourceRefBreakdown).toContainEqual({
      qualifiedPipeline: expect.objectContaining({
        maxUsdCents: 1_500_000,
        minUsdCents: 750_000,
        status: 'measured',
      }),
      rates: expect.any(Object),
      rowCount: 1,
      sourceRef: 'apollo_model_custody',
      stageCounts: expect.any(Array),
    })
  })

  test('rejects private prospect fields and receipt-less stage transitions', async () => {
    const db = makeDb()

    const unsafe = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline', {
        body: JSON.stringify({
          ownerRole: 'operator',
          pipelineRef: 'biz-pipe-unsafe',
          sourceRef: 'https://private.example.com',
          vertical: 'e-commerce',
        }),
        method: 'POST',
      }),
    )
    expect(unsafe.status).toBe(400)
    expect(await unsafe.json()).toMatchObject({
      error: 'business_pipeline_validation_error',
    })

    await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline', {
        body: JSON.stringify({
          ownerRole: 'operator',
          pipelineRef: 'biz-pipe-2026w27-002',
          sourceRef: 'apollo_agent_readiness_saas',
          vertical: 'saas',
        }),
        method: 'POST',
      }),
    )

    const missingReceipt = await runRoute(
      db,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-002/advance',
        {
          body: JSON.stringify({
            stage: 'scope_scheduled',
          }),
          method: 'POST',
        },
      ),
    )
    expect(missingReceipt.status).toBe(400)
    expect(await missingReceipt.json()).toMatchObject({
      error: 'business_pipeline_validation_error',
    })

    const metrics = await runRoute(
      db,
      operatorRequest('/api/operator/business/pipeline/metrics'),
    )
    expect(await metrics.json()).toMatchObject({
      commitmentCoverage: {
        missingCommitmentDefects: [
          'commitment.untracked pipelineRef=biz-pipe-2026w27-002 vertical=saas',
        ],
      },
      qualifiedPipeline: {
        status: 'not_measured',
      },
    })
  })

  test('requires the operator admin bearer token', async () => {
    const db = makeDb()
    const request = new Request('https://openagents.com/api/operator/business/pipeline')
    const response = await runRoute(db, request)

    expect(response.status).toBe(401)
  })
})
