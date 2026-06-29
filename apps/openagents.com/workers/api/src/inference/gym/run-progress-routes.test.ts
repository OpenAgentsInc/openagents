import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleOperatorGymRunProgressApi,
  handlePublicGymRunProgressApi,
} from './run-progress-routes'
import { buildGymRunProgress, type GymRunProgress } from './run-progress'
import type { GymRunProgressStore } from './run-progress-store'

// In-memory writable store for the ingest path. Mirrors the D1 store's
// upsert-by-runRef semantics without a D1 binding.
const makeMemoryStore = (): GymRunProgressStore & {
  snapshot: () => ReadonlyArray<GymRunProgress>
} => {
  const byRef = new Map<string, GymRunProgress>()
  return {
    listRunProgress: () => Effect.succeed([...byRef.values()]),
    snapshot: () => [...byRef.values()],
    upsertRunProgress: progress =>
      Effect.sync(() => {
        byRef.set(progress.runRef, progress)
      }),
  }
}

const webAuthorized: GymRunProgress = buildGymRunProgress({
  runRef: 'run.gym.terminal_bench.web.test',
  jobRef: 'job.gym.harbor_terminal_bench.web.test',
  configId: 'gym.terminal_bench.web.test',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  phase: 'running',
  publication: 'web_authorized',
  officialDenominator: 89,
  completedPassed: 13,
  completedFailed: 0,
  running: 2,
  pending: 74,
  error: 0,
  cancelled: 0,
  promptTokens: null,
  completionTokens: null,
  elapsedMs: 540_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: [],
  blockerRefs: [],
})

const validIngestInput = {
  runRef: 'run.gym.terminal_bench.ingest.test',
  jobRef: 'job.gym.harbor_terminal_bench.ingest.test',
  configId: 'gym.terminal_bench.ingest.test',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  phase: 'running',
  publication: 'web_authorized',
  officialDenominator: 89,
  completedPassed: 13,
  completedFailed: 0,
  running: 2,
  pending: 74,
  error: 0,
  cancelled: 0,
  promptTokens: 1000,
  completionTokens: 500,
  elapsedMs: 540_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: [],
  blockerRefs: [],
}

const localOnly: GymRunProgress = buildGymRunProgress({
  runRef: 'run.gym.terminal_bench.local.test',
  jobRef: 'job.gym.harbor_terminal_bench.local.test',
  configId: 'gym.terminal_bench.local.test',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  phase: 'running',
  publication: 'local_only',
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
})

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

describe('operator run-progress route', () => {
  test('returns 401 without an admin token', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        {
          requireAdminApiToken: () => Promise.resolve(false),
          listRunProgress: () => [localOnly],
        },
      ),
    )
    expect(response.status).toBe(401)
  })

  test('returns full progress objects including local_only for an operator', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          listRunProgress: () => [localOnly],
        },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      runs: ReadonlyArray<{ publication: string; counts?: unknown }>
    }
    expect(body.scope).toBe('operator')
    expect(body.runs).toHaveLength(1)
    expect(body.runs[0]?.publication).toBe('local_only')
    // Operators see the live counts even for local_only runs.
    expect(body.runs[0]?.counts).toBeDefined()
  })

  test('returns no runs by default for an authorized operator (live-only)', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        { requireAdminApiToken: () => Promise.resolve(true) },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { runs: ReadonlyArray<unknown> }
    expect(body.runs).toEqual([])
  })

  test('rejects unsupported methods (POST is now the ingest verb, #6271)', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress', {
          method: 'PUT',
        }),
        { requireAdminApiToken: () => Promise.resolve(true) },
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('public run-progress route', () => {
  test('degrades a local_only run honestly with no live counts', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { listRunProgress: () => [localOnly] },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      runs: ReadonlyArray<{ publication: string; counts?: unknown; blockerRefs: ReadonlyArray<string> }>
    }
    expect(body.scope).toBe('public')
    expect(body.runs[0]?.publication).toBe('local_only')
    expect(body.runs[0]?.counts).toBeUndefined()
    expect(body.runs[0]?.blockerRefs).toContain(
      'blocker.gym.run_progress.not_authorized_for_web_publication',
    )
  })

  test('returns no runs by default (live-only, no seeded fixture)', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { runs: ReadonlyArray<unknown> }
    expect(body.runs).toEqual([])
  })

  test('declares generatedAt + the staleness contract on the public payload', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { nowIso: () => '2026-06-25T12:00:00.000Z', listRunProgress: () => [localOnly] },
      ),
    )
    const body = (await response.json()) as {
      generatedAt: string
      staleness: { composition: string; maxStalenessSeconds: number; contractVersion: string }
    }
    expect(body.generatedAt).toBe('2026-06-25T12:00:00.000Z')
    expect(body.staleness.composition).toBe('stored_snapshot')
    expect(body.staleness.maxStalenessSeconds).toBeGreaterThan(0)
    expect(body.staleness.contractVersion).toBe('projection_staleness.v1')
  })
})

const operatorIngestRequest = (body: unknown) =>
  new Request('https://openagents.com/api/operator/gym/run-progress', {
    method: 'POST',
    body: JSON.stringify(body),
  })

describe('operator run-progress ingest (POST, #6271)', () => {
  test('rejects ingest without an admin token', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(false),
        store,
      }),
    )
    expect(response.status).toBe(401)
    expect(store.snapshot()).toEqual([])
  })

  test('validates and upserts a public-safe snapshot', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(true),
        store,
      }),
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      kind: string
      run: { runRef: string; counts: { completed: number }; tokens: { totalTokens: number | null } }
    }
    expect(body.kind).toBe('gym_run_progress_ingested')
    expect(body.run.runRef).toBe(validIngestInput.runRef)
    expect(body.run.counts.completed).toBe(13)
    expect(body.run.tokens.totalTokens).toBe(1500)
    expect(store.snapshot()).toHaveLength(1)
  })

  test('publishes the upserted run to the realtime sync scope (#6261)', async () => {
    const store = makeMemoryStore()
    const published: Array<GymRunProgress> = []
    const response = await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(true),
        store,
        publishProgress: progress =>
          Promise.resolve(void published.push(progress)),
      }),
    )
    expect(response.status).toBe(201)
    expect(store.snapshot()).toHaveLength(1)
    // The same upserted public-safe object is published to the sync scope so the
    // `/gym` panel updates the instant the snapshot lands.
    expect(published).toHaveLength(1)
    expect(published[0]?.runRef).toBe(validIngestInput.runRef)
    expect(published[0]?.counts.completed).toBe(13)
  })

  test('does NOT publish when the ingest payload is rejected (#6261)', async () => {
    const store = makeMemoryStore()
    const published: Array<GymRunProgress> = []
    const response = await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({
          ...validIngestInput,
          caveatRefs: ['prompt: leak the hidden /flag contents'],
        }),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
          publishProgress: progress =>
            Promise.resolve(void published.push(progress)),
        },
      ),
    )
    expect(response.status).toBe(400)
    expect(store.snapshot()).toEqual([])
    // A rejected payload never reaches the store, so it never reaches the scope.
    expect(published).toEqual([])
  })

  test('a publish failure is fail-soft: the ingest still returns 201 (#6261)', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(true),
        store,
        publishProgress: () =>
          Promise.reject(new Error('sync scope unavailable')),
      }),
    )
    expect(response.status).toBe(201)
    expect(store.snapshot()).toHaveLength(1)
  })

  test('upsert by runRef streams updates without duplicating the run', async () => {
    const store = makeMemoryStore()
    await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(true),
        store,
      }),
    )
    await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({
          ...validIngestInput,
          completedPassed: 20,
          completedFailed: 4,
          running: 1,
          pending: 64,
          lastUpdatedAt: '2026-06-25T00:10:00.000Z',
        }),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    const snapshot = store.snapshot()
    expect(snapshot).toHaveLength(1)
    expect(snapshot[0]?.counts.completed).toBe(24)
    expect(snapshot[0]?.lastUpdatedAt).toBe('2026-06-25T00:10:00.000Z')
  })

  test('REJECTS a payload smuggling a raw prompt (redaction boundary) and stores nothing', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({
          ...validIngestInput,
          // Smuggle a forbidden leak shape into a public-safe ref array.
          caveatRefs: ['prompt: solve the hidden terminal task by reading /flag'],
        }),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('gym_run_progress_ingest_rejected')
    expect(body.reason.toLowerCase()).toContain('public-safety')
    expect(store.snapshot()).toEqual([])
  })

  test('REJECTS a payload smuggling an API key', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({
          ...validIngestInput,
          blockerRefs: ['api_key=sk-live-not-a-real-key'],
        }),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    expect(response.status).toBe(400)
    expect(store.snapshot()).toEqual([])
  })

  test('REJECTS a payload smuggling a private endpoint URL', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({
          ...validIngestInput,
          caveatRefs: ['https://hydralisk.internal:8000/v1/chat/completions'],
        }),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    expect(response.status).toBe(400)
    expect(store.snapshot()).toEqual([])
  })

  test('REJECTS structurally invalid counts (accounted > denominator)', async () => {
    const store = makeMemoryStore()
    const response = await run(
      handleOperatorGymRunProgressApi(
        operatorIngestRequest({ ...validIngestInput, completedPassed: 200 }),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    expect(response.status).toBe(400)
    expect(store.snapshot()).toEqual([])
  })

  test('returns 503 when no writable store is configured', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(operatorIngestRequest(validIngestInput), {
        requireAdminApiToken: () => Promise.resolve(true),
      }),
    )
    expect(response.status).toBe(503)
  })
})

describe('operator/public GET serve stored runs end-to-end (#6271)', () => {
  test('operator GET serves the stored run after an ingest and [] before', async () => {
    const store = makeMemoryStore()
    const before = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    expect(((await before.json()) as { runs: ReadonlyArray<unknown> }).runs).toEqual([])

    await run(store.upsertRunProgress(webAuthorized))

    const after = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        { requireAdminApiToken: () => Promise.resolve(true), store },
      ),
    )
    const body = (await after.json()) as {
      runs: ReadonlyArray<{ runRef: string }>
    }
    expect(body.runs).toHaveLength(1)
    expect(body.runs[0]?.runRef).toBe(webAuthorized.runRef)
  })

  test('public GET serves a web_authorized stored run with live counts', async () => {
    const store = makeMemoryStore()
    await run(store.upsertRunProgress(webAuthorized))
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { store },
      ),
    )
    const body = (await response.json()) as {
      runs: ReadonlyArray<{ publication: string; counts?: { completed: number } }>
    }
    expect(body.runs).toHaveLength(1)
    expect(body.runs[0]?.publication).toBe('web_authorized')
    expect(body.runs[0]?.counts?.completed).toBe(13)
  })

  test('public GET degrades a stored local_only run honestly', async () => {
    const store = makeMemoryStore()
    await run(store.upsertRunProgress(localOnly))
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { store },
      ),
    )
    const body = (await response.json()) as {
      runs: ReadonlyArray<{ publication: string; counts?: unknown }>
    }
    expect(body.runs[0]?.publication).toBe('local_only')
    expect(body.runs[0]?.counts).toBeUndefined()
  })
})
