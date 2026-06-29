import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ResendEmailConfig,
  ResendEmailSender,
  WorkerSecret,
} from './config'
import {
  type EmailCampaignDispatcherResult,
  dispatchDueEmailCampaignSends,
} from './email-campaign-dispatcher'
import type { EmailSequenceSendPlan } from './email-sequence-send-service'

class RecordingD1Statement {
  readonly bound: Array<unknown> = []

  constructor(
    private readonly db: RecordingD1Database,
    readonly query: string,
  ) {}

  bind(...values: Array<unknown>): RecordingD1Statement {
    this.bound.push(...values)

    return this
  }

  all<T>(): Promise<D1Result<T>> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    return Promise.resolve({
      meta: {},
      results: (this.db.nextAll.shift() ?? []) as Array<T>,
      success: true,
    } as D1Result<T>)
  }

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    if (this.query.includes('FROM email_messages')) {
      return Promise.resolve({
        created_at: '2026-06-05T12:00:00.000Z',
        error_message: null,
        error_name: null,
        id: 'email_msg_1',
        idempotency_key: this.bound[0],
        kind: 'crm_transactional',
        provider: null,
        provider_message_id: null,
        status: 'rendered',
        updated_at: '2026-06-05T12:00:00.000Z',
      } as T)
    }

    return Promise.resolve((this.db.nextFirst.shift() ?? null) as T | null)
  }

  run(): Promise<D1Result> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve({
      meta: { changes: this.db.nextChanges.shift() ?? 1 },
      success: true,
    } as D1Result)
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly nextAll: Array<Array<unknown>> = []
  readonly nextChanges: Array<number> = []
  readonly nextFirst: Array<unknown | null> = []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  nowIso: () => '2026-06-05T12:00:00.000Z',
  randomId: (prefix: string) => `${prefix}_1`,
}

const resendConfig = (): ResendEmailConfig => ({
  apiKey: Redacted.make(WorkerSecret.make('resend_test_key')),
  fromEmail: ResendEmailSender.make('OpenAgents <chris+sites@openagents.com>'),
})

const dueRow = (
  overrides: Partial<{
    attempt_count: number
    lifecycle_kind: string | null
    metadata_json: string
    template_slug: string
    user_id: string | null
  }> = {},
) => ({
  attempt_count: overrides.attempt_count ?? 0,
  campaign_id: 'email_campaign_1',
  delay_seconds: 0,
  due_at: '2026-06-05T12:00:00.000Z',
  email: 'ben@silones.com',
  enrollment_id: 'email_campaign_enrollment_1',
  id: 'email_campaign_send_1',
  idempotency_key: 'email_campaign_send:email_campaign_enrollment_1:day_0',
  lifecycle_kind:
    'lifecycle_kind' in overrides ? overrides.lifecycle_kind : 'signup_day_0',
  metadata_json:
    overrides.metadata_json ?? JSON.stringify({ displayName: 'Ben' }),
  source_authority_ref: 'system.email_onboarding_drip.v1:send:day_0',
  step_id: 'email_campaign_step_day_0',
  step_key: 'day_0',
  template_slug: overrides.template_slug ?? 'drip.signup_day_0.v1',
  user_id: overrides.user_id ?? 'user_1',
})

const acceptedFetch: typeof fetch = (() =>
  Promise.resolve(
    new Response(JSON.stringify({ id: 'resend_message_1' }), { status: 200 }),
  )) as typeof fetch

const rejectedFetch: typeof fetch = (() =>
  Promise.resolve(
    new Response(
      JSON.stringify({ message: 'Temporary failure', name: 'rate_limit' }),
      {
        status: 429,
      },
    ),
  )) as typeof fetch

const dispatch = async (
  db: RecordingD1Database,
  fetcher: typeof fetch = acceptedFetch,
  sequenceSend?: Parameters<typeof dispatchDueEmailCampaignSends>[1]['sequenceSend'],
): Promise<EmailCampaignDispatcherResult> =>
  Effect.runPromise(
    dispatchDueEmailCampaignSends(db as unknown as D1Database, {
      appOrigin: 'https://openagents.com',
      fetcher,
      resend: resendConfig(),
      runtime,
      sequenceSend,
    }),
  )

describe('email campaign dispatcher', () => {
  test('claims a due send and sends through the email ledger', async () => {
    const db = new RecordingD1Database()
    db.nextAll.push([dueRow()])
    db.nextFirst.push(null, null, null)

    const result = await dispatch(db)

    expect(result).toEqual({
      claimed: 1,
      failed: 0,
      retried: 0,
      sent: 1,
      skipped: 0,
      suppressed: 0,
    })
    expect(
      db.runs.some(run => run.query.includes("SET status = 'claimed'")),
    ).toBe(true)
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_messages')),
    ).toBe(true)
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_deliveries')),
    ).toBe(true)
    expect(db.runs.some(run => run.query.includes("SET status = 'sent'"))).toBe(
      true,
    )
    expect(JSON.stringify(db.runs)).not.toContain('provider_account')
    expect(JSON.stringify(db.runs)).not.toContain('auth_grant')
  })

  test('does not process duplicate claims', async () => {
    const db = new RecordingD1Database()
    db.nextAll.push([dueRow()])
    db.nextChanges.push(0)

    const result = await dispatch(db)

    expect(result).toEqual({
      claimed: 0,
      failed: 0,
      retried: 0,
      sent: 0,
      skipped: 0,
      suppressed: 0,
    })
    expect(
      db.runs.some(run => run.query.includes('INSERT INTO email_messages')),
    ).toBe(false)
  })

  test('suppresses recipients with drip suppression or opt-out preference', async () => {
    const suppressedDb = new RecordingD1Database()
    suppressedDb.nextAll.push([dueRow()])
    suppressedDb.nextFirst.push({ id: 'email_suppression_1' })
    const optedOutDb = new RecordingD1Database()
    optedOutDb.nextAll.push([dueRow()])
    optedOutDb.nextFirst.push(null, {
      drip_opt_in: 0,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })

    await expect(dispatch(suppressedDb)).resolves.toMatchObject({
      claimed: 1,
      suppressed: 1,
    })
    await expect(dispatch(optedOutDb)).resolves.toMatchObject({
      claimed: 1,
      suppressed: 1,
    })
    expect(
      suppressedDb.runs.some(run => run.values.includes('drip_suppressed')),
    ).toBe(true)
    expect(
      optedOutDb.runs.some(run =>
        run.values.includes('drip_preference_disabled'),
      ),
    ).toBe(true)
  })

  test('skips users who already have active or delivered orders', async () => {
    const activeDb = new RecordingD1Database()
    activeDb.nextAll.push([dueRow()])
    activeDb.nextFirst.push(null, null, { status: 'agent_running' })
    const deliveredDb = new RecordingD1Database()
    deliveredDb.nextAll.push([dueRow()])
    deliveredDb.nextFirst.push(null, null, { status: 'delivered' })

    await expect(dispatch(activeDb)).resolves.toMatchObject({
      claimed: 1,
      skipped: 1,
    })
    await expect(dispatch(deliveredDb)).resolves.toMatchObject({
      claimed: 1,
      skipped: 1,
    })
    expect(activeDb.runs.some(run => run.values.includes('active_order'))).toBe(
      true,
    )
    expect(
      deliveredDb.runs.some(run => run.values.includes('delivered_order')),
    ).toBe(true)
  })

  test('retries bounded failures and permanently fails after max attempts', async () => {
    const retryDb = new RecordingD1Database()
    retryDb.nextAll.push([dueRow({ attempt_count: 0 })])
    retryDb.nextFirst.push(null, null, null)
    const finalDb = new RecordingD1Database()
    finalDb.nextAll.push([dueRow({ attempt_count: 2 })])
    finalDb.nextFirst.push(null, null, null)

    await expect(dispatch(retryDb, rejectedFetch)).resolves.toMatchObject({
      claimed: 1,
      retried: 1,
    })
    await expect(dispatch(finalDb, rejectedFetch)).resolves.toMatchObject({
      claimed: 1,
      failed: 1,
    })
    expect(
      retryDb.runs.some(run => run.query.includes("SET status = 'scheduled'")),
    ).toBe(true)
    expect(
      finalDb.runs.some(run => run.query.includes("SET status = 'failed'")),
    ).toBe(true)
  })

  test('keeps authored sequence sends dry-run/skipped when the sequence service is disabled', async () => {
    const db = new RecordingD1Database()
    db.nextAll.push([
      dueRow({
        lifecycle_kind: null,
        template_slug: 'sequence.welcome.day_0.v1',
      }),
    ])
    db.nextFirst.push(null, null, null)
    let calls = 0

    const result = await dispatch(db, acceptedFetch, {
      isEnabled: () => false,
      send: () => {
        calls += 1

        return Effect.succeed({
          emailMessageId: 'email_msg_1',
          ok: true,
          providerMessageId: 'cf_1',
        })
      },
    })

    expect(calls).toBe(0)
    expect(result).toMatchObject({ claimed: 1, skipped: 1 })
    expect(
      db.runs.some(run => run.values.includes('email_sequence_send_disabled')),
    ).toBe(true)
  })

  test('dispatches authored sequence sends through the armed sequence sender', async () => {
    const db = new RecordingD1Database()
    db.nextAll.push([
      dueRow({
        lifecycle_kind: null,
        template_slug: 'sequence.welcome.day_0.v1',
      }),
    ])
    db.nextFirst.push(null, null, null)
    const seen: Array<EmailSequenceSendPlan> = []

    const result = await dispatch(db, acceptedFetch, {
      isEnabled: () => true,
      send: plan => {
        seen.push(plan)

        return Effect.succeed({
          emailMessageId: 'email_msg_sequence_1',
          ok: true,
          providerMessageId: 'cf_sequence_1',
        })
      },
    })

    expect(result).toMatchObject({ claimed: 1, sent: 1 })
    expect(seen).toHaveLength(1)
    expect(seen[0]).toMatchObject({
      displayName: 'Ben',
      sourceAuthorityRef: 'system.email_onboarding_drip.v1:send:day_0',
      templateSlug: 'sequence.welcome.day_0.v1',
      to: 'ben@silones.com',
    })
    expect(db.runs.some(run => run.query.includes("SET status = 'sent'"))).toBe(
      true,
    )
  })
})
