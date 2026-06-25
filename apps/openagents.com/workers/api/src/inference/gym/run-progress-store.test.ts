import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { buildGymRunProgress, type GymRunProgress } from './run-progress'
import { makeD1GymRunProgressStore } from './run-progress-store'

// Minimal in-memory fake of the subset of D1 the store uses (prepare/bind/run/
// all). Models a single keyed table with ON CONFLICT(run_ref) upsert semantics so
// the store's upsert-by-runRef and ordered list can be exercised without a real
// D1 binding.
type Row = Readonly<{
  run_ref: string
  progress_json: string
  last_updated_at: string
  ingested_at: string
  created_at: string
}>

const makeFakeD1 = (): D1Database & { rows: Array<Row> } => {
  const rows: Array<Row> = []

  const statement = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => null as T | null,
      all: async <T,>() => ({
        meta: {} as D1Meta & Record<string, unknown>,
        results: [...rows]
          .sort((a, b) =>
            a.last_updated_at > b.last_updated_at
              ? -1
              : a.last_updated_at < b.last_updated_at
                ? 1
                : a.run_ref < b.run_ref
                  ? -1
                  : 1,
          ) as unknown as Array<T>,
        success: true as const,
      }),
      run: async <T,>() => {
        const [run_ref, progress_json, last_updated_at, ingested_at, created_at] =
          bound as [string, string, string, string, string]
        const index = rows.findIndex(r => r.run_ref === run_ref)
        const next: Row = {
          created_at,
          ingested_at,
          last_updated_at,
          progress_json,
          run_ref,
        }
        if (index === -1) {
          rows.push(next)
        } else {
          // ON CONFLICT keeps the original created_at.
          rows[index] = { ...next, created_at: rows[index]!.created_at }
        }
        return {
          meta: { changes: 1 } as D1Meta & Record<string, unknown>,
          results: [] as unknown as Array<T>,
          success: true as const,
        }
      },
      raw: async () => [] as never,
    }
    void query
    return stmt
  }

  return {
    prepare: (query: string) => statement(query),
    batch: async () => [] as never,
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    withSession: () => {
      throw new Error('not implemented')
    },
    rows,
  } as unknown as D1Database & { rows: Array<Row> }
}

const makeProgress = (
  overrides: Partial<Parameters<typeof buildGymRunProgress>[0] & object> = {},
): GymRunProgress =>
  buildGymRunProgress({
    runRef: 'run.gym.terminal_bench.store.test',
    jobRef: 'job.gym.harbor_terminal_bench.store.test',
    configId: 'gym.terminal_bench.store.test',
    profileRef: 'khala-public-heuristic',
    agent: 'opencode',
    phase: 'running',
    publication: 'web_authorized',
    officialDenominator: 89,
    completedPassed: 9,
    completedFailed: 3,
    running: 2,
    pending: 75,
    error: 0,
    cancelled: 0,
    promptTokens: null,
    completionTokens: null,
    elapsedMs: 540_000,
    lastUpdatedAt: '2026-06-25T00:00:00.000Z',
    caveatRefs: [],
    blockerRefs: [],
    ...overrides,
  })

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

describe('D1 gym run-progress store', () => {
  test('lists [] before any ingest', async () => {
    const store = makeD1GymRunProgressStore(makeFakeD1())
    const runs = await run(store.listRunProgress())
    expect(runs).toEqual([])
  })

  test('upserts a snapshot and serves it back', async () => {
    const store = makeD1GymRunProgressStore(makeFakeD1())
    const progress = makeProgress()
    await run(store.upsertRunProgress(progress))
    const runs = await run(store.listRunProgress())
    expect(runs).toHaveLength(1)
    expect(runs[0]?.runRef).toBe(progress.runRef)
    expect(runs[0]?.counts.completed).toBe(12)
    expect(runs[0]?.passRateOverCompleted).toBeCloseTo(9 / 12)
  })

  test('upsert by runRef replaces the prior snapshot (no duplicate row)', async () => {
    const db = makeFakeD1()
    const store = makeD1GymRunProgressStore(db)
    await run(store.upsertRunProgress(makeProgress()))
    await run(
      store.upsertRunProgress(
        makeProgress({
          completedPassed: 13,
          completedFailed: 5,
          running: 1,
          pending: 70,
          lastUpdatedAt: '2026-06-25T00:10:00.000Z',
        }),
      ),
    )
    expect(db.rows).toHaveLength(1)
    const runs = await run(store.listRunProgress())
    expect(runs).toHaveLength(1)
    expect(runs[0]?.counts.completed).toBe(18)
    expect(runs[0]?.lastUpdatedAt).toBe('2026-06-25T00:10:00.000Z')
  })

  test('drops a tampered/legacy row on read instead of serving it', async () => {
    const db = makeFakeD1()
    const store = makeD1GymRunProgressStore(db)
    db.rows.push({
      created_at: '2026-06-25T00:00:00.000Z',
      ingested_at: '2026-06-25T00:00:00.000Z',
      last_updated_at: '2026-06-25T00:00:00.000Z',
      progress_json: JSON.stringify({ not: 'a run progress object' }),
      run_ref: 'run.gym.tampered',
    })
    const runs = await run(store.listRunProgress())
    expect(runs).toEqual([])
  })
})
