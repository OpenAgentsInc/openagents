import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmRoutes } from './crm-routes'

// Fake D1 returning configurable canned results for whatever the route reads.
const cannedDb = (canned: {
  all?: Array<Record<string, unknown>>
  first?: Record<string, unknown> | null
}): D1Database => {
  const statement = (): D1PreparedStatement =>
    ({
      bind: () => statement(),
      first: <T,>() => Promise.resolve((canned.first ?? null) as T | null),
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: (canned.all ?? []) as unknown as Array<T>,
          success: true,
        } as D1Result<T>),
      run: () =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: [],
          success: true,
        } as unknown as D1Result),
      raw: () => Promise.reject(new Error('raw should not be used')),
    }) as unknown as D1PreparedStatement
  return {
    batch: () => Promise.reject(new Error('batch should not be used')),
    dump: () => Promise.reject(new Error('dump should not be used')),
    exec: () => Promise.reject(new Error('exec should not be used')),
    prepare: () => statement(),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

const ctx = {} as ExecutionContext

const routesWith = (admin: boolean, db: D1Database) => {
  const routes = makeCrmRoutes<TestEnv>({
    requireAdminApiToken: () => Promise.resolve(admin),
  })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmRequest(request, { OPENAGENTS_DB: db }, ctx)
    if (effect === undefined) {
      throw new Error(`route did not match: ${request.url}`)
    }
    return Effect.runPromise(effect)
  }
}

const get = (path: string): Request =>
  new Request(`https://openagents.com${path}`, { method: 'GET' })

const contactRow = {
  contact_type: 'investor',
  created_at: '2026-06-22T00:00:00.000Z',
  full_name: 'Ada Lovelace',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}

describe('CRM read routes — listing', () => {
  test('GET /contacts returns the projected list', async () => {
    const run = routesWith(true, cannedDb({ all: [contactRow] }))
    const response = await run(get('/api/operator/crm/contacts'))
    expect(response.status).toBe(200)
    const json = (await response.json()) as { contacts: Array<{ primaryEmail: string }> }
    expect(json.contacts).toHaveLength(1)
    expect(json.contacts[0]?.primaryEmail).toBe('ada@example.com')
  })

  test('GET /accounts is admin-gated (401 without token)', async () => {
    const run = routesWith(false, cannedDb({ all: [] }))
    const response = await run(get('/api/operator/crm/accounts'))
    expect(response.status).toBe(401)
  })
})

describe('CRM read routes — single resources', () => {
  test('GET /contacts/:id returns the contact when present', async () => {
    const run = routesWith(true, cannedDb({ first: contactRow }))
    const response = await run(get('/api/operator/crm/contacts/crm_contact_1'))
    expect(response.status).toBe(200)
    const json = (await response.json()) as { contact: { id: string } }
    expect(json.contact.id).toBe('crm_contact_1')
  })

  test('GET /contacts/:id is a 404 when absent', async () => {
    const run = routesWith(true, cannedDb({ first: null }))
    const response = await run(get('/api/operator/crm/contacts/missing'))
    expect(response.status).toBe(404)
    const json = (await response.json()) as { error: string; resource: string }
    expect(json.error).toBe('not_found')
    expect(json.resource).toBe('contact')
  })

  test('GET /contacts/:id/engagement is a 404 when no snapshot exists', async () => {
    const run = routesWith(true, cannedDb({ first: null }))
    const response = await run(get('/api/operator/crm/contacts/crm_contact_1/engagement'))
    expect(response.status).toBe(404)
  })

  test('GET /contacts/:id/activities returns the activity list', async () => {
    const run = routesWith(
      true,
      cannedDb({
        all: [
          {
            activity_type: 'email_sent',
            contact_id: 'crm_contact_1',
            created_at: '2026-06-22T00:00:00.000Z',
            id: 'crm_activity_1',
            occurred_at: '2026-06-22T00:00:00.000Z',
            source_system: 'crm',
            tenant_ref: 'tenant.openagents',
          },
        ],
      }),
    )
    const response = await run(get('/api/operator/crm/contacts/crm_contact_1/activities'))
    expect(response.status).toBe(200)
    const json = (await response.json()) as { activities: Array<{ activityType: string }> }
    expect(json.activities[0]?.activityType).toBe('email_sent')
  })
})

describe('CRM read routes — matching', () => {
  test('non-CRM paths pass through (undefined)', () => {
    const routes = makeCrmRoutes<TestEnv>({
      requireAdminApiToken: () => Promise.resolve(true),
    })
    const effect = routes.routeCrmRequest(
      get('/api/operator/partners/agreements'),
      { OPENAGENTS_DB: cannedDb({}) },
      ctx,
    )
    expect(effect).toBeUndefined()
  })

  test('non-GET method on a CRM path is a 405', async () => {
    const run = routesWith(true, cannedDb({}))
    const response = await run(
      new Request('https://openagents.com/api/operator/crm/contacts', { method: 'POST' }),
    )
    expect(response.status).toBe(405)
  })
})
