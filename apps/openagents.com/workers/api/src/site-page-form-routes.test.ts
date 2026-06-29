import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  makeNativeListsService,
  type NativeListsRuntime,
} from './native-lists'
import { makeSitePageFormRoutes } from './site-page-form-routes'
import type { FormCaptureSpec } from './site-page-kinds'

// Minimal real-SQL D1 adapter backed by node:sqlite (mirrors native-lists.test)
// so the sink exercises genuine idempotency against migration 0181.
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

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
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
}

const migrationSql = readFileSync(
  join(__dirname, '..', 'migrations', '0181_native_lists_subscribers.sql'),
  'utf8',
)

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec('CREATE TABLE users (id TEXT PRIMARY KEY)')
  db.exec('CREATE TABLE teams (id TEXT PRIMARY KEY)')
  db.exec('PRAGMA foreign_keys = ON')
  db.exec(migrationSql)
  return new SqliteD1(db) as unknown as D1Database
}

let counter = 0
const runtime: NativeListsRuntime = {
  makeId: (prefix: string) => `${prefix}_${(counter += 1)}`,
  nowIso: () => '2026-06-14T12:00:00.000Z',
}

const FORM_ID = 'opt_in_hero'

const formSpecFor = (listId: string): FormCaptureSpec => ({
  id: FORM_ID,
  listId,
  fields: [
    { name: 'email', kind: 'email' },
    { name: 'name', kind: 'text', required: true },
    { name: 'consent', kind: 'consent' },
  ],
})

type TestBindings = Readonly<Record<string, unknown>>

// Build routes whose sink is a real native-lists service over node:sqlite. The
// list is created first so the spec's listId points at a real subscriber_lists
// row (the list_subscribers FK), then the spec lookup returns it for the known
// formId only.
const makeHarness = async () => {
  const service = makeNativeListsService(makeDb(), runtime)
  const list = await service.createList({
    name: 'Opt-in Waitlist',
    sourceAuthorityRef: 'site.form.v1',
  })
  const spec = formSpecFor(list.id)
  const routes = makeSitePageFormRoutes<TestBindings>({
    makeSink: () => ({ addSubscriber: service.addSubscriber }),
    lookupFormSpec: async (_env, formId) =>
      formId === FORM_ID ? spec : undefined,
    nowIso: () => '2026-06-14T12:00:00.000Z',
  })
  return { routes, service, listId: list.id }
}

const post = (formId: string, body: unknown): Request =>
  new Request(`https://openagents.com/api/sites/forms/${formId}/submit`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })

const ctx = {} as ExecutionContext
const env: TestBindings = {}

const run = async (
  routes: ReturnType<typeof makeSitePageFormRoutes<TestBindings>>,
  request: Request,
): Promise<Response> => {
  const effect = routes.routeSitePageFormRequest(request, env, ctx)
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

beforeEach(() => {
  counter = 0
})

describe('site page form-capture route', () => {
  test('valid submission captures the lead → 201', async () => {
    const { routes, service, listId } = await makeHarness()

    const response = await run(
      routes,
      post('opt_in_hero', {
        email: 'LEAD@example.com',
        name: 'Ada Lovelace',
        consent: true,
      }),
    )

    expect(response.status).toBe(201)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.idempotent).toBe(false)
    expect(json.email).toBe('lead@example.com')
    expect(json.listId).toBe(listId)

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(1)
    expect(subscribers[0]?.sourceRef).toBe('site_form.opt_in_hero')
  })

  test('replayed submission is idempotent → 200', async () => {
    const { routes, service, listId } = await makeHarness()

    const first = await run(
      routes,
      post('opt_in_hero', { email: 'lead@example.com', name: 'Ada' }),
    )
    expect(first.status).toBe(201)

    const replay = await run(
      routes,
      post('opt_in_hero', { email: 'Lead@Example.com', name: 'Ada' }),
    )
    expect(replay.status).toBe(200)
    const json = (await replay.json()) as Record<string, unknown>
    expect(json.idempotent).toBe(true)
    expect(json.email).toBe('lead@example.com')

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(1)
  })

  test('missing email fails validation → 400', async () => {
    const { routes, service, listId } = await makeHarness()

    const response = await run(
      routes,
      post('opt_in_hero', { name: 'No Email' }),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('site_page_form_validation_error')

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(0)
  })

  test('missing required non-email field fails validation → 400', async () => {
    const { routes } = await makeHarness()
    const response = await run(
      routes,
      post('opt_in_hero', { email: 'lead@example.com' }),
    )
    expect(response.status).toBe(400)
  })

  test('malformed email fails validation → 400', async () => {
    const { routes } = await makeHarness()
    const response = await run(
      routes,
      post('opt_in_hero', { email: 'not-an-email', name: 'X' }),
    )
    expect(response.status).toBe(400)
  })

  test('unknown formId → 404', async () => {
    const { routes } = await makeHarness()
    const response = await run(routes, post('does_not_exist', {
      email: 'lead@example.com',
      name: 'Ada',
    }))
    expect(response.status).toBe(404)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('site_page_form_not_found')
  })

  test('non-POST method → 405', async () => {
    const { routes } = await makeHarness()
    const request = new Request(
      'https://openagents.com/api/sites/forms/opt_in_hero/submit',
      { method: 'GET' },
    )
    const response = await run(routes, request)
    expect(response.status).toBe(405)
  })

  test('non-matching path → route does not match (undefined)', async () => {
    const { routes } = await makeHarness()
    const request = new Request('https://openagents.com/api/lists/abc', {
      method: 'POST',
    })
    expect(
      routes.routeSitePageFormRequest(request, env, ctx),
    ).toBeUndefined()
  })
})
