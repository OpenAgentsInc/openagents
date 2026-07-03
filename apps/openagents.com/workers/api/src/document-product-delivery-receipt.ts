import { Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'

/**
 * BF-4.4 document-product pipeline receipt.
 *
 * Contract: typed intake spec -> generation grounded in the customer's own
 * template/corpus refs -> professional review gate -> delivery receipt.
 *
 * The receipt is refs-only and public-safe. It must never contain raw customer
 * documents, prompts, generated document bodies, legal advice, payment secrets,
 * local paths, or provider payloads. A verified receipt proves that a bounded
 * document product completed the pipeline; it does not grant publish, spend,
 * settlement, payout, or green product-promise authority.
 */

export const DOCUMENT_PRODUCT_DELIVERY_RECEIPT_KIND =
  'document_product_delivery' as const

export const DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION =
  'document.product.delivery_receipt.v1' as const

export const DocumentProductKind = S.Literals([
  'formation_document',
  'operating_agreement',
  'client_report',
  'proposal',
  'policy_packet',
])
export type DocumentProductKind = typeof DocumentProductKind.Type

export const DocumentProductPipelineStage = S.Literals([
  'blocked',
  'intake_ready',
  'generated_for_review',
  'delivered',
])
export type DocumentProductPipelineStage =
  typeof DocumentProductPipelineStage.Type

export const DocumentProductEvidenceState = S.Literals([
  'intake_received',
  'generated_grounded_draft',
  'review_accepted',
  'delivered',
  'not_yet_evidenced',
])
export type DocumentProductEvidenceState =
  typeof DocumentProductEvidenceState.Type

export const DocumentProductAuthorityGateId = S.Literals([
  'typed_intake_spec',
  'customer_template_or_corpus',
  'grounding_citations',
  'redaction_or_safe_handling',
  'professional_review',
  'customer_delivery_authority',
])
export type DocumentProductAuthorityGateId =
  typeof DocumentProductAuthorityGateId.Type

export const DocumentProductAuthorityGateState = S.Literals([
  'receipted',
  'blocked',
])
export type DocumentProductAuthorityGateState =
  typeof DocumentProductAuthorityGateState.Type

export const DocumentProductAuthorityGate = S.Struct({
  gateId: DocumentProductAuthorityGateId,
  state: DocumentProductAuthorityGateState,
})
export type DocumentProductAuthorityGate =
  typeof DocumentProductAuthorityGate.Type

export const DocumentProductReviewDecision = S.Struct({
  accepted: S.Boolean,
  reviewerRole: S.Literals(['operator', 'practitioner', 'customer']),
  reviewReceiptRef: S.NullOr(S.String),
})
export type DocumentProductReviewDecision =
  typeof DocumentProductReviewDecision.Type

export const DocumentProductDeliveryReceipt = S.Struct({
  receiptKind: S.Literal(DOCUMENT_PRODUCT_DELIVERY_RECEIPT_KIND),
  productKind: DocumentProductKind,
  workItemRef: S.String,
  pipelineStage: DocumentProductPipelineStage,
  evidenceState: DocumentProductEvidenceState,
  noAutoPublish: S.Literal(true),
  noAutoSend: S.Literal(true),
  noLegalAdvice: S.Literal(true),
  authorityGates: S.Array(DocumentProductAuthorityGate),
  outstandingAuthorityBlockers: S.Array(DocumentProductAuthorityGateId),
  intakeSpecRef: S.NullOr(S.String),
  customerTemplateRefs: S.Array(S.String),
  customerCorpusRefs: S.Array(S.String),
  generatedDraftRefs: S.Array(S.String),
  groundingCitationRefs: S.Array(S.String),
  redactionReportRef: S.NullOr(S.String),
  reviewDecision: DocumentProductReviewDecision,
  deliveredDocumentRefs: S.Array(S.String),
  deliveryReceiptRef: S.NullOr(S.String),
  freshnessTimestamp: S.String,
  publicSourceRefs: S.Array(S.String),
})
export type DocumentProductDeliveryReceipt =
  typeof DocumentProductDeliveryReceipt.Type

export class DocumentProductDeliveryReceiptInvariantError extends S.TaggedErrorClass<DocumentProductDeliveryReceiptInvariantError>()(
  'DocumentProductDeliveryReceiptInvariantError',
  { reason: S.String },
) {}

const ALL_GATE_IDS: ReadonlyArray<DocumentProductAuthorityGateId> = [
  'typed_intake_spec',
  'customer_template_or_corpus',
  'grounding_citations',
  'redaction_or_safe_handling',
  'professional_review',
  'customer_delivery_authority',
]

const RAW_PRIVATE_REF_PATTERN =
  /(@|\/Users\/|\/home\/|auth\.json|bearer|customer[_-]?(email|name|prompt|record|value)|raw[_-]?(customer|document|prompt|source|template)|private[_-]?(customer|document|repo|source)|provider[_-]?(payload|secret|token)|secret|token|wallet)/i

const assertPublicSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = refs.find(ref => RAW_PRIVATE_REF_PATTERN.test(ref))
  if (unsafe != null) {
    throw new DocumentProductDeliveryReceiptInvariantError({
      reason: `${label} contains non-public-safe ref: ${unsafe}`,
    })
  }
}

const isProfessionalReviewerRole = (
  reviewerRole: DocumentProductReviewDecision['reviewerRole'],
): boolean => reviewerRole === 'operator' || reviewerRole === 'practitioner'

export type DocumentProductDeliveryInput = Readonly<{
  productKind: DocumentProductKind
  workItemRef: string
  intakeSpecRef: string | null
  customerTemplateRefs: ReadonlyArray<string>
  customerCorpusRefs: ReadonlyArray<string>
  generatedDraftRefs: ReadonlyArray<string>
  groundingCitationRefs: ReadonlyArray<string>
  redactionReportRef: string | null
  reviewDecision: DocumentProductReviewDecision
  deliveredDocumentRefs: ReadonlyArray<string>
  deliveryReceiptRef: string | null
  freshnessTimestamp: string
  publicSourceRefs: ReadonlyArray<string>
}>

export const buildDocumentProductDeliveryReceipt = (
  input: DocumentProductDeliveryInput,
): DocumentProductDeliveryReceipt => {
  const receiptedGateIds = new Set<DocumentProductAuthorityGateId>()

  if (input.intakeSpecRef != null) {
    receiptedGateIds.add('typed_intake_spec')
  }
  if (
    input.customerTemplateRefs.length > 0 ||
    input.customerCorpusRefs.length > 0
  ) {
    receiptedGateIds.add('customer_template_or_corpus')
  }
  if (input.groundingCitationRefs.length > 0) {
    receiptedGateIds.add('grounding_citations')
  }
  if (input.redactionReportRef != null) {
    receiptedGateIds.add('redaction_or_safe_handling')
  }
  if (
    input.reviewDecision.accepted &&
    isProfessionalReviewerRole(input.reviewDecision.reviewerRole)
  ) {
    receiptedGateIds.add('professional_review')
  }
  if (input.deliveryReceiptRef != null) {
    receiptedGateIds.add('customer_delivery_authority')
  }

  const authorityGates: ReadonlyArray<DocumentProductAuthorityGate> =
    ALL_GATE_IDS.map(gateId => ({
      gateId,
      state: receiptedGateIds.has(gateId) ? 'receipted' : 'blocked',
    }))
  const outstandingAuthorityBlockers = authorityGates
    .filter(gate => gate.state === 'blocked')
    .map(gate => gate.gateId)

  const hasBlockedGate = outstandingAuthorityBlockers.length > 0

  if (input.reviewDecision.accepted && input.reviewDecision.reviewReceiptRef == null) {
    throw new DocumentProductDeliveryReceiptInvariantError({
      reason: 'accepted review decision requires a review receipt ref',
    })
  }

  if (
    input.reviewDecision.accepted &&
    !isProfessionalReviewerRole(input.reviewDecision.reviewerRole)
  ) {
    throw new DocumentProductDeliveryReceiptInvariantError({
      reason:
        'professional review gate requires an operator or practitioner reviewer role',
    })
  }

  if (hasBlockedGate && input.deliveredDocumentRefs.length > 0) {
    throw new DocumentProductDeliveryReceiptInvariantError({
      reason:
        'delivered document refs present while authority gates are blocked: ' +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  const allPublicRefs = [
    input.workItemRef,
    ...(input.intakeSpecRef == null ? [] : [input.intakeSpecRef]),
    ...input.customerTemplateRefs,
    ...input.customerCorpusRefs,
    ...input.generatedDraftRefs,
    ...input.groundingCitationRefs,
    ...(input.redactionReportRef == null ? [] : [input.redactionReportRef]),
    ...(input.reviewDecision.reviewReceiptRef == null
      ? []
      : [input.reviewDecision.reviewReceiptRef]),
    ...input.deliveredDocumentRefs,
    ...(input.deliveryReceiptRef == null ? [] : [input.deliveryReceiptRef]),
    ...input.publicSourceRefs,
  ]
  assertPublicSafeRefs('document-product receipt', allPublicRefs)

  const pipelineStage: DocumentProductPipelineStage = hasBlockedGate
    ? input.generatedDraftRefs.length > 0
      ? 'generated_for_review'
      : input.intakeSpecRef != null
        ? 'intake_ready'
        : 'blocked'
    : 'delivered'

  const evidenceState: DocumentProductEvidenceState =
    pipelineStage === 'delivered'
      ? 'delivered'
      : input.reviewDecision.accepted
        ? 'review_accepted'
        : input.generatedDraftRefs.length > 0
          ? 'generated_grounded_draft'
          : input.intakeSpecRef != null
            ? 'intake_received'
            : 'not_yet_evidenced'

  return {
    receiptKind: DOCUMENT_PRODUCT_DELIVERY_RECEIPT_KIND,
    productKind: input.productKind,
    workItemRef: input.workItemRef,
    pipelineStage,
    evidenceState,
    noAutoPublish: true,
    noAutoSend: true,
    noLegalAdvice: true,
    authorityGates,
    outstandingAuthorityBlockers,
    intakeSpecRef: input.intakeSpecRef,
    customerTemplateRefs: [...input.customerTemplateRefs],
    customerCorpusRefs: [...input.customerCorpusRefs],
    generatedDraftRefs: [...input.generatedDraftRefs],
    groundingCitationRefs: [...input.groundingCitationRefs],
    redactionReportRef: input.redactionReportRef,
    reviewDecision: input.reviewDecision,
    deliveredDocumentRefs: [...input.deliveredDocumentRefs],
    deliveryReceiptRef: input.deliveryReceiptRef,
    freshnessTimestamp: input.freshnessTimestamp,
    publicSourceRefs: [...input.publicSourceRefs],
  }
}

export const verifyDocumentProductDelivery = (
  receipt: DocumentProductDeliveryReceipt,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (receipt.pipelineStage !== 'delivered') {
    reasons.push(`pipeline stage is ${receipt.pipelineStage}, not delivered`)
  }
  if (receipt.outstandingAuthorityBlockers.length > 0) {
    reasons.push(
      'outstanding authority blockers: ' +
        receipt.outstandingAuthorityBlockers.join(', '),
    )
  }
  if (receipt.intakeSpecRef == null) {
    reasons.push('typed intake spec not receipted')
  }
  if (
    receipt.customerTemplateRefs.length === 0 &&
    receipt.customerCorpusRefs.length === 0
  ) {
    reasons.push('no customer template or corpus refs')
  }
  if (receipt.generatedDraftRefs.length === 0) {
    reasons.push('no generated draft refs')
  }
  if (receipt.groundingCitationRefs.length === 0) {
    reasons.push('no grounding citation refs')
  }
  if (receipt.redactionReportRef == null) {
    reasons.push('redaction/safe-handling report not receipted')
  }
  if (!receipt.reviewDecision.accepted) {
    reasons.push('professional-review gate not accepted')
  }
  if (
    receipt.reviewDecision.accepted &&
    receipt.reviewDecision.reviewReceiptRef == null
  ) {
    reasons.push('accepted review lacks review receipt ref')
  }
  if (receipt.deliveredDocumentRefs.length === 0) {
    reasons.push('no delivered document refs')
  }
  if (receipt.deliveryReceiptRef == null) {
    reasons.push('delivery receipt ref missing')
  }

  return reasons
}

export const DocumentProductDeliveryReceiptDocument = S.Struct({
  docVersion: S.Literal(DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION),
  receipt: DocumentProductDeliveryReceipt,
})
export type DocumentProductDeliveryReceiptDocument =
  typeof DocumentProductDeliveryReceiptDocument.Type

const encodeReceiptDocument = S.encodeSync(
  DocumentProductDeliveryReceiptDocument,
)

export const toDocumentProductDeliveryReceiptDocument = (
  receipt: DocumentProductDeliveryReceipt,
): DocumentProductDeliveryReceiptDocument => ({
  docVersion: DOCUMENT_PRODUCT_DELIVERY_RECEIPT_DOC_VERSION,
  receipt,
})

export const serializeDocumentProductDeliveryReceiptDocument = (
  document: DocumentProductDeliveryReceiptDocument,
): string => JSON.stringify(encodeReceiptDocument(document))

export const decodeDocumentProductDeliveryReceiptDocument = (
  body: string,
): DocumentProductDeliveryReceiptDocument =>
  parseJsonWithSchema(DocumentProductDeliveryReceiptDocument, body)

export const verifyDereferencedDocumentProductReceipt = (
  body: string,
): ReadonlyArray<string> => {
  let document: DocumentProductDeliveryReceiptDocument
  try {
    document = decodeDocumentProductDeliveryReceiptDocument(body)
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error)
    return [`receipt document failed to decode: ${detail}`]
  }

  return verifyDocumentProductDelivery(document.receipt)
}
