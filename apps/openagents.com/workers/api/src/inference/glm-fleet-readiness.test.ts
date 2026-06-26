import { describe, expect, test } from 'vitest'

import type { GlmPoolHeartbeatReplicaRecord } from './glm-pool-heartbeat'
import {
  projectGlmFleetReadiness,
  projectGlmFleetReadinessForEnv,
} from './glm-fleet-readiness'
import { resolveHydraliskGlm52Reap504bReplicaArmings } from './model-serving-policy'

const observedAt = '2026-06-26T12:00:00.000Z'

const heartbeat = (
  replicaId: string,
  overrides: Partial<GlmPoolHeartbeatReplicaRecord> = {},
): GlmPoolHeartbeatReplicaRecord => ({
  benchmarkReserved: false,
  breakerConsecutiveFailures: 0,
  breakerConsecutiveSuccesses: 0,
  breakerFailureThreshold: 3,
  breakerReadmitSuccessThreshold: 2,
  draining: false,
  healthStatus: 'ok',
  keepWarmStatus: 'completed',
  modelsStatus: 'ok',
  observedAt,
  replicaId,
  replicaRef: `replica.hydralisk.glm_52_reap_504b.${replicaId}`,
  runRef: 'heartbeat.hydralisk.glm_52_reap_504b.fixture',
  totalWallClockMs: 25,
  usage: {
    completionTokens: 1,
    promptTokens: 6,
    totalTokens: 7,
  },
  warmCompletionStatus: 'ok',
  warmState: 'warm',
  watchdogStatus: 'healthy',
  ...overrides,
})

const readyEnv = {
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS:
    'warm-one,ready-two,reclaimed-three,disabled-four,missing-five',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_BASE_URL:
    'https://warm-one.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_BEARER_TOKEN: 'secret-warm-one',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_PREFLIGHT_REF:
    'preflight.hydralisk.glm.warm_one',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_RECEIPT_REF:
    'receipt.hydralisk.glm.warm_one',
  HYDRALISK_GLM_52_REAP_504B_READY_TWO_BASE_URL:
    'https://ready-two.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_READY_TWO_BEARER_TOKEN: 'secret-ready-two',
  HYDRALISK_GLM_52_REAP_504B_READY_TWO_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_READY_TWO_PREFLIGHT_REF:
    'preflight.hydralisk.glm.ready_two',
  HYDRALISK_GLM_52_REAP_504B_READY_TWO_RECEIPT_REF:
    'receipt.hydralisk.glm.ready_two',
  HYDRALISK_GLM_52_REAP_504B_RECLAIMED_THREE_BASE_URL:
    'https://reclaimed-three.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_RECLAIMED_THREE_BEARER_TOKEN:
    'secret-reclaimed-three',
  HYDRALISK_GLM_52_REAP_504B_RECLAIMED_THREE_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_RECLAIMED_THREE_PREFLIGHT_REF:
    'preflight.hydralisk.glm.reclaimed_three',
  HYDRALISK_GLM_52_REAP_504B_RECLAIMED_THREE_RECEIPT_REF:
    'receipt.hydralisk.glm.reclaimed_three',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_BASE_URL:
    'https://disabled-four.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_BEARER_TOKEN:
    'secret-disabled-four',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_PREFLIGHT_REF:
    'preflight.hydralisk.glm.disabled_four',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_RECEIPT_REF:
    'receipt.hydralisk.glm.disabled_four',
  HYDRALISK_GLM_52_REAP_504B_DISABLED_FOUR_DRAINING: 'true',
} as const

describe('projectGlmFleetReadiness', () => {
  test('projects configured GLM replicas into stable aggregate health buckets', () => {
    const latest = new Map([
      ['warm-one', heartbeat('warm-one')],
      [
        'ready-two',
        heartbeat('ready-two', {
          keepWarmStatus: 'control_plane_only',
          warmCompletionStatus: 'skipped',
          warmState: 'unknown',
        }),
      ],
      [
        'reclaimed-three',
        heartbeat('reclaimed-three', {
          healthStatus: 'failed',
          keepWarmStatus: 'failed',
          modelsStatus: 'failed',
          warmCompletionStatus: 'failed',
          warmState: 'cold',
          watchdogStatus: 'unhealthy',
        }),
      ],
    ])

    const projection = projectGlmFleetReadinessForEnv(readyEnv, replicaId =>
      latest.get(replicaId),
    )

    expect(projection.status).toBe('degraded')
    expect(projection.configuredReplicaRefs).toEqual([
      'replica.hydralisk.glm_52_reap_504b.disabled-four',
      'replica.hydralisk.glm_52_reap_504b.missing-five',
      'replica.hydralisk.glm_52_reap_504b.ready-two',
      'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
      'replica.hydralisk.glm_52_reap_504b.warm-one',
    ])
    expect(projection.replicas).toEqual([
      {
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.disabled-four',
        status: 'disabled',
      },
      {
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.missing-five',
        status: 'unavailable',
      },
      {
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.ready-two',
        status: 'ready',
      },
      {
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
        status: 'reclaimed',
      },
      {
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.warm-one',
        status: 'warm',
      },
    ])
    expect(projection.counts).toEqual({
      disabledReplicaCount: 1,
      readyReplicaCount: 1,
      reclaimedReplicaCount: 1,
      totalReplicaCount: 5,
      unavailableReplicaCount: 1,
      warmReplicaCount: 1,
    })
  })

  test('counts unarmed route-disabled and missing replicas without secret fields', () => {
    const env = {
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'disabled,missing',
      HYDRALISK_GLM_52_REAP_504B_DISABLED_BASE_URL:
        'https://disabled.private.example.test',
      HYDRALISK_GLM_52_REAP_504B_DISABLED_BEARER_TOKEN: 'secret-disabled',
      HYDRALISK_GLM_52_REAP_504B_DISABLED_ENABLED: 'off',
      HYDRALISK_GLM_52_REAP_504B_DISABLED_PREFLIGHT_REF:
        'preflight.hydralisk.glm.disabled',
      HYDRALISK_GLM_52_REAP_504B_DISABLED_RECEIPT_REF:
        'receipt.hydralisk.glm.disabled',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, () => undefined)

    expect(projection.status).toBe('unavailable')
    expect(projection.counts).toMatchObject({
      disabledReplicaCount: 1,
      totalReplicaCount: 2,
      unavailableReplicaCount: 1,
    })
    expect(JSON.stringify(projection)).not.toContain('private.example.test')
    expect(JSON.stringify(projection)).not.toContain('secret-disabled')
    expect(JSON.stringify(projection)).not.toContain('BASE_URL')
    expect(JSON.stringify(projection)).not.toContain('BEARER')
  })

  test('returns stable count fields even when no replicas are configured', () => {
    const projection = projectGlmFleetReadiness({
      latestHeartbeatRecord: () => undefined,
      replicaArmings: [],
    })

    expect(projection).toMatchObject({
      configuredReplicaRefs: [],
      counts: {
        disabledReplicaCount: 0,
        readyReplicaCount: 0,
        reclaimedReplicaCount: 0,
        totalReplicaCount: 0,
        unavailableReplicaCount: 0,
        warmReplicaCount: 0,
      },
      replicas: [],
      status: 'unavailable',
    })
  })

  test('uses only stable replica refs from configured armings', () => {
    const armings = resolveHydraliskGlm52Reap504bReplicaArmings(readyEnv)
    const projection = projectGlmFleetReadiness({
      latestHeartbeatRecord: replicaId => heartbeat(replicaId),
      replicaArmings: armings,
    })
    const serialized = JSON.stringify(projection)

    expect(serialized).toContain('replica.hydralisk.glm_52_reap_504b.warm-one')
    expect(serialized).not.toContain('https://')
    expect(serialized).not.toContain('secret-')
    expect(serialized).not.toContain('observedAt')
    expect(serialized).not.toContain('runRef')
  })
})
