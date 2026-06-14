import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BlueprintProgramType as BlueprintProgramTypeSchema,
  blueprintProgramTypeRequiredReceiptRefs,
  blueprintProgramTypeRequiresApproval,
} from './schemas/program'
import {
  DELIVERY_PIPELINE_PROGRAM_TYPES,
  DELIVERY_PIPELINE_PROGRAMS,
  DELIVERY_PIPELINE_STAGE_ORDER,
  DeliveryPipelineBrandStoryOutput,
  DeliveryPipelineLeadMagnetOutput,
  DeliveryPipelineNurtureSequenceOutput,
  DeliveryPipelineOfferSuiteOutput,
  DeliveryPipelineSalesSequenceOutput,
  DeliveryPipelineWebCopyOutput,
  type DeliveryPipelineStage,
  deliveryPipelineProgramForStage,
  deliveryPipelineProgramTypeId,
} from './delivery-pipeline-programs'

describe('delivery-pipeline Blueprint programs', () => {
  test('covers exactly the seven pipeline stages in order', () => {
    expect(DELIVERY_PIPELINE_PROGRAMS.map(program => program.stage)).toEqual([
      'brand-story',
      'web-copy',
      'brand-style',
      'offer-suite',
      'lead-magnet',
      'nurture-sequence',
      'sales-sequence',
    ])
    expect(DELIVERY_PIPELINE_STAGE_ORDER).toHaveLength(7)
    expect(DELIVERY_PIPELINE_PROGRAM_TYPES).toHaveLength(7)
  })

  test('each program type decodes against the Blueprint Program schema', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      expect(
        S.decodeUnknownSync(BlueprintProgramTypeSchema)(program.programType),
      ).toEqual(program.programType)
    }
  })

  test('each program is evidence-only and approval-gated for publication', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      expect(program.programType.directMutationAllowed).toBe(false)
      expect(program.programType.family).toBe('artifact_review')
      expect(program.programType.status).toBe('draft')
      // A propose_action publish scope means publication requires approval.
      expect(
        blueprintProgramTypeRequiresApproval(program.programType),
      ).toBe(true)
    }
  })

  test('each program requires a context pack and a program-run receipt', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      const requiredEvidence = program.programType.evidenceRequirements.filter(
        requirement => requirement.required,
      )
      expect(
        requiredEvidence.some(
          requirement => requirement.kind === 'context_pack_ref',
        ),
      ).toBe(true)

      expect(
        blueprintProgramTypeRequiredReceiptRefs(program.programType),
      ).toContain('receipt.program_run')
    }
  })

  test('every stage after brand-story requires an upstream artifact ref', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      const requiresUpstreamArtifact =
        program.programType.evidenceRequirements.some(
          requirement =>
            requirement.kind === 'artifact_ref' && requirement.required,
        )

      if (program.stage === 'brand-story') {
        expect(requiresUpstreamArtifact).toBe(false)
      } else {
        expect(requiresUpstreamArtifact).toBe(true)
      }
    }
  })

  test('each program carries a required operator_review release gate', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      const requiredGates = program.programType.releaseGates.filter(
        gate => gate.required,
      )
      expect(requiredGates).toHaveLength(1)
      expect(requiredGates[0]?.gateKind).toBe('operator_review')
    }
  })

  test('program type ids are stable and derivable from the stage slug', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      expect(program.programType.id).toBe(
        deliveryPipelineProgramTypeId(program.stage),
      )
      expect(program.programType.id.startsWith('program_type.delivery_pipeline.')).toBe(
        true,
      )
    }
  })

  test('registry lookup returns each stage program', () => {
    const stages: ReadonlyArray<DeliveryPipelineStage> = [
      'brand-story',
      'web-copy',
      'brand-style',
      'offer-suite',
      'lead-magnet',
      'nurture-sequence',
      'sales-sequence',
    ]

    for (const stage of stages) {
      const program = deliveryPipelineProgramForStage(stage)
      expect(program).toBeDefined()
      expect(program?.stage).toBe(stage)
      expect(program?.outputSchemaRef).toBe(
        `schema.delivery_pipeline.${stage.replace(/-/g, '_')}.output.v1`,
      )
    }
  })

  test('lookup returns undefined for an unknown stage', () => {
    expect(
      deliveryPipelineProgramForStage(
        'not-a-stage' as unknown as DeliveryPipelineStage,
      ),
    ).toBeUndefined()
  })

  test('typed output schemas decode structured fixtures', () => {
    expect(
      S.decodeUnknownSync(DeliveryPipelineBrandStoryOutput)({
        brandName: 'Acme',
        missionStatement: 'mission',
        positioningStatement: 'positioning',
        originStory: 'origin',
        coreValues: ['craft', 'speed'],
        audienceSummary: 'founders',
        toneDescriptors: ['confident', 'plain'],
        sourceContextRefs: ['context.brand.intake.v1'],
      }),
    ).toMatchObject({ brandName: 'Acme' })

    expect(
      S.decodeUnknownSync(DeliveryPipelineWebCopyOutput)({
        pageRef: 'page.home',
        metaTitle: 'Home',
        metaDescription: 'desc',
        sections: [
          {
            sectionRef: 'hero',
            headline: 'Headline',
            body: 'Body',
            callToAction: 'Sign up',
          },
        ],
        primaryCallToAction: 'Sign up',
        brandStoryRef: 'artifact.brand_story.v1',
        sourceContextRefs: ['context.web_copy.v1'],
      }),
    ).toMatchObject({ pageRef: 'page.home' })

    expect(
      S.decodeUnknownSync(DeliveryPipelineOfferSuiteOutput)({
        offers: [
          {
            offerRef: 'offer.core',
            name: 'Core',
            summary: 'summary',
            priceLabel: '$99',
            deliverables: ['a', 'b'],
            positioningTier: 'core',
          },
        ],
        anchorOfferRef: 'offer.core',
        valueLadderSummary: 'ladder',
        brandStoryRef: 'artifact.brand_story.v1',
        sourceContextRefs: ['context.offer.v1'],
      }),
    ).toMatchObject({ anchorOfferRef: 'offer.core' })

    expect(
      S.decodeUnknownSync(DeliveryPipelineLeadMagnetOutput)({
        title: 'Guide',
        format: 'pdf',
        promise: 'promise',
        outline: ['ch1'],
        optInHeadline: 'Get it',
        optInBody: 'Body',
        targetOfferRef: 'offer.core',
        sourceContextRefs: ['context.lead_magnet.v1'],
      }),
    ).toMatchObject({ title: 'Guide' })

    expect(
      S.decodeUnknownSync(DeliveryPipelineNurtureSequenceOutput)({
        sequenceRef: 'seq.nurture',
        goalSummary: 'warm leads',
        emails: [
          {
            stepRef: 'step.1',
            sendOffsetHours: 0,
            subject: 'Welcome',
            previewText: 'preview',
            body: 'body',
            callToAction: null,
          },
        ],
        leadMagnetRef: 'artifact.lead_magnet.v1',
        sourceContextRefs: ['context.nurture.v1'],
      }),
    ).toMatchObject({ sequenceRef: 'seq.nurture' })

    expect(
      S.decodeUnknownSync(DeliveryPipelineSalesSequenceOutput)({
        sequenceRef: 'seq.sales',
        goalSummary: 'convert',
        emails: [
          {
            stepRef: 'step.1',
            sendOffsetHours: 24,
            subject: 'Offer',
            previewText: 'preview',
            body: 'body',
            callToAction: 'Buy',
          },
        ],
        offerSuiteRef: 'artifact.offer_suite.v1',
        nurtureSequenceRef: 'artifact.nurture_sequence.v1',
        sourceContextRefs: ['context.sales.v1'],
      }),
    ).toMatchObject({ sequenceRef: 'seq.sales' })
  })

  test('output schemas reject unknown stages-typed misuse and missing fields', () => {
    expect(() =>
      S.decodeUnknownSync(DeliveryPipelineBrandStoryOutput)({
        brandName: 'Acme',
      }),
    ).toThrow()
  })
})
