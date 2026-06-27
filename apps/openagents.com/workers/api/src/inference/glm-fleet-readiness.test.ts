import { describe, expect, test } from 'vitest'

import type { GlmPoolHeartbeatReplicaRecord } from './glm-pool-heartbeat'
import {
  projectGlmFleetReadiness,
  projectGlmFleetReadinessForEnv,
  summarizeGlmFleetReadinessForOperators,
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
      expect.objectContaining({
        draining: true,
        maxInflight: 1,
        replicaId: 'disabled-four',
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.disabled-four',
        status: 'disabled',
      }),
      expect.objectContaining({
        blockerRefs: expect.arrayContaining([
          'blocker.hydralisk_glm_52_reap_504b.missing-five.route_not_ready',
        ]),
        maxInflight: 0,
        replicaId: 'missing-five',
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.missing-five',
        status: 'unavailable',
      }),
      expect.objectContaining({
        keepWarmStatus: 'control_plane_only',
        maxInflight: 1,
        replicaId: 'ready-two',
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.ready-two',
        status: 'ready',
        watchdogStatus: 'healthy',
      }),
      expect.objectContaining({
        healthStatus: 'failed',
        keepWarmStatus: 'failed',
        maxInflight: 1,
        replicaId: 'reclaimed-three',
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
        status: 'reclaimed',
        watchdogStatus: 'unhealthy',
      }),
      expect.objectContaining({
        armingEvidenceRefs: [
          'preflight.hydralisk.glm.warm_one',
          'receipt.hydralisk.glm.warm_one',
        ],
        keepWarmStatus: 'completed',
        latestHeartbeatAt: observedAt,
        maxInflight: 1,
        replicaId: 'warm-one',
        replicaRef: 'replica.hydralisk.glm_52_reap_504b.warm-one',
        status: 'warm',
        warmState: 'warm',
      }),
    ])
    expect(projection.counts).toEqual({
      activeMaxInflight: 3,
      benchmarkReservedReplicaCount: 0,
      configuredMaxInflight: 4,
      disabledReplicaCount: 1,
      drainingReplicaCount: 1,
      readyMaxInflight: 1,
      readyReplicaCount: 1,
      reclaimedReplicaCount: 1,
      totalReplicaCount: 5,
      unavailableReplicaCount: 1,
      warmMaxInflight: 1,
      warmOrReadyMaxInflight: 2,
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
        activeMaxInflight: 0,
        benchmarkReservedReplicaCount: 0,
        configuredMaxInflight: 0,
        disabledReplicaCount: 0,
        drainingReplicaCount: 0,
        readyMaxInflight: 0,
        readyReplicaCount: 0,
        reclaimedReplicaCount: 0,
        totalReplicaCount: 0,
        unavailableReplicaCount: 0,
        warmMaxInflight: 0,
        warmOrReadyMaxInflight: 0,
        warmReplicaCount: 0,
      },
      replicas: [],
      status: 'unavailable',
    })
    expect(projection.acceptance).toMatchObject({
      allReplicaKeepWarmWatchdog: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_no_required_replicas',
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
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing',
          'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing',
          'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing',
          'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing',
        ],
        status: 'blocked',
      },
      quotaRequestTracking: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.quota_request_state_missing',
        ],
        requestState: 'missing',
        status: 'blocked',
      },
      status: 'blocked',
    })
  })

  test('fails closed on missing durable fleet acceptance evidence', () => {
    const projection = projectGlmFleetReadinessForEnv(readyEnv, replicaId =>
      replicaId === 'warm-one' ? heartbeat(replicaId) : undefined,
    )

    expect(projection.acceptance).toMatchObject({
      allReplicaKeepWarmWatchdog: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_incomplete',
          'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing',
        ],
        coveredReplicaCount: 1,
        forcedStopRecoveryEvidenceRefs: [],
        missingReplicaRefs: [
          'replica.hydralisk.glm_52_reap_504b.missing-five',
          'replica.hydralisk.glm_52_reap_504b.ready-two',
          'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
        ],
        status: 'blocked',
        totalRequiredReplicaCount: 4,
      },
      capacityFloorOwnerDecision: {
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

  test('summarizes missing acceptance evidence even when serving is ready', () => {
    const env = {
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

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )
    const readout = summarizeGlmFleetReadinessForOperators(projection)

    expect(readout).toEqual({
      acceptanceStatus: 'blocked',
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing',
        'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing',
        'blocker.hydralisk_glm_52_reap_504b.quota_request_state_missing',
      ],
      counts: {
        activeMaxInflight: 1,
        benchmarkReservedReplicaCount: 0,
        configuredMaxInflight: 1,
        disabledReplicaCount: 0,
        drainingReplicaCount: 0,
        readyMaxInflight: 0,
        readyReplicaCount: 0,
        reclaimedReplicaCount: 0,
        totalReplicaCount: 1,
        unavailableReplicaCount: 0,
        warmMaxInflight: 1,
        warmOrReadyMaxInflight: 1,
        warmReplicaCount: 1,
      },
      disabledReplicaRefs: [],
      dimensions: [
        {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing',
          ],
          dimension: 'all_replica_keep_warm_watchdog',
          evidenceRefs: ['replica.hydralisk.glm_52_reap_504b.warm-one'],
          missingReplicaRefs: [],
          status: 'blocked',
        },
        {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing',
          ],
          dimension: 'capacity_floor_owner_decision',
          evidenceRefs: [],
          missingReplicaRefs: [],
          status: 'blocked',
        },
        {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing',
          ],
          dimension: 'multi_region_auto_replace',
          evidenceRefs: [],
          missingReplicaRefs: [
            'replica.hydralisk.glm_52_reap_504b.replacement-region.missing',
          ],
          status: 'blocked',
        },
        {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.quota_request_state_missing',
          ],
          dimension: 'quota_request_tracking',
          evidenceRefs: [],
          missingReplicaRefs: [],
          status: 'blocked',
        },
      ],
      evidenceRefs: ['replica.hydralisk.glm_52_reap_504b.warm-one'],
      kind: 'glm_fleet_readiness_operator_readout',
      missingReplicaRefs: [
        'replica.hydralisk.glm_52_reap_504b.replacement-region.missing',
      ],
      operatorActionItems: [
        {
          action: 'record_forced_stop_recovery_evidence',
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing',
          ],
          label:
            'record a public forced Spot STOP auto-recovery evidence ref before marking watchdog acceptance complete',
          replicaRefs: [],
          severity: 'blocked',
        },
        {
          action: 'record_capacity_floor_owner_decision',
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing',
          ],
          label: 'record public-safe capacity-floor owner decision evidence',
          replicaRefs: [],
          severity: 'blocked',
        },
        {
          action: 'record_multi_region_auto_replace_evidence',
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing',
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing',
          ],
          label:
            'record public-safe multi-region replacement, reserve, and prebake evidence',
          replicaRefs: [
            'replica.hydralisk.glm_52_reap_504b.replacement-region.missing',
          ],
          severity: 'blocked',
        },
        {
          action: 'record_quota_request_tracking',
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.quota_request_state_missing',
          ],
          label: 'record public-safe quota request state and evidence ref',
          replicaRefs: [],
          severity: 'blocked',
        },
      ],
      readyReplicaRefs: [],
      reclaimedReplicaRefs: [],
      servingCapacitySummary:
        'serving capacity recovered: ready=0, warm=1, reclaimed=0, warmOrReadyMaxInflight=1; durability acceptance remains blocked',
      servingReadyButAcceptanceNotComplete: true,
      servingStatus: 'ready',
      unavailableReplicaRefs: [],
      warmReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.warm-one'],
    })
  })

  test('projects durable fleet acceptance dimensions only from public-safe evidence', () => {
    const env = {
      ...readyEnv,
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS:
        'warm-one,ready-two,reclaimed-three,disabled-four,missing-five,reserved-six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BASE_URL:
        'https://reserved-six.private.example.test',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BEARER_TOKEN:
        'secret-reserved-six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BENCHMARK_RESERVED: 'true',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_ENABLED: 'ready',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_PREFLIGHT_REF:
        'preflight.hydralisk.glm.reserved_six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_RECEIPT_REF:
        'receipt.hydralisk.glm.reserved_six',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
        'owner_accepted_all_spot',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
        'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS:
        'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS:
        'prebake.hydralisk.glm_52_reap_504b.reserved_six.20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS:
        'reserve.hydralisk.glm_52_reap_504b.reserved_six.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF:
        'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: 'pending',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )

    expect(projection.acceptance).toEqual({
      allReplicaKeepWarmWatchdog: {
        blockerRefs: [],
        coveredReplicaCount: 4,
        evidenceRefs: [
          'replica.hydralisk.glm_52_reap_504b.missing-five',
          'replica.hydralisk.glm_52_reap_504b.ready-two',
          'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
          'replica.hydralisk.glm_52_reap_504b.warm-one',
        ],
        forcedStopRecoveryEvidenceRefs: [
          'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
        ],
        missingReplicaRefs: [],
        status: 'complete',
        totalRequiredReplicaCount: 4,
      },
      capacityFloorOwnerDecision: {
        blockerRefs: [],
        decision: 'owner_accepted_all_spot',
        evidenceRefs: [
          'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
        ],
        status: 'complete',
      },
      multiRegionAutoReplace: {
        blockerRefs: [],
        coveredReplacementReplicaRefs: [
          'replica.hydralisk.glm_52_reap_504b.reserved-six',
        ],
        evidenceRefs: [
          'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
        ],
        missingReplacementReplicaRefs: [],
        prebakeEvidenceRefs: [
          'prebake.hydralisk.glm_52_reap_504b.reserved_six.20260626',
        ],
        reserveEvidenceRefs: [
          'reserve.hydralisk.glm_52_reap_504b.reserved_six.20260626',
          'preflight.hydralisk.glm.reserved_six',
          'receipt.hydralisk.glm.reserved_six',
        ],
        status: 'complete',
      },
      quotaRequestTracking: {
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.quota_request_pending',
        ],
        evidenceRefs: ['quota_request.gcp.us_central1.rtx_pro_6000.20260626'],
        requestState: 'pending',
        status: 'incomplete',
      },
      status: 'incomplete',
    })
    expect(JSON.stringify(projection)).not.toContain('private.example.test')
    expect(JSON.stringify(projection)).not.toContain('secret-')
  })

  test('summarizes pending-quota partial evidence deterministically', () => {
    const env = {
      ...readyEnv,
      HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS:
        'warm-one,ready-two,reclaimed-three,disabled-four,missing-five,reserved-six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BASE_URL:
        'https://reserved-six.private.example.test',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BEARER_TOKEN:
        'secret-reserved-six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BENCHMARK_RESERVED: 'true',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_ENABLED: 'ready',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_PREFLIGHT_REF:
        'preflight.hydralisk.glm.reserved_six',
      HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_RECEIPT_REF:
        'receipt.hydralisk.glm.reserved_six',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
        'owner_accepted_all_spot',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
        'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS:
        'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS:
        'prebake.hydralisk.glm_52_reap_504b.reserved_six.20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS:
        'reserve.hydralisk.glm_52_reap_504b.reserved_six.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF:
        'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: 'pending',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )
    const readout = summarizeGlmFleetReadinessForOperators(projection)

    expect(readout.acceptanceStatus).toBe('incomplete')
    expect(readout.blockerRefs).toEqual([
      'blocker.hydralisk_glm_52_reap_504b.quota_request_pending',
    ])
    expect(readout.dimensions.map(dimension => dimension.dimension)).toEqual([
      'all_replica_keep_warm_watchdog',
      'capacity_floor_owner_decision',
      'multi_region_auto_replace',
      'quota_request_tracking',
    ])
    expect(readout.evidenceRefs).toEqual([
      'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
      'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
      'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      'prebake.hydralisk.glm_52_reap_504b.reserved_six.20260626',
      'preflight.hydralisk.glm.reserved_six',
      'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
      'receipt.hydralisk.glm.reserved_six',
      'replica.hydralisk.glm_52_reap_504b.missing-five',
      'replica.hydralisk.glm_52_reap_504b.ready-two',
      'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
      'replica.hydralisk.glm_52_reap_504b.warm-one',
      'reserve.hydralisk.glm_52_reap_504b.reserved_six.20260626',
    ])
    expect(readout.dimensions[3]).toEqual({
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.quota_request_pending',
      ],
      dimension: 'quota_request_tracking',
      evidenceRefs: ['quota_request.gcp.us_central1.rtx_pro_6000.20260626'],
      missingReplicaRefs: [],
      status: 'incomplete',
    })
    expect(readout.missingReplicaRefs).toEqual([])
    expect(readout.operatorActionItems).toEqual([
      {
        action: 'record_quota_request_tracking',
        blockerRefs: [
          'blocker.hydralisk_glm_52_reap_504b.quota_request_pending',
        ],
        label: 'record public-safe quota request state and evidence ref',
        replicaRefs: [],
        severity: 'incomplete',
      },
    ])
    expect(readout.counts).toMatchObject({
      reclaimedReplicaCount: 0,
      totalReplicaCount: 6,
      unavailableReplicaCount: 1,
      warmReplicaCount: 3,
    })
    expect(readout.disabledReplicaRefs).toEqual([
      'replica.hydralisk.glm_52_reap_504b.disabled-four',
      'replica.hydralisk.glm_52_reap_504b.reserved-six',
    ])
    expect(readout.warmReplicaRefs).toEqual([
      'replica.hydralisk.glm_52_reap_504b.ready-two',
      'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
      'replica.hydralisk.glm_52_reap_504b.warm-one',
    ])
    expect(readout.unavailableReplicaRefs).toEqual([
      'replica.hydralisk.glm_52_reap_504b.missing-five',
    ])
    expect(readout.servingReadyButAcceptanceNotComplete).toBe(false)
    expect(JSON.stringify(readout)).not.toContain('private.example.test')
    expect(JSON.stringify(readout)).not.toContain('secret-')
  })

  test('summarizes reclaimed replicas as explicit operator recovery actions', () => {
    const latest = new Map([
      ['warm-one', heartbeat('warm-one')],
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
    const readout = summarizeGlmFleetReadinessForOperators(projection)

    expect(readout.servingStatus).toBe('degraded')
    expect(readout.reclaimedReplicaRefs).toEqual([
      'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
    ])
    expect(readout.operatorActionItems).toEqual(
      expect.arrayContaining([
        {
          action: 'recover_reclaimed_replicas',
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.reclaimed_replicas_present',
          ],
          label:
            'recover reclaimed GLM replicas before treating current serving capacity as durable',
          replicaRefs: [
            'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
          ],
          severity: 'degraded',
        },
        expect.objectContaining({
          action: 'attach_all_replica_keep_warm_watchdog',
          replicaRefs: [
            'replica.hydralisk.glm_52_reap_504b.missing-five',
            'replica.hydralisk.glm_52_reap_504b.ready-two',
            'replica.hydralisk.glm_52_reap_504b.reclaimed-three',
          ],
          severity: 'blocked',
        }),
      ]),
    )
    expect(JSON.stringify(readout)).not.toContain('private.example.test')
    expect(JSON.stringify(readout)).not.toContain('secret-')
  })

  test('fails closed when multi-region auto-replace reserve or prebake evidence is missing', () => {
    const env = {
      ...readyEnv,
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS:
        'https://operator-only.example.test/prebake',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS:
        'sk-reserve-secret',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )

    expect(projection.acceptance.multiRegionAutoReplace).toEqual({
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing',
        'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing',
      ],
      coveredReplacementReplicaRefs: [],
      evidenceRefs: [
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      ],
      missingReplacementReplicaRefs: [
        'replica.hydralisk.glm_52_reap_504b.replacement-region.missing',
      ],
      prebakeEvidenceRefs: [],
      reserveEvidenceRefs: [],
      status: 'blocked',
    })
    expect(projection.acceptance.status).toBe('blocked')
    expect(JSON.stringify(projection)).not.toContain('operator-only')
    expect(JSON.stringify(projection)).not.toContain('sk-reserve-secret')
  })

  test('fails closed when owner-confirmed capacity-floor evidence is missing or unsafe', () => {
    const env = {
      ...readyEnv,
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
        'non_spot_floor_approved',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
        'https://private.example.test/capacity-floor',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF:
        'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: 'approved',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )

    expect(projection.acceptance.capacityFloorOwnerDecision).toEqual({
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_evidence_missing',
      ],
      decision: 'non_spot_floor_approved',
      evidenceRefs: [],
      status: 'blocked',
    })
    expect(projection.acceptance.quotaRequestTracking).toMatchObject({
      requestState: 'approved',
      status: 'complete',
    })
    expect(projection.acceptance.status).toBe('blocked')
    expect(JSON.stringify(projection)).not.toContain('private.example.test')
  })

  test.each([
    {
      blockerRef: 'blocker.hydralisk_glm_52_reap_504b.quota_request_pending',
      expectedState: 'pending',
      expectedStatus: 'incomplete',
      state: 'pending',
    },
    {
      blockerRef: 'blocker.hydralisk_glm_52_reap_504b.quota_request_denied',
      expectedState: 'denied',
      expectedStatus: 'blocked',
      state: 'denied',
    },
    {
      blockerRef: 'blocker.hydralisk_glm_52_reap_504b.quota_request_denied',
      expectedState: 'denied',
      expectedStatus: 'blocked',
      state: 'rejected',
    },
    {
      blockerRef:
        'blocker.hydralisk_glm_52_reap_504b.quota_request_state_unknown',
      expectedState: 'unknown',
      expectedStatus: 'blocked',
      state: 'owner-said-maybe',
    },
  ])(
    'distinguishes quota request state $state as $expectedStatus',
    ({ blockerRef, expectedState, expectedStatus, state }) => {
      const env = {
        ...readyEnv,
        HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS:
          'warm-one,ready-two,reclaimed-three,disabled-four,missing-five,reserved-six',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BASE_URL:
          'https://reserved-six.private.example.test',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BEARER_TOKEN:
          'secret-reserved-six',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_BENCHMARK_RESERVED: 'true',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_ENABLED: 'ready',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_PREFLIGHT_REF:
          'preflight.hydralisk.glm.reserved_six',
        HYDRALISK_GLM_52_REAP_504B_RESERVED_SIX_RECEIPT_REF:
          'receipt.hydralisk.glm.reserved_six',
        HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
          'owner_accepted_all_spot',
        HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
          'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
        HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS:
          'evidence.hydralisk.glm_52_reap_504b.forced_stop_recovery.owner_20260626',
        HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
          'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
        HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS:
          'prebake.hydralisk.glm_52_reap_504b.reserved_six.20260626',
        HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS:
          'reserve.hydralisk.glm_52_reap_504b.reserved_six.20260626',
        HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF:
          'quota_request.gcp.us_central1.rtx_pro_6000.20260626',
        HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: state,
      } as const

      const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
        heartbeat(replicaId),
      )

      expect(projection.acceptance.quotaRequestTracking).toEqual({
        blockerRefs: [blockerRef],
        evidenceRefs: ['quota_request.gcp.us_central1.rtx_pro_6000.20260626'],
        requestState: expectedState,
        status: expectedStatus,
      })
      expect(projection.acceptance.status).toBe(expectedStatus)
    },
  )

  test('fails closed when quota request state exists without public-safe evidence', () => {
    const env = {
      ...readyEnv,
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION:
        'owner_accepted_all_spot',
      HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF:
        'decision.hydralisk.glm_52_reap_504b.capacity_floor.owner_20260626',
      HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF:
        'evidence.hydralisk.glm_52_reap_504b.multi_region_auto_replace.plan_20260626',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF: 'sk-private-not-a-ref',
      HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE: 'pending',
    } as const

    const projection = projectGlmFleetReadinessForEnv(env, replicaId =>
      heartbeat(replicaId),
    )

    expect(projection.acceptance.quotaRequestTracking).toEqual({
      blockerRefs: [
        'blocker.hydralisk_glm_52_reap_504b.quota_request_evidence_missing',
      ],
      evidenceRefs: [],
      requestState: 'pending',
      status: 'blocked',
    })
    expect(projection.acceptance.status).toBe('blocked')
    expect(JSON.stringify(projection)).not.toContain('sk-private-not-a-ref')
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
