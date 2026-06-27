import { describe, expect, test } from 'vitest'

import type { GlmPoolHeartbeatReplicaRecord } from './glm-pool-heartbeat'
import {
  buildGlmFleetDurabilityOperatorBundle,
  formatGlmFleetDurabilityOperatorReadme,
} from './glm-fleet-durability-operator'
import { projectGlmFleetReadinessForEnv } from './glm-fleet-readiness'

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
  observedAt: '2026-06-26T12:00:00.000Z',
  probeTimeoutMs: 2_000,
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

const baseEnv = {
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'warm-one',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_BASE_URL:
    'https://warm-one.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_BEARER_TOKEN: 'secret-warm-one',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_PREFLIGHT_REF:
    'preflight.hydralisk.glm.warm_one',
  HYDRALISK_GLM_52_REAP_504B_WARM_ONE_RECEIPT_REF:
    'receipt.hydralisk.glm.warm_one',
} as const

describe('GLM fleet durability operator bundle', () => {
  test('surfaces missing owner-only #6311 inputs without leaking private material', () => {
    const projection = projectGlmFleetReadinessForEnv(baseEnv, replicaId =>
      heartbeat(replicaId),
    )
    const bundle = buildGlmFleetDurabilityOperatorBundle({
      generatedAt: '2026-06-26T17:00:00.000Z',
      outputDir: '/Users/christopherdavid/private/ignored',
      projection,
      readinessUrl: 'https://openagents.com/v1/gateway/glm-fleet/readiness',
    })
    const serialized = JSON.stringify(bundle)

    expect(bundle.schemaVersion).toBe(
      'openagents.khala.glm_fleet_durability_operator_bundle.v1',
    )
    expect(bundle.readiness.acceptanceStatus).toBe('blocked')
    expect(bundle.readiness.counts).toMatchObject({
      totalReplicaCount: 1,
      warmReplicaCount: 1,
    })
    expect(bundle.missingOperatorInputs.map(input => input.env)).toEqual([
      'HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION',
      'HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS',
      'HYDRALISK_GLM_52_REAP_504B_<REPLICA>_BENCHMARK_RESERVED',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS',
      'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE',
    ])
    expect(
      bundle.readiness.operatorActionItems.map(item => item.action),
    ).toEqual([
      'record_forced_stop_recovery_evidence',
      'record_capacity_floor_owner_decision',
      'record_multi_region_auto_replace_evidence',
      'record_quota_request_tracking',
    ])
    expect(bundle.ownerArmedCommand).toContain(
      '.pilot-evidence/glm-fleet-durability-6311',
    )
    expect(bundle.ownerArmedCommand).not.toContain('/Users/christopherdavid')
    expect(serialized).not.toContain('private.example.test')
    expect(serialized).not.toContain('secret-warm-one')
  })

  test('accepts complete public-safe durability evidence', () => {
    const env = {
      ...baseEnv,
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'warm-one,reserve-two',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_BASE_URL:
        'https://reserve-two.private.example.test',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_BEARER_TOKEN:
        'secret-reserve-two',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_ENABLED: 'ready',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_BENCHMARK_RESERVED: 'true',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_PREFLIGHT_REF:
        'preflight.hydralisk.glm.reserve_two',
      HYDRALISK_GLM_52_REAP_504B_RESERVE_TWO_RECEIPT_REF:
        'receipt.hydralisk.glm.reserve_two',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
        'owner_accepted_all_spot',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
        'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS:
        'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS:
        'prebake.hydralisk.glm_52_reap_504b.reserve_two.20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS:
        'reserve.hydralisk.glm_52_reap_504b.reserve_two.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF:
        'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: 'approved',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId, { benchmarkReserved: replicaId === 'reserve-two' }),
    )
    const bundle = buildGlmFleetDurabilityOperatorBundle({
      generatedAt: '2026-06-26T17:05:00.000Z',
      projection,
    })
    const readme = formatGlmFleetDurabilityOperatorReadme(bundle)

    expect(bundle.readiness.acceptanceStatus).toBe('complete')
    expect(bundle.missingOperatorInputs).toEqual([])
    expect(bundle.readiness.operatorActionItems).toEqual([])
    expect(readme).toContain('- total replicas: 2')
    expect(readme).toContain('- warm replicas: 1')
    expect(readme).toContain('- none')
    expect(JSON.stringify(bundle)).not.toContain('private.example.test')
    expect(JSON.stringify(bundle)).not.toContain('secret-')
  })

  test('distinguishes recovered serving capacity from incomplete durability acceptance', () => {
    const replicaIds = [
      'ready-one',
      'ready-two',
      'ready-three',
      'ready-four',
      'ready-five',
      'ready-six',
      'ready-seven',
      'ready-eight',
      'warm-nine',
    ]
    const env = {
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: replicaIds.join(','),
      ...Object.fromEntries(
        replicaIds.flatMap(replicaId => {
          const key = replicaId.toUpperCase().replaceAll('-', '_')
          return [
            [
              `HYDRALISK_GLM_52_REAP_504B_${key}_BASE_URL`,
              `https://${replicaId}.private.example.test`,
            ],
            [
              `HYDRALISK_GLM_52_REAP_504B_${key}_BEARER_TOKEN`,
              `secret-${replicaId}`,
            ],
            [`HYDRALISK_GLM_52_REAP_504B_${key}_ENABLED`, 'ready'],
            [
              `HYDRALISK_GLM_52_REAP_504B_${key}_PREFLIGHT_REF`,
              `preflight.hydralisk.glm.${replicaId}`,
            ],
            [
              `HYDRALISK_GLM_52_REAP_504B_${key}_RECEIPT_REF`,
              `receipt.hydralisk.glm.${replicaId}`,
            ],
          ]
        }),
      ),
    }
    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId, {
        warmState: replicaId === 'warm-nine' ? 'warm' : 'cold',
      }),
    )
    const bundle = buildGlmFleetDurabilityOperatorBundle({
      generatedAt: '2026-06-27T18:00:00.000Z',
      projection,
    })
    const readme = formatGlmFleetDurabilityOperatorReadme(bundle)

    expect(bundle.readiness.acceptanceStatus).toBe('blocked')
    expect(bundle.readiness.servingStatus).toBe('ready')
    expect(bundle.readiness.servingReadyButAcceptanceNotComplete).toBe(true)
    expect(bundle.readiness.counts).toMatchObject({
      readyReplicaCount: 8,
      reclaimedReplicaCount: 0,
      warmOrReadyMaxInflight: 9,
      warmReplicaCount: 1,
    })
    expect(bundle.readiness.servingCapacitySummary).toBe(
      'serving capacity recovered: ready=8, warm=1, reclaimed=0, warmOrReadyMaxInflight=9; durability acceptance remains blocked',
    )
    expect(
      bundle.readiness.operatorActionItems.map(item => item.action),
    ).not.toContain('recover_reclaimed_replicas')
    expect(
      bundle.readiness.operatorActionItems.map(item => item.action),
    ).toEqual([
      'record_forced_stop_recovery_evidence',
      'record_capacity_floor_owner_decision',
      'record_multi_region_auto_replace_evidence',
      'record_quota_request_tracking',
    ])
    expect(readme).toContain('Serving: ready')
    expect(readme).toContain('Acceptance: blocked')
    expect(readme).toContain(
      'Serving ready but durability acceptance incomplete: true',
    )
    expect(readme).toContain('- reclaimed replicas: 0')
    expect(readme).not.toContain('recover_reclaimed_replicas')
    expect(JSON.stringify(bundle)).not.toContain('private.example.test')
    expect(JSON.stringify(bundle)).not.toContain('secret-')
  })

  test('prints reclaimed replica recovery actions in retained README', () => {
    const env = {
      ...baseEnv,
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'warm-one,reclaimed-two',
      HYDRALISK_GLM_52_REAP_504B_RECLAIMED_TWO_BASE_URL:
        'https://reclaimed-two.private.example.test',
      HYDRALISK_GLM_52_REAP_504B_RECLAIMED_TWO_BEARER_TOKEN:
        'secret-reclaimed-two',
      HYDRALISK_GLM_52_REAP_504B_RECLAIMED_TWO_ENABLED: 'ready',
      HYDRALISK_GLM_52_REAP_504B_RECLAIMED_TWO_PREFLIGHT_REF:
        'preflight.hydralisk.glm.reclaimed_two',
      HYDRALISK_GLM_52_REAP_504B_RECLAIMED_TWO_RECEIPT_REF:
        'receipt.hydralisk.glm.reclaimed_two',
    } as const
    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId, {
        ...(replicaId === 'reclaimed-two'
          ? {
              healthStatus: 'failed',
              keepWarmStatus: 'failed',
              modelsStatus: 'failed',
              warmCompletionStatus: 'failed',
              warmState: 'cold',
              watchdogStatus: 'unhealthy',
            }
          : {}),
      }),
    )
    const bundle = buildGlmFleetDurabilityOperatorBundle({
      generatedAt: '2026-06-26T17:10:00.000Z',
      projection,
    })
    const readme = formatGlmFleetDurabilityOperatorReadme(bundle)

    expect(bundle.readiness.operatorActionItems[0]).toEqual({
      action: 'recover_reclaimed_replicas',
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.reclaimed_replicas_present',
      ],
      label:
        'recover reclaimed GLM replicas before treating current serving capacity as durable',
      replicaRefs: ['replica.hydralisk.glm_52_reap_504b.reclaimed-two'],
      severity: 'degraded',
    })
    expect(readme).toContain('## Operator action items')
    expect(readme).toContain('- recover_reclaimed_replicas (degraded)')
    expect(readme).toContain(
      'replica.hydralisk.glm_52_reap_504b.reclaimed-two',
    )
    expect(JSON.stringify(bundle)).not.toContain('private.example.test')
    expect(JSON.stringify(bundle)).not.toContain('secret-')
  })
})
