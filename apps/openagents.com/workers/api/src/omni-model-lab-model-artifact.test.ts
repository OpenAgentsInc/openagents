import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_MODEL_ARTIFACT_READ_ONLY_AUTHORITY,
  OmniModelArtifactProjection,
  OmniModelArtifactRecord,
  OmniModelArtifactUnsafe,
  exampleOmniModelArtifact,
  omniModelArtifactProjectionHasPrivateMaterial,
  projectOmniModelArtifact,
} from './omni-model-lab-model-artifact'

const nowIso = '2026-06-06T23:30:00.000Z'

const artifactRecord = (
  overrides: Partial<OmniModelArtifactRecord> = {},
): OmniModelArtifactRecord =>
  S.decodeUnknownSync(OmniModelArtifactRecord)({
    ...exampleOmniModelArtifact(),
    ...overrides,
  })

describe('Omni Model Lab model artifact', () => {
  test('projects a reviewed artifact without training, runtime, routing, payment, settlement, or public-claim authority', () => {
    const projection = projectOmniModelArtifact(
      exampleOmniModelArtifact(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniModelArtifactProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      createdAtDisplay: '30 minutes ago',
      digestCount: 1,
      modelTrainingStartAllowed: false,
      payoutMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      rawWeightCopyAllowed: false,
      readiness: 'reviewed',
      readinessLabel: 'Reviewed evidence',
      redistributionAllowed: false,
      routingMutationAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      state: 'approved',
      stateLabel: 'Approved for reviewed use',
      storageState: 'digest_only',
      trainingReuseAllowed: true,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_MODEL_ARTIFACT_READ_ONLY_AUTHORITY,
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniModelArtifactProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('keeps readiness, rollback, storage, and rights caveats explicit', () => {
    expect(projectOmniModelArtifact(
      artifactRecord({ state: 'retained' }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'retained',
      rollback: { rollbackPosture: 'ready' },
      storageState: 'digest_only',
    })
    expect(projectOmniModelArtifact(
      artifactRecord({ state: 'validated' }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'validation_ready',
    })
    expect(projectOmniModelArtifact(
      artifactRecord({
        safety: {
          ...exampleOmniModelArtifact().safety,
          blockedReasonRefs: ['blocked.public.license_review'],
        },
        state: 'blocked',
      }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'blocked',
      readinessLabel: 'Blocked',
    })
    expect(projectOmniModelArtifact(
      artifactRecord({
        rights: {
          caveatRefs: ['caveat.public.apache_notice'],
          licenseRefs: ['license.public.apache_2'],
          redistributionAllowed: true,
          rightsState: 'redistributable',
          trainingReuseAllowed: true,
        },
        state: 'review_ready',
      }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'reviewed',
      redistributionAllowed: true,
      trainingReuseAllowed: true,
    })
  })

  test('requires source refs, digest evidence, eval/safety refs, approved rollback posture, and rights consistency', () => {
    for (const badRecord of [
      artifactRecord({ artifactDigests: [] }),
      artifactRecord({ sourceRefs: [] }),
      artifactRecord({
        artifactDigests: [
          {
            ...exampleOmniModelArtifact().artifactDigests[0]!,
            evidenceRefs: [],
          },
        ],
      }),
      artifactRecord({
        artifactDigests: [
          {
            ...exampleOmniModelArtifact().artifactDigests[0]!,
            noRawWeightCopy: false,
          },
        ],
      }),
      artifactRecord({
        evalRefs: [],
        state: 'validated',
      }),
      artifactRecord({
        rollback: {
          ...exampleOmniModelArtifact().rollback,
          rollbackPosture: 'candidate',
        },
        state: 'approved',
      }),
      artifactRecord({
        benchmarkRefs: [],
        state: 'approved',
      }),
      artifactRecord({
        rights: {
          caveatRefs: ['caveat.public.unknown_rights'],
          licenseRefs: [],
          redistributionAllowed: true,
          rightsState: 'restricted',
          trainingReuseAllowed: false,
        },
      }),
      artifactRecord({
        rights: {
          caveatRefs: ['caveat.public.unknown_rights'],
          licenseRefs: [],
          redistributionAllowed: false,
          rightsState: 'unknown',
          trainingReuseAllowed: true,
        },
      }),
      artifactRecord({
        safety: {
          ...exampleOmniModelArtifact().safety,
          blockedReasonRefs: [],
        },
        state: 'blocked',
      }),
    ]) {
      expect(() =>
        projectOmniModelArtifact(badRecord, 'operator', nowIso),
      ).toThrow(OmniModelArtifactUnsafe)
    }
  })

  test('redacts private providers, storage, source, training, digest, benchmark, and rollback refs publicly', () => {
    const projection = projectOmniModelArtifact(
      artifactRecord({
        artifactDigests: [
          {
            ...exampleOmniModelArtifact().artifactDigests[0]!,
            digestRef: 'digest.private.operator_sha256',
            evidenceRefs: [
              'evidence.public.digest_manifest',
              'digest.private.operator_evidence',
            ],
          },
        ],
        artifactRef: 'artifact.private.operator_artifact',
        benchmarkRefs: [
          'benchmark.public.regression_suite',
          'benchmark.private.operator_suite',
        ],
        evalRefs: [
          'eval.public.regression_pass',
          'eval.private.operator_eval',
        ],
        providerRefs: [
          'provider.public.psionic_lab',
          'provider.private.operator_gpu',
        ],
        rollback: {
          priorArtifactRefs: [
            'artifact.public.previous',
            'artifact.private.previous_operator',
          ],
          rollbackPosture: 'ready',
          rollbackRefs: [
            'rollback.public.restore',
            'rollback.private.operator_restore',
          ],
        },
        safety: {
          ...exampleOmniModelArtifact().safety,
          safetyReviewRefs: [
            'safety.public.operator_reviewed',
            'safety.private.internal_review',
          ],
        },
        sourceRefs: [
          'source.public.summary',
          'source.private.operator_archive',
        ],
        storageRefs: [
          'storage.public.digest_manifest_only',
          'storage.private.operator_bucket',
        ],
        trainingRunRefs: [
          'training_run.public.adapter_tune',
          'training_run.private.operator_run',
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.artifactRef).toBe('artifact.redacted.model_artifact')
    expect(projection.providerRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.storageRefs).toEqual([])
    expect(projection.trainingRunRefs).toEqual([
      'training_run.public.adapter_tune',
    ])
    expect(projection.artifactDigests[0]!.digestRef).toBe(
      'digest.redacted.model_artifact',
    )
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator_bucket')
    expect(omniModelArtifactProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('rejects raw weights, provider payloads, private datasets, secrets, payment material, raw timestamps, and mutable authority', () => {
    for (const badRecord of [
      artifactRecord({ storageRefs: ['weights.safetensors'] }),
      artifactRecord({ sourceRefs: ['source_archive.raw'] }),
      artifactRecord({ providerRefs: ['provider_payload.raw'] }),
      artifactRecord({ retainedFailureRefs: ['dataset.private.customer'] }),
      artifactRecord({ caveatRefs: ['secret.openai_api_key'] }),
      artifactRecord({ caveatRefs: ['payment_preimage.raw'] }),
      artifactRecord({ sourceRefs: ['source.public.2026-06-06T23:00:00'] }),
      artifactRecord({
        authority: {
          ...OMNI_MODEL_ARTIFACT_READ_ONLY_AUTHORITY,
          noRuntimePromotion: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniModelArtifact(badRecord, 'operator', nowIso),
      ).toThrow(OmniModelArtifactUnsafe)
    }
  })
})
