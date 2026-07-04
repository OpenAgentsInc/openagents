// KS-8.8 (#8319): treasury domain seam — flags, fail-soft dual-write,
// flag-routed reads, and the annotated-ledger-statement mirror (unit level;
// the cross-engine convergence proof lives in
// treasury-domain-repository.contract.test.ts).

import { describe, expect, test } from 'vitest'

import { runLedgerStatements, type LedgerStatement } from './payments-ledger'
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
    sumAgentBalancesMsat: () => Promise.resolve('0'),
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
  sqlite.exec(`
    CREATE TABLE agent_balances (
      actor_ref TEXT PRIMARY KEY,
      balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
      sweep_enabled INTEGER NOT NULL DEFAULT 1,
      sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
      send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
      receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      held_msat INTEGER NOT NULL DEFAULT 0,
      usd_credit_msat INTEGER NOT NULL DEFAULT 0
    );
  `)
  return sqlite
}

describe('mirrorTreasuryRows (fail-soft dual-write)', () => {
  test('a bare D1Database is a no-op (fail-safe degradation)', async () => {
    const sqlite = seededSqlite()
    await expect(
      mirrorTreasuryRows(sqlite.db, 'agent_balances', 'actor_ref', ['a']),
    ).resolves.toBeUndefined()
    sqlite.close()
  })

  test('mirrors the resolved D1 row (never the caller intent)', async () => {
    const sqlite = seededSqlite()
    sqlite.exec(`
      INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
      VALUES ('actor.a', 21000, 't0', 't0');
    `)
    const postgres = fakePostgres()
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await mirrorTreasuryRows(handle, 'agent_balances', 'actor_ref', [
      'actor.a',
    ])
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.table).toBe('agent_balances')
    expect(postgres.upserts[0]?.rows[0]?.['balance_msat']).toBe(21000)
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
      mirrorTreasuryRows(handle, 'agent_balances', 'balance_msat', [1]),
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
    await mirrorTreasuryRows(handle, 'agent_balances', 'actor_ref', ['a'])
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

describe('runLedgerStatements (annotated mirror)', () => {
  const balanceStatement = (
    actorRef: string,
    amountMsat: number,
  ): LedgerStatement => ({
    mirror: {
      keyColumn: 'actor_ref',
      keys: [actorRef],
      table: 'agent_balances',
    },
    params: [actorRef, amountMsat, 't0', 't0'],
    sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
          VALUES (?, ?, ?, ?)
          ON CONFLICT (actor_ref) DO UPDATE SET balance_msat = excluded.balance_msat`,
  })

  test('bare D1: batch runs, no mirror machinery involved', async () => {
    const sqlite = seededSqlite()
    await runLedgerStatements(sqlite.db, [balanceStatement('actor.a', 1000)])
    const row = await sqlite.db
      .prepare(`SELECT balance_msat FROM agent_balances WHERE actor_ref = ?`)
      .bind('actor.a')
      .first<{ balance_msat: number }>()
    expect(row?.balance_msat).toBe(1000)
    sqlite.close()
  })

  test('seam handle: annotated rows mirror ONCE per key after the batch', async () => {
    const sqlite = seededSqlite()
    const postgres = fakePostgres()
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      postgres,
      wait: () => Promise.resolve(),
    })
    await runLedgerStatements(handle, [
      balanceStatement('actor.a', 1000),
      balanceStatement('actor.a', 2000),
      balanceStatement('actor.b', 3000),
      // Unannotated statements never mirror.
      { params: [], sql: 'SELECT 1' },
    ])
    expect(postgres.upserts).toHaveLength(1)
    expect(postgres.upserts[0]?.table).toBe('agent_balances')
    const mirrored = postgres.upserts[0]?.rows.map(row => [
      row['actor_ref'],
      row['balance_msat'],
    ])
    // The RESOLVED balances (post-batch), one row per deduped key.
    expect(mirrored).toEqual([
      ['actor.a', 2000],
      ['actor.b', 3000],
    ])
    sqlite.close()
  })

  test('mirror failure never fails the ledger batch (fail-soft)', async () => {
    const sqlite = seededSqlite()
    const logged: Array<Logged> = []
    const handle = makeTreasuryDomainHandle({
      d1: sqlite.db,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push([event, fields]),
      postgres: fakePostgres({
        upsertRows: () => Promise.reject(new Error('postgres down')),
      }),
      wait: () => Promise.resolve(),
    })
    await expect(
      runLedgerStatements(handle, [balanceStatement('actor.a', 1000)]),
    ).resolves.toBeUndefined()
    const row = await sqlite.db
      .prepare(`SELECT balance_msat FROM agent_balances WHERE actor_ref = ?`)
      .bind('actor.a')
      .first<{ balance_msat: number }>()
    expect(row?.balance_msat).toBe(1000)
    expect(logged[0]?.[0]).toBe('khala_sync_treasury_dual_write_failed')
    sqlite.close()
  })
})

describe('registry discipline', () => {
  test('covers all 27 domain tables with conflict keys inside key columns', () => {
    const tables = Object.keys(TREASURY_DOMAIN_TABLES)
    expect(tables).toHaveLength(27)
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
    expect(Object.keys(backfill.TREASURY_TABLE_SPECS).sort()).toEqual(
      Object.keys(TREASURY_DOMAIN_TABLES).sort(),
    )
    for (const [table, spec] of Object.entries(TREASURY_DOMAIN_TABLES)) {
      const twin = backfill.TREASURY_TABLE_SPECS[table]!
      expect(twin.columns, table).toEqual(spec.columns)
      expect(twin.conflictKey, table).toBe(spec.conflictKey)
    }
  })
})
