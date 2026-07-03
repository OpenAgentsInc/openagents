import { Schema as S } from 'effect'

import {
  type OmniWorkroomOutboundDeliverableReviewDecision,
} from './omni-workroom-approval-gates'
import { currentIsoTimestamp } from './runtime-primitives'

export const BUSINESS_SOCIAL_PUBLISHING_CONNECTOR_RECEIPT_SCHEMA =
  'openagents.business.social_publishing_connector.receipt.v1' as const

export const BusinessSocialPublishingPlatform = S.Literals(['x'])
export type BusinessSocialPublishingPlatform =
  typeof BusinessSocialPublishingPlatform.Type

export const BUSINESS_SOCIAL_PUBLISHING_TOOL_REF =
  'tool.business.social_publish.x.v1' as const

export const BusinessSocialPublishingConnectorReceipt = S.Struct({
  schema: S.Literal(BUSINESS_SOCIAL_PUBLISHING_CONNECTOR_RECEIPT_SCHEMA),
  receiptKind: S.Literal('business.social_publishing_connector.post_published'),
  receiptRef: S.String,
  engagementRef: S.String,
  workspaceRef: S.String,
  campaignRef: S.String,
  deliverableRef: S.String,
  platform: BusinessSocialPublishingPlatform,
  connectorRef: S.String,
  toolRef: S.Literal(BUSINESS_SOCIAL_PUBLISHING_TOOL_REF),
  approvedPostRef: S.String,
  approvalDecisionReceiptRef: S.String,
  publishRequestRef: S.String,
  platformPostRef: S.String,
  platformReceiptRef: S.String,
  externalActionKind: S.Literal('publish'),
  publishAllowed: S.Literal(true),
  credentialMaterialInModelContext: S.Literal(false),
  rawProviderPayloadInModelContext: S.Literal(false),
  createdAt: S.String,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type BusinessSocialPublishingConnectorReceipt =
  typeof BusinessSocialPublishingConnectorReceipt.Type

export type BusinessSocialPublishingConnectorInput = Readonly<{
  approvalDecision: OmniWorkroomOutboundDeliverableReviewDecision
  approvedPostRef: string
  campaignRef: string
  caveatRefs?: ReadonlyArray<string> | undefined
  connectorRef: string
  createdAt?: string | undefined
  engagementRef: string
  platform: BusinessSocialPublishingPlatform
  publishRequestRef: string
  sourceRefs: ReadonlyArray<string>
}>

export type BusinessSocialPlatformPublishRequest = Readonly<{
  approvedPostRef: string
  campaignRef: string
  connectorRef: string
  engagementRef: string
  platform: BusinessSocialPublishingPlatform
  publishRequestRef: string
  toolRef: typeof BUSINESS_SOCIAL_PUBLISHING_TOOL_REF
  workspaceRef: string
}>

export type BusinessSocialPlatformPublishResult = Readonly<{
  platformPostRef: string
  platformReceiptRef: string
}>

export type BusinessSocialPlatformPublisher = (
  request: BusinessSocialPlatformPublishRequest,
) => Promise<BusinessSocialPlatformPublishResult>

export class BusinessSocialPublishingConnectorInvariantError extends S.TaggedErrorClass<BusinessSocialPublishingConnectorInvariantError>()(
  'BusinessSocialPublishingConnectorInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/=#-]{2,220}$/
const UNSAFE_REF_PATTERN =
  /(@|access[_-]?token|\bauth\b|auth[_-]?(content|json|token)|bearer|client[_-]?(email|name|phone)|contact[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone|record|value)|email|oauth|phone|private|provider[_-]?(credential|payload|secret|token)|raw[_-]?(body|connector|content|customer|message|payload|post|provider|text|webhook)|secret|token|twitter\.com\/|x\.com\/)/i

const DEFAULT_CAVEAT_REFS = [
  'caveat.business.social_publishing.x_first_only',
  'caveat.business.social_publishing.approval_gate_required',
  'caveat.business.social_publishing.receipt_per_post',
  'caveat.business.social_publishing.sensitive_material_excluded_from_context',
] as const

const trimOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) return null
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const requirePublicSafeRef = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (!SAFE_REF_PATTERN.test(trimmed) || UNSAFE_REF_PATTERN.test(trimmed)) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: `${field} must be an opaque public-safe ref.`,
    })
  }
  return trimmed
}

const requirePublicSafeRefs = (
  field: string,
  values: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  if (values.length === 0) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: `${field} must contain at least one public-safe ref.`,
    })
  }
  return values.map(value => requirePublicSafeRef(field, value))
}

const requireIsoLike = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null || Number.isNaN(Date.parse(trimmed))) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: `${field} must be an ISO-like timestamp.`,
    })
  }
  return trimmed
}

const refSuffix = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '')

const receiptRefFor = (
  campaignRef: string,
  platformPostRef: string,
): string =>
  `receipt.business.social_publish.${refSuffix(campaignRef)}.${refSuffix(platformPostRef)}`

const firstReceiptRef = (
  refs: ReadonlyArray<string>,
): string | undefined => refs[0]

const assertApprovalDecisionAllowsPublish = (
  decision: OmniWorkroomOutboundDeliverableReviewDecision,
): string => {
  if (decision.outboundActionKind !== 'publish') {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: 'social publishing connector only accepts BF-4.3 publish decisions.',
    })
  }
  if (decision.externalActionAllowed !== true) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason:
        'social publishing connector requires a recorded approval gate decision before posting.',
    })
  }
  if (decision.deliverableRef !== decision.deliverableRef.trim()) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: 'approval decision deliverableRef must be normalized.',
    })
  }
  const receiptRef = firstReceiptRef(decision.receiptRefs)
  if (receiptRef === undefined) {
    throw new BusinessSocialPublishingConnectorInvariantError({
      reason: 'approval decision must carry a receipt ref.',
    })
  }
  return requirePublicSafeRef('approvalDecisionReceiptRef', receiptRef)
}

export const buildBusinessSocialPublishingConnectorReceipt = (
  input: BusinessSocialPublishingConnectorInput &
    BusinessSocialPlatformPublishResult,
): BusinessSocialPublishingConnectorReceipt => {
  const approvalDecisionReceiptRef = assertApprovalDecisionAllowsPublish(
    input.approvalDecision,
  )
  const campaignRef = requirePublicSafeRef('campaignRef', input.campaignRef)
  const platformPostRef = requirePublicSafeRef(
    'platformPostRef',
    input.platformPostRef,
  )

  return {
    schema: BUSINESS_SOCIAL_PUBLISHING_CONNECTOR_RECEIPT_SCHEMA,
    receiptKind: 'business.social_publishing_connector.post_published',
    receiptRef: receiptRefFor(campaignRef, platformPostRef),
    engagementRef: requirePublicSafeRef('engagementRef', input.engagementRef),
    workspaceRef: requirePublicSafeRef(
      'workspaceRef',
      input.approvalDecision.workspaceRef,
    ),
    campaignRef,
    deliverableRef: requirePublicSafeRef(
      'deliverableRef',
      input.approvalDecision.deliverableRef,
    ),
    platform: input.platform,
    connectorRef: requirePublicSafeRef('connectorRef', input.connectorRef),
    toolRef: BUSINESS_SOCIAL_PUBLISHING_TOOL_REF,
    approvedPostRef: requirePublicSafeRef(
      'approvedPostRef',
      input.approvedPostRef,
    ),
    approvalDecisionReceiptRef,
    publishRequestRef: requirePublicSafeRef(
      'publishRequestRef',
      input.publishRequestRef,
    ),
    platformPostRef,
    platformReceiptRef: requirePublicSafeRef(
      'platformReceiptRef',
      input.platformReceiptRef,
    ),
    externalActionKind: 'publish',
    publishAllowed: true,
    credentialMaterialInModelContext: false,
    rawProviderPayloadInModelContext: false,
    createdAt: requireIsoLike(
      'createdAt',
      input.createdAt ?? currentIsoTimestamp(),
    ),
    sourceRefs: [
      ...requirePublicSafeRefs('sourceRefs', input.sourceRefs),
      'docs/fable/ROADMAP_BIZ.md#BF-6.3',
    ],
    caveatRefs: [
      ...requirePublicSafeRefs(
        'caveatRefs',
        input.caveatRefs ?? DEFAULT_CAVEAT_REFS,
      ),
    ],
  }
}

export const publishApprovedBusinessSocialPost = async (
  input: BusinessSocialPublishingConnectorInput,
  publisher: BusinessSocialPlatformPublisher,
): Promise<BusinessSocialPublishingConnectorReceipt> => {
  assertApprovalDecisionAllowsPublish(input.approvalDecision)

  const publishResult = await publisher({
    approvedPostRef: requirePublicSafeRef(
      'approvedPostRef',
      input.approvedPostRef,
    ),
    campaignRef: requirePublicSafeRef('campaignRef', input.campaignRef),
    connectorRef: requirePublicSafeRef('connectorRef', input.connectorRef),
    engagementRef: requirePublicSafeRef('engagementRef', input.engagementRef),
    platform: input.platform,
    publishRequestRef: requirePublicSafeRef(
      'publishRequestRef',
      input.publishRequestRef,
    ),
    toolRef: BUSINESS_SOCIAL_PUBLISHING_TOOL_REF,
    workspaceRef: requirePublicSafeRef(
      'workspaceRef',
      input.approvalDecision.workspaceRef,
    ),
  })

  return buildBusinessSocialPublishingConnectorReceipt({
    ...input,
    ...publishResult,
  })
}

export const publicBusinessSocialPublishingConnectorProjection = (
  receipt: BusinessSocialPublishingConnectorReceipt,
) => ({
  schema: receipt.schema,
  receiptKind: receipt.receiptKind,
  receiptRef: receipt.receiptRef,
  engagementRef: receipt.engagementRef,
  workspaceRef: receipt.workspaceRef,
  campaignRef: receipt.campaignRef,
  deliverableRef: receipt.deliverableRef,
  platform: receipt.platform,
  toolRef: receipt.toolRef,
  approvedPostRef: receipt.approvedPostRef,
  approvalDecisionReceiptRef: receipt.approvalDecisionReceiptRef,
  platformPostRef: receipt.platformPostRef,
  platformReceiptRef: receipt.platformReceiptRef,
  externalActionKind: receipt.externalActionKind,
  publishAllowed: receipt.publishAllowed,
  credentialMaterialInModelContext: receipt.credentialMaterialInModelContext,
  rawProviderPayloadInModelContext: receipt.rawProviderPayloadInModelContext,
  createdAt: receipt.createdAt,
  sourceRefs: receipt.sourceRefs,
  caveatRefs: receipt.caveatRefs,
})
