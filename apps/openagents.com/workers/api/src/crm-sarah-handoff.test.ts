import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  buildSarahHandoffUrl,
  CRM_SARAH_HANDOFF_DEFAULT_BASE_URL,
  CrmSarahHandoffValidationError,
  makeD1CrmSarahHandoffStore,
} from './crm-sarah-handoff'

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0218_crm_contacts.sql'))
  db.exec(migration('0311_crm_sarah_handoff_links.sql'))
  db.exec(
    `INSERT INTO crm_contacts (id, tenant_ref, primary_email, created_at, updated_at)
     VALUES ('crm_contact_1', 'tenant.openagents', 'ada@example.com', '2026-07-08T00:00:00.000Z', '2026-07-08T00:00:00.000Z')`,
  )
  return new SqliteD1(db) as unknown as D1Database
}

const runtime = {
  makeToken: (() => {
    let n = 0
    return () => `sh_test_${(n += 1)}`
  })(),
  nowIso: () => '2026-07-08T07:00:00.000Z',
}

describe('crm-sarah-handoff (OB-5, #8562)', () => {
  test('issues a token bound to the CRM contact and builds the prospect URL', async () => {
    const db = makeDb()
    const store = makeD1CrmSarahHandoffStore(db)

    const issued = await store.issueHandoffLink(
      {
        tenantRef: 'tenant.openagents',
        contactId: 'crm_contact_1',
        sourceRef: 'apollo_agent_readiness_ecommerce',
        replyEventId: 'crm_reply_event_1',
      },
      undefined,
      runtime,
    )

    expect(issued.handoffToken).toBe('sh_test_1')
    expect(issued.url).toBe(`${CRM_SARAH_HANDOFF_DEFAULT_BASE_URL}/continue/sh_test_1`)
    expect(issued.receiptRef).toBe('crm_sarah_handoff:sh_test_1')
    expect(issued.sourceRef).toBe('apollo_agent_readiness_ecommerce')

    const link = await store.readHandoffLinkByToken(issued.handoffToken)
    expect(link).not.toBeNull()
    expect(link?.contactId).toBe('crm_contact_1')
    expect(link?.replyEventId).toBe('crm_reply_event_1')
    expect(link?.clickCount).toBe(0)
  })

  test('honors a custom base URL and defaults a missing sourceRef to direct', async () => {
    const db = makeDb()
    const store = makeD1CrmSarahHandoffStore(db)

    const issued = await store.issueHandoffLink(
      { tenantRef: 'tenant.openagents', contactId: 'crm_contact_1' },
      'https://staging.openagents.com/sarah/',
      runtime,
    )

    expect(issued.url).toBe('https://staging.openagents.com/sarah/continue/sh_test_2')
    expect(issued.sourceRef).toBe('direct')
  })

  test('recordHandoffClick increments click count and returns null for unknown tokens', async () => {
    const db = makeDb()
    const store = makeD1CrmSarahHandoffStore(db)
    const issued = await store.issueHandoffLink(
      { tenantRef: 'tenant.openagents', contactId: 'crm_contact_1', sourceRef: 'direct' },
      undefined,
      runtime,
    )

    const click1 = await store.recordHandoffClick(issued.handoffToken, runtime)
    expect(click1).toEqual({
      sourceRef: 'direct',
      contactId: 'crm_contact_1',
      clickCount: 1,
    })

    const click2 = await store.recordHandoffClick(issued.handoffToken, runtime)
    expect(click2?.clickCount).toBe(2)

    expect(await store.recordHandoffClick('sh_does_not_exist', runtime)).toBeNull()
    expect(await store.recordHandoffClick('../etc/passwd', runtime)).toBeNull()
  })

  test('readHandoffLinkByToken returns null for malformed or unknown tokens', async () => {
    const db = makeDb()
    const store = makeD1CrmSarahHandoffStore(db)

    expect(await store.readHandoffLinkByToken('sh_does_not_exist')).toBeNull()
    expect(await store.readHandoffLinkByToken('../etc/passwd')).toBeNull()
  })

  test('rejects unsafe refs at the storage boundary', async () => {
    const db = makeDb()
    const store = makeD1CrmSarahHandoffStore(db)

    await expect(
      store.issueHandoffLink(
        { tenantRef: 'tenant.openagents', contactId: 'not safe / ref?' },
        undefined,
        runtime,
      ),
    ).rejects.toBeInstanceOf(CrmSarahHandoffValidationError)
  })

  test('buildSarahHandoffUrl trims trailing slashes and falls back to the default origin', () => {
    expect(buildSarahHandoffUrl('https://openagents.com/sarah/', 'tok')).toBe(
      'https://openagents.com/sarah/continue/tok',
    )
    expect(buildSarahHandoffUrl('', 'tok')).toBe(
      `${CRM_SARAH_HANDOFF_DEFAULT_BASE_URL}/continue/tok`,
    )
  })
})
