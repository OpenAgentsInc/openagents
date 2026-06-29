import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  exampleArtanisProductionLaunchGateRecord,
  projectArtanisProductionLaunchGate,
} from './artanis-production-launch-gate'
import {
  ARTANIS_RETAINED_LAUNCH_SMOKE_READ_ONLY_AUTHORITY,
  ArtanisRetainedLaunchSmokeProjection,
  ArtanisRetainedLaunchSmokeUnsafe,
  artanisProductionLaunchGateCheckFromRetainedSmoke,
  exampleArtanisRetainedLaunchSmokeRecord,
  projectArtanisRetainedLaunchSmoke,
} from './artanis-retained-launch-smoke'

const nowIso = '2026-06-07T02:05:00.000Z'

describe('Artanis retained production-equivalent launch smoke', () => {
  test('projects retained delivered-post smoke without granting live authority', () => {
    const projection = projectArtanisRetainedLaunchSmoke(
      exampleArtanisRetainedLaunchSmokeRecord(),
      'public',
      nowIso,
    )

    expect(
      S.decodeUnknownSync(ArtanisRetainedLaunchSmokeProjection)(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      agentRef: 'agent.public.artanis',
      buyerChargeMutationAllowed: false,
      deploymentAllowed: false,
      forumDeliveryVerified: true,
      forumMode: 'delivered_post',
      forumMutationAllowed: false,
      privateEvidenceRefs: [],
      providerMutationAllowed: false,
      pylonDispatchAllowed: false,
      schedulerMutationAllowed: false,
      settlementMutationAllowed: false,
      state: 'retained',
      stateLabel: 'Retained production-equivalent smoke evidence',
      trainingLaunchAllowed: false,
      updatedAtDisplay: '5 minutes ago',
      walletSpendAllowed: false,
    })
    expect(projection.persistedRowRefs).toEqual(
      expect.arrayContaining([
        'runtime.public.artanis.snapshot.retained_smoke',
        'loop.public.artanis.retained_smoke',
        'tick.public.artanis.retained_smoke',
        'health.public.artanis.snapshot.retained_smoke',
        'proposal.public.artanis.work_routing.retained_smoke',
        'forum.public.artanis.status_intent.retained_smoke',
      ]),
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(JSON.stringify(projection)).not.toContain('evidence.private')
  })

  test('supports no-publish production-equivalent smoke mode', () => {
    const base = exampleArtanisRetainedLaunchSmokeRecord()
    const projection = projectArtanisRetainedLaunchSmoke(
      {
        ...base,
        caveatRefs: [...base.caveatRefs, 'caveat.public.no_publish_test_mode'],
        deliveryReceiptRefs: [],
        forumMode: 'no_publish_test',
        forumPostRefs: [],
        noPublishProofRefs: ['proof.public.artanis.no_publish_test.retained'],
        schedulerMode: 'no_launch',
      },
      'public',
      nowIso,
    )

    expect(projection.forumDeliveryVerified).toBe(true)
    expect(projection.forumMode).toBe('no_publish_test')
    expect(projection.forumPostRefs).toEqual([])
    expect(projection.noPublishProofRefs).toEqual([
      'proof.public.artanis.no_publish_test.retained',
    ])
    expect(projection.state).toBe('retained')
  })

  test('retains safe private evidence only for operator projection', () => {
    const publicProjection = projectArtanisRetainedLaunchSmoke(
      exampleArtanisRetainedLaunchSmokeRecord(),
      'public',
      nowIso,
    )
    const operatorProjection = projectArtanisRetainedLaunchSmoke(
      exampleArtanisRetainedLaunchSmokeRecord(),
      'operator',
      nowIso,
    )

    expect(publicProjection.privateEvidenceRefs).toEqual([])
    expect(operatorProjection.privateEvidenceRefs).toEqual([
      'evidence.private.operator.artanis.retained_smoke_log',
    ])
  })

  test('rejects missing persistence, delivery, public report, and rollback refs', () => {
    const base = exampleArtanisRetainedLaunchSmokeRecord()

    for (const record of [
      {
        ...base,
        persistedRefs: {
          ...base.persistedRefs,
          loopTickRefs: [],
        },
      },
      {
        ...base,
        deliveryReceiptRefs: [],
        forumPostRefs: [],
      },
      {
        ...base,
        publicReportRefs: ['https://openagents.com/artanis'],
      },
      {
        ...base,
        rollbackDisableRefs: [],
      },
    ]) {
      expect(() =>
        projectArtanisRetainedLaunchSmoke(record, 'operator', nowIso),
      ).toThrow(ArtanisRetainedLaunchSmokeUnsafe)
    }
  })

  test('rejects mutable authority and unsafe private/raw refs', () => {
    const base = exampleArtanisRetainedLaunchSmokeRecord()

    for (const record of [
      {
        ...base,
        authority: {
          ...ARTANIS_RETAINED_LAUNCH_SMOKE_READ_ONLY_AUTHORITY,
          noWalletSpend: false,
        },
      },
      {
        ...base,
        privateEvidenceRefs: ['wallet.secret.material'],
      },
      {
        ...base,
        persistedRefs: {
          ...base.persistedRefs,
          runtimeSnapshotRefs: ['raw_d1.row_dump'],
        },
      },
      {
        ...base,
        publicReportRefs: ['https://github.com/team/private-repo'],
      },
    ]) {
      expect(() =>
        projectArtanisRetainedLaunchSmoke(record, 'operator', nowIso),
      ).toThrow(ArtanisRetainedLaunchSmokeUnsafe)
    }
  })

  test('can feed the production launch gate while autonomy remains blocked by authority split', () => {
    const smokeCheck = artanisProductionLaunchGateCheckFromRetainedSmoke(
      exampleArtanisRetainedLaunchSmokeRecord(),
      nowIso,
    )
    const gate = exampleArtanisProductionLaunchGateRecord(nowIso)
    const projection = projectArtanisProductionLaunchGate(
      {
        ...gate,
        checks: gate.checks.map(check =>
          check.category === 'production_e2e_smoke' ? smokeCheck : check,
        ),
      },
      nowIso,
    )

    expect(smokeCheck).toMatchObject({
      category: 'production_e2e_smoke',
      status: 'passed',
    })
    expect(projection.canClaimBoundedStatusProjection).toBe(true)
    expect(projection.canClaimContinuouslyRunning).toBe(false)
    expect(projection.state).toBe('ready')
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.production_e2e_smoke.blocked',
    )
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.scheduled_runner.blocked',
    )
  })
})
