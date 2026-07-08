import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmReplyRoutes } from './crm-reply-routes'

const contactRow = {
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}

const makeDb = () => {
  const replies = new Map<string, Record<string, unknown>>()
  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_reply_events')) {
          const match = [...replies.values()].find(
            row => row.provider === bound[0] && row.provider_event_id === bound[1],
          )
          return Promise.resolve((match ?? null) as T | null)
        }
        if (query.includes('FROM crm_contacts')) return Promise.resolve(contactRow as T)
        if (query.includes('FROM email_preferences')) return Promise.resolve(null as T | null)
        return Promise.resolve(null as T | null)
      },
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: (query.includes('FROM crm_reply_events')
            ? [...replies.values()]
            : []) as unknown as Array<T>,
          success: true,
        } as D1Result<T>),
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
        return Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement
  return {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const routesFor = (admin: boolean, db: D1Database) => {
  const routes = makeCrmReplyRoutes<Env>({ requireAdminApiToken: () => Promise.resolve(admin) })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmReplyRequest(request, { OPENAGENTS_DB: db }, ctx)
    if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
    return Effect.runPromise(effect)
  }
}

const base = 'https://openagents.com'
const post = (path: string, body: unknown): Request =>
  new Request(`${base}${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM reply routes', () => {
  test('inbound reply -> list happy path', async () => {
    const db = makeDb()
    const run = routesFor(true, db)

    const inboundRes = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Sounds great, tell me more.',
        fromEmail: 'ada@example.com',
        subject: 'Re: your report',
      }),
    )
    expect(inboundRes.status).toBe(201)
    const { result } = (await inboundRes.json()) as { result: { optOut: boolean; contactId: string | null } }
    expect(result.optOut).toBe(false)
    expect(result.contactId).toBe('crm_contact_1')

    const listRes = await run(new Request(`${base}/api/operator/crm/replies`))
    expect(listRes.status).toBe(200)
    const { replies } = (await listRes.json()) as { replies: Array<{ fromEmail: string }> }
    expect(replies.some(r => r.fromEmail === 'ada@example.com')).toBe(true)
  })

  test('opt-out reply is flagged', async () => {
    const run = routesFor(true, makeDb())
    const res = await run(
      post('/api/operator/crm/replies/inbound', {
        bodyText: 'Please unsubscribe me.',
        fromEmail: 'ada@example.com',
      }),
    )
    const { result } = (await res.json()) as { result: { optOut: boolean } }
    expect(result.optOut).toBe(true)
  })

  test('inbound without fromEmail => 400', async () => {
    const run = routesFor(true, makeDb())
    const res = await run(post('/api/operator/crm/replies/inbound', {}))
    expect(res.status).toBe(400)
  })

  test('401 without admin', async () => {
    const run = routesFor(false, makeDb())
    const res = await run(new Request(`${base}/api/operator/crm/replies`))
    expect(res.status).toBe(401)
  })

  test('non-matching path passes through', () => {
    const routes = makeCrmReplyRoutes<Env>({ requireAdminApiToken: () => Promise.resolve(true) })
    const effect = routes.routeCrmReplyRequest(
      new Request(`${base}/api/operator/crm/commands`),
      { OPENAGENTS_DB: makeDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
