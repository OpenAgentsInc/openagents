// KS-8.15 (#8326): training domain repository CONTRACT suite.
//
// One behavioral spec, TWO implementations of `TrainingDomainWriteStore`:
//   - D1: `makeD1TrainingDomainWriteStore` over real SQLite (node:sqlite —
//     the engine D1 is built on), schema from the worker migrations
//     (condensed in test/sqlite-d1.ts).
//   - Postgres: `makePostgresTrainingDomainStore` over a throwaway local
//     Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//     0019. Skipped when no local Postgres binaries exist.
//
// Every case runs identically against both stores — the KS-8.15
// load-bearing properties:
//   * state tables converge on their LIVE ref arbiters (training_run_ref /
//     window_ref / lease_ref / challenge_ref / contribution_ref) to the
//     latest authoritative row, byte-exactly (training receipts feed
//     public claims);
//   * the two event ledgers dedupe on exact replay (id PK) and never
//     clobber the original link — window/verification event chains stay
//     contiguous;
//   * the trace-contribution idempotency key UNIQUE(lease_ref,
//     workload_family) REJECTS a second pending contribution on BOTH
//     stores (the exact D1 INSERT OR IGNORE key);
//   * lease rows are unique per lease_ref (double-lease = double-payout
//     risk upstream — a replayed claim converges, never duplicates).
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
  makeD1TrainingDomainWriteStore,
  makeDualWriteTrainingDomainWriteStore,
  makePostgresTrainingDomainStore,
  makeTrainingDomainMirror,
  trainingFlagsFromEnv,
  type TrainingDiagnostic,
  type TrainingDiagnosticEvent,
  type TrainingDomainRow,
  type TrainingDomainWriteStore,
} from './training-domain-store'
import { TRAINING_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.contract.${++refCounter}`

const runRow = (
  runRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  archived_at: null,
  created_at: '2026-07-04T12:00:00.000Z',
  id: `id:${runRef}`,
  manifest_json: null,
  max_allowed_stale: 5,
  promise_ref: 'promise.decentralized-training-launch',
  public_projection_json: '{"state":"planned"}',
  receipt_refs_json: '["receipt.plan.1"]',
  seal_in_flight_at: null,
  seal_publication_cadence_windows: 1,
  source_refs_json: '["issue.contract"]',
  state: 'planned',
  training_run_ref: runRef,
  updated_at: '2026-07-04T12:00:00.000Z',
  ...overrides,
})

const windowRow = (
  windowRef: string,
  runRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  activated_at: null,
  archived_at: null,
  dataset_refs_json: '["dataset.smoke"]',
  homework_kind: 'auto_starter',
  id: `id:${windowRef}`,
  planned_at: '2026-07-04T12:00:00.000Z',
  priority: 1,
  public_projection_json: '{"state":"planned"}',
  receipt_refs_json: '["receipt.window.plan"]',
  reconciled_at: null,
  seal_metadata_json: null,
  sealed_at: null,
  source_refs_json: '["issue.contract"]',
  state: 'planned',
  training_run_ref: runRef,
  updated_at: '2026-07-04T12:00:00.000Z',
  window_ref: windowRef,
  ...overrides,
})

const windowEventRow = (
  id: string,
  windowRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  actor_ref: 'operator.owner',
  archived_at: null,
  created_at: '2026-07-04T12:00:00.000Z',
  id,
  receipt_ref: `receipt.${id}`,
  state_from: 'planned',
  state_to: 'active',
  transition_kind: 'activate',
  window_ref: windowRef,
  ...overrides,
})

const leaseRow = (
  leaseRef: string,
  windowRef: string,
  runRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  archived_at: null,
  claimed_at: '2026-07-04T12:00:00.000Z',
  id: `id:${leaseRef}`,
  lease_expires_at: '2026-07-04T13:00:00.000Z',
  lease_ref: leaseRef,
  public_projection_json: '{"state":"active"}',
  pylon_ref: 'pylon.contract',
  receipt_refs_json: '["receipt.lease.claim"]',
  state: 'active',
  training_run_ref: runRef,
  window_ref: windowRef,
  ...overrides,
})

const challengeRow = (
  challengeRef: string,
  runRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  archived_at: null,
  attempt_count: 0,
  challenge_ref: challengeRef,
  commitment_refs_json: '["commitment.1"]',
  contribution_ref: null,
  created_at: '2026-07-04T12:00:00.000Z',
  failure_codes_json: '[]',
  homework_kind: 'auto_starter',
  id: `id:${challengeRef}`,
  lease_expires_at: null,
  lease_ref: null,
  leased_to_ref: null,
  max_attempts: 3,
  payload_json: '{"kind":"replay"}',
  public_projection_json: '{"state":"Queued"}',
  rejected_at: null,
  sampling_policy: 'per_contribution',
  state: 'Queued',
  timed_out_at: null,
  training_run_ref: runRef,
  updated_at: '2026-07-04T12:00:00.000Z',
  verdict_refs_json: '[]',
  verification_class: 'exact_trace_replay',
  verified_at: null,
  window_ref: null,
  ...overrides,
})

const verificationEventRow = (
  id: string,
  challengeRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  archived_at: null,
  challenge_ref: challengeRef,
  created_at: '2026-07-04T12:00:00.000Z',
  failure_codes_json: '[]',
  id,
  receipt_refs_json: '[]',
  state_from: null,
  state_to: 'Queued',
  transition_kind: 'create',
  validator_ref: null,
  ...overrides,
})

const contributionRow = (
  contributionRef: string,
  leaseRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): TrainingDomainRow => ({
  archived_at: null,
  assignment_ref: 'assignment.contract',
  contribution_ref: contributionRef,
  id: `id:${contributionRef}`,
  lease_ref: leaseRef,
  public_projection_json: '{"state":"pending"}',
  pylon_device_ref: 'device.worker.contract',
  pylon_ref: 'pylon.contract',
  replay_digest_ref: null,
  sampled_window_end_step: 32,
  sampled_window_ref: 'sampled.window.1',
  sampled_window_start_step: 0,
  state: 'pending',
  submitted_at: '2026-07-04T12:00:00.000Z',
  trace_commitment_digest_ref: `digest.${contributionRef}`,
  training_run_ref: 'run.contract.shared',
  updated_at: '2026-07-04T12:00:00.000Z',
  validator_device_ref: null,
  verification_challenge_ref: null,
  window_ref: 'window.contract.shared',
  worker_receipt_ref: `receipt.worker.${contributionRef}`,
  workload_family: 'executor-trace',
  ...overrides,
})

type ContractHarness = Readonly<{
  store: TrainingDomainWriteStore
  /** Portable read-only SQL (SELECT …) against the same store's tables. */
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

// ---------------------------------------------------------------------------
// The shared behavioral spec
// ---------------------------------------------------------------------------

const specContractSuite = (getHarness: () => ContractHarness) => {
  test('runs converge on training_run_ref to the latest authoritative row', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('run')
    expect(await store.upsertRows('training_runs', [runRow(runRef)])).toBe(1)
    // A transition read back from D1 converges byte-exactly — including
    // the receipt refs that feed public claims.
    await store.upsertRows('training_runs', [
      runRow(runRef, {
        public_projection_json: '{"state":"active"}',
        receipt_refs_json: '["receipt.plan.1","receipt.activate.1"]',
        state: 'active',
        updated_at: '2026-07-04T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT state, receipt_refs_json FROM training_runs WHERE training_run_ref = '${runRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.state).toBe('active')
    expect(rows[0]?.receipt_refs_json).toBe(
      '["receipt.plan.1","receipt.activate.1"]',
    )
  })

  test('windows converge on window_ref; seal metadata round-trips byte-exactly', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('run')
    const windowRef = nextRef('window')
    await store.upsertRows('training_runs', [runRow(runRef)])
    await store.upsertRows('training_windows', [windowRow(windowRef, runRef)])
    const sealMetadata =
      '{"staleness":{"maxAllowedStale":5},"churn":{"events":[]}}'
    await store.upsertRows('training_windows', [
      windowRow(windowRef, runRef, {
        seal_metadata_json: sealMetadata,
        sealed_at: '2026-07-04T14:00:00.000Z',
        state: 'sealed',
        updated_at: '2026-07-04T14:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT state, seal_metadata_json FROM training_windows WHERE window_ref = '${windowRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.state).toBe('sealed')
    expect(rows[0]?.seal_metadata_json).toBe(sealMetadata)
  })

  test('window events: fresh inserts land; exact replay is a no-op that never clobbers', async () => {
    const { query, store } = getHarness()
    const windowRef = nextRef('window')
    const events = [
      windowEventRow(`${windowRef}:evt:1`, windowRef, {
        state_from: null,
        state_to: 'planned',
        transition_kind: 'plan',
      }),
      windowEventRow(`${windowRef}:evt:2`, windowRef, {
        created_at: '2026-07-04T12:01:00.000Z',
      }),
    ]
    expect(await store.upsertRows('training_window_events', events)).toBe(2)
    // Exact replay: id PK dedupe, no clobber.
    expect(
      await store.upsertRows('training_window_events', [
        windowEventRow(`${windowRef}:evt:1`, windowRef, {
          state_to: 'CLOBBERED',
        }),
      ]),
    ).toBe(0)
    const rows = await query(
      `SELECT id, state_to FROM training_window_events WHERE window_ref = '${windowRef}' ORDER BY created_at, id`,
    )
    expect(rows.map(row => row.state_to)).toEqual(['planned', 'active'])
  })

  test('leases converge on lease_ref — a replayed claim never duplicates', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('run')
    const windowRef = nextRef('window')
    const leaseRef = nextRef('lease')
    expect(
      await store.upsertRows('training_window_leases', [
        leaseRow(leaseRef, windowRef, runRef),
      ]),
    ).toBe(1)
    // Release converges the same row (state flip), never a second lease.
    await store.upsertRows('training_window_leases', [
      leaseRow(leaseRef, windowRef, runRef, { state: 'released' }),
    ])
    const rows = await query(
      `SELECT state FROM training_window_leases WHERE window_ref = '${windowRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.state).toBe('released')
  })

  test('verification challenges converge on challenge_ref; event ledger stays contiguous', async () => {
    const { query, store } = getHarness()
    const runRef = nextRef('run')
    const challengeRef = nextRef('challenge')
    await store.upsertRows('training_verification_challenges', [
      challengeRow(challengeRef, runRef),
    ])
    await store.upsertRows('training_verification_events', [
      verificationEventRow(`${challengeRef}:evt:1`, challengeRef),
    ])
    // The lease + finalize transitions converge the challenge and append
    // links — replaying the whole batch is a no-op for the ledger.
    const finalized = challengeRow(challengeRef, runRef, {
      state: 'Verified',
      updated_at: '2026-07-04T12:05:00.000Z',
      verdict_refs_json: '["verdict.replay.1"]',
      verified_at: '2026-07-04T12:05:00.000Z',
    })
    const links = [
      verificationEventRow(`${challengeRef}:evt:2`, challengeRef, {
        created_at: '2026-07-04T12:04:00.000Z',
        state_from: 'Queued',
        state_to: 'Leased',
        transition_kind: 'lease',
      }),
      verificationEventRow(`${challengeRef}:evt:3`, challengeRef, {
        created_at: '2026-07-04T12:05:00.000Z',
        state_from: 'Leased',
        state_to: 'Verified',
        transition_kind: 'finalize',
        validator_ref: 'validator.device.contract',
      }),
    ]
    await store.upsertRows('training_verification_challenges', [finalized])
    expect(
      await store.upsertRows('training_verification_events', links),
    ).toBe(2)
    expect(
      await store.upsertRows('training_verification_events', links),
    ).toBe(0)

    const challenge = await query(
      `SELECT state, verdict_refs_json FROM training_verification_challenges WHERE challenge_ref = '${challengeRef}'`,
    )
    expect(challenge[0]?.state).toBe('Verified')
    expect(challenge[0]?.verdict_refs_json).toBe('["verdict.replay.1"]')
    const chain = await query(
      `SELECT state_to FROM training_verification_events WHERE challenge_ref = '${challengeRef}' ORDER BY created_at, id`,
    )
    expect(chain.map(row => row.state_to)).toEqual([
      'Queued',
      'Leased',
      'Verified',
    ])
  })

  test('trace contributions: (lease_ref, workload_family) REJECTS a conflicting second row on BOTH stores', async () => {
    const { query, store } = getHarness()
    const leaseRef = nextRef('lease')
    const contributionRef = nextRef('contribution')
    await store.upsertRows('training_trace_contributions', [
      contributionRow(contributionRef, leaseRef),
    ])

    // Pairing converges the SAME contribution.
    await store.upsertRows('training_trace_contributions', [
      contributionRow(contributionRef, leaseRef, {
        replay_digest_ref: 'digest.replay.1',
        state: 'paired',
        updated_at: '2026-07-04T12:10:00.000Z',
        validator_device_ref: 'device.validator.contract',
        verification_challenge_ref: 'challenge.paired.1',
      }),
    ])
    const rows = await query(
      `SELECT state, validator_device_ref FROM training_trace_contributions WHERE lease_ref = '${leaseRef}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.state).toBe('paired')

    // A DIFFERENT contribution under the same (lease_ref, workload_family)
    // must be rejected — the exact D1 idempotency key (one pending worker
    // contribution per lease+family; payout correctness rides on this).
    await expect(
      store.upsertRows('training_trace_contributions', [
        contributionRow(nextRef('contribution'), leaseRef),
      ]),
    ).rejects.toThrow()
  })
}

// ---------------------------------------------------------------------------
// D1 (SQLite) harness
// ---------------------------------------------------------------------------

describe('training domain repository contract — D1 (SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(TRAINING_DOMAIN_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite.db.prepare(sql).all<Record<string, unknown>>()).results ??
        [],
      store: makeD1TrainingDomainWriteStore(sqlite.db),
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

const MIGRATION_0019 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0019_training_domain.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'training domain repository contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE training_domain_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('training_domain_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0019, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresTrainingDomainStore({
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

describe('training domain seam', () => {
  test('flags: dual-write defaults ON, reads default d1, typos never fail open', () => {
    expect(trainingFlagsFromEnv({})).toEqual({ dualWrite: true, reads: 'd1' })
    expect(
      trainingFlagsFromEnv({ KHALA_SYNC_TRAINING_DUAL_WRITE: 'off' }).dualWrite,
    ).toBe(false)
    expect(
      trainingFlagsFromEnv({ KHALA_SYNC_TRAINING_DUAL_WRITE: '0' }).dualWrite,
    ).toBe(false)
    expect(
      trainingFlagsFromEnv({ KHALA_SYNC_TRAINING_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      trainingFlagsFromEnv({ KHALA_SYNC_TRAINING_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      trainingFlagsFromEnv({ KHALA_SYNC_TRAINING_READS: 'postgrse' }).reads,
    ).toBe('d1')
  })

  test('dual-write: a Postgres mirror failure NEVER fails the D1 write; the drift metric fires with refs only', async () => {
    const written: Array<string> = []
    const logged: Array<{
      event: TrainingDiagnosticEvent
      fields: TrainingDiagnostic
    }> = []
    const d1: TrainingDomainWriteStore = {
      upsertRows: async (table, rows) => {
        written.push(`${table}:${String(rows.length)}`)
        return rows.length
      },
    }
    const failing: TrainingDomainWriteStore = {
      upsertRows: () =>
        Promise.reject(new Error('secret-bearing postgres detail')),
    }
    const store = makeDualWriteTrainingDomainWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1' },
      log: (event, fields) => logged.push({ event, fields }),
      postgres: failing,
    })

    const outcome = await store.upsertRows('training_runs', [
      runRow('run.failsoft.1'),
    ])
    expect(outcome).toBe(1)
    expect(written).toEqual(['training_runs:1'])
    expect(logged).toHaveLength(1)
    expect(logged[0]?.event).toBe('khala_sync_training_dual_write_failed')
    expect(logged[0]?.fields.refs).toEqual(['run.failsoft.1'])
    // Keys only — never projection payloads.
    expect(JSON.stringify(logged[0]?.fields)).not.toContain('projection')
  })

  test('dual-write disabled or missing binding degrades to plain D1', async () => {
    const d1: TrainingDomainWriteStore = {
      upsertRows: async (_table, rows) => rows.length,
    }
    const neverCalled: TrainingDomainWriteStore = {
      upsertRows: () => Promise.reject(new Error('must not be called')),
    }
    const offStore = makeDualWriteTrainingDomainWriteStore({
      d1,
      flags: { dualWrite: false, reads: 'd1' },
      postgres: neverCalled,
    })
    expect(
      await offStore.upsertRows('training_runs', [runRow('run.off.1')]),
    ).toBe(1)
    const unboundStore = makeDualWriteTrainingDomainWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1' },
      postgres: undefined,
    })
    expect(
      await unboundStore.upsertRows('training_runs', [runRow('run.off.2')]),
    ).toBe(1)
  })

  test('read-back mirror: reads the authoritative D1 rows by ref and upserts them; failures are swallowed', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(TRAINING_DOMAIN_D1_SCHEMA)
    try {
      const runRef = nextRef('run')
      await makeD1TrainingDomainWriteStore(sqlite.db).upsertRows(
        'training_runs',
        [runRow(runRef, { state: 'active' })],
      )

      const mirroredRows: Array<TrainingDomainRow> = []
      const logged: Array<TrainingDiagnosticEvent> = []
      const mirror = makeTrainingDomainMirror({
        db: sqlite.db,
        log: event => logged.push(event),
        postgres: {
          listClaimableWindowRows: () => Promise.resolve([]),
          listVerificationChallengeRecordsForRun: () => Promise.resolve([]),
          listWindowLeaseRecordsForRun: () => Promise.resolve([]),
          listWindowRecordsForRun: () => Promise.resolve([]),
          readRunRecord: () => Promise.resolve(undefined),
          upsertRows: async (_table, rows) => {
            mirroredRows.push(...rows)
            return rows.length
          },
        },
      })
      await mirror.mirrorRowsByRef('training_runs', [runRef])
      expect(mirroredRows).toHaveLength(1)
      expect(mirroredRows[0]?.['training_run_ref']).toBe(runRef)
      expect(mirroredRows[0]?.['state']).toBe('active')
      expect(logged).toEqual([])

      // A failing Postgres side never throws out of the mirror.
      const failingMirror = makeTrainingDomainMirror({
        db: sqlite.db,
        log: event => logged.push(event),
        postgres: {
          listClaimableWindowRows: () => Promise.resolve([]),
          listVerificationChallengeRecordsForRun: () => Promise.resolve([]),
          listWindowLeaseRecordsForRun: () => Promise.resolve([]),
          listWindowRecordsForRun: () => Promise.resolve([]),
          readRunRecord: () => Promise.resolve(undefined),
          upsertRows: () => Promise.reject(new Error('down')),
        },
      })
      await expect(
        failingMirror.mirrorRowsByRef('training_runs', [runRef]),
      ).resolves.toBeUndefined()
      expect(logged).toEqual(['khala_sync_training_dual_write_failed'])
    } finally {
      sqlite.close()
    }
  })
})
