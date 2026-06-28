import { describe, expect, test } from 'vitest'

import {
  ARTANIS_HEALTH_SIGNAL_KINDS,
  ArtanisHealthSignalRecord,
  ArtanisHealthSnapshotRecord,
  ArtanisHealthUnsafe,
  artanisHealthProjectionHasPrivateMaterial,
  exampleArtanisHealthSnapshot,
  projectArtanisHealthSnapshot,
} from './artanis-health'

const nowIso = '2026-06-07T03:30:00.000Z'

const snapshotWithSignals = (
  signals: ReadonlyArray<ArtanisHealthSignalRecord>,
): ArtanisHealthSnapshotRecord =>
  new ArtanisHealthSnapshotRecord({
    ...exampleArtanisHealthSnapshot,
    signals,
  })

describe('Artanis health and staleness monitor', () => {
  test('projects required health signals with operator detail and public-safe stale labels', () => {
    const operator = projectArtanisHealthSnapshot(
      exampleArtanisHealthSnapshot,
      'operator',
      nowIso,
    )
    const publicArtanis = projectArtanisHealthSnapshot(
      exampleArtanisHealthSnapshot,
      'public_artanis',
      nowIso,
    )
    const publicForum = projectArtanisHealthSnapshot(
      exampleArtanisHealthSnapshot,
      'public_forum',
      nowIso,
    )

    expect(operator.signals.map(signal => signal.kind).sort()).toEqual(
      [...ARTANIS_HEALTH_SIGNAL_KINDS].sort(),
    )
    expect(operator.overclaimBlocked).toBe(true)
    expect(operator.staleOrBlockedSignalCount).toBe(4)
    expect(operator.pendingApprovalRefs).toEqual([
      'approval.public.artanis.pylon_dispatch_pending',
    ])
    expect(operator.operatorRecoveryActionRefs).toEqual([
      'recovery.operator.artanis.inspect_publication_lag',
      'recovery.operator.artanis.refresh_model_lab_report',
    ])
    expect(operator.signals.some(signal => signal.operatorDetailRefs.length > 0))
      .toBe(true)
    expect(publicArtanis.operatorRecoveryActionRefs).toEqual([])
    expect(publicArtanis.pendingApprovalRefs).toEqual([])
    expect(publicArtanis.runnerBackendRefs).toEqual([])
    expect(publicArtanis.pendingApprovalCount).toBe(1)
    expect(publicArtanis.signals.every(
      signal => signal.operatorDetailRefs.length === 0,
    )).toBe(true)
    expect(publicArtanis.signals.find(
      signal => signal.kind === 'model_lab_report_freshness',
    )).toMatchObject({
      publicRecoveryActionRefs: [
        'recovery.public.artanis.refresh_model_lab_summary',
      ],
      state: 'stale',
    })
    expect(artanisHealthProjectionHasPrivateMaterial(publicArtanis)).toBe(false)
    expect(artanisHealthProjectionHasPrivateMaterial(publicForum)).toBe(false)
    expect(JSON.stringify(publicArtanis)).not.toContain('health.operator')
    expect(JSON.stringify(publicArtanis)).not.toContain('recovery.operator')
    expect(JSON.stringify(publicArtanis)).not.toContain('2026-06-07T')
  })

  test('requires all health signals and blocks overclaiming when stale or blocked', () => {
    const missingSignal = snapshotWithSignals(
      exampleArtanisHealthSnapshot.signals.filter(
        signal => signal.kind !== 'runner_backend_availability',
      ),
    )
    const noOverclaimBlock = new ArtanisHealthSnapshotRecord({
      ...exampleArtanisHealthSnapshot,
      overclaimBlocked: false,
      overclaimBlockerRefs: [],
    })
    const noRecoveryAction = new ArtanisHealthSnapshotRecord({
      ...exampleArtanisHealthSnapshot,
      operatorRecoveryActionRefs: [],
    })

    expect(() =>
      projectArtanisHealthSnapshot(missingSignal, 'operator', nowIso),
    ).toThrow(ArtanisHealthUnsafe)
    expect(() =>
      projectArtanisHealthSnapshot(noOverclaimBlock, 'operator', nowIso),
    ).toThrow(ArtanisHealthUnsafe)
    expect(() =>
      projectArtanisHealthSnapshot(noRecoveryAction, 'operator', nowIso),
    ).toThrow(ArtanisHealthUnsafe)
  })

  test('allows a healthy snapshot when every signal is fresh or available', () => {
    const healthySignals = exampleArtanisHealthSnapshot.signals.map(signal =>
      new ArtanisHealthSignalRecord({
        ...signal,
        blockerRefs: [],
        count: signal.kind === 'pending_approvals' ? 0 : signal.count,
        publicRecoveryActionRefs: [],
        state: signal.kind === 'runner_backend_availability'
          ? 'available'
          : 'fresh',
      }),
    )
    const healthy = new ArtanisHealthSnapshotRecord({
      ...exampleArtanisHealthSnapshot,
      blockerRefs: [],
      operatorRecoveryActionRefs: [],
      overallState: 'healthy',
      overclaimBlocked: false,
      overclaimBlockerRefs: [],
      pendingApprovalRefs: [],
      publicStatusRefs: ['health.public.artanis.status.healthy'],
      signals: healthySignals,
    })
    const projection = projectArtanisHealthSnapshot(
      healthy,
      'public_artanis',
      nowIso,
    )

    expect(projection.overallState).toBe('healthy')
    expect(projection.overclaimBlocked).toBe(false)
    expect(projection.staleOrBlockedSignalCount).toBe(0)
    expect(projection.pendingApprovalCount).toBe(0)
  })

  test('rejects unsafe refs and stale signals without recovery or blocker refs', () => {
    const unsafeSnapshot = new ArtanisHealthSnapshotRecord({
      ...exampleArtanisHealthSnapshot,
      sourceRefs: ['provider_secret.raw'],
    })
    const staleWithoutRecovery = new ArtanisHealthSnapshotRecord({
      ...exampleArtanisHealthSnapshot,
      signals: exampleArtanisHealthSnapshot.signals.map(signal =>
        signal.kind === 'forum_publication_lag'
          ? new ArtanisHealthSignalRecord({
              ...signal,
              blockerRefs: [],
              publicRecoveryActionRefs: [],
              state: 'stale',
            })
          : signal,
      ),
    })

    expect(() =>
      projectArtanisHealthSnapshot(unsafeSnapshot, 'operator', nowIso),
    ).toThrow(ArtanisHealthUnsafe)
    expect(() =>
      projectArtanisHealthSnapshot(staleWithoutRecovery, 'operator', nowIso),
    ).toThrow(ArtanisHealthUnsafe)
  })
})
