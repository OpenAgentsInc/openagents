import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { beforeEach, describe, expect, test } from 'vitest'

import {
  makeListSubscriberRecord,
  makeNativeListsService,
  makeSubscriberListRecord,
  type NativeListsRuntime,
} from './native-lists'

// Minimal real-SQL D1 adapter backed by node:sqlite so idempotency and
// uniqueness guarantees are exercised against genuine SQL.
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
  // Stub the referenced parent tables so FK references resolve.
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

describe('native lists records', () => {
  test('subscriber list slugifies name when no slug given', () => {
    const record = makeSubscriberListRecord(
      { name: 'Launch Waitlist!', sourceAuthorityRef: 'site.form.v1' },
      runtime,
    )
    expect(record.slug).toBe('launch-waitlist')
    expect(record.status).toBe('active')
  })

  test('subscriber idempotency key keys off list + normalized email', () => {
    const record = makeListSubscriberRecord(
      { email: 'Ben@Silones.com', listId: 'subscriber_list_1', sourceRef: 's' },
      runtime,
    )
    expect(record.email).toBe('ben@silones.com')
    expect(record.idempotencyKey).toBe(
      'list_subscriber:subscriber_list_1:ben@silones.com',
    )
  })
})

describe('native lists service', () => {
  test('createList then addSubscriber is idempotent on replay', async () => {
    const service = makeNativeListsService(makeDb(), runtime)
    const list = await service.createList({
      name: 'Launch Waitlist',
      sourceAuthorityRef: 'site.form.v1',
    })
    expect(list.slug).toBe('launch-waitlist')

    const first = await service.addSubscriber({
      email: 'LEAD@example.com',
      listId: list.id,
      sourceRef: 'homepage_form',
    })
    expect(first.idempotent).toBe(false)
    expect(first.subscriber.email).toBe('lead@example.com')
    expect(first.subscriber.status).toBe('active')

    const replay = await service.addSubscriber({
      email: 'lead@example.com',
      listId: list.id,
      sourceRef: 'different_form',
    })
    expect(replay.idempotent).toBe(true)
    // Original row preserved (same id, original source_ref).
    expect(replay.subscriber.id).toBe(first.subscriber.id)
    expect(replay.subscriber.sourceRef).toBe('homepage_form')

    const subscribers = await service.listSubscribers({ listId: list.id })
    expect(subscribers).toHaveLength(1)
  })

  test('listSubscribers can filter by status and unsubscribe flips status', async () => {
    const service = makeNativeListsService(makeDb(), runtime)
    const list = await service.createList({
      name: 'Newsletter',
      sourceAuthorityRef: 'site.form.v1',
    })
    await service.addSubscriber({
      email: 'a@example.com',
      listId: list.id,
      sourceRef: 'form',
    })
    await service.addSubscriber({
      email: 'b@example.com',
      listId: list.id,
      sourceRef: 'form',
    })

    const updated = await service.unsubscribe({
      email: 'A@example.com',
      listId: list.id,
    })
    expect(updated?.status).toBe('unsubscribed')

    const active = await service.listSubscribers({
      listId: list.id,
      status: 'active',
    })
    expect(active.map(s => s.email)).toEqual(['b@example.com'])

    const unsubscribed = await service.listSubscribers({
      listId: list.id,
      status: 'unsubscribed',
    })
    expect(unsubscribed.map(s => s.email)).toEqual(['a@example.com'])
  })

  test('readList returns undefined for unknown id', async () => {
    const service = makeNativeListsService(makeDb(), runtime)
    expect(await service.readList('subscriber_list_missing')).toBeUndefined()
  })
})
