import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  type BuyerPaymentEntitlementRecord,
  type BuyerPaymentLedgerStore,
  type BuyerPaymentReceiptRecord,
  type BuyerPaymentReconciliationEventRecord,
  BuyerPaymentChallengeRecord,
  type BuyerPaymentLedgerAmount,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  type OpenAgentsHostedMdkClient,
  OpenAgentsHostedMdkClientError,
  type OpenAgentsHostedMdkCheckoutProjection,
  type OpenAgentsHostedMdkCheckoutStatus,
  buildOpenAgentsHostedMdkCheckoutRequest,
  makeFakeOpenAgentsHostedMdkClient,
  projectOpenAgentsHostedMdkCheckoutResponse,
} from './hosted-mdk-client'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import {
  type NexusTreasuryPayoutLedgerStore,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import {
  projectPylonV02OmegaReleaseGate,
  readyPylonV02OmegaReleaseGateRecord,
} from './pylon-v02-omega-release-gate'
import {
  SiteCommerceAsset,
  SiteCommerceEntitlementScope,
  type SiteCommercePrice,
} from './site-commerce'
import {
  type OpenAgentsSiteCommerceReviewDecisionRecord,
  OpenAgentsSiteCommerceReviewDecisionRequest,
  type OpenAgentsSiteCommerceReviewStore,
  OpenAgentsSiteCommerceReviewUnsafe,
  projectOpenAgentsSiteCommerceReview,
  projectOpenAgentsSiteCommerceReviewDecision,
  siteCommerceReviewDecisionPublicJson,
} from './site-commerce-review'
import {
  OpenAgentsSiteCheckoutUiPrimitiveUnsafe,
  siteCheckoutUiPrimitivesFromCatalog,
} from './site-checkout-ui-primitives'
import {
  projectOpenAgentsSiteCheckoutReturn,
} from './site-checkout-return'
import {
  currentMdkAccountBindingForCatalogItem,
  type OpenAgentsSiteMdkAccountBindingRecord,
  OpenAgentsSiteMdkAccountBindingRequest,
  type OpenAgentsSiteMdkAccountBindingStore,
  OpenAgentsSiteMdkAccountBindingUnsafe,
  projectOpenAgentsSiteMdkAccountBinding,
  siteMdkAccountBindingPublicJson,
} from './site-mdk-account-bindings'
import {
  type OpenAgentsSiteMdkCheckoutIntentStore,
  type OpenAgentsSiteMdkCheckoutIntentRecord,
  projectSiteMdkCheckoutIntentPublicJson,
} from './site-mdk-checkout-intents'
import {
  type OpenAgentsSiteMdkProviderEvent,
  projectOpenAgentsSiteMdkReconciliation,
} from './site-mdk-reconciliation'
import {
  type OpenAgentsSiteMdkWebhookConfig,
  verifyOpenAgentsSiteMdkWebhook,
} from './site-mdk-webhooks'
import {
  OpenAgentsSitePaymentCatalog,
  OpenAgentsSitePaymentCatalogRecord,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  openAgentsSitePaymentCatalogHasPrivateMaterial,
  projectOpenAgentsSitePaymentCatalog,
} from './site-payment-catalog'
import { projectOpenAgentsSitePaymentDiscovery } from './site-payment-discovery'
import {
  OpenAgentsSitePaymentProofUnsafe,
  projectOpenAgentsSitePaymentProof,
} from './site-payment-proof'
import {
  OpenAgentsSitePaymentToPayoutBridgeRequest,
  buildOpenAgentsSitePaymentToPayoutBridge,
} from './site-payment-to-payout-bridge'

type HttpResponse = globalThis.Response

type SiteCommerceRoutesDependencies = Readonly<{
  authorizeCommerceReviewDecision?: (request: Request) => Promise<boolean>
  authorizePaidActionAgent?: (request: Request) => Promise<boolean>
  authorizeMdkAccountBinding?: (request: Request) => Promise<boolean>
  authorizePayoutBridge?: (request: Request) => Promise<boolean>
  buyerPaymentLedgerStore?: BuyerPaymentLedgerStore
  challengeExpiresAt: () => string
  checkoutCatalog?: OpenAgentsSitePaymentCatalog
  checkoutIntentStore?: OpenAgentsSiteMdkCheckoutIntentStore
  hostedMdkClient?: OpenAgentsHostedMdkClient
  mdkWebhookConfig?: OpenAgentsSiteMdkWebhookConfig | undefined
  mdkAccountBindingStore?: OpenAgentsSiteMdkAccountBindingStore
  nowEpochMillis: () => number
  nowIso: () => string
  payoutLedgerStore?: NexusTreasuryPayoutLedgerStore
  reviewStore?: OpenAgentsSiteCommerceReviewStore
}>

export class SiteCheckoutIntentRequest extends S.Class<SiteCheckoutIntentRequest>(
  'SiteCheckoutIntentRequest',
)({
  actionId: S.optionalKey(S.String),
  cancelReturnPath: S.String,
  catalogRef: S.optionalKey(S.String),
  customerDataRefs: S.optionalKey(S.Array(S.String)),
  expectedPrice: S.optionalKey(BuyerPaymentChallengeRecord.fields.price),
  itemKind: S.Literals(['paid_action', 'product']),
  productId: S.optionalKey(S.String),
  siteVersionId: S.String,
  successReturnPath: S.String,
}) {}

export class SiteL402ChallengeRequest extends S.Class<SiteL402ChallengeRequest>(
  'SiteL402ChallengeRequest',
)({
  entitlementScope: SiteCommerceEntitlementScope,
  method: S.Literals(['GET', 'POST']),
  paidActionId: S.String,
  path: S.String,
  price: S.Struct({
    amount: S.Number,
    asset: SiteCommerceAsset,
  }),
  spendCap: S.Struct({
    amount: S.Number,
    asset: SiteCommerceAsset,
  }),
}) {}

export class SiteL402RedemptionRequest extends S.Class<SiteL402RedemptionRequest>(
  'SiteL402RedemptionRequest',
)({
  challengeExpiresAt: S.String,
  challengeId: S.String,
  credentialId: S.String,
  entitlementScope: SiteCommerceEntitlementScope,
  method: S.Literals(['GET', 'POST']),
  paidActionId: S.String,
  path: S.String,
  paymentProofRef: S.String,
  price: S.Struct({
    amount: S.Number,
    asset: SiteCommerceAsset,
  }),
}) {}

type CommerceAction =
  | 'commerce_review_decision_create'
  | 'commerce_review_read'
  | 'checkout_intent_create'
  | 'checkout_return_read'
  | 'l402_challenge_create'
  | 'l402_redemption_accept'
  | 'mdk_account_binding_read'
  | 'mdk_account_binding_upsert'
  | 'mdk_webhook_reconcile'
  | 'payout_bridge_create'
  | 'payment_discovery_read'
  | 'payment_proof_read'

const SITE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,160}$/
const CATALOG_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/
const PAYMENT_PROOF_REF_PATTERN = /^mdk_payment_proof_[A-Za-z0-9_-]{8,160}$/
const CLEAN_REF_SEGMENT_PATTERN = /[^A-Za-z0-9_-]+/g
const PROHIBITED_RESPONSE_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key)/i

const emptyCheckoutCatalog: OpenAgentsSitePaymentCatalog = { items: [] }

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const idempotencyKey = request.headers.get('idempotency-key')?.trim()

  return idempotencyKey === '' ? undefined : idempotencyKey
}

const cleanRefSegment = (value: string): string =>
  value.replace(CLEAN_REF_SEGMENT_PATTERN, '_').slice(0, 96)

const opaqueRef = (
  prefix: string,
  siteId: string,
  idempotencyKey: string,
): string =>
  `${prefix}_${cleanRefSegment(siteId)}_${cleanRefSegment(idempotencyKey)}`

const isValidPrice = (price: SiteCommercePrice): boolean =>
  Number.isFinite(price.amount) && price.amount > 0

const priceWithinSpendCap = (
  price: SiteCommercePrice,
  spendCap: SiteCommercePrice,
): boolean => price.asset === spendCap.asset && price.amount <= spendCap.amount

const validateCatalogId = (value: string, field: string): string | undefined =>
  CATALOG_ID_PATTERN.test(value)
    ? undefined
    : `${field} must be a stable catalog id.`

const validateLocalPath = (
  value: string,
  field: string,
): string | undefined => {
  if (!value.startsWith('/')) {
    return `${field} must be a site-local absolute path.`
  }

  if (value.includes('?') || value.includes('#')) {
    return `${field} must not include query strings or fragments.`
  }

  return undefined
}

const validateNoSecretMaterial = (
  value: unknown,
  field: string,
): string | undefined =>
  containsProviderSecretMaterial(JSON.stringify(value)) ||
  PROHIBITED_RESPONSE_PATTERN.test(JSON.stringify(value))
    ? `${field} must not contain payment secrets, wallet material, raw invoices, or preimages.`
    : undefined

const amountEquals = (
  left: BuyerPaymentLedgerAmount,
  right: BuyerPaymentLedgerAmount,
): boolean =>
  left.amountMinorUnits === right.amountMinorUnits &&
  left.asset === right.asset &&
  left.denomination === right.denomination

const returnRefForPath = (
  prefix: string,
  siteId: string,
  path: string,
): string =>
  [
    prefix,
    'site_checkout',
    cleanRefSegment(siteId),
    cleanRefSegment(path.replace(/^\//u, '')),
  ].join('.')

const idempotencyHashRef = (siteId: string, idempotencyKey: string): string =>
  `hash.site_checkout.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`

const requestBodyDigestRef = (siteId: string, idempotencyKey: string): string =>
  `sha256:site_checkout:${cleanRefSegment(siteId)}:${cleanRefSegment(idempotencyKey)}`

const checkoutEventSuffix = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
): string =>
  cleanRefSegment(`${intent.siteId}_${intent.checkoutIntentRef}`)

const checkoutReceiptRef = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
): string =>
  `receipt.site_payment.${checkoutEventSuffix(intent)}`

const checkoutEntitlementRef = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
): string =>
  `entitlement.site_payment.${checkoutEventSuffix(intent)}`

const checkoutRedactedPaymentRef = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
): string =>
  `redacted_payment.site_payment.${checkoutEventSuffix(intent)}`

const hostedCheckoutProjectionFromIntent = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
  status: OpenAgentsHostedMdkCheckoutStatus = intent.status,
): OpenAgentsHostedMdkCheckoutProjection => ({
  acceptedWorkSettlementAuthority: false,
  amount: intent.amount,
  audience: 'agent',
  challengeRef: intent.challengeRef,
  ...(intent.checkoutLaunchPath === null
    ? {}
    : { checkoutLaunchPath: intent.checkoutLaunchPath }),
  checkoutRef: intent.checkoutRef,
  checkoutUrlRef: intent.checkoutUrlRef,
  environment: intent.environment,
  invoiceRef: null,
  paymentHashRef: null,
  productId: intent.productId,
  provider: 'mdk_hosted',
  providerPayoutAuthority: false,
  providerRef: intent.providerRef,
  sandbox: intent.sandbox,
  settlementAuthority: 'buyer_payment_evidence_only',
  siteRef: intent.siteId,
  status,
})

const publicCheckoutIntentJson = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
  hostedCheckout: OpenAgentsHostedMdkCheckoutProjection,
): string =>
  projectSiteMdkCheckoutIntentPublicJson({
    checkoutIntent: {
      ...intent,
      status: hostedCheckout.status,
    },
    hostedCheckout,
  })

const scopeRefsForCheckoutIntent = (
  dependencies: SiteCommerceRoutesDependencies,
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
): ReadonlyArray<string> => {
  const catalogItem = checkoutCatalogForDependencies(dependencies).items.find(
    item => item.catalogRef === intent.catalogRef,
  )

  return [
    `entitlement.site_payment.${cleanRefSegment(intent.siteId)}.${cleanRefSegment(intent.siteVersionId)}.${cleanRefSegment(intent.productId)}`,
    ...(catalogItem === undefined
      ? []
      : [
          `entitlement_scope.site_payment.${catalogItem.entitlementScope}`,
          ...catalogItem.metadataRefs,
        ]),
  ]
}

const receiptForCheckoutIntent = (
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
  challenge: BuyerPaymentChallengeRecord,
  occurredAt: string,
): BuyerPaymentReceiptRecord => ({
  actorRef: challenge.actorRef,
  amount: intent.amount,
  archivedAt: null,
  challengeRef: intent.challengeRef,
  createdAt: occurredAt,
  entitlementRef: checkoutEntitlementRef(intent),
  id: checkoutReceiptRef(intent),
  metadataRefs: [
    ...intent.metadataRefs,
    `metadata.site_mdk_receipt.${cleanRefSegment(intent.siteId)}`,
  ],
  ownerUserId: challenge.ownerUserId,
  productId: intent.productId,
  publicProjectionJson: JSON.stringify({
    checkoutIntentRef: intent.checkoutIntentRef,
    checkoutStatus: 'payment_received',
    siteId: intent.siteId,
  }),
  receiptRef: checkoutReceiptRef(intent),
  redactedPaymentRef: checkoutRedactedPaymentRef(intent),
  status: 'issued',
  surface: 'site_checkout',
})

const entitlementForCheckoutIntent = (
  dependencies: SiteCommerceRoutesDependencies,
  intent: OpenAgentsSiteMdkCheckoutIntentRecord,
  challenge: BuyerPaymentChallengeRecord,
  receipt: BuyerPaymentReceiptRecord,
  occurredAt: string,
): BuyerPaymentEntitlementRecord => ({
  actorRef: challenge.actorRef,
  archivedAt: null,
  challengeRef: intent.challengeRef,
  consumedAt: null,
  createdAt: occurredAt,
  entitlementRef: receipt.entitlementRef,
  expiresAt: null,
  id: receipt.entitlementRef,
  ownerUserId: challenge.ownerUserId,
  productId: intent.productId,
  receiptRef: receipt.receiptRef,
  scopeRefs: scopeRefsForCheckoutIntent(dependencies, intent),
  status: 'active',
  surface: 'site_checkout',
})

const checkoutCatalogForDependencies = (
  dependencies: SiteCommerceRoutesDependencies,
): OpenAgentsSitePaymentCatalog =>
  dependencies.checkoutCatalog ?? emptyCheckoutCatalog

const hostedMdkClientForDependencies = (
  dependencies: SiteCommerceRoutesDependencies,
): OpenAgentsHostedMdkClient =>
  dependencies.hostedMdkClient ??
  makeFakeOpenAgentsHostedMdkClient(
    {
      configRef: 'config.openagents.hosted_mdk.fake',
      credentialBindingRef: 'credential_binding.openagents.hosted_mdk.fake',
      environment: 'sandbox',
      providerRef: 'provider.openagents.hosted_mdk.fake',
      webhookBindingRef: null,
    },
    {
      nowIso: dependencies.nowIso(),
    },
  )

const hostedMdkModeForCatalogItem = (
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
) => (catalogItem.settlementMode === 'checkout_only' ? 'amount' : 'product')

const invalidContractResponse = (message: string): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'invalid_site_commerce_contract',
      message,
    },
    { status: 400 },
  )

const idempotencyRequiredResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'idempotency_key_required',
      message: 'Site commerce actions require Idempotency-Key.',
    },
    { status: 400 },
  )

const staleChallengeResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'l402_challenge_stale',
      message: 'The L402 challenge is stale. Request a fresh challenge.',
    },
    { status: 409 },
  )

const siteCommerceUnavailableResponse = (message: string): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'site_commerce_unavailable',
      message,
    },
    { status: 503 },
  )

const checkoutIntentNotFoundResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'checkout_intent_not_found',
      message: 'The checkout intent could not be found.',
    },
    { status: 404 },
  )

const hostedCheckoutUnavailableResponse = (
  error: OpenAgentsHostedMdkClientError,
): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'hosted_checkout_unavailable',
      message:
        'Hosted checkout is not available for this Site payment contract.',
      reason: error.reason,
    },
    { status: 503 },
  )

const payoutBridgeUnauthorizedResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'unauthorized',
      message: 'Site payment-to-payout bridge actions require operator authority.',
    },
    { status: 401 },
  )

const commerceReviewUnauthorizedResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'unauthorized',
      message: 'Site commerce review decisions require operator authority.',
    },
    { status: 401 },
  )

const mdkAccountBindingUnauthorizedResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'unauthorized',
      message: 'Site MDK account binding actions require operator authority.',
    },
    { status: 401 },
  )

const paidActionAgentUnauthorizedResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'unauthorized',
      message:
        'Generated Site paid actions require an active registered agent bearer token.',
    },
    { status: 401 },
  )

const normalizeHostedCheckoutError = (
  error: unknown,
): OpenAgentsHostedMdkClientError =>
  error instanceof OpenAgentsHostedMdkClientError
    ? error
    : new OpenAgentsHostedMdkClientError({
        detailRef: 'detail.mdk_hosted.unexpected_failure',
        reason: 'provider_unavailable',
      })

const redactedHeaders = (challengeRef: string, expiresAt: string): Headers => {
  const headers = new Headers()
  headers.set(
    'www-authenticate',
    `L402 realm="OpenAgents Sites", challenge_ref="${challengeRef}", invoice_ref="redacted", expires_at="${expiresAt}"`,
  )

  return headers
}

const createCheckoutIntentResponse = async (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
  idempotencyKey: string,
  input: SiteCheckoutIntentRequest,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): Promise<HttpResponse> => {
  const checkoutIntentId = opaqueRef(
    'site_checkout_intent',
    siteId,
    idempotencyKey,
  )
  const challengeExpiresAt = dependencies.challengeExpiresAt()
  const createdAt = dependencies.nowIso()
  const paidEndpointProduct =
    openAgentsPaidEndpointProductFromSitePaymentCatalogItem(catalogItem)
  const challenge: BuyerPaymentChallengeRecord = {
    actorRef: `site.${cleanRefSegment(siteId)}.checkout`,
    archivedAt: null,
    challengeRef: `challenge.site_checkout.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`,
    createdAt,
    expiresAt: challengeExpiresAt,
    id: `buyer_payment_challenge_${cleanRefSegment(siteId)}_${cleanRefSegment(idempotencyKey)}`,
    idempotencyKeyHash: idempotencyHashRef(siteId, idempotencyKey),
    metadataRefs: [
      ...catalogItem.metadataRefs,
      `metadata.site_checkout_intent.${cleanRefSegment(siteId)}`,
    ],
    method: 'POST',
    ownerUserId: null,
    path: catalogItem.checkoutPath,
    price: catalogItem.price,
    productId: paidEndpointProduct.productId,
    publicProjectionJson: JSON.stringify({
      catalogRef: catalogItem.catalogRef,
      checkoutIntentId,
      itemKind: catalogItem.itemKind,
      siteId,
      siteVersionId: catalogItem.siteVersionId,
    }),
    requestBodyDigest: requestBodyDigestRef(siteId, idempotencyKey),
    spendCap: catalogItem.price,
    status: 'issued',
    surface: 'site_checkout',
  }

  const hostedMdkClient = hostedMdkClientForDependencies(dependencies)
  const hostedRequest = buildOpenAgentsHostedMdkCheckoutRequest({
    cancelRef: returnRefForPath('cancel', siteId, input.cancelReturnPath),
    challenge,
    customerDataRefs: input.customerDataRefs ?? [],
    environment: catalogItem.sandbox ? 'sandbox' : 'production',
    l402Payload: null,
    metadataRefs: catalogItem.metadataRefs,
    mode: hostedMdkModeForCatalogItem(catalogItem),
    product: paidEndpointProduct,
    returnRef: returnRefForPath('return', siteId, input.successReturnPath),
    sandbox: catalogItem.sandbox,
    siteRef: catalogItem.siteId,
  })

  if (hostedRequest instanceof OpenAgentsHostedMdkClientError) {
    return hostedCheckoutUnavailableResponse(hostedRequest)
  }

  let hostedResponse

  try {
    hostedResponse = await hostedMdkClient.createCheckoutPromise(hostedRequest)
  } catch (error) {
    return hostedCheckoutUnavailableResponse(
      normalizeHostedCheckoutError(error),
    )
  }

  const hostedProjection = projectOpenAgentsHostedMdkCheckoutResponse(
    hostedResponse,
    'public',
  )
  const catalogProjection = projectOpenAgentsSitePaymentCatalog(
    { items: [catalogItem] },
    'customer',
  )
  let customerOwnedMdkBinding: OpenAgentsSiteMdkAccountBindingRecord | null

  try {
    customerOwnedMdkBinding = await currentMdkAccountBindingForCheckout(
      dependencies,
      catalogItem,
    )
  } catch {
    return hostedCheckoutUnavailableResponse(
      new OpenAgentsHostedMdkClientError({
        detailRef: 'detail.mdk_hosted.account_binding_store_unavailable',
        reason: 'provider_unavailable',
      }),
    )
  }

  const mdkAccountBindingProjection =
    projectOpenAgentsSiteMdkAccountBinding({
      audience: 'customer',
      binding: customerOwnedMdkBinding,
      siteId,
    })
  const buyerPaymentChallenge = projectBuyerPaymentLedgerRecord(
    'challenge',
    challenge,
    'public',
  )
  const checkoutIntentRecord = {
    amount: hostedResponse.amount,
    archivedAt: null,
    cancelReturnPath: input.cancelReturnPath,
    catalogRef: catalogItem.catalogRef,
    challengeRef: challenge.challengeRef,
    checkoutIntentRef: checkoutIntentId,
    checkoutLaunchPath: hostedResponse.checkoutLaunchPath ?? null,
    checkoutRef: hostedResponse.checkoutRef,
    checkoutUrlRef: hostedResponse.checkoutUrlRef,
    createdAt,
    environment: hostedResponse.environment,
    hostedCheckoutProjectionJson: JSON.stringify(hostedProjection),
    id: checkoutIntentId,
    idempotencyKeyHash: challenge.idempotencyKeyHash,
    metadataRefs: hostedResponse.metadataRefs,
    productId: paidEndpointProduct.productId,
    providerRef: hostedResponse.providerRef,
    publicProjectionJson: '{}',
    sandbox: hostedResponse.sandbox,
    siteId,
    siteVersionId: catalogItem.siteVersionId,
    status: hostedResponse.status,
    successReturnPath: input.successReturnPath,
    updatedAt: createdAt,
  }
  const checkoutIntentPublicProjectionJson =
    projectSiteMdkCheckoutIntentPublicJson({
      checkoutIntent: checkoutIntentRecord,
      hostedCheckout: hostedProjection,
    })

  if (dependencies.checkoutIntentStore !== undefined) {
    try {
      await dependencies.checkoutIntentStore.createCheckoutIntentBundle({
        buyerPaymentChallenge: challenge,
        checkoutIntent: {
          ...checkoutIntentRecord,
          publicProjectionJson: checkoutIntentPublicProjectionJson,
        },
      })
    } catch {
      return hostedCheckoutUnavailableResponse(
        new OpenAgentsHostedMdkClientError({
          detailRef: 'detail.mdk_hosted.checkout_intent_store_unavailable',
          reason: 'provider_unavailable',
        }),
      )
    }
  }

  return noStoreJsonResponse(
    {
      siteCommerce: {
        action: 'checkout_intent_create' satisfies CommerceAction,
        buyerPaymentChallenge,
        checkoutIntent: {
          cancelReturnPath: input.cancelReturnPath,
          catalogItem: catalogProjection.items[0] ?? null,
          checkoutLaunchPath: hostedProjection.checkoutLaunchPath ?? null,
          checkoutUrlRef: hostedProjection.checkoutUrlRef,
          entitlement: {
            scope: catalogItem.entitlementScope,
            state: 'pending_checkout',
          },
          hostedCheckout: hostedProjection,
          id: checkoutIntentId,
          itemKind: catalogItem.itemKind,
          mdkAccountBinding: mdkAccountBindingProjection,
          productId: paidEndpointProduct.productId,
          provider: 'openagents_hosted_mdk',
          providerMode: mdkAccountBindingProjection.providerMode,
          publicProjectionState: catalogItem.publicProjectionState,
          sandbox: catalogItem.sandbox,
          state: 'created',
          successReturnPath: input.successReturnPath,
        },
        implementationState: hostedMdkClient.implementationState,
        idempotency: {
          key: idempotencyKey,
          replaySafe: true,
        },
        redaction: {
          exposesCustomerPrivateData: false,
          exposesMdkCredentials: false,
          exposesProviderGrants: false,
          exposesProviderPayoutClaims: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    },
    { status: 201 },
  )
}

const createL402ChallengeResponse = (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
  idempotencyKey: string,
  input: SiteL402ChallengeRequest,
): HttpResponse => {
  const challengeId = opaqueRef('site_l402_challenge', siteId, idempotencyKey)
  const expiresAt = dependencies.challengeExpiresAt()

  return noStoreJsonResponse(
    {
      siteCommerce: {
        action: 'l402_challenge_create' satisfies CommerceAction,
        implementationState: 'hosted_contract_stub',
        l402: {
          challengeId,
          credential: {
            format: 'l402',
            invoiceRef: 'redacted',
            paymentHashRef: 'redacted',
          },
          entitlement: {
            scope: input.entitlementScope,
            state: 'payment_required',
          },
          expiresAt,
          method: input.method,
          paidActionId: input.paidActionId,
          path: input.path,
          price: input.price,
          spendCap: input.spendCap,
        },
        redaction: {
          exposesMdkCredentials: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    },
    { headers: redactedHeaders(challengeId, expiresAt), status: 402 },
  )
}

const createL402RedemptionResponse = (
  siteId: string,
  idempotencyKey: string,
  input: SiteL402RedemptionRequest,
): HttpResponse => {
  const redemptionId = opaqueRef('site_l402_redemption', siteId, idempotencyKey)

  return noStoreJsonResponse(
    {
      siteCommerce: {
        action: 'l402_redemption_accept' satisfies CommerceAction,
        entitlement: {
          scope: input.entitlementScope,
          state: 'granted_stub',
        },
        implementationState: 'hosted_contract_stub',
        l402: {
          challengeId: input.challengeId,
          credentialId: input.credentialId,
          paidActionId: input.paidActionId,
          path: input.path,
          redemptionId,
        },
        redaction: {
          exposesMdkCredentials: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    },
    { status: 202 },
  )
}

const decodeBody = async <A>(
  request: Request,
  schema: S.Decoder<A>,
): Promise<A> => {
  const body = await readJsonObject(request)

  return decodeUnknownWithSchema(schema, body)
}

const readCheckoutIntentState = async (
  dependencies: SiteCommerceRoutesDependencies,
  checkoutIntentRef: string,
): Promise<
  | Readonly<{
      challenge: BuyerPaymentChallengeRecord
      entitlement: BuyerPaymentEntitlementRecord | null
      hostedCheckout: OpenAgentsHostedMdkCheckoutProjection
      intent: OpenAgentsSiteMdkCheckoutIntentRecord
      receipt: BuyerPaymentReceiptRecord | null
    }>
  | undefined
> => {
  if (
    dependencies.checkoutIntentStore === undefined ||
    dependencies.buyerPaymentLedgerStore === undefined
  ) {
    return undefined
  }

  const intent =
    await dependencies.checkoutIntentStore.readCheckoutIntentByIntentRef(
      checkoutIntentRef,
    )

  if (intent === undefined) {
    return undefined
  }

  const challenge =
    await dependencies.buyerPaymentLedgerStore.readChallengeByIdempotencyKeyHash(
      intent.idempotencyKeyHash,
    )

  if (challenge === undefined) {
    return undefined
  }

  const receipt =
    await dependencies.buyerPaymentLedgerStore.readReceiptByRef(
      checkoutReceiptRef(intent),
    ) ?? null
  const entitlement =
    await dependencies.buyerPaymentLedgerStore.readEntitlementByRef(
      checkoutEntitlementRef(intent),
    ) ?? null

  return {
    challenge,
    entitlement,
    hostedCheckout: hostedCheckoutProjectionFromIntent(intent),
    intent,
    receipt,
  }
}

const reconciliationRecordFromProjection = (
  providerEvent: OpenAgentsSiteMdkProviderEvent,
  projection: ReturnType<typeof projectOpenAgentsSiteMdkReconciliation>,
): BuyerPaymentReconciliationEventRecord => ({
  archivedAt: null,
  challengeRef: providerEvent.challengeRef,
  createdAt: providerEvent.occurredAt,
  eventRef: providerEvent.eventRef,
  externalEventRef: providerEvent.providerEventRef,
  id: providerEvent.eventRef,
  idempotencyKeyHash: providerEvent.eventBodyDigestRef,
  metadataRefs: providerEvent.metadataRefs,
  productId: providerEvent.productId,
  providerRef: providerEvent.providerRef,
  publicProjectionJson:
    projection.buyerPaymentReconciliationEvent.publicProjectionJson,
  receiptRef: projection.receipt?.receiptRef ?? null,
  resultRef:
    projection.buyerPaymentReconciliationEvent.status === 'matched'
      ? 'result.site_mdk_reconciliation.matched'
      : projection.buyerPaymentReconciliationEvent.status === 'observed'
        ? 'result.site_mdk_reconciliation.observed'
        : projection.buyerPaymentReconciliationEvent.status === 'replayed'
          ? 'result.site_mdk_reconciliation.replayed'
          : 'result.site_mdk_reconciliation.rejected',
  status:
    projection.buyerPaymentReconciliationEvent
      .status as BuyerPaymentReconciliationEventRecord['status'],
})

const handleCheckoutReturn = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  actionPath: string,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const match = /^checkout-returns\/([^/]+)\/(cancel|status|success)$/u.exec(
    actionPath,
  )
  const checkoutIntentRef =
    match === null ? undefined : decodeURIComponent(match[1] ?? '')
  const returnAction = match?.[2] as 'cancel' | 'status' | 'success' | undefined

  if (checkoutIntentRef === undefined || returnAction === undefined) {
    return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  if (
    dependencies.checkoutIntentStore === undefined ||
    dependencies.buyerPaymentLedgerStore === undefined
  ) {
    return siteCommerceUnavailableResponse(
      'Checkout return handling requires durable checkout and payment stores.',
    )
  }

  const state = await readCheckoutIntentState(dependencies, checkoutIntentRef)

  if (state === undefined || state.intent.siteId !== siteId) {
    return checkoutIntentNotFoundResponse()
  }

  const observedReturnPath =
    returnAction === 'cancel'
      ? state.intent.cancelReturnPath
      : returnAction === 'success'
        ? state.intent.successReturnPath
        : `/commerce/checkout-returns/${cleanRefSegment(checkoutIntentRef)}/status`
  const returnProjection = projectOpenAgentsSiteCheckoutReturn({
    audience: 'agent',
    buyerPaymentChallenge: state.challenge,
    entitlement: state.entitlement,
    hostedCheckout: state.hostedCheckout,
    nowEpochMillis: dependencies.nowEpochMillis(),
    observedReturnPath,
    receipt: state.receipt,
    returnAction,
    route: {
      cancelPath: state.intent.cancelReturnPath,
      checkoutIntentRef: state.intent.checkoutIntentRef,
      checkoutRef: state.intent.checkoutRef,
      siteId: state.intent.siteId,
      siteVersionId: state.intent.siteVersionId,
      successPath: state.intent.successReturnPath,
    },
    uiPrimitives: null,
  })

  return noStoreJsonResponse({
    siteCommerce: {
      action: 'checkout_return_read' satisfies CommerceAction,
      checkoutIntentRef: state.intent.checkoutIntentRef,
      returnProjection,
      redaction: {
        exposesCustomerPrivateData: false,
        exposesMdkCredentials: false,
        exposesProviderPayoutClaims: false,
        exposesRawInvoice: false,
        exposesWalletMaterial: false,
      },
    },
  })
}

const handlePaymentProof = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  actionPath: string,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const match = /^payment-proofs\/([^/]+)$/u.exec(actionPath)
  const checkoutIntentRef =
    match === null ? undefined : decodeURIComponent(match[1] ?? '')

  if (checkoutIntentRef === undefined) {
    return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  if (
    dependencies.checkoutIntentStore === undefined ||
    dependencies.buyerPaymentLedgerStore === undefined
  ) {
    return siteCommerceUnavailableResponse(
      'Payment proof handling requires durable checkout and payment stores.',
    )
  }

  const state = await readCheckoutIntentState(dependencies, checkoutIntentRef)

  if (state === undefined || state.intent.siteId !== siteId) {
    return checkoutIntentNotFoundResponse()
  }

  const reconciliationEvent =
    state.receipt === null
      ? null
      : await dependencies.buyerPaymentLedgerStore
        .readReconciliationEventByReceiptRef(state.receipt.receiptRef) ?? null

  try {
    return noStoreJsonResponse({
      siteCommerce: {
        action: 'payment_proof_read' satisfies CommerceAction,
        checkoutIntentRef: state.intent.checkoutIntentRef,
        paymentProof: projectOpenAgentsSitePaymentProof({
          audience: 'public',
          buyerPaymentChallenge: state.challenge,
          checkoutIntent: state.intent,
          entitlement: state.entitlement,
          receipt: state.receipt,
          reconciliationEvent,
        }),
        redaction: {
          exposesCustomerPrivateData: false,
          exposesMdkCredentials: false,
          exposesProviderPayoutClaims: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    })
  } catch (error) {
    if (error instanceof OpenAgentsSitePaymentProofUnsafe) {
      return noStoreJsonResponse(
        {
          error: 'payment_proof_unsafe',
          message:
            'The payment proof state could not be projected safely.',
        },
        { status: 409 },
      )
    }

    throw error
  }
}

const handleMdkWebhook = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  if (
    dependencies.checkoutIntentStore === undefined ||
    dependencies.buyerPaymentLedgerStore === undefined
  ) {
    return siteCommerceUnavailableResponse(
      'MDK webhook reconciliation requires durable checkout and payment stores.',
    )
  }

  const body = await request.text()
  const verification = await verifyOpenAgentsSiteMdkWebhook({
    body,
    config: dependencies.mdkWebhookConfig,
    headers: request.headers,
    nowIso: dependencies.nowIso(),
  })

  if (verification._tag === 'Invalid') {
    return noStoreJsonResponse(
      {
        error: `mdk_webhook_${verification.reason}`,
        message: 'The MDK webhook could not be verified.',
      },
      {
        status:
          verification.reason === 'missing_configuration'
            ? 503
            : verification.reason === 'invalid_signature'
              ? 401
              : 400,
      },
    )
  }

  const intent =
    await dependencies.checkoutIntentStore.readCheckoutIntentByCheckoutRef(
      verification.event.checkoutRef,
    )

  if (intent === undefined || intent.siteId !== siteId) {
    return checkoutIntentNotFoundResponse()
  }

  const challenge =
    await dependencies.buyerPaymentLedgerStore.readChallengeByIdempotencyKeyHash(
      intent.idempotencyKeyHash,
    )

  if (challenge === undefined) {
    return checkoutIntentNotFoundResponse()
  }

  const previousEvent =
    await dependencies.buyerPaymentLedgerStore
      .readReconciliationEventByProviderEvent(
        intent.providerRef,
        verification.event.providerEventRef,
      )
  const hostedCheckout = hostedCheckoutProjectionFromIntent(
    intent,
    verification.event.checkoutStatus,
  )
  const hostedCheckoutJson = JSON.stringify(hostedCheckout)
  const checkoutPublicJson = publicCheckoutIntentJson(intent, hostedCheckout)

  if (previousEvent === undefined) {
    await dependencies.checkoutIntentStore.updateCheckoutIntentStatus({
      checkoutRef: intent.checkoutRef,
      hostedCheckoutProjectionJson: hostedCheckoutJson,
      publicProjectionJson: checkoutPublicJson,
      status: verification.event.checkoutStatus,
      updatedAt: verification.event.occurredAt,
    })
  }

  const updatedIntent = {
    ...intent,
    hostedCheckoutProjectionJson: hostedCheckoutJson,
    publicProjectionJson: checkoutPublicJson,
    status: verification.event.checkoutStatus,
    updatedAt: verification.event.occurredAt,
  }
  const receipt =
    verification.event.checkoutStatus === 'payment_received'
      ? receiptForCheckoutIntent(
          updatedIntent,
          challenge,
          verification.event.occurredAt,
        )
      : null
  const entitlement =
    receipt === null
      ? null
      : entitlementForCheckoutIntent(
          dependencies,
          updatedIntent,
          challenge,
          receipt,
          verification.event.occurredAt,
        )

  if (
    previousEvent === undefined &&
    receipt !== null &&
    entitlement !== null
  ) {
    await dependencies.buyerPaymentLedgerStore.createReceiptEntitlementBundle({
      entitlement,
      receipt,
    })
  }

  const returnProjection =
    receipt === null || entitlement === null
      ? null
      : projectOpenAgentsSiteCheckoutReturn({
          audience: 'agent',
          buyerPaymentChallenge: challenge,
          entitlement,
          hostedCheckout,
          nowEpochMillis: dependencies.nowEpochMillis(),
          observedReturnPath: updatedIntent.successReturnPath,
          receipt,
          returnAction: 'success',
          route: {
            cancelPath: updatedIntent.cancelReturnPath,
            checkoutIntentRef: updatedIntent.checkoutIntentRef,
            checkoutRef: updatedIntent.checkoutRef,
            siteId: updatedIntent.siteId,
            siteVersionId: updatedIntent.siteVersionId,
            successPath: updatedIntent.successReturnPath,
          },
          uiPrimitives: null,
        })
  const providerEvent: OpenAgentsSiteMdkProviderEvent = {
    challengeRef: updatedIntent.challengeRef,
    checkoutRef: updatedIntent.checkoutRef,
    checkoutStatus: verification.event.checkoutStatus,
    environment: updatedIntent.environment,
    eventBodyDigestRef: verification.event.eventBodyDigestRef,
    eventKind: verification.event.eventKind,
    eventRef: `event.site_mdk.${cleanRefSegment(siteId)}.${cleanRefSegment(
      verification.event.providerEventRef,
    )}`,
    fakeProvider: false,
    metadataRefs: [
      ...updatedIntent.metadataRefs,
      `metadata.site_mdk_webhook.${cleanRefSegment(siteId)}`,
    ],
    occurredAt: verification.event.occurredAt,
    productId: updatedIntent.productId,
    providerEventRef: verification.event.providerEventRef,
    providerRef: updatedIntent.providerRef,
    sandbox: updatedIntent.sandbox,
    signatureBindingRef: verification.event.signatureBindingRef,
    signatureVerified: true,
    siteId: updatedIntent.siteId,
    siteVersionId: updatedIntent.siteVersionId,
  }
  const reconciliation = projectOpenAgentsSiteMdkReconciliation({
    audience: 'agent',
    entitlement,
    hostedCheckout,
    previousEventRef: previousEvent?.eventRef ?? null,
    providerEvent,
    receipt,
    returnProjection,
  })

  if (previousEvent === undefined) {
    await dependencies.buyerPaymentLedgerStore.createReconciliationEvent(
      reconciliationRecordFromProjection(providerEvent, reconciliation),
    )
  }

  return noStoreJsonResponse(
    {
      siteCommerce: {
        action: 'mdk_webhook_reconcile' satisfies CommerceAction,
        checkoutIntentRef: updatedIntent.checkoutIntentRef,
        duplicate: previousEvent !== undefined,
        reconciliation,
        redaction: {
          exposesCustomerPrivateData: false,
          exposesMdkCredentials: false,
          exposesProviderPayoutClaims: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    },
    { status: previousEvent === undefined ? 202 : 200 },
  )
}

const handlePayoutBridge = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  if (
    dependencies.checkoutIntentStore === undefined ||
    dependencies.buyerPaymentLedgerStore === undefined ||
    dependencies.payoutLedgerStore === undefined
  ) {
    return siteCommerceUnavailableResponse(
      'Site payment-to-payout bridge requires durable checkout, buyer payment, and payout ledgers.',
    )
  }

  if (dependencies.authorizePayoutBridge === undefined) {
    return payoutBridgeUnauthorizedResponse()
  }

  const authorized = await dependencies.authorizePayoutBridge(request)

  if (!authorized) {
    return payoutBridgeUnauthorizedResponse()
  }

  let input: OpenAgentsSitePaymentToPayoutBridgeRequest

  try {
    input = await decodeBody(request, OpenAgentsSitePaymentToPayoutBridgeRequest)
  } catch {
    return invalidContractResponse(
      'Request body does not match the payment-to-payout bridge schema.',
    )
  }

  if (input.checkoutIntentRef.trim() === '') {
    return invalidContractResponse('checkoutIntentRef is required.')
  }

  const state = await readCheckoutIntentState(
    dependencies,
    input.checkoutIntentRef,
  )

  if (state === undefined || state.intent.siteId !== siteId) {
    return checkoutIntentNotFoundResponse()
  }

  const reconciliationEvent =
    state.receipt === null
      ? null
      : await dependencies.buyerPaymentLedgerStore
        .readReconciliationEventByReceiptRef(state.receipt.receiptRef) ?? null
  const existingPayoutIntent =
    state.receipt === null
      ? null
      : await dependencies.payoutLedgerStore
        .readPayoutIntentByBuyerPaymentRef(state.receipt.receiptRef) ?? null
  const releaseGate = projectPylonV02OmegaReleaseGate(
    readyPylonV02OmegaReleaseGateRecord(),
    'operator',
    dependencies.nowIso(),
  )
  const bridgeResult = buildOpenAgentsSitePaymentToPayoutBridge({
    audience: 'operator',
    existingPayoutIntentForBuyerPaymentRef: existingPayoutIntent,
    idempotencyKey,
    nowIso: dependencies.nowIso(),
    receipt: state.receipt,
    reconciliationEvent,
    releaseGate,
    request: input,
    returnProjection: null,
    siteCheckoutIntent: state.intent,
  })

  if (bridgeResult._tag === 'Blocked') {
    return noStoreJsonResponse(
      {
        siteCommerce: {
          action: 'payout_bridge_create' satisfies CommerceAction,
          bridge: bridgeResult.projection,
          redaction: {
            exposesCustomerPrivateData: false,
            exposesMdkCredentials: false,
            exposesProviderPayoutClaims: false,
            exposesRawInvoice: false,
            exposesWalletMaterial: false,
          },
        },
      },
      { status: 409 },
    )
  }

  try {
    await dependencies.payoutLedgerStore.createPayoutIntent(bridgeResult.intent)

    return noStoreJsonResponse(
      {
        siteCommerce: {
          action: 'payout_bridge_create' satisfies CommerceAction,
          bridge: bridgeResult.projection,
          idempotency: {
            key: idempotencyKey,
            replaySafe: true,
          },
          payoutIntent: projectNexusTreasuryPayoutLedgerRecord(
            'intent',
            bridgeResult.intent,
            'operator',
          ),
          redaction: {
            exposesCustomerPrivateData: false,
            exposesMdkCredentials: false,
            exposesProviderPayoutClaims: false,
            exposesRawInvoice: false,
            exposesWalletMaterial: false,
          },
        },
      },
      { status: 201 },
    )
  } catch {
    return noStoreJsonResponse(
      {
        error: 'payout_bridge_store_unavailable',
        message: 'The payout intent ledger could not persist the bridge.',
      },
      { status: 409 },
    )
  }
}

const validateCheckoutIntentRequest = (
  input: SiteCheckoutIntentRequest,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): string | undefined =>
  validateCatalogId(input.siteVersionId, 'siteVersionId') ??
  (input.catalogRef === undefined
    ? undefined
    : validateCatalogId(input.catalogRef, 'catalogRef')) ??
  (input.productId === undefined
    ? undefined
    : validateCatalogId(input.productId, 'productId')) ??
  (input.actionId === undefined
    ? undefined
    : validateCatalogId(input.actionId, 'actionId')) ??
  validateLocalPath(input.successReturnPath, 'successReturnPath') ??
  validateLocalPath(input.cancelReturnPath, 'cancelReturnPath') ??
  validateLocalPath(catalogItem.checkoutPath, 'catalog.checkoutPath') ??
  (openAgentsSitePaymentCatalogHasPrivateMaterial(input)
    ? 'checkout intent request must not contain customer private data, payment material, provider grants, or secrets.'
    : undefined) ??
  (input.expectedPrice !== undefined &&
  !amountEquals(input.expectedPrice, catalogItem.price)
    ? 'expectedPrice must match the Site payment catalog price.'
    : undefined) ??
  validateRequiredCustomerDataRefs(input, catalogItem)

const validateRequiredCustomerDataRefs = (
  input: SiteCheckoutIntentRequest,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): string | undefined => {
  const customerDataRefs = input.customerDataRefs ?? []
  const invalidRef = customerDataRefs.find(
    ref => validateCatalogId(ref, 'customerDataRefs') !== undefined,
  )
  const requiredRefs = catalogItem.customerDataRequirements
    .filter(requirement => requirement.required)
    .map(requirement => requirement.key)
  const missingRef = requiredRefs.find(
    requiredRef => !customerDataRefs.includes(requiredRef),
  )

  if (invalidRef !== undefined) {
    return 'customerDataRefs must contain stable public-safe requirement refs.'
  }

  return missingRef === undefined
    ? undefined
    : `customerDataRefs must include required field ${missingRef}.`
}

const catalogItemMatchesCheckoutIntent = (
  item: OpenAgentsSitePaymentCatalogRecord,
  siteId: string,
  input: SiteCheckoutIntentRequest,
): boolean =>
  item.siteId === siteId &&
  item.siteVersionId === input.siteVersionId &&
  item.status === 'active' &&
  item.itemKind === input.itemKind &&
  (input.catalogRef === undefined || item.catalogRef === input.catalogRef) &&
  (item.itemKind === 'product'
    ? input.productId !== undefined && item.productId === input.productId
    : input.actionId !== undefined && item.actionId === input.actionId)

const lookupCheckoutCatalogItem = (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
  input: SiteCheckoutIntentRequest,
): OpenAgentsSitePaymentCatalogRecord | undefined =>
  checkoutCatalogForDependencies(dependencies).items.find(item =>
    catalogItemMatchesCheckoutIntent(item, siteId, input),
  )

const lookupReviewCatalogItem = (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
  catalogRef: string,
): OpenAgentsSitePaymentCatalogRecord | undefined =>
  checkoutCatalogForDependencies(dependencies).items.find(item =>
    item.siteId === siteId &&
    item.catalogRef === catalogRef &&
    item.archivedAt === null,
  )

const commerceReviewActorRef = (
  request: Request,
  idempotencyKey: string,
): string => {
  const actorRef = request.headers.get('x-openagents-actor-ref')?.trim()

  return actorRef === undefined || actorRef === ''
    ? `operator.site_commerce_review.${cleanRefSegment(idempotencyKey)}`
    : `operator.${cleanRefSegment(actorRef)}`
}

const buildCommerceReviewDecisionRecord = (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
  input: OpenAgentsSiteCommerceReviewDecisionRequest,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
): OpenAgentsSiteCommerceReviewDecisionRecord => {
  const id = opaqueRef('site_commerce_review_decision', siteId, idempotencyKey)
  const decisionRef =
    `site_commerce_review:${cleanRefSegment(siteId)}:${cleanRefSegment(catalogItem.siteVersionId)}:${cleanRefSegment(catalogItem.catalogRef)}:${cleanRefSegment(idempotencyKey)}`
  const idempotencyKeyHash =
    `hash.site_commerce_review.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`
  const record = {
    actorRef: commerceReviewActorRef(request, idempotencyKey),
    archivedAt: null,
    catalogRef: catalogItem.catalogRef,
    createdAt: dependencies.nowIso(),
    customerInputRequirementRefs:
      input.reviewStatus === 'needs_customer_input'
        ? input.customerInputRequirementRefs ?? []
        : input.customerInputRequirementRefs ?? [],
    decisionRef,
    id,
    idempotencyKeyHash,
    publicProjectionJson: '{}',
    reasonRefs: input.reasonRefs ?? [],
    reviewStatus: input.reviewStatus,
    siteId,
    siteVersionId: catalogItem.siteVersionId,
    updatedAt: dependencies.nowIso(),
  } satisfies OpenAgentsSiteCommerceReviewDecisionRecord

  return {
    ...record,
    publicProjectionJson: siteCommerceReviewDecisionPublicJson(record),
  }
}

const mdkAccountBindingsForSite = async (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
): Promise<ReadonlyArray<OpenAgentsSiteMdkAccountBindingRecord>> =>
  dependencies.mdkAccountBindingStore === undefined
    ? []
    : dependencies.mdkAccountBindingStore.listBindingsForSite(siteId)

const mdkAccountBindingAudience = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
): Promise<'customer' | 'operator'> =>
  dependencies.authorizeMdkAccountBinding !== undefined &&
    await dependencies.authorizeMdkAccountBinding(request)
    ? 'operator'
    : 'customer'

const currentMdkAccountBindingForSite = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
) => {
  const bindings = await mdkAccountBindingsForSite(dependencies, siteId)
  const binding = bindings[0] ?? null

  return projectOpenAgentsSiteMdkAccountBinding({
    audience: await mdkAccountBindingAudience(dependencies, request),
    binding,
    siteId,
  })
}

const currentMdkAccountBindingForCheckout = async (
  dependencies: SiteCommerceRoutesDependencies,
  catalogItem: OpenAgentsSitePaymentCatalogRecord,
) => {
  const bindings = await mdkAccountBindingsForSite(
    dependencies,
    catalogItem.siteId,
  )

  return currentMdkAccountBindingForCatalogItem(bindings, catalogItem)
}

const mdkAccountBindingActorRef = (
  request: Request,
  idempotencyKey: string,
): string => {
  const actorRef = request.headers.get('x-openagents-actor-ref')?.trim()

  return actorRef === undefined || actorRef === ''
    ? `operator.site_mdk_account.${cleanRefSegment(idempotencyKey)}`
    : `operator.${cleanRefSegment(actorRef)}`
}

const buildMdkAccountBindingRecord = (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
  input: OpenAgentsSiteMdkAccountBindingRequest,
): OpenAgentsSiteMdkAccountBindingRecord => {
  const bindingSegment = cleanRefSegment(
    input.bindingRef ?? `${siteId}_${idempotencyKey}`,
  )
  const bindingRef = input.bindingRef ??
    `site_mdk_account:${cleanRefSegment(siteId)}:${bindingSegment}`
  const idempotencyKeyHash =
    `hash.site_mdk_account.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`
  const record = {
    allowedActionRefs: input.allowedActionRefs ?? [],
    allowedCatalogRefs: input.allowedCatalogRefs ?? [],
    allowedProductRefs: input.allowedProductRefs ?? [],
    archivedAt: null,
    bindingRef,
    caveatRefs: input.caveatRefs ?? [],
    createdAt: dependencies.nowIso(),
    customerRef: input.customerRef,
    environment: input.environment,
    id: opaqueRef('site_mdk_account_binding', siteId, idempotencyKey),
    idempotencyKeyHash,
    orderRef: input.orderRef,
    publicProjectionJson: '{}',
    requestedProviderMode: input.requestedProviderMode,
    reviewStatus: input.reviewStatus,
    reviewerRefs: [
      mdkAccountBindingActorRef(request, idempotencyKey),
      ...(input.reviewerRefs ?? []),
    ],
    secretBindingRefs: input.secretBindingRefs,
    siteId,
    siteVersionId: input.siteVersionId,
    updatedAt: dependencies.nowIso(),
  } satisfies OpenAgentsSiteMdkAccountBindingRecord

  return {
    ...record,
    publicProjectionJson: siteMdkAccountBindingPublicJson(record),
  }
}

const commerceReviewRedaction = {
  exposesCheckoutQueryState: false,
  exposesCustomerPrivateData: false,
  exposesMdkCredentials: false,
  exposesProviderGrants: false,
  exposesRawInvoice: false,
  exposesRawTimestamps: false,
  exposesWalletMaterial: false,
} as const

const mdkAccountBindingRedaction = {
  exposesCustomerPrivateData: false,
  exposesMdkCredentials: false,
  exposesProviderGrants: false,
  exposesRawInvoice: false,
  exposesRawTimestamps: false,
  exposesWalletMaterial: false,
} as const

const commerceReviewDecisionsForSite = async (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
): Promise<ReadonlyArray<OpenAgentsSiteCommerceReviewDecisionRecord>> =>
  dependencies.reviewStore === undefined
    ? []
    : dependencies.reviewStore.listReviewDecisionsForSite(siteId)

const commerceReviewAudience = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
): Promise<'agent' | 'operator'> =>
  dependencies.authorizeCommerceReviewDecision !== undefined &&
    await dependencies.authorizeCommerceReviewDecision(request)
    ? 'operator'
    : 'agent'

const buildCommerceReviewProjection = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
) => {
  const catalog = checkoutCatalogForDependencies(dependencies)
  const uiPrimitives = siteCheckoutUiPrimitivesFromCatalog({
    cancelPath: '/checkout/cancelled',
    catalog,
    runtimeTarget: 'workers_for_platforms',
    sourceSurface: 'generated_react',
    successPath: '/checkout/complete',
  })

  return projectOpenAgentsSiteCommerceReview({
    audience: await commerceReviewAudience(dependencies, request),
    cancelPath: '/checkout/cancelled',
    catalog,
    decisions: await commerceReviewDecisionsForSite(dependencies, siteId),
    successPath: '/checkout/complete',
    uiPrimitives,
  })
}

const createCommerceReviewResponse = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
): Promise<HttpResponse> => {
  try {
    return noStoreJsonResponse({
      siteCommerce: {
        action: 'commerce_review_read' satisfies CommerceAction,
        redaction: commerceReviewRedaction,
        review: await buildCommerceReviewProjection(
          dependencies,
          request,
          siteId,
        ),
      },
    })
  } catch (error) {
    if (
      error instanceof OpenAgentsSiteCommerceReviewUnsafe ||
      error instanceof OpenAgentsSiteCheckoutUiPrimitiveUnsafe
    ) {
      return noStoreJsonResponse(
        {
          error: 'site_commerce_review_unsafe',
          message:
            'The Site commerce review projection could not be emitted safely.',
        },
        { status: 409 },
      )
    }

    return siteCommerceUnavailableResponse(
      'The Site commerce review store could not be read.',
    )
  }
}

const handleCommerceReviewDecision = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  if (dependencies.reviewStore === undefined) {
    return siteCommerceUnavailableResponse(
      'Site commerce review decisions require durable review storage.',
    )
  }

  if (dependencies.authorizeCommerceReviewDecision === undefined) {
    return commerceReviewUnauthorizedResponse()
  }

  const authorized = await dependencies.authorizeCommerceReviewDecision(request)

  if (!authorized) {
    return commerceReviewUnauthorizedResponse()
  }

  let input: OpenAgentsSiteCommerceReviewDecisionRequest

  try {
    input = await decodeBody(
      request,
      OpenAgentsSiteCommerceReviewDecisionRequest,
    )
  } catch {
    return invalidContractResponse(
      'Request body does not match the Site commerce review decision schema.',
    )
  }

  const catalogItem = lookupReviewCatalogItem(
    dependencies,
    siteId,
    input.catalogRef,
  )

  if (catalogItem === undefined) {
    return invalidContractResponse(
      'review decision does not match a Site payment catalog item.',
    )
  }

  if (validateNoSecretMaterial(input, 'review decision') !== undefined) {
    return invalidContractResponse(
      'review decision must not contain customer private data, payment material, provider grants, or secrets.',
    )
  }

  const idempotencyKeyHash =
    `hash.site_commerce_review.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`
  const existing =
    await dependencies.reviewStore.readReviewDecisionByIdempotencyKeyHash(
      idempotencyKeyHash,
    )

  if (existing !== undefined) {
    return noStoreJsonResponse(
      {
        siteCommerce: {
          action:
            'commerce_review_decision_create' satisfies CommerceAction,
          decision: projectOpenAgentsSiteCommerceReviewDecision(existing),
          duplicate: true,
          idempotency: {
            key: idempotencyKey,
            replaySafe: true,
          },
          redaction: commerceReviewRedaction,
          review: await buildCommerceReviewProjection(
            dependencies,
            request,
            siteId,
          ),
        },
      },
      { status: 200 },
    )
  }

  const decision = buildCommerceReviewDecisionRecord(
    dependencies,
    request,
    siteId,
    idempotencyKey,
    input,
    catalogItem,
  )

  try {
    const stored = await dependencies.reviewStore.upsertReviewDecision(
      decision,
    )

    return noStoreJsonResponse(
      {
        siteCommerce: {
          action:
            'commerce_review_decision_create' satisfies CommerceAction,
          decision: projectOpenAgentsSiteCommerceReviewDecision(stored),
          duplicate: false,
          idempotency: {
            key: idempotencyKey,
            replaySafe: true,
          },
          redaction: commerceReviewRedaction,
          review: await buildCommerceReviewProjection(
            dependencies,
            request,
            siteId,
          ),
        },
      },
      { status: 201 },
    )
  } catch {
    return siteCommerceUnavailableResponse(
      'The Site commerce review decision could not be persisted.',
    )
  }
}

const createMdkAccountBindingResponse = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
): Promise<HttpResponse> => {
  try {
    return noStoreJsonResponse({
      siteCommerce: {
        action: 'mdk_account_binding_read' satisfies CommerceAction,
        mdkAccountBinding: await currentMdkAccountBindingForSite(
          dependencies,
          request,
          siteId,
        ),
        redaction: mdkAccountBindingRedaction,
      },
    })
  } catch (error) {
    if (error instanceof OpenAgentsSiteMdkAccountBindingUnsafe) {
      return noStoreJsonResponse(
        {
          error: 'site_mdk_account_binding_unsafe',
          message:
            'The Site MDK account binding projection could not be emitted safely.',
        },
        { status: 409 },
      )
    }

    return siteCommerceUnavailableResponse(
      'The Site MDK account binding store could not be read.',
    )
  }
}

const handleMdkAccountBindingUpsert = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  if (dependencies.mdkAccountBindingStore === undefined) {
    return siteCommerceUnavailableResponse(
      'Site MDK account binding actions require durable binding storage.',
    )
  }

  if (dependencies.authorizeMdkAccountBinding === undefined) {
    return mdkAccountBindingUnauthorizedResponse()
  }

  const authorized = await dependencies.authorizeMdkAccountBinding(request)

  if (!authorized) {
    return mdkAccountBindingUnauthorizedResponse()
  }

  let input: OpenAgentsSiteMdkAccountBindingRequest

  try {
    input = await decodeBody(request, OpenAgentsSiteMdkAccountBindingRequest)
  } catch {
    return invalidContractResponse(
      'Request body does not match the Site MDK account binding schema.',
    )
  }

  if (validateNoSecretMaterial(input, 'MDK account binding') !== undefined) {
    return invalidContractResponse(
      'MDK account binding must contain hosted secret-binding refs only, never MDK tokens, mnemonics, webhook secrets, wallet material, raw invoices, payment hashes, preimages, customer private values, provider grants, or secrets.',
    )
  }

  const idempotencyKeyHash =
    `hash.site_mdk_account.${cleanRefSegment(siteId)}.${cleanRefSegment(idempotencyKey)}`
  const existing =
    await dependencies.mdkAccountBindingStore
      .readBindingByIdempotencyKeyHash(idempotencyKeyHash)

  if (existing !== undefined) {
    return noStoreJsonResponse(
      {
        siteCommerce: {
          action: 'mdk_account_binding_upsert' satisfies CommerceAction,
          duplicate: true,
          idempotency: {
            key: idempotencyKey,
            replaySafe: true,
          },
          mdkAccountBinding: projectOpenAgentsSiteMdkAccountBinding({
            audience: 'operator',
            binding: existing,
            siteId,
          }),
          redaction: mdkAccountBindingRedaction,
        },
      },
      { status: 200 },
    )
  }

  const binding = buildMdkAccountBindingRecord(
    dependencies,
    request,
    siteId,
    idempotencyKey,
    input,
  )

  try {
    const stored =
      await dependencies.mdkAccountBindingStore.upsertBinding(binding)

    return noStoreJsonResponse(
      {
        siteCommerce: {
          action: 'mdk_account_binding_upsert' satisfies CommerceAction,
          duplicate: false,
          idempotency: {
            key: idempotencyKey,
            replaySafe: true,
          },
          mdkAccountBinding: projectOpenAgentsSiteMdkAccountBinding({
            audience: 'operator',
            binding: stored,
            siteId,
          }),
          redaction: mdkAccountBindingRedaction,
        },
      },
      { status: 201 },
    )
  } catch {
    return siteCommerceUnavailableResponse(
      'The Site MDK account binding could not be persisted.',
    )
  }
}

const validateL402ChallengeRequest = (
  input: SiteL402ChallengeRequest,
): string | undefined =>
  validateCatalogId(input.paidActionId, 'paidActionId') ??
  validateLocalPath(input.path, 'path') ??
  (!isValidPrice(input.price)
    ? 'price must be greater than zero.'
    : undefined) ??
  (!priceWithinSpendCap(input.price, input.spendCap)
    ? 'price must be within the declared spend cap.'
    : undefined)

const validateL402RedemptionRequest = (
  dependencies: SiteCommerceRoutesDependencies,
  input: SiteL402RedemptionRequest,
): string | undefined =>
  validateCatalogId(input.paidActionId, 'paidActionId') ??
  validateLocalPath(input.path, 'path') ??
  (!isValidPrice(input.price)
    ? 'price must be greater than zero.'
    : undefined) ??
  (!PAYMENT_PROOF_REF_PATTERN.test(input.paymentProofRef)
    ? 'paymentProofRef must be a redacted hosted payment proof ref.'
    : undefined) ??
  validateNoSecretMaterial(input.paymentProofRef, 'paymentProofRef') ??
  (Number.isNaN(Date.parse(input.challengeExpiresAt))
    ? 'challengeExpiresAt must be an ISO timestamp.'
    : undefined) ??
  (Date.parse(input.challengeExpiresAt) <= dependencies.nowEpochMillis()
    ? 'stale'
    : undefined)

const handleCheckoutIntent = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  let input: SiteCheckoutIntentRequest

  try {
    input = await decodeBody(request, SiteCheckoutIntentRequest)
  } catch {
    return invalidContractResponse(
      'Request body does not match the checkout intent schema.',
    )
  }

  const catalogItem = lookupCheckoutCatalogItem(dependencies, siteId, input)

  if (catalogItem === undefined) {
    return invalidContractResponse(
      'checkout intent does not match an active Site payment catalog item.',
    )
  }

  const validationError = validateCheckoutIntentRequest(input, catalogItem)

  return validationError === undefined
    ? createCheckoutIntentResponse(
        dependencies,
        siteId,
        idempotencyKey,
        input,
        catalogItem,
      )
    : invalidContractResponse(validationError)
}

const handleL402Challenge = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  if (dependencies.authorizePaidActionAgent === undefined) {
    return paidActionAgentUnauthorizedResponse()
  }

  const authorized = await dependencies.authorizePaidActionAgent(request)

  if (!authorized) {
    return paidActionAgentUnauthorizedResponse()
  }

  let input: SiteL402ChallengeRequest

  try {
    input = await decodeBody(request, SiteL402ChallengeRequest)
  } catch {
    return invalidContractResponse(
      'Request body does not match the L402 challenge schema.',
    )
  }

  const validationError = validateL402ChallengeRequest(input)

  return validationError === undefined
    ? createL402ChallengeResponse(dependencies, siteId, idempotencyKey, input)
    : invalidContractResponse(validationError)
}

const handleL402Redemption = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  idempotencyKey: string,
): Promise<HttpResponse> => {
  if (dependencies.authorizePaidActionAgent === undefined) {
    return paidActionAgentUnauthorizedResponse()
  }

  const authorized = await dependencies.authorizePaidActionAgent(request)

  if (!authorized) {
    return paidActionAgentUnauthorizedResponse()
  }

  let input: SiteL402RedemptionRequest

  try {
    input = await decodeBody(request, SiteL402RedemptionRequest)
  } catch {
    return invalidContractResponse(
      'Request body does not match the L402 redemption schema.',
    )
  }

  const validationError = validateL402RedemptionRequest(dependencies, input)

  if (validationError === 'stale') {
    return staleChallengeResponse()
  }

  return validationError === undefined
    ? createL402RedemptionResponse(siteId, idempotencyKey, input)
    : invalidContractResponse(validationError)
}

const createPaymentDiscoveryResponse = (
  dependencies: SiteCommerceRoutesDependencies,
  siteId: string,
): HttpResponse =>
  noStoreJsonResponse({
    siteCommerce: {
      action: 'payment_discovery_read' satisfies CommerceAction,
      discovery: projectOpenAgentsSitePaymentDiscovery({
        audience: 'agent',
        catalog: checkoutCatalogForDependencies(dependencies),
        siteId,
      }),
      implementationState: 'fake_provider_contract',
    },
  })

const handleSiteCommerceAction = async (
  dependencies: SiteCommerceRoutesDependencies,
  request: Request,
  siteId: string,
  actionPath: string,
): Promise<HttpResponse> => {
  if (!SITE_ID_PATTERN.test(siteId)) {
    return invalidContractResponse('siteId is invalid.')
  }

  if (actionPath === 'discovery') {
    return request.method === 'GET'
      ? createPaymentDiscoveryResponse(dependencies, siteId)
      : methodNotAllowed(['GET'])
  }

  if (actionPath === 'review') {
    return request.method === 'GET'
      ? createCommerceReviewResponse(dependencies, request, siteId)
      : methodNotAllowed(['GET'])
  }

  if (actionPath === 'mdk-account-binding') {
    return request.method === 'GET'
      ? createMdkAccountBindingResponse(dependencies, request, siteId)
      : methodNotAllowed(['GET'])
  }

  if (actionPath.startsWith('checkout-returns/')) {
    return handleCheckoutReturn(dependencies, request, siteId, actionPath)
  }

  if (actionPath.startsWith('payment-proofs/')) {
    return handlePaymentProof(dependencies, request, siteId, actionPath)
  }

  if (actionPath === 'mdk/webhooks') {
    return handleMdkWebhook(dependencies, request, siteId)
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return idempotencyRequiredResponse()
  }

  if (actionPath === 'checkout-intents') {
    return handleCheckoutIntent(dependencies, request, siteId, idempotencyKey)
  }

  if (actionPath === 'review-decisions') {
    return handleCommerceReviewDecision(
      dependencies,
      request,
      siteId,
      idempotencyKey,
    )
  }

  if (actionPath === 'mdk-account-bindings') {
    return handleMdkAccountBindingUpsert(
      dependencies,
      request,
      siteId,
      idempotencyKey,
    )
  }

  if (actionPath === 'payout-bridges') {
    return handlePayoutBridge(dependencies, request, siteId, idempotencyKey)
  }

  if (actionPath === 'l402/challenges') {
    return handleL402Challenge(dependencies, request, siteId, idempotencyKey)
  }

  if (actionPath === 'l402/redemptions') {
    return handleL402Redemption(dependencies, request, siteId, idempotencyKey)
  }

  return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
}

export const makeSiteCommerceRoutes = (
  dependencies: SiteCommerceRoutesDependencies,
) => ({
  routeSiteCommerceRequest: (
    request: Request,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const match = /^\/api\/sites\/([^/]+)\/commerce\/(.+)$/.exec(url.pathname)

    if (match === null) {
      return undefined
    }

    const siteId = decodeURIComponent(match[1] ?? '')
    const actionPath = match[2] ?? ''

    return Effect.promise(() =>
      handleSiteCommerceAction(dependencies, request, siteId, actionPath),
    )
  },
})
