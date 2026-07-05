// KS-8.6 (#8317): artanis domain seam — flags, fail-soft dual-write, and
// flag-routed reads (unit level; the cross-engine convergence proof lives in
// artanis-domain-repository.contract.test.ts).
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_DOMAIN_TABLES,
  type ArtanisDomainDiagnostic,
  type ArtanisDomainDiagnosticEvent,
  type ArtanisDomainRow,
  type PostgresArtanisDomainStore,
  artanisAuthorityDb,
  artanisDomainFlagsFromEnv,
  artanisRead,
  isArtanisDomainHandle,
  makeArtanisDatabaseForEnv,
  makeArtanisDomainHandle,
  makePostgresArtanisDomainStore,
  mirrorArtanisRows,
} from './artanis-domain-store'
import {
  recordArtanisResponderComposeTick,
  recordArtanisResponderScanTick,
} from './artanis-responder-ticks'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import { makeSqliteD1 } from './test/sqlite-d1'

type Logged = readonly [ArtanisDomainDiagnosticEvent, ArtanisDomainDiagnostic]

const fakePostgres = (
  overrides: Partial<PostgresArtanisDomainStore> = {},
): PostgresArtanisDomainStore & {
  upserts: Array<{
    table: string
    rows: ReadonlyArray<ArtanisDomainRow>
    updateColumns: ReadonlyArray<string> | undefined
  }>
} => {
  const upserts: Array<{
    table: string
    rows: ReadonlyArray<ArtanisDomainRow>
    updateColumns: ReadonlyArray<string> | undefined
  }> = []
  return {
    selectLatestRows: () => Promise.resolve([]),
    selectRowsByKey: () => Promise.resolve([]),
    upsertRows: (table, rows, updateColumns) => {
      upserts.push({ rows, table, updateColumns })
      return Promise.resolve()
    },
    upserts,
    ...overrides,
  }
}

const capturePostgresStatements = () => {
  const statements: Array<{
    params: Array<unknown>
    text: string
  }> = []
  const client: KhalaSyncPushSqlClient = {
    end: () => Promise.resolve(),
    sql: {
      unsafe: (text: string, params: Array<unknown>) => {
        statements.push({ params, text })
        return Promise.resolve([])
      },
    } as unknown as KhalaSyncPushSqlClient['sql'],
  }
  return {
    statements,
    store: makePostgresArtanisDomainStore({
      acquireSql: () => Promise.resolve(client),
    }),
  }
}

const responderTickRow = (
  overrides: ArtanisDomainRow = {},
): ArtanisDomainRow => ({
  compose_blocked: 0,
  compose_considered: 3,
  compose_responded: 2,
  compose_skipped_reason: null,
  compose_state: 'ran',
  compose_tipped: 1,
  created_at: '2026-07-05T00:00:00.000Z',
  scan_blocked: 0,
  scan_proposed: 2,
  scan_scanned: 4,
  scan_skipped: 0,
  scan_skipped_reason: null,
  scan_state: 'ran',
  scheduled_at: '2026-07-05T00:00:00.000Z',
  tick_ref: 'receipt.artanis_responder.tick.20260705000000000Z',
  updated_at: '2026-07-05T00:00:00.000Z',
  ...overrides,
})

const createResponderTicksSqlite = () => {
  const sqlite = makeSqliteD1()
  sqlite.exec(`
    CREATE TABLE artanis_responder_ticks (
      tick_ref TEXT PRIMARY KEY,
      scheduled_at TEXT NOT NULL UNIQUE,
      scan_state TEXT NOT NULL DEFAULT 'pending',
      scan_scanned INTEGER NOT NULL DEFAULT 0,
      scan_proposed INTEGER NOT NULL DEFAULT 0,
      scan_blocked INTEGER NOT NULL DEFAULT 0,
      scan_skipped INTEGER NOT NULL DEFAULT 0,
      scan_skipped_reason TEXT,
      compose_state TEXT NOT NULL DEFAULT 'pending',
      compose_considered INTEGER NOT NULL DEFAULT 0,
      compose_responded INTEGER NOT NULL DEFAULT 0,
      compose_blocked INTEGER NOT NULL DEFAULT 0,
      compose_tipped INTEGER NOT NULL DEFAULT 0,
      compose_skipped_reason TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );
  `)
  return sqlite
}

const selectResponderTickRow = async (
  db: D1Database,
  scheduledAt: string,
): Promise<ArtanisDomainRow> => {
  const row = await db
    .prepare(
      `SELECT ${ARTANIS_DOMAIN_TABLES.artanis_responder_ticks.columns.join(', ')}
         FROM artanis_responder_ticks
        WHERE scheduled_at = ?`,
    )
    .bind(scheduledAt)
    .first<ArtanisDomainRow>()
  if (row === null) {
    throw new Error(`expected artanis_responder_ticks row ${scheduledAt}`)
  }
  return row
}

const statefulResponderTickPostgres = (): PostgresArtanisDomainStore & {
  row: (scheduledAt: string) => ArtanisDomainRow | undefined
} => {
  const rows = new Map<string, ArtanisDomainRow>()
  const spec = ARTANIS_DOMAIN_TABLES.artanis_responder_ticks
  const completeRow = (row: ArtanisDomainRow): ArtanisDomainRow =>
    Object.fromEntries(
      spec.columns.map(column => [column, row[column] ?? null]),
    )

  return {
    row: scheduledAt => rows.get(scheduledAt),
    selectLatestRows: () => Promise.resolve([]),
    selectRowsByKey: () => Promise.resolve([]),
    upsertRows: (table, upsertRows, updateColumns) => {
      if (table !== 'artanis_responder_ticks') {
        return Promise.reject(new Error(`unexpected table ${table}`))
      }
      const updateColumnList =
        updateColumns ??
        spec.columns.filter(column => column !== spec.conflictKey)
      for (const row of upsertRows) {
        const key = String(row[spec.conflictKey] ?? '')
        const current = rows.get(key)
        if (current === undefined) {
          rows.set(key, completeRow(row))
          continue
        }
        const next = { ...current }
        for (const column of updateColumnList) {
          next[column] = row[column] ?? null
        }
        rows.set(key, next)
      }
      return Promise.resolve()
    },
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
      artanisDomainFlagsFromEnv({ KHALA_SYNC_ARTANIS_READS: 'postgrse' }).reads,
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
    expect(postgres.upserts[0]?.updateColumns).toBeUndefined()
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

  test('a Postgres failure NEVER throws — it logs the dual-write diagnostic (2d46d808 fail-soft)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres({
        upsertRows: () => Promise.reject(new Error('pg down\nwith  detail')),
      }),
    })

    await expect(
      mirrorArtanisRows(handle, 'artanis_owner_memory', 'memory_ref', [
        'mem-1',
      ]),
    ).resolves.toBeUndefined()
    expect(logged).toHaveLength(1)
    expect(logged[0]?.[0]).toBe('khala_sync_artanis_dual_write_failed')
    expect(logged[0]?.[1].op).toBe('artanis_owner_memory')
    expect(logged[0]?.[1].refs).toEqual(['mem-1'])
    // message is bounded and single-line
    expect(logged[0]?.[1].messageSafe).toBe('pg down with detail')
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

  test('an unregistered scoped update column is refused through the fail-soft mirror', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const postgres = fakePostgres()
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres,
    })
    await expect(
      mirrorArtanisRows(
        handle,
        'artanis_owner_memory',
        'memory_ref',
        ['mem-1'],
        ['owner_id', 'not_a_column'],
      ),
    ).resolves.toBeUndefined()
    expect(logged[0]?.[0]).toBe('khala_sync_artanis_dual_write_failed')
    expect(postgres.upserts).toEqual([])
    sqlite.close()
  })

  test('scoped Postgres upserts insert all registry columns but only update the requested subset', async () => {
    const { statements, store } = capturePostgresStatements()
    await store.upsertRows(
      'artanis_responder_ticks',
      [responderTickRow()],
      ['scan_state', 'scan_scanned', 'updated_at'],
    )

    const statement = statements[0]
    expect(statement).toBeDefined()
    expect(statement?.params).toHaveLength(
      ARTANIS_DOMAIN_TABLES.artanis_responder_ticks.columns.length,
    )
    expect(statement?.text).toContain(
      `INSERT INTO artanis_responder_ticks (${ARTANIS_DOMAIN_TABLES.artanis_responder_ticks.columns.join(', ')})`,
    )
    expect(statement?.text).toContain(
      'ON CONFLICT (scheduled_at) DO UPDATE SET scan_state = EXCLUDED.scan_state, scan_scanned = EXCLUDED.scan_scanned, updated_at = EXCLUDED.updated_at',
    )
    expect(statement?.text).not.toContain(
      'compose_state = EXCLUDED.compose_state',
    )
  })

  test('scoped Postgres upserts reject unknown update columns before SQL text is emitted', async () => {
    const { statements, store } = capturePostgresStatements()
    await expect(
      store.upsertRows(
        'artanis_responder_ticks',
        [responderTickRow()],
        ['scan_state', 'scan_state; DROP TABLE artanis_responder_ticks'],
      ),
    ).rejects.toThrow(TypeError)
    expect(statements).toEqual([])
  })

  test('Postgres upserts without scoped columns still update all non-key columns', async () => {
    const { statements, store } = capturePostgresStatements()
    await store.upsertRows('artanis_owner_memory', [
      {
        body: 'body v2',
        created_at: '2026-07-04T00:00:00.000Z',
        kind: 'note',
        memory_ref: 'mem-1',
        note_category: 'fact',
        owner_id: 'owner-1',
        role: null,
      },
    ])

    const expectedUpdates = ARTANIS_DOMAIN_TABLES.artanis_owner_memory.columns
      .filter(
        column =>
          column !== ARTANIS_DOMAIN_TABLES.artanis_owner_memory.conflictKey,
      )
      .map(column => `${column} = EXCLUDED.${column}`)
      .join(', ')
    expect(statements[0]?.text).toContain(`DO UPDATE SET ${expectedUpdates}`)
  })

  test('responder tick scoped mirrors keep scan and compose columns through a delayed stale snapshot replay', async () => {
    const sqlite = createResponderTicksSqlite()
    const postgres = statefulResponderTickPostgres()
    const handle = makeArtanisDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      postgres,
    })
    const nowIso = '2026-07-05T00:01:00.000Z'
    const composeUpdateColumns = [
      'compose_state',
      'compose_considered',
      'compose_responded',
      'compose_blocked',
      'compose_tipped',
      'compose_skipped_reason',
      'updated_at',
    ] as const

    await recordArtanisResponderComposeTick(handle, {
      nowIso,
      outcome: {
        blocked: 0,
        considered: 3,
        responded: 2,
        skippedReason: null,
        tipped: 1,
      },
    })
    const staleComposeSnapshot = await selectResponderTickRow(sqlite.db, nowIso)
    const afterComposeInsert = postgres.row(nowIso)
    if (afterComposeInsert === undefined) {
      throw new Error('expected responder tick Postgres row after compose')
    }
    for (const column of ARTANIS_DOMAIN_TABLES.artanis_responder_ticks
      .columns) {
      expect(Object.hasOwn(afterComposeInsert, column)).toBe(true)
    }
    expect(afterComposeInsert['scan_state']).toBe('pending')
    expect(afterComposeInsert['compose_state']).toBe('ran')

    await postgres.upsertRows(
      'artanis_responder_ticks',
      [staleComposeSnapshot],
      composeUpdateColumns,
    )
    expect(postgres.row(nowIso)).toEqual(afterComposeInsert)

    await recordArtanisResponderScanTick(handle, {
      nowIso,
      outcome: {
        blocked: 0,
        proposed: 2,
        scanned: 4,
        skipped: 0,
        skippedReason: null,
      },
    })
    expect(postgres.row(nowIso)?.['scan_state']).toBe('ran')
    expect(postgres.row(nowIso)?.['compose_state']).toBe('ran')

    await postgres.upsertRows(
      'artanis_responder_ticks',
      [staleComposeSnapshot],
      composeUpdateColumns,
    )
    expect(postgres.row(nowIso)?.['scan_state']).toBe('ran')
    expect(postgres.row(nowIso)?.['compose_state']).toBe('ran')
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
    const backfill =
      (await import('../../../../../packages/khala-sync-server/src/artanis-backfill.js')) as {
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
