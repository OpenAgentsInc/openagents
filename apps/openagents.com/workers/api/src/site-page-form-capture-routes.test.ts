import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  makeNativeListsService,
  type NativeListsRuntime,
} from './native-lists'
import {
  isSiteFormCaptureEnabled,
  makeSitePageFormCaptureRoutes,
  resolveSiteFormSpecFromMetadata,
  SITE_FORM_CAPTURE_BLOCKER_CLEARED,
  SITE_FORM_CAPTURE_PROMISE_ID,
} from './site-page-form-capture-routes'

// Real-SQL D1 adapter backed by node:sqlite (mirrors site-page-form-routes.test
// and native-lists.test) so the sink exercises genuine idempotency against
// migration 0181 — the wiring is proven end to end, not against a stub sink.
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

type TestBindings = Readonly<Record<string, unknown>>

const ctx = {} as ExecutionContext
const env: TestBindings = {}

const post = (formId: string, body: unknown): Request =>
  new Request(`https://openagents.com/api/sites/forms/${formId}/submit`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })

// Build the flag-gated capture routes over a real native-lists sink, resolving
// the FormCaptureSpec from a published-site metadata_json blob via the registry
// (the join that was previously missing). The list is created first so the
// spec's listId points at a real subscriber_lists row.
const makeHarness = async (enabled: boolean) => {
  const service = makeNativeListsService(makeDb(), runtime)
  const list = await service.createList({
    name: 'Opt-in Waitlist',
    sourceAuthorityRef: 'site.form.v1',
  })

  // A realistic published-site metadata_json carrying a formSpecs map — exactly
  // what site_versions.metadata_json holds in production.
  const siteMetadataJson = JSON.stringify({
    title: 'Acme launch site',
    formSpecs: {
      [FORM_ID]: {
        id: FORM_ID,
        listId: list.id,
        fields: [
          { name: 'email', kind: 'email', required: true },
          { name: 'name', kind: 'text', required: true },
        ],
      },
    },
  })

  const routes = makeSitePageFormCaptureRoutes<TestBindings>({
    isEnabled: () => enabled,
    makeSink: () => ({ addSubscriber: service.addSubscriber }),
    // Stand-in for the index.ts D1 read of the active site version's
    // metadata_json that owns this formId. Only the known form resolves.
    readSiteFormMetadata: async (_env, formId) =>
      formId === FORM_ID ? siteMetadataJson : undefined,
    nowIso: () => '2026-06-14T12:00:00.000Z',
  })

  return { routes, service, listId: list.id }
}

const run = async (
  routes: Awaited<ReturnType<typeof makeHarness>>['routes'],
  request: Request,
): Promise<Response> => {
  const effect = routes.routeSitePageFormCaptureRequest(request, env, ctx)
  if (effect === undefined) {
    throw new Error('route did not match (flag off or non-matching path)')
  }
  return Effect.runPromise(effect)
}

beforeEach(() => {
  counter = 0
})

describe('site form-capture feature flag', () => {
  test('absent / non-truthy values disable the route', () => {
    expect(isSiteFormCaptureEnabled(undefined)).toBe(false)
    expect(isSiteFormCaptureEnabled('')).toBe(false)
    expect(isSiteFormCaptureEnabled('false')).toBe(false)
    expect(isSiteFormCaptureEnabled('0')).toBe(false)
    expect(isSiteFormCaptureEnabled('off')).toBe(false)
  })

  test('truthy values enable the route', () => {
    for (const value of ['1', 'true', 'yes', 'on', 'TRUE', ' On ']) {
      expect(isSiteFormCaptureEnabled(value)).toBe(true)
    }
  })
})

describe('site page form-capture wiring (flag OFF → inert)', () => {
  test('every request falls through (undefined) when disabled', async () => {
    const { routes } = await makeHarness(false)

    // A request that WOULD be a valid capture when armed still returns
    // undefined, so the omni dispatch chain falls through unchanged.
    expect(
      routes.routeSitePageFormCaptureRequest(
        post(FORM_ID, { email: 'lead@example.com', name: 'Ada' }),
        env,
        ctx,
      ),
    ).toBeUndefined()

    // Non-matching paths are also undefined (no accidental capture).
    expect(
      routes.routeSitePageFormCaptureRequest(
        new Request('https://openagents.com/api/lists/abc', { method: 'POST' }),
        env,
        ctx,
      ),
    ).toBeUndefined()
  })
})

describe('site page form-capture wiring (flag ON → registry-resolved)', () => {
  test('valid submission resolves the spec from metadata and captures → 201', async () => {
    const { routes, service, listId } = await makeHarness(true)

    const response = await run(
      routes,
      post(FORM_ID, {
        email: 'LEAD@example.com',
        name: 'Ada Lovelace',
      }),
    )

    expect(response.status).toBe(201)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.idempotent).toBe(false)
    expect(json.email).toBe('lead@example.com')
    expect(json.listId).toBe(listId)

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(1)
    expect(subscribers[0]?.sourceRef).toBe(`site_form.${FORM_ID}`)
  })

  test('replayed submission is idempotent → 200', async () => {
    const { routes, service, listId } = await makeHarness(true)

    const first = await run(
      routes,
      post(FORM_ID, { email: 'lead@example.com', name: 'Ada' }),
    )
    expect(first.status).toBe(201)

    const replay = await run(
      routes,
      post(FORM_ID, { email: 'Lead@Example.com', name: 'Ada' }),
    )
    expect(replay.status).toBe(200)
    const json = (await replay.json()) as Record<string, unknown>
    expect(json.idempotent).toBe(true)

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(1)
  })

  test('missing required field fails validation → 400', async () => {
    const { routes, service, listId } = await makeHarness(true)

    const response = await run(routes, post(FORM_ID, { name: 'No Email' }))
    expect(response.status).toBe(400)

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers).toHaveLength(0)
  })

  test('unknown formId (no published spec in metadata) → 404', async () => {
    const { routes } = await makeHarness(true)

    const response = await run(
      routes,
      post('does_not_exist', { email: 'lead@example.com', name: 'Ada' }),
    )
    expect(response.status).toBe(404)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.error).toBe('site_page_form_not_found')
  })

  test('non-POST method → 405', async () => {
    const { routes } = await makeHarness(true)

    const response = await run(
      routes,
      new Request(
        `https://openagents.com/api/sites/forms/${FORM_ID}/submit`,
        { method: 'GET' },
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('resolveSiteFormSpecFromMetadata (registry join)', () => {
  test('decodes a published metadata_json into a typed FormCaptureSpec', async () => {
    const metadataJson = JSON.stringify({
      formSpecs: {
        [FORM_ID]: {
          id: FORM_ID,
          listId: 'list.newsletter',
          fields: [{ name: 'email', kind: 'email', required: true }],
        },
      },
    })

    const spec = await resolveSiteFormSpecFromMetadata(
      async (_env: TestBindings, _formId) => metadataJson,
      env,
      FORM_ID,
    )

    expect(spec?.id).toBe(FORM_ID)
    expect(spec?.listId).toBe('list.newsletter')
  })

  test('malformed metadata degrades to undefined (route renders 404)', async () => {
    const spec = await resolveSiteFormSpecFromMetadata(
      async (_env: TestBindings, _formId) => 'not json {',
      env,
      FORM_ID,
    )
    expect(spec).toBeUndefined()
  })

  test('absent published site (no metadata) degrades to undefined', async () => {
    const spec = await resolveSiteFormSpecFromMetadata(
      async (_env: TestBindings, _formId) => undefined,
      env,
      FORM_ID,
    )
    expect(spec).toBeUndefined()
  })
})

describe('site form-capture blocker/promise refs (honest scope)', () => {
  test('clears only the route-unmounted blocker; promise stays yellow', () => {
    expect(SITE_FORM_CAPTURE_PROMISE_ID).toBe(
      'autopilot_sites.native_email_sequences.v1',
    )
    expect(SITE_FORM_CAPTURE_BLOCKER_CLEARED).toBe(
      'blocker.product_promises.site_form_capture_route_unmounted',
    )
  })
})
