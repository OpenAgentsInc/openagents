import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import {
  type PrefilledWorkspaceServiceShape,
  toPublicProjection,
} from './prefilled-workspace'
import { makeEcommerceDesignPartnerWorkspaceInput } from './prefilled-workspace-vertical-templates'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type EcommerceCampaignSelfServeRoutesDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => PrefilledWorkspaceServiceShape
  enabled: boolean
}>

export const ECOMMERCE_WORKSPACE_PACK_PROMISE =
  'business.ecommerce_workspace_pack.v1' as const

export const ECOMMERCE_PACK_SELF_SERVE_BLOCKER_REF =
  'blocker.product_promises.ecommerce_pack_self_serve_missing' as const

export const ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF =
  'blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing' as const

const handleSelfServeOrder = <Bindings>(
  request: Request,
  env: Bindings,
  dependencies: EcommerceCampaignSelfServeRoutesDependencies<Bindings>,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (!dependencies.enabled) {
      return noStoreJsonResponse(
        {
          error: 'ecommerce_campaign_self_serve_disabled',
          reason: 'The self-serve e-commerce campaign flow is not armed.',
            promiseIds: [ECOMMERCE_WORKSPACE_PACK_PROMISE],
            promiseState: 'yellow',
            inert: true,
            generatedAt: currentIsoTimestamp(),
            staleness: liveAtReadStaleness(['workspace_created']),
          },
          { status: 503 },
      )
    }

    const input = makeEcommerceDesignPartnerWorkspaceInput()
    const store = dependencies.makeStore(env)

    const record = yield* Effect.tryPromise({
      try: () => store.createWorkspace({ ...input, accessMode: 'public_safe' }),
      catch: () => serverError(),
    })

    return noStoreJsonResponse(
      {
        schema: 'openagents.ecommerce_campaign.self_serve_workspace.v1',
        promiseIds: [ECOMMERCE_WORKSPACE_PACK_PROMISE],
        promiseState: 'yellow',
        inert: true,
        unclearedBlockerRefs: [ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF],
        generatedAt: currentIsoTimestamp(),
        staleness: liveAtReadStaleness(['workspace_created']),
        workspace: toPublicProjection(record),
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(error as HttpResponse)))

export const makeEcommerceCampaignSelfServeRoutes = <Bindings>(
  dependencies: EcommerceCampaignSelfServeRoutesDependencies<Bindings>,
) => ({
  routeEcommerceCampaignSelfServeRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/public/ecommerce-campaign/workspaces') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }
      return handleSelfServeOrder(request, env, dependencies)
    }

    return undefined
  },
})
