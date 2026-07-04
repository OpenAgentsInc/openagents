import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS,
  lintBusinessOutreachClaims,
  makeD1BusinessOutreachStore,
} from './business-outreach'
import {
  makeD1BusinessPipelineStore,
  type BusinessPipelineRuntime,
} from './business-pipeline-queue'
import { makeOperatorBusinessOutreachRoutes } from './business-outreach-routes'

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
  db.exec(migration('0296_business_outreach_sequences.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const runtime: BusinessPipelineRuntime = {
  makeId: (prefix: string) => `${prefix}_test`,
  nowIso: () => '2026-07-04T12:00:00.000Z',
}

const PIPELINE_REF = 'biz-pipe-2026w27-outreach-001'
const SUBJECT_REF = 'prospect.agent_ready.001'
const TEMPLATE_REF = 'business.outreach.agent_readiness_ecommerce.report_led.v1'
const MODEL_CUSTODY_TEMPLATE_REF =
  'business.outreach.model_custody_regulated.reactor_assessment.v1'
const APPROVAL_RECEIPT_REF = 'receipt.owner.template_approval.lg4.001'

const makeStores = (db: D1Database) => {
  const pipelineStore = makeD1BusinessPipelineStore(db)
  return {
    outreachStore: makeD1BusinessOutreachStore(db, pipelineStore),
    pipelineStore,
  }
}

const seedPipeline = async (
  db: D1Database,
  overrides: Partial<Parameters<ReturnType<typeof makeD1BusinessPipelineStore>['createPipelineRow']>[0]> = {},
): Promise<void> => {
  await makeStores(db).pipelineStore.createPipelineRow(
    {
      ownerRole: 'operator',
      pipelineRef: PIPELINE_REF,
      sourceRef: 'apollo_agent_readiness_ecommerce',
      vertical: 'e-commerce',
      ...overrides,
    },
    runtime,
  )
}

const authedRoutes = (db: D1Database) =>
  makeOperatorBusinessOutreachRoutes({
    makeStore: () => makeStores(db).outreachStore,
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
  const routed = authedRoutes(db).routeOperatorBusinessOutreachRequest(
    request,
    {} as never,
    {} as ExecutionContext,
  )
  if (routed === undefined) throw new Error('route did not match')
  return Effect.runPromise(routed)
}

const renderDraft = async (
  db: D1Database,
  body: Record<string, unknown> = {},
): Promise<Response> =>
  runRoute(
    db,
    operatorRequest(`/api/operator/business/pipeline/${PIPELINE_REF}/outreach-drafts`, {
      body: JSON.stringify({
        auditReportRef: 'audit.agent_readiness.001',
        findingRefs: ['finding.agent_readiness.blank_shell'],
        observedFact:
          'your public site gives agents a thin shell before the useful product details',
        subjectRef: SUBJECT_REF,
        ...body,
      }),
      method: 'POST',
    }),
  )

describe('business outreach sequence routes', () => {
  test('renders a drafts-first report-led sequence from a pipeline row and audit refs', async () => {
    const db = makeDb()
    await seedPipeline(db)

    const response = await renderDraft(db)
    const body = await response.json() as {
      draft: {
        auditReportRef: string
        bodyText: string
        claimLintRefs: ReadonlyArray<string>
        findingRefs: ReadonlyArray<string>
        state: string
      }
    }

    expect(response.status, JSON.stringify(body)).toBe(201)
    expect(body.draft).toMatchObject({
      auditReportRef: 'audit.agent_readiness.001',
      claimLintRefs: [],
      findingRefs: ['finding.agent_readiness.blank_shell'],
      state: 'draft',
    })
    expect(body.draft.bodyText).toContain('Observed fact:')
    expect(body.draft.bodyText).toContain('Agent-Ready Quick Win')
    expect(body.draft.bodyText).toContain('Registry-true proof point')
    expect(body.draft.bodyText).toContain('Open to a 15-minute walkthrough')
    expect(body.draft.bodyText).toContain('Reply opt out')
    expect(body.draft.bodyText).not.toMatch(
      /lead@example\.com|private\.example\.com|raw crm|apollo payload/i,
    )
  })

  test('enforces suppression list and active-intake refusal at render time', async () => {
    const db = makeDb()
    await seedPipeline(db)

    const suppression = await runRoute(
      db,
      operatorRequest('/api/operator/business/outreach/suppressions', {
        body: JSON.stringify({
          reason: 'existing_customer',
          sourceRef: 'crm.suppression.20260704',
          subjectRef: SUBJECT_REF,
        }),
        method: 'POST',
      }),
    )
    expect(suppression.status).toBe(201)

    const suppressed = await renderDraft(db)
    expect(suppressed.status).toBe(409)
    expect(await suppressed.json()).toMatchObject({
      error: 'business_outreach_refused',
      reason: 'suppressed_subject',
      suppression: { reason: 'existing_customer' },
    })

    const activeDb = makeDb()
    await seedPipeline(activeDb, {
      pipelineRef: 'biz-pipe-2026w27-outreach-active',
      receiptRefs: ['receipt.business.scope_scheduled.001'],
      stage: 'scope_scheduled',
    })
    const active = await runRoute(
      activeDb,
      operatorRequest(
        '/api/operator/business/pipeline/biz-pipe-2026w27-outreach-active/outreach-drafts',
        {
          body: JSON.stringify({
            auditReportRef: 'audit.agent_readiness.002',
            subjectRef: 'prospect.agent_ready.002',
          }),
          method: 'POST',
        },
      ),
    )
    expect(active.status).toBe(409)
    expect(await active.json()).toMatchObject({
      error: 'business_outreach_refused',
      reason: 'active_intake',
    })
  })

  test('keeps built-in templates claims-lint clean and refuses gated draft claims', async () => {
    for (const template of BUSINESS_OUTREACH_TEMPLATE_VERSIONS) {
      expect(
        lintBusinessOutreachClaims(
          [
            template.offerSentence,
            template.proofPoint,
            template.cta,
            template.identificationOptOut,
          ].join('\n'),
        ),
      ).toEqual([])
    }

    const db = makeDb()
    await seedPipeline(db)
    const response = await renderDraft(db, {
      observedFact: 'HIPAA-ready self-serve delivery is available now',
    })
    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      claimLintRefs: [
        'claim_lint.self_serve_delivery',
        'claim_lint.hipaa_sovereign',
      ],
      error: 'business_outreach_refused',
      reason: 'claim_lint_failed',
    })
  })

  test('renders the RX-8 regulated model-custody Reactor Assessment template without gated claims', async () => {
    const db = makeDb()
    await seedPipeline(db, {
      sourceRef: 'apollo_model_custody',
      vertical: 'regulated legal',
    })

    const response = await renderDraft(db, {
      auditReportRef: 'model_custody.report.001',
      findingRefs: ['model_custody.finding.frontier_lab_subprocessor'],
      observedFact:
        'public subprocessors page names OpenAI and Anthropic as AI providers',
      subjectRef: 'prospect.model_custody.001',
      templateVersionRef: MODEL_CUSTODY_TEMPLATE_REF,
    })
    const body = await response.json() as {
      draft: {
        bodyText: string
        claimLintRefs: ReadonlyArray<string>
        segmentRef: string
        sourceRef: string
      }
    }

    expect(response.status, JSON.stringify(body)).toBe(201)
    expect(body.draft).toMatchObject({
      claimLintRefs: [],
      segmentRef: 'model_custody_regulated',
      sourceRef: 'apollo_model_custody',
    })
    expect(body.draft.bodyText).toContain('Reactor Assessment')
    expect(body.draft.bodyText).toContain('Friedberg/Mistral')
    expect(body.draft.bodyText).toContain('Own Your AI')
    expect(body.draft.bodyText).not.toMatch(/\b(HIPAA|sovereign|\$\s?\d)/i)
  })

  test('requires owner-approved template versions and enforces mailbox send caps', async () => {
    const db = makeDb()
    await seedPipeline(db)
    const draftResponse = await renderDraft(db, {
      draftRef: 'business.outreach.draft.001',
    })
    expect(draftResponse.status).toBe(201)

    const sendBeforeApproval = await runRoute(
      db,
      operatorRequest(`/api/operator/business/pipeline/${PIPELINE_REF}/outreach-sends`, {
        body: JSON.stringify({
          dailyMailboxSendCap: 1,
          draftRef: 'business.outreach.draft.001',
          mailboxRef: 'mailbox.operator.apollo',
          sourceRef: 'apollo.sequence.agent_readiness_ecommerce',
        }),
        method: 'POST',
      }),
    )
    expect(sendBeforeApproval.status).toBe(409)
    expect(await sendBeforeApproval.json()).toMatchObject({
      error: 'business_outreach_refused',
      reason: 'template_not_approved',
    })

    const approval = await runRoute(
      db,
      operatorRequest('/api/operator/business/outreach/template-approvals', {
        body: JSON.stringify({
          approvalReceiptRef: APPROVAL_RECEIPT_REF,
          approvedByRef: 'owner.openagents',
          sourceRef: 'github:OpenAgentsInc/openagents#8265',
          templateVersionRef: TEMPLATE_REF,
        }),
        method: 'POST',
      }),
    )
    expect(approval.status).toBe(201)

    const send = await runRoute(
      db,
      operatorRequest(`/api/operator/business/pipeline/${PIPELINE_REF}/outreach-sends`, {
        body: JSON.stringify({
          approvalReceiptRef: APPROVAL_RECEIPT_REF,
          dailyMailboxSendCap: 1,
          draftRef: 'business.outreach.draft.001',
          mailboxRef: 'mailbox.operator.apollo',
          sendRef: 'business.outreach.send.001',
          sourceRef: 'apollo.sequence.agent_readiness_ecommerce',
        }),
        method: 'POST',
      }),
    )
    const sendBody = await send.json() as {
      pipelineReceiptRefs: ReadonlyArray<string>
      send: { sendReceiptRef: string; sourceRef: string }
    }
    expect(send.status, JSON.stringify(sendBody)).toBe(201)
    expect(sendBody.send).toMatchObject({
      sendReceiptRef: 'receipt.business.outreach_send.business.outreach.send.001',
      sourceRef: 'apollo.sequence.agent_readiness_ecommerce',
    })
    expect(sendBody.pipelineReceiptRefs).toContain(sendBody.send.sendReceiptRef)

    const capped = await runRoute(
      db,
      operatorRequest(`/api/operator/business/pipeline/${PIPELINE_REF}/outreach-sends`, {
        body: JSON.stringify({
          approvalReceiptRef: APPROVAL_RECEIPT_REF,
          dailyMailboxSendCap: 1,
          draftRef: 'business.outreach.draft.001',
          mailboxRef: 'mailbox.operator.apollo',
          sendRef: 'business.outreach.send.002',
          sourceRef: 'apollo.sequence.agent_readiness_ecommerce',
        }),
        method: 'POST',
      }),
    )
    expect(capped.status).toBe(409)
    expect(await capped.json()).toMatchObject({
      error: 'business_outreach_refused',
      reason: 'daily_mailbox_send_cap_exceeded',
    })
  })

  test('requires the operator admin bearer token', async () => {
    const db = makeDb()
    const request = new Request(
      'https://openagents.com/api/operator/business/outreach/templates',
    )
    const response = await runRoute(db, request)

    expect(response.status).toBe(401)
  })
})
