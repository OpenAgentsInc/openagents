// KS-8.6 (#8317): artanis domain seam — flags, fail-soft dual-write, and
// flag-routed reads (unit level; the cross-engine convergence proof lives in
// artanis-domain-repository.contract.test.ts).

import { describe, expect, test } from 'vitest'

import {
  ARTANIS_DOMAIN_TABLES,
  artanisAuthorityDb,
  artanisDomainFlagsFromEnv,
  artanisRead,
  isArtanisDomainHandle,
  makeArtanisDatabaseForEnv,
  makeArtanisDomainHandle,
  mirrorArtanisRows,
  type ArtanisDomainDiagnostic,
  type ArtanisDomainDiagnosticEvent,
  type ArtanisDomainRow,
  type PostgresArtanisDomainStore,
} from './artanis-domain-store'
import { makeSqliteD1 } from './test/sqlite-d1'

type Logged = readonly [ArtanisDomainDiagnosticEvent, ArtanisDomainDiagnostic]

const fakePostgres = (
  overrides: Partial<PostgresArtanisDomainStore> = {},
): PostgresArtanisDomainStore & {
  upserts: Array<{ table: string; rows: ReadonlyArray<ArtanisDomainRow> }>
} => {
  const upserts: Array<{
    table: string
    rows: ReadonlyArray<ArtanisDomainRow>
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

describe('artanisDomainFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1', () => {
    expect(artanisDomainFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('dual-write off values', () => {
    for (const value of ['0', 'off', 'false', 'disabled', 'no', ' OFF ']) {
      expect(
        artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('reads: postgres and compare route; typos fall back to d1', () => {
    expect(
      artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_READS: ' Compare ' })
        .reads,
    ).toBe('compare')
    expect(
      artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_READS: 'postgrse' })
        .reads,
    ).toBe('d1')
  })
})

describe('makeArtanisDatabaseForEnv', () => {
  const d1 = {} as D1Database

  test('no KHALA_SYNC_DB binding degrades to the plain D1 database', () => {
    const db = makeArtanisDatabaseForEnv({ OPENAGENTS_DB: d1 })
    expect(isArtanisDomainHandle(db)).toBe(false)
    expect(artanisAuthorityDb(db)).toBe(d1)
  })

  test('everything flagged off degrades to plain D1 even with a binding', () => {
    const db = makeArtanisDatabaseForEnv({
      KHALA_SYNC_ARTANIS_DUAL_WRITE: 'off',
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      OPENAGENTS_DB: d1,
    })
    expect(isArtanisDomainHandle(db)).toBe(false)
  })

  test('binding + default flags yields the dual-write handle over D1 authority', () => {
    const db = makeArtanisDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      OPENAGENTS_DB: d1,
    })
    expect(isArtanisDomainHandle(db)).toBe(true)
    expect(artanisAuthorityDb(db)).toBe(d1)
  })
})

describe('mirrorArtanisRows (fail-soft dual-write)', () => {
  const seededSqlite = () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(`
      CREATE TABLE artanis_owner_memory (
        memory_ref TEXT PRIMARY KEY,
        owner_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        role TEXT,
        note_category TEXT,
        body TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      INSERT INTO artanis_owner_memory
        (memory_ref, owner_id, kind, role, note_category, body, created_at)
      VALUES ('mem-1', 'owner-1', 'note', NULL, 'fact', 'body', '2026-07-04T00:00:00.000Z');
    `)
    return sqlite
  }

  test('plain D1Database: no-op (no seam, no throw)', async () => {
    const sqlite = seededSqlite()
    await mirrorArtanisRows(sqlite.db, 'artanis_owner_memory', 'memory_ref', [
      'mem-1',
    ])
    sqlite.close()
  })

  test('mirrors the resolved D1 row to Postgres on the same natural key', async () => {
    const sqlite = seededSqlite()
    const postgres = fakePostgres()
    const logged: Array<Logged> = []
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres,
    })

    await mirrorArtanisRows(handle, 'artanis_owner_memory', 'memory_ref', [
      'mem-1',
      'mem-missing',
    ])
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.table).toBe('artanis_owner_memory')
    expect(postgres.upserts[0]?.rows.map(row => row['memory_ref'])).toEqual([
      'mem-1',
    ])
    expect(logged).toEqual([])
    sqlite.close()
  })

  test('dual-write off: never touches Postgres', async () => {
    const sqlite = seededSqlite()
    const postgres = fakePostgres()
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: false, reads: 'd1' },
      postgres,
    })
    await mirrorArtanisRows(handle, 'artanis_owner_memory', 'memory_ref', [
      'mem-1',
    ])
    expect(postgres.upserts).toEqual([])
    sqlite.close()
  })

  test('a Postgres failure that persists across all retries NEVER throws — it retries then logs the dual-write diagnostic (2d46d808 fail-soft, #8409 follow-up)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const waited: Array<number> = []
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres({
        upsertRows: () => Promise.reject(new Error('pg down\nwith  detail')),
      }),
      wait: ms => {
        waited.push(ms)
        return Promise.resolve()
      },
    })

    await expect(
      mirrorArtanisRows(handle, 'artanis_owner_memory', 'memory_ref', [
        'mem-1',
      ]),
    ).resolves.toBeUndefined()
    // #8409 follow-up: 2 bounded retries (logged distinctly) before the
    // permanent-failure diagnostic — a transient blip that recovers within
    // the retry budget must never again silently drop this writer's own
    // column update the way the original bug did.
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_artanis_dual_write_retry',
      'khala_sync_artanis_dual_write_retry',
      'khala_sync_artanis_dual_write_failed',
    ])
    expect(waited).toEqual([100, 400])
    for (const entry of logged) {
      expect(entry[1].op).toBe('artanis_owner_memory')
      expect(entry[1].refs).toEqual(['mem-1'])
      // message is bounded and single-line
      expect(entry[1].messageSafe).toBe('pg down with detail')
    }
    sqlite.close()
  })

  test('a transient Postgres failure recovers on retry — no drift, no failure diagnostic (#8409 follow-up)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    let attempts = 0
    const postgres = fakePostgres({
      upsertRows: (table, rows) => {
        attempts += 1
        if (attempts === 1) {
          return Promise.reject(new Error('connect ETIMEDOUT'))
        }
        postgres.upserts.push({ rows, table })
        return Promise.resolve()
      },
    })
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres,
      wait: () => Promise.resolve(),
    })

    await expect(
      mirrorArtanisRows(handle, 'artanis_owner_memory', 'memory_ref', [
        'mem-1',
      ]),
    ).resolves.toBeUndefined()
    expect(attempts).toBe(2)
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.rows.map(row => row['memory_ref'])).toEqual([
      'mem-1',
    ])
    // Recovered within the retry budget: only the retry diagnostic fires,
    // never the permanent-failure one — this write is NOT lost.
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_artanis_dual_write_retry',
    ])
    sqlite.close()
  })

  test('an unregistered key column is refused (logged, never thrown)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres(),
    })
    await expect(
      mirrorArtanisRows(handle, 'artanis_owner_memory', 'body', ['x']),
    ).resolves.toBeUndefined()
    expect(logged[0]?.[0]).toBe('khala_sync_artanis_dual_write_failed')
    sqlite.close()
  })
})

describe('artanisRead (flag routing)', () => {
  const d1 = {} as D1Database
  const handleWith = (
    reads: 'd1' | 'postgres' | 'compare',
    postgres: PostgresArtanisDomainStore,
    logged: Array<Logged>,
  ) =>
    makeArtanisDomainHandle({
      d1,
      flags: { dualWrite: true, reads },
      log: (event, fields) => logged.push([event, fields]),
      postgres,
      wait: () => Promise.resolve(),
    })

  test('plain D1Database always reads D1', async () => {
    const result = await artanisRead(
      d1,
      'op',
      [],
      () => Promise.resolve('from-d1'),
      () => Promise.resolve('from-postgres'),
    )
    expect(result).toBe('from-d1')
  })

  test("reads: 'd1' (default) never calls the Postgres read", async () => {
    const logged: Array<Logged> = []
    let postgresCalls = 0
    const result = await artanisRead(
      handleWith('d1', fakePostgres(), logged),
      'op',
      [],
      () => Promise.resolve('from-d1'),
      () => {
        postgresCalls += 1
        return Promise.resolve('from-postgres')
      },
    )
    expect(result).toBe('from-d1')
    expect(postgresCalls).toBe(0)
  })

  test('a read with no Postgres twin stays on D1 regardless of the flag', async () => {
    const logged: Array<Logged> = []
    const result = await artanisRead(
      handleWith('postgres', fakePostgres(), logged),
      'op',
      [],
      () => Promise.resolve('from-d1'),
    )
    expect(result).toBe('from-d1')
  })

  test("reads: 'postgres' serves Postgres when healthy", async () => {
    const logged: Array<Logged> = []
    const result = await artanisRead(
      handleWith('postgres', fakePostgres(), logged),
      'op',
      [],
      () => Promise.resolve('from-d1'),
      () => Promise.resolve('from-postgres'),
    )
    expect(result).toBe('from-postgres')
    expect(logged).toEqual([])
  })

  test("reads: 'postgres' retries with bounded backoff, then falls back to D1 with diagnostics", async () => {
    const logged: Array<Logged> = []
    let attempts = 0
    const result = await artanisRead(
      handleWith('postgres', fakePostgres(), logged),
      'op',
      ['ref-1'],
      () => Promise.resolve('from-d1'),
      () => {
        attempts += 1
        return Promise.reject(new Error(`boom ${attempts}`))
      },
    )
    expect(result).toBe('from-d1')
    expect(attempts).toBe(3) // initial + 2 bounded retries
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_artanis_postgres_read_failed',
      'khala_sync_artanis_postgres_read_failed',
      'khala_sync_artanis_postgres_read_fallback',
    ])
  })

  test("reads: 'compare' SERVES D1 and logs a mismatch diagnostic", async () => {
    const logged: Array<Logged> = []
    const result = await artanisRead(
      handleWith('compare', fakePostgres(), logged),
      'op',
      ['ref-1'],
      () => Promise.resolve({ a: 1, b: 2 }),
      () => Promise.resolve({ a: 1, b: 3 }),
    )
    expect(result).toEqual({ a: 1, b: 2 })
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_artanis_read_compare_mismatch',
    ])
  })

  test("reads: 'compare' is silent on equal results regardless of key order", async () => {
    const logged: Array<Logged> = []
    const result = await artanisRead(
      handleWith('compare', fakePostgres(), logged),
      'op',
      [],
      () => Promise.resolve({ a: 1, b: 2 }),
      () => Promise.resolve({ b: 2, a: 1 }),
    )
    expect(result).toEqual({ a: 1, b: 2 })
    expect(logged).toEqual([])
  })

  test("reads: 'compare' never fails the read when the Postgres side throws", async () => {
    const logged: Array<Logged> = []
    const result = await artanisRead(
      handleWith('compare', fakePostgres(), logged),
      'op',
      [],
      () => Promise.resolve('from-d1'),
      () => Promise.reject(new Error('pg down')),
    )
    expect(result).toBe('from-d1')
    expect(logged.map(([event]) => event)).toEqual([
      'khala_sync_artanis_postgres_read_failed',
    ])
  })
})

describe('registry', () => {
  test('covers all twenty artanis tables with natural keys among the key columns', () => {
    const tables = Object.keys(ARTANIS_DOMAIN_TABLES)
    expect(tables).toHaveLength(20)
    for (const [table, spec] of Object.entries(ARTANIS_DOMAIN_TABLES)) {
      expect(spec.keyColumns, table).toContain(spec.conflictKey)
      expect(spec.columns, table).toContain(spec.conflictKey)
      expect(spec.columns, table).toContain(spec.orderColumn)
    }
  })

  test('stays in lock-step with the khala-sync-server backfill registry', async () => {
    const backfill = (await import(
      '../../../../../packages/khala-sync-server/src/artanis-backfill.js'
    )) as {
      ARTANIS_TABLE_SPECS: Record<
        string,
        { columns: ReadonlyArray<string>; conflictKey: string }
      >
    }
    expect(Object.keys(backfill.ARTANIS_TABLE_SPECS).sort()).toEqual(
      Object.keys(ARTANIS_DOMAIN_TABLES).sort(),
    )
    for (const [table, spec] of Object.entries(ARTANIS_DOMAIN_TABLES)) {
      const twin = backfill.ARTANIS_TABLE_SPECS[table]!
      expect(twin.columns, table).toEqual(spec.columns)
      expect(twin.conflictKey, table).toBe(spec.conflictKey)
    }
  })
})
