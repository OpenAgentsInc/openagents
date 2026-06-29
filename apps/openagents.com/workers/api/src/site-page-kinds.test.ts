import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { beforeEach, describe, expect, test } from 'vitest'

import {
  makeNativeListsService,
  type NativeListsRuntime,
} from './native-lists'
import {
  ALL_SITE_PAGE_KINDS,
  captureFormSubmission,
  describeSitePageKind,
  type FormCaptureSink,
  type FormCaptureSpec,
  missingRequiredSections,
  SITE_PAGE_KIND_DEFINITIONS,
} from './site-page-kinds'

// Reuse the native-lists test fake: a minimal real-SQL D1 adapter backed by
// node:sqlite so the form-capture path exercises genuine addSubscriber
// idempotency.
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

beforeEach(() => {
  counter = 0
})

describe('site page kinds', () => {
  test('every kind has a definition with title, summary, sections', () => {
    for (const kind of ALL_SITE_PAGE_KINDS) {
      const def = describeSitePageKind(kind)
      expect(def.kind).toBe(kind)
      expect(def.title.length).toBeGreaterThan(0)
      expect(def.summary.length).toBeGreaterThan(0)
      expect(def.requiredSections.length).toBeGreaterThan(0)
    }
    expect(Object.keys(SITE_PAGE_KIND_DEFINITIONS).sort()).toEqual(
      ['landing', 'opt_in', 'sales', 'thank_you'].sort(),
    )
  })

  test('landing requires hero + cta', () => {
    expect(describeSitePageKind('landing').requiredSections).toEqual([
      'hero',
      'cta',
    ])
    expect(describeSitePageKind('landing').capturesLeads).toBe(false)
  })

  test('sales requires hero, value_props, pricing, cta', () => {
    expect(describeSitePageKind('sales').requiredSections).toEqual([
      'hero',
      'value_props',
      'pricing',
      'cta',
    ])
  })

  test('opt_in captures leads and requires a lead_form', () => {
    const def = describeSitePageKind('opt_in')
    expect(def.capturesLeads).toBe(true)
    expect(def.requiredSections).toContain('lead_form')
  })

  test('thank_you requires confirmation + next_steps', () => {
    expect(describeSitePageKind('thank_you').requiredSections).toEqual([
      'confirmation',
      'next_steps',
    ])
  })

  test('missingRequiredSections flags gaps and passes complete pages', () => {
    expect(missingRequiredSections('sales', ['hero', 'value_props'])).toEqual([
      'pricing',
      'cta',
    ])
    expect(
      missingRequiredSections('landing', ['hero', 'cta', 'footer']),
    ).toEqual([])
  })
})

describe('form-capture primitive', () => {
  const spec: FormCaptureSpec = {
    id: 'optin_main',
    listId: 'subscriber_list_seed',
    fields: [
      { name: 'email', kind: 'email', required: true },
      { name: 'name', kind: 'text' },
      { name: 'consent', kind: 'consent', required: true },
    ],
  }

  const makeSink = async (): Promise<{
    sink: FormCaptureSink
    listId: string
    service: ReturnType<typeof makeNativeListsService>
  }> => {
    const service = makeNativeListsService(makeDb(), runtime)
    const list = await service.createList({
      name: 'Opt-in Waitlist',
      sourceAuthorityRef: 'site.form.v1',
    })
    const boundSpec = { ...spec, listId: list.id }
    return {
      sink: { addSubscriber: service.addSubscriber },
      listId: boundSpec.listId,
      service,
    }
  }

  test('valid submission captures a subscriber into the native list', async () => {
    const { sink, listId, service } = await makeSink()

    const outcome = await captureFormSubmission(
      {
        formSpec: { ...spec, listId },
        submission: {
          email: 'LEAD@Example.com',
          name: 'Ada',
          consent: true,
        },
      },
      sink,
    )

    expect(outcome._tag).toBe('captured')
    if (outcome._tag === 'captured') {
      expect(outcome.email).toBe('lead@example.com')
      expect(outcome.listId).toBe(listId)
    }

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers.map(s => s.email)).toEqual(['lead@example.com'])
    expect(subscribers[0]?.sourceRef).toBe('site_form.optin_main')
    expect(JSON.parse(subscribers[0]?.metadataJson ?? '{}')).toMatchObject({
      name: 'Ada',
      consent: true,
    })
  })

  test('replay of the same email is idempotent', async () => {
    const { sink, listId, service } = await makeSink()

    await captureFormSubmission(
      { formSpec: { ...spec, listId }, submission: { email: 'a@b.com', consent: true } },
      sink,
    )
    const replay = await captureFormSubmission(
      { formSpec: { ...spec, listId }, submission: { email: 'a@b.com', consent: true } },
      sink,
    )

    expect(replay._tag).toBe('idempotent')
    expect(await service.listSubscribers({ listId })).toHaveLength(1)
  })

  test('missing email is a validation error and writes nothing', async () => {
    const { sink, listId, service } = await makeSink()

    const outcome = await captureFormSubmission(
      { formSpec: { ...spec, listId }, submission: { name: 'NoEmail', consent: true } },
      sink,
    )

    expect(outcome._tag).toBe('validation_error')
    expect(await service.listSubscribers({ listId })).toHaveLength(0)
  })

  test('invalid email is a validation error', async () => {
    const { sink, listId } = await makeSink()

    const outcome = await captureFormSubmission(
      { formSpec: { ...spec, listId }, submission: { email: 'not-an-email', consent: true } },
      sink,
    )

    expect(outcome._tag).toBe('validation_error')
  })

  test('missing required non-email field is a validation error', async () => {
    const { sink, listId, service } = await makeSink()

    const outcome = await captureFormSubmission(
      { formSpec: { ...spec, listId }, submission: { email: 'has@email.com' } },
      sink,
    )

    expect(outcome._tag).toBe('validation_error')
    if (outcome._tag === 'validation_error') {
      expect(outcome.reason).toContain('consent')
    }
    expect(await service.listSubscribers({ listId })).toHaveLength(0)
  })

  test('custom sourceRef overrides the default', async () => {
    const { sink, listId, service } = await makeSink()

    await captureFormSubmission(
      {
        formSpec: { ...spec, listId },
        submission: { email: 'c@d.com', consent: true },
        sourceRef: 'optin.hero_cta',
      },
      sink,
    )

    const subscribers = await service.listSubscribers({ listId })
    expect(subscribers[0]?.sourceRef).toBe('optin.hero_cta')
  })
})
