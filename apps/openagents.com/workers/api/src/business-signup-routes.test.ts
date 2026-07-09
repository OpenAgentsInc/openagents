import { Effect, Redacted } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { beforeEach, describe, expect, test } from 'vitest'

import { ResendEmailSender, WorkerSecret } from './config'
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
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as Array<T>
    return { results }
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

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<ReadonlyArray<{ success: true }>> {
    return Promise.all(statements.map(statement => statement.run()))
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
  '0190_prefilled_workspaces.sql',
  '0192_prefilled_workspace_invite_engagement.sql',
  '0195_private_prefilled_workspace_access.sql',
  '0194_team_workspace_invites.sql',
  '0271_business_signup_fulfillment.sql',
  '0278_business_commitment_ledger.sql',
  '0294_business_pipeline_queue.sql',
  '0299_business_pipeline_partner_routing.sql',
  '0314_business_pipeline_subject_ref.sql',
  '0297_business_source_attribution.sql',
].map(migration)

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

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(SUPPORT_SCHEMA)
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
  test('creates workspace, invite, fulfillment receipt, and accepted email evidence', async () => {
    const db = makeDb()
    const response = await Effect.runPromise(
      handleBusinessSignupApi(
        new Request('https://openagents.com/api/public/business-signup', {
          method: 'POST',
          headers: {
            accept: 'application/json',
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            businessName: 'Acme Agency',
            contactEmail: 'lead@example.com',
            helpWith: 'Need a landing page and email launch workflow.',
            phone: '+1 555 000 0000',
          }),
        }),
        db,
        runtime,
        {
          getResendEmailConfig: () => ({
            apiKey: Redacted.make(WorkerSecret.make('resend_test_key')),
            fromEmail: ResendEmailSender.make(
              'OpenAgents <hello@example.com>',
            ),
          }),
          sendInviteEmailWithLedger: (_config, input) =>
            Effect.tryPromise({
              try: async () => {
                const emailMessageId = `email_msg_${input.inviteId}`
                await db
                  .prepare(
                    `INSERT INTO email_messages
                      (id, idempotency_key, kind, status, created_at,
                       updated_at)
                     VALUES (?, ?, 'operator_notification', 'accepted', ?, ?)`,
                  )
                  .bind(
                    emailMessageId,
                    input.idempotencyKey,
                    runtime.nowIso(),
                    runtime.nowIso(),
                  )
                  .run()

                return {
                  emailMessageId,
                  ok: true as const,
                  providerMessageId: 'resend_message_1',
                }
              },
              catch: error => error as never,
            }),
        },
      ),
    )
    const body = await response.json<{
      request: Readonly<{
        fulfillmentRef: string
        fulfillmentStatus: string
        nextAction: string
      }>
    }>()

    const fulfillment = await db
      .prepare(
        `SELECT status, workspace_id, invite_id, email_message_id,
                email_delivery_status, reason
           FROM business_signup_fulfillments
          WHERE business_signup_request_id = ?`,
      )
      .bind('business_signup_1')
      .first<Row>()

    expect(response.status).toBe(201)
    expect(body.request).toMatchObject({
      fulfillmentRef: 'business_signup_fulfillment:business_signup_1',
      fulfillmentStatus: 'invited',
      nextAction: 'workspace_invite_sent',
    })

    expect(fulfillment).toMatchObject({
      email_delivery_status: 'accepted',
      status: 'invited',
    })
    expect(String(fulfillment?.email_message_id)).toContain(
      'email_msg_team_workspace_invite_',
    )
    expect(String(fulfillment?.workspace_id)).toContain('workspace_')
    expect(String(fulfillment?.invite_id)).toContain('team_workspace_invite_')

    const signup = await readBusinessSignupRequest(db, 'business_signup_1')
    expect(signup).toMatchObject({
      fulfillmentRef: 'business_signup_fulfillment:business_signup_1',
      fulfillmentStatus: 'invited',
    })
  })

  test('parks signup explicitly when invite email config is absent', async () => {
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
        }),
      }),
      db,
    )
    const body = await response.json<{
      request: Readonly<{ fulfillmentStatus: string; nextAction: string }>
    }>()

    expect(response.status).toBe(201)
    expect(body.request).toMatchObject({
      fulfillmentStatus: 'operator_parked',
      nextAction: 'operator_workspace_intake',
    })

    const fulfillment = await db
      .prepare(
        `SELECT status, reason, workspace_id, invite_id, email_delivery_status
           FROM business_signup_fulfillments
          WHERE business_signup_request_id = ?`,
      )
      .bind('business_signup_1')
      .first<Row>()

    expect(fulfillment).toMatchObject({
      email_delivery_status: 'missing_config',
      reason: 'business_signup_invite_email_config_missing',
      status: 'operator_parked',
    })
    expect(String(fulfillment?.workspace_id)).toContain('workspace_')
    expect(String(fulfillment?.invite_id)).toContain('team_workspace_invite_')
  })

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

  test('persists bounded sourceRef and records a signup-stage funnel event', async () => {
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
          sourceRef: 'apollo_agent_readiness_a',
        }),
      }),
      db,
    )

    expect(response.status).toBe(201)

    const signup = await readBusinessSignupRequest(db, 'business_signup_1')
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
      source_kind: 'outbound',
      source_ref: 'apollo_agent_readiness_a',
    })
    expect(signup?.sourceRef).toBe('apollo_agent_readiness_a')
  })

  test('defaults sourceRef to direct and rejects raw source values', async () => {
    const db = makeDb()
    const direct = await run(
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
        }),
      }),
      db,
    )

    expect(direct.status).toBe(201)
    expect(await direct.json()).toMatchObject({
      request: { sourceRef: 'direct' },
    })

    const unsafe = await run(
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
          sourceRef: 'https://tracking.example.com/?utm_source=apollo',
        }),
      }),
      db,
    )

    expect(unsafe.status).toBe(400)
    expect(await unsafe.json()).toEqual({
      error: 'business_signup_validation_error',
      reason: 'sourceRef must be a bounded public-safe token',
    })

    const unsafeSegment = await run(
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
          sourceRef: 'affiliate_email',
        }),
      }),
      db,
    )

    expect(unsafeSegment.status).toBe(400)
    expect(await unsafeSegment.json()).toEqual({
      error: 'business_signup_validation_error',
      reason: 'sourceRef must be a bounded public-safe token',
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
