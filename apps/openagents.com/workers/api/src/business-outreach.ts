import { Schema as S } from 'effect'

import {
  assertBusinessPipelinePublicSafeRef,
  businessPipelineSafeRefPart,
  type BusinessPipelineRuntime,
  type BusinessPipelineStore,
  BusinessPipelineValidationError,
} from './business-pipeline-queue'
import { parseJsonStringArray } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF =
  'business.outreach.report_led.v1'
export const BUSINESS_OUTREACH_DEFAULT_DAILY_MAILBOX_SEND_CAP = 95

export const BusinessOutreachSegmentRef = S.Literals([
  'agent_readiness_ecommerce',
  'agent_readiness_saas',
  'agent_readiness_agency',
  'agent_readiness_marketplace',
  'agent_readiness_bitcoin',
  'model_custody_regulated',
])
export type BusinessOutreachSegmentRef = typeof BusinessOutreachSegmentRef.Type

export const BusinessOutreachSuppressionReason = S.Literals([
  'existing_partner',
  'existing_customer',
  'active_intake',
])
export type BusinessOutreachSuppressionReason =
  typeof BusinessOutreachSuppressionReason.Type

export const BusinessOutreachChannel = S.Literals([
  'apollo_sequence',
  'customer_mailbox',
  'manual',
])
export type BusinessOutreachChannel = typeof BusinessOutreachChannel.Type

export const BusinessOutreachTemplateVersion = S.Struct({
  cta: S.String,
  familyRef: S.Literal('business.outreach.report_led.v1'),
  identificationOptOut: S.String,
  offerSentence: S.String,
  proofPoint: S.String,
  requiredSlots: S.Array(S.String),
  segmentRef: BusinessOutreachSegmentRef,
  skeleton: S.Tuple([
    S.Literal('observed_fact'),
    S.Literal('offer_sentence'),
    S.Literal('registry_true_proof_point'),
    S.Literal('single_cta'),
    S.Literal('identification_opt_out'),
  ]),
  sourceRefs: S.Array(S.String),
  templateVersionRef: S.String,
})
export type BusinessOutreachTemplateVersion =
  typeof BusinessOutreachTemplateVersion.Type

export const BusinessOutreachTemplateApproval = S.Struct({
  approvalReceiptRef: S.String,
  approvedByRef: S.String,
  createdAt: S.String,
  sourceRef: S.String,
  templateVersionRef: S.String,
})
export type BusinessOutreachTemplateApproval =
  typeof BusinessOutreachTemplateApproval.Type

export const BusinessOutreachSuppression = S.Struct({
  createdAt: S.String,
  reason: BusinessOutreachSuppressionReason,
  sourceRef: S.String,
  subjectRef: S.String,
  suppressionRef: S.String,
})
export type BusinessOutreachSuppression =
  typeof BusinessOutreachSuppression.Type

export const BusinessOutreachDraft = S.Struct({
  auditReportRef: S.String,
  bodyText: S.String,
  claimLintRefs: S.Array(S.String),
  createdAt: S.String,
  draftRef: S.String,
  findingRefs: S.Array(S.String),
  pipelineRef: S.String,
  segmentRef: BusinessOutreachSegmentRef,
  sourceRef: S.String,
  state: S.Literal('draft'),
  subjectRef: S.String,
  templateVersionRef: S.String,
})
export type BusinessOutreachDraft = typeof BusinessOutreachDraft.Type

export const BusinessOutreachSend = S.Struct({
  approvalReceiptRef: S.String,
  channel: BusinessOutreachChannel,
  createdAt: S.String,
  draftRef: S.String,
  mailboxRef: S.String,
  pipelineRef: S.String,
  sendReceiptRef: S.String,
  sendRef: S.String,
  sentAt: S.String,
  sourceRef: S.String,
  subjectRef: S.String,
  templateVersionRef: S.String,
})
export type BusinessOutreachSend = typeof BusinessOutreachSend.Type

export const BusinessOutreachRefusalReason = S.Literals([
  'active_intake',
  'claim_lint_failed',
  'daily_mailbox_send_cap_exceeded',
  'draft_not_found',
  'suppressed_subject',
  'template_mismatch',
  'template_not_approved',
  'template_not_found',
])
export type BusinessOutreachRefusalReason =
  typeof BusinessOutreachRefusalReason.Type

export type BusinessOutreachRenderOutcome =
  | Readonly<{ ok: true; draft: BusinessOutreachDraft }>
  | Readonly<{
      ok: false
      reason: BusinessOutreachRefusalReason
      message: string
      claimLintRefs?: ReadonlyArray<string>
      suppression?: BusinessOutreachSuppression
    }>

export type BusinessOutreachSendOutcome =
  | Readonly<{
      ok: true
      pipelineReceiptRefs: ReadonlyArray<string>
      send: BusinessOutreachSend
    }>
  | Readonly<{
      ok: false
      reason: BusinessOutreachRefusalReason
      message: string
    }>

export type BusinessOutreachTemplateApprovalInput = Readonly<{
  approvalReceiptRef: string
  approvedByRef: string
  sourceRef?: string
  templateVersionRef: string
}>

export type BusinessOutreachSuppressionInput = Readonly<{
  reason: BusinessOutreachSuppressionReason
  sourceRef: string
  subjectRef: string
  suppressionRef?: string
}>

export type BusinessOutreachDraftInput = Readonly<{
  auditReportRef: string
  draftRef?: string
  findingRefs?: ReadonlyArray<string>
  observedFact?: string
  sourceRef?: string
  subjectRef: string
  templateVersionRef?: string
}>

export type BusinessOutreachSendInput = Readonly<{
  approvalReceiptRef?: string
  channel?: BusinessOutreachChannel
  dailyMailboxSendCap?: number
  draftRef: string
  mailboxRef: string
  sendRef?: string
  sentAt?: string
  sourceRef: string
}>

export class BusinessOutreachStoreError extends S.TaggedErrorClass<BusinessOutreachStoreError>()(
  'BusinessOutreachStoreError',
  {
    kind: S.Literals(['conflict', 'not_found', 'storage_error', 'validation_error']),
    reason: S.String,
  },
) {}

export type BusinessOutreachRuntime = BusinessPipelineRuntime

export const systemBusinessOutreachRuntime: BusinessOutreachRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

type TemplateApprovalRow = Readonly<{
  approval_receipt_ref: string
  approved_by_ref: string
  created_at: string
  source_ref: string
  template_version_ref: string
}>

type SuppressionRow = Readonly<{
  created_at: string
  reason: BusinessOutreachSuppressionReason
  source_ref: string
  subject_ref: string
  suppression_ref: string
}>

type DraftRow = Readonly<{
  audit_report_ref: string
  body_text: string
  claim_lint_refs_json: string
  created_at: string
  draft_ref: string
  finding_refs_json: string
  pipeline_ref: string
  segment_ref: BusinessOutreachSegmentRef
  source_ref: string
  state: 'draft'
  subject_ref: string
  template_version_ref: string
}>

type SendRow = Readonly<{
  approval_receipt_ref: string
  channel: BusinessOutreachChannel
  created_at: string
  draft_ref: string
  mailbox_ref: string
  pipeline_ref: string
  send_receipt_ref: string
  send_ref: string
  sent_at: string
  source_ref: string
  subject_ref: string
  template_version_ref: string
}>

type CountRow = Readonly<{ count: number }>

const skeleton = [
  'observed_fact',
  'offer_sentence',
  'registry_true_proof_point',
  'single_cta',
  'identification_opt_out',
] as const

export const BUSINESS_OUTREACH_TEMPLATE_VERSIONS: ReadonlyArray<BusinessOutreachTemplateVersion> =
  [
    {
      cta: 'Open to a 15-minute walkthrough this week?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can turn the audit into an Agent-Ready Quick Win with a receipt plan before work starts.',
      proofPoint:
        'Registry-true proof point: BF-9.2 pipeline rows and BF-9.1 commitments keep quoted pipeline separate from revenue.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'agent_readiness_ecommerce',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8265',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#7-pipeline-definition-and-the-25k-math-honest',
      ],
      templateVersionRef:
        'business.outreach.agent_readiness_ecommerce.report_led.v1',
    },
    {
      cta: 'Worth a short walkthrough to compare the report against your docs?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can convert the gaps into an agent-readable docs/API quick win with before-and-after receipts.',
      proofPoint:
        'Registry-true proof point: public promise records stay planned or yellow until receipts support stronger copy.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'agent_readiness_saas',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8265',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#5-target-segments-and-credit-budget',
      ],
      templateVersionRef: 'business.outreach.agent_readiness_saas.report_led.v1',
    },
    {
      cta: 'Should we walk through how this could work for your own client book?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can package the same report-led workflow as an approval-gated leadgen engine for agencies.',
      proofPoint:
        'Registry-true proof point: outbound-assist remains approval-gated and suppression-aware before any send.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'agent_readiness_agency',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8265',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10-the-second-product-autopilot-lead-gen-dogfood--product',
      ],
      templateVersionRef:
        'business.outreach.agent_readiness_agency.report_led.v1',
    },
    {
      cta: 'Open to reviewing the agent-readiness report together?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can turn the visible booking or marketplace gaps into a bounded quick-win scope.',
      proofPoint:
        'Registry-true proof point: every quoted opportunity lands in the BF-9.2 queue with receipts.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'agent_readiness_marketplace',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8265',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#5-target-segments-and-credit-budget',
      ],
      templateVersionRef:
        'business.outreach.agent_readiness_marketplace.report_led.v1',
    },
    {
      cta: 'Worth a short walkthrough of what an agent sees before it reaches your product?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can make the public agent path more legible while keeping payment and custody claims receipt-backed.',
      proofPoint:
        'Registry-true proof point: Lightning/payment claims stay separated from delivery, payout, and custody authority.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'agent_readiness_bitcoin',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8265',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#5-target-segments-and-credit-budget',
      ],
      templateVersionRef:
        'business.outreach.agent_readiness_bitcoin.report_led.v1',
    },
    {
      cta: 'Worth a 30-minute Own Your AI review?',
      familyRef: BUSINESS_OUTREACH_TEMPLATE_FAMILY_REF,
      identificationOptOut:
        'Christopher at OpenAgents. Reply opt out and we will not follow up.',
      offerSentence:
        'We can turn the dossier into a Reactor Assessment: model-policy workshop, custody map, and deployment roadmap.',
      proofPoint:
        'Registry-true proof point: Friedberg/Mistral frames the independence problem; Reactor records stay planned until receipts support stronger copy.',
      requiredSlots: ['auditReportRef', 'findingRefs', 'observedFact', 'pipelineRef'],
      segmentRef: 'model_custody_regulated',
      skeleton,
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8281',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#11-campaign-b-own-your-ai',
        'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md',
      ],
      templateVersionRef:
        'business.outreach.model_custody_regulated.reactor_assessment.v1',
    },
  ].map(template =>
    S.decodeUnknownSync(BusinessOutreachTemplateVersion)(template),
  )

export const BUSINESS_OUTREACH_GATED_CLAIM_DENYLIST: ReadonlyArray<{
  claimRef: string
  description: string
  pattern: RegExp
}> = [
  {
    claimRef: 'claim_lint.self_serve_delivery',
    description: 'self-serve delivery is gated until receipts support it',
    pattern: /\b(self[- ]serve|instant delivery|launch yourself|fully automated delivery)\b/i,
  },
  {
    claimRef: 'claim_lint.pays_you_loop',
    description: 'pays-you loop copy is gated',
    pattern: /\b(pays? you|earn passive|automatic revenue share|revenue-share payout)\b/i,
  },
  {
    claimRef: 'claim_lint.hipaa_sovereign',
    description: 'regulated or sovereignty posture is gated',
    pattern: /\b(HIPAA|sovereign|sovereignty|compliance-ready|US-origin-only)\b/i,
  },
  {
    claimRef: 'claim_lint.published_prices',
    description: 'published prices are owner-gated',
    pattern: /\b(published price|fixed price|price sheet|\$\s?\d)\b/i,
  },
  {
    claimRef: 'claim_lint.referral_payouts',
    description: 'referral payout claims are gated',
    pattern: /\b(referral payout|partner payout|affiliate payout|paid referral)\b/i,
  },
]

const PRIVATE_TEXT_PATTERN =
  /@|https?:\/\/|www\.|\b(access_token|refresh_token|private_key|wallet_secret|payment_preimage|webhook_secret|xprv|mnemonic)\b/i
const DOMAIN_LIKE_PATTERN =
  /\b[a-z0-9-]+\.(com|net|org|io|ai|co|dev|app|biz|info|us|co\.uk)\b/i

const templateByRef = new Map(
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS.map(template => [
    template.templateVersionRef,
    template,
  ]),
)

export const lintBusinessOutreachClaims = (
  text: string,
): ReadonlyArray<string> =>
  BUSINESS_OUTREACH_GATED_CLAIM_DENYLIST.filter(entry =>
    entry.pattern.test(text),
  ).map(entry => entry.claimRef)

const normalizeRefs = (
  field: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = [...new Set((refs ?? []).map(ref => ref.trim()).filter(Boolean))]
  normalized.forEach(ref => assertBusinessPipelinePublicSafeRef(field, ref))
  return normalized
}

const assertPublicSafeText = (field: string, value: string): void => {
  if (
    value.trim() === '' ||
    value.length > 600 ||
    PRIVATE_TEXT_PATTERN.test(value) ||
    DOMAIN_LIKE_PATTERN.test(value)
  ) {
    throw new BusinessOutreachStoreError({
      kind: 'validation_error',
      reason: `${field} must be public-safe draft text without private contact data`,
    })
  }
}

const segmentFromVertical = (vertical: string): BusinessOutreachSegmentRef => {
  const normalized = vertical.toLowerCase()
  if (
    /\b(regulated|legal|law|health|biotech|finance|insurance|defense|manufacturing|logistics|ip-sensitive|data-rich)\b/i
      .test(normalized)
  ) {
    return 'model_custody_regulated'
  }
  if (normalized.includes('commerce')) return 'agent_readiness_ecommerce'
  if (normalized.includes('saas') || normalized.includes('api')) {
    return 'agent_readiness_saas'
  }
  if (normalized.includes('agency')) return 'agent_readiness_agency'
  if (normalized.includes('marketplace') || normalized.includes('booking')) {
    return 'agent_readiness_marketplace'
  }
  if (normalized.includes('bitcoin') || normalized.includes('lightning')) {
    return 'agent_readiness_bitcoin'
  }
  return 'agent_readiness_saas'
}

const defaultTemplateRefForSegment = (
  segmentRef: BusinessOutreachSegmentRef,
): string =>
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS.find(
    template => template.segmentRef === segmentRef,
  )?.templateVersionRef ??
  'business.outreach.agent_readiness_saas.report_led.v1'

const defaultObservedFact = (
  findingRefs: ReadonlyArray<string>,
): string =>
  findingRefs.length === 0
    ? 'the public audit found agent-readiness gaps worth reviewing'
    : `the public audit found ${findingRefs.length} agent-readiness finding refs worth reviewing`

const assertPublicSafeTimestamp = (field: string, value: string): void => {
  if (
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(value)
  ) {
    throw new BusinessOutreachStoreError({
      kind: 'validation_error',
      reason: `${field} must be an ISO timestamp`,
    })
  }
}

const renderDraftText = (
  template: BusinessOutreachTemplateVersion,
  input: Readonly<{
    auditReportRef: string
    findingRefs: ReadonlyArray<string>
    observedFact: string
    pipelineRef: string
  }>,
): string =>
  [
    `Observed fact: ${input.observedFact}.`,
    template.offerSentence,
    `${template.proofPoint} Audit ref: ${input.auditReportRef}. Finding refs: ${input.findingRefs.join(', ') || 'none'}. Pipeline ref: ${input.pipelineRef}.`,
    template.cta,
    template.identificationOptOut,
  ].join('\n\n')

const storageError = (error: unknown): BusinessOutreachStoreError =>
  error instanceof BusinessOutreachStoreError
    ? error
    : error instanceof BusinessPipelineValidationError
      ? new BusinessOutreachStoreError({
          kind: 'validation_error',
          reason: error.reason,
        })
      : new BusinessOutreachStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        })

const approvalFromRow = (
  row: TemplateApprovalRow,
): BusinessOutreachTemplateApproval => {
  const approval: BusinessOutreachTemplateApproval = {
    approvalReceiptRef: row.approval_receipt_ref,
    approvedByRef: row.approved_by_ref,
    createdAt: row.created_at,
    sourceRef: row.source_ref,
    templateVersionRef: row.template_version_ref,
  }
  assertBusinessPipelinePublicSafeRef(
    'approvalReceiptRef',
    approval.approvalReceiptRef,
  )
  assertBusinessPipelinePublicSafeRef('approvedByRef', approval.approvedByRef)
  assertBusinessPipelinePublicSafeRef('sourceRef', approval.sourceRef)
  assertBusinessPipelinePublicSafeRef(
    'templateVersionRef',
    approval.templateVersionRef,
  )
  return S.decodeUnknownSync(BusinessOutreachTemplateApproval)(approval)
}

const suppressionFromRow = (row: SuppressionRow): BusinessOutreachSuppression => {
  const suppression: BusinessOutreachSuppression = {
    createdAt: row.created_at,
    reason: row.reason,
    sourceRef: row.source_ref,
    subjectRef: row.subject_ref,
    suppressionRef: row.suppression_ref,
  }
  assertBusinessPipelinePublicSafeRef('suppressionRef', suppression.suppressionRef)
  assertBusinessPipelinePublicSafeRef('subjectRef', suppression.subjectRef)
  assertBusinessPipelinePublicSafeRef('sourceRef', suppression.sourceRef)
  return S.decodeUnknownSync(BusinessOutreachSuppression)(suppression)
}

const draftFromRow = (row: DraftRow): BusinessOutreachDraft => {
  const draft: BusinessOutreachDraft = {
    auditReportRef: row.audit_report_ref,
    bodyText: row.body_text,
    claimLintRefs: parseJsonStringArray(row.claim_lint_refs_json),
    createdAt: row.created_at,
    draftRef: row.draft_ref,
    findingRefs: parseJsonStringArray(row.finding_refs_json),
    pipelineRef: row.pipeline_ref,
    segmentRef: row.segment_ref,
    sourceRef: row.source_ref,
    state: row.state,
    subjectRef: row.subject_ref,
    templateVersionRef: row.template_version_ref,
  }
  assertBusinessPipelinePublicSafeRef('draftRef', draft.draftRef)
  assertBusinessPipelinePublicSafeRef('pipelineRef', draft.pipelineRef)
  assertBusinessPipelinePublicSafeRef('subjectRef', draft.subjectRef)
  assertBusinessPipelinePublicSafeRef(
    'templateVersionRef',
    draft.templateVersionRef,
  )
  assertBusinessPipelinePublicSafeRef('auditReportRef', draft.auditReportRef)
  assertBusinessPipelinePublicSafeRef('sourceRef', draft.sourceRef)
  draft.findingRefs.forEach(ref =>
    assertBusinessPipelinePublicSafeRef('findingRefs', ref),
  )
  draft.claimLintRefs.forEach(ref =>
    assertBusinessPipelinePublicSafeRef('claimLintRefs', ref),
  )
  assertPublicSafeText('bodyText', draft.bodyText)
  return S.decodeUnknownSync(BusinessOutreachDraft)(draft)
}

const sendFromRow = (row: SendRow): BusinessOutreachSend => {
  const send: BusinessOutreachSend = {
    approvalReceiptRef: row.approval_receipt_ref,
    channel: row.channel,
    createdAt: row.created_at,
    draftRef: row.draft_ref,
    mailboxRef: row.mailbox_ref,
    pipelineRef: row.pipeline_ref,
    sendReceiptRef: row.send_receipt_ref,
    sendRef: row.send_ref,
    sentAt: row.sent_at,
    sourceRef: row.source_ref,
    subjectRef: row.subject_ref,
    templateVersionRef: row.template_version_ref,
  }
  assertBusinessPipelinePublicSafeRef('sendRef', send.sendRef)
  assertBusinessPipelinePublicSafeRef('pipelineRef', send.pipelineRef)
  assertBusinessPipelinePublicSafeRef('draftRef', send.draftRef)
  assertBusinessPipelinePublicSafeRef('subjectRef', send.subjectRef)
  assertBusinessPipelinePublicSafeRef('mailboxRef', send.mailboxRef)
  assertBusinessPipelinePublicSafeRef('sourceRef', send.sourceRef)
  assertBusinessPipelinePublicSafeRef(
    'approvalReceiptRef',
    send.approvalReceiptRef,
  )
  assertBusinessPipelinePublicSafeRef('sendReceiptRef', send.sendReceiptRef)
  return S.decodeUnknownSync(BusinessOutreachSend)(send)
}

export type BusinessOutreachStore = Readonly<{
  approveTemplate: (
    input: BusinessOutreachTemplateApprovalInput,
    runtime?: BusinessOutreachRuntime,
  ) => Promise<BusinessOutreachTemplateApproval>
  createSuppression: (
    input: BusinessOutreachSuppressionInput,
    runtime?: BusinessOutreachRuntime,
  ) => Promise<BusinessOutreachSuppression>
  listTemplates: () => ReadonlyArray<BusinessOutreachTemplateVersion>
  recordSend: (
    pipelineRef: string,
    input: BusinessOutreachSendInput,
    runtime?: BusinessOutreachRuntime,
  ) => Promise<BusinessOutreachSendOutcome>
  renderDraft: (
    pipelineRef: string,
    input: BusinessOutreachDraftInput,
    runtime?: BusinessOutreachRuntime,
  ) => Promise<BusinessOutreachRenderOutcome>
}>

export const makeD1BusinessOutreachStore = (
  db: D1Database,
  pipelineStore: BusinessPipelineStore,
): BusinessOutreachStore => {
  const findTemplateApproval = async (
    templateVersionRef: string,
    approvalReceiptRef?: string,
  ): Promise<BusinessOutreachTemplateApproval | null> => {
    assertBusinessPipelinePublicSafeRef('templateVersionRef', templateVersionRef)
    if (approvalReceiptRef !== undefined) {
      assertBusinessPipelinePublicSafeRef('approvalReceiptRef', approvalReceiptRef)
    }
    const row = await db
      .prepare(
        `SELECT
          approval_receipt_ref,
          template_version_ref,
          approved_by_ref,
          source_ref,
          created_at
         FROM business_outreach_template_approvals
         WHERE template_version_ref = ?
           AND (? IS NULL OR approval_receipt_ref = ?)
         ORDER BY created_at DESC
         LIMIT 1`,
      )
      .bind(
        templateVersionRef,
        approvalReceiptRef ?? null,
        approvalReceiptRef ?? null,
      )
      .first<TemplateApprovalRow>()

    return row === null ? null : approvalFromRow(row)
  }

  const findSuppression = async (
    subjectRef: string,
  ): Promise<BusinessOutreachSuppression | null> => {
    assertBusinessPipelinePublicSafeRef('subjectRef', subjectRef)
    const row = await db
      .prepare(
        `SELECT
          suppression_ref,
          subject_ref,
          reason,
          source_ref,
          created_at
         FROM business_outreach_suppressions
         WHERE subject_ref = ?
         ORDER BY created_at DESC
         LIMIT 1`,
      )
      .bind(subjectRef)
      .first<SuppressionRow>()

    return row === null ? null : suppressionFromRow(row)
  }

  const readDraft = async (
    draftRef: string,
  ): Promise<BusinessOutreachDraft | null> => {
    assertBusinessPipelinePublicSafeRef('draftRef', draftRef)
    const row = await db
      .prepare(
        `SELECT
          draft_ref,
          pipeline_ref,
          subject_ref,
          template_version_ref,
          segment_ref,
          audit_report_ref,
          finding_refs_json,
          body_text,
          claim_lint_refs_json,
          source_ref,
          state,
          created_at
         FROM business_outreach_drafts
         WHERE draft_ref = ?`,
      )
      .bind(draftRef)
      .first<DraftRow>()

    return row === null ? null : draftFromRow(row)
  }

  const approveTemplate = async (
    input: BusinessOutreachTemplateApprovalInput,
    runtime: BusinessOutreachRuntime = systemBusinessOutreachRuntime,
  ): Promise<BusinessOutreachTemplateApproval> => {
    try {
      const templateVersionRef = input.templateVersionRef.trim()
      const approvalReceiptRef = input.approvalReceiptRef.trim()
      const approvedByRef = input.approvedByRef.trim()
      const sourceRef =
        input.sourceRef?.trim() ?? 'github:OpenAgentsInc/openagents#8265'
      const nowIso = runtime.nowIso()

      if (!templateByRef.has(templateVersionRef)) {
        throw new BusinessOutreachStoreError({
          kind: 'not_found',
          reason: `template version not found: ${templateVersionRef}`,
        })
      }
      assertBusinessPipelinePublicSafeRef('templateVersionRef', templateVersionRef)
      assertBusinessPipelinePublicSafeRef('approvalReceiptRef', approvalReceiptRef)
      assertBusinessPipelinePublicSafeRef('approvedByRef', approvedByRef)
      assertBusinessPipelinePublicSafeRef('sourceRef', sourceRef)

      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_outreach_template_approvals (
            approval_receipt_ref,
            template_version_ref,
            approved_by_ref,
            source_ref,
            created_at
          ) VALUES (?, ?, ?, ?, ?)`,
        )
        .bind(
          approvalReceiptRef,
          templateVersionRef,
          approvedByRef,
          sourceRef,
          nowIso,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessOutreachStoreError({
          kind: 'conflict',
          reason: `approval already exists: ${approvalReceiptRef}`,
        })
      }

      const approval = await findTemplateApproval(
        templateVersionRef,
        approvalReceiptRef,
      )
      if (approval === null) {
        throw new BusinessOutreachStoreError({
          kind: 'storage_error',
          reason: `approval was not readable after create: ${approvalReceiptRef}`,
        })
      }
      return approval
    } catch (error) {
      throw storageError(error)
    }
  }

  const createSuppression = async (
    input: BusinessOutreachSuppressionInput,
    runtime: BusinessOutreachRuntime = systemBusinessOutreachRuntime,
  ): Promise<BusinessOutreachSuppression> => {
    try {
      const subjectRef = input.subjectRef.trim()
      const sourceRef = input.sourceRef.trim()
      const suppressionRef =
        input.suppressionRef?.trim() ??
        `business.outreach.suppression.${businessPipelineSafeRefPart(subjectRef)}.${input.reason}`
      const nowIso = runtime.nowIso()

      assertBusinessPipelinePublicSafeRef('suppressionRef', suppressionRef)
      assertBusinessPipelinePublicSafeRef('subjectRef', subjectRef)
      assertBusinessPipelinePublicSafeRef('sourceRef', sourceRef)
      S.decodeUnknownSync(BusinessOutreachSuppressionReason)(input.reason)

      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_outreach_suppressions (
            suppression_ref,
            subject_ref,
            reason,
            source_ref,
            created_at
          ) VALUES (?, ?, ?, ?, ?)`,
        )
        .bind(suppressionRef, subjectRef, input.reason, sourceRef, nowIso)
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessOutreachStoreError({
          kind: 'conflict',
          reason: `suppression already exists: ${subjectRef}/${input.reason}`,
        })
      }

      const suppression = await findSuppression(subjectRef)
      if (suppression === null) {
        throw new BusinessOutreachStoreError({
          kind: 'storage_error',
          reason: `suppression was not readable after create: ${suppressionRef}`,
        })
      }
      return suppression
    } catch (error) {
      throw storageError(error)
    }
  }

  const renderDraft = async (
    pipelineRef: string,
    input: BusinessOutreachDraftInput,
    runtime: BusinessOutreachRuntime = systemBusinessOutreachRuntime,
  ): Promise<BusinessOutreachRenderOutcome> => {
    try {
      const pipeline = await pipelineStore.readPipelineRow(pipelineRef)
      if (pipeline === null) {
        throw new BusinessOutreachStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }
      if (pipeline.partnerRouteFlag) {
        return {
          ok: false,
          message: 'Partner-routed rows are suppressed from cold outreach.',
          reason: 'suppressed_subject',
        }
      }
      if (pipeline.stage !== 'intake_received') {
        return {
          ok: false,
          message: 'Active intake rows cannot enter a cold sequence.',
          reason: 'active_intake',
        }
      }

      const subjectRef = input.subjectRef.trim()
      const auditReportRef = input.auditReportRef.trim()
      const sourceRef = input.sourceRef?.trim() ?? pipeline.sourceRef
      const findingRefs = normalizeRefs('findingRefs', input.findingRefs)
      const segmentRef = segmentFromVertical(pipeline.vertical)
      const templateVersionRef =
        input.templateVersionRef?.trim() ?? defaultTemplateRefForSegment(segmentRef)
      const template = templateByRef.get(templateVersionRef)
      if (template === undefined) {
        return {
          ok: false,
          message: `Template version not found: ${templateVersionRef}`,
          reason: 'template_not_found',
        }
      }
      if (template.segmentRef !== segmentRef) {
        return {
          ok: false,
          message: `Template segment ${template.segmentRef} does not match pipeline segment ${segmentRef}.`,
          reason: 'template_mismatch',
        }
      }

      assertBusinessPipelinePublicSafeRef('pipelineRef', pipeline.pipelineRef)
      assertBusinessPipelinePublicSafeRef('subjectRef', subjectRef)
      assertBusinessPipelinePublicSafeRef('auditReportRef', auditReportRef)
      assertBusinessPipelinePublicSafeRef('sourceRef', sourceRef)
      const suppression = await findSuppression(subjectRef)
      if (suppression !== null) {
        return {
          ok: false,
          message: `Subject is suppressed from cold outreach: ${suppression.reason}`,
          reason: 'suppressed_subject',
          suppression,
        }
      }

      const observedFact =
        input.observedFact?.trim() ?? defaultObservedFact(findingRefs)
      assertPublicSafeText('observedFact', observedFact)
      const bodyText = renderDraftText(template, {
        auditReportRef,
        findingRefs,
        observedFact,
        pipelineRef: pipeline.pipelineRef,
      })
      assertPublicSafeText('bodyText', bodyText)
      const claimLintRefs = lintBusinessOutreachClaims(bodyText)
      if (claimLintRefs.length > 0) {
        return {
          ok: false,
          claimLintRefs,
          message: `Draft failed gated-claim lint: ${claimLintRefs.join(', ')}`,
          reason: 'claim_lint_failed',
        }
      }

      const draftRef =
        input.draftRef?.trim() ??
        `business_outreach_draft:${businessPipelineSafeRefPart(pipeline.pipelineRef)}:${runtime.makeId('draft')}`
      const nowIso = runtime.nowIso()
      assertBusinessPipelinePublicSafeRef('draftRef', draftRef)

      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_outreach_drafts (
            draft_ref,
            pipeline_ref,
            subject_ref,
            template_version_ref,
            segment_ref,
            audit_report_ref,
            finding_refs_json,
            body_text,
            claim_lint_refs_json,
            source_ref,
            state,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?)`,
        )
        .bind(
          draftRef,
          pipeline.pipelineRef,
          subjectRef,
          template.templateVersionRef,
          template.segmentRef,
          auditReportRef,
          JSON.stringify(findingRefs),
          bodyText,
          JSON.stringify(claimLintRefs),
          sourceRef,
          nowIso,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessOutreachStoreError({
          kind: 'conflict',
          reason: `draft already exists: ${draftRef}`,
        })
      }

      const draft = await readDraft(draftRef)
      if (draft === null) {
        throw new BusinessOutreachStoreError({
          kind: 'storage_error',
          reason: `draft was not readable after create: ${draftRef}`,
        })
      }
      return { draft, ok: true }
    } catch (error) {
      throw storageError(error)
    }
  }

  const recordSend = async (
    pipelineRef: string,
    input: BusinessOutreachSendInput,
    runtime: BusinessOutreachRuntime = systemBusinessOutreachRuntime,
  ): Promise<BusinessOutreachSendOutcome> => {
    try {
      const pipeline = await pipelineStore.readPipelineRow(pipelineRef)
      if (pipeline === null) {
        throw new BusinessOutreachStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }
      const draft = await readDraft(input.draftRef.trim())
      if (draft === null || draft.pipelineRef !== pipeline.pipelineRef) {
        return {
          ok: false,
          message: `Draft not found for pipeline: ${input.draftRef}`,
          reason: 'draft_not_found',
        }
      }

      const approval = await findTemplateApproval(
        draft.templateVersionRef,
        input.approvalReceiptRef?.trim(),
      )
      if (approval === null) {
        return {
          ok: false,
          message: 'Send recording requires an owner-approved template version.',
          reason: 'template_not_approved',
        }
      }

      const mailboxRef = input.mailboxRef.trim()
      const sourceRef = input.sourceRef.trim()
      const channel = input.channel ?? 'apollo_sequence'
      const sentAt = input.sentAt?.trim() ?? runtime.nowIso()
      const dayPrefix = sentAt.slice(0, 10)
      const dailyMailboxSendCap =
        input.dailyMailboxSendCap ?? BUSINESS_OUTREACH_DEFAULT_DAILY_MAILBOX_SEND_CAP
      if (
        !Number.isInteger(dailyMailboxSendCap) ||
        dailyMailboxSendCap <= 0 ||
        dailyMailboxSendCap > BUSINESS_OUTREACH_DEFAULT_DAILY_MAILBOX_SEND_CAP
      ) {
        throw new BusinessOutreachStoreError({
          kind: 'validation_error',
          reason: `dailyMailboxSendCap must be 1-${BUSINESS_OUTREACH_DEFAULT_DAILY_MAILBOX_SEND_CAP}`,
        })
      }
      assertBusinessPipelinePublicSafeRef('mailboxRef', mailboxRef)
      assertBusinessPipelinePublicSafeRef('sourceRef', sourceRef)
      assertPublicSafeTimestamp('sentAt', sentAt)
      S.decodeUnknownSync(BusinessOutreachChannel)(channel)

      const countRow = await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM business_outreach_sends
            WHERE mailbox_ref = ?
              AND substr(sent_at, 1, 10) = ?`,
        )
        .bind(mailboxRef, dayPrefix)
        .first<CountRow>()
      if (Number(countRow?.count ?? 0) >= dailyMailboxSendCap) {
        return {
          ok: false,
          message: `Mailbox send cap reached for ${mailboxRef} on ${dayPrefix}.`,
          reason: 'daily_mailbox_send_cap_exceeded',
        }
      }

      const sendRef =
        input.sendRef?.trim() ??
        `business_outreach_send:${businessPipelineSafeRefPart(draft.draftRef)}:${runtime.makeId('send')}`
      const sendReceiptRef = `receipt.business.outreach_send.${businessPipelineSafeRefPart(sendRef)}`
      const nowIso = runtime.nowIso()
      assertBusinessPipelinePublicSafeRef('sendRef', sendRef)
      assertBusinessPipelinePublicSafeRef('sendReceiptRef', sendReceiptRef)

      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_outreach_sends (
            send_ref,
            pipeline_ref,
            draft_ref,
            subject_ref,
            template_version_ref,
            mailbox_ref,
            channel,
            source_ref,
            approval_receipt_ref,
            send_receipt_ref,
            sent_at,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          sendRef,
          pipeline.pipelineRef,
          draft.draftRef,
          draft.subjectRef,
          draft.templateVersionRef,
          mailboxRef,
          channel,
          sourceRef,
          approval.approvalReceiptRef,
          sendReceiptRef,
          sentAt,
          nowIso,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessOutreachStoreError({
          kind: 'conflict',
          reason: `send already exists: ${sendRef}`,
        })
      }

      const row = await db
        .prepare(
          `SELECT
            send_ref,
            pipeline_ref,
            draft_ref,
            subject_ref,
            template_version_ref,
            mailbox_ref,
            channel,
            source_ref,
            approval_receipt_ref,
            send_receipt_ref,
            sent_at,
            created_at
           FROM business_outreach_sends
           WHERE send_ref = ?`,
        )
        .bind(sendRef)
        .first<SendRow>()
      if (row === null) {
        throw new BusinessOutreachStoreError({
          kind: 'storage_error',
          reason: `send was not readable after create: ${sendRef}`,
        })
      }
      const pipelineAfterReceipt = await pipelineStore.appendPipelineReceiptRefs(
        pipeline.pipelineRef,
        [sendReceiptRef],
        runtime,
      )

      return {
        ok: true,
        pipelineReceiptRefs: pipelineAfterReceipt.receiptRefs,
        send: sendFromRow(row),
      }
    } catch (error) {
      throw storageError(error)
    }
  }

  return {
    approveTemplate,
    createSuppression,
    listTemplates: () => BUSINESS_OUTREACH_TEMPLATE_VERSIONS,
    recordSend,
    renderDraft,
  }
}
