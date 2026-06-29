import { Schema as S } from 'effect'

import type {
  BlueprintProgramEvidenceRequirement,
  BlueprintProgramReceiptRequirement,
  BlueprintProgramToolScope,
  BlueprintProgramType,
} from './schemas/program'

// ---------------------------------------------------------------------------
// Delivery-pipeline Blueprint programs (WS-B, OpenAgents #4980)
//
// This module declares one typed Blueprint Program per delivery-pipeline stage:
//
//   brand-story, web-copy, brand-style, offer-suite, lead-magnet,
//   nurture-sequence, sales-sequence
//
// Each stage ships:
//   - a typed OUTPUT schema with structured fields (Effect Schema),
//   - a `BlueprintProgramType` (family, riskClass, status, evidence
//     requirements, receipt requirements, tool scopes, release gates),
//   - evidence + receipt requirements that reference upstream context and the
//     program-run / action-submission boundary.
//
// All programs are evidence-only: `directMutationAllowed` is false and the only
// write-adjacent tool scope is `propose_action` (approval-gated). Live
// publication of the produced copy/assets stays behind Action Submissions, per
// the Blueprint kernel boundary rules (README.md "Program Runs are evidence,
// not write authority").
// ---------------------------------------------------------------------------

export const DELIVERY_PIPELINE_STAGES = S.Literals([
  'brand-story',
  'web-copy',
  'brand-style',
  'offer-suite',
  'lead-magnet',
  'nurture-sequence',
  'sales-sequence',
])
export type DeliveryPipelineStage = typeof DELIVERY_PIPELINE_STAGES.Type

export const DELIVERY_PIPELINE_STAGE_ORDER: ReadonlyArray<DeliveryPipelineStage> =
  [
    'brand-story',
    'web-copy',
    'brand-style',
    'offer-suite',
    'lead-magnet',
    'nurture-sequence',
    'sales-sequence',
  ]

// ---------------------------------------------------------------------------
// Typed output schemas (structured fields) per stage.
// ---------------------------------------------------------------------------

export const DeliveryPipelineBrandStoryOutput = S.Struct({
  brandName: S.String,
  missionStatement: S.String,
  positioningStatement: S.String,
  originStory: S.String,
  coreValues: S.Array(S.String),
  audienceSummary: S.String,
  toneDescriptors: S.Array(S.String),
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineBrandStoryOutput =
  typeof DeliveryPipelineBrandStoryOutput.Type

export const DeliveryPipelineWebCopySection = S.Struct({
  sectionRef: S.String,
  headline: S.String,
  body: S.String,
  callToAction: S.NullOr(S.String),
})
export type DeliveryPipelineWebCopySection =
  typeof DeliveryPipelineWebCopySection.Type

export const DeliveryPipelineWebCopyOutput = S.Struct({
  pageRef: S.String,
  metaTitle: S.String,
  metaDescription: S.String,
  sections: S.Array(DeliveryPipelineWebCopySection),
  primaryCallToAction: S.String,
  brandStoryRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineWebCopyOutput =
  typeof DeliveryPipelineWebCopyOutput.Type

export const DeliveryPipelineBrandColor = S.Struct({
  roleRef: S.String,
  hex: S.String,
})
export type DeliveryPipelineBrandColor =
  typeof DeliveryPipelineBrandColor.Type

export const DeliveryPipelineBrandFont = S.Struct({
  roleRef: S.String,
  family: S.String,
})
export type DeliveryPipelineBrandFont =
  typeof DeliveryPipelineBrandFont.Type

export const DeliveryPipelineBrandStyleOutput = S.Struct({
  palette: S.Array(DeliveryPipelineBrandColor),
  typography: S.Array(DeliveryPipelineBrandFont),
  logoUsageNotes: S.String,
  voiceGuidelines: S.String,
  imageryGuidelines: S.String,
  brandStoryRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineBrandStyleOutput =
  typeof DeliveryPipelineBrandStyleOutput.Type

export const DeliveryPipelineOffer = S.Struct({
  offerRef: S.String,
  name: S.String,
  summary: S.String,
  priceLabel: S.String,
  deliverables: S.Array(S.String),
  positioningTier: S.String,
})
export type DeliveryPipelineOffer = typeof DeliveryPipelineOffer.Type

export const DeliveryPipelineOfferSuiteOutput = S.Struct({
  offers: S.Array(DeliveryPipelineOffer),
  anchorOfferRef: S.String,
  valueLadderSummary: S.String,
  brandStoryRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineOfferSuiteOutput =
  typeof DeliveryPipelineOfferSuiteOutput.Type

export const DeliveryPipelineLeadMagnetOutput = S.Struct({
  title: S.String,
  format: S.String,
  promise: S.String,
  outline: S.Array(S.String),
  optInHeadline: S.String,
  optInBody: S.String,
  targetOfferRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineLeadMagnetOutput =
  typeof DeliveryPipelineLeadMagnetOutput.Type

export const DeliveryPipelineEmailStep = S.Struct({
  stepRef: S.String,
  sendOffsetHours: S.Number,
  subject: S.String,
  previewText: S.String,
  body: S.String,
  callToAction: S.NullOr(S.String),
})
export type DeliveryPipelineEmailStep =
  typeof DeliveryPipelineEmailStep.Type

export const DeliveryPipelineNurtureSequenceOutput = S.Struct({
  sequenceRef: S.String,
  goalSummary: S.String,
  emails: S.Array(DeliveryPipelineEmailStep),
  leadMagnetRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineNurtureSequenceOutput =
  typeof DeliveryPipelineNurtureSequenceOutput.Type

export const DeliveryPipelineSalesSequenceOutput = S.Struct({
  sequenceRef: S.String,
  goalSummary: S.String,
  emails: S.Array(DeliveryPipelineEmailStep),
  offerSuiteRef: S.String,
  nurtureSequenceRef: S.String,
  sourceContextRefs: S.Array(S.String),
})
export type DeliveryPipelineSalesSequenceOutput =
  typeof DeliveryPipelineSalesSequenceOutput.Type

// ---------------------------------------------------------------------------
// Shared declaration helpers.
// ---------------------------------------------------------------------------

const ID_PREFIX = 'program_type.delivery_pipeline'

const stageSlug = (stage: DeliveryPipelineStage): string =>
  stage.replace(/-/g, '_')

const contextPackEvidence = (
  stage: DeliveryPipelineStage,
): BlueprintProgramEvidenceRequirement => ({
  descriptionRef: `evidence.delivery_pipeline.${stageSlug(
    stage,
  )}.context_pack_required`,
  kind: 'context_pack_ref',
  minimumCount: 1,
  required: true,
})

const upstreamArtifactEvidence = (
  stage: DeliveryPipelineStage,
): BlueprintProgramEvidenceRequirement => ({
  descriptionRef: `evidence.delivery_pipeline.${stageSlug(
    stage,
  )}.upstream_artifact_required`,
  kind: 'artifact_ref',
  minimumCount: 1,
  required: true,
})

const reviewEvidence = (
  stage: DeliveryPipelineStage,
): BlueprintProgramEvidenceRequirement => ({
  descriptionRef: `evidence.delivery_pipeline.${stageSlug(
    stage,
  )}.human_review_recommended`,
  kind: 'human_review_ref',
  minimumCount: 0,
  required: false,
})

const programRunReceipt: BlueprintProgramReceiptRequirement = {
  kind: 'program_run',
  receiptRef: 'receipt.program_run',
  required: true,
}

const actionSubmissionReceipt: BlueprintProgramReceiptRequirement = {
  kind: 'action_submission',
  receiptRef: 'receipt.action_submission',
  required: false,
}

const reviewReceipt: BlueprintProgramReceiptRequirement = {
  kind: 'review',
  receiptRef: 'receipt.review',
  required: false,
}

const baseToolScopes = (
  stage: DeliveryPipelineStage,
): ReadonlyArray<BlueprintProgramToolScope> => [
  {
    access: 'read',
    allowedSurfaces: ['agent_api', 'omni_workroom', 'operator_dashboard'],
    requiresApproval: false,
    toolRef: `tool.delivery_pipeline.${stageSlug(stage)}.context_pack.read`,
  },
  {
    access: 'evidence',
    allowedSurfaces: ['agent_api', 'omni_workroom'],
    requiresApproval: false,
    toolRef: `tool.delivery_pipeline.${stageSlug(stage)}.artifact.write_evidence`,
  },
  {
    access: 'propose_action',
    allowedSurfaces: ['operator_dashboard', 'customer_dashboard'],
    requiresApproval: true,
    toolRef: `tool.delivery_pipeline.${stageSlug(stage)}.publish.propose`,
  },
]

export interface DeliveryPipelineProgram {
  readonly stage: DeliveryPipelineStage
  readonly outputSchema: S.Schema<unknown>
  readonly outputSchemaRef: string
  readonly programType: BlueprintProgramType
}

const declareProgram = (params: {
  readonly stage: DeliveryPipelineStage
  readonly outputSchema: S.Schema<unknown>
  readonly evidenceRequirements: ReadonlyArray<BlueprintProgramEvidenceRequirement>
}): DeliveryPipelineProgram => {
  const slug = stageSlug(params.stage)

  return {
    stage: params.stage,
    outputSchema: params.outputSchema,
    outputSchemaRef: `schema.delivery_pipeline.${slug}.output.v1`,
    programType: {
      allowedStrategyRefs: [
        `strategy.delivery_pipeline.${slug}.evidence_only`,
      ],
      directMutationAllowed: false,
      evidenceRequirements: [...params.evidenceRequirements],
      family: 'artifact_review',
      id: `${ID_PREFIX}.${slug}`,
      instructionRefs: [`instruction.delivery_pipeline.${slug}.v1`],
      instructionsVersionRef: `instruction.delivery_pipeline.${slug}.v1`,
      purposeRef: `purpose.delivery_pipeline.${slug}`,
      receiptRequirements: [
        programRunReceipt,
        actionSubmissionReceipt,
        reviewReceipt,
      ],
      releaseGates: [
        {
          evidenceRefs: [`fixture.delivery_pipeline.${slug}.v1`],
          gateKind: 'operator_review',
          gateRef: `release_gate.delivery_pipeline.${slug}.v1`,
          required: true,
        },
      ],
      riskClass: 'medium',
      status: 'draft',
      toolScopes: [...baseToolScopes(params.stage)],
    },
  }
}

// ---------------------------------------------------------------------------
// One Blueprint program per delivery-pipeline stage.
//
// Evidence requirements encode the stage dependency chain: every stage after
// brand-story requires at least one upstream artifact ref in addition to a
// context pack.
// ---------------------------------------------------------------------------

export const DELIVERY_PIPELINE_BRAND_STORY_PROGRAM = declareProgram({
  stage: 'brand-story',
  outputSchema: DeliveryPipelineBrandStoryOutput,
  evidenceRequirements: [
    contextPackEvidence('brand-story'),
    reviewEvidence('brand-story'),
  ],
})

export const DELIVERY_PIPELINE_WEB_COPY_PROGRAM = declareProgram({
  stage: 'web-copy',
  outputSchema: DeliveryPipelineWebCopyOutput,
  evidenceRequirements: [
    contextPackEvidence('web-copy'),
    upstreamArtifactEvidence('web-copy'),
    reviewEvidence('web-copy'),
  ],
})

export const DELIVERY_PIPELINE_BRAND_STYLE_PROGRAM = declareProgram({
  stage: 'brand-style',
  outputSchema: DeliveryPipelineBrandStyleOutput,
  evidenceRequirements: [
    contextPackEvidence('brand-style'),
    upstreamArtifactEvidence('brand-style'),
    reviewEvidence('brand-style'),
  ],
})

export const DELIVERY_PIPELINE_OFFER_SUITE_PROGRAM = declareProgram({
  stage: 'offer-suite',
  outputSchema: DeliveryPipelineOfferSuiteOutput,
  evidenceRequirements: [
    contextPackEvidence('offer-suite'),
    upstreamArtifactEvidence('offer-suite'),
    reviewEvidence('offer-suite'),
  ],
})

export const DELIVERY_PIPELINE_LEAD_MAGNET_PROGRAM = declareProgram({
  stage: 'lead-magnet',
  outputSchema: DeliveryPipelineLeadMagnetOutput,
  evidenceRequirements: [
    contextPackEvidence('lead-magnet'),
    upstreamArtifactEvidence('lead-magnet'),
    reviewEvidence('lead-magnet'),
  ],
})

export const DELIVERY_PIPELINE_NURTURE_SEQUENCE_PROGRAM = declareProgram({
  stage: 'nurture-sequence',
  outputSchema: DeliveryPipelineNurtureSequenceOutput,
  evidenceRequirements: [
    contextPackEvidence('nurture-sequence'),
    upstreamArtifactEvidence('nurture-sequence'),
    reviewEvidence('nurture-sequence'),
  ],
})

export const DELIVERY_PIPELINE_SALES_SEQUENCE_PROGRAM = declareProgram({
  stage: 'sales-sequence',
  outputSchema: DeliveryPipelineSalesSequenceOutput,
  evidenceRequirements: [
    contextPackEvidence('sales-sequence'),
    upstreamArtifactEvidence('sales-sequence'),
    reviewEvidence('sales-sequence'),
  ],
})

// ---------------------------------------------------------------------------
// Registry + lookup.
// ---------------------------------------------------------------------------

export const DELIVERY_PIPELINE_PROGRAMS: ReadonlyArray<DeliveryPipelineProgram> =
  [
    DELIVERY_PIPELINE_BRAND_STORY_PROGRAM,
    DELIVERY_PIPELINE_WEB_COPY_PROGRAM,
    DELIVERY_PIPELINE_BRAND_STYLE_PROGRAM,
    DELIVERY_PIPELINE_OFFER_SUITE_PROGRAM,
    DELIVERY_PIPELINE_LEAD_MAGNET_PROGRAM,
    DELIVERY_PIPELINE_NURTURE_SEQUENCE_PROGRAM,
    DELIVERY_PIPELINE_SALES_SEQUENCE_PROGRAM,
  ]

export const DELIVERY_PIPELINE_PROGRAM_TYPES: ReadonlyArray<BlueprintProgramType> =
  DELIVERY_PIPELINE_PROGRAMS.map(program => program.programType)

const DELIVERY_PIPELINE_PROGRAM_BY_STAGE: ReadonlyMap<
  DeliveryPipelineStage,
  DeliveryPipelineProgram
> = new Map(
  DELIVERY_PIPELINE_PROGRAMS.map(program => [program.stage, program] as const),
)

export const deliveryPipelineProgramForStage = (
  stage: DeliveryPipelineStage,
): DeliveryPipelineProgram | undefined =>
  DELIVERY_PIPELINE_PROGRAM_BY_STAGE.get(stage)

export const deliveryPipelineProgramTypeId = (
  stage: DeliveryPipelineStage,
): string => `${ID_PREFIX}.${stageSlug(stage)}`

// ---------------------------------------------------------------------------
// COORDINATOR WIRING:
//
// Add the following export block to
// `workers/api/src/blueprint/index.ts` (do NOT edit index.ts in this lane):
//
//   export {
//     DELIVERY_PIPELINE_BRAND_STORY_PROGRAM,
//     DELIVERY_PIPELINE_BRAND_STYLE_PROGRAM,
//     DELIVERY_PIPELINE_LEAD_MAGNET_PROGRAM,
//     DELIVERY_PIPELINE_NURTURE_SEQUENCE_PROGRAM,
//     DELIVERY_PIPELINE_OFFER_SUITE_PROGRAM,
//     DELIVERY_PIPELINE_PROGRAM_TYPES,
//     DELIVERY_PIPELINE_PROGRAMS,
//     DELIVERY_PIPELINE_SALES_SEQUENCE_PROGRAM,
//     DELIVERY_PIPELINE_STAGE_ORDER,
//     DELIVERY_PIPELINE_STAGES,
//     DELIVERY_PIPELINE_WEB_COPY_PROGRAM,
//     DeliveryPipelineBrandStoryOutput,
//     type DeliveryPipelineBrandStoryOutput as DeliveryPipelineBrandStoryOutputType,
//     DeliveryPipelineBrandStyleOutput,
//     type DeliveryPipelineBrandStyleOutput as DeliveryPipelineBrandStyleOutputType,
//     DeliveryPipelineLeadMagnetOutput,
//     type DeliveryPipelineLeadMagnetOutput as DeliveryPipelineLeadMagnetOutputType,
//     DeliveryPipelineNurtureSequenceOutput,
//     type DeliveryPipelineNurtureSequenceOutput as DeliveryPipelineNurtureSequenceOutputType,
//     DeliveryPipelineOfferSuiteOutput,
//     type DeliveryPipelineOfferSuiteOutput as DeliveryPipelineOfferSuiteOutputType,
//     type DeliveryPipelineProgram,
//     DeliveryPipelineSalesSequenceOutput,
//     type DeliveryPipelineSalesSequenceOutput as DeliveryPipelineSalesSequenceOutputType,
//     type DeliveryPipelineStage,
//     DeliveryPipelineWebCopyOutput,
//     type DeliveryPipelineWebCopyOutput as DeliveryPipelineWebCopyOutputType,
//     deliveryPipelineProgramForStage,
//     deliveryPipelineProgramTypeId,
//   } from './delivery-pipeline-programs'
//
// To register these in the Blueprint program registry, the coordinator should
// fold `DELIVERY_PIPELINE_PROGRAM_TYPES` into the `programTypes` array passed to
// `blueprintProgramRegistryProjection({ ... })` (see
// `fixtures/program-registry.ts`, alongside `AUTOPILOT_CONTINUATION_PROGRAM_TYPES`),
// minting one `BlueprintProgramSignature` per stage whose `programTypeId` equals
// `deliveryPipelineProgramTypeId(stage)` and whose `outputSchema.schemaRef`
// equals the program's `outputSchemaRef`. These programs are evidence-only
// (`directMutationAllowed: false`, status `draft`); live publication stays
// behind Action Submissions and the per-stage `operator_review` release gate.
// ---------------------------------------------------------------------------
