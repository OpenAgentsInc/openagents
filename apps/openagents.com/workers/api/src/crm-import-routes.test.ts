import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmImportRoutes } from './crm-import-routes'

// Fake D1 sufficient for one import: contact "find by email" misses (new),
// the post-insert reread by id returns a synthetic row, writes are no-ops.
const importDb = (): D1Database => {
  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('WHERE id = ?')) {
          return Promise.resolve({
            created_at: '2026-06-22T00:00:00.000Z',
            id: String(bound[0] ?? 'c_1'),
            primary_email: 'ada@example.com',
            tenant_ref: 'tenant.openagents',
            updated_at: '2026-06-22T00:00:00.000Z',
          } as unknown as T)
        }
        return Promise.resolve(null as T | null)
      },
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: [] as unknown as Array<T>,
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
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext

const run = (admin: boolean, request: Request): Promise<Response> => {
  const routes = makeCrmImportRoutes<TestEnv>({
    requireAdminApiToken: () => Promise.resolve(admin),
  })
  const effect = routes.routeCrmImportRequest(request, { OPENAGENTS_DB: importDb() }, ctx)
  if (effect === undefined) {
    throw new Error(`route did not match: ${request.url}`)
  }
  return Effect.runPromise(effect)
}

const importUrl = 'https://openagents.com/api/operator/crm/import'

const jsonPost = (body: unknown): Request =>
  new Request(importUrl, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM import route', () => {
  test('imports a JSON CSV body and returns the summary', async () => {
    const response = await run(true, jsonPost({ csv: 'email\nada@example.com', sourceLabel: 'csv:test' }))
    expect(response.status).toBe(200)
    const json = (await response.json()) as { summary: { importedRows: number; totalRows: number } }
    expect(json.summary.totalRows).toBe(1)
    expect(json.summary.importedRows).toBe(1)
  })

  test('401 without an admin token', async () => {
    const response = await run(false, jsonPost({ csv: 'email\nada@example.com' }))
    expect(response.status).toBe(401)
  })

  test('400 when the csv body is empty', async () => {
    const response = await run(true, jsonPost({ csv: '   ' }))
    expect(response.status).toBe(400)
  })

  test('405 on a non-POST method', async () => {
    const response = await run(true, new Request(importUrl, { method: 'GET' }))
    expect(response.status).toBe(405)
  })

  test('non-import paths pass through (undefined)', () => {
    const routes = makeCrmImportRoutes<TestEnv>({
      requireAdminApiToken: () => Promise.resolve(true),
    })
    const effect = routes.routeCrmImportRequest(
      new Request('https://openagents.com/api/operator/crm/contacts', { method: 'POST' }),
      { OPENAGENTS_DB: importDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
