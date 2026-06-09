import { describe, expect, test } from 'vitest'

import {
  readEmailPreferenceState,
  readEmailSendEligibility,
  recordEmailUnsubscribe,
  recordProviderEmailSuppression,
  upsertEmailPreferenceCategory,
} from './email-preferences'

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

  first<T>(): Promise<T | null> {
    this.db.lookups.push({ query: this.query, values: this.bound })

    return Promise.resolve((this.db.nextFirst.shift() ?? null) as T | null)
  }

  run(): Promise<void> {
    this.db.runs.push({ query: this.query, values: this.bound })

    return Promise.resolve()
  }
}

class RecordingD1Database {
  readonly lookups: Array<Readonly<{ query: string; values: Array<unknown> }>> =
    []
  readonly nextFirst: Array<unknown | null> = []
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

const runtime = {
  makeId: (prefix: string) => `${prefix}_1`,
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

describe('email preferences and suppression policy', () => {
  test('reads default preferences and upserts one category without collapsing others', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push(null)
    await expect(
      readEmailPreferenceState(db as unknown as D1Database, 'BEN@SILONES.COM'),
    ).resolves.toEqual({
      dripOptIn: true,
      marketingOptIn: true,
      transactionalOptIn: true,
    })

    db.nextFirst.push({
      drip_opt_in: 1,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })
    await upsertEmailPreferenceCategory(
      db as unknown as D1Database,
      {
        category: 'drip',
        email: 'BEN@SILONES.COM',
        optIn: false,
        sourceAuthorityRef: 'customer.unsubscribe',
        userId: 'user_1',
      },
      runtime,
    )

    expect(db.runs[0]?.query).toContain('INSERT INTO email_preferences')
    expect(db.runs[0]?.values).toContain('ben@silones.com')
    expect(db.runs[0]?.values).toContain(0)
    expect(db.runs[0]?.values).toContain(1)
  })

  test('records drip unsubscribe as preference state only', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push({
      drip_opt_in: 1,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })

    await recordEmailUnsubscribe(
      db as unknown as D1Database,
      {
        category: 'drip',
        email: 'ben@silones.com',
        sourceAuthorityRef: 'customer.unsubscribe',
        userId: 'user_1',
      },
      runtime,
    )

    expect(db.runs).toHaveLength(1)
    expect(db.runs[0]?.query).toContain('email_preferences')
    expect(JSON.stringify(db.runs)).not.toContain('email_suppression_entries')
  })

  test('blocks drip and marketing by category but keeps transactional separate', async () => {
    const dripDb = new RecordingD1Database()
    dripDb.nextFirst.push(null, {
      drip_opt_in: 0,
      marketing_opt_in: 1,
      transactional_opt_in: 1,
    })
    const transactionalDb = new RecordingD1Database()
    transactionalDb.nextFirst.push(null, {
      drip_opt_in: 0,
      marketing_opt_in: 0,
      transactional_opt_in: 1,
    })

    await expect(
      readEmailSendEligibility(dripDb as unknown as D1Database, {
        category: 'drip',
        email: 'ben@silones.com',
      }),
    ).resolves.toEqual({ allowed: false, reason: 'drip_opted_out' })
    await expect(
      readEmailSendEligibility(transactionalDb as unknown as D1Database, {
        category: 'transactional',
        email: 'ben@silones.com',
      }),
    ).resolves.toEqual({ allowed: true, reason: 'allowed' })
  })

  test('provider bounces and complaints create all-scope suppressions', async () => {
    const db = new RecordingD1Database()

    await recordProviderEmailSuppression(
      db as unknown as D1Database,
      {
        email: 'ben@silones.com',
        providerEventId: 'resend_evt_1',
        reason: 'bounce',
        sourceAuthorityRef: 'resend.webhook:bounce',
      },
      runtime,
    )

    expect(db.runs[0]?.query).toContain('INSERT INTO email_suppression_entries')
    expect(db.runs[0]?.values).toEqual([
      'email_suppression_1',
      'ben@silones.com',
      'bounce',
      'all',
      'resend.webhook:bounce',
      'resend_evt_1',
      null,
      '2026-06-05T12:00:00.000Z',
      '2026-06-05T12:00:00.000Z',
    ])
  })

  test('all-scope suppression blocks transactional mail', async () => {
    const db = new RecordingD1Database()
    db.nextFirst.push({ id: 'email_suppression_1', scope: 'all' })

    await expect(
      readEmailSendEligibility(db as unknown as D1Database, {
        category: 'transactional',
        email: 'ben@silones.com',
      }),
    ).resolves.toEqual({ allowed: false, reason: 'all_suppressed' })
  })
})
