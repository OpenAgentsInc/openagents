import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY,
  OmniModelLabReportProjection,
  OmniModelLabReportRecord,
  OmniModelLabReportUnsafe,
  exampleOmniModelLabReport,
  omniModelLabReportProjectionHasPrivateMaterial,
  projectOmniModelLabReport,
} from './omni-model-lab-report'

const nowIso = '2026-06-07T00:40:00.000Z'

const reportRecord = (
  overrides: Partial<OmniModelLabReportRecord> = {},
): OmniModelLabReportRecord =>
  S.decodeUnknownSync(OmniModelLabReportRecord)({
    ...exampleOmniModelLabReport(),
    ...overrides,
  })

describe('Omni Model Lab report projection', () => {
  test('projects a complete report without training, eval, provider, adapter, raw-artifact, report-publication, runtime, payment, payout, settlement, or public-claim authority', () => {
    const projection = projectOmniModelLabReport(
      exampleOmniModelLabReport(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniModelLabReportProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      blockedSectionCount: 0,
      claimState: 'promotion_passed_not_deployed',
      completeSectionCount: 9,
      createdAtDisplay: '20 minutes ago',
      evalExecutionAllowed: false,
      missingSectionCount: 0,
      partialSectionCount: 0,
      paymentSpendAllowed: false,
      payoutMutationAllowed: false,
      providerCallAllowed: false,
      publicClaimMutationAllowed: false,
      rawArtifactExportAllowed: false,
      readiness: 'complete',
      reportPublicationAllowed: false,
      runtimePromotionAllowed: false,
      sectionCount: 9,
      settlementMutationAllowed: false,
      trainingLaunchAllowed: false,
      updatedAtDisplay: '14 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY,
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(omniModelLabReportProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('validates section completeness, duplicate sections, missing evidence, blocked readiness, and claim-state caveats', () => {
    const base = exampleOmniModelLabReport()
    const partialSection = {
      ...base.sections[0]!,
      caveatRefs: ['caveat.public.retained_failure_needs_rerun'],
      evidenceRefs: [],
      missingEvidenceRefs: ['missing.public.retained_failure_rerun'],
      readiness: 'partial' as const,
    }

    const partialProjection = projectOmniModelLabReport(
      reportRecord({
        missingEvidenceRefs: ['missing.public.retained_failure_rerun'],
        readiness: 'partial',
        sections: [partialSection, ...base.sections.slice(1)],
      }),
      'operator',
      nowIso,
    )

    expect(partialProjection.readiness).toBe('partial')
    expect(partialProjection.partialSectionCount).toBe(1)

    const missingSection = {
      ...base.sections[0]!,
      evidenceRefs: [],
      itemRefs: [],
      missingEvidenceRefs: ['missing.public.retained_failure_packet'],
      readiness: 'missing_evidence' as const,
    }
    const missingProjection = projectOmniModelLabReport(
      reportRecord({
        claimState: 'missing_evidence',
        missingEvidenceRefs: ['missing.public.retained_failure_packet'],
        readiness: 'missing_evidence',
        sections: [missingSection, ...base.sections.slice(1)],
      }),
      'operator',
      nowIso,
    )

    expect(missingProjection.readiness).toBe('missing_evidence')
    expect(missingProjection.missingSectionCount).toBe(1)

    const blockedSection = {
      ...base.sections[0]!,
      blockerRefs: ['blocker.public.benchmark_cloud_unavailable'],
      readiness: 'blocked' as const,
    }
    const blockedProjection = projectOmniModelLabReport(
      reportRecord({
        blockerRefs: ['blocker.public.benchmark_cloud_unavailable'],
        claimState: 'blocked',
        readiness: 'blocked',
        sections: [blockedSection, ...base.sections.slice(1)],
      }),
      'operator',
      nowIso,
    )

    expect(blockedProjection.readiness).toBe('blocked')
    expect(blockedProjection.blockedSectionCount).toBe(1)

    for (const badReport of [
      reportRecord({ sections: [] }),
      reportRecord({
        sections: [
          base.sections[0]!,
          { ...base.sections[0]!, sectionRef: 'section.public.duplicate' },
          ...base.sections.slice(2),
        ],
      }),
      reportRecord({
        sections: [
          { ...base.sections[0]!, evidenceRefs: [], readiness: 'complete' },
          ...base.sections.slice(1),
        ],
      }),
      reportRecord({
        claimState: 'promotion_passed_not_deployed',
        caveatRefs: [],
      }),
      reportRecord({
        claimState: 'missing_evidence',
        missingEvidenceRefs: [],
      }),
      reportRecord({
        readiness: 'blocked',
        blockerRefs: [],
      }),
    ]) {
      expect(() =>
        projectOmniModelLabReport(badReport, 'operator', nowIso),
      ).toThrow(OmniModelLabReportUnsafe)
    }
  })

  test('redacts private report refs, artifacts, candidates, benchmark evidence, promotion decisions, rollback, attribution, marketplace memory, and withheld classes publicly', () => {
    const base = exampleOmniModelLabReport()
    const projection = projectOmniModelLabReport(
      reportRecord({
        artifactRefs: ['artifact.private.operator_lora'],
        attributionRefs: ['attribution.private.operator_outcome'],
        benchmarkEvidenceRefs: ['benchmark.private.operator_cloud'],
        candidateRefs: ['candidate.private.operator_candidate'],
        id: 'report.private.operator_model_lab',
        marketplaceMemoryRefs: ['marketplace.private.operator_memory'],
        promotionDecisionRefs: ['decision.private.operator_promotion'],
        redactionPolicyRefs: ['policy.private.operator_redaction'],
        reportRef: 'report.private.operator_model_lab',
        retainedFailureRefs: ['failure.private.operator_failure'],
        rollbackRefs: ['rollback.private.operator_restore'],
        sections: [
          {
            ...base.sections[0]!,
            evidenceRefs: ['evidence.private.operator_section'],
            itemRefs: ['candidate.private.operator_candidate'],
            sectionRef: 'section.private.operator_section',
          },
          ...base.sections.slice(1),
        ],
        trainingRunRefs: ['training.private.operator_training'],
        withheldClassRefs: ['withheld.private.input_classes'],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.id).toBe('model-lab-report.redacted')
    expect(projection.reportRef).toBe('report.redacted.model_lab')
    expect(projection.sections[0]!.sectionRef).toBe(
      'section.redacted.model_lab_report',
    )
    expect(projection.redaction.redactedRefCount).toBeGreaterThan(0)
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator')
    expect(omniModelLabReportProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('supports no-public-claim caveats without granting publication authority', () => {
    const projection = projectOmniModelLabReport(
      reportRecord({
        caveatRefs: ['caveat.public.report_is_internal_until_review'],
        claimState: 'no_public_claim',
        readiness: 'complete',
      }),
      'public',
      nowIso,
    )

    expect(projection.claimState).toBe('no_public_claim')
    expect(projection.reportPublicationAllowed).toBe(false)
    expect(projection.publicClaimMutationAllowed).toBe(false)
  })

  test('rejects raw artifacts, source archives, provider payloads, model weights, secrets, payment material, raw timestamps, and mutable authority', () => {
    for (const badReport of [
      reportRecord({ caveatRefs: ['raw_prompt.customer'] }),
      reportRecord({ blockerRefs: ['source_archive.raw'] }),
      reportRecord({ caveatRefs: ['provider_payload.raw'] }),
      reportRecord({ blockerRefs: ['dataset.raw.customer'] }),
      reportRecord({ blockerRefs: ['weights.safetensors'] }),
      reportRecord({ caveatRefs: ['secret.report_token'] }),
      reportRecord({ caveatRefs: ['payment_preimage.raw'] }),
      reportRecord({ caveatRefs: ['caveat.public.2026-06-07T00:00:00'] }),
      reportRecord({
        authority: {
          ...OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY,
          noRuntimePromotion: false,
        },
      }),
      reportRecord({
        authority: {
          ...OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY,
          noReportPublication: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniModelLabReport(badReport, 'operator', nowIso),
      ).toThrow(OmniModelLabReportUnsafe)
    }
  })
})
