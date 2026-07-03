import { Schema as S } from 'effect'

import {
  BlueprintContextPack,
  type BlueprintContextPack as BlueprintContextPackType,
  type BlueprintSourceAuthority,
} from './schemas/source-context'

// Vertical Packs are reusable, customer-agnostic Context Pack templates for a
// given line of business. They are config, not forks: the workroom and
// fulfillment UI reads stage templates, rubrics, starter workflows, and
// compliance profiles from these packs instead of introducing per-vertical
// screens.
//
// A Vertical Pack must NOT name a specific customer, brand, matter, patient, or
// campaign. Concrete customer Context Packs are derived from it at runtime by
// binding real source authorities.

export const VerticalPackStageKey = S.Literals([
  'signal',
  'triage',
  'codegen',
  'validate',
  'release',
  'document',
  'monitor',
  'deploy',
])
export type VerticalPackStageKey = typeof VerticalPackStageKey.Type

export const VerticalPackStageTemplate = S.Struct({
  stageKey: VerticalPackStageKey,
  displayName: S.String,
  inputContractRef: S.String,
  outputContractRef: S.String,
  requiredEvidenceRefs: S.Array(S.String),
})
export type VerticalPackStageTemplate = typeof VerticalPackStageTemplate.Type

export const VerticalPackRubricCriterion = S.Struct({
  criterionRef: S.String,
  description: S.String,
  requiredEvidenceRefs: S.Array(S.String),
})
export type VerticalPackRubricCriterion =
  typeof VerticalPackRubricCriterion.Type

export const VerticalPackVerificationRubric = S.Struct({
  rubricRef: S.String,
  reviewGateRef: S.String,
  criteria: S.Array(VerticalPackRubricCriterion),
})
export type VerticalPackVerificationRubric =
  typeof VerticalPackVerificationRubric.Type

export const VerticalPackStarterWorkflow = S.Struct({
  workflowRef: S.String,
  name: S.String,
  deliverableKind: S.String,
  startingStage: VerticalPackStageKey,
  requiredStageKeys: S.Array(VerticalPackStageKey),
})
export type VerticalPackStarterWorkflow =
  typeof VerticalPackStarterWorkflow.Type

export const VerticalPackComplianceProfile = S.Struct({
  profileRef: S.String,
  regulatedDataHandlingRef: S.String,
  requiresHumanReview: S.Boolean,
  professionalReviewRequired: S.Boolean,
  consentChannelRefs: S.Array(S.String),
  provenanceRequirementRefs: S.Array(S.String),
  noScrapedOutreach: S.Boolean,
  advertisingRuleConstraintRefs: S.Array(S.String),
  outboundActionGateRefs: S.Array(S.String),
  prohibitedActionRefs: S.Array(S.String),
})
export type VerticalPackComplianceProfile =
  typeof VerticalPackComplianceProfile.Type

export const VerticalPackOutboundActionKind = S.Literals([
  'send',
  'publish',
  'file',
  'spend',
])
export type VerticalPackOutboundActionKind =
  typeof VerticalPackOutboundActionKind.Type

export class VerticalPackOutboundComplianceCheckInput extends S.Class<VerticalPackOutboundComplianceCheckInput>(
  'VerticalPackOutboundComplianceCheckInput',
)({
  actionRef: S.String,
  advertisingRuleConstraintRefs: S.Array(S.String),
  consentChannelRefs: S.Array(S.String),
  outboundActionKind: VerticalPackOutboundActionKind,
  proposedActionRefs: S.Array(S.String),
  provenanceReceiptRefs: S.Array(S.String),
  regulatedDataHandlingRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  verticalPackId: S.String,
}) {}

export class VerticalPackOutboundComplianceDecision extends S.Class<VerticalPackOutboundComplianceDecision>(
  'VerticalPackOutboundComplianceDecision',
)({
  actionRef: S.String,
  advertisingRuleConstraintRefs: S.Array(S.String),
  blockedOutboundAction: S.Boolean,
  blockerRefs: S.Array(S.String),
  consentChannelRefs: S.Array(S.String),
  outboundActionAllowed: S.Boolean,
  outboundActionKind: VerticalPackOutboundActionKind,
  profileRef: S.String,
  prohibitedActionRefs: S.Array(S.String),
  provenanceReceiptRefs: S.Array(S.String),
  regulatedDataHandlingRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  verticalPackId: S.String,
}) {}

export const VerticalPackScreenPolicy = S.Struct({
  policyRef: S.String,
  sharedSurfaceRefs: S.Array(S.String),
  bespokeScreenRefs: S.Array(S.String),
  reviewFailureRef: S.String,
})
export type VerticalPackScreenPolicy = typeof VerticalPackScreenPolicy.Type

export const VerticalPackEthicalMarketingPolicy = S.Struct({
  policyRef: S.String,
  noFabricatedTestimonials: S.Boolean,
  noFabricatedCredentials: S.Boolean,
  noFakeUrgency: S.Boolean,
  clarityOverHype: S.Boolean,
  humanInLoopOnSensitiveSends: S.Boolean,
  ruleRefs: S.Array(S.String),
})
export type VerticalPackEthicalMarketingPolicy =
  typeof VerticalPackEthicalMarketingPolicy.Type

export const VerticalPack = S.Struct({
  id: S.String,
  vertical: S.String,
  description: S.String,
  contextPack: BlueprintContextPack,
  stageTemplates: S.Array(VerticalPackStageTemplate),
  verificationRubric: VerticalPackVerificationRubric,
  starterWorkflows: S.Array(VerticalPackStarterWorkflow),
  complianceProfile: VerticalPackComplianceProfile,
  ethicalMarketingPolicy: VerticalPackEthicalMarketingPolicy,
  screenPolicy: VerticalPackScreenPolicy,
})
export type VerticalPack = typeof VerticalPack.Type

const stageOrder: ReadonlyArray<VerticalPackStageKey> = [
  'signal',
  'triage',
  'codegen',
  'validate',
  'release',
  'document',
  'monitor',
  'deploy',
]

export const verticalPackStageOrder = stageOrder

const stageTemplates = (
  vertical: string,
  names: Readonly<Record<VerticalPackStageKey, string>>,
): ReadonlyArray<VerticalPackStageTemplate> =>
  stageOrder.map(stageKey => ({
    displayName: names[stageKey],
    inputContractRef: `vertical.${vertical}.stage.${stageKey}.input`,
    outputContractRef: `vertical.${vertical}.stage.${stageKey}.output`,
    requiredEvidenceRefs: [
      `evidence.${stageKey}.source_refs`,
      `evidence.${stageKey}.decision_or_receipt`,
    ],
    stageKey,
  }))

const baseSource = (
  sourceRef: string,
  sourceKind: BlueprintSourceAuthority['sourceKind'],
  dataClassification: BlueprintSourceAuthority['dataClassification'],
  consentState: BlueprintSourceAuthority['consentState'],
  includedInContext: boolean,
  publicSafe: boolean,
): BlueprintSourceAuthority => ({
  classificationCaveatRef: `classification.${sourceRef}`,
  confidence: includedInContext ? 'high' : 'medium',
  consentState,
  customerSafe: consentState !== 'internal_only',
  dataClassification,
  excludedReasonRef: includedInContext ? null : `excluded.${sourceRef}`,
  freshness: includedInContext ? 'current' : 'recent',
  includedInContext,
  publicSafe,
  publicSummaryRef: publicSafe ? `summary.${sourceRef}` : null,
  sourceKind,
  sourceRef,
  trustTier: includedInContext ? 'verified' : 'reviewed',
})

const contextPack = (
  vertical: string,
  dataClassification: BlueprintSourceAuthority['dataClassification'],
  sources: ReadonlyArray<BlueprintSourceAuthority>,
): BlueprintContextPackType => ({
  createdAt: '2026-07-02T00:00:00.000Z',
  customerSafeProjection: true,
  dataClassification,
  excludedContextRefs: sources
    .filter(source => !source.includedInContext)
    .map(source => source.sourceRef),
  id: `context_pack.${vertical}_template`,
  includedContextRefs: sources
    .filter(source => source.includedInContext)
    .map(source => source.sourceRef),
  publicSafeProjection: sources.some(
    source => source.includedInContext && source.publicSafe,
  ),
  sourceAuthorities: [...sources],
  trustTier: 'reviewed',
  updatedAt: '2026-07-02T00:00:00.000Z',
})

const ethicalMarketingPolicy = (
  vertical: string,
): VerticalPackEthicalMarketingPolicy => ({
  clarityOverHype: true,
  humanInLoopOnSensitiveSends: true,
  noFabricatedCredentials: true,
  noFabricatedTestimonials: true,
  noFakeUrgency: true,
  policyRef: `policy.ethical_marketing.${vertical}`,
  ruleRefs: [
    'rule.no_fabricated_testimonials_or_reviews',
    'rule.no_invented_credentials_or_affiliations',
    'rule.no_fake_scarcity_or_countdowns',
    'rule.describe_accurately_avoid_superlatives',
    'rule.human_approval_before_sensitive_send',
  ],
})

const screenPolicy = (vertical: string): VerticalPackScreenPolicy => ({
  bespokeScreenRefs: [],
  policyRef: `policy.vertical_screens.${vertical}`,
  reviewFailureRef: 'review.failure.vertical_requires_bespoke_screen',
  sharedSurfaceRefs: [
    'surface.omni_workroom',
    'surface.approval_ladder',
    'surface.evidence_bundle',
    'surface.customer_handoff',
  ],
})

const complianceProfile = (
  vertical: string,
  regulatedDataHandlingRef: string,
  professionalReviewRequired: boolean,
  prohibitedActionRefs: ReadonlyArray<string>,
  advertisingRuleConstraintRefs: ReadonlyArray<string>,
): VerticalPackComplianceProfile => ({
  advertisingRuleConstraintRefs: [...advertisingRuleConstraintRefs],
  consentChannelRefs: [
    'consent.customer_provided_sources',
    'consent.outbound_action_approval',
  ],
  noScrapedOutreach: true,
  outboundActionGateRefs: [
    'gate.human_approval_before_send_publish_file_or_spend',
    'gate.provenance_before_customer_delivery',
    'gate.vertical_compliance_profile_before_outbound_action',
  ],
  professionalReviewRequired,
  prohibitedActionRefs: [...prohibitedActionRefs],
  profileRef: `compliance_profile.${vertical}`,
  provenanceRequirementRefs: [
    'provenance.customer_or_public_source_receipt',
    'provenance.deliverable_source_map_receipt',
  ],
  regulatedDataHandlingRef,
  requiresHumanReview: true,
})

const uniqueSorted = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

const outboundActionUsesAdvertisingRules = (
  actionKind: VerticalPackOutboundActionKind,
): boolean => actionKind === 'send' || actionKind === 'publish'

const hasScrapedOutreachSource = (refs: ReadonlyArray<string>): boolean =>
  refs.some(ref => /(^|[_.:/-])scraped([_.:/-]|$)|scraped_outreach|raw_scrape/i.test(ref))

export const decideVerticalPackOutboundCompliance = (
  profile: VerticalPackComplianceProfile,
  input: VerticalPackOutboundComplianceCheckInput,
): VerticalPackOutboundComplianceDecision => {
  const missingConsentRefs = profile.consentChannelRefs.filter(
    ref => !input.consentChannelRefs.includes(ref),
  )
  const missingProvenanceRefs = profile.provenanceRequirementRefs.filter(
    ref => !input.provenanceReceiptRefs.includes(ref),
  )
  const missingAdvertisingRuleRefs = outboundActionUsesAdvertisingRules(
    input.outboundActionKind,
  )
    ? profile.advertisingRuleConstraintRefs.filter(
        ref => !input.advertisingRuleConstraintRefs.includes(ref),
      )
    : []
  const prohibitedActionRefs = input.proposedActionRefs.filter(ref =>
    profile.prohibitedActionRefs.includes(ref),
  )
  const regulatedDataHandlingRecorded =
    input.regulatedDataHandlingRefs.includes(profile.regulatedDataHandlingRef)
  const scrapedOutreachBlocked =
    profile.noScrapedOutreach && hasScrapedOutreachSource(input.sourceRefs)

  const blockerRefs = uniqueSorted([
    ...missingConsentRefs.map(ref => `blocker.vertical_compliance.missing.${ref}`),
    ...missingProvenanceRefs.map(ref => `blocker.vertical_compliance.missing.${ref}`),
    ...(!regulatedDataHandlingRecorded
      ? [
          `blocker.vertical_compliance.missing.${profile.regulatedDataHandlingRef}`,
        ]
      : []),
    ...missingAdvertisingRuleRefs.map(
      ref => `blocker.vertical_compliance.missing.${ref}`,
    ),
    ...prohibitedActionRefs.map(ref => `blocker.vertical_compliance.${ref}`),
    ...(scrapedOutreachBlocked
      ? ['blocker.vertical_compliance.no_scraped_outreach']
      : []),
  ])

  return new VerticalPackOutboundComplianceDecision({
    actionRef: input.actionRef,
    advertisingRuleConstraintRefs: uniqueSorted(
      input.advertisingRuleConstraintRefs,
    ),
    blockedOutboundAction: blockerRefs.length > 0,
    blockerRefs,
    consentChannelRefs: uniqueSorted(input.consentChannelRefs),
    outboundActionAllowed: blockerRefs.length === 0,
    outboundActionKind: input.outboundActionKind,
    profileRef: profile.profileRef,
    prohibitedActionRefs: uniqueSorted(prohibitedActionRefs),
    provenanceReceiptRefs: uniqueSorted(input.provenanceReceiptRefs),
    regulatedDataHandlingRefs: uniqueSorted(input.regulatedDataHandlingRefs),
    sourceRefs: uniqueSorted(input.sourceRefs),
    verticalPackId: input.verticalPackId,
  })
}

const verificationRubric = (
  vertical: string,
  extraCriterion: VerticalPackRubricCriterion,
): VerticalPackVerificationRubric => ({
  reviewGateRef: `review_gate.${vertical}.fulfillment`,
  rubricRef: `rubric.vertical_pack.${vertical}.v1`,
  criteria: [
    {
      criterionRef: `criterion.${vertical}.grounded_sources`,
      description: 'Every deliverable claim is grounded in bound source refs.',
      requiredEvidenceRefs: [
        'evidence.source_map',
        'evidence.provenance_receipt',
      ],
    },
    {
      criterionRef: `criterion.${vertical}.approval_recorded`,
      description:
        'External send, publish, filing, or spend waits for approval.',
      requiredEvidenceRefs: [
        'evidence.approval_decision',
        'evidence.action_receipt',
      ],
    },
    extraCriterion,
  ],
})

const starterWorkflow = (
  vertical: string,
  workflow: string,
  name: string,
  deliverableKind: string,
): VerticalPackStarterWorkflow => ({
  deliverableKind,
  name,
  requiredStageKeys: [...stageOrder],
  startingStage: 'signal',
  workflowRef: `workflow.${vertical}.${workflow}`,
})

export const legalVerticalPack: VerticalPack = {
  id: 'vertical_pack.legal',
  vertical: 'legal',
  description:
    'Legal workflow-assistance vertical for intake, document preparation, ' +
    'source-cited packets, and secure delivery under qualified human review.',
  contextPack: contextPack('legal', 'private', [
    baseSource(
      'intake.legal_matter',
      'order',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'customer_asset.legal_templates',
      'customer_asset',
      'private',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'generated_summary.matter_public_safe',
      'generated_summary',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'email.raw_legal_correspondence',
      'email',
      'private',
      'internal_only',
      false,
      false,
    ),
  ]),
  stageTemplates: stageTemplates('legal', {
    signal: 'Matter Signal',
    triage: 'Intake Triage',
    codegen: 'Draft Assembly',
    validate: 'Legal Review Gate',
    release: 'Client-Ready Packet',
    document: 'Matter Handoff',
    monitor: 'Matter Follow-Up',
    deploy: 'Secure Delivery',
  }),
  verificationRubric: verificationRubric('legal', {
    criterionRef: 'criterion.legal.qualified_review',
    description:
      'Jurisdiction-sensitive, rights-impacting, or filing-adjacent outputs are reviewed by a qualified human.',
    requiredEvidenceRefs: [
      'evidence.professional_review_decision',
      'evidence.jurisdiction_caveat',
    ],
  }),
  starterWorkflows: [
    starterWorkflow(
      'legal',
      'document_packet',
      'Reviewed document packet',
      'document_packet',
    ),
    starterWorkflow(
      'legal',
      'intake_summary',
      'Matter intake summary',
      'intake_summary',
    ),
  ],
  complianceProfile: complianceProfile(
    'legal',
    'regulated_data.legal_confidential',
    true,
    [
      'prohibited.legal_advice_without_reviewer',
      'prohibited.unapproved_filing_or_client_send',
    ],
    [
      'advertising_rule.legal.no_outcome_guarantees',
      'advertising_rule.legal.no_attorney_client_relationship_claim_without_review',
    ],
  ),
  ethicalMarketingPolicy: ethicalMarketingPolicy('legal'),
  screenPolicy: screenPolicy('legal'),
}

export const healthVerticalPack: VerticalPack = {
  id: 'vertical_pack.health',
  vertical: 'health',
  description:
    'Health intake and coordination vertical for sensitive-help workflows, ' +
    'PHI-redacted summaries, coach matching, and human-reviewed outbound action.',
  contextPack: contextPack('health', 'private', [
    baseSource(
      'intake.health_request',
      'order',
      'private',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'generated_summary.health_redacted',
      'generated_summary',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'artifact.care_resource_directory',
      'artifact',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'email.raw_health_correspondence',
      'email',
      'private',
      'internal_only',
      false,
      false,
    ),
  ]),
  stageTemplates: stageTemplates('health', {
    signal: 'Help Signal',
    triage: 'Care Triage',
    codegen: 'Support Plan Draft',
    validate: 'Clinical Safety Review',
    release: 'Reviewer-Ready Plan',
    document: 'Care Handoff',
    monitor: 'Support Follow-Up',
    deploy: 'Approved Outreach',
  }),
  verificationRubric: verificationRubric('health', {
    criterionRef: 'criterion.health.redaction_before_inference',
    description:
      'PHI or sensitive health context is redacted before external inference and checked before outreach.',
    requiredEvidenceRefs: [
      'evidence.redaction_receipt',
      'evidence.human_review_decision',
    ],
  }),
  starterWorkflows: [
    starterWorkflow(
      'health',
      'triage_summary',
      'Redacted triage summary',
      'triage_summary',
    ),
    starterWorkflow(
      'health',
      'coach_match',
      'Human-reviewed coach match',
      'coach_match',
    ),
  ],
  complianceProfile: complianceProfile(
    'health',
    'regulated_data.phi_redaction_required',
    true,
    [
      'prohibited.external_inference_before_redaction',
      'prohibited.medical_advice_without_reviewer',
    ],
    [
      'advertising_rule.health.no_diagnosis_or_treatment_claim',
      'advertising_rule.health.no_sensitive_condition_targeting',
    ],
  ),
  ethicalMarketingPolicy: ethicalMarketingPolicy('health'),
  screenPolicy: screenPolicy('health'),
}

export const agencyVerticalPack: VerticalPack = {
  id: 'vertical_pack.agency',
  vertical: 'agency',
  description:
    'Marketing-agency and services-operator vertical for client briefs, brand ' +
    'deliverables, campaign plans, approval receipts, and channel launches.',
  contextPack: contextPack('agency', 'customer', [
    baseSource(
      'order.services_engagement',
      'order',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'customer_asset.brand_kit',
      'customer_asset',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'exa_brief.market_positioning',
      'exa_brief',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'repo.public_marketing_site',
      'repo',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'email.raw_customer_inbox',
      'email',
      'private',
      'internal_only',
      false,
      false,
    ),
  ]),
  stageTemplates: stageTemplates('agency', {
    signal: 'Client Signal',
    triage: 'Creative Triage',
    codegen: 'Creative Production',
    validate: 'Brand QA',
    release: 'Client Approval',
    document: 'Account Handoff',
    monitor: 'Campaign Watch',
    deploy: 'Channel Launch',
  }),
  verificationRubric: verificationRubric('agency', {
    criterionRef: 'criterion.agency.brand_and_claim_fit',
    description:
      'Creative output matches approved brand sources, avoids fabricated claims, and passes channel checks.',
    requiredEvidenceRefs: [
      'evidence.brand_source_map',
      'evidence.link_or_channel_check',
    ],
  }),
  starterWorkflows: [
    starterWorkflow(
      'agency',
      'landing_page',
      'Client landing page',
      'landing_page',
    ),
    starterWorkflow(
      'agency',
      'email_sequence',
      'Approved email sequence',
      'email_sequence',
    ),
  ],
  complianceProfile: complianceProfile(
    'agency',
    'regulated_data.customer_marketing',
    false,
    [
      'prohibited.fabricated_case_study_or_testimonial',
      'prohibited.unapproved_customer_channel_send',
    ],
    [
      'advertising_rule.agency.no_fabricated_results',
      'advertising_rule.agency.channel_terms_checked',
    ],
  ),
  ethicalMarketingPolicy: ethicalMarketingPolicy('agency'),
  screenPolicy: screenPolicy('agency'),
}

export const ecommerceVerticalPack: VerticalPack = {
  id: 'vertical_pack.ecommerce',
  vertical: 'ecommerce',
  description:
    'E-commerce vertical for inventory-aware offers, campaign artifacts, stock ' +
    'checks, merchandising approval, channel publishing, and conversion watch.',
  contextPack: contextPack('ecommerce', 'customer', [
    baseSource(
      'customer_asset.product_catalog',
      'customer_asset',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'artifact.inventory_snapshot',
      'artifact',
      'customer',
      'customer_provided',
      true,
      false,
    ),
    baseSource(
      'repo.public_storefront',
      'repo',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'generated_summary.commerce_public_safe',
      'generated_summary',
      'public',
      'public',
      true,
      true,
    ),
    baseSource(
      'email.raw_buyer_messages',
      'email',
      'private',
      'internal_only',
      false,
      false,
    ),
  ]),
  stageTemplates: stageTemplates('ecommerce', {
    signal: 'Demand Signal',
    triage: 'Offer Triage',
    codegen: 'Campaign Build',
    validate: 'Commerce QA',
    release: 'Merchandising Release',
    document: 'Merchant Handoff',
    monitor: 'Conversion Watch',
    deploy: 'Channel Publish',
  }),
  verificationRubric: verificationRubric('ecommerce', {
    criterionRef: 'criterion.ecommerce.inventory_and_offer_math',
    description:
      'Campaigns cite live-enough inventory, price, margin, link, and spend-cap evidence before publishing.',
    requiredEvidenceRefs: [
      'evidence.inventory_snapshot',
      'evidence.offer_math_check',
    ],
  }),
  starterWorkflows: [
    starterWorkflow(
      'ecommerce',
      'inventory_campaign',
      'Inventory-aware campaign',
      'campaign',
    ),
    starterWorkflow(
      'ecommerce',
      'storefront_offer',
      'Storefront offer update',
      'storefront_update',
    ),
  ],
  complianceProfile: complianceProfile(
    'ecommerce',
    'regulated_data.commerce_customer_sources',
    false,
    [
      'prohibited.out_of_stock_or_unavailable_offer',
      'prohibited.unapproved_ad_spend_or_channel_publish',
    ],
    [
      'advertising_rule.ecommerce.inventory_claims_match_sources',
      'advertising_rule.ecommerce.price_and_discount_claims_match_sources',
    ],
  ),
  ethicalMarketingPolicy: ethicalMarketingPolicy('ecommerce'),
  screenPolicy: screenPolicy('ecommerce'),
}

// Backward-compatible name for earlier services-business callers. The actual
// pack is the agency/services config pack; callers should prefer agencyVerticalPack.
export const servicesBusinessVerticalPack = agencyVerticalPack

export const verticalPackRegistry: Readonly<Record<string, VerticalPack>> = {
  [agencyVerticalPack.id]: agencyVerticalPack,
  [ecommerceVerticalPack.id]: ecommerceVerticalPack,
  [healthVerticalPack.id]: healthVerticalPack,
  [legalVerticalPack.id]: legalVerticalPack,
  'vertical_pack.services_business': servicesBusinessVerticalPack,
}

export const getVerticalPack = (id: string): VerticalPack | undefined =>
  verticalPackRegistry[id]

export type { BlueprintContextPackType }
