// KS-8.8 (#8319): treasury domain seam — flags, fail-soft dual-write,
// flag-routed reads, and the annotated-ledger-statement mirror (unit level;
// the cross-engine convergence proof lives in
// treasury-domain-repository.contract.test.ts).

import { describe, expect, test } from 'vitest'

import {
  TREASURY_DOMAIN_TABLES,
  isTreasuryDomainHandle,
  makeTreasuryDatabaseForEnv,
  makeTreasuryDomainHandle,
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  treasuryDomainFlagsFromEnv,
  treasuryRead,
  type PostgresTreasuryDomainStore,
  type TreasuryDomainDiagnostic,
  type TreasuryDomainDiagnosticEvent,
  type TreasuryDomainRow,
} from './treasury-domain-store'
import { makeSqliteD1 } from './test/sqlite-d1'

type Logged = readonly [TreasuryDomainDiagnosticEvent, TreasuryDomainDiagnostic]

const fakePostgres = (
  overrides: Partial<PostgresTreasuryDomainStore> = {},
): PostgresTreasuryDomainStore & {
  upserts: Array<{ table: string; rows: ReadonlyArray<TreasuryDomainRow> }>
} => {
  const upserts: Array<{
    table: string
    rows: ReadonlyArray<TreasuryDomainRow>
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

describe('treasuryDomainFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1 (read flips are epic-gated)', () => {
    expect(treasuryDomainFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('dual-write off values', () => {
    for (const value of ['0', 'off', 'false', 'disabled', 'no', ' OFF ']) {
      expect(
        treasuryDomainFlagsFromEnv({ KHALA_SYNC_TREASURY_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    }
    expect(
      treasuryDomainFlagsFromEnv({ KHALA_SYNC_TREASURY_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
  })

  test('reads: postgres and compare route; typos fall back to d1', () => {
    expect(
      treasuryDomainFlagsFromEnv({ KHALA_SYNC_TREASURY_READS: 'postgres' })
        .reads,
    ).toBe('postgres')
    expect(
      treasuryDomainFlagsFromEnv({ KHALA_SYNC_TREASURY_READS: ' Compare ' })
        .reads,
    ).toBe('compare')
    expect(
      treasuryDomainFlagsFromEnv({ KHALA_SYNC_TREASURY_READS: 'postgrse' })
        .reads,
    ).toBe('d1')
  })
})

describe('makeTreasuryDatabaseForEnv', () => {
  const d1 = {} as D1Database

  test('no KHALA_SYNC_DB binding degrades to the plain D1 database', () => {
    const db = makeTreasuryDatabaseForEnv({ OPENAGENTS_DB: d1 })
    expect(isTreasuryDomainHandle(db)).toBe(false)
    expect(treasuryAuthorityDb(db)).toBe(d1)
  })

  test('everything flagged off degrades to plain D1 even with a binding', () => {
    const db = makeTreasuryDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      KHALA_SYNC_TREASURY_DUAL_WRITE: 'off',
      OPENAGENTS_DB: d1,
    })
    expect(isTreasuryDomainHandle(db)).toBe(false)
  })

  test('binding + default flags yields the dual-write handle over D1 authority', () => {
    const db = makeTreasuryDatabaseForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://example' },
      OPENAGENTS_DB: d1,
    })
    expect(isTreasuryDomainHandle(db)).toBe(true)
    expect(treasuryAuthorityDb(db)).toBe(d1)
  })
})

const seededSqlite = () => {
  const sqlite = makeSqliteD1()
  // CFG-4 (#8519): the mirror fixture uses forum_receipts — the credits
  // tables (agent_balances & co.) left this registry for the Postgres-only
  // payments ledger.
  sqlite.exec(`
    CREATE TABLE forum_receipts (
      id TEXT PRIMARY KEY,
      receipt_ref TEXT NOT NULL,
      action_kind TEXT NOT NULL DEFAULT 'tip',
      target_forum_id TEXT,
      target_topic_id TEXT,
      target_post_id TEXT,
      amount_asset TEXT NOT NULL DEFAULT 'credits',
      amount_value INTEGER NOT NULL DEFAULT 0,
      recipient_actor_ref TEXT,
      redacted_payment_ref TEXT,
      public_projection_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL,
      archived_at TEXT
    );
  `)
  return sqlite
}

describe('mirrorTreasuryRows (fail-soft dual-write)', () => {
  test('a bare D1Database is a no-op (fail-safe degradation)', async () => {
    const sqlite = seededSqlite()
    await expect(
      mirrorTreasuryRows(sqlite.db, 'forum_receipts', 'id', ['a']),
    ).resolves.toBeUndefined()
    sqlite.close()
  })

  test('mirrors the resolved D1 row (never the caller intent)', async () => {
    const sqlite = seededSqlite()
    sqlite.exec(`
      INSERT INTO forum_receipts (id, receipt_ref, amount_value, created_at)
      VALUES ('receipt.a', 'receipt.public.a', 21000, 't0');
    `)
    const postgres = fakePostgres()
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await mirrorTreasuryRows(handle, 'forum_receipts', 'id', [
      'receipt.a',
    ])
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.table).toBe('forum_receipts')
    expect(postgres.upserts[0]?.rows[0]?.['amount_value']).toBe(21000)
    sqlite.close()
  })

  test('an unregistered key column is refused fail-soft (diagnostic, no throw)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres(),
      wait: () => Promise.resolve(),
    })
    await expect(
      mirrorTreasuryRows(handle, 'forum_receipts', 'amount_value', [1]),
    ).resolves.toBeUndefined()
    expect(logged[0]?.[0]).toBe('khala_sync_treasury_dual_write_failed')
    sqlite.close()
  })

  test('dual-write off is a no-op even with a postgres store', async () => {
    const sqlite = seededSqlite()
    const postgres = fakePostgres()
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: false, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await mirrorTreasuryRows(handle, 'forum_receipts', 'id', ['a'])
    expect(postgres.upserts).toHaveLength(0)
    sqlite.close()
  })
})

describe('treasuryRead (flag routing)', () => {
  const d1 = {} as D1Database

  const handleWith = (
    reads: 'd1' | 'postgres' | 'compare',
    postgres: PostgresTreasuryDomainStore,
    logged: Array<Logged>,
  ) =>
    makeTreasuryDomainHandle({
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
    const result = await treasuryRead(
      handle,
      'op',
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

  test('a read without a postgres twin stays on D1 regardless of the flag (dispatcher rule)', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('postgres', fakePostgres(), logged)
    const result = await treasuryRead(handle, 'op', [], () =>
      Promise.resolve('authority'),
    )
    expect(result).toBe('authority')
    expect(logged).toEqual([])
  })

  test('postgres mode: bounded retry then D1 fallback with diagnostics', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('postgres', fakePostgres(), logged)
    let attempts = 0
    const result = await treasuryRead(
      handle,
      'op',
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
      'khala_sync_treasury_postgres_read_failed',
      'khala_sync_treasury_postgres_read_failed',
      'khala_sync_treasury_postgres_read_fallback',
    ])
  })

  test('compare mode: serves D1 and logs mismatches only', async () => {
    const logged: Array<Logged> = []
    const handle = handleWith('compare', fakePostgres(), logged)
    const same = await treasuryRead(
      handle,
      'op',
      [],
      () => Promise.resolve({ total: '21000' }),
      () => Promise.resolve({ total: '21000' }),
    )
    expect(same).toEqual({ total: '21000' })
    expect(logged).toEqual([])

    const drifted = await treasuryRead(
      handle,
      'op',
      ['ref-1'],
      () => Promise.resolve({ total: '21000' }),
      () => Promise.resolve({ total: '21001' }),
    )
    expect(drifted).toEqual({ total: '21000' })
    expect(logged[0]?.[0]).toBe('khala_sync_treasury_read_compare_mismatch')
  })
})

// CFG-4 (#8519): the 'runLedgerStatements (annotated mirror)' suite was
// DELETED with the machinery it tested — ledger statements no longer carry
// mirror annotations and `runLedgerStatements` executes on the Postgres-only
// `PaymentsLedgerDb` (see payments-ledger-postgres.contract.test.ts).

describe('registry discipline', () => {
  // Was 27 until commit 87e6992d1e ("refactor(cleanup): remove unarmed MPP
  // chat endpoint") intentionally dropped `mpp_lightning_replay` and
  // `mpp_spt_replay`; 25 until CFG-4 (#8519) removed `agent_balances`,
  // `labor_escrows`, and `labor_escrow_receipts` — those are
  // Postgres-AUTHORITATIVE via the payments ledger and must never be
  // mirrored from D1 again. The lock-step test below pins the exact
  // relationship with the khala-sync-server backfill registry.
  test('covers all 22 domain tables with conflict keys inside key columns', () => {
    const tables = Object.keys(TREASURY_DOMAIN_TABLES)
    expect(tables).toHaveLength(22)
    for (const [table, spec] of Object.entries(TREASURY_DOMAIN_TABLES)) {
      expect(spec.keyColumns, table).toContain(spec.conflictKey)
      for (const keyColumn of spec.keyColumns) {
        expect(spec.columns, `${table}.${keyColumn}`).toContain(keyColumn)
      }
    }
  })

  test('stays in lock-step with the khala-sync-server backfill registry', async () => {
    const backfill = (await import(
      '../../../../../packages/khala-sync-server/src/treasury-backfill.js'
    )) as {
      TREASURY_TABLE_SPECS: Record<
        string,
        { columns: ReadonlyArray<string>; conflictKey: string }
      >
    }
    // CFG-4 (#8519): the backfill registry deliberately KEEPS the three
    // hard-cut credits tables — they are needed for the one-time pre-cutover
    // converge sweep (and are excluded from routine sweeps in the script) —
    // so the relationship is worker registry + credits trio = backfill set.
    const cfg4HardCutTables = [
      'agent_balances',
      'labor_escrow_receipts',
      'labor_escrows',
    ]
    expect(Object.keys(backfill.TREASURY_TABLE_SPECS).sort()).toEqual(
      [...Object.keys(TREASURY_DOMAIN_TABLES), ...cfg4HardCutTables].sort(),
    )
    for (const [table, spec] of Object.entries(TREASURY_DOMAIN_TABLES)) {
      const twin = backfill.TREASURY_TABLE_SPECS[table]!
      expect(twin.columns, table).toEqual(spec.columns)
      expect(twin.conflictKey, table).toBe(spec.conflictKey)
    }
  })
})
