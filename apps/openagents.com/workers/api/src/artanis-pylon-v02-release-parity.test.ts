import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_PYLON_V02_RELEASE_PARITY_NO_AUTHORITY,
  ArtanisPylonV02ReleaseParityProjection,
  ArtanisPylonV02ReleaseParityUnsafe,
  exampleArtanisPylonV02ReleaseParityEvidence,
  projectArtanisPylonV02ReleaseParity,
  releaseReadyArtanisPylonV02ReleaseParityEvidence,
} from './artanis-pylon-v02-release-parity'

const nowIso = '2026-06-07T08:00:00.000Z'

describe('Artanis Pylon v0.2 release parity', () => {
  test('projects no-release evidence as source-visible but blocked', () => {
    const projection = projectArtanisPylonV02ReleaseParity(
      exampleArtanisPylonV02ReleaseParityEvidence(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisPylonV02ReleaseParityProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkClaimAllowed: false,
      audience: 'public',
      eligibilityMutationAllowed: false,
      generalAvailabilityClaimAllowed: false,
      packagePublishAllowed: false,
      packageVersionMatched: false,
      platformReady: false,
      privateEvidenceRefs: [],
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      releasePublicationAllowed: false,
      releaseReady: false,
      releaseTagRef: null,
      settlementAllowed: false,
      shippedClaimAllowed: false,
      sourceLevelSupportVisible: true,
      state: 'blocked',
      walletSpendAllowed: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.pylon_v0_2.release_tag_missing',
        'blocker.public.pylon_v0_2.package_version_mismatch',
        'blocker.public.pylon_v0_2.runtime_smoke_missing',
        'blocker.public.pylon_v0_2.eligibility_telemetry_missing',
        'blocker.public.pylon_v0_2.accepted_work_proof_missing',
        'blocker.public.pylon_v0_2.paid_work_receipt_missing',
        'blocker.public.pylon_v0_2.settlement_receipt_missing',
      ]),
    )
    expect(projection.publicClaimSummary).not.toContain(
      'Pylon v0.2 is shipped',
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('keeps source-only support distinct from release parity', () => {
    const input = {
      ...exampleArtanisPylonV02ReleaseParityEvidence(),
      packageVersionRefs: [],
      packageVersionState: 'missing' as const,
      releaseAssetRefs: [],
      sourceSupportRefs: [
        'source.public.pylon_v0_2_ldk_target_contract',
      ],
    }
    const projection = projectArtanisPylonV02ReleaseParity(
      input,
      'public',
      nowIso,
    )

    expect(projection.sourceLevelSupportVisible).toBe(true)
    expect(projection.releaseReady).toBe(false)
    expect(projection.stages.find(stage => stage.stage === 'source_support'))
      .toMatchObject({ state: 'verified' })
    expect(projection.stages.find(stage => stage.stage === 'release_assets'))
      .toMatchObject({ state: 'blocked' })
  })

  test('blocks release tag without required assets', () => {
    const projection = projectArtanisPylonV02ReleaseParity(
      {
        ...exampleArtanisPylonV02ReleaseParityEvidence(),
        packageVersionRefs: ['version.public.pylon.package.0_2_0'],
        packageVersionState: 'matched',
        releaseAssetRefs: [],
        releaseTag: 'pylon-v0.2.0',
        runtimeSmokeRefs: ['smoke.public.pylon_v0_2.runtime.first_boot'],
      },
      'public',
      nowIso,
    )

    expect(projection.releaseTagRef).toBe(
      'release.public.openagents.pylon_v0_2_0',
    )
    expect(projection.releaseReady).toBe(false)
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'missing.public.pylon_v0_2.release_asset.asset_public_openagents_pylon_v0_2_0_linux_x64',
        'missing.public.pylon_v0_2.release_asset.asset_public_openagents_pylon_v0_2_0_windows_x64',
      ]),
    )
  })

  test('blocks package version mismatch even with release assets', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()
    const projection = projectArtanisPylonV02ReleaseParity(
      {
        ...input,
        packageVersionRefs: ['version.public.pylon.package.0_1_23'],
        packageVersionState: 'mismatched',
      },
      'public',
      nowIso,
    )

    expect(projection.packageVersionMatched).toBe(false)
    expect(projection.releaseReady).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.pylon_v0_2.package_version_mismatch',
    )
  })

  test('blocks missing platform smoke after release readiness', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()
    const projection = projectArtanisPylonV02ReleaseParity(
      {
        ...input,
        platformSmokeRefs: [
          'smoke.public.pylon.v0_2.platform.macos_apple_silicon',
        ],
      },
      'public',
      nowIso,
    )

    expect(projection.releaseReady).toBe(true)
    expect(projection.platformReady).toBe(false)
    expect(projection.generalAvailabilityClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'missing.public.pylon_v0_2.platform_smoke.smoke_public_pylon_v0_2_platform_linux',
        'missing.public.pylon_v0_2.platform_smoke.smoke_public_pylon_v0_2_platform_native_windows',
      ]),
    )
  })

  test('blocks missing eligibility telemetry and payment target registration', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()
    const projection = projectArtanisPylonV02ReleaseParity(
      {
        ...input,
        eligibilityTelemetryRefs: [],
        paymentTargetRegistrationRefs: [],
      },
      'public',
      nowIso,
    )

    expect(projection.eligibilityReady).toBe(false)
    expect(projection.generalAvailabilityClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.pylon_v0_2.eligibility_telemetry_missing',
        'blocker.public.pylon_v0_2.payment_target_registration_missing',
      ]),
    )
  })

  test('blocks missing accepted-work proof and paid/settled receipts', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()
    const projection = projectArtanisPylonV02ReleaseParity(
      {
        ...input,
        acceptedWorkProofRefs: [],
        paidWorkReceiptRefs: [],
        settlementReceiptRefs: [],
      },
      'public',
      nowIso,
    )

    expect(projection.acceptedWorkClaimAllowed).toBe(false)
    expect(projection.paidClaimAllowed).toBe(false)
    expect(projection.settledClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.pylon_v0_2.accepted_work_proof_missing',
        'blocker.public.pylon_v0_2.paid_work_receipt_missing',
        'blocker.public.pylon_v0_2.settlement_receipt_missing',
      ]),
    )
  })

  test('projects fully release-ready modeled evidence', () => {
    const projection = projectArtanisPylonV02ReleaseParity(
      releaseReadyArtanisPylonV02ReleaseParityEvidence(),
      'public',
      nowIso,
    )

    expect(projection).toMatchObject({
      acceptedWorkClaimAllowed: true,
      eligibilityReady: true,
      generalAvailabilityClaimAllowed: true,
      packageVersionMatched: true,
      paidClaimAllowed: true,
      platformReady: true,
      releaseReady: true,
      settledClaimAllowed: true,
      shippedClaimAllowed: true,
      state: 'verified',
      stateLabel: 'Pylon v0.2 release parity modeled complete',
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.stageSummaryRefs).toEqual(
      expect.arrayContaining([
        'stage_summary.public.pylon_v0_2.release_parity.release_assets.verified',
        'stage_summary.public.pylon_v0_2.release_parity.platform_smoke.verified',
        'stage_summary.public.pylon_v0_2.release_parity.settlement.verified',
      ]),
    )
  })

  test('redacts operator evidence for public and retains safe refs for operator', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()
    const publicProjection = projectArtanisPylonV02ReleaseParity(
      input,
      'public',
      nowIso,
    )
    const operatorProjection = projectArtanisPylonV02ReleaseParity(
      input,
      'operator',
      nowIso,
    )

    expect(publicProjection.privateEvidenceRefs).toEqual([])
    expect(operatorProjection.privateEvidenceRefs).toEqual([
      'evidence.operator.pylon_v0_2.release_command_redacted',
    ])
  })

  test('rejects raw payout targets, payment, wallet, provider, raw command, and private telemetry material', () => {
    const input = releaseReadyArtanisPylonV02ReleaseParityEvidence()

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          paymentTargetRegistrationRefs: ['payout_target.raw.bc1secret'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          paidWorkReceiptRefs: ['payment_proof.raw_invoice'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          privateEvidenceRefs: ['wallet.secret.material'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          sourceRefs: ['provider_secret.token'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          runtimeSmokeRefs: ['release_command_output.raw'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)

    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...input,
          eligibilityTelemetryRefs: ['private_node_telemetry.full'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)
  })

  test('rejects mutable release authority', () => {
    expect(() =>
      projectArtanisPylonV02ReleaseParity(
        {
          ...releaseReadyArtanisPylonV02ReleaseParityEvidence(),
          authority: {
            ...ARTANIS_PYLON_V02_RELEASE_PARITY_NO_AUTHORITY,
            releasePublicationAllowed: true,
          },
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisPylonV02ReleaseParityUnsafe)
  })
})
