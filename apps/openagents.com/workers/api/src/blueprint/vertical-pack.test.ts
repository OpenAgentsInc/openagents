import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { BlueprintContextPack } from './schemas/source-context'
import {
  VerticalPack,
  VerticalPackComplianceProfile,
  VerticalPackEthicalMarketingPolicy,
  VerticalPackScreenPolicy,
  VerticalPackStageTemplate,
  VerticalPackStarterWorkflow,
  VerticalPackVerificationRubric,
  agencyVerticalPack,
  ecommerceVerticalPack,
  getVerticalPack,
  healthVerticalPack,
  legalVerticalPack,
  servicesBusinessVerticalPack,
  verticalPackRegistry,
  verticalPackStageOrder,
} from './vertical-pack'

const builtInPacks = [
  legalVerticalPack,
  healthVerticalPack,
  agencyVerticalPack,
  ecommerceVerticalPack,
]

describe('vertical config packs', () => {
  test('the BF-4.8 vertical packs are registered as data', () => {
    expect(builtInPacks.map(pack => pack.id)).toEqual([
      'vertical_pack.legal',
      'vertical_pack.health',
      'vertical_pack.agency',
      'vertical_pack.ecommerce',
    ])

    for (const pack of builtInPacks) {
      expect(getVerticalPack(pack.id)).toBe(pack)
      expect(verticalPackRegistry[pack.id]).toBe(pack)
    }
  })

  test('every built-in pack decodes against its schema', () => {
    for (const pack of builtInPacks) {
      expect(S.decodeUnknownSync(VerticalPack)(pack)).toEqual(pack)
      expect(
        S.decodeUnknownSync(BlueprintContextPack)(pack.contextPack),
      ).toEqual(pack.contextPack)
      expect(
        S.decodeUnknownSync(VerticalPackEthicalMarketingPolicy)(
          pack.ethicalMarketingPolicy,
        ),
      ).toEqual(pack.ethicalMarketingPolicy)
      expect(
        S.decodeUnknownSync(VerticalPackVerificationRubric)(
          pack.verificationRubric,
        ),
      ).toEqual(pack.verificationRubric)
      expect(
        S.decodeUnknownSync(VerticalPackComplianceProfile)(
          pack.complianceProfile,
        ),
      ).toEqual(pack.complianceProfile)
      expect(
        S.decodeUnknownSync(VerticalPackScreenPolicy)(pack.screenPolicy),
      ).toEqual(pack.screenPolicy)
    }
  })

  test('stage templates preserve the canonical fulfillment stage keys', () => {
    for (const pack of builtInPacks) {
      expect(pack.stageTemplates.map(stage => stage.stageKey)).toEqual(
        verticalPackStageOrder,
      )

      for (const stageTemplate of pack.stageTemplates) {
        expect(
          S.decodeUnknownSync(VerticalPackStageTemplate)(stageTemplate),
        ).toEqual(stageTemplate)
        expect(stageTemplate.displayName.length).toBeGreaterThan(0)
        expect(stageTemplate.requiredEvidenceRefs).toEqual(
          expect.arrayContaining([
            `evidence.${stageTemplate.stageKey}.source_refs`,
            `evidence.${stageTemplate.stageKey}.decision_or_receipt`,
          ]),
        )
      }
    }
  })

  test('starter workflows require the shared stage pipeline', () => {
    for (const pack of builtInPacks) {
      expect(pack.starterWorkflows.length).toBeGreaterThanOrEqual(2)

      for (const workflow of pack.starterWorkflows) {
        expect(
          S.decodeUnknownSync(VerticalPackStarterWorkflow)(workflow),
        ).toEqual(workflow)
        expect(workflow.startingStage).toBe('signal')
        expect(workflow.requiredStageKeys).toEqual(verticalPackStageOrder)
        expect(workflow.workflowRef).toContain(`workflow.${pack.vertical}.`)
      }
    }
  })

  test('verification rubrics enforce grounded delivery and approval receipts', () => {
    for (const pack of builtInPacks) {
      const criterionRefs = pack.verificationRubric.criteria.map(
        criterion => criterion.criterionRef,
      )
      const evidenceRefs = pack.verificationRubric.criteria.flatMap(
        criterion => criterion.requiredEvidenceRefs,
      )

      expect(pack.verificationRubric.rubricRef).toBe(
        `rubric.vertical_pack.${pack.vertical}.v1`,
      )
      expect(pack.verificationRubric.reviewGateRef).toBe(
        `review_gate.${pack.vertical}.fulfillment`,
      )
      expect(criterionRefs).toEqual(
        expect.arrayContaining([
          `criterion.${pack.vertical}.grounded_sources`,
          `criterion.${pack.vertical}.approval_recorded`,
        ]),
      )
      expect(evidenceRefs).toEqual(
        expect.arrayContaining([
          'evidence.source_map',
          'evidence.provenance_receipt',
          'evidence.approval_decision',
          'evidence.action_receipt',
        ]),
      )
    }
  })

  test('compliance profiles gate outbound actions for each vertical', () => {
    for (const pack of builtInPacks) {
      expect(pack.complianceProfile.profileRef).toBe(
        `compliance_profile.${pack.vertical}`,
      )
      expect(pack.complianceProfile.requiresHumanReview).toBe(true)
      expect(pack.complianceProfile.consentChannelRefs).toEqual(
        expect.arrayContaining([
          'consent.customer_provided_sources',
          'consent.outbound_action_approval',
        ]),
      )
      expect(pack.complianceProfile.outboundActionGateRefs).toEqual(
        expect.arrayContaining([
          'gate.human_approval_before_send_publish_file_or_spend',
          'gate.provenance_before_customer_delivery',
        ]),
      )
      expect(
        pack.complianceProfile.prohibitedActionRefs.length,
      ).toBeGreaterThan(0)
    }

    expect(legalVerticalPack.complianceProfile.professionalReviewRequired).toBe(
      true,
    )
    expect(
      healthVerticalPack.complianceProfile.professionalReviewRequired,
    ).toBe(true)
    expect(
      agencyVerticalPack.complianceProfile.professionalReviewRequired,
    ).toBe(false)
    expect(
      ecommerceVerticalPack.complianceProfile.professionalReviewRequired,
    ).toBe(false)
  })

  test('new verticals onboard through shared screens only', () => {
    for (const pack of builtInPacks) {
      expect(pack.screenPolicy.sharedSurfaceRefs).toEqual(
        expect.arrayContaining([
          'surface.omni_workroom',
          'surface.approval_ladder',
          'surface.evidence_bundle',
          'surface.customer_handoff',
        ]),
      )
      expect(pack.screenPolicy.bespokeScreenRefs).toEqual([])
      expect(pack.screenPolicy.reviewFailureRef).toBe(
        'review.failure.vertical_requires_bespoke_screen',
      )
    }
  })

  test('source authorities scope consent and projection posture', () => {
    for (const pack of builtInPacks) {
      const sources = pack.contextPack.sourceAuthorities

      expect(
        sources.some(
          source =>
            !source.includedInContext &&
            source.dataClassification === 'private',
        ),
      ).toBe(true)
      expect(sources.some(source => source.publicSafe)).toBe(true)
      expect(
        sources.some(source => source.consentState === 'customer_provided'),
      ).toBe(true)
      expect(pack.contextPack.includedContextRefs).not.toContain(
        expect.stringMatching(/^email\.raw_/),
      )
    }
  })

  test('registry lookup returns undefined for unknown ids', () => {
    expect(getVerticalPack('vertical_pack.unknown')).toBeUndefined()
  })

  test('the old services-business export remains an agency pack alias', () => {
    expect(servicesBusinessVerticalPack).toBe(agencyVerticalPack)
    expect(getVerticalPack('vertical_pack.services_business')).toBe(
      servicesBusinessVerticalPack,
    )
  })
})
