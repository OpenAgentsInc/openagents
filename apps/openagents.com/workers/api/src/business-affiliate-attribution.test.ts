import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  createBusinessAffiliateCode,
  readBusinessAffiliateAttributionReport,
  type BusinessAffiliateAttributionRuntime,
} from './business-affiliate-attribution'
import {
  handleOperatorBusinessAffiliateAttributionApi,
  handleOperatorBusinessAffiliateCodeApi,
} from './business-affiliate-attribution-routes'
import {
  type BusinessPipelineRuntime,
  makeD1BusinessPipelineStore,
} from './business-pipeline-queue'
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
  ): Promise<ReadonlyArray<{ success: true }>> {
    return Promise.all(statements.map(statement => statement.run()))
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const SUPPORT_SCHEMA = `
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT,
  name TEXT,
  kind TEXT NOT NULL DEFAULT 'human',
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT,
  updated_at TEXT,
  deleted_at TEXT
);

CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT UNIQUE,
  kind TEXT NOT NULL DEFAULT 'organization',
  plan TEXT,
  logo_url TEXT,
  credits INTEGER,
  owner_user_id TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE team_projects (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (team_id, slug)
);

CREATE TABLE email_messages (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  provider TEXT,
  provider_message_id TEXT,
  subject TEXT,
  to_email TEXT,
  from_email TEXT,
  reply_to_email TEXT,
  template_slug TEXT,
  template_context_json TEXT,
  metadata_json TEXT,
  rendered_html TEXT,
  rendered_text TEXT,
  error_name TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const SCHEMA = [
  '0191_business_signup_requests.sql',
  '0216_business_signup_referral_attribution.sql',
  '0270_business_funnel_events.sql',
  '0190_prefilled_workspaces.sql',
  '0192_prefilled_workspace_invite_engagement.sql',
  '0195_private_prefilled_workspace_access.sql',
  '0194_team_workspace_invites.sql',
  '0271_business_signup_fulfillment.sql',
  '0272_business_checkout_kickoffs.sql',
  '0278_business_commitment_ledger.sql',
  '0294_business_pipeline_queue.sql',
  '0299_business_pipeline_partner_routing.sql',
  '0297_business_source_attribution.sql',
  '0298_business_affiliate_attribution.sql',
].map(migration)

type Db = Readonly<{ d1: D1Database; raw: DatabaseSync }>

const makeDb = (): Db => {
  const raw = new DatabaseSync(':memory:')
  raw.exec('PRAGMA foreign_keys = OFF;')
  raw.exec(SUPPORT_SCHEMA)
  for (const sql of SCHEMA) {
    raw.exec(sql)
  }
  return { d1: new SqliteD1(raw) as unknown as D1Database, raw }
}

let counter = 0

const runtime: BusinessSignupRuntime &
  BusinessPipelineRuntime &
  BusinessAffiliateAttributionRuntime = {
  makeId: (prefix: string) => `${prefix}_${(counter += 1)}`,
  nowIso: () => '2026-07-04T12:00:00.000Z',
  expiresAtFromNow: () => '2026-08-03T12:00:00.000Z',
}

const signup = (referralCode: string): Request =>
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
      referralCode,
    }),
  })

const createPipelineForSignup = (db: D1Database) =>
  makeD1BusinessPipelineStore(db).createPipelineRow(
    {
      businessSignupRequestId: 'business_signup_1',
      ownerRole: 'operator',
      pipelineRef: 'biz-pipe-affiliate-001',
      quotedBand: {
        label: 'agent readiness quick win',
        maxUsdCents: 500_000,
        minUsdCents: 150_000,
      },
      receiptRefs: ['receipt.business.intake.affiliate_001'],
      sourceRef: 'affiliate_launch-aug',
      vertical: 'agency',
    },
    runtime,
  )

beforeEach(() => {
  counter = 0
})

describe('business affiliate attribution', () => {
  test('captures active operator-issued referral codes through business signup', async () => {
    const db = makeDb()
    await createBusinessAffiliateCode(
      db.d1,
      {
        code: 'launch-aug',
        issuedByRef: 'operator.business',
        ownerRef: 'owner.partner.launch_aug',
      },
      runtime,
    )

    const response = await Effect.runPromise(
      handleBusinessSignupApi(signup('launch-aug'), db.d1, runtime),
    )
    expect(response.status).toBe(201)

    const record = await readBusinessSignupRequest(db.d1, 'business_signup_1')
    expect(record?.sourceRef).toBe('affiliate_launch-aug')

    const attribution = db.raw
      .prepare(
        `SELECT code, source_ref, owner_ref, business_signup_request_id,
                pipeline_ref, payment_receipt_ref
           FROM business_affiliate_attributions
          WHERE business_signup_request_id = ?`,
      )
      .get('business_signup_1') as Row | undefined

    expect(attribution).toMatchObject({
      business_signup_request_id: 'business_signup_1',
      code: 'launch-aug',
      owner_ref: 'owner.partner.launch_aug',
      source_ref: 'affiliate_launch-aug',
    })
    expect(attribution?.pipeline_ref).toBeNull()
    expect(attribution?.payment_receipt_ref).toBeNull()

    const report = await readBusinessAffiliateAttributionReport(
      db.d1,
      {
        code: 'launch-aug',
        nowIso: runtime.nowIso(),
      },
    )
    expect(report.totals).toEqual({
      attributedSignupCount: 1,
      paymentReceiptCount: 0,
      pipelineLinkedCount: 0,
    })
    expect(report.conversions[0]).toMatchObject({
      businessSignupRequestId: 'business_signup_1',
      intake: { ref: 'business_signup:business_signup_1', status: 'measured' },
      payment: { ref: null, status: 'not_measured' },
      pipeline: { ref: null, status: 'not_measured' },
    })
    expect(JSON.stringify(report)).not.toMatch(/lead@example\.com|555/i)
  })

  test('links code attribution to pipeline and exact payment receipt legs', async () => {
    const db = makeDb()
    await createBusinessAffiliateCode(
      db.d1,
      {
        code: 'launch-aug',
        ownerRef: 'owner.partner.launch_aug',
      },
      runtime,
    )
    await Effect.runPromise(
      handleBusinessSignupApi(signup('launch-aug'), db.d1, runtime),
    )
    await createPipelineForSignup(db.d1)
    await db.d1
      .prepare(
        `INSERT INTO business_checkout_kickoffs
          (checkout_session_id,
           business_signup_request_id,
           user_id,
           total_amount_cents,
           setup_fee_cents,
           credit_grant_cents,
           workspace_id,
           service_promise_contract_id,
           public_receipt_ref,
           created_at,
           updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        'cs_affiliate_001',
        'business_signup_1',
        'user_customer_001',
        500_000,
        500_000,
        0,
        'workspace_affiliate_001',
        'contract_affiliate_001',
        'receipt.billing.stripe_checkout.cs_affiliate_001',
        runtime.nowIso(),
        runtime.nowIso(),
      )
      .run()

    const report = await readBusinessAffiliateAttributionReport(
      db.d1,
      {
        code: 'launch-aug',
        nowIso: runtime.nowIso(),
      },
    )

    expect(report.totals).toEqual({
      attributedSignupCount: 1,
      paymentReceiptCount: 1,
      pipelineLinkedCount: 1,
    })
    expect(report.rates).toMatchObject({
      intakeToPayment: {
        denominator: 1,
        numerator: 1,
        status: 'measured',
        value: 1,
      },
      intakeToPipeline: {
        denominator: 1,
        numerator: 1,
        status: 'measured',
        value: 1,
      },
    })
    expect(report.conversions[0]).toMatchObject({
      payment: {
        ref: 'receipt.billing.stripe_checkout.cs_affiliate_001',
        status: 'measured',
      },
      pipeline: {
        ref: 'biz-pipe-affiliate-001',
        status: 'measured',
      },
    })
    expect(report.authorityBoundary).toContain('grants no payout')
  })

  test('operator routes issue codes and read exact-only reports behind admin auth', async () => {
    const db = makeDb()
    const authed = {
      requireAdminApiToken: async (request: Request) =>
        request.headers.get('authorization') === 'Bearer test-admin',
      runtime,
    }
    const issue = await Effect.runPromise(
      handleOperatorBusinessAffiliateCodeApi(
        new Request('https://openagents.com/api/operator/business/affiliate-codes', {
          method: 'POST',
          headers: {
            authorization: 'Bearer test-admin',
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            code: 'launch-aug',
            ownerRef: 'owner.partner.launch_aug',
          }),
        }),
        db.d1,
        authed,
      ),
    )

    expect(issue.status).toBe(201)
    expect(await issue.json()).toMatchObject({
      code: {
        code: 'launch-aug',
        sourceRef: 'affiliate_launch-aug',
      },
    })

    const denied = await Effect.runPromise(
      handleOperatorBusinessAffiliateAttributionApi(
        new Request(
          'https://openagents.com/api/operator/business/affiliate-attribution?code=launch-aug',
        ),
        db.d1,
        authed,
      ),
    )
    expect(denied.status).toBe(401)

    const report = await Effect.runPromise(
      handleOperatorBusinessAffiliateAttributionApi(
        new Request(
          'https://openagents.com/api/operator/business/affiliate-attribution?code=launch-aug',
          { headers: { authorization: 'Bearer test-admin' } },
        ),
        db.d1,
        authed,
      ),
    )
    expect(report.status).toBe(200)
    expect(await report.json()).toMatchObject({
      report: {
        code: {
          ownerRef: 'owner.partner.launch_aug',
        },
        totals: {
          attributedSignupCount: 0,
          paymentReceiptCount: 0,
          pipelineLinkedCount: 0,
        },
      },
    })
  })
})
