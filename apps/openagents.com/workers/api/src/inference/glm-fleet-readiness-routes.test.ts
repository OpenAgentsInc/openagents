import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleGlmFleetReadiness,
  readPersistedGlmFleetReadinessHeartbeatRecords,
} from './glm-fleet-readiness-routes'

type HeartbeatRow = Readonly<{
  benchmark_reserved?: number | string | null
  breaker_consecutive_failures?: number | null
  breaker_consecutive_successes?: number | null
  breaker_failure_threshold?: number | null
  breaker_readmit_success_threshold?: number | null
  demand_source: string | null
  draining?: number | string | null
  health_status?: string | null
  heartbeat_kind: string | null
  heartbeat_run_ref?: string | null
  keep_warm_status?: string | null
  models_status?: string | null
  observed_at: string | null
  probe_timeout_ms?: number | null
  provider: string | null
  replica_id: string | null
  total_wall_clock_ms?: number | null
  total_tokens: number | null
  warm_completion_status?: string | null
  warm_state: string | null
  watchdog_status: string | null
}>

type FleetReadinessJson = Readonly<{
  acceptance?: Readonly<{
    allReplicaKeepWarmWatchdog?: Readonly<Record<string, unknown>>
    capacityFloorOwnerDecision?: Readonly<Record<string, unknown>>
    multiRegionAutoReplace?: Readonly<Record<string, unknown>>
    quotaRequestTracking?: Readonly<Record<string, unknown>>
    status?: string
  }>
  counts: Readonly<Record<string, number>>
  replicas?: ReadonlyArray<Record<string, unknown>>
  status: string
}>

const env = {
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BASE_URL:
    'https://primary.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BEARER_TOKEN: 'secret-primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_PREFLIGHT_REF:
    'preflight.hydralisk.glm.primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_RECEIPT_REF:
    'receipt.hydralisk.glm.primary',
} as const

const get = () =>
  new Request('https://openagents.com/v1/gateway/glm-fleet/readiness', {
    method: 'GET',
  })

const dbWithRows = (rows: ReadonlyArray<HeartbeatRow>): D1Database =>
  ({
    prepare: () => ({
      all: async () => ({ results: rows }),
    }),
  }) as unknown as D1Database

describe('handleGlmFleetReadiness', () => {
  test('404s with inference_gateway_disabled when the gateway is off', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), { enabled: false, env }),
    )

    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })

  test('405s on non-GET requests', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(
        new Request('https://openagents.com/v1/gateway/glm-fleet/readiness', {
          method: 'POST',
        }),
        { enabled: true, env },
      ),
    )

    expect(response.status).toBe(405)
    expect(await response.json()).toEqual({ error: 'method_not_allowed' })
  })

  test('returns a no-store public-safe fleet projection', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), { enabled: true, env }),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const text = await response.text()
    expect(text).toContain('replica.hydralisk.glm_52_reap_504b.primary')
    expect(text).toContain('capacityFloorOwnerDecision')
    expect(text).toContain('unavailableReplicaCount')
    expect(text).not.toContain('private.example.test')
    expect(text).not.toContain('secret-primary')
    expect(text).not.toContain('PRIMARY_BASE_URL')
  })

  test('public route reports durable fleet acceptance blockers when proof evidence is absent', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), {
        enabled: true,
        env,
        readPersistedHeartbeatRecords: async () => [
          {
            keepWarmStatus: 'control_plane_only',
            observedAt: '2026-06-26T14:00:00.000Z',
            replicaId: 'primary',
            warmState: 'unknown',
            watchdogStatus: 'healthy',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as FleetReadinessJson
    expect(body.acceptance).toMatchObject({
      allReplicaKeepWarmWatchdog: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_incomplete',
          'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing',
        ],
        coveredReplicaCount: 0,
        forcedStopRecoveryEvidenceRefs: [],
        missingReplicaRefs: [
          'replica.hydralisk.glm_52_reap_504b.primary',
        ],
        status: 'blocked',
      },
      capacityFloorOwnerDecision: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing',
        ],
        decision: 'missing',
        status: 'blocked',
      },
      multiRegionAutoReplace: {
        status: 'blocked',
      },
      quotaRequestTracking: {
        requestState: 'missing',
        status: 'blocked',
      },
      status: 'blocked',
    })
  })

  test('uses persisted heartbeat records when isolate memory has no heartbeat yet', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), {
        enabled: true,
        env,
        readPersistedHeartbeatRecords: async () => [
          {
            observedAt: '2026-06-26T14:00:00.000Z',
            replicaId: 'primary',
            warmState: 'warm',
            watchdogStatus: 'healthy',
          },
        ],
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as FleetReadinessJson
    expect(body.counts).toMatchObject({
      totalReplicaCount: 1,
      warmReplicaCount: 1,
      unavailableReplicaCount: 0,
    })
    expect(body.status).toBe('ready')
  })

  test('hydrates persisted watchdog and keep-warm evidence into the public projection', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), {
        db: dbWithRows([
          {
            benchmark_reserved: 0,
            breaker_consecutive_failures: 0,
            breaker_consecutive_successes: 0,
            breaker_failure_threshold: 3,
            breaker_readmit_success_threshold: 2,
            demand_source: 'glm-pool-heartbeat',
            draining: 0,
            health_status: 'ok',
            heartbeat_kind: 'glm_pool_heartbeat',
            heartbeat_run_ref:
              'heartbeat.hydralisk.glm_52_reap_504b.20260626t140000000z',
            keep_warm_status: 'completed',
            models_status: 'ok',
            observed_at: '2026-06-26T14:00:00.000Z',
            probe_timeout_ms: 2000,
            provider: 'hydralisk-vllm-glm-5p2-reap-504b',
            replica_id: 'primary',
            total_tokens: 7,
            total_wall_clock_ms: 123.456,
            warm_completion_status: 'ok',
            warm_state: 'warm',
            watchdog_status: 'healthy',
          },
        ]),
        enabled: true,
        env,
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as FleetReadinessJson
    expect(body.counts).toMatchObject({
      configuredMaxInflight: 1,
      warmMaxInflight: 1,
      warmOrReadyMaxInflight: 1,
      warmReplicaCount: 1,
    })
    expect(body.replicas?.[0]).toMatchObject({
      armingEvidenceRefs: [
        'preflight.hydralisk.glm.primary',
        'receipt.hydralisk.glm.primary',
      ],
      healthStatus: 'ok',
      keepWarmStatus: 'completed',
      latestHeartbeatAt: '2026-06-26T14:00:00.000Z',
      modelsStatus: 'ok',
      status: 'warm',
      totalWallClockMs: 123.456,
      warmCompletionStatus: 'ok',
      watchdogStatus: 'healthy',
    })
    expect(JSON.stringify(body)).not.toContain('private.example.test')
    expect(JSON.stringify(body)).not.toContain('secret-primary')
  })

  test('fails soft to configured replica projection when persisted read fails', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), {
        enabled: true,
        env,
        readPersistedHeartbeatRecords: async () => {
          throw new Error('d1 unavailable')
        },
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as FleetReadinessJson
    expect(body.counts).toMatchObject({
      totalReplicaCount: 1,
      unavailableReplicaCount: 1,
    })
    expect(body.status).toBe('unavailable')
  })

  test('reads production routed heartbeat completions as ready fallback evidence', async () => {
    const records = await readPersistedGlmFleetReadinessHeartbeatRecords(
      dbWithRows([
        {
          demand_source: 'glm-pool-heartbeat',
          heartbeat_kind: 'glm_pool_heartbeat',
          observed_at: '2026-06-26T14:45:55.581Z',
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          replica_id: null,
          total_tokens: 0,
          warm_state: null,
          watchdog_status: null,
        },
        {
          demand_source: 'heartbeat',
          heartbeat_kind: null,
          observed_at: '2026-06-26T14:44:55.581Z',
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          replica_id: 'primary',
          total_tokens: 28,
          warm_state: 'unknown',
          watchdog_status: null,
        },
      ]),
    )

    expect(records).toEqual([
      {
        observedAt: '2026-06-26T14:44:55.581Z',
        replicaId: 'primary',
        warmState: 'unknown',
        watchdogStatus: 'healthy',
      },
    ])
  })
})
