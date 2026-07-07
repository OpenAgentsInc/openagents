// KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
// mirrorcode eval domain repository CONTRACT suite.
//
// One behavioral spec, TWO implementations of `GymEvalsDomainWriteStore`:
//   - D1: `makeD1GymEvalsDomainWriteStore` over real SQLite (node:sqlite — the
//     engine D1 is built on), schema from GYM_EVALS_DOMAIN_D1_SCHEMA.
//   - Postgres: `makePostgresGymEvalsDomainStore` over a throwaway local
//     Postgres, schema from khala-sync-server migration 0026. Skipped when no
//     local Postgres binaries exist.
//
// Every case runs identically against both stores — the KS-8.15 remainder
// load-bearing properties:
//   * snapshot/state tables converge on their live arbiters (run_ref / run_id /
//     ladder_ref / composite (run_ref, stage)) to the latest authoritative
//     row, byte-exactly (gym / ladder / mirrorcode rows feed public
//     projections; this is the "leaderboard recomputation equality" acceptance
//     proven by byte-equal round-trip, not recomputation);
//   * insert-once tables (harbor archives) dedupe on exact replay AND on their
//     secondary unique (harbor artifact_sha256) and never clobber;
//   * a converge table's secondary unique (blueprint idempotency_key) REJECTS
//     a conflicting second row on BOTH stores.
//
// Plus the seam-level cases (D1-backed, no Postgres needed): dual-write
// fail-soft, the read-back mirror, and flag parsing.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  gymEvalsFlagsFromEnv,
  makeD1GymEvalsDomainWriteStore,
  makeDualWriteGymEvalsDomainWriteStore,
  makeGymEvalsDomainMirror,
  makeGymEvalsDomainMirrorForEnv,
  makePostgresGymEvalsDomainStore,
  type GymEvalsDiagnostic,
  type GymEvalsDiagnosticEvent,
  type GymEvalsDomainRow,
  type GymEvalsDomainWriteStore,
  type GymEvalsStoreEnv,
} from './gym-evals-domain-store'
import { GYM_EVALS_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.contract.${++refCounter}`

const runProgressRow = (
  runRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  created_at: '2026-07-04T12:00:00.000Z',
  ingested_at: '2026-07-04T12:00:00.000Z',
  last_updated_at: '2026-07-04T12:00:00.000Z',
  progress_json: '{"schemaVersion":"openagents.gym.run_progress.v1"}',
  run_ref: runRef,
  ...overrides,
})

const mirrorcodeRow = (
  runId: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  bucket: 'public',
  created_at: '2026-07-04T12:00:00.000Z',
  grade: 'A',
  run_id: runId,
  run_json: '{"runId":"' + runId + '"}',
  started_at: '2026-07-04T12:00:00.000Z',
  status: 'running',
  updated_at: '2026-07-04T12:00:00.000Z',
  ...overrides,
})

const ladderRow = (
  ladderRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  created_at: '2026-07-04T12:00:00.000Z',
  ladder_json: '{"ladderRef":"' + ladderRef + '","rungs":[]}',
  ladder_ref: ladderRef,
  published_at: '2026-07-04T12:00:00.000Z',
  ...overrides,
})

const mutaliskProgressRow = (
  runRef: string,
  stage: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  progress_json: '{"stage":"' + stage + '"}',
  run_ref: runRef,
  stage,
  updated_at: '2026-07-04T12:00:00.000Z',
  ...overrides,
})

const harborRow = (
  archiveRef: string,
  sha256: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  archive_ref: archiveRef,
  artifact_bytes: 4096,
  artifact_r2_key: `private/gym/harbor/${archiveRef}.tar.gz`,
  artifact_sha256: sha256,
  capture_completed_at: '2026-07-04T12:00:00.000Z',
  capture_started_at: null,
  contains_private_material: 1,
  contains_raw_logs: 1,
  contains_raw_prompts: 1,
  content_type: 'application/gzip',
  created_at: '2026-07-04T12:00:00.000Z',
  demand_kind: 'internal',
  demand_source: 'harbor_terminal_bench',
  job_ref: `job.${archiveRef}`,
  run_ref: `run.${archiveRef}`,
  source_kind: 'harbor_job_tarball',
  updated_at: '2026-07-04T12:00:00.000Z',
  visibility: 'operator_only',
  ...overrides,
})

const programRunRow = (
  id: string,
  idempotencyKey: string,
  overrides: Partial<Record<string, unknown>> = {},
): GymEvalsDomainRow => ({
  actor_ref: 'actor.contract',
  archived_at: null,
  authority_boundary: 'evidence_only',
  confidence: 0.5,
  cost_ref: 'cost.contract',
  created_at: '2026-07-04T12:00:00.000Z',
  direct_mutation_disabled: 1,
  evidence_refs_json: '[]',
  id,
  idempotency_key: idempotencyKey,
  input_snapshot_hash: 'hash.contract',
  latency_ms: 12,
  metadata_json: '{}',
  module_version_id: 'module.v1',
  no_deploy: 1,
  no_email: 1,
  no_source_mutation: 1,
  no_spend: 1,
  program_signature_id: 'signature.v1',
  program_type_id: 'type.v1',
  purpose_ref: 'purpose.contract',
  receipt_refs_json: '[]',
  route_ref: 'route.contract',
  typed_output_json: '{}',
  updated_at: '2026-07-04T12:00:00.000Z',
  ...overrides,
})

type ContractHarness = Readonly<{
  store: GymEvalsDomainWriteStore
  /** Portable read-only SQL (SELECT …) against the same store's tables. */
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

// ---------------------------------------------------------------------------
// The shared behavioral spec
// ---------------------------------------------------------------------------

const specContractSuite = (getHarness: () => ContractHarness) => {
  test('run-progress snapshots converge on run_ref to the latest bytes', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('run')
    expect(
      await store.upsertRows('gym_run_progress_snapshots', [
        runProgressRow(runRef),
      ]),
    ).toBe(1)
    await store.upsertRows('gym_run_progress_snapshots', [
      runProgressRow(runRef, {
        last_updated_at: '2026-07-04T13:00:00.000Z',
        progress_json:
          '{"schemaVersion":"openagents.gym.run_progress.v1","done":true}',
      }),
    ])
    const rows = await query(
      `SELECT progress_json FROM gym_run_progress_snapshots WHERE run_ref = '${runRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.progress_json).toBe(
      '{"schemaVersion":"openagents.gym.run_progress.v1","done":true}',
    )
  })

  test('mirrorcode runs converge on run_id to the latest status', async () => {
    const { query, store } = getHarness()
    const runId = nextRef('mirrorcode')
    await store.upsertRows('mirrorcode_runs', [mirrorcodeRow(runId)])
    await store.upsertRows('mirrorcode_runs', [
      mirrorcodeRow(runId, {
        grade: 'S',
        status: 'succeeded',
        updated_at: '2026-07-04T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT status, grade FROM mirrorcode_runs WHERE run_id = '${runId}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.status).toBe('succeeded')
    expect(rows[0]?.grade).toBe('S')
  })

  test('ladder snapshots converge on ladder_ref; ladder_json round-trips byte-exactly (leaderboard equality)', async () => {
    const { query, store } = getHarness()
    const ladderRef = nextRef('ladder')
    await store.upsertRows('gym_ladder_leaderboard_snapshots', [
      ladderRow(ladderRef),
    ])
    const recomputed =
      '{"ladderRef":"' + ladderRef + '","rungs":[{"rung":"rung1"}]}'
    await store.upsertRows('gym_ladder_leaderboard_snapshots', [
      ladderRow(ladderRef, {
        ladder_json: recomputed,
        published_at: '2026-07-04T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT ladder_json FROM gym_ladder_leaderboard_snapshots WHERE ladder_ref = '${ladderRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.ladder_json).toBe(recomputed)
  })

  test('mutalisk progress converges on the composite (run_ref, stage)', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('mutalisk')
    await store.upsertRows('gym_mutalisk_khala_delegation_progress', [
      mutaliskProgressRow(runRef, 'queued'),
      mutaliskProgressRow(runRef, 'optimizing'),
    ])
    // Re-emitting one stage converges that row, never a duplicate.
    await store.upsertRows('gym_mutalisk_khala_delegation_progress', [
      mutaliskProgressRow(runRef, 'optimizing', {
        progress_json: '{"stage":"optimizing","pct":50}',
        updated_at: '2026-07-04T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT stage, progress_json FROM gym_mutalisk_khala_delegation_progress WHERE run_ref = '${runRef}' ORDER BY stage`,
    )
    expect(rows.map(row => row.stage)).toEqual(['optimizing', 'queued'])
    expect(rows[0]?.progress_json).toBe('{"stage":"optimizing","pct":50}')
  })

  test('harbor archives insert-once: fresh lands; exact replay + digest collision are no-ops that never clobber', async () => {
    const { query, store } = getHarness()
    const archiveRef = nextRef('archive')
    const sha256 = `sha.${archiveRef}`
    expect(
      await store.upsertRows('gym_harbor_full_trace_archives', [
        harborRow(archiveRef, sha256),
      ]),
    ).toBe(1)
    // Exact replay: archive_ref PK dedupe, no clobber.
    expect(
      await store.upsertRows('gym_harbor_full_trace_archives', [
        harborRow(archiveRef, sha256, { artifact_r2_key: 'CLOBBERED' }),
      ]),
    ).toBe(0)
    // A different archive_ref with the same digest is silently ignored (the
    // artifact_sha256 UNIQUE dedupe) — never a duplicate archive body ref.
    expect(
      await store.upsertRows('gym_harbor_full_trace_archives', [
        harborRow(nextRef('archive'), sha256),
      ]),
    ).toBe(0)
    const rows = await query(
      `SELECT artifact_r2_key FROM gym_harbor_full_trace_archives WHERE artifact_sha256 = '${sha256}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.artifact_r2_key).toBe(
      `private/gym/harbor/${archiveRef}.tar.gz`,
    )
  })

  test('blueprint program runs: a conflicting second row under the same idempotency_key REJECTS on BOTH stores', async () => {
    const { query, store } = getHarness()
    const idempotencyKey = nextRef('idem')
    const id = nextRef('program_run')
    await store.upsertRows('blueprint_program_runs', [
      programRunRow(id, idempotencyKey),
    ])
    // Converge on the same id is fine (a status/confidence update).
    await store.upsertRows('blueprint_program_runs', [
      programRunRow(id, idempotencyKey, {
        confidence: 0.9,
        updated_at: '2026-07-04T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT confidence FROM blueprint_program_runs WHERE id = '${id}'`,
    )
    expect(rows).toHaveLength(1)
    expect(Number(rows[0]?.confidence)).toBe(0.9)

    // A DIFFERENT id under the same idempotency_key must be rejected (the
    // UNIQUE(idempotency_key) the live writer relies on).
    await expect(
      store.upsertRows('blueprint_program_runs', [
        programRunRow(nextRef('program_run'), idempotencyKey),
      ]),
    ).rejects.toThrow()
  })
}

// ---------------------------------------------------------------------------
// D1 (SQLite) harness
// ---------------------------------------------------------------------------

describe('gym/evals domain repository contract — D1 (SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(GYM_EVALS_DOMAIN_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite.db.prepare(sql).all<Record<string, unknown>>()).results ??
        [],
      store: makeD1GymEvalsDomainWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

// ---------------------------------------------------------------------------
// Postgres harness (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

const MIGRATION_0026 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0026_gym_evals_domain.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'gym/evals domain repository contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE gym_evals_domain_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('gym_evals_domain_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0026, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresGymEvalsDomainStore({
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
// Seam-level cases: flags, fail-soft dual-write, read-back mirror
// ---------------------------------------------------------------------------

describe('gym/evals domain seam', () => {
  test('flags: dual-write defaults ON, reads default d1, writes default postgres, typos never fail open', () => {
    expect(gymEvalsFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
      writes: 'postgres',
    })
    // #8515 WRITE cutover: default postgres; only an explicit 'd1' opts out.
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_WRITES: 'd1' }).writes,
    ).toBe('d1')
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_WRITES: 'postgrse' }).writes,
    ).toBe('postgres')
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_DUAL_WRITE: 'off' })
        .dualWrite,
    ).toBe(false)
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_DUAL_WRITE: '0' }).dualWrite,
    ).toBe(false)
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      gymEvalsFlagsFromEnv({ KHALA_SYNC_GYM_EVALS_READS: 'postgrse' }).reads,
    ).toBe('d1')
  })

  test('#8515 writes cutover: the D1->Postgres mirror is DISABLED when writes=postgres and ACTIVE on the d1 rollback path', () => {
    const sqlite = makeSqliteD1()
    try {
      const baseEnv = {
        KHALA_SYNC_DB: { connectionString: 'postgres://scripted/test' },
        OPENAGENTS_DB: sqlite.db,
      }
      // Default (postgres): base runs on the adapter, so the read-back mirror
      // is redundant and disabled.
      expect(
        makeGymEvalsDomainMirrorForEnv(baseEnv as unknown as GymEvalsStoreEnv),
      ).toBeUndefined()
      // Explicit d1 rollback: the dual-write mirror is active again.
      expect(
        makeGymEvalsDomainMirrorForEnv({
          ...baseEnv,
          KHALA_SYNC_GYM_EVALS_WRITES: 'd1',
        } as unknown as GymEvalsStoreEnv),
      ).toBeDefined()
    } finally {
      sqlite.close()
    }
  })

  test('dual-write: a Postgres mirror failure NEVER fails the D1 write; the drift metric fires with refs only', async () => {
    const written: Array<string> = []
    const logged: Array<{
      event: GymEvalsDiagnosticEvent
      fields: GymEvalsDiagnostic
    }> = []
    const d1: GymEvalsDomainWriteStore = {
      upsertRows: async (table, rows) => {
        written.push(`${table}:${String(rows.length)}`)
        return rows.length
      },
    }
    const failing: GymEvalsDomainWriteStore = {
      upsertRows: () =>
        Promise.reject(new Error('secret-bearing postgres detail')),
    }
    const store = makeDualWriteGymEvalsDomainWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      log: (event, fields) => logged.push({ event, fields }),
      postgres: failing,
    })

    const outcome = await store.upsertRows('gym_run_progress_snapshots', [
      runProgressRow('run.failsoft.1'),
    ])
    expect(outcome).toBe(1)
    expect(written).toEqual(['gym_run_progress_snapshots:1'])
    expect(logged).toHaveLength(1)
    expect(logged[0]?.event).toBe('khala_sync_gym_evals_dual_write_failed')
    expect(logged[0]?.fields.refs).toEqual(['run.failsoft.1'])
    // Keys only — never projection payloads.
    expect(JSON.stringify(logged[0]?.fields)).not.toContain('progress_json')
  })

  test('dual-write disabled or missing binding degrades to plain D1', async () => {
    const d1: GymEvalsDomainWriteStore = {
      upsertRows: async (_table, rows) => rows.length,
    }
    const neverCalled: GymEvalsDomainWriteStore = {
      upsertRows: () => Promise.reject(new Error('must not be called')),
    }
    const offStore = makeDualWriteGymEvalsDomainWriteStore({
      d1,
      flags: { dualWrite: false, reads: 'd1', writes: 'd1' },
      postgres: neverCalled,
    })
    expect(
      await offStore.upsertRows('mirrorcode_runs', [mirrorcodeRow('m.off.1')]),
    ).toBe(1)
    const unboundStore = makeDualWriteGymEvalsDomainWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      postgres: undefined,
    })
    expect(
      await unboundStore.upsertRows('mirrorcode_runs', [
        mirrorcodeRow('m.off.2'),
      ]),
    ).toBe(1)
  })

  test('read-back mirror: reads the authoritative D1 rows by ref and upserts them; failures are swallowed', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(GYM_EVALS_DOMAIN_D1_SCHEMA)
    try {
      const runRef = nextRef('run')
      await makeD1GymEvalsDomainWriteStore(sqlite.db).upsertRows(
        'gym_run_progress_snapshots',
        [runProgressRow(runRef, { last_updated_at: '2026-07-04T14:00:00.000Z' })],
      )

      const mirroredRows: Array<GymEvalsDomainRow> = []
      const logged: Array<GymEvalsDiagnosticEvent> = []
      const mirror = makeGymEvalsDomainMirror({
        db: sqlite.db,
        log: event => logged.push(event),
        postgres: {
          upsertRows: async (_table, rows) => {
            mirroredRows.push(...rows)
            return rows.length
          },
        },
      })
      await mirror.mirrorRowsByRef('gym_run_progress_snapshots', [runRef])
      expect(mirroredRows).toHaveLength(1)
      expect(mirroredRows[0]?.['run_ref']).toBe(runRef)
      expect(logged).toEqual([])

      // A failing Postgres side never throws out of the mirror.
      const failingMirror = makeGymEvalsDomainMirror({
        db: sqlite.db,
        log: event => logged.push(event),
        postgres: {
          upsertRows: () => Promise.reject(new Error('down')),
        },
      })
      await expect(
        failingMirror.mirrorRowsByRef('gym_run_progress_snapshots', [runRef]),
      ).resolves.toBeUndefined()
      expect(logged).toEqual(['khala_sync_gym_evals_dual_write_failed'])
    } finally {
      sqlite.close()
    }
  })
})
