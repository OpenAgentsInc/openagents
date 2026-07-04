// KS-8.12 (#8323): sites content store — flags, statement classifier, and
// mirroring-database behavior.
//
// Load-bearing properties:
//   * flags parse per convention (dual-write default ON, reads default
//     'd1', typos never fail open into an unproven read path);
//   * the statement classifier resolves the mirror KEY of every write
//     shape the sites modules actually issue (site_projects INSERT with
//     literal/NULL tuple items interleaved, builder INSERT OR IGNORE
//     dedupe inserts, UPDATEs keyed on the PK with binds in SET, the
//     PARENT-keyed deployment/session transitions `WHERE site_id = ?`,
//     and composite `WHERE id = ? AND site_id = ?` preferring the PK) and
//     refuses loudly (`unclassified-write`) rather than guessing;
//   * the mirroring database read-back-mirrors the EXACT post-write D1
//     rows (bounded parent-key fan-out included), skips dedupe-discarded
//     INSERT OR IGNORE ids, never fails a request on a Postgres error
//     (fail-soft + typed diagnostic), mirrors batch() members, and passes
//     non-scoped statements through untouched;
//   * compare mode shadow-reads scoped SELECTs, SERVES D1 always, and
//     logs mismatch/failed diagnostics with statement heads only;
//   * `sitesContentDatabaseForEnv` degrades to the RAW database when no
//     binding / flags off, and `reads=postgres` defers (compare + one
//     diagnostic) instead of serving an unproven path.

import { describe, expect, test } from 'vitest'

import {
  classifySitesContentStatement,
  makeDualWriteSitesContentWriteStore,
  makeSitesContentMirror,
  makeSitesContentMirroringDatabase,
  resolveSitesContentKey,
  sitesContentDatabaseForEnv,
  sitesContentFlagsFromEnv,
  toPostgresPlaceholders,
  type PostgresSitesContentStore,
  type SitesContentDiagnostic,
  type SitesContentDiagnosticEvent,
  type SitesContentRow,
  type SitesContentTable,
} from './sites-content-store'
import { makeSqliteD1, SITES_CONTENT_D1_SCHEMA } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

type LoggedDiagnostic = Readonly<{
  event: SitesContentDiagnosticEvent
  fields: SitesContentDiagnostic
}>

const makeLogCapture = () => {
  const events: Array<LoggedDiagnostic> = []
  return {
    events,
    log: (
      event: SitesContentDiagnosticEvent,
      fields: SitesContentDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

type CapturedUpsert = Readonly<{
  table: SitesContentTable
  rows: ReadonlyArray<SitesContentRow>
}>

const makeFakePostgresStore = (
  options: Readonly<{
    failUpserts?: boolean
    queryRows?: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }> = {},
) => {
  const upserts: Array<CapturedUpsert> = []
  const store: PostgresSitesContentStore = {
    queryRows:
      options.queryRows ?? (() => Promise.reject(new Error('no query fake'))),
    upsertRows: (table, rows) => {
      if (options.failUpserts === true) {
        return Promise.reject(new Error('simulated postgres outage'))
      }
      upserts.push({ rows, table })
      return Promise.resolve(rows.length)
    },
  }
  return { store, upserts }
}

const T0 = '2026-07-04T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('sitesContentFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1', () => {
    expect(sitesContentFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('off values disable dual-write', () => {
    for (const value of ['off', '0', 'false', 'disabled', 'NO']) {
      expect(
        sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('read routing values parse; typos fall back to d1', () => {
    expect(
      sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_READS: 'POSTGRES' }).reads,
    ).toBe('postgres')
    expect(
      sitesContentFlagsFromEnv({ KHALA_SYNC_SITES_READS: 'postgress' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Statement classifier — pinned against the modules' real write shapes
// ---------------------------------------------------------------------------

describe('classifySitesContentStatement', () => {
  test('site_projects INSERT with interleaved literals/NULLs resolves the id bind (sites.ts)', () => {
    const classified = classifySitesContentStatement(
      `INSERT INTO site_projects
         (id,
          software_order_id,
          owner_user_id,
          team_id,
          project_id,
          slug,
          title,
          prompt,
          status,
          access_mode,
          visibility,
          source_repository_provider,
          source_repository_owner,
          source_repository_name,
          source_repository_ref,
          active_version_id,
          active_deployment_id,
          created_at,
          updated_at,
          archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL)`,
    )
    expect(classified).toEqual({
      keySource: { column: 'id', index: 0, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_projects',
    })
  })

  test('builder INSERT OR IGNORE dedupe insert resolves the id bind (sites-builder-sessions.ts)', () => {
    const classified = classifySitesContentStatement(
      `INSERT OR IGNORE INTO site_builder_messages (
         id,
         idempotency_key,
         session_id,
         sequence,
         actor_kind,
         visibility,
         body,
         metadata_json,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    expect(classified).toEqual({
      keySource: { column: 'id', index: 0, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_builder_messages',
    })
  })

  test('UPDATE keyed on the PK counts SET binds before the WHERE bind (sites.ts activate)', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_projects
          SET active_version_id = ?,
              active_deployment_id = ?,
              updated_at = ?
        WHERE id = ?
          AND archived_at IS NULL`,
    )
    expect(classified).toEqual({
      keySource: { column: 'id', index: 3, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_projects',
    })
    if (classified.kind !== 'mirrored-write') throw new Error('unreachable')
    expect(
      resolveSitesContentKey(classified.keySource, [
        'version_1',
        'deployment_1',
        T0,
        'site_1',
      ]),
    ).toBe('site_1')
  })

  test('parent-keyed deployment transition resolves the site_id secondary key (rollback/disable)', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_deployments
          SET status = 'rolled_back',
              rolled_back_at = ?,
              updated_at = ?
        WHERE site_id = ?
          AND status = 'active'`,
    )
    expect(classified).toEqual({
      keySource: { column: 'site_id', index: 2, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_deployments',
    })
  })

  test('parent-keyed session archival resolves site_id (site-library.ts)', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_builder_sessions
          SET status = 'archived',
              archived_at = ?,
              updated_at = ?
        WHERE site_id = ?
          AND archived_at IS NULL`,
    )
    expect(classified).toEqual({
      keySource: { column: 'site_id', index: 2, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_builder_sessions',
    })
  })

  test('composite WHERE id = ? AND site_id = ? prefers the PK (adjutant-run-lifecycle.ts)', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_deployments
          SET status = 'active',
              activated_at = COALESCE(activated_at, ?),
              updated_at = ?
        WHERE id = ?
          AND site_id = ?`,
    )
    expect(classified).toEqual({
      keySource: { column: 'id', index: 2, kind: 'bind' },
      kind: 'mirrored-write',
      table: 'site_deployments',
    })
  })

  test('a quoted-literal key resolves without binds', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_projects SET updated_at = ? WHERE id = 'site_lit''eral'`,
    )
    expect(classified).toEqual({
      keySource: { column: 'id', kind: 'literal', value: "site_lit'eral" },
      kind: 'mirrored-write',
      table: 'site_projects',
    })
    if (classified.kind !== 'mirrored-write') throw new Error('unreachable')
    expect(resolveSitesContentKey(classified.keySource, [T0])).toBe(
      "site_lit'eral",
    )
  })

  test('column matching is word-bounded: version_id never matches id', () => {
    const classified = classifySitesContentStatement(
      `UPDATE site_versions SET build_status = ? WHERE version_id = ?`,
    )
    // No `id` or `site_id` equality in the WHERE clause → refuse loudly.
    expect(classified).toEqual({
      kind: 'unclassified-write',
      table: 'site_versions',
    })
  })

  test('a DELETE on a scoped table is a loud unclassified write', () => {
    expect(
      classifySitesContentStatement(`DELETE FROM site_events WHERE id = ?`),
    ).toEqual({ kind: 'unclassified-write', table: 'site_events' })
  })

  test('writes to non-scoped tables pass through', () => {
    expect(
      classifySitesContentStatement(
        `INSERT INTO site_environment_values (id, site_id, key, kind) VALUES (?, ?, ?, ?)`,
      ),
    ).toEqual({ kind: 'passthrough' })
    expect(
      classifySitesContentStatement(
        `UPDATE software_orders SET status = ? WHERE id = ?`,
      ),
    ).toEqual({ kind: 'passthrough' })
  })

  test('SELECTs over only scoped tables are comparable; mixed refs pass through', () => {
    expect(
      classifySitesContentStatement(
        `SELECT * FROM site_versions WHERE site_id = ? ORDER BY created_at DESC`,
      ),
    ).toEqual({ kind: 'comparable-select' })
    expect(
      classifySitesContentStatement(
        `SELECT p.id FROM site_projects p JOIN software_orders o ON o.id = p.software_order_id`,
      ),
    ).toEqual({ kind: 'passthrough' })
  })
})

describe('toPostgresPlaceholders', () => {
  test('converts ? to $n outside string literals only', () => {
    expect(
      toPostgresPlaceholders(
        `SELECT * FROM site_events WHERE site_id = ? AND type = 'q?mark' AND created_at > ?`,
      ),
    ).toBe(
      `SELECT * FROM site_events WHERE site_id = $1 AND type = 'q?mark' AND created_at > $2`,
    )
  })
})

// ---------------------------------------------------------------------------
// Mirroring database behavior (real SQLite as the D1 authority)
// ---------------------------------------------------------------------------

const grantRow = (n: number): ReadonlyArray<unknown> => [
  `grant_${n}`,
  'site_1',
  'user',
  `user_${n}`,
  'viewer',
  T0,
]

const GRANT_INSERT = `INSERT INTO site_access_grants
  (id, site_id, principal_kind, principal_ref, role, created_at, revoked_at)
  VALUES (?, ?, ?, ?, ?, ?, NULL)`

const makeMirroringHarness = (
  options: Readonly<{
    failUpserts?: boolean
    compare?: boolean
    queryRows?: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }> = {},
) => {
  const sqlite = makeSqliteD1()
  sqlite.exec(SITES_CONTENT_D1_SCHEMA)
  const capture = makeLogCapture()
  const fake = makeFakePostgresStore(options)
  const db = makeSitesContentMirroringDatabase({
    compareStore: options.compare === true ? fake.store : undefined,
    db: sqlite.db,
    log: capture.log,
    mirror: makeSitesContentMirror({
      db: sqlite.db,
      log: capture.log,
      postgres: fake.store,
    }),
  })
  return { capture, db, fake, sqlite }
}

describe('makeSitesContentMirroringDatabase', () => {
  test('a scoped INSERT read-back-mirrors the exact post-write D1 row', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    await db.prepare(GRANT_INSERT).bind(...grantRow(1)).run()
    expect(capture.events).toEqual([])
    expect(fake.upserts).toHaveLength(1)
    expect(fake.upserts[0]?.table).toBe('site_access_grants')
    expect(fake.upserts[0]?.rows[0]?.['id']).toBe('grant_1')
    expect(fake.upserts[0]?.rows[0]?.['principal_ref']).toBe('user_1')
    sqlite.close()
  })

  test('INSERT OR IGNORE dedupe: the discarded id mirrors zero rows (no phantom)', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    const insert = `INSERT OR IGNORE INTO site_builder_sessions (
        id, idempotency_key, site_id, order_id, workroom_id, owner_user_id,
        customer_user_id, created_by_actor_ref, status, prompt_summary,
        source_site_version_id, source_revision_id, active_preview_id,
        active_artifact_id, metadata_json, created_at, updated_at, archived_at
      ) VALUES (?, ?, ?, NULL, NULL, ?, NULL, ?, ?, ?, NULL, NULL, NULL, NULL, '{}', ?, ?, NULL)`
    await db
      .prepare(insert)
      .bind('session_1', 'dedupe-key', 'site_1', 'user_1', 'agent_raynor', 'draft', 'p', T0, T0)
      .run()
    // Same idempotency key under a NEW id: D1 discards the row; the mirror
    // reads back zero rows for session_2 and must not invent one.
    await db
      .prepare(insert)
      .bind('session_2', 'dedupe-key', 'site_1', 'user_1', 'agent_raynor', 'draft', 'p', T0, T0)
      .run()
    expect(capture.events).toEqual([])
    const mirrored = fake.upserts.filter(
      entry => entry.table === 'site_builder_sessions',
    )
    expect(mirrored).toHaveLength(2)
    expect(mirrored[0]?.rows.map(row => row['id'])).toEqual(['session_1'])
    expect(mirrored[1]?.rows).toEqual([])
    sqlite.close()
  })

  test('a parent-keyed transition mirrors ALL the parent rows (bounded fan-out)', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    const insertDeployment = `INSERT INTO site_deployments
        (id, site_id, version_id, slug, url, runtime_kind, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'workers_for_platforms', ?, ?, ?)`
    await db
      .prepare(insertDeployment)
      .bind('deploy_1', 'site_1', 'version_1', 's', 'https://s', 'active', T0, T0)
      .run()
    await db
      .prepare(insertDeployment)
      .bind('deploy_2', 'site_1', 'version_2', 's', 'https://s', 'queued', T0, T0)
      .run()
    fake.upserts.length = 0

    await db
      .prepare(
        `UPDATE site_deployments
            SET status = 'disabled', disabled_at = ?, updated_at = ?
          WHERE site_id = ?
            AND status = 'active'`,
      )
      .bind(T0, T0, 'site_1')
      .run()
    expect(capture.events).toEqual([])
    expect(fake.upserts).toHaveLength(1)
    const rows = [...(fake.upserts[0]?.rows ?? [])].sort((a, b) =>
      String(a['id']).localeCompare(String(b['id'])),
    )
    expect(rows.map(row => [row['id'], row['status']])).toEqual([
      ['deploy_1', 'disabled'],
      ['deploy_2', 'queued'],
    ])
    sqlite.close()
  })

  test('batch() members mirror after the batch succeeds', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    await db.batch([
      db.prepare(GRANT_INSERT).bind(...grantRow(2)),
      db.prepare(GRANT_INSERT).bind(...grantRow(3)),
    ])
    expect(capture.events).toEqual([])
    expect(
      fake.upserts.flatMap(entry => entry.rows.map(row => row['id'])),
    ).toEqual(['grant_2', 'grant_3'])
    sqlite.close()
  })

  test('a Postgres outage never fails the write — fail-soft + typed diagnostic', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness({
      failUpserts: true,
    })
    await db.prepare(GRANT_INSERT).bind(...grantRow(4)).run()
    const persisted = await sqlite.db
      .prepare(`SELECT id FROM site_access_grants WHERE id = 'grant_4'`)
      .first<{ id: string }>()
    expect(persisted?.id).toBe('grant_4')
    expect(fake.upserts).toEqual([])
    expect(capture.events).toHaveLength(1)
    expect(capture.events[0]?.event).toBe('khala_sync_sites_dual_write_failed')
    expect(capture.events[0]?.fields.refs).toEqual(['grant_4'])
    // Public safety: no row values in the diagnostic.
    expect(capture.events[0]?.fields.messageSafe).not.toContain('user_')
    sqlite.close()
  })

  test('an unclassifiable scoped write logs the loud diagnostic and still succeeds', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    await db.prepare(GRANT_INSERT).bind(...grantRow(5)).run()
    fake.upserts.length = 0
    await db
      .prepare(`DELETE FROM site_access_grants WHERE id = ?`)
      .bind('grant_5')
      .run()
    expect(fake.upserts).toEqual([])
    expect(
      capture.events.map(entry => entry.event),
    ).toEqual(['khala_sync_sites_write_unclassified'])
    sqlite.close()
  })

  test('non-scoped statements pass through untouched (no mirror, no diagnostics)', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness()
    sqlite.exec(`CREATE TABLE unrelated (id TEXT PRIMARY KEY, v TEXT)`)
    await db
      .prepare(`INSERT INTO unrelated (id, v) VALUES (?, ?)`)
      .bind('u1', 'x')
      .run()
    expect(capture.events).toEqual([])
    expect(fake.upserts).toEqual([])
    sqlite.close()
  })

  test('compare mode shadow-reads scoped SELECTs, serves D1, and logs mismatches', async () => {
    const { capture, db, fake, sqlite } = makeMirroringHarness({
      compare: true,
      queryRows: () => Promise.resolve([]),
    })
    await db.prepare(GRANT_INSERT).bind(...grantRow(6)).run()
    fake.upserts.length = 0
    capture.events.length = 0

    const served = await db
      .prepare(`SELECT * FROM site_access_grants WHERE site_id = ?`)
      .bind('site_1')
      .all<Record<string, unknown>>()
    // D1 rows are always served even though the Postgres twin disagreed.
    expect(served.results?.map(row => row['id'])).toEqual(['grant_6'])
    expect(capture.events.map(entry => entry.event)).toEqual([
      'khala_sync_sites_read_compare_mismatch',
    ])
    // Statement heads only — never row values.
    expect(capture.events[0]?.fields.op).toContain('SELECT')
    sqlite.close()
  })

  test('a compare-read failure logs and never breaks the D1 read', async () => {
    const { capture, db, sqlite } = makeMirroringHarness({
      compare: true,
      queryRows: () => Promise.reject(new Error('twin down')),
    })
    const served = await db
      .prepare(`SELECT * FROM site_access_grants WHERE site_id = ?`)
      .bind('site_1')
      .all()
    expect(served.results).toEqual([])
    expect(capture.events.map(entry => entry.event)).toEqual([
      'khala_sync_sites_read_compare_failed',
    ])
    sqlite.close()
  })
})

// ---------------------------------------------------------------------------
// Dual-write row-seam wrapper
// ---------------------------------------------------------------------------

describe('makeDualWriteSitesContentWriteStore', () => {
  const d1Store = () => {
    const upserts: Array<CapturedUpsert> = []
    return {
      store: {
        upsertRows: (
          table: SitesContentTable,
          rows: ReadonlyArray<SitesContentRow>,
        ) => {
          upserts.push({ rows, table })
          return Promise.resolve(rows.length)
        },
      },
      upserts,
    }
  }

  test('mirror failure never fails the authoritative write', async () => {
    const d1 = d1Store()
    const capture = makeLogCapture()
    const store = makeDualWriteSitesContentWriteStore({
      d1: d1.store,
      flags: { dualWrite: true, reads: 'd1' },
      log: capture.log,
      postgres: makeFakePostgresStore({ failUpserts: true }).store,
    })
    const outcome = await store.upsertRows('site_events', [
      { id: 'event_1' },
    ])
    expect(outcome).toBe(1)
    expect(d1.upserts).toHaveLength(1)
    expect(capture.events.map(entry => entry.event)).toEqual([
      'khala_sync_sites_dual_write_failed',
    ])
  })

  test('dual-write off or missing binding returns the raw D1 store', () => {
    const d1 = d1Store()
    expect(
      makeDualWriteSitesContentWriteStore({
        d1: d1.store,
        flags: { dualWrite: false, reads: 'd1' },
        postgres: makeFakePostgresStore().store,
      }),
    ).toBe(d1.store)
    expect(
      makeDualWriteSitesContentWriteStore({
        d1: d1.store,
        flags: { dualWrite: true, reads: 'd1' },
        postgres: undefined,
      }),
    ).toBe(d1.store)
  })
})

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

describe('sitesContentDatabaseForEnv', () => {
  test('no KHALA_SYNC_DB binding → the raw database (zero overhead)', () => {
    const sqlite = makeSqliteD1()
    const db = sitesContentDatabaseForEnv({ OPENAGENTS_DB: sqlite.db })
    expect(db).toBe(sqlite.db)
    sqlite.close()
  })

  test('dual-write off and reads d1 → the raw database even with a binding', () => {
    const sqlite = makeSqliteD1()
    const db = sitesContentDatabaseForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://example' },
        KHALA_SYNC_SITES_DUAL_WRITE: 'off',
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: () => Promise.reject(new Error('never')) },
    )
    expect(db).toBe(sqlite.db)
    sqlite.close()
  })

  test('reads=postgres defers: one diagnostic, compare behavior, D1 served', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SITES_CONTENT_D1_SCHEMA)
    const capture = makeLogCapture()
    const db = sitesContentDatabaseForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://example' },
        KHALA_SYNC_SITES_READS: 'postgres',
        OPENAGENTS_DB: sqlite.db,
      },
      {
        log: capture.log,
        makeSqlClient: () => Promise.reject(new Error('unreachable twin')),
      },
    )
    expect(
      capture.events.map(entry => entry.event),
    ).toEqual(['khala_sync_sites_postgres_reads_deferred'])
    // Scoped reads still serve D1 (the compare shadow fails softly).
    const served = await db
      .prepare(`SELECT * FROM site_projects WHERE id = ?`)
      .bind('site_none')
      .all()
    expect(served.results).toEqual([])
    sqlite.close()
  })

  test('dual-write on wraps the database and mirrors through the injected client factory', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SITES_CONTENT_D1_SCHEMA)
    const statements: Array<string> = []
    const db = sitesContentDatabaseForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://example' },
        OPENAGENTS_DB: sqlite.db,
      },
      {
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: {
              unsafe: (text: string) => {
                statements.push(text)
                return Promise.resolve([{ touched: 1 }])
              },
            } as never,
          }),
      },
    )
    expect(db).not.toBe(sqlite.db)
    await db
      .prepare(GRANT_INSERT)
      .bind(...grantRow(7))
      .run()
    expect(statements).toHaveLength(1)
    expect(statements[0]).toContain('INSERT INTO site_access_grants')
    expect(statements[0]).toContain('ON CONFLICT (id) DO UPDATE SET')
    sqlite.close()
  })
})
