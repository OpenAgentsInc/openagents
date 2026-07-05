// KS-8.17 (#8328): supervision long-tail domain repository CONTRACT suite.
//
// Three layers, one behavioral spec:
//
//  1. `SupervisionLongtailWriteStore` contract — the row seam's composite-PK
//     converge semantics run identically against BOTH implementations:
//     - D1: `makeD1SupervisionLongtailWriteStore` over real SQLite
//       (node:sqlite — the engine D1 is built on), schema from
//       test/sqlite-d1.ts (D1 uniques KEPT).
//     - Postgres: `makePostgresSupervisionLongtailStore` over a throwaway
//       local Postgres, schema from khala-sync-server migration 0022.
//       Skipped when no local Postgres binaries exist.
//
//  2. READ-BACK MIRROR fidelity — the mirror machinery the wired store
//     factories use (mirrorRowsByKey / mirrorRowsWhere / pruneRowsOlderThan)
//     converges the Postgres twin byte-for-byte to the D1 authority,
//     including the RelayHealth.probeTick retention-prune path, and is
//     fail-soft (a broken twin never throws; a typed drift diagnostic is
//     emitted instead).
//
//  3. Flag routing + gate: dual-write defaults ON, off-values disable, an
//     unknown read value can never fail open to postgres; and a store-factory
//     with no KHALA_SYNC_DB binding (or dual-write off) returns the untouched
//     D1 base store.

import {
  SUPERVISION_LONGTAIL_TABLE_SPECS,
  normalizeSupervisionLongtailValue,
  type CompareSoakSample,
  type SupervisionLongtailTable,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1SupervisionLongtailWriteStore,
  makeHygieneDebtReceiptStoreForEnv,
  makeOmniPublicProofBundleCompareReader,
  makeOmniPublicProofBundlePostgresServerForEnv,
  makePostgresSupervisionLongtailStore,
  makeSupervisionLongtailMirror,
  makeSupervisionLongtailMirrorForEnv,
  supervisionLongtailFlagsFromEnv,
  type SupervisionLongtailDiagnostic,
  type SupervisionLongtailDiagnosticEvent,
  type SupervisionLongtailRow,
  type SupervisionLongtailWriteStore,
} from './supervision-longtail-domain-store'
import { makeSqliteD1, SUPERVISION_LONGTAIL_D1_SCHEMA } from './test/sqlite-d1'

const T0 = '2026-07-04T00:00:00.000Z'
const T1 = '2026-07-04T01:00:00.000Z'
const T2 = '2026-07-04T02:00:00.000Z'

// ---------------------------------------------------------------------------
// Flags (pure)
// ---------------------------------------------------------------------------

describe('supervisionLongtailFlagsFromEnv (pure)', () => {
  test('dual-write defaults ON; reads default d1; off-values disable', () => {
    expect(supervisionLongtailFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
    expect(
      supervisionLongtailFlagsFromEnv({
        KHALA_SYNC_SUPERVISION_DUAL_WRITE: 'off',
      }).dualWrite,
    ).toBe(false)
    expect(
      supervisionLongtailFlagsFromEnv({
        KHALA_SYNC_SUPERVISION_DUAL_WRITE: '0',
      }).dualWrite,
    ).toBe(false)
    expect(
      supervisionLongtailFlagsFromEnv({
        KHALA_SYNC_SUPERVISION_READS: 'compare',
      }).reads,
    ).toBe('compare')
    expect(
      supervisionLongtailFlagsFromEnv({
        KHALA_SYNC_SUPERVISION_READS: 'postgres',
      }).reads,
    ).toBe('postgres')
    // A typo can never fail open into an unproven read path.
    expect(
      supervisionLongtailFlagsFromEnv({
        KHALA_SYNC_SUPERVISION_READS: 'psotgres',
      }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Flag gate: no binding / dual-write off returns the untouched base store
// ---------------------------------------------------------------------------

describe('store-factory gate (no Postgres binding needed)', () => {
  test('no KHALA_SYNC_DB → factory returns the D1 base store', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const store = makeHygieneDebtReceiptStoreForEnv(
      { OPENAGENTS_DB: sqlite.db },
      { db: sqlite.db },
    )
    // Base store shape (no mirror wrapping observable at the type level; the
    // gate is that construction succeeds with no binding).
    expect(typeof store.create).toBe('function')
    expect(makeSupervisionLongtailMirrorForEnv({ OPENAGENTS_DB: sqlite.db }, { db: sqlite.db })).toBeUndefined()
    sqlite.close()
  })

  test('dual-write off → no mirror even with a binding', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const mirror = makeSupervisionLongtailMirrorForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
        KHALA_SYNC_SUPERVISION_DUAL_WRITE: 'off',
        OPENAGENTS_DB: sqlite.db,
      },
      { db: sqlite.db, makeSqlClient: () => Promise.reject(new Error('unused')) },
    )
    expect(mirror).toBeUndefined()
    sqlite.close()
  })
})

// ---------------------------------------------------------------------------
// Layer 1 + 2: write-store contract + mirror (Postgres-backed)
// ---------------------------------------------------------------------------

type ContractHarness = Readonly<{
  store: SupervisionLongtailWriteStore
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

const proofBundleRow = (
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): SupervisionLongtailRow => ({
  acceptance_state_ref: 'acceptance.pending',
  archived_at: null,
  artifact_refs_json: '[]',
  created_at: T0,
  economics_caveat_ref: 'econ.caveat',
  id,
  idempotency_key: `idem.${id}`,
  legal_caveat_ref: null,
  legal_sensitive: 0,
  metadata_json: '{}',
  no_settlement_implication: 1,
  privacy_caveat_ref: 'privacy.caveat',
  public_receipt_ref: `receipt.${id}`,
  receipt_refs_json: '[]',
  review_state_ref: 'review.pending',
  source_refs_json: '[]',
  status: 'ready',
  updated_at: T0,
  work_kind: 'site',
  workroom_id: 'workroom.1',
  ...overrides,
})

const enrichmentRow = (
  assignmentId: string,
  runId: string,
  overrides: Partial<Record<string, unknown>> = {},
): SupervisionLongtailRow => ({
  approved_at: null,
  assignment_id: assignmentId,
  created_at: T0,
  enrichment_run_id: runId,
  required_for_launch: 0,
  research_brief_id: null,
  status: 'planned',
  updated_at: T0,
  ...overrides,
})

const specContractSuite = (harness: () => ContractHarness) => {
  test('single-PK converge is idempotent and newer wins', async () => {
    const { query, store } = harness()
    expect(
      await store.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('bundle.a'),
        proofBundleRow('bundle.b'),
      ]),
    ).toBe(2)
    // Re-run: converge, no duplication.
    await store.upsertRows('omni_public_proof_bundles', [
      proofBundleRow('bundle.a'),
      proofBundleRow('bundle.b'),
    ])
    const counted = await query(
      `SELECT COUNT(*) AS total FROM omni_public_proof_bundles WHERE id LIKE 'bundle.%'`,
    )
    expect(Number(counted[0]?.['total'])).toBe(2)
    // Newer snapshot wins.
    await store.upsertRows('omni_public_proof_bundles', [
      proofBundleRow('bundle.a', { status: 'superseded', updated_at: T1 }),
    ])
    const rows = await query(
      `SELECT status FROM omni_public_proof_bundles WHERE id = 'bundle.a'`,
    )
    expect(rows[0]?.['status']).toBe('superseded')
  })

  test('composite PK never cross-contaminates', async () => {
    const { query, store } = harness()
    await store.upsertRows('adjutant_assignment_enrichments', [
      enrichmentRow('assign.1', 'run.1'),
      enrichmentRow('assign.1', 'run.2'),
    ])
    const rows = await query(
      `SELECT COUNT(*) AS total FROM adjutant_assignment_enrichments WHERE assignment_id = 'assign.1'`,
    )
    expect(Number(rows[0]?.['total'])).toBe(2)
  })
}

describe('supervision write-store contract — D1 (real SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1> | undefined
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite!.db.prepare(sql).all<Record<string, unknown>>())
          .results ?? [],
      store: makeD1SupervisionLongtailWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

const MIGRATION_0024 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0024_supervision_longtail.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'supervision write-store contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE supervision_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('supervision_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresSupervisionLongtailStore({
          acquireSql: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)
  },
)

// ---------------------------------------------------------------------------
// Layer 2: read-back mirror fidelity (Postgres-backed)
// ---------------------------------------------------------------------------

const projectRow = (
  table: SupervisionLongtailTable,
  row: Record<string, unknown>,
): Record<string, string | null> =>
  Object.fromEntries(
    SUPERVISION_LONGTAIL_TABLE_SPECS[table].columns.map(column => {
      const value = normalizeSupervisionLongtailValue(row[column])
      return [column, value === null ? null : String(value)]
    }),
  )

describe.skipIf(!hasLocalPostgres())(
  'read-back mirror converges the Postgres twin and is fail-soft',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE supervision_mirror')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('supervision_mirror'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))
      sqlite = makeSqliteD1()
      sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    const postgresStore = () =>
      makePostgresSupervisionLongtailStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: (client as unknown as { unsafe: unknown }) as never,
          }),
      })

    test('mirrorRowsByKey + mirrorRowsWhere + prune converge byte-for-byte', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const mirror = makeSupervisionLongtailMirror({
        db: sqlite!.db,
        log: (event, fields) => diagnostics.push({ event, fields }),
        postgres: postgresStore(),
      })
      const d1 = makeD1SupervisionLongtailWriteStore(sqlite!.db)

      // Seed the D1 authority (proof bundle + relay probes/transitions).
      await d1.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('mirror.a'),
        proofBundleRow('mirror.b', { workroom_id: 'workroom.2' }),
      ])
      await d1.upsertRows('relay_health_probes', [
        {
          created_at: T0,
          id: 'probe.old',
          nip11_http_status: 200,
          nip11_latency_ms: 12,
          nip11_outcome: 'ok',
          nip11_relay_name: 'market',
          probed_at: T0,
          relay_url: 'wss://relay',
          status: 'healthy',
          ws_latency_ms: 30,
          ws_outcome: 'ok',
        },
        {
          created_at: T2,
          id: 'probe.new',
          nip11_http_status: 530,
          nip11_latency_ms: null,
          nip11_outcome: 'http_error',
          nip11_relay_name: null,
          probed_at: T2,
          relay_url: 'wss://relay',
          status: 'unhealthy',
          ws_latency_ms: null,
          ws_outcome: 'refused',
        },
      ])

      await mirror.mirrorRowsByKey('omni_public_proof_bundles', [
        ['mirror.a'],
        ['mirror.b'],
      ])
      await mirror.mirrorRowsWhere('relay_health_probes', ['relay_url'], [
        'wss://relay',
      ])

      expect(diagnostics).toEqual([])

      for (const table of [
        'omni_public_proof_bundles',
        'relay_health_probes',
      ] as const) {
        const order = SUPERVISION_LONGTAIL_TABLE_SPECS[table].keyColumns.join(
          ', ',
        )
        const d1Rows =
          (
            await sqlite!.db
              .prepare(`SELECT * FROM ${table} ORDER BY ${order}`)
              .all<Record<string, unknown>>()
          ).results ?? []
        const pgRows = await (client as PgClient).unsafe(
          `SELECT * FROM ${table} ORDER BY ${order}`,
        )
        expect(
          pgRows.map(row => projectRow(table, row)),
          `table ${table} must mirror byte-faithfully`,
        ).toEqual(d1Rows.map(row => projectRow(table, row)))
      }

      // Retention prune (the probeTick path): drop the old probe on D1, then
      // converge the prune onto the twin.
      await sqlite!.db
        .prepare(`DELETE FROM relay_health_probes WHERE probed_at < ?`)
        .bind(T1)
        .run()
      await mirror.pruneRowsOlderThan('relay_health_probes', 'probed_at', T1)
      const remaining = await (client as PgClient).unsafe(
        `SELECT COUNT(*) AS total FROM relay_health_probes`,
      )
      expect(Number(remaining[0]?.['total'])).toBe(1)
      expect(diagnostics).toEqual([])
    })

    test('a broken Postgres twin never throws — it emits the drift diagnostic', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const brokenMirror = makeSupervisionLongtailMirror({
        db: sqlite!.db,
        log: (event, fields) => diagnostics.push({ event, fields }),
        postgres: makePostgresSupervisionLongtailStore({
          acquireSql: () => Promise.reject(new Error('postgres is down')),
        }),
      })
      // Never throws.
      await brokenMirror.mirrorRowsByKey('hygiene_debt_receipts', [['key.1']])
      expect(diagnostics.map(d => d.event)).toContain(
        'khala_sync_supervision_dual_write_failed',
      )
      // Diagnostic carries the row KEY only — no custody value.
      expect(diagnostics[0]?.fields.refs).toEqual(['key.1'])
    })
  },
)

// ---------------------------------------------------------------------------
// Read-compare shadow (KS-8.17 follow-up, #8361): the public proof-bundle
// reader is fire-and-forget, D1-serving-always, and only ever LOGS a diff —
// it never changes a served response, at any flag value.
// ---------------------------------------------------------------------------

describe('makeOmniPublicProofBundleCompareReader — flag gate (no Postgres binding needed)', () => {
  test('no KHALA_SYNC_DB binding → undefined even when reads=compare', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const reader = makeOmniPublicProofBundleCompareReader(
      { KHALA_SYNC_SUPERVISION_READS: 'compare' },
      { db: sqlite.db },
    )
    expect(reader).toBeUndefined()
    sqlite.close()
  })

  test('reads=d1 (default) → undefined even with a Postgres binding', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const reader = makeOmniPublicProofBundleCompareReader(
      { KHALA_SYNC_DB: { connectionString: 'postgres://contract' } },
      {
        db: sqlite.db,
        makeSqlClient: () => Promise.reject(new Error('unused')),
      },
    )
    expect(reader).toBeUndefined()
    sqlite.close()
  })
})

describe.skipIf(!hasLocalPostgres())(
  'makeOmniPublicProofBundleCompareReader — shadow compare against the Postgres twin',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE supervision_compare')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('supervision_compare'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))
      sqlite = makeSqliteD1()
      sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    const postgresStore = () =>
      makePostgresSupervisionLongtailStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: (client as unknown as { unsafe: unknown }) as never,
          }),
      })

    const makeReader = (
      reads: 'compare' | 'postgres',
      diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }>,
      samples?: CompareSoakSample[],
    ) =>
      makeOmniPublicProofBundleCompareReader(
        {
          KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
          KHALA_SYNC_SUPERVISION_READS: reads,
        },
        {
          db: sqlite!.db,
          log: (event, fields) => diagnostics.push({ event, fields }),
          makeSqlClient: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: (client as unknown as { unsafe: unknown }) as never,
            }),
          metrics: samples ? { record: sample => samples.push(sample) } : undefined,
        },
      )

    test('rows match in D1 and Postgres → no diagnostic, but records a durable match sample (#8282)', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const samples: CompareSoakSample[] = []
      const d1 = makeD1SupervisionLongtailWriteStore(sqlite!.db)
      await d1.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.match'),
      ])
      await postgresStore().upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.match'),
      ])

      const reader = makeReader('compare', diagnostics, samples)
      expect(reader).toBeDefined()
      await reader!('compare.match')
      expect(diagnostics).toEqual([])
      expect(samples).toEqual([
        {
          domain: 'supervision',
          outcome: 'match',
          readKind: 'omni_public_proof_bundles:readById',
        },
      ])
    })

    test('rows differ → khala_sync_supervision_read_compare_mismatch (D1 still serves the response) and a durable mismatch sample', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const samples: CompareSoakSample[] = []
      const d1 = makeD1SupervisionLongtailWriteStore(sqlite!.db)
      await d1.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.mismatch', { status: 'ready' }),
      ])
      await postgresStore().upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.mismatch', { status: 'superseded' }),
      ])

      const reader = makeReader('compare', diagnostics, samples)
      await reader!('compare.mismatch')
      expect(diagnostics.map(d => d.event)).toContain(
        'khala_sync_supervision_read_compare_mismatch',
      )
      // Diagnostic carries the row KEY only — no custody value.
      expect(diagnostics[0]?.fields.refs).toEqual(['compare.mismatch'])
      expect(samples).toEqual([
        {
          domain: 'supervision',
          outcome: 'mismatch',
          readKind: 'omni_public_proof_bundles:readById',
        },
      ])
    })

    test('present in D1, missing in Postgres → mismatch diagnostic', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const d1 = makeD1SupervisionLongtailWriteStore(sqlite!.db)
      await d1.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.missing-pg'),
      ])

      const reader = makeReader('compare', diagnostics)
      await reader!('compare.missing-pg')
      expect(diagnostics.map(d => d.event)).toContain(
        'khala_sync_supervision_read_compare_mismatch',
      )
    })

    test('absent from both stores → no diagnostic (not a mismatch)', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const reader = makeReader('compare', diagnostics)
      await reader!('compare.nowhere')
      expect(diagnostics).toEqual([])
    })

    test('reads=postgres logs the deferred diagnostic once and never serves Postgres', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const d1 = makeD1SupervisionLongtailWriteStore(sqlite!.db)
      await d1.upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.deferred'),
      ])
      await postgresStore().upsertRows('omni_public_proof_bundles', [
        proofBundleRow('compare.deferred'),
      ])

      const reader = makeReader('postgres', diagnostics)
      await reader!('compare.deferred')
      await reader!('compare.deferred')
      const deferredCount = diagnostics.filter(
        d => d.event === 'khala_sync_supervision_postgres_reads_deferred',
      ).length
      expect(deferredCount).toBe(1)
      expect(diagnostics.map(d => d.event)).not.toContain(
        'khala_sync_supervision_read_compare_mismatch',
      )
    })

    test('a broken Postgres twin never throws — it emits the failed diagnostic and records an error sample', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const samples: CompareSoakSample[] = []
      const reader = makeOmniPublicProofBundleCompareReader(
        {
          KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
          KHALA_SYNC_SUPERVISION_READS: 'compare',
        },
        {
          db: sqlite!.db,
          log: (event, fields) => diagnostics.push({ event, fields }),
          makeSqlClient: () => Promise.reject(new Error('postgres is down')),
          metrics: { record: sample => samples.push(sample) },
        },
      )
      expect(reader).toBeDefined()
      await expect(reader!('compare.broken')).resolves.toBeUndefined()
      expect(diagnostics.map(d => d.event)).toContain(
        'khala_sync_supervision_read_compare_failed',
      )
      expect(samples).toEqual([
        {
          domain: 'supervision',
          outcome: 'error',
          readKind: 'omni_public_proof_bundles:readById',
        },
      ])
    })
  },
)

// ---------------------------------------------------------------------------
// makeOmniPublicProofBundlePostgresServerForEnv (KS-8.17 read-cutover
// follow-up, #8361) — the bounded real-Postgres-serve reader, distinct from
// the shadow-compare reader above (which never serves).
// ---------------------------------------------------------------------------

describe('makeOmniPublicProofBundlePostgresServerForEnv — flag gate (no Postgres binding needed)', () => {
  test('no KHALA_SYNC_DB binding → undefined even when reads=postgres', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const server = makeOmniPublicProofBundlePostgresServerForEnv(
      { KHALA_SYNC_SUPERVISION_READS: 'postgres' },
      { db: sqlite.db },
    )
    expect(server).toBeUndefined()
    sqlite.close()
  })

  test('reads=d1 (default) → undefined even with a Postgres binding', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const server = makeOmniPublicProofBundlePostgresServerForEnv(
      { KHALA_SYNC_DB: { connectionString: 'postgres://contract' } },
      {
        db: sqlite.db,
        makeSqlClient: () => Promise.reject(new Error('unused')),
      },
    )
    expect(server).toBeUndefined()
    sqlite.close()
  })

  test('reads=compare → undefined (compare mode never serves — see makeOmniPublicProofBundleCompareReader)', () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    const server = makeOmniPublicProofBundlePostgresServerForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
        KHALA_SYNC_SUPERVISION_READS: 'compare',
      },
      {
        db: sqlite.db,
        makeSqlClient: () => Promise.reject(new Error('unused')),
      },
    )
    expect(server).toBeUndefined()
    sqlite.close()
  })
})

describe.skipIf(!hasLocalPostgres())(
  'makeOmniPublicProofBundlePostgresServerForEnv — real serving against the Postgres twin',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1> | undefined

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE supervision_postgres_serve')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('supervision_postgres_serve'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0024, 'utf8'))
      sqlite = makeSqliteD1()
      sqlite.exec(SUPERVISION_LONGTAIL_D1_SCHEMA)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
      sqlite?.close()
    }, 60_000)

    const postgresStore = () =>
      makePostgresSupervisionLongtailStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: (client as unknown as { unsafe: unknown }) as never,
          }),
      })

    const makeServer = (
      diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }>,
    ) =>
      makeOmniPublicProofBundlePostgresServerForEnv(
        {
          KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
          KHALA_SYNC_SUPERVISION_READS: 'postgres',
        },
        {
          db: sqlite!.db,
          log: (event, fields) => diagnostics.push({ event, fields }),
          makeSqlClient: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: (client as unknown as { unsafe: unknown }) as never,
            }),
        },
      )

    test('row present in Postgres → serves the record, converted the same way the D1 path does', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      await postgresStore().upsertRows('omni_public_proof_bundles', [
        proofBundleRow('serve.present', { status: 'ready' }),
      ])

      const server = makeServer(diagnostics)
      expect(server).toBeDefined()
      const result = await server!('serve.present')
      expect(result).toBeDefined()
      expect(result?.record?.id).toBe('serve.present')
      expect(result?.record?.status).toBe('ready')
      expect(result?.record?.workroomId).toBe('workroom.1')
      expect(diagnostics).toEqual([])
    })

    test('row absent from Postgres → a genuine final "not found" (record: null), not a fallback signal', async () => {
      const server = makeServer([])
      const result = await server!('serve.nowhere')
      expect(result).toEqual({ record: null })
    })

    test('a broken Postgres twin never throws — returns undefined (fall back to D1) and logs the serve-failed diagnostic', async () => {
      const diagnostics: Array<{
        event: SupervisionLongtailDiagnosticEvent
        fields: SupervisionLongtailDiagnostic
      }> = []
      const server = makeOmniPublicProofBundlePostgresServerForEnv(
        {
          KHALA_SYNC_DB: { connectionString: 'postgres://contract' },
          KHALA_SYNC_SUPERVISION_READS: 'postgres',
        },
        {
          db: sqlite!.db,
          log: (event, fields) => diagnostics.push({ event, fields }),
          makeSqlClient: () => Promise.reject(new Error('postgres is down')),
        },
      )
      expect(server).toBeDefined()
      await expect(server!('serve.broken')).resolves.toBeUndefined()
      expect(diagnostics.map(d => d.event)).toContain(
        'khala_sync_supervision_postgres_read_serve_failed',
      )
      expect(diagnostics[0]?.fields.refs).toEqual(['serve.broken'])
    })
  },
)
