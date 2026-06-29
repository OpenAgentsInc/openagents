import {
  CollectionName,
  CursorGap,
  EntityId,
  IsoTimestamp,
  SyncPatch,
  SyncScope,
  SyncSequence,
} from '@openagentsinc/sync-schema'
import { describe, expect, test } from 'vitest'

import {
  GYM_RUN_PROGRESS_SCOPE,
  GYM_RUN_PROGRESS_SYNC_COLLECTION,
  applyGymRunProgressPatch,
  gymRunProgressAfterSnapshot,
  gymRunProgressStreamAfterCursorGap,
  gymRunProgressStreamFailed,
  gymRunProgressStreamOpen,
} from './runProgressFeed'
import type { GymRunProgressPublicProjection } from './runProgress'
import {
  type PublicGymRunProgressModel,
  IdlePublicGymRunProgress,
  LoadedPublicGymRunProgress,
  initGymRunProgressStreamModel,
} from '../model'

// A public-safe `web_authorized` projection matching the client run-progress
// mirror schema (counts / pass-rate over completed / freshness only).
const webRun = (
  runRef: string,
  overrides: Partial<{
    completedPassed: number
    completedFailed: number
    running: number
    pending: number
    lastUpdatedAt: string
  }> = {},
): GymRunProgressPublicProjection => {
  const completedPassed = overrides.completedPassed ?? 13
  const completedFailed = overrides.completedFailed ?? 0
  const completed = completedPassed + completedFailed
  return {
    schemaVersion: 'openagents.gym.run_progress.v1',
    runRef,
    jobRef: `job.${runRef}`,
    configId: `config.${runRef}`,
    environmentRef: 'terminal-bench',
    datasetRef: 'terminal-bench@2.0',
    runner: 'harbor',
    agent: 'opencode',
    profile: {
      profileRef: 'khala-public-heuristic',
      publicLabel: 'Khala heuristic public route',
      model: 'openagents/khala',
      attribution: 'OpenAgents Khala orchestrator',
      hardwareProfile: 'khala-router',
      contextWindowTokens: 250_000,
    },
    phase: 'running',
    decisionGrade: false,
    inProgress: true,
    publication: 'web_authorized',
    counts: {
      officialDenominator: 89,
      completed,
      completedPassed,
      completedFailed,
      running: overrides.running ?? 2,
      pending: overrides.pending ?? 74,
      error: 0,
      cancelled: 0,
    },
    passRateOverCompleted: completed === 0 ? null : completedPassed / completed,
    completionFraction: completed / 89,
    tokens: { promptTokens: null, completionTokens: null, totalTokens: null },
    elapsedMs: 540_000,
    lastUpdatedAt: overrides.lastUpdatedAt ?? '2026-06-25T00:00:00.000Z',
    caveatRefs: [],
    blockerRefs: [],
  }
}

const runPatch = (
  input: Readonly<{ run: GymRunProgressPublicProjection; seq: number }>,
): SyncPatch =>
  new SyncPatch({
    scope: SyncScope.make(GYM_RUN_PROGRESS_SCOPE),
    seq: SyncSequence.make(input.seq),
    collection: CollectionName.make(GYM_RUN_PROGRESS_SYNC_COLLECTION),
    op: 'put',
    id: EntityId.make(input.run.runRef),
    value: input.run,
    serverTime: IsoTimestamp.make('2026-06-25T00:00:01.000Z'),
  })

const loaded = (
  runs: ReadonlyArray<GymRunProgressPublicProjection>,
): PublicGymRunProgressModel => LoadedPublicGymRunProgress({ runs })

describe('gymRunProgressAfterSnapshot', () => {
  test('seeds the run cards and advances the cursor', () => {
    const seeded = gymRunProgressAfterSnapshot({
      counter: IdlePublicGymRunProgress(),
      cursor: 7,
      runs: [webRun('run.alpha'), webRun('run.beta')],
      stream: initGymRunProgressStreamModel(),
    })

    expect(seeded.counter._tag).toBe('PublicGymRunProgressLoaded')
    expect(
      seeded.counter._tag === 'PublicGymRunProgressLoaded'
        ? seeded.counter.runs.map(run => run.runRef)
        : [],
    ).toStrictEqual(['run.alpha', 'run.beta'])
    expect(seeded.stream.cursor).toBe(7)
  })

  test('an empty snapshot seeds an honest empty Loaded panel', () => {
    const seeded = gymRunProgressAfterSnapshot({
      counter: IdlePublicGymRunProgress(),
      cursor: 0,
      runs: [],
      stream: initGymRunProgressStreamModel(),
    })

    expect(seeded.counter).toStrictEqual(LoadedPublicGymRunProgress({ runs: [] }))
  })
})

describe('applyGymRunProgressPatch', () => {
  test('upserts a run by runRef: replaces that card, keeps the others', () => {
    const applied = applyGymRunProgressPatch({
      counter: loaded([webRun('run.alpha'), webRun('run.beta')]),
      patch: runPatch({
        run: webRun('run.alpha', { completedPassed: 20, pending: 67 }),
        seq: 5,
      }),
      stream: initGymRunProgressStreamModel(),
    })

    const runs =
      applied.counter._tag === 'PublicGymRunProgressLoaded'
        ? applied.counter.runs
        : []
    // Same number of runs (no dup), alpha updated in place, beta untouched.
    expect(runs.map(run => run.runRef)).toStrictEqual(['run.alpha', 'run.beta'])
    const alpha = runs.find(run => run.runRef === 'run.alpha')
    expect(
      alpha?.publication === 'web_authorized'
        ? alpha.counts.completedPassed
        : null,
    ).toBe(20)
    expect(applied.stream.cursor).toBe(5)
  })

  test('a new runRef is appended (single card per run)', () => {
    const applied = applyGymRunProgressPatch({
      counter: loaded([webRun('run.alpha')]),
      patch: runPatch({ run: webRun('run.gamma'), seq: 3 }),
      stream: initGymRunProgressStreamModel(),
    })

    const runs =
      applied.counter._tag === 'PublicGymRunProgressLoaded'
        ? applied.counter.runs
        : []
    expect(runs.map(run => run.runRef)).toStrictEqual(['run.alpha', 'run.gamma'])
  })

  test('a replayed patch (same runRef@seq) is a no-op (de-dup, no double)', () => {
    const patch = runPatch({
      run: webRun('run.alpha', { completedPassed: 20 }),
      seq: 5,
    })
    const first = applyGymRunProgressPatch({
      counter: loaded([webRun('run.alpha')]),
      patch,
      stream: initGymRunProgressStreamModel(),
    })
    const second = applyGymRunProgressPatch({
      counter: first.counter,
      patch,
      stream: first.stream,
    })

    const firstRuns =
      first.counter._tag === 'PublicGymRunProgressLoaded'
        ? first.counter.runs
        : []
    const secondRuns =
      second.counter._tag === 'PublicGymRunProgressLoaded'
        ? second.counter.runs
        : []
    expect(secondRuns).toStrictEqual(firstRuns)
    expect(secondRuns).toHaveLength(1)
  })

  test('a push that races ahead of the seed still creates a Loaded card', () => {
    const applied = applyGymRunProgressPatch({
      counter: IdlePublicGymRunProgress(),
      patch: runPatch({ run: webRun('run.alpha'), seq: 2 }),
      stream: initGymRunProgressStreamModel(),
    })

    expect(applied.counter._tag).toBe('PublicGymRunProgressLoaded')
    expect(
      applied.counter._tag === 'PublicGymRunProgressLoaded'
        ? applied.counter.runs.map(run => run.runRef)
        : [],
    ).toStrictEqual(['run.alpha'])
  })

  test('the cursor advances monotonically (never moves backward)', () => {
    const stream = { ...initGymRunProgressStreamModel(), cursor: 10 }
    const applied = applyGymRunProgressPatch({
      counter: loaded([webRun('run.alpha')]),
      patch: runPatch({ run: webRun('run.alpha'), seq: 4 }),
      stream,
    })

    expect(applied.stream.cursor).toBe(10)
  })

  test('a patch on an unknown collection only advances the cursor', () => {
    const applied = applyGymRunProgressPatch({
      counter: loaded([webRun('run.alpha')]),
      patch: new SyncPatch({
        scope: SyncScope.make(GYM_RUN_PROGRESS_SCOPE),
        seq: SyncSequence.make(9),
        collection: CollectionName.make('some_other_collection'),
        op: 'put',
        id: EntityId.make('whatever'),
        value: { runRef: 'run.alpha' },
        serverTime: IsoTimestamp.make('2026-06-25T00:00:01.000Z'),
      }),
      stream: initGymRunProgressStreamModel(),
    })

    expect(applied.counter).toStrictEqual(loaded([webRun('run.alpha')]))
    expect(applied.stream.cursor).toBe(9)
  })
})

describe('stream connection reducers', () => {
  test('open / failed set the connection state', () => {
    expect(gymRunProgressStreamOpen(initGymRunProgressStreamModel()).connection).toBe(
      'open',
    )
    expect(
      gymRunProgressStreamFailed(initGymRunProgressStreamModel()).connection,
    ).toBe('failed')
  })

  test('a cursor gap advances the cursor for the next reconnect', () => {
    const gapped = gymRunProgressStreamAfterCursorGap(
      initGymRunProgressStreamModel(),
      new CursorGap({
        scope: SyncScope.make(GYM_RUN_PROGRESS_SCOPE),
        expectedSeq: SyncSequence.make(5),
        receivedSeq: SyncSequence.make(12),
      }),
    )

    expect(gapped.cursor).toBe(12)
  })
})
