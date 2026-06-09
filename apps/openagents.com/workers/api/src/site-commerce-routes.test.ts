import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { previewOpenAgentsSpendCap } from './agent-spend-cap-preview'
import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentCreditDebitRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerStore,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
  BuyerPaymentRedemptionRecord,
  BuyerPaymentSpendLimitRecord,
} from './buyer-payment-ledger'
import {
  exampleOpenAgentsGeneratedSitePaymentCatalog,
  exampleOpenAgentsGeneratedSitePaymentHelperPlans,
} from './generated-site-payment-smoke-fixture'
import { isRecord } from './json-boundary'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusPayoutTargetApprovalRecord,
  NexusReleaseGateRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type {
  OpenAgentsSiteCommerceReviewDecisionRecord,
  OpenAgentsSiteCommerceReviewStore,
} from './site-commerce-review'
import { makeSiteCommerceRoutes } from './site-commerce-routes'
import type {
  OpenAgentsSiteMdkAccountBindingRecord,
  OpenAgentsSiteMdkAccountBindingStore,
} from './site-mdk-account-bindings'
import type {
  OpenAgentsSiteMdkCheckoutIntentBundle,
  OpenAgentsSiteMdkCheckoutIntentRecord,
  OpenAgentsSiteMdkCheckoutIntentStore,
} from './site-mdk-checkout-intents'
import {
  exampleOpenAgentsSiteMdkSmokeRecord,
  projectOpenAgentsSiteMdkSmoke,
} from './site-mdk-smoke'
import {
  type OpenAgentsSitePaymentPaidActionCatalogRecord,
  openAgentsPaidEndpointProductFromSitePaymentCatalogItem,
  sitePaymentCatalogFromManifest,
} from './site-payment-catalog'
import type { OpenAgentsUnifiedPaymentDecisionProjection } from './unified-payment-decision'

const fixedNow = new Date('2026-06-05T18:00:00.000Z')

const sitePaymentCatalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-05T17:55:00.000Z',
  deploymentId: 'deployment.site_otec.v2',
  manifest: {
    payments: {
      agentReadable: true,
      enabled: true,
      metadataRefs: ['metadata.site_payment.site_otec.v2'],
      paidActions: [
        {
          actionRef: 'action.report.download',
          agentReadable: true,
          checkoutPath: '/checkout/download-report',
          customerDataRequirements: [],
          displayRef: 'display.download_report',
          entitlementScope: 'action',
          id: 'download_report',
          metadataRefs: ['metadata.action.download_report'],
          method: 'GET',
          path: '/api/actions/download-report',
          price: {
            amountMinorUnits: 25_000,
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'deferred',
        },
      ],
      products: [
        {
          agentReadable: true,
          checkoutPath: '/checkout/consultation-deposit',
          customerDataRequirements: [
            {
              key: 'email',
              kind: 'email',
              labelRef: 'label.customer.email',
              required: true,
            },
          ],
          displayRef: 'display.consultation_deposit',
          entitlementScope: 'product',
          id: 'consultation_deposit',
          metadataRefs: ['metadata.product.consultation_deposit'],
          price: {
            amountMinorUnits: 2500,
            asset: 'usd',
            denomination: 'usd_cent',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'checkout_only',
        },
      ],
      provider: 'openagents_hosted_mdk',
      sandboxDefault: true,
    },
  },
  manifestRef: 'manifest.site_otec.payments.v2',
  orderRef: 'order.site_otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v2',
  sourceManifestDigest: 'sha256:site_otec_manifest',
  status: 'active',
  updatedAt: '2026-06-05T17:56:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

const routes = makeSiteCommerceRoutes({
  authorizePaidActionAgent: async () => true,
  challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
  checkoutCatalog: sitePaymentCatalog,
  nowEpochMillis: () => fixedNow.getTime(),
  nowIso: () => fixedNow.toISOString(),
})

const makeRequest = (
  path: string,
  input: Readonly<{
    authorization?: string
    body?: unknown
    idempotencyKey?: string
    method?: string
  }> = {},
) =>
  new Request(`https://openagents.com${path}`, {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      ...(input.idempotencyKey === undefined
        ? {}
        : { 'idempotency-key': input.idempotencyKey }),
      ...(input.authorization === undefined
        ? {}
        : { authorization: input.authorization }),
      ...(input.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
    },
    method: input.method ?? 'POST',
  })

type SmokeL402RoutePrice = Readonly<{
  amount: number
  asset: 'credits' | 'sats' | 'usd'
}>

type SmokeL402ChallengeBody = Readonly<{
  entitlementScope: 'account' | 'action' | 'path' | 'product' | 'site'
  method: 'GET' | 'POST'
  paidActionId: string
  path: string
  price: SmokeL402RoutePrice
  spendCap: SmokeL402RoutePrice
}>

type SmokeL402RedemptionBody = Readonly<{
  challengeExpiresAt: string
  challengeId: string
  credentialId: string
  entitlementScope: 'account' | 'action' | 'path' | 'product' | 'site'
  method: 'GET' | 'POST'
  paidActionId: string
  path: string
  paymentProofRef: string
  price: SmokeL402RoutePrice
}>

const routeRequest = async (request: Request): Promise<Response> => {
  const routed = routes.routeSiteCommerceRequest(request)

  if (routed === undefined) {
    throw new Error('Expected Site commerce route to match.')
  }

  return Effect.runPromise(routed)
}

const routeWith = async (
  inputRoutes: ReturnType<typeof makeSiteCommerceRoutes>,
  request: Request,
): Promise<Response> => {
  const routed = inputRoutes.routeSiteCommerceRequest(request)

  if (routed === undefined) {
    throw new Error('Expected Site commerce route to match.')
  }

  return Effect.runPromise(routed)
}

class MemoryBuyerPaymentLedgerStore implements BuyerPaymentLedgerStore {
  challenges = new Map<string, BuyerPaymentChallengeRecord>()
  entitlements = new Map<string, BuyerPaymentEntitlementRecord>()
  receipts = new Map<string, BuyerPaymentReceiptRecord>()
  reconciliations = new Map<string, BuyerPaymentReconciliationEventRecord>()
  redemptions = new Map<string, BuyerPaymentRedemptionRecord>()

  createChallenge = async (record: BuyerPaymentChallengeRecord) => {
    this.challenges.set(record.idempotencyKeyHash, record)
  }

  createCreditDebit = async (_record: BuyerPaymentCreditDebitRecord) => {}

  createReceiptEntitlementBundle = async (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
  }) => {
    this.receipts.set(input.receipt.receiptRef, input.receipt)
    this.entitlements.set(input.entitlement.entitlementRef, input.entitlement)
  }

  createReconciliationEvent = async (
    record: BuyerPaymentReconciliationEventRecord,
  ) => {
    this.reconciliations.set(
      `${record.providerRef}:${record.externalEventRef}`,
      record,
    )
  }

  createRedemptionBundle = async (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
    redemption: BuyerPaymentRedemptionRecord
  }) => {
    await this.createReceiptEntitlementBundle(input)
    this.redemptions.set(input.redemption.challengeRef, input.redemption)
  }

  createSpendLimit = async (_record: BuyerPaymentSpendLimitRecord) => {}

  readChallengeByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.challenges.get(idempotencyKeyHash)

  readEntitlementByRef = async (entitlementRef: string) =>
    this.entitlements.get(entitlementRef)

  readReceiptByRef = async (receiptRef: string) => this.receipts.get(receiptRef)

  readReconciliationEventByReceiptRef = async (receiptRef: string) =>
    [...this.reconciliations.values()].find(
      record => record.receiptRef === receiptRef,
    )

  readReconciliationEventByProviderEvent = async (
    providerRef: string,
    externalEventRef: string,
  ) => this.reconciliations.get(`${providerRef}:${externalEventRef}`)

  readRedemptionByChallengeRef = async (challengeRef: string) =>
    this.redemptions.get(challengeRef)
}

class MemoryCheckoutIntentStore implements OpenAgentsSiteMdkCheckoutIntentStore {
  bundles: OpenAgentsSiteMdkCheckoutIntentBundle[] = []
  intentsByCheckoutRef = new Map<
    string,
    OpenAgentsSiteMdkCheckoutIntentRecord
  >()
  intentsByIntentRef = new Map<string, OpenAgentsSiteMdkCheckoutIntentRecord>()

  constructor(private readonly ledger?: MemoryBuyerPaymentLedgerStore) {}

  createCheckoutIntentBundle = async (
    bundle: OpenAgentsSiteMdkCheckoutIntentBundle,
  ) => {
    this.bundles.push(bundle)
    this.intentsByCheckoutRef.set(
      bundle.checkoutIntent.checkoutRef,
      bundle.checkoutIntent,
    )
    this.intentsByIntentRef.set(
      bundle.checkoutIntent.checkoutIntentRef,
      bundle.checkoutIntent,
    )
    await this.ledger?.createChallenge(bundle.buyerPaymentChallenge)
  }

  readCheckoutIntentByCheckoutRef = async (checkoutRef: string) =>
    this.intentsByCheckoutRef.get(checkoutRef)

  readCheckoutIntentByIntentRef = async (checkoutIntentRef: string) =>
    this.intentsByIntentRef.get(checkoutIntentRef)

  updateCheckoutIntentStatus = async (input: {
    checkoutRef: string
    hostedCheckoutProjectionJson: string
    publicProjectionJson: string
    status: OpenAgentsSiteMdkCheckoutIntentRecord['status']
    updatedAt: string
  }) => {
    const current = this.intentsByCheckoutRef.get(input.checkoutRef)

    if (current === undefined) {
      return undefined
    }

    const next = {
      ...current,
      hostedCheckoutProjectionJson: input.hostedCheckoutProjectionJson,
      publicProjectionJson: input.publicProjectionJson,
      status: input.status,
      updatedAt: input.updatedAt,
    }

    this.intentsByCheckoutRef.set(next.checkoutRef, next)
    this.intentsByIntentRef.set(next.checkoutIntentRef, next)

    return next
  }
}

class MemorySiteCommerceReviewStore implements OpenAgentsSiteCommerceReviewStore {
  decisionsByCatalog = new Map<
    string,
    OpenAgentsSiteCommerceReviewDecisionRecord
  >()
  decisionsByIdempotency = new Map<
    string,
    OpenAgentsSiteCommerceReviewDecisionRecord
  >()

  listReviewDecisionsForSite = async (siteId: string) =>
    [...this.decisionsByCatalog.values()].filter(
      decision => decision.siteId === siteId && decision.archivedAt === null,
    )

  readReviewDecisionByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.decisionsByIdempotency.get(idempotencyKeyHash)

  upsertReviewDecision = async (
    decision: OpenAgentsSiteCommerceReviewDecisionRecord,
  ) => {
    this.decisionsByIdempotency.set(decision.idempotencyKeyHash, decision)
    this.decisionsByCatalog.set(
      [decision.siteId, decision.siteVersionId, decision.catalogRef].join(':'),
      decision,
    )

    return decision
  }
}

class MemorySiteMdkAccountBindingStore implements OpenAgentsSiteMdkAccountBindingStore {
  bindingsByIdempotency = new Map<
    string,
    OpenAgentsSiteMdkAccountBindingRecord
  >()
  bindingsByRef = new Map<string, OpenAgentsSiteMdkAccountBindingRecord>()

  listBindingsForSite = async (siteId: string) =>
    [...this.bindingsByRef.values()].filter(
      binding => binding.siteId === siteId && binding.archivedAt === null,
    )

  readBindingByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.bindingsByIdempotency.get(idempotencyKeyHash)

  upsertBinding = async (binding: OpenAgentsSiteMdkAccountBindingRecord) => {
    this.bindingsByIdempotency.set(binding.idempotencyKeyHash, binding)
    this.bindingsByRef.set(`${binding.siteId}:${binding.bindingRef}`, binding)

    return binding
  }
}

class MemoryPayoutLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
    this.attempts.set(record.payoutAttemptRef, record)
  }

  createPayoutIntent = async (record: NexusTreasuryPayoutIntentRecord) => {
    this.intents.set(record.payoutIntentRef, record)
    this.intentsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutTargetApproval = async (
    _record: NexusPayoutTargetApprovalRecord,
  ) => {}

  createPaymentAuthorityReceipt = async (
    record: NexusPaymentAuthorityReceiptRecord,
  ) => {
    this.receipts.set(record.receiptRef, record)
  }

  createReconciliationEvent = async (
    record: NexusTreasuryPayoutReconciliationEventRecord,
  ) => {
    this.events.set(record.eventRef, record)
  }

  createReleaseGate = async (_record: NexusReleaseGateRecord) => {}

  readPayoutAttemptByRef = async (payoutAttemptRef: string) =>
    this.attempts.get(payoutAttemptRef)

  readPayoutAttemptByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    [...this.attempts.values()].find(
      attempt => attempt.idempotencyKeyHash === idempotencyKeyHash,
    )

  readPayoutIntentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.intentsByIdempotency.get(idempotencyKeyHash)

  readPayoutIntentByBuyerPaymentRef = async (buyerPaymentRef: string) =>
    [...this.intents.values()].find(
      intent => intent.buyerPaymentRef === buyerPaymentRef,
    )

  readPayoutIntentByRef = async (payoutIntentRef: string) =>
    this.intents.get(payoutIntentRef)
  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPaymentAuthorityReceiptByRef = async (receiptRef: string) =>
    this.receipts.get(receiptRef)

  readReconciliationEventByRef = async (eventRef: string) =>
    this.events.get(eventRef)
}

const encoder = new TextEncoder()

const base64 = (bytes: ArrayBuffer): string =>
  btoa(String.fromCharCode(...new Uint8Array(bytes)))

const signStandardWebhook = async (
  secret: string,
  webhookId: string,
  timestamp: string,
  body: string,
): Promise<string> => {
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const digest = await crypto.subtle.sign(
    'HMAC',
    key,
    encoder.encode(`${webhookId}.${timestamp}.${body}`),
  )

  return `v1,${base64(digest)}`
}

const validCheckoutBody = {
  cancelReturnPath: '/pricing',
  customerDataRefs: ['email'],
  expectedPrice: {
    amountMinorUnits: 2500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  itemKind: 'product',
  productId: 'consultation_deposit',
  siteVersionId: 'version_site_otec_v2',
  successReturnPath: '/checkout/thanks',
} as const

const makeApprovedMdkAccountBindingRecord = (
  input: Partial<OpenAgentsSiteMdkAccountBindingRecord> = {},
): OpenAgentsSiteMdkAccountBindingRecord => ({
  allowedActionRefs: [],
  allowedCatalogRefs: [sitePaymentCatalog.items[0]!.catalogRef],
  allowedProductRefs: ['consultation_deposit'],
  archivedAt: null,
  bindingRef: 'site_mdk_account:site_otec:customer_wallet',
  caveatRefs: ['caveat.site_mdk_account.binding_reviewed'],
  createdAt: fixedNow.toISOString(),
  customerRef: 'customer.site_otec',
  environment: 'sandbox',
  id: 'site_mdk_account_binding_site_otec_customer_wallet',
  idempotencyKeyHash: 'hash.site_mdk_account.site_otec.customer_wallet',
  orderRef: 'order.site_otec',
  publicProjectionJson: '{}',
  requestedProviderMode: 'customer_owned_mdk',
  reviewStatus: 'approved',
  reviewerRefs: ['operator.site_mdk_account'],
  secretBindingRefs: ['hosted_secret.site_mdk_account.site_otec.mdk'],
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v2',
  updatedAt: fixedNow.toISOString(),
  ...input,
})

const validChallengeBody = {
  entitlementScope: 'action',
  method: 'POST',
  paidActionId: 'generate-report',
  path: '/api/actions/generate-report',
  price: {
    amount: 1200,
    asset: 'sats',
  },
  spendCap: {
    amount: 1200,
    asset: 'sats',
  },
} as const

const responseTextIncludesProhibitedPaymentMaterial = async (
  response: Response,
): Promise<boolean> =>
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key)/i.test(
    await response.clone().text(),
  )

describe('Site commerce hosted contracts', () => {
  test('serves agent-readable payment discovery without idempotency or private material', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/discovery', {
        method: 'GET',
      }),
    )
    const responseText = await response.text()
    const body = JSON.parse(responseText) as {
      siteCommerce: {
        action: string
        discovery: {
          endpoints: {
            checkoutReturn: string
            checkoutIntent: string
            commerceReview: string
            commerceReviewDecision: string
            l402Challenge: string
            l402Redemption: string
            mdkAccountBinding: string
            mdkAccountBindingReview: string
            payoutBridge: string
            paymentProof: string
            providerEventReconcile: string
          }
          items: ReadonlyArray<{
            catalogRef: string
            itemKind: string
            l402ChallengeEndpoint: string | null
            metadataRefs: ReadonlyArray<string>
            spendCapHintRefs: ReadonlyArray<string>
          }>
          surfaceStates: Record<string, string>
        }
      }
    }

    expect(response.status).toBe(200)
    expect(body.siteCommerce.action).toBe('payment_discovery_read')
    expect(body.siteCommerce.discovery.endpoints).toEqual({
      checkoutIntent: '/api/sites/site_otec/commerce/checkout-intents',
      checkoutReturn:
        '/api/sites/site_otec/commerce/checkout-returns/{checkoutIntentRef}/{returnAction}',
      commerceReview: '/api/sites/site_otec/commerce/review',
      commerceReviewDecision: '/api/sites/site_otec/commerce/review-decisions',
      l402Challenge: '/api/sites/site_otec/commerce/l402/challenges',
      l402Redemption: '/api/sites/site_otec/commerce/l402/redemptions',
      mdkAccountBinding: '/api/sites/site_otec/commerce/mdk-account-binding',
      mdkAccountBindingReview:
        '/api/sites/site_otec/commerce/mdk-account-bindings',
      payoutBridge: '/api/sites/site_otec/commerce/payout-bridges',
      paymentProof:
        '/api/sites/site_otec/commerce/payment-proofs/{checkoutIntentRef}',
      providerEventReconcile: '/api/sites/site_otec/commerce/mdk/webhooks',
    })
    expect(body.siteCommerce.discovery.surfaceStates).toMatchObject({
      checkoutIntent: 'gated',
      checkoutReturn: 'available',
      commerceReview: 'available',
      commerceReviewDecision: 'gated',
      entitlementProjection: 'available',
      mdkAccountBinding: 'available',
      mdkAccountBindingReview: 'gated',
      payoutBridge: 'gated',
      paymentProof: 'available',
      providerEventReconciliation: 'gated',
      settlement: 'gated',
      wfpMiddleware: 'available_contract',
    })
    expect(body.siteCommerce.discovery.items).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          itemKind: 'product',
          l402ChallengeEndpoint: null,
          spendCapHintRefs: ['spend_cap.usd.product'],
        }),
        expect.objectContaining({
          itemKind: 'paid_action',
          l402ChallengeEndpoint:
            '/api/sites/site_otec/commerce/l402/challenges',
          metadataRefs: [
            'metadata.site_payment.site_otec.v2',
            'metadata.action.download_report',
          ],
          spendCapHintRefs: ['spend_cap.bitcoin.action'],
        }),
      ]),
    )
    expect(
      /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key)/i.test(
        responseText,
      ),
    ).toBe(false)
  })

  test('serves Site commerce review without idempotency or raw review internals', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/review', {
        method: 'GET',
      }),
    )
    const responseText = await response.text()
    const body = JSON.parse(responseText) as {
      siteCommerce: {
        action: string
        review: {
          items: ReadonlyArray<{
            itemKind: string
            paymentAuthorityCreated: boolean
            reviewStatus: string
            sourceSafeCheckoutPrimitiveRefs: ReadonlyArray<string>
          }>
          redaction: {
            exposesRawTimestamps: boolean
          }
          sourceSafety: {
            usesHostedOmegaPaymentBoundary: boolean
          }
        }
      }
    }

    expect(response.status).toBe(200)
    expect(body.siteCommerce.action).toBe('commerce_review_read')
    expect(body.siteCommerce.review.items).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          itemKind: 'product',
          paymentAuthorityCreated: false,
          reviewStatus: 'needs_review',
          sourceSafeCheckoutPrimitiveRefs: expect.arrayContaining([
            'site_checkout_ui:site_otec:version_site_otec_v2:product_card:consultation_deposit',
          ]),
        }),
        expect.objectContaining({
          itemKind: 'paid_action',
          reviewStatus: 'needs_review',
        }),
      ]),
    )
    expect(body.siteCommerce.review.redaction.exposesRawTimestamps).toBe(false)
    expect(
      body.siteCommerce.review.sourceSafety.usesHostedOmegaPaymentBoundary,
    ).toBe(true)
    expect(responseText).not.toMatch(
      /(2026-\d{2}-\d{2}T|checkout_id=|lnbc|mdk_access_token|mnemonic|wallet_secret)/i,
    )
  })

  test('persists idempotent operator Site commerce review decisions', async () => {
    const reviewStore = new MemorySiteCommerceReviewStore()
    const routesWithReview = makeSiteCommerceRoutes({
      authorizeCommerceReviewDecision: async request =>
        request.headers.get('authorization') === 'Bearer operator-token',
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
      reviewStore,
    })
    const catalogRef = sitePaymentCatalog.items[0]!.catalogRef
    const unauthorized = await routeWith(
      routesWithReview,
      makeRequest('/api/sites/site_otec/commerce/review-decisions', {
        body: {
          catalogRef,
          reviewStatus: 'accepted',
        },
        idempotencyKey: 'review-decision-unauthorized',
      }),
    )
    const accepted = await routeWith(
      routesWithReview,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/review-decisions',
        {
          body: JSON.stringify({
            catalogRef,
            reasonRefs: ['reason.site_commerce_review.catalog_ok'],
            reviewStatus: 'accepted',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'review-decision-1',
            'x-openagents-actor-ref': 'operator.site_builder',
          },
          method: 'POST',
        },
      ),
    )
    const acceptedPayload = await accepted.json()
    const replay = await routeWith(
      routesWithReview,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/review-decisions',
        {
          body: JSON.stringify({
            catalogRef,
            reasonRefs: ['reason.site_commerce_review.catalog_ok'],
            reviewStatus: 'accepted',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'review-decision-1',
          },
          method: 'POST',
        },
      ),
    )
    const review = await routeWith(
      routesWithReview,
      makeRequest('/api/sites/site_otec/commerce/review', {
        method: 'GET',
      }),
    )
    const reviewPayload = await review.json()

    expect(unauthorized.status).toBe(401)
    expect(accepted.status).toBe(201)
    expect(acceptedPayload).toMatchObject({
      siteCommerce: {
        decision: {
          paymentAuthorityCreated: false,
          payoutAuthorityCreated: false,
          reviewStatus: 'accepted',
        },
        duplicate: false,
        review: {
          reviewStatusCounts: {
            accepted: 1,
            needsReview: 1,
          },
        },
      },
    })
    expect(replay.status).toBe(200)
    await expect(replay.json()).resolves.toMatchObject({
      siteCommerce: {
        duplicate: true,
      },
    })
    expect(reviewPayload).toMatchObject({
      siteCommerce: {
        review: {
          items: [
            expect.objectContaining({
              reviewStatus: 'accepted',
            }),
            expect.objectContaining({
              reviewStatus: 'needs_review',
            }),
          ],
        },
      },
    })
    expect(JSON.stringify(acceptedPayload)).not.toMatch(
      /(2026-\d{2}-\d{2}T|checkout_id=|lnbc|mdk_access_token|mnemonic|wallet_secret)/i,
    )
  })

  test('reads and writes customer-owned MDK account binding state safely', async () => {
    const bindingStore = new MemorySiteMdkAccountBindingStore()
    const routesWithBindings = makeSiteCommerceRoutes({
      authorizeMdkAccountBinding: async request =>
        request.headers.get('authorization') === 'Bearer operator-token',
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      mdkAccountBindingStore: bindingStore,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })
    const initial = await routeWith(
      routesWithBindings,
      makeRequest('/api/sites/site_otec/commerce/mdk-account-binding', {
        method: 'GET',
      }),
    )
    const unauthorized = await routeWith(
      routesWithBindings,
      makeRequest('/api/sites/site_otec/commerce/mdk-account-bindings', {
        body: {
          customerRef: 'customer.site_otec',
          environment: 'sandbox',
          orderRef: 'order.site_otec',
          requestedProviderMode: 'customer_owned_mdk',
          reviewStatus: 'approved',
          secretBindingRefs: ['hosted_secret.site_mdk_account.site_otec.mdk'],
          siteVersionId: 'version_site_otec_v2',
        },
        idempotencyKey: 'mdk-binding-unauthorized',
      }),
    )
    const approved = await routeWith(
      routesWithBindings,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk-account-bindings',
        {
          body: JSON.stringify({
            allowedCatalogRefs: [sitePaymentCatalog.items[0]!.catalogRef],
            allowedProductRefs: ['consultation_deposit'],
            bindingRef: 'site_mdk_account:site_otec:customer_wallet',
            caveatRefs: ['caveat.site_mdk_account.binding_reviewed'],
            customerRef: 'customer.site_otec',
            environment: 'sandbox',
            orderRef: 'order.site_otec',
            requestedProviderMode: 'customer_owned_mdk',
            reviewStatus: 'approved',
            reviewerRefs: ['operator.site_mdk_account'],
            secretBindingRefs: ['hosted_secret.site_mdk_account.site_otec.mdk'],
            siteVersionId: 'version_site_otec_v2',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'mdk-binding-1',
          },
          method: 'POST',
        },
      ),
    )
    const approvedPayload = await approved.json()
    const customerRead = await routeWith(
      routesWithBindings,
      makeRequest('/api/sites/site_otec/commerce/mdk-account-binding', {
        method: 'GET',
      }),
    )
    const customerPayload = await customerRead.json()
    const unsafe = await routeWith(
      routesWithBindings,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk-account-bindings',
        {
          body: JSON.stringify({
            customerRef: 'customer.site_otec',
            environment: 'sandbox',
            orderRef: 'order.site_otec',
            requestedProviderMode: 'customer_owned_mdk',
            reviewStatus: 'approved',
            secretBindingRefs: ['MDK_ACCESS_TOKEN=re_secret'],
            siteVersionId: 'version_site_otec_v2',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'mdk-binding-unsafe',
          },
          method: 'POST',
        },
      ),
    )

    expect(initial.status).toBe(200)
    await expect(initial.json()).resolves.toMatchObject({
      siteCommerce: {
        mdkAccountBinding: {
          bindingState: 'unavailable',
          providerMode: 'openagents_hosted_mdk',
        },
      },
    })
    expect(unauthorized.status).toBe(401)
    expect(approved.status).toBe(201)
    expect(approvedPayload).toMatchObject({
      siteCommerce: {
        mdkAccountBinding: {
          bindingState: 'configured',
          providerMode: 'customer_owned_mdk',
          secretBindingRefs: ['hosted_secret.site_mdk_account.site_otec.mdk'],
          secretBindingState: 'hosted_secret_refs_present',
        },
      },
    })
    expect(customerRead.status).toBe(200)
    expect(customerPayload).toMatchObject({
      siteCommerce: {
        mdkAccountBinding: {
          bindingState: 'configured',
          customerRef: null,
          providerMode: 'customer_owned_mdk',
          secretBindingRefs: [],
          secretBindingState: 'redacted',
        },
      },
    })
    expect(unsafe.status).toBe(400)
    expect(JSON.stringify(customerPayload)).not.toMatch(
      /(customer\\.site_otec|MDK_ACCESS_TOKEN|mnemonic|lnbc|wallet_secret)/i,
    )
  })

  test('requires idempotency keys for checkout intent creation', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
      }),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'idempotency_key_required',
      message: 'Site commerce actions require Idempotency-Key.',
    })
  })

  test('rejects missing catalog membership and price mismatches', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: {
          ...validCheckoutBody,
          expectedPrice: {
            amountMinorUnits: 1,
            asset: 'usd',
            denomination: 'usd_cent',
          },
        },
        idempotencyKey: 'checkout-invalid-product',
      }),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'invalid_site_commerce_contract',
      message: 'expectedPrice must match the Site payment catalog price.',
    })

    const missing = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: {
          ...validCheckoutBody,
          productId: 'unknown_product',
        },
        idempotencyKey: 'checkout-missing-product',
      }),
    )

    expect(missing.status).toBe(400)
    await expect(missing.json()).resolves.toEqual({
      error: 'invalid_site_commerce_contract',
      message:
        'checkout intent does not match an active Site payment catalog item.',
    })
  })

  test('rejects missing customer data refs without accepting private values', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: {
          ...validCheckoutBody,
          customerDataRefs: [],
        },
        idempotencyKey: 'checkout-missing-customer-ref',
      }),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'invalid_site_commerce_contract',
      message: 'customerDataRefs must include required field email.',
    })
  })

  test('returns redacted hosted checkout intent projections and deterministic replay-safe refs', async () => {
    const request = makeRequest(
      '/api/sites/site_otec/commerce/checkout-intents',
      {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-replay-1',
      },
    )
    const first = await routeRequest(request)
    const second = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-replay-1',
      }),
    )
    const firstPayload = (await first.json()) as {
      siteCommerce: {
        buyerPaymentChallenge: {
          challengeRef: string | null
          publicProjectionJson: string
        }
        checkoutIntent: {
          cancelReturnPath: string
          checkoutLaunchPath: string
          checkoutUrlRef: string
          hostedCheckout: {
            checkoutLaunchPath: string
            checkoutUrlRef: string
            invoiceRef: string | null
            paymentHashRef: string | null
          }
          successReturnPath: string
        }
      }
    }
    const secondPayload = await second.json()

    expect(first.status).toBe(201)
    expect(firstPayload).toEqual(secondPayload)
    expect(firstPayload).toMatchObject({
      siteCommerce: {
        buyerPaymentChallenge: {
          challengeRef: 'challenge.site_checkout.site_otec.checkout-replay-1',
        },
        checkoutIntent: {
          cancelReturnPath: '/pricing',
          checkoutLaunchPath:
            '/checkout/site_payment_site_otec_version_site_otec_v2_product_consultation_deposit_hash_site_checkout_site_otec_checkout-replay-1',
          hostedCheckout: {
            checkoutLaunchPath:
              '/checkout/site_payment_site_otec_version_site_otec_v2_product_consultation_deposit_hash_site_checkout_site_otec_checkout-replay-1',
            invoiceRef: null,
            paymentHashRef: null,
          },
          provider: 'openagents_hosted_mdk',
          state: 'created',
          successReturnPath: '/checkout/thanks',
        },
        implementationState: 'fake_provider_contract',
        idempotency: {
          key: 'checkout-replay-1',
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
    })
    expect(
      firstPayload.siteCommerce.checkoutIntent.successReturnPath,
    ).not.toContain('?')
    expect(
      firstPayload.siteCommerce.checkoutIntent.cancelReturnPath,
    ).not.toContain('?')
    expect(
      firstPayload.siteCommerce.checkoutIntent.hostedCheckout.checkoutUrlRef,
    ).toBe(firstPayload.siteCommerce.checkoutIntent.checkoutUrlRef)
    expect(
      firstPayload.siteCommerce.checkoutIntent.hostedCheckout
        .checkoutLaunchPath,
    ).toBe(firstPayload.siteCommerce.checkoutIntent.checkoutLaunchPath)
    expect(
      firstPayload.siteCommerce.checkoutIntent.checkoutLaunchPath,
    ).not.toContain('?')
    expect(
      firstPayload.siteCommerce.checkoutIntent.checkoutUrlRef.startsWith(
        'mdk_checkout_url.site_payment_site_otec_version_site_otec_v2_product_consultation_deposit_hash_site_checkout_site',
      ),
    ).toBe(true)
    expect(
      firstPayload.siteCommerce.checkoutIntent.hostedCheckout.invoiceRef,
    ).toBe(null)
    expect(
      firstPayload.siteCommerce.checkoutIntent.hostedCheckout.paymentHashRef,
    ).toBe(null)
    expect(
      firstPayload.siteCommerce.buyerPaymentChallenge.publicProjectionJson,
    ).not.toContain('order.site_otec')
  })

  test('persists checkout challenge and provider intent when a store is injected', async () => {
    const bundles: OpenAgentsSiteMdkCheckoutIntentBundle[] = []
    const store: OpenAgentsSiteMdkCheckoutIntentStore = {
      createCheckoutIntentBundle: async bundle => {
        bundles.push(bundle)
      },
      readCheckoutIntentByCheckoutRef: async () => undefined,
      readCheckoutIntentByIntentRef: async () => undefined,
      updateCheckoutIntentStatus: async () => undefined,
    }
    const routesWithStore = makeSiteCommerceRoutes({
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      checkoutIntentStore: store,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })
    const response = await routeWith(
      routesWithStore,
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-persist-1',
      }),
    )

    expect(response.status).toBe(201)
    expect(bundles).toHaveLength(1)
    expect(bundles[0]).toMatchObject({
      buyerPaymentChallenge: {
        challengeRef: 'challenge.site_checkout.site_otec.checkout-persist-1',
        surface: 'site_checkout',
      },
      checkoutIntent: {
        checkoutIntentRef: 'site_checkout_intent_site_otec_checkout-persist-1',
        checkoutLaunchPath:
          '/checkout/site_payment_site_otec_version_site_otec_v2_product_consultation_deposit_hash_site_checkout_site_otec_checkout-persist-1',
        checkoutRef:
          'mdk_checkout.site_payment_site_otec_version_site_otec_v2_product_consultation_deposit_hash_site_checkout_site_otec_checkout-persist-1',
        idempotencyKeyHash: 'hash.site_checkout.site_otec.checkout-persist-1',
        providerRef: 'provider.openagents.hosted_mdk.fake',
        status: 'created',
      },
    })
    expect(bundles[0]?.checkoutIntent.publicProjectionJson).toContain(
      '"checkoutStatus":"created"',
    )
    expect(JSON.stringify(bundles[0])).not.toMatch(
      /(lnbc|mnemonic|mdk_access_token|payment_preimage|wallet_secret)/i,
    )
  })

  test('runs generated Site human checkout fixture through commerce APIs without payment verification', async () => {
    const generatedCatalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const generatedCheckoutPlan =
      exampleOpenAgentsGeneratedSitePaymentHelperPlans().find(
        plan => plan.helperKind === 'checkout_intent_create',
      )
    const ledger = new MemoryBuyerPaymentLedgerStore()
    const checkoutStore = new MemoryCheckoutIntentStore(ledger)
    const generatedRoutes = makeSiteCommerceRoutes({
      buyerPaymentLedgerStore: ledger,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: generatedCatalog,
      checkoutIntentStore: checkoutStore,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })

    expect(generatedCheckoutPlan).toBeDefined()

    const discovery = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/discovery', {
        method: 'GET',
      }),
    )
    const checkout = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/checkout-intents', {
        body: generatedCheckoutPlan?.body,
        idempotencyKey:
          generatedCheckoutPlan?.idempotencyKey ??
          'generated-site-payment-smoke-checkout',
      }),
    )
    const checkoutPayload = (await checkout.json()) as {
      siteCommerce: {
        buyerPaymentChallenge: {
          challengeRef: string | null
          status: string
        }
        checkoutIntent: {
          cancelReturnPath: string
          hostedCheckout: {
            invoiceRef: string | null
            paymentHashRef: string | null
          }
          id: string
          state: string
          successReturnPath: string
        }
        redaction: {
          exposesCustomerPrivateData: boolean
          exposesMdkCredentials: boolean
          exposesProviderGrants: boolean
          exposesRawInvoice: boolean
          exposesWalletMaterial: boolean
        }
      }
    }
    const returnStatus = await routeWith(
      generatedRoutes,
      makeRequest(
        `/api/sites/site_payment_smoke/commerce/checkout-returns/${checkoutPayload.siteCommerce.checkoutIntent.id}/status`,
        { method: 'GET' },
      ),
    )
    const discoveryPayload = await discovery.json()
    const returnPayload = await returnStatus.json()

    expect(discovery.status).toBe(200)
    expect(discoveryPayload).toMatchObject({
      siteCommerce: {
        discovery: {
          agentReadable: true,
          items: [
            expect.objectContaining({
              itemKind: 'product',
              productId: 'human_brief_checkout',
            }),
            expect.objectContaining({
              actionId: 'agent_research_note',
              itemKind: 'paid_action',
            }),
          ],
        },
      },
    })
    expect(checkout.status).toBe(201)
    expect(checkoutPayload).toMatchObject({
      siteCommerce: {
        buyerPaymentChallenge: {
          challengeRef:
            'challenge.site_checkout.site_payment_smoke.generated-site-payment-smoke-checkout',
          status: 'issued',
        },
        checkoutIntent: {
          cancelReturnPath: '/checkout/cancel',
          hostedCheckout: {
            invoiceRef: null,
            paymentHashRef: null,
          },
          state: 'created',
          successReturnPath: '/checkout/complete',
        },
        redaction: {
          exposesCustomerPrivateData: false,
          exposesMdkCredentials: false,
          exposesProviderGrants: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    })
    expect(checkoutStore.bundles).toHaveLength(1)
    expect(ledger.challenges.size).toBe(1)
    expect(ledger.receipts.size).toBe(0)
    expect(ledger.entitlements.size).toBe(0)
    expect(
      checkoutStore.intentsByIntentRef.get(
        checkoutPayload.siteCommerce.checkoutIntent.id,
      )?.status,
    ).toBe('created')
    expect(returnStatus.status).toBe(200)
    expect(returnPayload).toMatchObject({
      siteCommerce: {
        action: 'checkout_return_read',
        returnProjection: {
          cleanReturnPath: `/commerce/checkout-returns/${checkoutPayload.siteCommerce.checkoutIntent.id}/status`,
          entitlementStatus: 'none',
          finalEntitlementCreated: false,
          returnState: 'unpaid',
        },
      },
    })
    expect(
      JSON.stringify([discoveryPayload, checkoutPayload, returnPayload]),
    ).not.toMatch(
      /(checkout_id=|lnbc|lntb|lno1|mdk_access_token|mnemonic|payment_hash|payment_preimage|preimage=|provider_grant|wallet_secret|raw_customer_email)/i,
    )
  })

  test('runs generated Site checkout reconciliation through exact-source MDK webhook smoke', async () => {
    const generatedCatalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const generatedCheckoutPlan =
      exampleOpenAgentsGeneratedSitePaymentHelperPlans().find(
        plan => plan.helperKind === 'checkout_intent_create',
      )
    const ledger = new MemoryBuyerPaymentLedgerStore()
    const checkoutStore = new MemoryCheckoutIntentStore(ledger)
    const generatedRoutes = makeSiteCommerceRoutes({
      buyerPaymentLedgerStore: ledger,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: generatedCatalog,
      checkoutIntentStore: checkoutStore,
      mdkWebhookConfig: {
        bindingRef:
          'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
        secret: 'generated-site-mdk-webhook-secret',
        source: 'dashboard_standard_webhooks',
      },
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })

    expect(generatedCheckoutPlan).toBeDefined()

    if (
      generatedCheckoutPlan === undefined ||
      !isRecord(generatedCheckoutPlan.body) ||
      generatedCheckoutPlan.idempotencyKey === null
    ) {
      throw new Error('Generated Site checkout fixture is incomplete.')
    }

    const checkout = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/checkout-intents', {
        body: generatedCheckoutPlan.body,
        idempotencyKey: generatedCheckoutPlan.idempotencyKey,
      }),
    )
    const checkoutPayload = (await checkout.json()) as {
      siteCommerce: {
        checkoutIntent: {
          hostedCheckout: {
            checkoutRef: string
          }
          id: string
          state: string
        }
      }
    }
    const checkoutIntentRef = checkoutPayload.siteCommerce.checkoutIntent.id
    const providerCheckoutId =
      checkoutPayload.siteCommerce.checkoutIntent.hostedCheckout.checkoutRef.replace(
        /^mdk_checkout\./u,
        '',
      )
    const beforeReturn = await routeWith(
      generatedRoutes,
      makeRequest(
        `/api/sites/site_payment_smoke/commerce/checkout-returns/${checkoutIntentRef}/status`,
        { method: 'GET' },
      ),
    )
    const beforeProof = await routeWith(
      generatedRoutes,
      makeRequest(
        `/api/sites/site_payment_smoke/commerce/payment-proofs/${checkoutIntentRef}`,
        { method: 'GET' },
      ),
    )
    const webhookBody = JSON.stringify({
      createdAt: '2026-06-05T18:01:00.000Z',
      data: {
        checkout: {
          id: providerCheckoutId,
          status: 'PAYMENT_RECEIVED',
        },
      },
      id: 'evt_generated_site_payment_smoke_paid_1',
      type: 'checkout.completed',
    })
    const webhookTimestamp = '1780682460'
    const webhookSignature = await signStandardWebhook(
      'generated-site-mdk-webhook-secret',
      'evt_generated_site_payment_smoke_paid_1',
      webhookTimestamp,
      webhookBody,
    )
    const webhook = await routeWith(
      generatedRoutes,
      new Request(
        'https://openagents.com/api/sites/site_payment_smoke/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_generated_site_payment_smoke_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const replay = await routeWith(
      generatedRoutes,
      new Request(
        'https://openagents.com/api/sites/site_payment_smoke/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_generated_site_payment_smoke_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const afterReturn = await routeWith(
      generatedRoutes,
      makeRequest(
        `/api/sites/site_payment_smoke/commerce/checkout-returns/${checkoutIntentRef}/success`,
        { method: 'GET' },
      ),
    )
    const afterProof = await routeWith(
      generatedRoutes,
      makeRequest(
        `/api/sites/site_payment_smoke/commerce/payment-proofs/${checkoutIntentRef}`,
        { method: 'GET' },
      ),
    )
    const beforeReturnPayload = await beforeReturn.json()
    const beforeProofPayload = await beforeProof.json()
    const webhookPayload = await webhook.json()
    const replayPayload = await replay.json()
    const afterReturnPayload = await afterReturn.json()
    const afterProofPayload = await afterProof.json()
    const publicEvidenceText = JSON.stringify([
      beforeReturnPayload,
      beforeProofPayload,
      webhookPayload,
      replayPayload,
      afterReturnPayload,
      afterProofPayload,
    ])

    expect(checkout.status).toBe(201)
    expect(checkoutPayload.siteCommerce.checkoutIntent.state).toBe('created')
    expect(beforeReturn.status).toBe(200)
    expect(beforeProof.status).toBe(200)
    expect(beforeReturnPayload).toMatchObject({
      siteCommerce: {
        action: 'checkout_return_read',
        returnProjection: {
          entitlementStatus: 'none',
          returnState: 'unpaid',
        },
      },
    })
    expect(beforeProofPayload).toMatchObject({
      siteCommerce: {
        action: 'payment_proof_read',
        paymentProof: {
          claimState: 'checkout_intent_recorded',
          entitlementState: 'none',
          proofState: 'pending_checkout',
        },
      },
    })
    expect(webhook.status).toBe(202)
    expect(replay.status).toBe(200)
    expect(afterReturn.status).toBe(200)
    expect(afterProof.status).toBe(200)
    expect(
      checkoutStore.intentsByIntentRef.get(checkoutIntentRef)?.status,
    ).toBe('payment_received')
    expect(ledger.receipts.size).toBe(1)
    expect(ledger.entitlements.size).toBe(1)
    expect(ledger.reconciliations.size).toBe(1)
    expect(webhookPayload).toMatchObject({
      siteCommerce: {
        action: 'mdk_webhook_reconcile',
        duplicate: false,
        reconciliation: {
          buyerPaymentReconciliationEvent: {
            status: 'matched',
          },
          entitlement: {
            status: 'active',
          },
          fakeProviderOnly: false,
          implementationState: 'verification_config_gated',
          providerEvent: {
            eventKind: 'payment_received',
            signatureBindingRef:
              'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
            signatureVerified: true,
            siteId: 'site_payment_smoke',
          },
          receipt: {
            status: 'issued',
          },
          returnProjection: {
            entitlementStatus: 'active',
            returnState: 'entitled',
          },
        },
      },
    })
    expect(replayPayload).toMatchObject({
      siteCommerce: {
        duplicate: true,
        reconciliation: {
          buyerPaymentReconciliationEvent: {
            status: 'replayed',
          },
        },
      },
    })
    expect(afterReturnPayload).toMatchObject({
      siteCommerce: {
        action: 'checkout_return_read',
        returnProjection: {
          cleanReturnPath: '/checkout/complete',
          entitlementStatus: 'active',
          returnState: 'entitled',
        },
      },
    })
    expect(afterProofPayload).toMatchObject({
      siteCommerce: {
        action: 'payment_proof_read',
        paymentProof: {
          acceptedWorkPayoutAuthority: false,
          claimState: 'entitlement_active',
          entitlementState: 'active',
          finalSettlementClaim: false,
          payoutClaimAllowed: false,
          proofState: 'verified_entitlement',
          providerPayoutAuthority: false,
          settlementClaimAllowed: false,
          siteId: 'site_payment_smoke',
        },
      },
    })
    expect(publicEvidenceText).not.toMatch(
      /(checkout_id=|generated-site-mdk-webhook-secret|lnbc|lntb|lno1|mdk_access_token|mnemonic|payment_hash|payment_preimage|preimage=|provider_grant|raw_customer_email|wallet_secret)/i,
    )
  })

  test('runs generated Site agent-paid L402 action smoke with registered agent gating', async () => {
    const generatedCatalog = exampleOpenAgentsGeneratedSitePaymentCatalog()
    const helperPlans = exampleOpenAgentsGeneratedSitePaymentHelperPlans()
    const challengePlan = helperPlans.find(
      plan => plan.helperKind === 'l402_challenge_create',
    )
    const redemptionPlan = helperPlans.find(
      plan => plan.helperKind === 'l402_redemption_create',
    )
    const paidAction = generatedCatalog.items.find(
      (item): item is OpenAgentsSitePaymentPaidActionCatalogRecord =>
        item.itemKind === 'paid_action' &&
        item.actionId === 'agent_research_note',
    )
    const agentAuthorization = 'Bearer oa_agent_generated_site_payment_smoke'
    const generatedRoutes = makeSiteCommerceRoutes({
      authorizePaidActionAgent: async request =>
        request.headers.get('authorization') === agentAuthorization,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: generatedCatalog,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })

    expect(challengePlan).toBeDefined()
    expect(redemptionPlan).toBeDefined()
    expect(paidAction).toBeDefined()

    if (
      challengePlan === undefined ||
      redemptionPlan === undefined ||
      paidAction === undefined ||
      !isRecord(challengePlan.body) ||
      !isRecord(redemptionPlan.body) ||
      challengePlan.idempotencyKey === null ||
      redemptionPlan.idempotencyKey === null
    ) {
      throw new Error('Generated Site paid action fixture is incomplete.')
    }

    const challengeBody =
      challengePlan.body as unknown as SmokeL402ChallengeBody
    const redemptionPlanBody =
      redemptionPlan.body as unknown as SmokeL402RedemptionBody
    const paidProduct =
      openAgentsPaidEndpointProductFromSitePaymentCatalogItem(paidAction)
    const paymentDecision: OpenAgentsUnifiedPaymentDecisionProjection = {
      actorRef: 'agent:generated_site_payment_smoke',
      audience: 'agent',
      creditDebit: null,
      decisionRef: 'decision.generated_site_payment_smoke.agent_research_note',
      entitlementDecision: null,
      freeBetaAllowanceRef: null,
      l402MdkReceipt: null,
      l402MdkRedemption: null,
      nextActions: ['pay_l402_mdk'],
      paymentSource: 'none',
      policyDecision: {
        audience: 'agent',
        decisionStatus: 'recoverable',
        entitlementScopeRefs: paidProduct.entitlement.scopeRefs,
        limitClass: 'economic_usage',
        operatorCostRefs: [],
        privateAccountRefs: [],
        publicSummaryRef:
          'summary.generated_site_payment_smoke.agent_research_note',
        reasonRefs: ['reason.payment_policy.economic_limit_recoverable'],
        recoveryActions: ['l402_mdk'],
        requiredEndpointRefs: [paidAction.path],
        requiredProductRefs: [paidProduct.productId],
        spendCapCaveatRefs: paidProduct.spendCapHintRefs,
        statusRefs: ['status.payment_policy.payment_recovery_available'],
        surface: 'site_checkout',
      },
      productRef: paidProduct.productId,
      reasonRefs: ['reason.unified_payment.recoverable_by_l402_mdk'],
      safeBody: {
        action: 'unified_payment_decision',
        status: 'recoverable_by_l402_mdk',
      },
      sourceRefs: {
        creditLedgerRefs: [],
        entitlementRefs: [],
        l402RedemptionRef: null,
        mdkCheckoutReceiptRef: null,
        policyRefs: ['policy.generated_site_payment_smoke.agent_research_note'],
        spendCapRefs: paidProduct.spendCapHintRefs,
        stripeTopUpRefs: [],
      },
      spendLimit: null,
      status: 'recoverable_by_l402_mdk',
      statusCode: 402,
      surface: 'site_checkout',
    }
    const spendCapPreview = previewOpenAgentsSpendCap({
      actionRef: paidAction.actionRef,
      actorRef: 'agent:generated_site_payment_smoke',
      agentAuthenticated: true,
      audience: 'agent',
      availableCreditAllowanceMinorUnits: 0,
      freeAllowanceUses: null,
      idempotencyKeyHintRef:
        'idempotency.generated_site_payment_smoke.agent_research_note',
      idempotencyKeyRequired: true,
      l402MdkRecoveryAvailable: true,
      maxPerCall: {
        ...paidAction.price,
        amountMinorUnits: paidAction.price.amountMinorUnits * 2,
      },
      maxPerWindow: {
        ...paidAction.price,
        amountMinorUnits: paidAction.price.amountMinorUnits * 3,
      },
      nowIso: fixedNow.toISOString(),
      paymentDecision,
      price: paidAction.price,
      product: paidProduct,
      requestedRail: 'bitcoin_l402_mdk',
      retryBehaviorRefs: [
        'retry.generated_site_payment_smoke.after_l402_redemption',
      ],
      route: {
        method: paidAction.method,
        ownerGrantOnly: false,
        path: paidAction.path,
        privateRoute: false,
        routeRef: 'route.generated_site_payment_smoke.agent_research_note',
      },
      settlementMode: 'deferred_until_success',
      supportedRails: ['bitcoin_l402_mdk'],
      surface: 'site_checkout',
      windowSpent: {
        ...paidAction.price,
        amountMinorUnits: 0,
      },
    })
    const discovery = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/discovery', {
        method: 'GET',
      }),
    )
    const unauthorizedChallenge = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/challenges', {
        body: challengeBody,
        idempotencyKey: 'generated-site-agent-l402-unauthorized',
      }),
    )
    const challenge = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/challenges', {
        authorization: agentAuthorization,
        body: challengeBody,
        idempotencyKey: challengePlan.idempotencyKey,
      }),
    )
    const challengePayload = (await challenge.json()) as {
      siteCommerce: {
        l402: {
          challengeId: string
          credential: {
            invoiceRef: string
            paymentHashRef: string
          }
          entitlement: {
            state: string
          }
          expiresAt: string
        }
      }
    }
    const overCap = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/challenges', {
        authorization: agentAuthorization,
        body: {
          ...challengeBody,
          spendCap: {
            amount: 1,
            asset: 'sats',
          },
        },
        idempotencyKey: 'generated-site-agent-l402-over-cap',
      }),
    )
    const unsafeProof = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/redemptions', {
        authorization: agentAuthorization,
        body: {
          ...redemptionPlanBody,
          challengeExpiresAt: '2026-06-05T18:05:00.000Z',
          challengeId: challengePayload.siteCommerce.l402.challengeId,
          paymentProofRef: 'payment_preimage_unsafe',
        },
        idempotencyKey: 'generated-site-agent-l402-unsafe-proof',
      }),
    )
    const redemptionBody = {
      ...redemptionPlanBody,
      challengeExpiresAt: '2026-06-05T18:05:00.000Z',
      challengeId: challengePayload.siteCommerce.l402.challengeId,
    }
    const redemption = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/redemptions', {
        authorization: agentAuthorization,
        body: redemptionBody,
        idempotencyKey: redemptionPlan.idempotencyKey,
      }),
    )
    const redemptionReplay = await routeWith(
      generatedRoutes,
      makeRequest('/api/sites/site_payment_smoke/commerce/l402/redemptions', {
        authorization: agentAuthorization,
        body: redemptionBody,
        idempotencyKey: redemptionPlan.idempotencyKey,
      }),
    )
    const redemptionPayload = (await redemption.json()) as {
      siteCommerce: {
        entitlement: {
          state: string
        }
        l402: {
          paidActionId: string
          path: string
        }
      }
    }
    const redemptionReplayPayload = await redemptionReplay.json()
    const retryProjection = {
      actionId: paidAction.actionId,
      entitlementScope: 'action',
      entitlementState:
        redemptionPayload.siteCommerce.entitlement.state === 'granted_stub'
          ? 'allowed_after_l402_redemption_stub'
          : 'blocked',
      retryBehaviorRefs: spendCapPreview.retryBehaviorRefs,
      routeRef: 'route.generated_site_payment_smoke.agent_research_note',
    }
    const smokeText = JSON.stringify([
      await discovery.json(),
      spendCapPreview,
      challengePayload,
      await overCap.json(),
      await unsafeProof.json(),
      redemptionPayload,
      redemptionReplayPayload,
      retryProjection,
      challenge.headers.get('www-authenticate'),
    ])

    expect(discovery.status).toBe(200)
    expect(unauthorizedChallenge.status).toBe(401)
    expect(spendCapPreview.status).toBe('under_cap')
    expect(spendCapPreview.dryRun).toBe(true)
    expect(spendCapPreview.sideEffectSummary.callsMdk).toBe(false)
    expect(spendCapPreview.sideEffectSummary.createsEntitlement).toBe(false)
    expect(challenge.status).toBe(402)
    expect(challenge.headers.get('www-authenticate')).toContain('L402')
    expect(challengePayload.siteCommerce.l402).toMatchObject({
      credential: {
        invoiceRef: 'redacted',
        paymentHashRef: 'redacted',
      },
      entitlement: {
        state: 'payment_required',
      },
      expiresAt: '2026-06-05T18:10:00.000Z',
    })
    expect(overCap.status).toBe(400)
    expect(unsafeProof.status).toBe(400)
    expect(redemption.status).toBe(202)
    expect(redemptionReplay.status).toBe(202)
    expect(redemptionReplayPayload).toEqual(redemptionPayload)
    expect(redemptionPayload).toMatchObject({
      siteCommerce: {
        entitlement: {
          scope: 'action',
          state: 'granted_stub',
        },
        l402: {
          paidActionId: 'agent_research_note',
          path: '/api/actions/research-note',
        },
      },
    })
    expect(retryProjection).toMatchObject({
      entitlementState: 'allowed_after_l402_redemption_stub',
    })
    expect(smokeText).not.toMatch(
      /(lnbc|lntb|lnbcrt|lno1|payment_preimage|mdk_access_token|mnemonic|wallet_secret)/i,
    )
  })

  test('includes customer-owned MDK binding state in checkout intent responses', async () => {
    const bindingStore = new MemorySiteMdkAccountBindingStore()
    await bindingStore.upsertBinding(makeApprovedMdkAccountBindingRecord())
    const routesWithBinding = makeSiteCommerceRoutes({
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      mdkAccountBindingStore: bindingStore,
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })
    const response = await routeWith(
      routesWithBinding,
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-customer-binding-1',
      }),
    )
    const payload = await response.json()

    expect(response.status).toBe(201)
    expect(payload).toMatchObject({
      siteCommerce: {
        checkoutIntent: {
          mdkAccountBinding: {
            bindingState: 'configured',
            providerMode: 'customer_owned_mdk',
            secretBindingRefs: [],
            secretBindingState: 'redacted',
          },
          provider: 'openagents_hosted_mdk',
          providerMode: 'customer_owned_mdk',
        },
      },
    })
    expect(JSON.stringify(payload)).not.toMatch(
      /(customer\\.site_otec|hosted_secret|MDK_ACCESS_TOKEN|mnemonic|lnbc|wallet_secret)/i,
    )
  })

  test('reconciles verified MDK webhooks into checkout status, receipt, entitlement, and clean return state', async () => {
    const ledger = new MemoryBuyerPaymentLedgerStore()
    const checkoutStore = new MemoryCheckoutIntentStore(ledger)
    const routesWithStores = makeSiteCommerceRoutes({
      authorizePaidActionAgent: async () => true,
      buyerPaymentLedgerStore: ledger,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      checkoutIntentStore: checkoutStore,
      mdkWebhookConfig: {
        bindingRef:
          'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
        secret: 'site-mdk-webhook-secret',
        source: 'dashboard_standard_webhooks',
      },
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })
    const checkoutResponse = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-webhook-1',
      }),
    )
    const checkoutPayload = (await checkoutResponse.json()) as {
      siteCommerce: {
        checkoutIntent: {
          hostedCheckout: { checkoutRef: string }
          id: string
        }
      }
    }
    const providerCheckoutId =
      checkoutPayload.siteCommerce.checkoutIntent.hostedCheckout.checkoutRef.replace(
        /^mdk_checkout\./u,
        '',
      )
    const webhookBody = JSON.stringify({
      createdAt: '2026-06-05T18:01:00.000Z',
      data: {
        checkout: {
          id: providerCheckoutId,
          status: 'PAYMENT_RECEIVED',
        },
      },
      id: 'evt_site_checkout_paid_1',
      type: 'checkout.completed',
    })
    const webhookTimestamp = '1780682460'
    const webhookSignature = await signStandardWebhook(
      'site-mdk-webhook-secret',
      'evt_site_checkout_paid_1',
      webhookTimestamp,
      webhookBody,
    )
    const webhookResponse = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_site_checkout_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const replayResponse = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_site_checkout_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const returnResponse = await routeWith(
      routesWithStores,
      makeRequest(
        `/api/sites/site_otec/commerce/checkout-returns/${checkoutPayload.siteCommerce.checkoutIntent.id}/success`,
        { method: 'GET' },
      ),
    )
    const paymentProofResponse = await routeWith(
      routesWithStores,
      makeRequest(
        `/api/sites/site_otec/commerce/payment-proofs/${checkoutPayload.siteCommerce.checkoutIntent.id}`,
        { method: 'GET' },
      ),
    )
    const webhookPayload = await webhookResponse.json()
    const replayPayload = await replayResponse.json()
    const returnPayload = await returnResponse.json()
    const paymentProofPayload = await paymentProofResponse.json()

    expect(webhookResponse.status).toBe(202)
    expect(replayResponse.status).toBe(200)
    expect(returnResponse.status).toBe(200)
    expect(paymentProofResponse.status).toBe(200)
    expect(
      checkoutStore.intentsByIntentRef.get(
        checkoutPayload.siteCommerce.checkoutIntent.id,
      )?.status,
    ).toBe('payment_received')
    expect(ledger.receipts.size).toBe(1)
    expect(ledger.entitlements.size).toBe(1)
    expect(ledger.reconciliations.size).toBe(1)
    expect(webhookPayload).toMatchObject({
      siteCommerce: {
        action: 'mdk_webhook_reconcile',
        duplicate: false,
        reconciliation: {
          buyerPaymentReconciliationEvent: {
            status: 'matched',
          },
          entitlement: {
            status: 'active',
          },
          returnProjection: {
            returnState: 'entitled',
          },
        },
      },
    })
    expect(replayPayload).toMatchObject({
      siteCommerce: {
        duplicate: true,
        reconciliation: {
          buyerPaymentReconciliationEvent: {
            status: 'replayed',
          },
        },
      },
    })
    expect(returnPayload).toMatchObject({
      siteCommerce: {
        action: 'checkout_return_read',
        returnProjection: {
          cleanReturnPath: '/checkout/thanks',
          entitlementStatus: 'active',
          returnState: 'entitled',
        },
      },
    })
    expect(paymentProofPayload).toMatchObject({
      siteCommerce: {
        action: 'payment_proof_read',
        paymentProof: {
          acceptedWorkPayoutAuthority: false,
          claimState: 'entitlement_active',
          entitlementState: 'active',
          finalSettlementClaim: false,
          payoutClaimAllowed: false,
          proofState: 'verified_entitlement',
          providerPayoutAuthority: false,
          settlementClaimAllowed: false,
        },
      },
    })
    expect(JSON.stringify(paymentProofPayload)).not.toMatch(
      /(2026-\d{2}-\d{2}T|site-mdk-webhook-secret|lnbc|payment_preimage|mnemonic|wallet_secret)/i,
    )
    expect(JSON.stringify(webhookPayload)).not.toMatch(
      /(site-mdk-webhook-secret|lnbc|payment_preimage|mnemonic|wallet_secret)/i,
    )
  })

  test('bridges verified buyer payments to one authorized payout intent', async () => {
    const ledger = new MemoryBuyerPaymentLedgerStore()
    const checkoutStore = new MemoryCheckoutIntentStore(ledger)
    const payoutLedger = new MemoryPayoutLedgerStore()
    const routesWithStores = makeSiteCommerceRoutes({
      authorizePayoutBridge: async request =>
        request.headers.get('authorization') === 'Bearer operator-token',
      buyerPaymentLedgerStore: ledger,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      checkoutIntentStore: checkoutStore,
      mdkWebhookConfig: {
        bindingRef:
          'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
        secret: 'site-mdk-webhook-secret',
        source: 'dashboard_standard_webhooks',
      },
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
      payoutLedgerStore: payoutLedger,
    })
    const checkoutResponse = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 'checkout-bridge-1',
      }),
    )
    const checkoutPayload = (await checkoutResponse.json()) as {
      siteCommerce: {
        checkoutIntent: {
          hostedCheckout: { checkoutRef: string }
          id: string
        }
      }
    }
    const providerCheckoutId =
      checkoutPayload.siteCommerce.checkoutIntent.hostedCheckout.checkoutRef.replace(
        /^mdk_checkout\./u,
        '',
      )
    const webhookBody = JSON.stringify({
      createdAt: '2026-06-05T18:01:00.000Z',
      data: {
        checkout: {
          id: providerCheckoutId,
          status: 'PAYMENT_RECEIVED',
        },
      },
      id: 'evt_site_checkout_bridge_paid_1',
      type: 'checkout.completed',
    })
    const webhookTimestamp = '1780682460'
    const webhookSignature = await signStandardWebhook(
      'site-mdk-webhook-secret',
      'evt_site_checkout_bridge_paid_1',
      webhookTimestamp,
      webhookBody,
    )

    await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_site_checkout_bridge_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )

    const unauthenticated = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/payout-bridges', {
        body: {
          acceptedWorkRefs: ['accepted_work.public.bridge.demo'],
          amount: {
            amountMinorUnits: 1_000,
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
          checkoutIntentRef: checkoutPayload.siteCommerce.checkoutIntent.id,
          payoutTargetApprovalRef: 'approval.public.pylon.bridge.demo',
          payoutTargetRef: 'payout_target.public.pylon.bridge.demo',
          policySnapshotRef: 'policy_snapshot.public.bridge.demo',
          spendCap: {
            amountMinorUnits: 2_000,
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
          walletReadiness: 'ready',
        },
        idempotencyKey: 'bridge-intent-unauthorized',
      }),
    )
    const bridgeResponse = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/payout-bridges',
        {
          body: JSON.stringify({
            acceptedWorkRefs: ['accepted_work.public.bridge.demo'],
            adapterKind: 'simulation',
            amount: {
              amountMinorUnits: 1_000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            },
            artanisDispatchRef: 'artanis.dispatch.public.bridge.demo',
            assignmentRef: 'assignment.public.bridge.demo',
            checkoutIntentRef: checkoutPayload.siteCommerce.checkoutIntent.id,
            metadataRefs: ['metadata.site_payment_to_payout.bridge.demo'],
            payoutTargetApprovalRef: 'approval.public.pylon.bridge.demo',
            payoutTargetRef: 'payout_target.public.pylon.bridge.demo',
            policySnapshotRef: 'policy_snapshot.public.bridge.demo',
            pylonJobRef: 'pylon_job.public.bridge.demo',
            spendCap: {
              amountMinorUnits: 2_000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            },
            walletReadiness: 'ready',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'bridge-intent-1',
          },
          method: 'POST',
        },
      ),
    )
    const duplicateResponse = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/payout-bridges',
        {
          body: JSON.stringify({
            acceptedWorkRefs: ['accepted_work.public.bridge.demo'],
            amount: {
              amountMinorUnits: 1_000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            },
            checkoutIntentRef: checkoutPayload.siteCommerce.checkoutIntent.id,
            payoutTargetApprovalRef: 'approval.public.pylon.bridge.demo',
            payoutTargetRef: 'payout_target.public.pylon.bridge.demo',
            policySnapshotRef: 'policy_snapshot.public.bridge.demo',
            spendCap: {
              amountMinorUnits: 2_000,
              asset: 'bitcoin',
              denomination: 'bitcoin_millisatoshi',
            },
            walletReadiness: 'ready',
          }),
          headers: {
            authorization: 'Bearer operator-token',
            'content-type': 'application/json',
            'idempotency-key': 'bridge-intent-2',
          },
          method: 'POST',
        },
      ),
    )
    const bridgePayload = await bridgeResponse.json()
    const duplicatePayload = await duplicateResponse.json()

    expect(unauthenticated.status).toBe(401)
    expect(bridgeResponse.status).toBe(201)
    expect(duplicateResponse.status).toBe(409)
    expect(payoutLedger.intents.size).toBe(1)
    expect(bridgePayload).toMatchObject({
      siteCommerce: {
        action: 'payout_bridge_create',
        bridge: {
          checkoutReturnAuthority: false,
          state: 'payout_intent_ready',
          verifiedBuyerPaymentRef:
            'receipt.site_payment.site_otec_site_checkout_intent_site_otec_checkout-bridge-1',
        },
        payoutIntent: {
          status: 'approved',
        },
      },
    })
    expect(duplicatePayload).toMatchObject({
      siteCommerce: {
        bridge: {
          blockerRefs: ['duplicate_buyer_payment_ref'],
          state: 'blocked',
        },
      },
    })
    expect(JSON.stringify(bridgePayload)).not.toMatch(
      /(site-mdk-webhook-secret|lnbc|payment_preimage|mnemonic|wallet_secret)/i,
    )
  })

  test('rejects L402 challenges that exceed the declared spend cap', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/l402/challenges', {
        body: {
          ...validChallengeBody,
          spendCap: {
            amount: 100,
            asset: 'sats',
          },
        },
        idempotencyKey: 'l402-over-cap',
      }),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'invalid_site_commerce_contract',
      message: 'price must be within the declared spend cap.',
    })
  })

  test('returns redacted L402 challenges without raw invoices or wallet material', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/l402/challenges', {
        body: validChallengeBody,
        idempotencyKey: 'l402-challenge-1',
      }),
    )
    const redactionResponse = response.clone()
    const payload = await response.json()

    expect(response.status).toBe(402)
    expect(response.headers.get('www-authenticate')).toContain('L402')
    expect(response.headers.get('www-authenticate')).toContain(
      'invoice_ref="redacted"',
    )
    expect(payload).toMatchObject({
      siteCommerce: {
        l402: {
          challengeId: 'site_l402_challenge_site_otec_l402-challenge-1',
          credential: {
            invoiceRef: 'redacted',
            paymentHashRef: 'redacted',
          },
          expiresAt: '2026-06-05T18:10:00.000Z',
        },
        redaction: {
          exposesMdkCredentials: false,
          exposesRawInvoice: false,
          exposesWalletMaterial: false,
        },
      },
    })
    expect(
      await responseTextIncludesProhibitedPaymentMaterial(redactionResponse),
    ).toBe(false)
  })

  test('rejects stale L402 challenge redemptions', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/l402/redemptions', {
        body: {
          challengeExpiresAt: '2026-06-05T17:59:00.000Z',
          challengeId: 'site_l402_challenge_site_otec_l402-challenge-1',
          credentialId: 'site_l402_credential_1',
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: 'generate-report',
          path: '/api/actions/generate-report',
          paymentProofRef: 'mdk_payment_proof_12345678',
          price: {
            amount: 1200,
            asset: 'sats',
          },
        },
        idempotencyKey: 'l402-stale-redemption',
      }),
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'l402_challenge_stale',
      message: 'The L402 challenge is stale. Request a fresh challenge.',
    })
  })

  test('accepts fresh redacted L402 payment-proof refs as entitlement stubs', async () => {
    const response = await routeRequest(
      makeRequest('/api/sites/site_otec/commerce/l402/redemptions', {
        body: {
          challengeExpiresAt: '2026-06-05T18:05:00.000Z',
          challengeId: 'site_l402_challenge_site_otec_l402-challenge-1',
          credentialId: 'site_l402_credential_1',
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: 'generate-report',
          path: '/api/actions/generate-report',
          paymentProofRef: 'mdk_payment_proof_12345678',
          price: {
            amount: 1200,
            asset: 'sats',
          },
        },
        idempotencyKey: 'l402-redemption-1',
      }),
    )
    const redactionResponse = response.clone()
    const payload = await response.json()

    expect(response.status).toBe(202)
    expect(isRecord(payload)).toBe(true)
    expect(payload).toMatchObject({
      siteCommerce: {
        entitlement: {
          scope: 'action',
          state: 'granted_stub',
        },
        l402: {
          redemptionId: 'site_l402_redemption_site_otec_l402-redemption-1',
        },
      },
    })
    expect(
      await responseTextIncludesProhibitedPaymentMaterial(redactionResponse),
    ).toBe(false)
  })

  test('runs repeatable fake-provider Site MDK smoke across checkout, proof, L402, and redaction boundaries', async () => {
    const ledger = new MemoryBuyerPaymentLedgerStore()
    const checkoutStore = new MemoryCheckoutIntentStore(ledger)
    const routesWithStores = makeSiteCommerceRoutes({
      authorizePaidActionAgent: async () => true,
      buyerPaymentLedgerStore: ledger,
      challengeExpiresAt: () => '2026-06-05T18:10:00.000Z',
      checkoutCatalog: sitePaymentCatalog,
      checkoutIntentStore: checkoutStore,
      mdkWebhookConfig: {
        bindingRef:
          'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
        secret: 'site-mdk-webhook-secret',
        source: 'dashboard_standard_webhooks',
      },
      nowEpochMillis: () => fixedNow.getTime(),
      nowIso: () => fixedNow.toISOString(),
    })
    const discovery = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/discovery', {
        method: 'GET',
      }),
    )
    const checkout = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/checkout-intents', {
        body: validCheckoutBody,
        idempotencyKey: 's1',
      }),
    )
    const checkoutPayload = (await checkout.json()) as {
      siteCommerce: {
        checkoutIntent: {
          hostedCheckout: { checkoutRef: string }
          id: string
        }
      }
    }
    const cleanStatus = await routeWith(
      routesWithStores,
      makeRequest(
        `/api/sites/site_otec/commerce/checkout-returns/${checkoutPayload.siteCommerce.checkoutIntent.id}/status`,
        { method: 'GET' },
      ),
    )
    const cleanCancel = await routeWith(
      routesWithStores,
      makeRequest(
        `/api/sites/site_otec/commerce/checkout-returns/${checkoutPayload.siteCommerce.checkoutIntent.id}/cancel`,
        { method: 'GET' },
      ),
    )
    const providerCheckoutId =
      checkoutPayload.siteCommerce.checkoutIntent.hostedCheckout.checkoutRef.replace(
        /^mdk_checkout\./u,
        '',
      )
    const webhookBody = JSON.stringify({
      createdAt: '2026-06-05T18:01:00.000Z',
      data: {
        checkout: {
          id: providerCheckoutId,
          status: 'PAYMENT_RECEIVED',
        },
      },
      id: 'evt_site_mdk_smoke_paid_1',
      type: 'checkout.completed',
    })
    const webhookTimestamp = '1780682460'
    const webhookSignature = await signStandardWebhook(
      'site-mdk-webhook-secret',
      'evt_site_mdk_smoke_paid_1',
      webhookTimestamp,
      webhookBody,
    )
    const webhook = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_site_mdk_smoke_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const webhookReplay = await routeWith(
      routesWithStores,
      new Request(
        'https://openagents.com/api/sites/site_otec/commerce/mdk/webhooks',
        {
          body: webhookBody,
          headers: {
            'content-type': 'application/json',
            'webhook-id': 'evt_site_mdk_smoke_paid_1',
            'webhook-signature': webhookSignature,
            'webhook-timestamp': webhookTimestamp,
          },
          method: 'POST',
        },
      ),
    )
    const proof = await routeWith(
      routesWithStores,
      makeRequest(
        `/api/sites/site_otec/commerce/payment-proofs/${checkoutPayload.siteCommerce.checkoutIntent.id}`,
        { method: 'GET' },
      ),
    )
    const l402Challenge = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/l402/challenges', {
        body: validChallengeBody,
        idempotencyKey: 'site-mdk-smoke-l402-challenge-1',
      }),
    )
    const l402Payload = (await l402Challenge.json()) as {
      siteCommerce: {
        l402: {
          challengeId: string
        }
      }
    }
    const l402Redemption = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/l402/redemptions', {
        body: {
          challengeExpiresAt: '2026-06-05T18:05:00.000Z',
          challengeId: l402Payload.siteCommerce.l402.challengeId,
          credentialId: 'site_l402_credential_smoke',
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: 'generate-report',
          path: '/api/actions/generate-report',
          paymentProofRef: 'mdk_payment_proof_smoke1234',
          price: {
            amount: 1200,
            asset: 'sats',
          },
        },
        idempotencyKey: 'site-mdk-smoke-l402-redemption-1',
      }),
    )
    const staleRedemption = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/l402/redemptions', {
        body: {
          challengeExpiresAt: '2026-06-05T17:59:00.000Z',
          challengeId: l402Payload.siteCommerce.l402.challengeId,
          credentialId: 'site_l402_credential_smoke',
          entitlementScope: 'action',
          method: 'POST',
          paidActionId: 'generate-report',
          path: '/api/actions/generate-report',
          paymentProofRef: 'mdk_payment_proof_smoke1234',
          price: {
            amount: 1200,
            asset: 'sats',
          },
        },
        idempotencyKey: 'site-mdk-smoke-l402-stale-1',
      }),
    )
    const spendCapRejection = await routeWith(
      routesWithStores,
      makeRequest('/api/sites/site_otec/commerce/l402/challenges', {
        body: {
          ...validChallengeBody,
          spendCap: {
            amount: 100,
            asset: 'sats',
          },
        },
        idempotencyKey: 'site-mdk-smoke-spend-cap-1',
      }),
    )
    const webhookPayload = await webhook.json()
    const replayPayload = await webhookReplay.json()
    const proofPayload = await proof.json()
    const redemptionPayload = (await l402Redemption.json()) as {
      siteCommerce: {
        l402: {
          redemptionId: string
        }
      }
    }
    const smokeText = JSON.stringify([
      await discovery.json(),
      checkoutPayload,
      await cleanStatus.json(),
      await cleanCancel.json(),
      webhookPayload,
      replayPayload,
      proofPayload,
      l402Payload,
      redemptionPayload,
      await staleRedemption.json(),
      await spendCapRejection.json(),
      l402Challenge.headers.get('www-authenticate'),
    ])
    const receiptRef =
      [...ledger.receipts.values()][0]?.receiptRef ??
      'receipt.public.site_mdk_smoke.missing'
    const smokeRecord = {
      ...exampleOpenAgentsSiteMdkSmokeRecord(),
      checkoutIntentRefs: [checkoutPayload.siteCommerce.checkoutIntent.id],
      l402ChallengeRefs: [l402Payload.siteCommerce.l402.challengeId],
      l402RedemptionRefs: [redemptionPayload.siteCommerce.l402.redemptionId],
      receiptRefs: [receiptRef],
      smokeRef: 'smoke.public.site_mdk.fake_provider.route',
    }
    const smokeProjection = projectOpenAgentsSiteMdkSmoke(smokeRecord, 'public')

    expect(discovery.status).toBe(200)
    expect(checkout.status).toBe(201)
    expect(cleanStatus.status).toBe(200)
    expect(cleanCancel.status).toBe(200)
    expect(webhook.status).toBe(202)
    expect(webhookReplay.status).toBe(200)
    expect(proof.status).toBe(200)
    expect(l402Challenge.status).toBe(402)
    expect(l402Redemption.status).toBe(202)
    expect(staleRedemption.status).toBe(409)
    expect(spendCapRejection.status).toBe(400)
    expect(replayPayload).toMatchObject({
      siteCommerce: { duplicate: true },
    })
    expect(proofPayload).toMatchObject({
      siteCommerce: {
        paymentProof: {
          proofState: 'verified_entitlement',
          settlementClaimAllowed: false,
        },
      },
    })
    expect(smokeProjection).toMatchObject({
      implementationState: 'fake_provider',
      notProductionPaymentEvidence: true,
      passedCheckCount: 11,
      providerPayoutClaimAllowed: false,
      settlementClaimAllowed: false,
      smokeState: 'passed',
      walletSpendAllowed: false,
    })
    expect(smokeText).not.toMatch(
      /(site-mdk-webhook-secret|lnbc|lntb|payment_hash|payment_preimage|preimage|mnemonic|wallet_secret|MDK_ACCESS_TOKEN)/i,
    )
  })
})
