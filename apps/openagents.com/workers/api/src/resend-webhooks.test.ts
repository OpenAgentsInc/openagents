import { Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import { WorkerSecret } from './config'
import { handleResendWebhook } from './resend-webhooks'

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

  run(): Promise<D1Result> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve({
      meta: { changes: this.db.nextChanges.shift() ?? 1 },
      success: true,
    } as D1Result)
  }
}

class RecordingD1Database {
  readonly nextChanges: Array<number> = []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  makeId: (prefix: string) => `${prefix}_1`,
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

const encoder = new TextEncoder()
const rawSecret = 'resend-webhook-secret'
const webhookSecret = Redacted.make(
  WorkerSecret.make(`whsec_${btoa(rawSecret)}`),
)

const sign = async (
  body: string,
  input: Readonly<{ id?: string; timestamp?: string }> = {},
): Promise<Headers> => {
  const id = input.id ?? 'evt_resend_1'
  const timestamp = input.timestamp ?? '1790000000'
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(rawSecret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const digest = await crypto.subtle.sign(
    'HMAC',
    key,
    encoder.encode(`${id}.${timestamp}.${body}`),
  )
  const signature = btoa(String.fromCharCode(...new Uint8Array(digest)))

  return new Headers({
    'svix-id': id,
    'svix-signature': `v1,${signature}`,
    'svix-timestamp': timestamp,
  })
}

const bodyFor = (
  type: string,
  overrides: Record<string, unknown> = {},
): string =>
  JSON.stringify({
    created_at: '2026-06-05T12:00:00.000Z',
    data: {
      email_id: 'resend_msg_1',
      to: ['ben@silones.com'],
      ...overrides,
    },
    type,
  })

describe('Resend webhook ingestion', () => {
  test('verifies signatures and records delivered events without raw payloads', async () => {
    const db = new RecordingD1Database()
    const body = bodyFor('email.delivered', {
      text: 'Raw provider payload should not be stored.',
    })

    const result = await handleResendWebhook(
      db as unknown as D1Database,
      {
        body,
        headers: await sign(body),
        secret: webhookSecret,
      },
      runtime,
    )

    expect(result).toEqual({
      duplicate: false,
      eventType: 'email.delivered',
      providerEventId: 'evt_resend_1',
      status: 'accepted',
    })
    expect(db.runs[0]?.query).toContain('INSERT INTO email_provider_events')
    expect(db.runs[0]?.values).toContain('email.delivered')
    expect(db.runs[1]?.query).toContain('UPDATE email_deliveries')
    expect(db.runs[1]?.values).toContain('accepted')
    expect(JSON.stringify(db.runs)).not.toContain(
      'Raw provider payload should not be stored.',
    )
  })

  test('rejects invalid signatures before writing', async () => {
    const db = new RecordingD1Database()
    const body = bodyFor('email.delivered')
    const headers = await sign(body)
    headers.set('svix-signature', 'v1,invalid')

    await expect(
      handleResendWebhook(
        db as unknown as D1Database,
        {
          body,
          headers,
          secret: webhookSecret,
        },
        runtime,
      ),
    ).resolves.toEqual({
      duplicate: false,
      eventType: 'invalid_signature',
      providerEventId: 'evt_resend_1',
      status: 'unauthorized',
    })
    expect(db.runs).toHaveLength(0)
  })

  test('records bounces and complaints as all-scope suppressions', async () => {
    const bouncedDb = new RecordingD1Database()
    const complainedDb = new RecordingD1Database()
    const bouncedBody = bodyFor('email.bounced', {
      reason: 'Mailbox unavailable',
    })
    const complainedBody = bodyFor('email.complained')

    await handleResendWebhook(
      bouncedDb as unknown as D1Database,
      {
        body: bouncedBody,
        headers: await sign(bouncedBody, { id: 'evt_bounce' }),
        secret: webhookSecret,
      },
      runtime,
    )
    await handleResendWebhook(
      complainedDb as unknown as D1Database,
      {
        body: complainedBody,
        headers: await sign(complainedBody, { id: 'evt_complaint' }),
        secret: webhookSecret,
      },
      runtime,
    )

    expect(
      bouncedDb.runs.some(run =>
        run.query.includes('INSERT INTO email_suppression_entries'),
      ),
    ).toBe(true)
    expect(JSON.stringify(bouncedDb.runs)).toContain('"bounce"')
    expect(JSON.stringify(complainedDb.runs)).toContain('"complaint"')
  })

  test('records failed events with bounded delivery errors', async () => {
    const db = new RecordingD1Database()
    const body = bodyFor('email.failed', {
      error: {
        message: 'Temporary provider failure. '.repeat(80),
        name: 'provider_failure',
      },
    })

    await handleResendWebhook(
      db as unknown as D1Database,
      {
        body,
        headers: await sign(body),
        secret: webhookSecret,
      },
      runtime,
    )

    const deliveryUpdate = db.runs.find(run =>
      run.query.includes('UPDATE email_deliveries'),
    )
    expect(deliveryUpdate?.values[0]).toBe('failed')
    expect(String(deliveryUpdate?.values[2]).length).toBeLessThanOrEqual(500)
  })

  test('handles duplicate provider events idempotently', async () => {
    const db = new RecordingD1Database()
    db.nextChanges.push(0)
    const body = bodyFor('email.delivered')

    await expect(
      handleResendWebhook(
        db as unknown as D1Database,
        {
          body,
          headers: await sign(body),
          secret: webhookSecret,
        },
        runtime,
      ),
    ).resolves.toMatchObject({
      duplicate: true,
      status: 'accepted',
    })
    expect(
      db.runs.filter(run => run.query.includes('UPDATE email_deliveries')),
    ).toHaveLength(0)
  })

  test('accepts unconfigured signature verification for safe smoke payloads', async () => {
    const db = new RecordingD1Database()
    const body = bodyFor('email.delivered')

    await expect(
      handleResendWebhook(
        db as unknown as D1Database,
        {
          body,
          headers: new Headers({ 'svix-id': 'evt_unsigned_smoke' }),
        },
        runtime,
      ),
    ).resolves.toMatchObject({
      providerEventId: 'evt_unsigned_smoke',
      status: 'accepted',
    })
  })
})
