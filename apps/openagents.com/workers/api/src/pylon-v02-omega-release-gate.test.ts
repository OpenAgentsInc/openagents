import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { publicScannerSafeRef } from './public-ref-scanner-safety'
import {
  PYLON_V02_OMEGA_RELEASE_GATE_NO_AUTHORITY,
  PylonV02OmegaReleaseGateProjection,
  PylonV02OmegaReleaseGateUnsafe,
  currentPylonV02OmegaReleaseGateRecord,
  projectPylonV02OmegaReleaseGate,
  readyPylonV02OmegaReleaseGateRecord,
} from './pylon-v02-omega-release-gate'

const nowIso = '2026-06-07T12:30:00.000Z'
const scannerShapedBridgeRef = 'artanis-mdk-bridge-8b378373002501f3e896dcd3'
const scannerSafeBridgeRef = publicScannerSafeRef(
  'evidence.public.pylon_v0_2.omega_gate',
  scannerShapedBridgeRef,
)

describe('Pylon v0.2 Omega release gate', () => {
  test('projects the current multi-Pylon proof as a limited shipped launcher release', () => {
    const projection = projectPylonV02OmegaReleaseGate(
      currentPylonV02OmegaReleaseGateRecord(),
      'public',
      nowIso,
    )

    expect(
      S.decodeUnknownSync(PylonV02OmegaReleaseGateProjection)(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      audience: 'public',
      canAnnouncePylonV02AcceptedWork: true,
      canAnnouncePylonV02Payments: true,
      canAnnouncePylonV02Release: true,
      canAnnouncePylonV02Settlement: true,
      failedOrPendingRequiredCount: 0,
      hostedMdkDirectPayoutClaimAllowed: false,
      multiPylonObservedDistinctPylonCount: 2,
      multiPylonPaidWorkProofComplete: true,
      multiPylonRequiredDistinctPylonCount: 2,
      oldGoogleCloudNexusRequired: false,
      releaseCreationAllowedByThisRecord: false,
      releasePublicationAllowed: false,
      requiredPassedCount: projection.requiredCheckCount,
      state: 'limited_launcher_release_shipped',
      walletSpendAllowed: false,
    })
    expect(projection.payoutModeGate).toMatchObject({
      activeMode: 'local_mdk_agent_wallet_bridge',
      hostedDirectPayoutClaimAllowed: false,
      localBridgePayoutClaimAllowed: true,
      livePayoutClaimAllowed: true,
      state: 'ready',
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.evidenceRefs).toEqual(
      expect.arrayContaining([
        'runtime.public.mdk.worker_safe_route_boundary.issue_434',
        'smoke.public.issue_438.artanis_real_assignment.issue_438_artanis_1780822221',
        'assignment.public.issue_438.issue_438_artanis_1780822221',
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
        scannerSafeBridgeRef,
        'pylon.public.artanis.bridge.8b378373',
        'receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
        'forum.public.artanis.nexus_pylon.release_gate_pass.issue_438_artanis_1780822221',
        'blocker.mdk.hosted_programmatic_payouts_disabled',
        'evidence.mdk_agent_wallet.local_bridge_authority_recorded',
      ]),
    )
    expect(projection.optionalTransitionEvidenceRefs).toEqual([
      'transition.public.old_google_cloud_nexus.not_release_gate',
    ])
    expect(projection.multiPylonProofRefs).toEqual(
      expect.arrayContaining([scannerSafeBridgeRef]),
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
    expect(JSON.stringify(projection)).not.toContain(scannerShapedBridgeRef)
  })

  test('projects a fully satisfied evidence packet as a limited shipped launcher release without granting release authority', () => {
    const projection = projectPylonV02OmegaReleaseGate(
      readyPylonV02OmegaReleaseGateRecord(),
      'public',
      nowIso,
    )

    expect(projection).toMatchObject({
      canAnnouncePylonV02AcceptedWork: true,
      canAnnouncePylonV02Payments: true,
      canAnnouncePylonV02Release: true,
      canAnnouncePylonV02Settlement: true,
      failedOrPendingRequiredCount: 0,
      hostedMdkDirectPayoutClaimAllowed: false,
      multiPylonObservedDistinctPylonCount: 2,
      multiPylonPaidWorkProofComplete: true,
      releaseCreationAllowedByThisRecord: false,
      releasePublicationAllowed: false,
      requiredPassedCount: projection.requiredCheckCount,
      state: 'limited_launcher_release_shipped',
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.payoutModeGate.activeMode).toBe(
      'local_mdk_agent_wallet_bridge',
    )
  })

  test('blocks a passed multi-Pylon check if it repeats the same Pylon ref', () => {
    const ready = readyPylonV02OmegaReleaseGateRecord()
    const projection = projectPylonV02OmegaReleaseGate(
      {
        ...ready,
        checks: ready.checks.map(check =>
          check.checkKind === 'multi_pylon_paid_work_proof'
            ? {
                ...check,
                evidenceRefs: [
                  'assignment.public.multi_pylon.one',
                  'pylon.public.same_edge',
                  'receipt.nexus_pylon.settlement.assignment_public_multi_pylon_one',
                  'assignment.public.multi_pylon.two',
                  'pylon.public.same_edge',
                  'receipt.nexus_pylon.settlement.assignment_public_multi_pylon_two',
                ],
              }
            : check,
        ),
      },
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.multiPylonObservedDistinctPylonCount).toBe(1)
    expect(projection.blockerRefs).toContain(
      'blocker.public.pylon_v0_2.multi_pylon.distinct_pylon_count_missing',
    )
  })

  test('blocks a passed multi-Pylon check if evidence is simulation-only', () => {
    const ready = readyPylonV02OmegaReleaseGateRecord()
    const projection = projectPylonV02OmegaReleaseGate(
      {
        ...ready,
        checks: ready.checks.map(check =>
          check.checkKind === 'multi_pylon_paid_work_proof'
            ? {
                ...check,
                evidenceRefs: [
                  ...check.evidenceRefs,
                  'receipt.nexus_pylon.simulation.settlement.assignment_public_multi_pylon_simulation',
                ],
              }
            : check,
        ),
      },
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.multiPylonPaidWorkProofComplete).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.pylon_v0_2.multi_pylon.simulation_only',
    )
  })

  test('blocks a passed multi-Pylon check if terminal settlement receipt refs are missing', () => {
    const ready = readyPylonV02OmegaReleaseGateRecord()
    const projection = projectPylonV02OmegaReleaseGate(
      {
        ...ready,
        checks: ready.checks.map(check =>
          check.checkKind === 'multi_pylon_paid_work_proof'
            ? {
                ...check,
                evidenceRefs: [
                  'assignment.public.multi_pylon.one',
                  'pylon.public.edge_one',
                  'assignment.public.multi_pylon.two',
                  'pylon.public.edge_two',
                ],
              }
            : check,
        ),
      },
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.multiPylonObservedDistinctPylonCount).toBe(2)
    expect(projection.blockerRefs).toContain(
      'blocker.public.pylon_v0_2.multi_pylon.terminal_settlement_missing',
    )
  })

  test('treats old Google Cloud Nexus evidence as optional transition context', () => {
    const current = currentPylonV02OmegaReleaseGateRecord()
    const withoutOldGoogleCloud = {
      ...current,
      checks: current.checks.filter(
        check => check.checkKind !== 'old_google_cloud_nexus_transition',
      ),
    }
    const projection = projectPylonV02OmegaReleaseGate(
      withoutOldGoogleCloud,
      'public',
      nowIso,
    )

    expect(projection.optionalTransitionEvidenceRefs).toEqual([])
    expect(projection.missingRequiredCheckRefs).not.toContain(
      'missing.public.pylon_v0_2.omega_gate.old_google_cloud_nexus_transition',
    )
    expect(projection.oldGoogleCloudNexusRequired).toBe(false)
  })

  test('redacts operator-only transition refs from public but keeps them for operator', () => {
    const current = currentPylonV02OmegaReleaseGateRecord()
    const withOperatorTransition = {
      ...current,
      checks: current.checks.map(check =>
        check.checkKind === 'old_google_cloud_nexus_transition'
          ? {
              ...check,
              evidenceRefs: [
                ...check.evidenceRefs,
                'operator.public.old_google_cloud_nexus.manual_note',
              ],
            }
          : check,
      ),
    }

    expect(
      projectPylonV02OmegaReleaseGate(withOperatorTransition, 'public', nowIso)
        .optionalTransitionEvidenceRefs,
    ).not.toContain('operator.public.old_google_cloud_nexus.manual_note')
    expect(
      projectPylonV02OmegaReleaseGate(
        withOperatorTransition,
        'operator',
        nowIso,
      ).optionalTransitionEvidenceRefs,
    ).toContain('operator.public.old_google_cloud_nexus.manual_note')
  })

  test('adds missing required check blockers instead of treating partial evidence as releasable', () => {
    const ready = currentPylonV02OmegaReleaseGateRecord()
    const projection = projectPylonV02OmegaReleaseGate(
      {
        ...ready,
        checks: ready.checks.filter(
          check => check.checkKind !== 'public_receipt_page',
        ),
      },
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.missingRequiredCheckRefs).toContain(
      'missing.public.pylon_v0_2.omega_gate.public_receipt_page',
    )
  })

  test('keeps current gate blocked if Artanis real-assignment evidence is removed', () => {
    const current = currentPylonV02OmegaReleaseGateRecord()
    const projection = projectPylonV02OmegaReleaseGate(
      {
        ...current,
        checks: current.checks.filter(
          check => check.checkKind !== 'artanis_real_small_bitcoin_assignment',
        ),
      },
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.missingRequiredCheckRefs).toContain(
      'missing.public.pylon_v0_2.omega_gate.artanis_real_small_bitcoin_assignment',
    )
  })

  test('rejects raw payment, wallet, provider, timestamp, and release authority material', () => {
    const current = currentPylonV02OmegaReleaseGateRecord()

    expect(() =>
      projectPylonV02OmegaReleaseGate(
        {
          ...current,
          checks: current.checks.map(check =>
            check.checkKind === 'real_two_wallet_mdk_movement'
              ? {
                  ...check,
                  evidenceRefs: ['payment_hash.raw_secret'],
                }
              : check,
          ),
        },
        'operator',
        nowIso,
      ),
    ).toThrow(PylonV02OmegaReleaseGateUnsafe)

    expect(() =>
      projectPylonV02OmegaReleaseGate(
        {
          ...current,
          authority: {
            ...PYLON_V02_OMEGA_RELEASE_GATE_NO_AUTHORITY,
            releasePublicationAllowed: true,
          },
        },
        'operator',
        nowIso,
      ),
    ).toThrow(PylonV02OmegaReleaseGateUnsafe)

    expect(() =>
      projectPylonV02OmegaReleaseGate(
        {
          ...current,
          checks: current.checks.map(check =>
            check.checkKind === 'agents_openapi_current'
              ? {
                  ...check,
                  evidenceRefs: ['evidence.public.2026-06-07T12:00:00.000Z'],
                }
              : check,
          ),
        },
        'operator',
        nowIso,
      ),
    ).toThrow(PylonV02OmegaReleaseGateUnsafe)
  })
})
