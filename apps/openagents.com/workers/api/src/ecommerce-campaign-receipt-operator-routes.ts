import { Effect, Schema as S } from 'effect'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import type { EcommerceCampaignReceiptStore } from './ecommerce-campaign-receipt-store'
import {
  type EcommerceCampaignDeliveryInput,
  EcommerceCampaignOutcomeKind,
  EcommerceCampaignAuthorityGateId,
  EcommerceCampaignPaidSettlement,
  buildEcommerceCampaignDeliveryReceipt,
  toEcommerceCampaignDeliveryReceiptDocument,
} from './ecommerce-campaign-delivery-receipt'
import { EcommerceCampaignWorkflowReceipt } from './ecommerce-campaign-workflow'

export type EcommerceCampaignReceiptOperatorRoutesDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => EcommerceCampaignReceiptStore
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

type HttpResponse = globalThis.Response

const RecordReceiptRequest = S.Struct({
  receiptRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(300)),
  workItemRef: S.String,
  outcomeKind: EcommerceCampaignOutcomeKind,
  humanReviewAccepted: S.Boolean,
  receiptedGates: S.Record(EcommerceCampaignAuthorityGateId, S.Boolean),
  spendCapCents: S.Number,
  spendObservedCents: S.NullOr(S.Number),
  publishedArtifactRefs: S.Array(S.String),
  statsWindow: S.NullOr(S.String),
  attributionCaveat: S.String,
  stockoutFollowUp: S.String,
  campaignWorkflow: S.optionalKey(EcommerceCampaignWorkflowReceipt),
  paidSettlement: EcommerceCampaignPaidSettlement,
  freshnessTimestamp: S.String,
  publicSourceRefs: S.Array(S.String),
})

type RecordReceiptRequestType = typeof RecordReceiptRequest.Type

export const makeEcommerceCampaignReceiptOperatorRoutes = <Bindings>(
  dependencies: EcommerceCampaignReceiptOperatorRoutesDependencies<Bindings>,
) => ({
  routeEcommerceCampaignReceiptOperatorRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/ecommerce-campaign/receipts') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return Effect.gen(function* () {
        const authorized = yield* Effect.tryPromise({
          catch: () => ({ _tag: 'unauthorized' } as const),
          try: () => dependencies.requireAdminApiToken(request, env),
        })

        if (!authorized) {
          return yield* Effect.fail({ _tag: 'unauthorized' } as const)
        }

        const body = yield* Effect.tryPromise({
          catch: () => ({ _tag: 'bad_request' } as const),
          try: async () => request.json(),
        })

        const parsed = yield* Effect.try({
          catch: () => ({ _tag: 'bad_request' } as const),
          try: () => S.decodeUnknownSync(RecordReceiptRequest)(body) as RecordReceiptRequestType,
        })

        const receiptInput: EcommerceCampaignDeliveryInput = {
          workItemRef: parsed.workItemRef,
          outcomeKind: parsed.outcomeKind,
          humanReviewAccepted: parsed.humanReviewAccepted,
          receiptedGates: parsed.receiptedGates as Record<typeof EcommerceCampaignAuthorityGateId.Type, boolean>,
          spendCapCents: parsed.spendCapCents,
          spendObservedCents: parsed.spendObservedCents,
          publishedArtifactRefs: parsed.publishedArtifactRefs,
          statsWindow: parsed.statsWindow,
          attributionCaveat: parsed.attributionCaveat,
          stockoutFollowUp: parsed.stockoutFollowUp,
          campaignWorkflow: parsed.campaignWorkflow,
          paidSettlement: parsed.paidSettlement,
          freshnessTimestamp: parsed.freshnessTimestamp,
          publicSourceRefs: parsed.publicSourceRefs,
        }

        const receipt = yield* Effect.try({
          catch: error => ({ _tag: 'receipt_build_error' as const, reason: error instanceof Error ? error.message : String(error) }),
          try: () => buildEcommerceCampaignDeliveryReceipt(receiptInput),
        })

        const document = toEcommerceCampaignDeliveryReceiptDocument(receipt)

        const result = yield* Effect.tryPromise({
          catch: () => ({ _tag: 'storage_error' } as const),
          try: () => dependencies.makeStore(env).put(document, parsed.receiptRef),
        })

        if (result.kind === 'already_stored') {
          return noStoreJsonResponse(
            { error: 'receipt_already_exists', receiptRef: parsed.receiptRef },
            { status: 409 },
          )
        }

        return noStoreJsonResponse({ receipt: document.receipt }, { status: 201 })
      }).pipe(
        Effect.catchTags({
          unauthorized: () => Effect.succeed(noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })),
          bad_request: () => Effect.succeed(noStoreJsonResponse({ error: 'bad_request' }, { status: 400 })),
          receipt_build_error: ({ reason }) => Effect.succeed(noStoreJsonResponse({ error: 'receipt_build_error', reason }, { status: 400 })),
          storage_error: () => Effect.succeed(serverError()),
        })
      )
    }

    return undefined
  },
})
