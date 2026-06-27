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
    expect(bundle.missingOperatorInputs.map(input => input.env)).toEqual([
      'HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION',
      'HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS',
      'HYDRALISK_GLM_52_REAP_504B_<REPLICA>_BENCHMARK_RESERVED',
      'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS',
      'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE',
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
    expect(readme).toContain('- none')
    expect(JSON.stringify(bundle)).not.toContain('private.example.test')
    expect(JSON.stringify(bundle)).not.toContain('secret-')
  })
})
