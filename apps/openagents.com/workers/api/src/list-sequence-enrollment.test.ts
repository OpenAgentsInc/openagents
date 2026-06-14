import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { beforeEach, describe, expect, test } from 'vitest'

import {
  addEmailSuppression,
  type EmailCampaignRuntime,
  upsertEmailPreference,
} from './email-campaigns'
import { createEmailSequence } from './email-sequence-authoring'
import {
  enrollListSubscriberInSequence,
  enrollNewSubscriberAndSequence,
} from './list-sequence-enrollment'
import {
  makeNativeListsService,
  type NativeListsRuntime,
} from './native-lists'

// Real-SQL D1 adapter backed by node:sqlite (mirrors native-lists.test.ts) so
// idempotency, uniqueness, and suppression/preference behavior are exercised
// against genuine SQL across migrations 0181 (native lists) and 0063 (email
// campaign/sequence engine).
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

const readMigration = (file: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', file), 'utf8')

const listsMigration = readMigration('0181_native_lists_subscribers.sql')
const campaignsMigration = readMigration('0063_email_campaign_records.sql')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  // Stub referenced parent tables so FK references resolve.
  db.exec('CREATE TABLE users (id TEXT PRIMARY KEY)')
  db.exec('CREATE TABLE teams (id TEXT PRIMARY KEY)')
  db.exec('CREATE TABLE email_messages (id TEXT PRIMARY KEY)')
  db.exec('PRAGMA foreign_keys = ON')
  db.exec(listsMigration)
  db.exec(campaignsMigration)
  return new SqliteD1(db) as unknown as D1Database
}

let counter = 0
const nextId = (prefix: string): string => `${prefix}_${(counter += 1)}`

const listsRuntime: NativeListsRuntime = {
  makeId: nextId,
  nowIso: () => '2026-06-14T12:00:00.000Z',
}
const campaignRuntime: EmailCampaignRuntime = {
  makeId: nextId,
  nowIso: () => '2026-06-14T12:00:00.000Z',
}

const OPERATOR = 'operator_user_1'
const SEQUENCE_SLUG = 'welcome-nurture'

// Seed a list with one active subscriber and an active two-step sequence.
// Returns the created listId.
const seedListAndSequence = async (
  db: D1Database,
  email: string,
): Promise<string> => {
  const service = makeNativeListsService(db, listsRuntime)
  const list = await service.createList({
    name: 'Launch Waitlist',
    sourceAuthorityRef: 'site.form.v1',
  })
  await service.addSubscriber({
    email,
    listId: list.id,
    sourceRef: 'site.form.v1',
  })

  await createEmailSequence(
    db,
    OPERATOR,
    {
      audience: 'sales_qualified_leads',
      name: 'Welcome nurture',
      slug: SEQUENCE_SLUG,
      status: 'active',
      steps: [
        {
          delaySeconds: 0,
          name: 'Day 0 intro',
          stepKey: 'day_0',
          templateSlug: 'sequence.welcome.day_0.v1',
        },
        {
          delaySeconds: 86_400,
          name: 'Day 1 value',
          stepKey: 'day_1',
          templateSlug: 'sequence.welcome.day_1.v1',
        },
      ],
    },
    campaignRuntime,
  )

  return list.id
}

const countSends = (db: D1Database, email: string): Promise<number> =>
  db
    .prepare(
      `SELECT COUNT(*) AS n FROM email_campaign_sends WHERE email = ?`,
    )
    .bind(email)
    .first<{ n: number }>()
    .then(row => row?.n ?? 0)

beforeEach(() => {
  counter = 0
})

describe('enrollListSubscriberInSequence', () => {
  test('enrolls an active list subscriber into the sequence', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)

    const result = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result.status).toBe('enrolled')
    if (result.status === 'enrolled') {
      expect(result.scheduledSendCount).toBe(2)
      expect(result.subscriber.email).toBe(email)
    }
    expect(await countSends(db, email)).toBe(2)
  })

  test('normalizes mixed-case email when resolving the subscriber', async () => {
    const db = makeDb()
    const stored = 'lead@example.com'
    const listId = await seedListAndSequence(db, stored)

    const result = await enrollListSubscriberInSequence(
      db,
      { email: 'LEAD@Example.com', listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result.status).toBe('enrolled')
    expect(await countSends(db, stored)).toBe(2)
  })

  test('skips when subscriber is not on the list', async () => {
    const db = makeDb()
    const listId = await seedListAndSequence(db, 'lead@example.com')

    const result = await enrollListSubscriberInSequence(
      db,
      { email: 'stranger@example.com', listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result).toEqual({
      reason: 'subscriber_not_on_list',
      status: 'skipped',
    })
    expect(await countSends(db, 'stranger@example.com')).toBe(0)
  })

  test('skips an unsubscribed (inactive) list subscriber', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)
    const service = makeNativeListsService(db, listsRuntime)
    await service.unsubscribe({ email, listId })

    const result = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result).toEqual({
      reason: 'subscriber_not_active',
      status: 'skipped',
    })
    expect(await countSends(db, email)).toBe(0)
  })

  test('honors drip suppression downstream (active on list, suppressed email)', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)
    await addEmailSuppression(
      db,
      {
        email,
        reason: 'unsubscribe',
        scope: 'drip',
        sourceAuthorityRef: 'test',
      },
      campaignRuntime,
    )

    const result = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result.status).toBe('skipped')
    if (result.status === 'skipped') {
      expect(result.reason).toBe('drip_suppressed')
    }
    expect(await countSends(db, email)).toBe(0)
  })

  test('honors drip preference opt-out downstream', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)
    await upsertEmailPreference(
      db,
      {
        dripOptIn: false,
        email,
        marketingOptIn: true,
        sourceAuthorityRef: 'test',
        transactionalOptIn: true,
      },
      campaignRuntime,
    )

    const result = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result.status).toBe('skipped')
    if (result.status === 'skipped') {
      expect(result.reason).toBe('drip_preference_disabled')
    }
    expect(await countSends(db, email)).toBe(0)
  })

  test('is idempotent on replay (no duplicate scheduled sends)', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)

    const first = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )
    const second = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: SEQUENCE_SLUG },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(first.status).toBe('enrolled')
    expect(second.status).toBe('enrolled')
    expect(await countSends(db, email)).toBe(2)
  })

  test('skips when the named sequence does not exist', async () => {
    const db = makeDb()
    const email = 'lead@example.com'
    const listId = await seedListAndSequence(db, email)

    const result = await enrollListSubscriberInSequence(
      db,
      { email, listId, sequenceSlug: 'no-such-sequence' },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result).toEqual({
      reason: 'sequence_not_found',
      status: 'skipped',
    })
  })
})

describe('enrollNewSubscriberAndSequence', () => {
  test('captures a new subscriber and enrolls in one call', async () => {
    const db = makeDb()
    const email = 'fresh@example.com'
    // List + sequence exist, but the subscriber is NOT pre-seeded.
    const service = makeNativeListsService(db, listsRuntime)
    const list = await service.createList({
      name: 'Launch Waitlist',
      sourceAuthorityRef: 'site.form.v1',
    })
    await createEmailSequence(
      db,
      OPERATOR,
      {
        audience: 'sales_qualified_leads',
        name: 'Welcome nurture',
        slug: SEQUENCE_SLUG,
        status: 'active',
        steps: [
          {
            delaySeconds: 0,
            name: 'Day 0 intro',
            stepKey: 'day_0',
            templateSlug: 'sequence.welcome.day_0.v1',
          },
        ],
      },
      campaignRuntime,
    )

    const result = await enrollNewSubscriberAndSequence(
      db,
      {
        email,
        listId: list.id,
        sequenceSlug: SEQUENCE_SLUG,
        sourceRef: 'site.form.v1',
      },
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(result.status).toBe('enrolled')
    expect(result.listIdempotent).toBe(false)
    expect(await countSends(db, email)).toBe(1)
  })

  test('double form submit is idempotent (one subscriber, no duplicate sends)', async () => {
    const db = makeDb()
    const email = 'fresh@example.com'
    const service = makeNativeListsService(db, listsRuntime)
    const list = await service.createList({
      name: 'Launch Waitlist',
      sourceAuthorityRef: 'site.form.v1',
    })
    await createEmailSequence(
      db,
      OPERATOR,
      {
        audience: 'sales_qualified_leads',
        name: 'Welcome nurture',
        slug: SEQUENCE_SLUG,
        status: 'active',
        steps: [
          {
            delaySeconds: 0,
            name: 'Day 0 intro',
            stepKey: 'day_0',
            templateSlug: 'sequence.welcome.day_0.v1',
          },
        ],
      },
      campaignRuntime,
    )

    const args = {
      email,
      listId: list.id,
      sequenceSlug: SEQUENCE_SLUG,
      sourceRef: 'site.form.v1',
    }
    const first = await enrollNewSubscriberAndSequence(
      db,
      args,
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )
    const second = await enrollNewSubscriberAndSequence(
      db,
      args,
      OPERATOR,
      listsRuntime,
      campaignRuntime,
    )

    expect(first.listIdempotent).toBe(false)
    expect(second.listIdempotent).toBe(true)
    expect(await countSends(db, email)).toBe(1)
    const subscribers = await service.listSubscribers({ listId: list.id })
    expect(subscribers.length).toBe(1)
  })
})
