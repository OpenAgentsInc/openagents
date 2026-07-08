import { describe, expect, test } from 'vitest'

import { detectCrmReplyOptOut, listCrmReplyEvents, recordCrmReplyEvent } from './crm-reply'

const contactRow = {
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}

const makeDb = (opts: { contact?: Record<string, unknown> | null } = {}) => {
  const replies = new Map<string, Record<string, unknown>>()
  const activities: Array<ReadonlyArray<unknown>> = []
  const preferenceUpserts: Array<ReadonlyArray<unknown>> = []
  const suppressions: Array<ReadonlyArray<unknown>> = []

  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_reply_events')) {
          const provider = String(bound[0] ?? '')
          const providerEventId = String(bound[1] ?? '')
          const match = [...replies.values()].find(
            row => row.provider === provider && row.provider_event_id === providerEventId,
          )
          return Promise.resolve((match ?? null) as T | null)
        }
        if (query.includes('FROM crm_contacts')) {
          return Promise.resolve(
            (opts.contact === undefined ? contactRow : opts.contact) as T | null,
          )
        }
        if (query.includes('FROM email_preferences')) return Promise.resolve(null as T | null)
        return Promise.resolve(null as T | null)
      },
      all: <T,>() => {
        if (query.includes('FROM crm_reply_events')) {
          return Promise.resolve({
            meta: {} as D1Meta,
            results: [...replies.values()] as unknown as Array<T>,
            success: true,
          } as D1Result<T>)
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [] as unknown as Array<T>, success: true } as D1Result<T>)
      },
      run: () => {
        if (query.includes('INSERT INTO crm_reply_events')) {
          const [
            id, tenantRef, contactId, fromEmail, subject, bodyText,
            inReplyToRef, provider, providerEventId, optOut, routedTo, createdAt,
          ] = bound
          replies.set(String(id), {
            body_text: bodyText,
            contact_id: contactId,
            created_at: createdAt,
            from_email: fromEmail,
            id,
            in_reply_to_ref: inReplyToRef,
            opt_out: optOut,
            provider,
            provider_event_id: providerEventId,
            routed_to: routedTo,
            subject,
            tenant_ref: tenantRef,
          })
        }
        if (query.includes('INSERT OR IGNORE INTO crm_activities')) {
          activities.push(bound)
        }
        if (query.includes('email_preferences')) {
          preferenceUpserts.push(bound)
        }
        if (query.includes('INSERT INTO email_suppression_entries')) {
          suppressions.push(bound)
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement

  const db = {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
  return { activities, db, preferenceUpserts, replies, suppressions }
}

let counter = 0
const runtime = {
  makeId: (p: string) => `${p}_${(counter += 1)}`,
  nowIso: () => '2026-06-22T00:00:00.000Z',
}
const tenant = 'tenant.openagents'

describe('detectCrmReplyOptOut', () => {
  test('flags common opt-out phrasing', () => {
    expect(detectCrmReplyOptOut({ bodyText: 'please unsubscribe me' })).toBe(true)
    expect(detectCrmReplyOptOut({ subject: 'STOP' })).toBe(false) // whole-word boundary, not a substring match on its own without context
    expect(detectCrmReplyOptOut({ bodyText: 'stop emailing me' })).toBe(true)
    expect(detectCrmReplyOptOut({ bodyText: 'take me off this list' })).toBe(true)
  })
  test('does not flag an ordinary reply', () => {
    expect(detectCrmReplyOptOut({ bodyText: 'Thanks, this looks interesting — tell me more.' })).toBe(false)
  })
})

describe('recordCrmReplyEvent', () => {
  test('matches an existing contact and logs a crm_activities row', async () => {
    const { activities, db } = makeDb()
    const result = await recordCrmReplyEvent(
      db,
      { bodyText: 'Sounds good, let’s talk.', fromEmail: 'ada@example.com', subject: 'Re: your report', tenantRef: tenant },
      runtime,
    )
    expect(result.contactId).toBe('crm_contact_1')
    expect(result.optOut).toBe(false)
    expect(result.routedTo).toBe('operator_notification')
    expect(activities.length).toBe(1)
  })

  test('an opt-out reply auto-suppresses through the existing suppression machinery', async () => {
    const { db, preferenceUpserts, suppressions } = makeDb()
    const result = await recordCrmReplyEvent(
      db,
      { bodyText: 'Please unsubscribe me from this list.', fromEmail: 'ada@example.com', tenantRef: tenant },
      runtime,
    )
    expect(result.optOut).toBe(true)
    expect(preferenceUpserts.length).toBe(1)
    expect(suppressions.length).toBe(1)
    expect(suppressions[0]?.[2]).toBe('unsubscribe')
    expect(suppressions[0]?.[3]).toBe('all')
  })

  test('no matching contact still records the reply event with a null contactId', async () => {
    const { db } = makeDb({ contact: null })
    const result = await recordCrmReplyEvent(
      db,
      { bodyText: 'hi', fromEmail: 'unknown@example.com', tenantRef: tenant },
      runtime,
    )
    expect(result.contactId).toBeNull()
  })

  test('a replayed provider event id is idempotent', async () => {
    const { db } = makeDb()
    const first = await recordCrmReplyEvent(
      db,
      {
        bodyText: 'hi',
        fromEmail: 'ada@example.com',
        provider: 'sarah_repo',
        providerEventId: 'evt_1',
        tenantRef: tenant,
      },
      runtime,
    )
    const second = await recordCrmReplyEvent(
      db,
      {
        bodyText: 'hi',
        fromEmail: 'ada@example.com',
        provider: 'sarah_repo',
        providerEventId: 'evt_1',
        tenantRef: tenant,
      },
      runtime,
    )
    expect(first.duplicate).toBe(false)
    expect(second.duplicate).toBe(true)
    expect(second.replyEventId).toBe(first.replyEventId)
  })
})

describe('listCrmReplyEvents', () => {
  test('lists recorded replies for a tenant', async () => {
    const { db } = makeDb()
    await recordCrmReplyEvent(db, { bodyText: 'hi', fromEmail: 'ada@example.com', tenantRef: tenant }, runtime)
    const events = await listCrmReplyEvents(db, tenant)
    expect(events.length).toBe(1)
    expect(events[0]?.fromEmail).toBe('ada@example.com')
  })
})
