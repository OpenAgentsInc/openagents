// KS-8.11 (#8322): CRM/email/enrichment domain seam — flags, fail-soft
// dual-write, flag-routed reads (including the suppression compliance
// gate's atomic-per-read guarantee), PII-safe diagnostics, and registry
// lock-step with the khala-sync-server backfill core.

import { describe, expect, test } from 'vitest'

import {
  CRM_EMAIL_DOMAIN_TABLES,
  crmEmailAuthorityDb,
  crmEmailDomainFlagsFromEnv,
  crmEmailRead,
  isCrmEmailDomainHandle,
  makeCrmEmailDatabaseForEnv,
  makeCrmEmailDomainHandle,
  mirrorCrmEmailRows,
  publicSafeRefs,
  type CrmEmailDomainDiagnostic,
  type CrmEmailDomainDiagnosticEvent,
  type CrmEmailDomainRow,
  type PostgresCrmEmailDomainStore,
} from './crm-email-domain-store'
import { makeSqliteD1 } from './test/sqlite-d1'

type Logged = readonly [CrmEmailDomainDiagnosticEvent, CrmEmailDomainDiagnostic]

const fakePostgres = (
  overrides: Partial<PostgresCrmEmailDomainStore> = {},
): PostgresCrmEmailDomainStore & {
  upserts: Array<{ table: string; rows: ReadonlyArray<CrmEmailDomainRow> }>
} => {
  const upserts: Array<{
    table: string
    rows: ReadonlyArray<CrmEmailDomainRow>
  }> = []
  return {
    selectLatestRows: () => Promise.resolve([]),
    selectRowsByKey: () => Promise.resolve([]),
    upsertRows: (table, rows) => {
      upserts.push({ rows, table })
      return Promise.resolve()
    },
    upserts,
    ...overrides,
  }
}

describe('crmEmailDomainFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1 (read flips are runbook-gated)', () => {
    expect(crmEmailDomainFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('dual-write off values', () => {
    for (const value of ['0', 'off', 'false', 'disabled', 'no', ' OFF ']) {
      expect(
        crmEmailDomainFlagsFromEnv({ KHALA_SYNC_CRM_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      crmEmailDomainFlagsFromEnv({ KHALA_SYNC_CRM_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('reads: postgres and compare route; typos fall back to d1 (suppression gate never fails open)', () => {
    expect(
      crmEmailDomainFlagsFromEnv({ KHALA_SYNC_CRM_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      crmEmailDomainFlagsFromEnv({ KHALA_SYNC_CRM_READS: ' Compare ' }).reads,
    ).toBe('compare')
    expect(
      crmEmailDomainFlagsFromEnv({ KHALA_SYNC_CRM_READS: 'postgrse' }).reads,
    ).toBe('d1')
  })
})

describe('makeCrmEmailDatabaseForEnv', () => {
  const d1 = {} as D1Database

  test('no KHALA_SYNC_DB binding degrades to the plain D1 database', () => {
    const db = makeCrmEmailDatabaseForEnv({ OPENAGENTS_DB: d1 })
    expect(isCrmEmailDomainHandle(db)).toBe(false)
    expect(crmEmailAuthorityDb(db)).toBe(d1)
  })

  test('everything flagged off degrades to plain D1 even with a binding', () => {
    const db = makeCrmEmailDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      KHALA_SYNC_CRM_DUAL_WRITE: 'off',
      OPENAGENTS_DB: d1,
    })
    expect(isCrmEmailDomainHandle(db)).toBe(false)
  })

  test('binding + default flags yields the dual-write handle over D1 authority', () => {
    const db = makeCrmEmailDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      OPENAGENTS_DB: d1,
    })
    expect(isCrmEmailDomainHandle(db)).toBe(true)
    expect(crmEmailAuthorityDb(db)).toBe(d1)
  })
})

describe('publicSafeRefs (PII discipline)', () => {
  test('email-valued keys are hashed; opaque ids pass through', async () => {
    const refs = await publicSafeRefs(['sub_123', 'person@example.com', 42])
    expect(refs[0]).toBe('sub_123')
    expect(refs[1]).toMatch(/^sha256:[0-9a-f]{12}$/)
    expect(refs[1]).not.toContain('@')
    expect(refs[2]).toBe('42')
  })
})

const seededSqlite = () => {
  const sqlite = makeSqliteD1()
  sqlite.exec(`
    CREATE TABLE email_suppression_entries (
      id TEXT PRIMARY KEY,
      email TEXT NOT NULL,
      reason TEXT NOT NULL,
      scope TEXT NOT NULL DEFAULT 'marketing',
      active INTEGER NOT NULL DEFAULT 1,
      source_authority_ref TEXT,
      provider_event_id TEXT,
      note TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      archived_at TEXT
    );
  `)
  return sqlite
}

describe('mirrorCrmEmailRows (fail-soft dual-write)', () => {
  test('a bare D1Database is a no-op (fail-safe degradation)', async () => {
    const sqlite = seededSqlite()
    await expect(
      mirrorCrmEmailRows(sqlite.db, 'email_suppression_entries', 'id', ['a']),
    ).resolves.toBeUndefined()
    sqlite.close()
  })

  test('mirrors the resolved D1 row (never the caller intent)', async () => {
    const sqlite = seededSqlite()
    sqlite.exec(`
      INSERT INTO email_suppression_entries (id, email, reason, created_at, updated_at)
      VALUES ('sup_1', 'person@example.com', 'hard_bounce', 't0', 't0');
    `)
    const postgres = fakePostgres()
    const handle = makeCrmEmailDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await mirrorCrmEmailRows(handle, 'email_suppression_entries', 'id', [
      'sup_1',
    ])
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.table).toBe('email_suppression_entries')
    expect(postgres.upserts[0]?.rows[0]?.['reason']).toBe('hard_bounce')
    sqlite.close()
  })

  test('an unregistered key column is refused fail-soft (diagnostic, no throw)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const handle = makeCrmEmailDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres(),
      wait: () => Promise.resolve(),
    })
    await expect(
      mirrorCrmEmailRows(handle, 'email_suppression_entries', 'reason', ['x']),
    ).resolves.toBeUndefined()
    expect(logged[0]?.[0]).toBe('khala_sync_crm_dual_write_failed')
    sqlite.close()
  })

  test('mirror failure never throws AND its diagnostic hashes email-valued keys', async () => {
    const sqlite = seededSqlite()
    sqlite.exec(`
      INSERT INTO email_suppression_entries (id, email, reason, created_at, updated_at)
      VALUES ('sup_2', 'person@example.com', 'unsubscribe', 't0', 't0');
    `)
    const logged: Array<Logged> = []
    const handle = makeCrmEmailDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres({
        upsertRows: () => Promise.reject(new Error('postgres down')),
      }),
      wait: () => Promise.resolve(),
    })
    await expect(
      // Keyed by email here to prove the diagnostic-side hashing.
      mirrorCrmEmailRows(handle, 'email_suppression_entries', 'email', [
        'person@example.com',
      ]),
    ).resolves.toBeUndefined()
    expect(logged[0]?.[0]).toBe('khala_sync_crm_dual_write_failed')
    const refs = logged[0]?.[1].refs ?? []
    expect(refs[0]).toMatch(/^sha256:[0-9a-f]{12}$/)
    expect(JSON.stringify(logged)).not.toContain('person@example.com')
    sqlite.close()
  })

  test('dual-write off is a no-op even with a postgres store', async () => {
    const sqlite = seededSqlite()
    const postgres = fakePostgres()
    const handle = makeCrmEmailDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: false, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await mirrorCrmEmailRows(handle, 'email_suppression_entries', 'id', ['a'])
    expect(postgres.upserts).toHaveLength(0)
    sqlite.close()
  })
})

describe('crmEmailRead (flag routing — the suppression gate rides this)', () => {
  const d1 = {} as D1Database

  const handleWith = (
    reads: 'd1' | 'postgres' | 'compare',
    postgres: PostgresCrmEmailDomainStore,
    logged: Array<Logged>,
  ) =>
    makeCrmEmailDomainHandle({
      d1,
      flags: { dualWrite: true, reads },
      log: (event, fields) => logged.push([event, fields]),
      postgres,
      wait: () => Promise.resolve(),
    })

  test('d1 mode never touches postgres', async () => {
    const logged: Array<Logged> = []
    let touched = 0
    const handle = handleWith('d1', fakePostgres(), logged)
    const result = await crmEmailRead(
      handle,
      'suppression.read',
      [],
      () => Promise.resolve('from-d1'),
      () => {
        touched += 1
        return Promise.resolve('from-postgres')
      },
    )
    expect(result).toBe('from-d1')
    expect(touched).toBe(0)
  })

  test('a read without a postgres twin stays on D1 regardless of the flag', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('postgres', fakePostgres(), logged)
    const result = await crmEmailRead(handle, 'op', [], () =>
      Promise.resolve('authority'),
    )
    expect(result).toBe('authority')
    expect(logged).toEqual([])
  })

  test('postgres mode: bounded retry then D1 fallback with diagnostics', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('postgres', fakePostgres(), logged)
    let attempts = 0
    const result = await crmEmailRead(
      handle,
      'suppression.read',
      ['ref-1'],
      () => Promise.resolve('from-d1'),
      () => {
        attempts += 1
        return Promise.reject(new Error('boom'))
      },
    )
    expect(result).toBe('from-d1')
    expect(attempts).toBe(3)
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_crm_postgres_read_failed',
      'khala_sync_crm_postgres_read_failed',
      'khala_sync_crm_postgres_read_fallback',
    ])
  })

  test('compare mode: serves D1 and logs mismatches with PII-safe refs only', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('compare', fakePostgres(), logged)
    const same = await crmEmailRead(
      handle,
      'suppression.read',
      [],
      () => Promise.resolve({ allowed: false, reason: 'suppressed' }),
      () => Promise.resolve({ allowed: false, reason: 'suppressed' }),
    )
    expect(same).toEqual({ allowed: false, reason: 'suppressed' })
    expect(logged).toEqual([])

    const drifted = await crmEmailRead(
      handle,
      'suppression.read',
      ['person@example.com'],
      () => Promise.resolve({ allowed: false }),
      () => Promise.resolve({ allowed: true }),
    )
    // D1 stays authoritative: the caller NEVER sees the postgres answer.
    expect(drifted).toEqual({ allowed: false })
    expect(logged[0]?.[0]).toBe('khala_sync_crm_read_compare_mismatch')
    expect(JSON.stringify(logged)).not.toContain('person@example.com')
  })

  test('compare mode: a postgres fault never fails the read', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('compare', fakePostgres(), logged)
    const result = await crmEmailRead(
      handle,
      'op',
      [],
      () => Promise.resolve('from-d1'),
      () => Promise.reject(new Error('boom')),
    )
    expect(result).toBe('from-d1')
    expect(logged[0]?.[0]).toBe('khala_sync_crm_postgres_read_failed')
  })
})

describe('registry discipline', () => {
  test('covers all 36 domain tables with conflict keys inside key columns', () => {
    const tables = Object.keys(CRM_EMAIL_DOMAIN_TABLES)
    expect(tables).toHaveLength(36)
    for (const [table, spec] of Object.entries(CRM_EMAIL_DOMAIN_TABLES)) {
      expect(spec.keyColumns, table).toContain(spec.conflictKey)
      expect(spec.columns, table).toContain(spec.orderColumn)
      for (const keyColumn of spec.keyColumns) {
        expect(spec.columns, `${table}.${keyColumn}`).toContain(keyColumn)
      }
    }
  })

  test('stays in lock-step with the khala-sync-server backfill registry', async () => {
    const backfill = (await import(
      '../../../../../packages/khala-sync-server/src/crm-email-backfill.js'
    )) as {
      CRM_EMAIL_TABLE_SPECS: Record<
        string,
        { columns: ReadonlyArray<string>; conflictKey: string }
      >
    }
    expect(Object.keys(backfill.CRM_EMAIL_TABLE_SPECS).sort()).toEqual(
      Object.keys(CRM_EMAIL_DOMAIN_TABLES).sort(),
    )
    for (const [table, spec] of Object.entries(CRM_EMAIL_DOMAIN_TABLES)) {
      const twin = backfill.CRM_EMAIL_TABLE_SPECS[table]!
      expect(twin.columns, table).toEqual(spec.columns)
      expect(twin.conflictKey, table).toBe(spec.conflictKey)
    }
  })
})
