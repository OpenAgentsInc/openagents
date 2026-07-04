import { Effect, Redacted } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  type BusinessSignupRuntime,
  readBusinessSignupRequest,
} from './business-signup-routes'
import { EmailAddress, ResendEmailSender, WorkerSecret } from './config'
import {
  type VerticalFunnelEmailInput,
  handleVerticalFunnelRequest,
} from './vertical-funnel-routes'

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

const SCHEMA = [
  '0191_business_signup_requests.sql',
  '0216_business_signup_referral_attribution.sql',
  '0270_business_funnel_events.sql',
  '0271_business_signup_fulfillment.sql',
  '0278_business_commitment_ledger.sql',
  '0294_business_pipeline_queue.sql',
  '0297_business_source_attribution.sql',
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
  nowIso: () => '2026-07-02T12:00:00.000Z',
  expiresAtFromNow: () => '2026-08-01T12:00:00.000Z',
}

beforeEach(() => {
  counter = 0
})

describe('vertical funnel routes', () => {
  test('renders legal page/apply/confirmed/follow-up templates without demo placeholders', async () => {
    const db = makeDb()
    const page = await Effect.runPromise(
      handleVerticalFunnelRequest(
        new Request('https://openagents.com/business/legal'),
        { db, runtime },
      )!,
    )
    const pageHtml = await page.text()
    expect(pageHtml).toContain('Review-gated agent workspaces for legal teams.')
    expect(pageHtml).toContain('/business/legal/apply')
    expect(pageHtml).not.toContain('Demo only')
    expect(pageHtml).not.toContain('placeholder')

    const apply = await Effect.runPromise(
      handleVerticalFunnelRequest(
        new Request('https://openagents.com/business/legal/apply'),
        { db, runtime },
      )!,
    )
    expect(await apply.text()).toContain(
      'action="/api/business/vertical-funnels/legal/apply"',
    )

    const confirmed = await Effect.runPromise(
      handleVerticalFunnelRequest(
        new Request(
          'https://openagents.com/business/legal/confirmed?ref=business_signup_1',
        ),
        { db, runtime },
      )!,
    )
    expect(await confirmed.text()).toContain(
      'https://cal.com/openagents/legal-workspace-intake',
    )

    const followUp = await Effect.runPromise(
      handleVerticalFunnelRequest(
        new Request('https://openagents.com/business/legal/follow-up'),
        { db, runtime },
      )!,
    )
    expect(await followUp.text()).toContain(
      'OpenAgents legal qualification worksheet',
    )
  })

  test('POST stores the application and sends the Resend-backed worksheet email', async () => {
    const db = makeDb()
    const calls: Array<VerticalFunnelEmailInput> = []
    const body = new URLSearchParams({
      businessName: 'Example Legal Team',
      contactEmail: 'lead@example.com',
      phone: '+1 555 0100',
      website: 'https://example.com',
      practiceArea: 'Business contracts',
      primaryGoal: 'Prepare a review-gated contract intake workspace.',
      systems: 'Document library and CRM.',
    })

    const response = await Effect.runPromise(
      handleVerticalFunnelRequest(
        new Request(
          'https://openagents.com/api/business/vertical-funnels/legal/apply',
          {
            body,
            headers: {
              accept: 'application/json',
              'content-type': 'application/x-www-form-urlencoded',
            },
            method: 'POST',
          },
        ),
        {
          db,
          resend: {
            apiKey: Redacted.make(WorkerSecret.make('re_test')),
            fromEmail: ResendEmailSender.make('OpenAgents <ops@example.com>'),
            replyToEmail: EmailAddress.make('ops@example.com'),
          },
          runtime,
          sender: async input => {
            calls.push(input)
            return { ok: true, providerMessageId: 'resend_123' }
          },
        },
      )!,
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toMatchObject({
      application: {
        id: 'business_signup_1',
        sourceRoute: '/business/legal/apply',
        bookingUrl: 'https://cal.com/openagents/legal-workspace-intake',
        followUpEmail: {
          kind: 'sent',
          providerMessageId: 'resend_123',
        },
      },
    })

    const stored = await readBusinessSignupRequest(db, 'business_signup_1')
    expect(stored).toMatchObject({
      businessName: 'Example Legal Team',
      contactEmail: 'lead@example.com',
      sourceRoute: '/business/legal/apply',
      website: 'https://example.com/',
    })
    expect(stored?.helpWith).toContain('vertical=legal')
    expect(stored?.helpWith).toContain(
      'primary_goal=Prepare a review-gated contract intake workspace.',
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]).toMatchObject({
      from: 'OpenAgents <ops@example.com>',
      to: 'lead@example.com',
      subject: 'OpenAgents legal workspace qualification worksheet',
      idempotencyKey: 'vertical_funnel:legal:business_signup_1:follow_up_v1',
      replyTo: 'ops@example.com',
    })
    expect(calls[0]?.attachments[0]).toMatchObject({
      filename: 'openagents-legal-qualification-worksheet.txt',
    })
    expect(atob(calls[0]?.attachments[0]?.content ?? '')).toContain(
      'OpenAgents legal qualification worksheet',
    )
  })
})
