import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  OpenAgentsSiteCheckoutUiPrimitiveContract,
  projectOpenAgentsSiteCheckoutUiPrimitives,
} from './site-checkout-ui-primitives'
import {
  OpenAgentsSitePaymentCatalog,
  OpenAgentsSitePaymentCatalogRecord,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  projectOpenAgentsSitePaymentCatalog,
} from './site-payment-catalog'

export const OpenAgentsSiteCommerceReviewDecisionStatus = S.Literals([
  'accepted',
  'held',
  'needs_customer_input',
  'rejected',
])
export type OpenAgentsSiteCommerceReviewDecisionStatus =
  typeof OpenAgentsSiteCommerceReviewDecisionStatus.Type

export const OpenAgentsSiteCommerceReviewStatus = S.Literals([
  'accepted',
  'held',
  'needs_customer_input',
  'needs_review',
  'rejected',
])
export type OpenAgentsSiteCommerceReviewStatus =
  typeof OpenAgentsSiteCommerceReviewStatus.Type

export class OpenAgentsSiteCommerceReviewDecisionRequest extends S.Class<OpenAgentsSiteCommerceReviewDecisionRequest>(
  'OpenAgentsSiteCommerceReviewDecisionRequest',
)({
  catalogRef: S.String,
  customerInputRequirementRefs: S.optionalKey(S.Array(S.String)),
  reasonRefs: S.optionalKey(S.Array(S.String)),
  reviewStatus: OpenAgentsSiteCommerceReviewDecisionStatus,
}) {}

export const OpenAgentsSiteCommerceReviewDecisionRecord = S.Struct({
  actorRef: S.String,
  archivedAt: S.NullOr(S.String),
  catalogRef: S.String,
  createdAt: S.String,
  customerInputRequirementRefs: S.Array(S.String),
  decisionRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  publicProjectionJson: S.String,
  reasonRefs: S.Array(S.String),
  reviewStatus: OpenAgentsSiteCommerceReviewDecisionStatus,
  siteId: S.String,
  siteVersionId: S.String,
  updatedAt: S.String,
})
export type OpenAgentsSiteCommerceReviewDecisionRecord =
  typeof OpenAgentsSiteCommerceReviewDecisionRecord.Type

export const OpenAgentsSiteCommerceReviewDecisionProjection = S.Struct({
  actorRef: S.String,
  catalogRef: S.String,
  customerInputRequirementRefs: S.Array(S.String),
  decisionRef: S.String,
  paymentAuthorityCreated: S.Literal(false),
  payoutAuthorityCreated: S.Literal(false),
  reasonRefs: S.Array(S.String),
  reviewStatus: OpenAgentsSiteCommerceReviewDecisionStatus,
  siteId: S.String,
  siteVersionId: S.String,
})
export type OpenAgentsSiteCommerceReviewDecisionProjection =
  typeof OpenAgentsSiteCommerceReviewDecisionProjection.Type

export const OpenAgentsSiteCommerceReviewItem = S.Struct({
  actionId: S.NullOr(S.String),
  agentReadable: S.Boolean,
  catalogRef: S.String,
  cancelPath: S.String,
  checkoutPath: S.String,
  customerDataRequirementRefs: S.Array(S.String),
  customerDataRequirementLabelRefs: S.Array(S.String),
  decision: S.NullOr(OpenAgentsSiteCommerceReviewDecisionProjection),
  deploymentAuthorityCreated: S.Literal(false),
  displayRef: S.String,
  entitlementScope: S.String,
  itemKind: S.Literals(['paid_action', 'product']),
  metadataRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  paymentAuthorityCreated: S.Literal(false),
  payoutAuthorityCreated: S.Literal(false),
  price: OpenAgentsPaidEndpointProductRecord.fields.price,
  priceRef: S.String,
  productId: S.NullOr(S.String),
  providerMode: S.Literals(['live_provider', 'sandbox']),
  reviewActionRefs: S.Array(S.String),
  reviewStatus: OpenAgentsSiteCommerceReviewStatus,
  sandbox: S.Boolean,
  settlementMode: S.String,
  sourceSafeCheckoutPrimitiveRefs: S.Array(S.String),
  spendCapHintRefs: S.Array(S.String),
  successPath: S.String,
})
export type OpenAgentsSiteCommerceReviewItem =
  typeof OpenAgentsSiteCommerceReviewItem.Type

export class OpenAgentsSiteCommerceReviewProjection extends S.Class<OpenAgentsSiteCommerceReviewProjection>(
  'OpenAgentsSiteCommerceReviewProjection',
)({
  audience: OpenAgentsPaymentPolicyAudience,
  caveatRefs: S.Array(S.String),
  checkoutPrimitiveRefs: S.Array(S.String),
  decisionAuthority: S.Struct({
    createsDeploymentAuthority: S.Literal(false),
    createsPaymentAuthority: S.Literal(false),
    createsPayoutAuthority: S.Literal(false),
    createsSettlementClaim: S.Literal(false),
  }),
  implementationState: S.Literals([
    'live_provider_candidate',
    'sandbox_review',
  ]),
  items: S.Array(OpenAgentsSiteCommerceReviewItem),
  redaction: S.Struct({
    exposesCheckoutQueryState: S.Literal(false),
    exposesCustomerPrivateData: S.Literal(false),
    exposesMdkCredentials: S.Literal(false),
    exposesProviderGrants: S.Literal(false),
    exposesRawInvoice: S.Literal(false),
    exposesRawTimestamps: S.Literal(false),
    exposesWalletMaterial: S.Literal(false),
  }),
  reviewStatusCounts: S.Struct({
    accepted: S.Number,
    held: S.Number,
    needsCustomerInput: S.Number,
    needsReview: S.Number,
    rejected: S.Number,
  }),
  siteId: S.String,
  siteVersionId: S.String,
  sourceSafety: S.Struct({
    generatedSourceEmbedsMdkCredentials: S.Literal(false),
    generatedSourceEmbedsProviderGrants: S.Literal(false),
    generatedSourceEmbedsRawInvoices: S.Literal(false),
    generatedSourceEmbedsWalletMaterial: S.Literal(false),
    usesHostedOmegaPaymentBoundary: S.Literal(true),
  }),
}) {}

export const OpenAgentsSiteCommerceReviewInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  catalog: OpenAgentsSitePaymentCatalog,
  cancelPath: S.String,
  decisions: S.Array(OpenAgentsSiteCommerceReviewDecisionRecord),
  successPath: S.String,
  uiPrimitives: OpenAgentsSiteCheckoutUiPrimitiveContract,
})
export type OpenAgentsSiteCommerceReviewInput =
  typeof OpenAgentsSiteCommerceReviewInput.Type

export class OpenAgentsSiteCommerceReviewUnsafe extends S.TaggedErrorClass<OpenAgentsSiteCommerceReviewUnsafe>()(
  'OpenAgentsSiteCommerceReviewUnsafe',
  {
    reason: S.String,
  },
) {}

export class OpenAgentsSiteCommerceReviewStorageError extends S.TaggedErrorClass<OpenAgentsSiteCommerceReviewStorageError>()(
  'OpenAgentsSiteCommerceReviewStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type OpenAgentsSiteCommerceReviewStore = Readonly<{
  listReviewDecisionsForSite: (
    siteId: string,
  ) => Promise<ReadonlyArray<OpenAgentsSiteCommerceReviewDecisionRecord>>
  readReviewDecisionByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<OpenAgentsSiteCommerceReviewDecisionRecord | undefined>
  upsertReviewDecision: (
    decision: OpenAgentsSiteCommerceReviewDecisionRecord,
  ) => Promise<OpenAgentsSiteCommerceReviewDecisionRecord>
}>

type ReviewDecisionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  catalog_ref: string
  created_at: string
  customer_input_requirement_refs_json: string
  decision_ref: string
  id: string
  idempotency_key_hash: string
  public_projection_json: string
  reason_refs_json: string
  review_status:
    | 'accepted'
    | 'held'
    | 'needs_customer_input'
    | 'rejected'
  site_id: string
  site_version_id: string
  updated_at: string
}>

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const stableIdPattern = /^[A-Za-z][A-Za-z0-9_.:-]{0,180}$/
const cleanLocalPathPattern = /^\/(?!\/)[^?#]*$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|full[_-]?destination|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|wallet[_-]?(config|mnemonic|secret|state)|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?(config|mnemonic|secret|state))/i

const valueHasUnsafeMaterial = (
  value: unknown,
  options: Readonly<{ rejectRawTimestamps: boolean }> = {
    rejectRawTimestamps: true,
  },
): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      (options.rejectRawTimestamps && rawTimestampPattern.test(value)) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(item => valueHasUnsafeMaterial(item, options))
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        (typeof item !== 'boolean' && unsafeKeyPattern.test(key)) ||
        valueHasUnsafeMaterial(item, options),
      )
  }

  return false
}

const refIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !rawTimestampPattern.test(value) &&
  !valueHasUnsafeMaterial(value)

const idIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableIdPattern.test(value) &&
  !rawTimestampPattern.test(value) &&
  !valueHasUnsafeMaterial(value)

const pathIsSafe = (value: string): boolean =>
  cleanLocalPathPattern.test(value) &&
  !rawTimestampPattern.test(value) &&
  !valueHasUnsafeMaterial(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(refIsSafe)

const jsonStringArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify(safeRefs(values))

const storageError = (
  operation: string,
  error: unknown,
): OpenAgentsSiteCommerceReviewStorageError =>
  new OpenAgentsSiteCommerceReviewStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const reviewDecisionFromRow = (
  row: ReviewDecisionRow,
): OpenAgentsSiteCommerceReviewDecisionRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  catalogRef: row.catalog_ref,
  createdAt: row.created_at,
  customerInputRequirementRefs: [
    ...parseJsonStringArray(row.customer_input_requirement_refs_json),
  ],
  decisionRef: row.decision_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  publicProjectionJson: row.public_projection_json,
  reasonRefs: [...parseJsonStringArray(row.reason_refs_json)],
  reviewStatus: row.review_status,
  siteId: row.site_id,
  siteVersionId: row.site_version_id,
  updatedAt: row.updated_at,
})

const decisionProjection = (
  decision: OpenAgentsSiteCommerceReviewDecisionRecord,
): OpenAgentsSiteCommerceReviewDecisionProjection => ({
  actorRef: refIsSafe(decision.actorRef) ? decision.actorRef : 'actor.redacted',
  catalogRef: refIsSafe(decision.catalogRef)
    ? decision.catalogRef
    : 'site_payment:redacted',
  customerInputRequirementRefs: safeRefs(
    decision.customerInputRequirementRefs,
  ),
  decisionRef: refIsSafe(decision.decisionRef)
    ? decision.decisionRef
    : 'site_commerce_review:redacted',
  paymentAuthorityCreated: false,
  payoutAuthorityCreated: false,
  reasonRefs: safeRefs(decision.reasonRefs),
  reviewStatus: decision.reviewStatus,
  siteId: refIsSafe(decision.siteId) ? decision.siteId : 'site.redacted',
  siteVersionId: refIsSafe(decision.siteVersionId)
    ? decision.siteVersionId
    : 'version.redacted',
})

export const projectOpenAgentsSiteCommerceReviewDecision = (
  decision: OpenAgentsSiteCommerceReviewDecisionRecord,
): OpenAgentsSiteCommerceReviewDecisionProjection =>
  S.decodeUnknownSync(OpenAgentsSiteCommerceReviewDecisionProjection)(
    decisionProjection(decision),
  )

export const assertOpenAgentsSiteCommerceReviewDecisionSafe = (
  decision: OpenAgentsSiteCommerceReviewDecisionRecord,
): void => {
  S.decodeUnknownSync(OpenAgentsSiteCommerceReviewDecisionRecord)(decision)

  if (
    valueHasUnsafeMaterial(decision, { rejectRawTimestamps: false }) ||
    !refIsSafe(decision.siteId) ||
    !refIsSafe(decision.siteVersionId) ||
    !refIsSafe(decision.catalogRef) ||
    !refIsSafe(decision.decisionRef) ||
    !idIsSafe(decision.id) ||
    !refIsSafe(decision.idempotencyKeyHash) ||
    !refIsSafe(decision.actorRef) ||
    decision.reasonRefs.some(ref => !refIsSafe(ref)) ||
    decision.customerInputRequirementRefs.some(ref => !refIsSafe(ref))
  ) {
    throw new OpenAgentsSiteCommerceReviewUnsafe({
      reason:
        'Site commerce review decisions must use stable public-safe refs and must not contain private customer, payment, provider, wallet, raw checkout, or secret material.',
    })
  }
}

const decisionForCatalogItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
  decisions: ReadonlyArray<OpenAgentsSiteCommerceReviewDecisionRecord>,
): OpenAgentsSiteCommerceReviewDecisionRecord | undefined =>
  decisions.find(decision =>
    decision.siteId === item.siteId &&
    decision.siteVersionId === item.siteVersionId &&
    decision.catalogRef === item.catalogRef &&
    decision.archivedAt === null,
  )

const priceRefForCatalogItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): string =>
  [
    'price',
    item.price.asset,
    item.price.denomination,
    String(item.price.amountMinorUnits),
  ].join('.')

const reviewActionRefsForCatalogItem = (
  item: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<string> =>
  [
    'accept',
    'hold',
    'reject',
    'needs_customer_input',
  ].map(action => `review.site_commerce.${item.catalogRef}:${action}`)

const primitivesForCatalogItem = (
  uiPrimitives: OpenAgentsSiteCheckoutUiPrimitiveContract,
  item: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<string> =>
  uiPrimitives.primitives
    .filter(primitive => primitive.catalogRef === item.catalogRef)
    .map(primitive => primitive.id)
    .filter(refIsSafe)

const customerDataRequirementRefs = (
  item: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<string> =>
  item.customerDataRequirements.map(requirement => requirement.key)

const customerDataRequirementLabelRefs = (
  item: OpenAgentsSitePaymentCatalogRecord,
): ReadonlyArray<string> =>
  item.customerDataRequirements.map(requirement => requirement.labelRef)

const reviewItemForCatalogItem = (
  input: OpenAgentsSiteCommerceReviewInput,
  item: OpenAgentsSitePaymentCatalogRecord,
  projectedCatalogItem: ReturnType<
    typeof projectOpenAgentsSitePaymentCatalog
  >['items'][number],
): OpenAgentsSiteCommerceReviewItem => {
  const decision = decisionForCatalogItem(item, input.decisions)
  const paidEndpointProduct =
    openAgentsPaidEndpointProductFromSitePaymentCatalogItem(item)

  return {
    actionId: item.itemKind === 'paid_action'
      ? item.actionId
      : null,
    agentReadable: projectedCatalogItem.agentReadable,
    catalogRef: projectedCatalogItem.catalogRef,
    cancelPath: input.cancelPath,
    checkoutPath: projectedCatalogItem.checkoutPath,
    customerDataRequirementLabelRefs: safeRefs(
      customerDataRequirementLabelRefs(item),
    ),
    customerDataRequirementRefs: safeRefs(customerDataRequirementRefs(item)),
    decision: decision === undefined
      ? null
      : projectOpenAgentsSiteCommerceReviewDecision(decision),
    deploymentAuthorityCreated: false,
    displayRef: projectedCatalogItem.displayRef,
    entitlementScope: projectedCatalogItem.entitlementScope,
    itemKind: projectedCatalogItem.itemKind,
    metadataRefs: projectedCatalogItem.metadataRefs,
    operatorRefs: projectedCatalogItem.operatorRefs,
    paymentAuthorityCreated: false,
    payoutAuthorityCreated: false,
    price: projectedCatalogItem.price,
    priceRef: priceRefForCatalogItem(item),
    productId: item.itemKind === 'product'
      ? item.productId
      : null,
    providerMode: item.sandbox ? 'sandbox' : 'live_provider',
    reviewActionRefs: safeRefs(reviewActionRefsForCatalogItem(item)),
    reviewStatus: decision?.reviewStatus ?? 'needs_review',
    sandbox: item.sandbox,
    settlementMode: projectedCatalogItem.settlementMode,
    sourceSafeCheckoutPrimitiveRefs: safeRefs(
      primitivesForCatalogItem(input.uiPrimitives, item),
    ),
    spendCapHintRefs: safeRefs(paidEndpointProduct.spendCapHintRefs),
    successPath: input.successPath,
  }
}

const statusCounts = (
  items: ReadonlyArray<OpenAgentsSiteCommerceReviewItem>,
) => ({
  accepted: items.filter(item => item.reviewStatus === 'accepted').length,
  held: items.filter(item => item.reviewStatus === 'held').length,
  needsCustomerInput: items.filter(
    item => item.reviewStatus === 'needs_customer_input',
  ).length,
  needsReview: items.filter(item => item.reviewStatus === 'needs_review')
    .length,
  rejected: items.filter(item => item.reviewStatus === 'rejected').length,
})

export const projectOpenAgentsSiteCommerceReview = (
  input: OpenAgentsSiteCommerceReviewInput,
): OpenAgentsSiteCommerceReviewProjection => {
  const projectedCatalog = projectOpenAgentsSitePaymentCatalog(
    input.catalog,
    input.audience,
  )
  const projectedPrimitives = projectOpenAgentsSiteCheckoutUiPrimitives(
    input.uiPrimitives,
    input.audience,
  )
  const items = input.catalog.items
    .map(item => ({
      item,
      projection: projectedCatalog.items.find(
        projected => projected.catalogRef === item.catalogRef,
      ),
    }))
    .filter(
      (
        pair,
      ): pair is {
        item: OpenAgentsSitePaymentCatalogRecord
        projection: typeof projectedCatalog.items[number]
      } => pair.projection !== undefined,
    )
    .map(pair => reviewItemForCatalogItem(input, pair.item, pair.projection))
  const firstItem = input.catalog.items[0]
  const projection = new OpenAgentsSiteCommerceReviewProjection({
    audience: input.audience,
    caveatRefs: [
      'caveat.site_commerce_review.not_payment_authority',
      'caveat.site_commerce_review.not_payout_authority',
      'caveat.site_commerce_review.not_deployment_authority',
    ],
    checkoutPrimitiveRefs: safeRefs(
      projectedPrimitives.primitives.map(primitive => primitive.id),
    ),
    decisionAuthority: {
      createsDeploymentAuthority: false,
      createsPaymentAuthority: false,
      createsPayoutAuthority: false,
      createsSettlementClaim: false,
    },
    implementationState: input.catalog.items.some(item => !item.sandbox)
      ? 'live_provider_candidate'
      : 'sandbox_review',
    items,
    redaction: {
      exposesCheckoutQueryState: false,
      exposesCustomerPrivateData: false,
      exposesMdkCredentials: false,
      exposesProviderGrants: false,
      exposesRawInvoice: false,
      exposesRawTimestamps: false,
      exposesWalletMaterial: false,
    },
    reviewStatusCounts: statusCounts(items),
    siteId: firstItem?.siteId ?? 'site.generated',
    siteVersionId: firstItem?.siteVersionId ?? 'version.generated',
    sourceSafety: {
      generatedSourceEmbedsMdkCredentials: false,
      generatedSourceEmbedsProviderGrants: false,
      generatedSourceEmbedsRawInvoices: false,
      generatedSourceEmbedsWalletMaterial: false,
      usesHostedOmegaPaymentBoundary: true,
    },
  })

  if (
    valueHasUnsafeMaterial(projection, { rejectRawTimestamps: true }) ||
    !refIsSafe(projection.siteId) ||
    !refIsSafe(projection.siteVersionId) ||
    !pathIsSafe(input.successPath) ||
    !pathIsSafe(input.cancelPath)
  ) {
    throw new OpenAgentsSiteCommerceReviewUnsafe({
      reason:
        'Site commerce review projection must not expose private customer, payment, provider, wallet, raw checkout, timestamp, or secret material.',
    })
  }

  return projection
}

export const siteCommerceReviewDecisionPublicJson = (
  decision: OpenAgentsSiteCommerceReviewDecisionRecord,
): string =>
  JSON.stringify(projectOpenAgentsSiteCommerceReviewDecision(decision))

const bindDecision = (
  db: D1Database,
  decision: OpenAgentsSiteCommerceReviewDecisionRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO site_commerce_review_decisions
       (id, decision_ref, idempotency_key_hash, site_id, site_version_id,
        catalog_ref, review_status, reason_refs_json,
        customer_input_requirement_refs_json, actor_ref,
        public_projection_json, created_at, updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(site_id, site_version_id, catalog_ref)
       DO UPDATE SET
         decision_ref = excluded.decision_ref,
         idempotency_key_hash = excluded.idempotency_key_hash,
         review_status = excluded.review_status,
         reason_refs_json = excluded.reason_refs_json,
         customer_input_requirement_refs_json =
           excluded.customer_input_requirement_refs_json,
         actor_ref = excluded.actor_ref,
         public_projection_json = excluded.public_projection_json,
         updated_at = excluded.updated_at,
         archived_at = NULL`,
    )
    .bind(
      decision.id,
      decision.decisionRef,
      decision.idempotencyKeyHash,
      decision.siteId,
      decision.siteVersionId,
      decision.catalogRef,
      decision.reviewStatus,
      jsonStringArray(decision.reasonRefs),
      jsonStringArray(decision.customerInputRequirementRefs),
      decision.actorRef,
      decision.publicProjectionJson,
      decision.createdAt,
      decision.updatedAt,
      decision.archivedAt,
    )

export const makeD1SiteCommerceReviewStore = (
  db: D1Database,
): OpenAgentsSiteCommerceReviewStore => ({
  listReviewDecisionsForSite: async siteId => {
    try {
      const rows = await db
        .prepare(
          `SELECT *
             FROM site_commerce_review_decisions
            WHERE site_id = ?
              AND archived_at IS NULL
            ORDER BY updated_at DESC`,
        )
        .bind(siteId)
        .all<ReviewDecisionRow>()

      return rows.results.map(reviewDecisionFromRow)
    } catch (error) {
      throw storageError('siteCommerceReview.listDecisionsForSite', error)
    }
  },
  readReviewDecisionByIdempotencyKeyHash: async idempotencyKeyHash => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_commerce_review_decisions
            WHERE idempotency_key_hash = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKeyHash)
        .first<ReviewDecisionRow>()

      return row === null ? undefined : reviewDecisionFromRow(row)
    } catch (error) {
      throw storageError('siteCommerceReview.readDecisionByIdempotency', error)
    }
  },
  upsertReviewDecision: async decision => {
    assertOpenAgentsSiteCommerceReviewDecisionSafe(decision)

    try {
      await bindDecision(db, decision).run()

      const row = await db
        .prepare(
          `SELECT *
             FROM site_commerce_review_decisions
            WHERE decision_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(decision.decisionRef)
        .first<ReviewDecisionRow>()

      if (row === null) {
        return decision
      }

      return reviewDecisionFromRow(row)
    } catch (error) {
      throw storageError('siteCommerceReview.upsertDecision', error)
    }
  },
})
