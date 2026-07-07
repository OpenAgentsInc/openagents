// KS-8.15 (#8326): training domain store — ForEnv factories: fail-soft
// read-back mirroring through the REAL D1 stores, degradation without a
// binding, and the routed listClaimableWindows read.
//
// Load-bearing properties:
//   * KHALA_SYNC_TRAINING_DUAL_WRITE defaults ON; unknown read modes fall
//     back to 'd1' (never fail open into an unproven read path);
//   * every authority/verification/contribution write mirrors the exact
//     D1 rows to Postgres AFTER the authoritative D1 write; a Postgres
//     failure NEVER fails the request — it emits
//     `khala_sync_training_dual_write_failed` with row KEYS only;
//   * with no KHALA_SYNC_DB binding the factories return the plain D1
//     stores (zero Postgres calls);
//   * listClaimableWindows routes per KHALA_SYNC_TRAINING_READS — the
//     SelfServeWindowProducer.topUp read this domain re-homes: compare
//     serves D1 and logs divergence; postgres serves Postgres with
//     bounded retry and falls back to D1 on exhaustion.

import { describe, expect, test } from 'vitest'

import {
  makeTrainingAuthorityStoreForEnv,
  makeTrainingTraceContributionStoreForEnv,
  makeTrainingVerificationStoreForEnv,
  type TrainingDiagnostic,
  type TrainingDiagnosticEvent,
  type TrainingStoreEnv,
} from './training-domain-store'
import type {
  TrainingRunRecord,
  TrainingWindowEventRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import { TRAINING_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

type LogEntry = Readonly<{
  event: TrainingDiagnosticEvent
  fields: TrainingDiagnostic
}>

const makeLogRecorder = () => {
  const entries: Array<LogEntry> = []
  return {
    entries,
    log: (event: TrainingDiagnosticEvent, fields: TrainingDiagnostic) => {
      entries.push({ event, fields })
    },
  }
}

/**
 * A scripted SQL client for the ForEnv factories. The Postgres store uses
 * BOTH surfaces: `sql.unsafe(text, params)` (registry upserts) and the
 * tagged template (the claimable-windows scan). Executed statement heads
 * are recorded; template calls pop scripted results.
 */
const makeScriptedSqlClient = (options?: {
  templateResults?: Array<ReadonlyArray<Record<string, unknown>>>
  throwOnEveryCall?: boolean
}) => {
  const executed: Array<string> = []
  const templateResults = [...(options?.templateResults ?? [])]
  const record = (text: string) => {
    executed.push(text.replaceAll(/\s+/g, ' ').trim())
  }
  const sql = (strings: TemplateStringsArray, ..._values: Array<unknown>) => {
    record(strings.join('?'))
    if (options?.throwOnEveryCall === true) {
      return Promise.reject(new Error('pg down'))
    }
    return Promise.resolve(templateResults.shift() ?? [])
  }
  ;(sql as unknown as { unsafe: unknown }).unsafe = (
    text: string,
    _params: Array<unknown>,
  ) => {
    record(text)
    if (options?.throwOnEveryCall === true) {
      return Promise.reject(new Error('pg down'))
    }
    return Promise.resolve([{ touched: 1 }])
  }
  return {
    executed,
    makeSqlClient: async (_connectionString: string) => ({
      end: () => Promise.resolve(),
      sql: sql as never,
    }),
  }
}

const makeEnv = (
  db: D1Database,
  vars: Partial<Record<string, string>> = {},
  withBinding = true,
): TrainingStoreEnv =>
  ({
    OPENAGENTS_DB: db,
    ...(withBinding
      ? { KHALA_SYNC_DB: { connectionString: 'postgres://scripted/test' } }
      : {}),
    ...vars,
  }) as TrainingStoreEnv

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.unit.${++refCounter}`

const runRecord = (runRef: string): TrainingRunRecord => ({
  createdAt: '2026-07-04T12:00:00.000Z',
  id: `id:${runRef}`,
  manifest: null,
  maxAllowedStale: 5,
  promiseRef: 'promise.decentralized-training-launch',
  publicProjectionJson: '{"state":"planned"}',
  receiptRefs: ['receipt.plan.1'],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ['issue.unit'],
  state: 'planned',
  trainingRunRef: runRef,
  updatedAt: '2026-07-04T12:00:00.000Z',
})

const windowRecord = (
  windowRef: string,
  runRef: string,
): TrainingWindowRecord => ({
  activatedAt: null,
  datasetRefs: ['dataset.smoke'],
  homeworkKind: 'auto_starter',
  id: `id:${windowRef}`,
  plannedAt: '2026-07-04T12:00:00.000Z',
  priority: 1,
  publicProjectionJson: '{"state":"planned"}',
  receiptRefs: ['receipt.window.plan'],
  reconciledAt: null,
  sealMetadata: null,
  sealedAt: null,
  sourceRefs: ['issue.unit'],
  state: 'planned',
  trainingRunRef: runRef,
  updatedAt: '2026-07-04T12:00:00.000Z',
  windowRef,
})

const windowEventRecord = (
  windowRef: string,
): TrainingWindowEventRecord => ({
  actorRef: 'operator.owner',
  createdAt: '2026-07-04T12:01:00.000Z',
  id: nextRef('event'),
  receiptRef: 'receipt.window.activate',
  stateFrom: 'planned',
  stateTo: 'active',
  transitionKind: 'activate',
  windowRef,
})

const leaseRecord = (
  leaseRef: string,
  windowRef: string,
  runRef: string,
): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-07-04T12:02:00.000Z',
  id: `id:${leaseRef}`,
  leaseExpiresAt: '2026-07-04T13:02:00.000Z',
  leaseRef,
  publicProjectionJson: '{"state":"active"}',
  pylonRef: 'pylon.unit',
  receiptRefs: ['receipt.lease.claim'],
  state: 'active',
  trainingRunRef: runRef,
  windowRef,
})

const claimableWindowPgRow = (windowRef: string, runRef: string) => ({
  activated_at: '2026-07-04T12:01:00.000Z',
  archived_at: null,
  dataset_refs_json: '["dataset.smoke"]',
  homework_kind: 'auto_starter',
  id: `id:${windowRef}`,
  planned_at: '2026-07-04T12:00:00.000Z',
  priority: 1,
  public_projection_json: '{"state":"active"}',
  receipt_refs_json: '["receipt.window.plan"]',
  reconciled_at: null,
  seal_metadata_json: null,
  sealed_at: null,
  source_refs_json: '["issue.unit"]',
  state: 'active',
  training_run_ref: runRef,
  updated_at: '2026-07-04T12:01:00.000Z',
  window_ref: windowRef,
})

const withSqlite = async (
  run: (db: D1Database) => Promise<void>,
): Promise<void> => {
  const sqlite = makeSqliteD1()
  sqlite.exec(TRAINING_DOMAIN_D1_SCHEMA)
  try {
    await run(sqlite.db)
  } finally {
    sqlite.close()
  }
}

// ---------------------------------------------------------------------------
// Authority store ForEnv
// ---------------------------------------------------------------------------

describe('makeTrainingAuthorityStoreForEnv', () => {
  test('without KHALA_SYNC_DB the plain D1 store round-trips and no Postgres statement runs', async () => {
    await withSqlite(async db => {
      const scripted = makeScriptedSqlClient()
      const store = makeTrainingAuthorityStoreForEnv(
        makeEnv(db, {}, false),
        { makeSqlClient: scripted.makeSqlClient },
      )
      const runRef = nextRef('run')
      await store.planRun(runRecord(runRef))
      expect((await store.readRun(runRef))?.trainingRunRef).toBe(runRef)
      expect(scripted.executed).toEqual([])
    })
  })

  test('dual-write default ON: plan/transition/claim mirror the exact D1 rows to Postgres', async () => {
    await withSqlite(async db => {
      const scripted = makeScriptedSqlClient()
      const recorder = makeLogRecorder()
      const store = makeTrainingAuthorityStoreForEnv(makeEnv(db), {
        log: recorder.log,
        makeSqlClient: scripted.makeSqlClient,
      })

      const runRef = nextRef('run')
      const windowRef = nextRef('window')
      const leaseRef = nextRef('lease')
      await store.planRun(runRecord(runRef))
      await store.planWindow(windowRecord(windowRef, runRef))
      const activated = {
        ...windowRecord(windowRef, runRef),
        activatedAt: '2026-07-04T12:01:00.000Z',
        state: 'active' as const,
        updatedAt: '2026-07-04T12:01:00.000Z',
      }
      await store.transitionWindow(activated, windowEventRecord(windowRef))
      await store.claimLease(
        leaseRecord(leaseRef, windowRef, runRef),
        '2026-07-04T12:02:00.000Z',
      )

      const heads = scripted.executed.map(text => text.slice(0, 40))
      expect(
        heads.filter(head => head.startsWith('INSERT INTO training_runs')),
      ).toHaveLength(1)
      expect(
        heads.filter(head => head.startsWith('INSERT INTO training_windows')),
      ).toHaveLength(2)
      expect(
        heads.filter(head =>
          head.startsWith('INSERT INTO training_window_events'),
        ),
      ).toHaveLength(1)
      expect(
        heads.filter(head =>
          head.startsWith('INSERT INTO training_window_leases'),
        ),
      ).toHaveLength(1)
      expect(recorder.entries).toEqual([])

      // D1 authority reflects the transition (read-back source of truth).
      const claimable = await store.listClaimableWindows(
        '2026-07-04T14:00:00.000Z',
        10,
      )
      expect(claimable.map(record => record.windowRef)).toEqual([windowRef])
    })
  })

  test('a Postgres mirror failure NEVER fails the write and logs keys only', async () => {
    await withSqlite(async db => {
      const scripted = makeScriptedSqlClient({ throwOnEveryCall: true })
      const recorder = makeLogRecorder()
      const store = makeTrainingAuthorityStoreForEnv(makeEnv(db), {
        log: recorder.log,
        makeSqlClient: scripted.makeSqlClient,
      })

      const runRef = nextRef('run')
      await expect(store.planRun(runRecord(runRef))).resolves.toBeDefined()
      expect((await store.readRun(runRef))?.trainingRunRef).toBe(runRef)
      expect(recorder.entries).toHaveLength(1)
      expect(recorder.entries[0]?.event).toBe(
        'khala_sync_training_dual_write_failed',
      )
      expect(recorder.entries[0]?.fields.refs).toEqual([runRef])
      expect(JSON.stringify(recorder.entries[0]?.fields)).not.toContain(
        'projection',
      )
    })
  })

  test('reads=compare serves D1 and logs divergence; reads=postgres serves Postgres and falls back on exhaustion', async () => {
    await withSqlite(async db => {
      const runRef = nextRef('run')
      const windowRef = nextRef('window')

      // compare: scripted Postgres disagrees (empty result) — serve D1, log.
      {
        const scripted = makeScriptedSqlClient({ templateResults: [[]] })
        const recorder = makeLogRecorder()
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, { KHALA_SYNC_TRAINING_READS: 'compare' }),
          { log: recorder.log, makeSqlClient: scripted.makeSqlClient },
        )
        await store.planRun(runRecord(runRef))
        const window = {
          ...windowRecord(windowRef, runRef),
          state: 'active' as const,
        }
        await store.planWindow(window)
        const served = await store.listClaimableWindows(
          '2026-07-04T14:00:00.000Z',
          10,
        )
        expect(served.map(record => record.windowRef)).toEqual([windowRef])
        expect(
          recorder.entries.some(
            entry =>
              entry.event === 'khala_sync_training_read_compare_mismatch',
          ),
        ).toBe(true)
      }

      // postgres: scripted rows are served without touching D1's ranking.
      {
        const pgWindowRef = nextRef('window')
        const scripted = makeScriptedSqlClient({
          templateResults: [[claimableWindowPgRow(pgWindowRef, runRef)]],
        })
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, { KHALA_SYNC_TRAINING_READS: 'postgres' }),
          { makeSqlClient: scripted.makeSqlClient, wait: () => Promise.resolve() },
        )
        const served = await store.listClaimableWindows(
          '2026-07-04T14:00:00.000Z',
          10,
        )
        expect(served.map(record => record.windowRef)).toEqual([pgWindowRef])
        expect(served[0]?.priority).toBe(1)
      }

      // postgres exhaustion: bounded retries then D1 fallback + fallback log.
      {
        const scripted = makeScriptedSqlClient({ throwOnEveryCall: true })
        const recorder = makeLogRecorder()
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, {
            KHALA_SYNC_TRAINING_DUAL_WRITE: 'off',
            KHALA_SYNC_TRAINING_READS: 'postgres',
          }),
          {
            log: recorder.log,
            makeSqlClient: scripted.makeSqlClient,
            wait: () => Promise.resolve(),
          },
        )
        const served = await store.listClaimableWindows(
          '2026-07-04T14:00:00.000Z',
          10,
        )
        expect(served.map(record => record.windowRef)).toEqual([windowRef])
        expect(
          recorder.entries.filter(
            entry => entry.event === 'khala_sync_training_postgres_read_failed',
          ),
        ).toHaveLength(2)
        expect(
          recorder.entries.filter(
            entry =>
              entry.event === 'khala_sync_training_postgres_read_fallback',
          ),
        ).toHaveLength(1)
      }
    })
  })

  // CFG D1 evacuation (#8515): the public run-detail READ set behind
  // `GET /api/training/runs/:id` (readRun + listWindowsForRun +
  // listWindowLeasesForRun + listVerificationChallengesForRun). On the dead
  // D1 bridge these still-live D1 reads 500 the route; under reads=postgres
  // they must serve from Postgres, coerce int8 columns back to numbers, and
  // fall back to D1 (never 500) on Postgres exhaustion.
  test('reads=postgres serves run-detail reads from Postgres (int8-coerced) and falls back to D1 on exhaustion', async () => {
    await withSqlite(async db => {
      const runRef = nextRef('run')
      const windowRef = nextRef('window')

      // Serve readRun from Postgres — max_allowed_stale arrives as a STRING
      // (postgres.js int8 shape) and must coerce to the number the mapper +
      // route decoder expect.
      {
        const runPgRow = {
          archived_at: null,
          created_at: '2026-07-04T12:00:00.000Z',
          id: `id:${runRef}`,
          manifest_json: null,
          max_allowed_stale: '7',
          promise_ref: 'promise.decentralized-training-launch',
          public_projection_json: '{"state":"planned"}',
          receipt_refs_json: '["receipt.plan.1"]',
          seal_in_flight_at: null,
          seal_publication_cadence_windows: '2',
          source_refs_json: '["issue.unit"]',
          state: 'planned',
          training_run_ref: runRef,
          updated_at: '2026-07-04T12:00:00.000Z',
        }
        const scripted = makeScriptedSqlClient({
          templateResults: [[runPgRow]],
        })
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, {
            KHALA_SYNC_TRAINING_DUAL_WRITE: 'off',
            KHALA_SYNC_TRAINING_READS: 'postgres',
          }),
          {
            makeSqlClient: scripted.makeSqlClient,
            wait: () => Promise.resolve(),
          },
        )
        const served = await store.readRun(runRef)
        expect(served?.trainingRunRef).toBe(runRef)
        expect(served?.maxAllowedStale).toBe(7)
        expect(served?.sealPublicationCadenceWindows).toBe(2)
      }

      // Serve listWindowsForRun from Postgres — priority coerces to a number.
      {
        const scripted = makeScriptedSqlClient({
          templateResults: [[claimableWindowPgRow(windowRef, runRef)]],
        })
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, {
            KHALA_SYNC_TRAINING_DUAL_WRITE: 'off',
            KHALA_SYNC_TRAINING_READS: 'postgres',
          }),
          {
            makeSqlClient: scripted.makeSqlClient,
            wait: () => Promise.resolve(),
          },
        )
        const served = await store.listWindowsForRun(runRef, 100)
        expect(served.map(record => record.windowRef)).toEqual([windowRef])
        expect(served[0]?.priority).toBe(1)
      }

      // Exhaustion: readRun retries then falls back to the D1 authority (which
      // holds the row) rather than 500-ing, and logs the fallback.
      {
        const seedScripted = makeScriptedSqlClient()
        const seedStore = makeTrainingAuthorityStoreForEnv(makeEnv(db), {
          makeSqlClient: seedScripted.makeSqlClient,
        })
        await seedStore.planRun(runRecord(runRef))

        const scripted = makeScriptedSqlClient({ throwOnEveryCall: true })
        const recorder = makeLogRecorder()
        const store = makeTrainingAuthorityStoreForEnv(
          makeEnv(db, {
            KHALA_SYNC_TRAINING_DUAL_WRITE: 'off',
            KHALA_SYNC_TRAINING_READS: 'postgres',
          }),
          {
            log: recorder.log,
            makeSqlClient: scripted.makeSqlClient,
            wait: () => Promise.resolve(),
          },
        )
        const served = await store.readRun(runRef)
        expect(served?.trainingRunRef).toBe(runRef)
        expect(
          recorder.entries.some(
            entry =>
              entry.event === 'khala_sync_training_postgres_read_fallback' &&
              entry.fields.op === 'readRun',
          ),
        ).toBe(true)
      }
    })
  })
})

// ---------------------------------------------------------------------------
// Verification + contribution ForEnv
// ---------------------------------------------------------------------------

describe('makeTrainingVerificationStoreForEnv / makeTrainingTraceContributionStoreForEnv', () => {
  test('createChallenge mirrors the challenge AND its ledger event; contribution submit mirrors the stored row idempotently', async () => {
    await withSqlite(async db => {
      const scripted = makeScriptedSqlClient()
      const recorder = makeLogRecorder()

      const verification = makeTrainingVerificationStoreForEnv(makeEnv(db), {
        log: recorder.log,
        makeSqlClient: scripted.makeSqlClient,
      })
      const challengeRef = nextRef('challenge')
      await verification.createChallenge(
        {
          attemptCount: 0,
          challengeRef,
          commitmentRefs: ['commitment.1'],
          contributionRef: null,
          createdAt: '2026-07-04T12:00:00.000Z',
          failureCodes: [],
          homeworkKind: 'auto_starter',
          id: `id:${challengeRef}`,
          leaseExpiresAt: null,
          leaseRef: null,
          leasedToRef: null,
          maxAttempts: 3,
          payloadJson: '{"kind":"replay"}',
          publicProjectionJson: '{"state":"Queued"}',
          rejectedAt: null,
          samplingPolicy: 'per_contribution',
          state: 'Queued',
          timedOutAt: null,
          trainingRunRef: 'run.unit.shared',
          updatedAt: '2026-07-04T12:00:00.000Z',
          verdictRefs: [],
          verificationClass: 'exact_trace_replay',
          verifiedAt: null,
          windowRef: null,
        } as never,
        {
          challengeRef,
          createdAt: '2026-07-04T12:00:00.000Z',
          failureCodes: [],
          id: `id:${challengeRef}:evt:1`,
          receiptRefs: [],
          stateFrom: null,
          stateTo: 'Queued',
          transitionKind: 'create',
          validatorRef: null,
        } as never,
      )
      const heads = scripted.executed.map(text => text.slice(0, 60))
      expect(
        heads.filter(head =>
          head.startsWith('INSERT INTO training_verification_challenges'),
        ),
      ).toHaveLength(1)
      expect(
        heads.filter(head =>
          head.startsWith('INSERT INTO training_verification_events'),
        ),
      ).toHaveLength(1)

      const contributions = makeTrainingTraceContributionStoreForEnv(
        makeEnv(db),
        { log: recorder.log, makeSqlClient: scripted.makeSqlClient },
      )
      const leaseRef = nextRef('lease')
      const record = {
        assignmentRef: 'assignment.unit',
        contributionRef: nextRef('contribution'),
        id: `id:${leaseRef}`,
        leaseRef,
        publicProjectionJson: '{"state":"pending"}',
        pylonDeviceRef: 'device.worker.unit',
        pylonRef: 'pylon.unit',
        replayDigestRef: null,
        sampledWindow: { endStep: 32, startStep: 0 },
        sampledWindowRef: 'sampled.window.unit',
        state: 'pending',
        submittedAt: '2026-07-04T12:00:00.000Z',
        traceCommitmentDigestRef: 'digest.unit.1',
        trainingRunRef: 'run.unit.shared',
        updatedAt: '2026-07-04T12:00:00.000Z',
        validatorDeviceRef: null,
        verificationChallengeRef: null,
        windowRef: 'window.unit.shared',
        workerReceiptRef: 'receipt.worker.unit',
        workloadFamily: 'executor-trace',
      }
      const stored = await contributions.recordWorkerContribution(
        record as never,
      )
      // A retried submission (new contribution_ref, same lease+family)
      // re-reads and re-mirrors the ORIGINAL stored row.
      const retried = await contributions.recordWorkerContribution({
        ...record,
        contributionRef: nextRef('contribution'),
        id: `id:${leaseRef}:retry`,
      } as never)
      expect(retried.contributionRef).toBe(stored.contributionRef)
      expect(
        scripted.executed.filter(text =>
          text.startsWith('INSERT INTO training_trace_contributions'),
        ),
      ).toHaveLength(2)
      expect(recorder.entries).toEqual([])
    })
  })
})
