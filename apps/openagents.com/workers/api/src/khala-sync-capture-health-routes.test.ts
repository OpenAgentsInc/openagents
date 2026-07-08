import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { KhalaSyncSmokeSqlClient } from './khala-sync-db-smoke-routes'
import {
  KHALA_SYNC_CAPTURE_HEALTH_DEFAULT_THRESHOLD_MS,
  KHALA_SYNC_CAPTURE_HEALTH_QUERY,
  KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF,
  KHALA_SYNC_CAPTURE_STALE_EVENT,
  captureHealthFromRows,
  evaluateCaptureHealth,
  handleKhalaSyncCaptureHealth,
  runKhalaSyncCaptureStalenessProbe,
} from './khala-sync-capture-health-routes'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@10.1.2.3:5432/khala_sync_prod'

const THRESHOLD = KHALA_SYNC_CAPTURE_HEALTH_DEFAULT_THRESHOLD_MS

// ---- pure threshold logic ------------------------------------------------

describe('evaluateCaptureHealth', () => {
  test('healthy: fresh checkpoint with a draining backlog', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 3,
      maxUpdatedAtMs: 100_000,
      nowMs: 105_000, // 5s stale, well under threshold
      scopesBehind: 1,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 12,
    })
    expect(snapshot.status).toBe('healthy')
    expect(snapshot.stalenessMs).toBe(5_000)
  })

  test('healthy: idle system (no backlog) even when updated_at is ancient', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 3,
      maxUpdatedAtMs: 0,
      nowMs: 10 * 60 * 60 * 1000, // 10h stale
      scopesBehind: 0,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 0,
    })
    expect(snapshot.status).toBe('healthy')
  })

  test('stale: backlog present AND staleness over threshold (stuck-but-running)', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 5,
      maxUpdatedAtMs: 0,
      nowMs: 32 * 60 * 60 * 1000, // the incident: 32h with backlog
      scopesBehind: 5,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 20_000,
    })
    expect(snapshot.status).toBe('stale')
    expect(snapshot.stalenessMs).toBeGreaterThan(THRESHOLD)
    expect(snapshot.versionsUndelivered).toBe(20_000)
  })

  test('healthy: backlog present but exactly at threshold is not yet stale', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 2,
      maxUpdatedAtMs: 0,
      nowMs: THRESHOLD, // staleness === threshold (not strictly greater)
      scopesBehind: 1,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 1,
    })
    expect(snapshot.status).toBe('healthy')
  })

  test('stale: backlog present but NO checkpoint has ever been written', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 0,
      maxUpdatedAtMs: null,
      nowMs: 1_000,
      scopesBehind: 4,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 40,
    })
    expect(snapshot.status).toBe('stale')
    expect(snapshot.stalenessMs).toBeNull()
  })

  test('staleness clamps to >= 0 under clock skew', () => {
    const snapshot = evaluateCaptureHealth({
      checkpointCount: 1,
      maxUpdatedAtMs: 200_000,
      nowMs: 100_000, // now BEFORE last update
      scopesBehind: 0,
      thresholdMs: THRESHOLD,
      versionsUndelivered: 0,
    })
    expect(snapshot.stalenessMs).toBe(0)
  })
})

// ---- row folding ---------------------------------------------------------

describe('captureHealthFromRows', () => {
  test('folds a healthy DB row (bigints as strings) into a snapshot', () => {
    const snapshot = captureHealthFromRows(
      [
        {
          checkpoint_count: '4',
          db_now_epoch: 1_000_000,
          max_updated_at_epoch: 999_998, // 2s ago
          scopes_behind: '0',
          versions_undelivered: '0',
        },
      ],
      THRESHOLD,
    )
    expect(snapshot.status).toBe('healthy')
    expect(snapshot.stalenessMs).toBe(2_000)
    expect(snapshot.checkpointCount).toBe(4)
  })

  test('folds a stalled DB row into a stale snapshot', () => {
    const snapshot = captureHealthFromRows(
      [
        {
          checkpoint_count: '6',
          db_now_epoch: 2_000_000,
          max_updated_at_epoch: 1_990_000, // 10_000s ago
          scopes_behind: '6',
          versions_undelivered: '18000',
        },
      ],
      THRESHOLD,
    )
    expect(snapshot.status).toBe('stale')
    expect(snapshot.versionsUndelivered).toBe(18_000)
  })

  test('null max_updated_at_epoch (no checkpoints) parses without throwing', () => {
    const snapshot = captureHealthFromRows(
      [
        {
          checkpoint_count: '0',
          db_now_epoch: 1_000,
          max_updated_at_epoch: null,
          scopes_behind: '2',
          versions_undelivered: '5',
        },
      ],
      THRESHOLD,
    )
    expect(snapshot.status).toBe('stale')
    expect(snapshot.stalenessMs).toBeNull()
  })

  test('throws on a malformed row rather than reporting a false healthy', () => {
    expect(() => captureHealthFromRows([{}], THRESHOLD)).toThrow()
  })
})

// ---- route ---------------------------------------------------------------

const makeFakeClient = (
  rows: ReadonlyArray<Record<string, unknown>>,
  opts: { queryError?: Error } = {},
) => {
  let ended = 0
  const client: KhalaSyncSmokeSqlClient = {
    end: () => {
      ended += 1
      return Promise.resolve()
    },
    query: (text) => {
      expect(text).toBe(KHALA_SYNC_CAPTURE_HEALTH_QUERY)
      return opts.queryError === undefined
        ? Promise.resolve(rows)
        : Promise.reject(opts.queryError)
    },
  }
  return { client, endedCount: () => ended }
}

const getRequest = () =>
  new Request('https://openagents.com/api/internal/khala-sync/capture-health')

describe('handleKhalaSyncCaptureHealth', () => {
  test('401 when the admin bearer check fails', async () => {
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(getRequest(), {
        binding: { connectionString: FAKE_CONNECTION_STRING },
        requireOperator: () => Promise.resolve(false),
      }),
    )
    expect(response.status).toBe(401)
  })

  test('honest ok:false when the binding is absent', async () => {
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(getRequest(), {
        binding: undefined,
        requireOperator: () => Promise.resolve(true),
      }),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(200)
    expect(body.ok).toBe(false)
    expect(body.routeRef).toBe(KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF)
  })

  test('healthy path returns ok:true, status healthy, and closes the client', async () => {
    const fake = makeFakeClient([
      {
        checkpoint_count: '3',
        db_now_epoch: 1_000_000,
        max_updated_at_epoch: 999_997,
        scopes_behind: '0',
        versions_undelivered: '0',
      },
    ])
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(getRequest(), {
        binding: { connectionString: FAKE_CONNECTION_STRING },
        makeSqlClient: () => Promise.resolve(fake.client),
        requireOperator: () => Promise.resolve(true),
      }),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(200)
    expect(body.ok).toBe(true)
    expect(body.status).toBe('healthy')
    expect(fake.endedCount()).toBe(1)
  })

  test('stale DB reports status stale (still ok:true — the probe ran)', async () => {
    const fake = makeFakeClient([
      {
        checkpoint_count: '6',
        db_now_epoch: 2_000_000,
        max_updated_at_epoch: 1_980_000,
        scopes_behind: '6',
        versions_undelivered: '20000',
      },
    ])
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(getRequest(), {
        binding: { connectionString: FAKE_CONNECTION_STRING },
        makeSqlClient: () => Promise.resolve(fake.client),
        requireOperator: () => Promise.resolve(true),
      }),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.ok).toBe(true)
    expect(body.status).toBe('stale')
    expect(body.versionsUndelivered).toBe(20_000)
  })

  test('query failure -> 503 with a redacted reason (no connection details)', async () => {
    const fake = makeFakeClient([], {
      queryError: new Error(
        `connect ECONNREFUSED ${FAKE_CONNECTION_STRING} at 10.1.2.3:5432`,
      ),
    })
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(getRequest(), {
        binding: { connectionString: FAKE_CONNECTION_STRING },
        makeSqlClient: () => Promise.resolve(fake.client),
        requireOperator: () => Promise.resolve(true),
      }),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(response.status).toBe(503)
    expect(body.ok).toBe(false)
    expect(String(body.reason)).not.toContain('10.1.2.3')
    expect(String(body.reason)).not.toContain('secret')
    expect(fake.endedCount()).toBe(1)
  })

  test('POST is rejected as method-not-allowed', async () => {
    const response = await Effect.runPromise(
      handleKhalaSyncCaptureHealth(
        new Request(
          'https://openagents.com/api/internal/khala-sync/capture-health',
          { method: 'POST' },
        ),
        {
          binding: { connectionString: FAKE_CONNECTION_STRING },
          requireOperator: () => Promise.resolve(true),
        },
      ),
    )
    expect(response.status).toBe(405)
  })
})

// ---- scheduled probe -----------------------------------------------------

describe('runKhalaSyncCaptureStalenessProbe', () => {
  test('emits the khala_sync_capture_stale warning exactly once on a stall', async () => {
    const fake = makeFakeClient([
      {
        checkpoint_count: '6',
        db_now_epoch: 2_000_000,
        max_updated_at_epoch: 1_980_000,
        scopes_behind: '6',
        versions_undelivered: '20000',
      },
    ])
    const emitted: Array<Record<string, unknown>> = []
    const snapshot = await runKhalaSyncCaptureStalenessProbe({
      binding: { connectionString: FAKE_CONNECTION_STRING },
      emitStructuredLog: (line) => emitted.push(line),
      makeSqlClient: () => Promise.resolve(fake.client),
    })
    expect(snapshot?.status).toBe('stale')
    expect(emitted).toHaveLength(1)
    expect(emitted[0]?.event).toBe(KHALA_SYNC_CAPTURE_STALE_EVENT)
    expect(emitted[0]?.severity).toBe('WARNING')
    expect(emitted[0]?.versionsUndelivered).toBe(20_000)
    expect(fake.endedCount()).toBe(1)
  })

  test('stays silent on a healthy tick', async () => {
    const fake = makeFakeClient([
      {
        checkpoint_count: '3',
        db_now_epoch: 1_000_000,
        max_updated_at_epoch: 999_998,
        scopes_behind: '0',
        versions_undelivered: '0',
      },
    ])
    const emitted: Array<Record<string, unknown>> = []
    const snapshot = await runKhalaSyncCaptureStalenessProbe({
      binding: { connectionString: FAKE_CONNECTION_STRING },
      emitStructuredLog: (line) => emitted.push(line),
      makeSqlClient: () => Promise.resolve(fake.client),
    })
    expect(snapshot?.status).toBe('healthy')
    expect(emitted).toHaveLength(0)
  })

  test('no binding: returns null and emits nothing', async () => {
    const emitted: Array<Record<string, unknown>> = []
    const snapshot = await runKhalaSyncCaptureStalenessProbe({
      binding: undefined,
      emitStructuredLog: (line) => emitted.push(line),
    })
    expect(snapshot).toBeNull()
    expect(emitted).toHaveLength(0)
  })

  test('fail-soft: a query error emits a probe_error, never throws, closes client', async () => {
    const fake = makeFakeClient([], {
      queryError: new Error('boom at 10.1.2.3'),
    })
    const stale: Array<Record<string, unknown>> = []
    const errors: Array<string> = []
    const snapshot = await runKhalaSyncCaptureStalenessProbe({
      binding: { connectionString: FAKE_CONNECTION_STRING },
      emitProbeError: (message) => errors.push(message),
      emitStructuredLog: (line) => stale.push(line),
      makeSqlClient: () => Promise.resolve(fake.client),
    })
    expect(snapshot).toBeNull()
    expect(stale).toHaveLength(0)
    expect(errors).toHaveLength(1)
    expect(errors[0]).not.toContain('10.1.2.3')
    expect(fake.endedCount()).toBe(1)
  })
})
