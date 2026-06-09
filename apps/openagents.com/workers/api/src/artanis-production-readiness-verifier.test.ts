import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_PRODUCTION_READINESS_READ_ONLY_AUTHORITY,
  ArtanisProductionReadinessProjection,
  ArtanisProductionReadinessUnsafe,
  buildArtanisProductionReadinessVerificationRecordFromObservation,
  exampleArtanisProductionReadinessObservation,
  projectArtanisProductionReadinessVerification,
} from './artanis-production-readiness-verifier'

const nowIso = '2026-06-07T01:00:00.000Z'

const allTables = [
  'artanis_approval_gates',
  'artanis_forum_publication_intents',
  'artanis_health_snapshots',
  'artanis_loop_records',
  'artanis_loop_ticks',
  'artanis_nexus_pylon_adapter_dispatches',
  'artanis_runtime_snapshots',
  'artanis_work_routing_proposals',
]

const allReportFields = [
  'autonomousLoop',
  'forumRewardSmoke',
  'healthSummary',
  'productionLaunchGate',
  'pylonLaunchCommunication',
]

const readyObservation = () => ({
  ...exampleArtanisProductionReadinessObservation(),
  d1TableNames: allTables,
  productionSmokeRef: 'smoke.public.artanis.production_equivalent.20260607',
  publicReportFields: allReportFields,
  pylonV02ReleaseAssetCount: 2,
  pylonV02ReleaseTag: 'pylon-v0.2.0',
  scheduledRunnerEnabled: true,
})

describe('Artanis production readiness verifier', () => {
  test('projects the current blocked audit state without granting authority', () => {
    const projection = projectArtanisProductionReadinessVerification(
      buildArtanisProductionReadinessVerificationRecordFromObservation(
        exampleArtanisProductionReadinessObservation(),
        nowIso,
      ),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisProductionReadinessProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      agentRef: 'agent.public.artanis',
      d1MutationAllowed: false,
      deployedParityReady: false,
      deploymentAllowed: false,
      failedRequiredCount: 5,
      forumMutationAllowed: false,
      gitHubReleaseMutationAllowed: false,
      persistenceReady: false,
      publicClaimUpgradeAllowed: false,
      pylonDispatchAllowed: false,
      releaseReady: false,
      schedulerMutationAllowed: false,
      schedulerReady: false,
      smokeReady: false,
      sourceReady: true,
      state: 'blocked',
      stateLabel: 'Blocked before Artanis production autonomy',
      updatedAtDisplay: 'Just now',
      walletSpendAllowed: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.production_readiness.d1_persistence',
        'blocker.public.artanis.production_readiness.public_report_fields',
        'blocker.public.artanis.production_readiness.pylon_v0_2_release_not_shipped',
        'blocker.public.artanis.production_readiness.production_smoke_missing',
        'blocker.public.artanis.production_readiness.scheduler_not_enabled',
      ]),
    )
    expect(projection.privateEvidenceRefs).toEqual([])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
  })

  test('projects ready only when every production readiness check passes', () => {
    const projection = projectArtanisProductionReadinessVerification(
      buildArtanisProductionReadinessVerificationRecordFromObservation(
        readyObservation(),
        nowIso,
      ),
      'operator',
      nowIso,
    )

    expect(projection).toMatchObject({
      deployedParityReady: true,
      failedRequiredCount: 0,
      persistenceReady: true,
      releaseReady: true,
      schedulerReady: true,
      smokeReady: true,
      sourceReady: true,
      state: 'ready',
      stateLabel: 'Ready for controlled operator launch window',
    })
    expect(projection.stageStatuses.map(stage => [
      stage.stage,
      stage.status,
    ])).toEqual([
      ['deployed_parity_ready', 'passed'],
      ['persistence_ready', 'passed'],
      ['release_ready', 'passed'],
      ['scheduler_ready', 'passed'],
      ['smoke_ready', 'passed'],
      ['source_ready', 'passed'],
    ])
  })

  test('distinguishes missing D1 tables, missing report fields, and old release evidence', () => {
    const projection = projectArtanisProductionReadinessVerification(
      buildArtanisProductionReadinessVerificationRecordFromObservation(
        {
          ...readyObservation(),
          d1TableNames: ['artanis_runtime_snapshots'],
          latestPylonReleaseTag: 'pylon-v0.1.23',
          productionSmokeRef: null,
          publicReportFields: ['autonomousLoop'],
          pylonV02ReleaseAssetCount: 0,
          pylonV02ReleaseTag: null,
          scheduledRunnerEnabled: false,
        },
        nowIso,
      ),
      'public',
      nowIso,
    )

    expect(projection.persistenceReady).toBe(false)
    expect(projection.deployedParityReady).toBe(false)
    expect(projection.releaseReady).toBe(false)
    expect(projection.stageStatuses.find(
      stage => stage.stage === 'release_ready',
    )?.evidenceRefs).toContain('release.public.openagents.latest.pylon-v0.1.23')
  })

  test('projects unavailable and stale checks without pretending readiness', () => {
    const projection = projectArtanisProductionReadinessVerification(
      buildArtanisProductionReadinessVerificationRecordFromObservation(
        {
          ...readyObservation(),
          d1TableNames: null,
          pylonStatsStatus: 'stale',
          scheduledRunnerEnabled: null,
          statusTopicPostCount: null,
        },
        nowIso,
      ),
      'public',
      nowIso,
    )

    expect(projection.persistenceReady).toBe(false)
    expect(projection.schedulerReady).toBe(false)
    expect(projection.smokeReady).toBe(false)
    expect(projection.stageStatuses.find(
      stage => stage.stage === 'persistence_ready',
    )?.status).toBe('unavailable')
    expect(projection.stageStatuses.find(
      stage => stage.stage === 'scheduler_ready',
    )?.status).toBe('unavailable')
    expect(projection.stageStatuses.find(
      stage => stage.stage === 'smoke_ready',
    )?.status).toBe('unavailable')
  })

  test('rejects mutable authority and scheduler readiness before retained smoke', () => {
    const record = buildArtanisProductionReadinessVerificationRecordFromObservation(
      readyObservation(),
      nowIso,
    )

    expect(() =>
      projectArtanisProductionReadinessVerification(
        {
          ...record,
          authority: {
            ...ARTANIS_PRODUCTION_READINESS_READ_ONLY_AUTHORITY,
            noDeployment: false,
          },
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisProductionReadinessUnsafe)

    expect(() =>
      projectArtanisProductionReadinessVerification(
        {
          ...record,
          checks: record.checks.map(check =>
            check.checkKind === 'production_e2e_smoke'
              ? { ...check, status: 'blocked' as const }
              : check
          ),
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisProductionReadinessUnsafe)
  })

  test('rejects unsafe private, raw, secret, and literal timestamp refs', () => {
    const record = buildArtanisProductionReadinessVerificationRecordFromObservation(
      readyObservation(),
      nowIso,
    )

    for (const unsafeRecord of [
      {
        ...record,
        privateEvidenceRefs: ['wallet.secret.material'],
      },
      {
        ...record,
        checks: record.checks.map(check =>
          check.checkKind === 'd1_persistence'
            ? { ...check, evidenceRefs: ['raw_d1.rows'] }
            : check
        ),
      },
      {
        ...record,
        checks: record.checks.map(check =>
          check.checkKind === 'public_report_fields'
            ? { ...check, sourceRefs: ['https://github.com/team/private-repo'] }
            : check
        ),
      },
      {
        ...record,
        sourceRefs: ['source.public.2026-06-07T01:00:00'],
      },
    ]) {
      expect(() =>
        projectArtanisProductionReadinessVerification(
          unsafeRecord,
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisProductionReadinessUnsafe)
    }
  })
})
