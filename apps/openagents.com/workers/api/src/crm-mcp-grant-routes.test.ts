import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmMcpGrantRoutes } from './crm-mcp-grant-routes'

type GrantRow = {
  grant_ref: string
  token_hash: string
  tenant_ref: string
  authority_classes_json: string
  label: string | null
  status: string
  created_at: string
  expires_at: string | null
}

const makeDb = () => {
  const rows: Array<GrantRow> = []
  const statement = (q: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...v: ReadonlyArray<unknown>) => statement(q, v),
      first: <T,>() => Promise.resolve(null as T | null),
      all: <T,>() => {
        const tenant = String(bound[0] ?? '')
        return Promise.resolve({
          meta: {} as D1Meta,
          results: rows.filter(r => r.tenant_ref === tenant) as unknown as Array<T>,
          success: true,
        } as D1Result<T>)
      },
      run: () => {
        if (q.includes('INSERT INTO crm_mcp_grants')) {
          const [, grantRef, tokenHash, tenantRef, authoritiesJson, label, createdAt, expiresAt] = bound
          rows.push({
            authority_classes_json: String(authoritiesJson),
            created_at: String(createdAt),
            expires_at: expiresAt === null ? null : String(expiresAt),
            grant_ref: String(grantRef),
            label: label === null ? null : String(label),
            status: 'active',
            tenant_ref: String(tenantRef),
            token_hash: String(tokenHash),
          })
          return Promise.resolve({ meta: { changes: 1 } as D1Meta, results: [], success: true } as unknown as D1Result)
        }
        return Promise.resolve({ meta: { changes: 0 } as D1Meta, results: [], success: true } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement
  return {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (q: string) => statement(q),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const run = (admin: boolean, db: D1Database, request: Request): Promise<Response> => {
  const routes = makeCrmMcpGrantRoutes<Env>({
    requireAdminApiToken: () => Promise.resolve(admin),
  })
  const effect = routes.routeCrmMcpGrantRequest(request, { OPENAGENTS_DB: db }, ctx)
  if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
  return Effect.runPromise(effect)
}

const url = 'https://openagents.com/api/operator/crm/mcp-grants'
const post = (body: unknown): Request =>
  new Request(url, { body: JSON.stringify(body), headers: { 'content-type': 'application/json' }, method: 'POST' })

describe('CRM MCP grant routes', () => {
  test('POST mints a grant and returns the token once (201)', async () => {
    const res = await run(true, makeDb(), post({ authorities: ['operator_read'], label: 'bot', tenant: 'tenant.acme' }))
    expect(res.status).toBe(201)
    const json = (await res.json()) as { grant: { tenantRef: string }; token: string }
    expect(json.token.startsWith('oa_mcp_')).toBe(true)
    expect(json.grant.tenantRef).toBe('tenant.acme')
  })

  test('POST with no valid authorities is a 400', async () => {
    const res = await run(true, makeDb(), post({ authorities: ['bogus'] }))
    expect(res.status).toBe(400)
  })

  test('GET lists grants for the tenant', async () => {
    const db = makeDb()
    const run1 = (request: Request) => run(true, db, request)
    await run1(post({ authorities: ['operator_read'], tenant: 'tenant.acme' }))
    const res = await run1(new Request(`${url}?tenant=tenant.acme`, { method: 'GET' }))
    expect(res.status).toBe(200)
    const json = (await res.json()) as { grants: Array<{ authorities: string[] }> }
    expect(json.grants).toHaveLength(1)
  })

  test('401 without an admin token', async () => {
    const res = await run(false, makeDb(), post({ authorities: ['operator_read'] }))
    expect(res.status).toBe(401)
  })

  test('non-grant path passes through', () => {
    const routes = makeCrmMcpGrantRoutes<Env>({ requireAdminApiToken: () => Promise.resolve(true) })
    const effect = routes.routeCrmMcpGrantRequest(
      new Request('https://openagents.com/api/operator/crm/contacts'),
      { OPENAGENTS_DB: makeDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
