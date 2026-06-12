import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSitePaymentCatalogRecord } from './site-payment-catalog'

export const OpenAgentsSiteMdkAccountProviderMode = S.Literals([
  'customer_owned_mdk',
  'openagents_hosted_mdk',
])
export type OpenAgentsSiteMdkAccountProviderMode =
  typeof OpenAgentsSiteMdkAccountProviderMode.Type

export const OpenAgentsSiteMdkAccountEnvironment = S.Literals([
  'production',
  'sandbox',
])
export type OpenAgentsSiteMdkAccountEnvironment =
  typeof OpenAgentsSiteMdkAccountEnvironment.Type

export const OpenAgentsSiteMdkAccountReviewStatus = S.Literals([
  'approved',
  'blocked',
  'pending_review',
  'revoked',
])
export type OpenAgentsSiteMdkAccountReviewStatus =
  typeof OpenAgentsSiteMdkAccountReviewStatus.Type

export const OpenAgentsSiteMdkAccountBindingState = S.Literals([
  'blocked',
  'configured',
  'pending_review',
  'revoked',
  'unavailable',
])
export type OpenAgentsSiteMdkAccountBindingState =
  typeof OpenAgentsSiteMdkAccountBindingState.Type

export class OpenAgentsSiteMdkAccountBindingRequest extends S.Class<OpenAgentsSiteMdkAccountBindingRequest>(
  'OpenAgentsSiteMdkAccountBindingRequest',
)({
  allowedActionRefs: S.optionalKey(S.Array(S.String)),
  allowedCatalogRefs: S.optionalKey(S.Array(S.String)),
  allowedProductRefs: S.optionalKey(S.Array(S.String)),
  bindingRef: S.optionalKey(S.String),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  customerRef: S.NullOr(S.String),
  environment: OpenAgentsSiteMdkAccountEnvironment,
  orderRef: S.NullOr(S.String),
  requestedProviderMode: S.Literal('customer_owned_mdk'),
  reviewStatus: OpenAgentsSiteMdkAccountReviewStatus,
  reviewerRefs: S.optionalKey(S.Array(S.String)),
  secretBindingRefs: S.Array(S.String),
  siteVersionId: S.NullOr(S.String),
}) {}

export const OpenAgentsSiteMdkAccountBindingRecord = S.Struct({
  allowedActionRefs: S.Array(S.String),
  allowedCatalogRefs: S.Array(S.String),
  allowedProductRefs: S.Array(S.String),
  archivedAt: S.NullOr(S.String),
  bindingRef: S.String,
  caveatRefs: S.Array(S.String),
  createdAt: S.String,
  customerRef: S.NullOr(S.String),
  environment: OpenAgentsSiteMdkAccountEnvironment,
  id: S.String,
  idempotencyKeyHash: S.String,
  orderRef: S.NullOr(S.String),
  publicProjectionJson: S.String,
  requestedProviderMode: S.Literal('customer_owned_mdk'),
  reviewStatus: OpenAgentsSiteMdkAccountReviewStatus,
  reviewerRefs: S.Array(S.String),
  secretBindingRefs: S.Array(S.String),
  siteId: S.String,
  siteVersionId: S.NullOr(S.String),
  updatedAt: S.String,
})
export type OpenAgentsSiteMdkAccountBindingRecord =
  typeof OpenAgentsSiteMdkAccountBindingRecord.Type

export class OpenAgentsSiteMdkAccountBindingProjection extends S.Class<OpenAgentsSiteMdkAccountBindingProjection>(
  'OpenAgentsSiteMdkAccountBindingProjection',
)({
  allowedActionRefs: S.Array(S.String),
  allowedCatalogRefs: S.Array(S.String),
  allowedProductRefs: S.Array(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  bindingRef: S.NullOr(S.String),
  bindingState: OpenAgentsSiteMdkAccountBindingState,
  caveatRefs: S.Array(S.String),
  checkoutAuthorityCreated: S.Literal(false),
  customerRef: S.NullOr(S.String),
  environment: S.NullOr(OpenAgentsSiteMdkAccountEnvironment),
  liveSpendAuthorityCreated: S.Literal(false),
  orderRef: S.NullOr(S.String),
  payoutAuthorityCreated: S.Literal(false),
  providerMode: OpenAgentsSiteMdkAccountProviderMode,
  requestedProviderMode: S.NullOr(S.Literal('customer_owned_mdk')),
  reviewStatus: S.NullOr(OpenAgentsSiteMdkAccountReviewStatus),
  reviewerRefs: S.Array(S.String),
  secretBindingRefs: S.Array(S.String),
  secretBindingState: S.Literals([
    'hosted_secret_refs_present',
    'not_configured',
    'redacted',
  ]),
  settlementAuthorityCreated: S.Literal(false),
  siteId: S.String,
  siteVersionId: S.NullOr(S.String),
  walletMaterialExposed: S.Literal(false),
}) {}

export class OpenAgentsSiteMdkAccountBindingUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMdkAccountBindingUnsafe>()(
  'OpenAgentsSiteMdkAccountBindingUnsafe',
  {
    reason: S.String,
  },
) {}

export class OpenAgentsSiteMdkAccountBindingStorageError extends S.TaggedErrorClass<OpenAgentsSiteMdkAccountBindingStorageError>()(
  'OpenAgentsSiteMdkAccountBindingStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type OpenAgentsSiteMdkAccountBindingStore = Readonly<{
  listBindingsForSite: (
    siteId: string,
  ) => Promise<ReadonlyArray<OpenAgentsSiteMdkAccountBindingRecord>>
  readBindingByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<OpenAgentsSiteMdkAccountBindingRecord | undefined>
  upsertBinding: (
    binding: OpenAgentsSiteMdkAccountBindingRecord,
  ) => Promise<OpenAgentsSiteMdkAccountBindingRecord>
}>

type SiteMdkAccountBindingRow = Readonly<{
  allowed_action_refs_json: string
  allowed_catalog_refs_json: string
  allowed_product_refs_json: string
  archived_at: string | null
  binding_ref: string
  caveat_refs_json: string
  created_at: string
  customer_ref: string | null
  environment: 'production' | 'sandbox'
  id: string
  idempotency_key_hash: string
  order_ref: string | null
  public_projection_json: string
  requested_provider_mode: 'customer_owned_mdk'
  review_status: 'approved' | 'blocked' | 'pending_review' | 'revoked'
  reviewer_refs_json: string
  secret_binding_refs_json: string
  site_id: string
  site_version_id: string | null
  updated_at: string
}>

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const stableIdPattern = /^[A-Za-z][A-Za-z0-9_.:-]{0,180}$/
const hostedSecretRefPattern = /^hosted_secret\.[a-z0-9_.:/-]{1,240}$/u
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|value)|email[_-]?body|full[_-]?destination|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|wallet[_-]?(config|mnemonic|secret|state)|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?(config|mnemonic|secret|state))/i
const safeBindingKeyPattern =
  /^(checkoutAuthorityCreated|liveSpendAuthorityCreated|payoutAuthorityCreated|providerMode|requestedProviderMode|secretBindingRefs|secretBindingState|settlementAuthorityCreated|walletMaterialExposed)$/u
const safeBindingValuePattern =
  /^(hosted_secret_refs_present|not_configured|redacted)$/u

const valueHasUnsafeMaterial = (
  value: unknown,
  options: Readonly<{ rejectRawTimestamps: boolean }> = {
    rejectRawTimestamps: true,
  },
): boolean => {
  if (typeof value === 'string') {
    if (
      hostedSecretRefPattern.test(value) ||
      safeBindingValuePattern.test(value)
    ) {
      return false
    }

    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      (options.rejectRawTimestamps && rawTimestampPattern.test(value)) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(item => valueHasUnsafeMaterial(item, options))
  }

  if (value !== null && typeof value === 'object') {
    return Object.entries(value).some(([key, item]) =>
      (typeof item !== 'boolean' &&
        !safeBindingKeyPattern.test(key) &&
        unsafeKeyPattern.test(key)) ||
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

const nullableRefIsSafe = (value: string | null): boolean =>
  value === null || refIsSafe(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(refIsSafe)

const jsonStringArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify(safeRefs(values))

const bindingStateForStatus = (
  status: OpenAgentsSiteMdkAccountReviewStatus,
): OpenAgentsSiteMdkAccountBindingState =>
  status === 'approved'
    ? 'configured'
    : status === 'pending_review'
      ? 'pending_review'
      : status

const storageError = (
  operation: string,
  error: unknown,
): OpenAgentsSiteMdkAccountBindingStorageError =>
  new OpenAgentsSiteMdkAccountBindingStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const bindingFromRow = (
  row: SiteMdkAccountBindingRow,
): OpenAgentsSiteMdkAccountBindingRecord => ({
  allowedActionRefs: [...parseJsonStringArray(row.allowed_action_refs_json)],
  allowedCatalogRefs: [...parseJsonStringArray(row.allowed_catalog_refs_json)],
  allowedProductRefs: [...parseJsonStringArray(row.allowed_product_refs_json)],
  archivedAt: row.archived_at,
  bindingRef: row.binding_ref,
  caveatRefs: [...parseJsonStringArray(row.caveat_refs_json)],
  createdAt: row.created_at,
  customerRef: row.customer_ref,
  environment: row.environment,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  orderRef: row.order_ref,
  publicProjectionJson: row.public_projection_json,
  requestedProviderMode: row.requested_provider_mode,
  reviewStatus: row.review_status,
  reviewerRefs: [...parseJsonStringArray(row.reviewer_refs_json)],
  secretBindingRefs: [...parseJsonStringArray(row.secret_binding_refs_json)],
  siteId: row.site_id,
  siteVersionId: row.site_version_id,
  updatedAt: row.updated_at,
})

export const assertOpenAgentsSiteMdkAccountBindingSafe = (
  binding: OpenAgentsSiteMdkAccountBindingRecord,
): void => {
  S.decodeUnknownSync(OpenAgentsSiteMdkAccountBindingRecord)(binding)

  if (
    valueHasUnsafeMaterial(binding, { rejectRawTimestamps: false }) ||
    !idIsSafe(binding.id) ||
    !refIsSafe(binding.bindingRef) ||
    !refIsSafe(binding.idempotencyKeyHash) ||
    !refIsSafe(binding.siteId) ||
    !nullableRefIsSafe(binding.siteVersionId) ||
    !nullableRefIsSafe(binding.customerRef) ||
    !nullableRefIsSafe(binding.orderRef) ||
    binding.secretBindingRefs.some(ref => !refIsSafe(ref)) ||
    binding.allowedCatalogRefs.some(ref => !refIsSafe(ref)) ||
    binding.allowedActionRefs.some(ref => !refIsSafe(ref)) ||
    binding.allowedProductRefs.some(ref => !refIsSafe(ref)) ||
    binding.reviewerRefs.some(ref => !refIsSafe(ref)) ||
    binding.caveatRefs.some(ref => !refIsSafe(ref))
  ) {
    throw new OpenAgentsSiteMdkAccountBindingUnsafe({
      reason:
        'Site MDK account bindings must use stable hosted-secret refs and must not contain MDK tokens, mnemonics, webhook secrets, raw invoices, payment hashes, preimages, wallet material, customer private values, provider grants, or secrets.',
    })
  }
}

const projectionForBinding = (
  binding: OpenAgentsSiteMdkAccountBindingRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): OpenAgentsSiteMdkAccountBindingProjection =>
  new OpenAgentsSiteMdkAccountBindingProjection({
    allowedActionRefs: safeRefs(binding.allowedActionRefs),
    allowedCatalogRefs: safeRefs(binding.allowedCatalogRefs),
    allowedProductRefs: safeRefs(binding.allowedProductRefs),
    audience,
    bindingRef: refIsSafe(binding.bindingRef) ? binding.bindingRef : null,
    bindingState: bindingStateForStatus(binding.reviewStatus),
    caveatRefs: safeRefs(binding.caveatRefs),
    checkoutAuthorityCreated: false,
    customerRef: audience === 'operator' ? binding.customerRef : null,
    environment: binding.environment,
    liveSpendAuthorityCreated: false,
    orderRef: audience === 'operator' ? binding.orderRef : null,
    payoutAuthorityCreated: false,
    providerMode:
      binding.reviewStatus === 'approved'
        ? 'customer_owned_mdk'
        : 'openagents_hosted_mdk',
    requestedProviderMode: binding.requestedProviderMode,
    reviewStatus: binding.reviewStatus,
    reviewerRefs: audience === 'operator'
      ? safeRefs(binding.reviewerRefs)
      : [],
    secretBindingRefs: audience === 'operator'
      ? safeRefs(binding.secretBindingRefs)
      : [],
    secretBindingState: binding.secretBindingRefs.length === 0
      ? 'not_configured'
      : audience === 'operator'
        ? 'hosted_secret_refs_present'
        : 'redacted',
    settlementAuthorityCreated: false,
    siteId: binding.siteId,
    siteVersionId: binding.siteVersionId,
    walletMaterialExposed: false,
  })

export const projectOpenAgentsSiteMdkAccountBinding = (
  input: Readonly<{
    audience: typeof OpenAgentsPaymentPolicyAudience.Type
    binding: OpenAgentsSiteMdkAccountBindingRecord | null
    siteId: string
  }>,
): OpenAgentsSiteMdkAccountBindingProjection => {
  const projection = input.binding === null
    ? new OpenAgentsSiteMdkAccountBindingProjection({
      allowedActionRefs: [],
      allowedCatalogRefs: [],
      allowedProductRefs: [],
      audience: input.audience,
      bindingRef: null,
      bindingState: 'unavailable',
      caveatRefs: ['caveat.site_mdk_account.customer_owned_mode_not_configured'],
      checkoutAuthorityCreated: false,
      customerRef: null,
      environment: null,
      liveSpendAuthorityCreated: false,
      orderRef: null,
      payoutAuthorityCreated: false,
      providerMode: 'openagents_hosted_mdk',
      requestedProviderMode: null,
      reviewStatus: null,
      reviewerRefs: [],
      secretBindingRefs: [],
      secretBindingState: 'not_configured',
      settlementAuthorityCreated: false,
      siteId: input.siteId,
      siteVersionId: null,
      walletMaterialExposed: false,
    })
    : projectionForBinding(input.binding, input.audience)

  if (
    valueHasUnsafeMaterial(projection, { rejectRawTimestamps: true }) ||
    !refIsSafe(projection.siteId) ||
    !nullableRefIsSafe(projection.siteVersionId) ||
    (projection.bindingRef !== null && !refIsSafe(projection.bindingRef))
  ) {
    throw new OpenAgentsSiteMdkAccountBindingUnsafe({
      reason:
        'Site MDK account binding projection must not expose private customer, payment, provider, wallet, raw checkout, timestamp, or secret material.',
    })
  }

  return projection
}

export const siteMdkAccountBindingPublicJson = (
  binding: OpenAgentsSiteMdkAccountBindingRecord,
): string =>
  JSON.stringify(projectOpenAgentsSiteMdkAccountBinding({
    audience: 'customer',
    binding,
    siteId: binding.siteId,
  }))

export const mdkAccountBindingAppliesToCatalogItem = (
  binding: OpenAgentsSiteMdkAccountBindingRecord,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): boolean =>
  binding.reviewStatus === 'approved' &&
  binding.archivedAt === null &&
  binding.siteId === catalogItem.siteId &&
  (binding.siteVersionId === null ||
    binding.siteVersionId === catalogItem.siteVersionId) &&
  (
    binding.allowedCatalogRefs.length === 0 ||
    binding.allowedCatalogRefs.includes(catalogItem.catalogRef)
  ) &&
  (
    catalogItem.itemKind === 'product'
      ? binding.allowedProductRefs.length === 0 ||
        binding.allowedProductRefs.includes(catalogItem.productId)
      : binding.allowedActionRefs.length === 0 ||
        binding.allowedActionRefs.includes(catalogItem.actionId)
  )

export const currentMdkAccountBindingForCatalogItem = (
  bindings: ReadonlyArray<OpenAgentsSiteMdkAccountBindingRecord>,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsSiteMdkAccountBindingRecord | null =>
  bindings.find(binding =>
    mdkAccountBindingAppliesToCatalogItem(binding, catalogItem),
  ) ?? null

const bindInsert = (
  db: D1Database,
  binding: OpenAgentsSiteMdkAccountBindingRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO site_mdk_account_bindings
       (id, binding_ref, idempotency_key_hash, site_id, site_version_id,
        customer_ref, order_ref, requested_provider_mode, environment,
        review_status, secret_binding_refs_json, allowed_catalog_refs_json,
        allowed_product_refs_json, allowed_action_refs_json, reviewer_refs_json,
        caveat_refs_json, public_projection_json, created_at, updated_at,
        archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(site_id, binding_ref)
       DO UPDATE SET
         idempotency_key_hash = excluded.idempotency_key_hash,
         site_version_id = excluded.site_version_id,
         customer_ref = excluded.customer_ref,
         order_ref = excluded.order_ref,
         requested_provider_mode = excluded.requested_provider_mode,
         environment = excluded.environment,
         review_status = excluded.review_status,
         secret_binding_refs_json = excluded.secret_binding_refs_json,
         allowed_catalog_refs_json = excluded.allowed_catalog_refs_json,
         allowed_product_refs_json = excluded.allowed_product_refs_json,
         allowed_action_refs_json = excluded.allowed_action_refs_json,
         reviewer_refs_json = excluded.reviewer_refs_json,
         caveat_refs_json = excluded.caveat_refs_json,
         public_projection_json = excluded.public_projection_json,
         updated_at = excluded.updated_at,
         archived_at = excluded.archived_at`,
    )
    .bind(
      binding.id,
      binding.bindingRef,
      binding.idempotencyKeyHash,
      binding.siteId,
      binding.siteVersionId,
      binding.customerRef,
      binding.orderRef,
      binding.requestedProviderMode,
      binding.environment,
      binding.reviewStatus,
      jsonStringArray(binding.secretBindingRefs),
      jsonStringArray(binding.allowedCatalogRefs),
      jsonStringArray(binding.allowedProductRefs),
      jsonStringArray(binding.allowedActionRefs),
      jsonStringArray(binding.reviewerRefs),
      jsonStringArray(binding.caveatRefs),
      binding.publicProjectionJson,
      binding.createdAt,
      binding.updatedAt,
      binding.archivedAt,
    )

export const makeD1SiteMdkAccountBindingStore = (
  db: D1Database,
): OpenAgentsSiteMdkAccountBindingStore => ({
  listBindingsForSite: async siteId => {
    try {
      const rows = await db
        .prepare(
          `SELECT *
             FROM site_mdk_account_bindings
            WHERE site_id = ?
              AND archived_at IS NULL
            ORDER BY updated_at DESC`,
        )
        .bind(siteId)
        .all<SiteMdkAccountBindingRow>()

      return rows.results.map(bindingFromRow)
    } catch (error) {
      throw storageError('siteMdkAccountBindings.listBindingsForSite', error)
    }
  },
  readBindingByIdempotencyKeyHash: async idempotencyKeyHash => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_mdk_account_bindings
            WHERE idempotency_key_hash = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKeyHash)
        .first<SiteMdkAccountBindingRow>()

      return row === null ? undefined : bindingFromRow(row)
    } catch (error) {
      throw storageError(
        'siteMdkAccountBindings.readBindingByIdempotency',
        error,
      )
    }
  },
  upsertBinding: async binding => {
    assertOpenAgentsSiteMdkAccountBindingSafe(binding)

    try {
      await bindInsert(db, binding).run()

      const row = await db
        .prepare(
          `SELECT *
             FROM site_mdk_account_bindings
            WHERE site_id = ?
              AND binding_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(binding.siteId, binding.bindingRef)
        .first<SiteMdkAccountBindingRow>()

      return row === null ? binding : bindingFromRow(row)
    } catch (error) {
      throw storageError('siteMdkAccountBindings.upsertBinding', error)
    }
  },
})
