import { describe, expect, test } from 'vitest'

import {
  completeCrmSourceImportRun,
  type CrmRuntime,
  listCrmContacts,
  normalizeCrmEmail,
  recordCrmActivity,
  startCrmSourceImportRun,
  upsertCrmContact,
} from './crm-store'

// ---------------------------------------------------------------------------
// Recording D1 fake: captures prepared queries + bindings, and serves a small
// queue of `.first()` results so we can drive the upsert SELECT->write->reread
// path deterministically.
// ---------------------------------------------------------------------------

type Recorded = Readonly<{ bound: ReadonlyArray<unknown>; query: string }>

class RecordingDb {
  readonly runs: Array<Recorded> = []
  readonly firsts: Array<Recorded> = []
  readonly alls: Array<Recorded> = []
  firstQueue: Array<unknown> = []
  allResults: Array<Record<string, unknown>> = []

  prepare(query: string): D1PreparedStatement {
    const db = this
    const make = (bound: ReadonlyArray<unknown>): D1PreparedStatement =>
      ({
        bind: (...values: ReadonlyArray<unknown>) => make(values),
        first: <T,>() => {
          db.firsts.push({ bound, query })
          const next = db.firstQueue.length > 0 ? db.firstQueue.shift() : null
          return Promise.resolve((next ?? null) as T | null)
        },
        all: <T,>() => {
          db.alls.push({ bound, query })
          return Promise.resolve({
            meta: {} as D1Meta,
            results: db.allResults as unknown as Array<T>,
            success: true,
          } as D1Result<T>)
        },
        run: () => {
          db.runs.push({ bound, query })
          return Promise.resolve({
            meta: {} as D1Meta,
            results: [],
            success: true,
          } as unknown as D1Result)
        },
        raw: () => Promise.reject(new Error('raw should not be used')),
      }) as unknown as D1PreparedStatement
    return make([])
  }
}

const asDb = (db: RecordingDb): D1Database => db as unknown as D1Database

const runtime: CrmRuntime = {
  makeId: (prefix: string) => `${prefix}_test`,
  nowIso: () => '2026-06-22T00:00:00.000Z',
}

const contactRow = (over: Record<string, unknown> = {}): Record<string, unknown> => ({
  account_id: null,
  contact_type: 'investor',
  created_at: '2026-06-22T00:00:00.000Z',
  engagement_score: 7,
  full_name: 'Ada Lovelace',
  id: 'crm_contact_test',
  lifecycle_stage: 'lead',
  primary_email: 'ada@example.com',
  relationship_stage: 'new',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
  ...over,
})

describe('normalizeCrmEmail', () => {
  test('trims and lowercases', () => {
    expect(normalizeCrmEmail('  Ada@Example.COM ')).toBe('ada@example.com')
  })
})

describe('upsertCrmContact', () => {
  test('inserts a new contact (normalized email) and reports created=true', async () => {
    const db = new RecordingDb()
    // find -> null; created reread -> row
    db.firstQueue = [null, contactRow()]

    const result = await upsertCrmContact(
      asDb(db),
      { primaryEmail: '  Ada@Example.com ', tenantRef: 'tenant.openagents' },
      runtime,
    )

    expect(result.created).toBe(true)
    expect(result.contact.primaryEmail).toBe('ada@example.com')
    expect(result.contact.engagementScore).toBe(7)

    const insert = db.runs.find(r => r.query.includes('INSERT INTO crm_contacts'))
    expect(insert).toBeDefined()
    // normalized email is bound (third positional bind after id, tenant_ref)
    expect(insert?.bound).toContain('ada@example.com')
    expect(insert?.bound).toContain('tenant.openagents')
    // the initial find SELECT used the normalized email
    const find = db.firsts[0]
    expect(find?.query).toContain('SELECT * FROM crm_contacts WHERE tenant_ref = ? AND primary_email = ?')
    expect(find?.bound[1]).toBe('ada@example.com')
  })

  test('updates an existing contact and reports created=false', async () => {
    const db = new RecordingDb()
    db.firstQueue = [contactRow(), contactRow({ full_name: 'Ada L.' })]

    const result = await upsertCrmContact(
      asDb(db),
      { fullName: 'Ada L.', primaryEmail: 'ada@example.com', tenantRef: 'tenant.openagents' },
      runtime,
    )

    expect(result.created).toBe(false)
    expect(result.contact.fullName).toBe('Ada L.')
    const update = db.runs.find(r => r.query.includes('UPDATE crm_contacts SET'))
    expect(update).toBeDefined()
    expect(db.runs.some(r => r.query.includes('INSERT INTO crm_contacts'))).toBe(false)
  })
})

describe('recordCrmActivity', () => {
  test('uses INSERT OR IGNORE and binds the source dedupe pair', async () => {
    const db = new RecordingDb()
    await recordCrmActivity(
      asDb(db),
      {
        activityType: 'email_sent',
        contactId: 'crm_contact_test',
        sourceRecordId: 'msg_1',
        sourceRecordType: 'email_message',
        tenantRef: 'tenant.openagents',
      },
      runtime,
    )
    const insert = db.runs[0]
    expect(insert?.query).toContain('INSERT OR IGNORE INTO crm_activities')
    expect(insert?.bound).toContain('email_message')
    expect(insert?.bound).toContain('msg_1')
    expect(insert?.bound).toContain('email_sent')
  })
})

describe('listCrmContacts', () => {
  test('decodes rows and filters by tenant', async () => {
    const db = new RecordingDb()
    db.allResults = [contactRow(), contactRow({ id: 'crm_contact_2', primary_email: 'b@e.com' })]
    const contacts = await listCrmContacts(asDb(db), 'tenant.openagents', {})
    expect(contacts).toHaveLength(2)
    expect(contacts[0]?.primaryEmail).toBe('ada@example.com')
    expect(db.alls[0]?.bound[0]).toBe('tenant.openagents')
    expect(db.alls[0]?.query).not.toContain('LIKE')
  })

  test('search path uses a LIKE clause with the lowercased term', async () => {
    const db = new RecordingDb()
    db.allResults = []
    await listCrmContacts(asDb(db), 'tenant.openagents', { search: 'Ada' })
    expect(db.alls[0]?.query).toContain('LIKE')
    expect(db.alls[0]?.bound).toContain('%ada%')
  })

  test('clamps an absurd limit to the max', async () => {
    const db = new RecordingDb()
    db.allResults = []
    await listCrmContacts(asDb(db), 'tenant.openagents', { limit: 100000 })
    expect(db.alls[0]?.bound).toContain(500)
  })
})

describe('crm source import run lifecycle', () => {
  test('starts running then completes with honest counts', async () => {
    const db = new RecordingDb()
    const id = await startCrmSourceImportRun(
      asDb(db),
      { sourceLabel: 'csv:investors', tenantRef: 'tenant.openagents' },
      runtime,
    )
    expect(id).toBe('crm_import_test')
    expect(db.runs[0]?.query).toContain('INSERT INTO crm_source_import_runs')
    expect(db.runs[0]?.query).toContain("'running'")

    await completeCrmSourceImportRun(
      asDb(db),
      {
        duplicateRows: 2,
        failedRows: 0,
        id,
        importedRows: 8,
        status: 'completed',
        totalRows: 10,
        updatedRows: 0,
      },
      runtime,
    )
    const update = db.runs.find(r => r.query.includes('UPDATE crm_source_import_runs'))
    expect(update?.bound).toContain('completed')
    expect(update?.bound).toContain(8)
  })
})
