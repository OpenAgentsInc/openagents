import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import { redirectResponse } from './http/responses'
import { type RouteEffect, routeEffectOrResponse } from './http/route-effects'
import { type ExactRoute, routeExact } from './http/router'
import type { Env } from './index'
import { OpenAgentsWorkerRequest } from './runtime'

type OptionalEffectRoute = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
) => Effect.Effect<Response> | undefined

type WorkerRouteEnv = Parameters<OptionalEffectRoute>[1]

type WorkerRouteDependencies = Readonly<{
  cleanProductRouteRedirectLocation: (url: URL) => string | undefined
  exactRoutes: ReadonlyArray<ExactRoute<Env>>
  handleAssetRequest: (request: Request, env: WorkerRouteEnv) => RouteEffect
  handleAppShellPage: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => RouteEffect
  handleThreadPage: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    threadId: string,
  ) => RouteEffect
  handleForumThreadPage: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    topicId: string,
  ) => RouteEffect
  optionalUuid: (value: string | undefined) => string | undefined
  routeAutopilotWorkRequest: OptionalEffectRoute
  routeCloudCodingSessionRequest: OptionalEffectRoute
  routeAgentGoalRequest: OptionalEffectRoute
  routeAgentOwnerClaimRequest: OptionalEffectRoute
  routeCheckoutPageRequest: OptionalEffectRoute
  routeTreasuryPageRequest: OptionalEffectRoute
  routeAgentProposalRequest: OptionalEffectRoute
  routeAgentSearchRequest: OptionalEffectRoute
  routeAgentScopedGrantRequest: OptionalEffectRoute
  routeAgentSiteRequest: OptionalEffectRoute
  routeForumRequest: OptionalEffectRoute
  routeImageGenerationRequest: OptionalEffectRoute
  routeModelRetrieveRequest: OptionalEffectRoute
  // Durable inference resume read GET /v1/chat/completions/durable/{requestId}
  // (durable-stream Rank-1, #6058 — the path-param resume surface the exact-route
  // registry cannot match). Reads stored bytes only; NEVER meters.
  routeDurableInferenceReadRequest: OptionalEffectRoute
  routeMulletRequest: OptionalEffectRoute
  routeOmniRequest: OptionalEffectRoute
  routeOnboardingRequest: OptionalEffectRoute
  routeNexusPylonVisibilityRequest: OptionalEffectRoute
  routePublicCardCreditSpendReceiptRequest: OptionalEffectRoute
  routePublicCloudPrimitiveReceiptRequest: OptionalEffectRoute
  routePublicInferenceReceiptRequest: OptionalEffectRoute
  routePublicNip90MarketReceiptRequest: OptionalEffectRoute
  routePublicPartnerPayoutReceiptRequest: OptionalEffectRoute
  routePublicSiteReferralPayoutReceiptRequest: OptionalEffectRoute
  routePublicStripeCheckoutReceiptRequest: OptionalEffectRoute
  routeEcommerceCampaignReceiptRequest: OptionalEffectRoute
  routeEcommerceCampaignReceiptOperatorRequest: OptionalEffectRoute
  routeEcommerceCampaignSelfServeRequest: OptionalEffectRoute
  routeMarketingAgencyReceiptRequest: OptionalEffectRoute
  routeMarketingAgencySelfServeRequest: OptionalEffectRoute
  routePylonApiRequest: OptionalEffectRoute
  routeSiteCommerceRequest: OptionalEffectRoute
  routeSiteReferralInspectionRequest: OptionalEffectRoute
  routeSiteReferralPayoutLedgerRequest: OptionalEffectRoute
  routeInferenceReferralRequest: OptionalEffectRoute
  routeSiteReferralRequest: OptionalEffectRoute
  routeOperatorAdjutantRequest: OptionalEffectRoute
  routeOperatorArtanisConsoleRequest: OptionalEffectRoute
  routeOperatorEmailInspectionRequest: OptionalEffectRoute
  routeOperatorOrderTriageRequest: OptionalEffectRoute
  routeOperatorPylonMarketplaceRequest: OptionalEffectRoute
  routeOperatorProviderAccountRequest: OptionalEffectRoute
  routeOperatorSitesRequest: OptionalEffectRoute
  routeProviderAccountRequest: OptionalEffectRoute
  routeShareRequest: OptionalEffectRoute
  routeSyncRequest: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Effect.Effect<Response>
  routeHygieneLaneSettlementRequest: OptionalEffectRoute
  routeFirmupLaneSettlementRequest: OptionalEffectRoute
  routeTassadarTraceContributionRequest: OptionalEffectRoute
  routeTeamChatRequest: OptionalEffectRoute
  routeThreadFileRequest: OptionalEffectRoute
  routeTrainingRunWindowRequest: OptionalEffectRoute
  routeTrainingVerificationRequest: OptionalEffectRoute
}>

const knownDocumentPathPatterns: ReadonlyArray<RegExp> = [
  /^\/$/,
  /^\/adjutant$/,
  /^\/admin$/,
  /^\/agents\/[^/]+$/,
  /^\/artanis$/,
  /^\/autopilot$/,
  /^\/billing$/,
  /^\/blog(?:\/[^/]+)?$/,
  /^\/business$/,
  /^\/components(?:\/[^/]+)?$/,
  /^\/animations$/,
  /^\/run$/,
  /^\/login$/,
  /^\/demo(?:\/.*)?$/,
  /^\/dashboard$/,
  /^\/docs(?:\/[^/]+)?$/,
  /^\/files(?:\/[^/]+)?$/,
  /^\/forge$/,
  /^\/forum(?:\/.*)?$/,
  /^\/images$/,
  /^\/landing$/,
  /^\/moksha$/,
  /^\/moksha2$/,
  /^\/mullet$/,
  /^\/pylon$/,
  /^\/onboarding$/,
  /^\/order$/,
  /^\/orders\/[^/]+$/,
  /^\/promises$/,
  /^\/settings(?:\/[^/]+)?$/,
  /^\/share\/[^/]+$/,
  /^\/sites\/demo-checkout(?:\/[^/]+)?$/,
  /^\/stats$/,
  /^\/stats-old$/,
  /^\/tassadar$/,
  /^\/tassadar\/replay\/[^/]+$/,
  /^\/teams\/[^/]+(?:\/chat|\/files(?:\/[^/]+)?|\/projects\/[^/]+\/chat)$/,
  /^\/t\/[^/]+$/,
  /^\/training\/runs(?:\/[^/]+)?$/,
  /^\/usage$/,
]

const safeDecodeTopicSegment = (value: string): string | undefined => {
  try {
    const decoded = decodeURIComponent(value)
    return decoded.length > 0 ? decoded : undefined
  } catch {
    return undefined
  }
}

const acceptsDocument = (request: Request): boolean => {
  const accept = request.headers.get('accept')?.toLowerCase() ?? ''

  return accept === '' || accept.includes('text/html') || accept.includes('*/*')
}

const pathLooksLikeFile = (pathname: string): boolean =>
  /\/[^/]+\.[A-Za-z0-9]{1,12}$/.test(pathname)

export const shouldRedirectUnknownDocumentToHome = (
  request: Request,
  pathname: string,
): boolean => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return false
  }

  if (!acceptsDocument(request) || pathLooksLikeFile(pathname)) {
    return false
  }

  if (
    pathname.startsWith('/api/') ||
    pathname.startsWith('/assets/') ||
    pathname.startsWith('/auth/') ||
    pathname.startsWith('/checkout') ||
    pathname.startsWith('/openagents-agent-claim')
  ) {
    return false
  }

  return !knownDocumentPathPatterns.some(pattern => pattern.test(pathname))
}

export const makeWorkerRouteRequest =
  (dependencies: WorkerRouteDependencies) =>
  (): Effect.Effect<Response, never, OpenAgentsWorkerRequest> =>
    Effect.gen(function* () {
      const { ctx, env, request, url } = yield* OpenAgentsWorkerRequest
      const cleanProductRouteLocation =
        dependencies.cleanProductRouteRedirectLocation(url)

      if (cleanProductRouteLocation !== undefined) {
        return redirectResponse(cleanProductRouteLocation)
      }

      const exactResponse = routeExact(
        dependencies.exactRoutes,
        url.pathname,
        request,
        env,
        ctx,
      )

      if (exactResponse !== undefined) {
        return yield* exactResponse
      }

      // Cloud coding-session surface (autopilot.cloud_coding_sessions.v1, red).
      // INERT by default; the dispatcher routes both the launch base path and
      // the /:id lifecycle read, returning undefined for any other path.
      const cloudCodingSessionResponse =
        dependencies.routeCloudCodingSessionRequest(request, env, ctx)

      if (cloudCodingSessionResponse !== undefined) {
        return yield* cloudCodingSessionResponse
      }

      const teamChatResponse = dependencies.routeTeamChatRequest(
        request,
        env,
        ctx,
      )

      if (teamChatResponse !== undefined) {
        return yield* teamChatResponse
      }

      const onboardingResponse = dependencies.routeOnboardingRequest(
        request,
        env,
        ctx,
      )

      if (onboardingResponse !== undefined) {
        return yield* onboardingResponse
      }

      const autopilotWorkResponse = dependencies.routeAutopilotWorkRequest(
        request,
        env,
        ctx,
      )

      if (autopilotWorkResponse !== undefined) {
        return yield* autopilotWorkResponse
      }

      const imageGenerationResponse = dependencies.routeImageGenerationRequest(
        request,
        env,
        ctx,
      )

      if (imageGenerationResponse !== undefined) {
        return yield* imageGenerationResponse
      }

      // OpenAI-compatible GET /v1/models/{model} retrieve (the path-param
      // surface the exact-route registry cannot match). INERT-gated in the
      // handler; the list /v1/models is an exact route handled above.
      const modelRetrieveResponse = dependencies.routeModelRetrieveRequest(
        request,
        env,
        ctx,
      )

      if (modelRetrieveResponse !== undefined) {
        return yield* modelRetrieveResponse
      }

      // Durable inference resume read (durable-stream Rank-1, #6058): the
      // path-param resume surface for a dropped streaming completion. Reads stored
      // bytes only — never meters. INERT-gated in the handler (shares the gateway
      // flag); off/unwired => undefined and the router falls through.
      const durableInferenceReadResponse =
        dependencies.routeDurableInferenceReadRequest(request, env, ctx)

      if (durableInferenceReadResponse !== undefined) {
        return yield* durableInferenceReadResponse
      }

      const threadFileResponse = dependencies.routeThreadFileRequest(
        request,
        env,
        ctx,
      )

      if (threadFileResponse !== undefined) {
        return yield* threadFileResponse
      }

      const omniResponse = dependencies.routeOmniRequest(request, env, ctx)

      if (omniResponse !== undefined) {
        return yield* omniResponse
      }

      const providerAccountResponse = dependencies.routeProviderAccountRequest(
        request,
        env,
        ctx,
      )

      if (providerAccountResponse !== undefined) {
        return yield* providerAccountResponse
      }

      const shareResponse = dependencies.routeShareRequest(request, env, ctx)

      if (shareResponse !== undefined) {
        return yield* shareResponse
      }

      const agentGoalResponse = dependencies.routeAgentGoalRequest(
        request,
        env,
        ctx,
      )

      if (agentGoalResponse !== undefined) {
        return yield* agentGoalResponse
      }

      const agentOwnerClaimResponse = dependencies.routeAgentOwnerClaimRequest(
        request,
        env,
        ctx,
      )

      if (agentOwnerClaimResponse !== undefined) {
        return yield* agentOwnerClaimResponse
      }

      const checkoutPageResponse = dependencies.routeCheckoutPageRequest(
        request,
        env,
        ctx,
      )

      if (checkoutPageResponse !== undefined) {
        return yield* checkoutPageResponse
      }

      const treasuryPageResponse = dependencies.routeTreasuryPageRequest(
        request,
        env,
        ctx,
      )

      if (treasuryPageResponse !== undefined) {
        return yield* treasuryPageResponse
      }

      const agentProposalResponse = dependencies.routeAgentProposalRequest(
        request,
        env,
        ctx,
      )

      if (agentProposalResponse !== undefined) {
        return yield* agentProposalResponse
      }

      const agentSearchResponse = dependencies.routeAgentSearchRequest(
        request,
        env,
        ctx,
      )

      if (agentSearchResponse !== undefined) {
        return yield* agentSearchResponse
      }

      const agentScopedGrantResponse =
        dependencies.routeAgentScopedGrantRequest(request, env, ctx)

      if (agentScopedGrantResponse !== undefined) {
        return yield* agentScopedGrantResponse
      }

      const agentSiteResponse = dependencies.routeAgentSiteRequest(
        request,
        env,
        ctx,
      )

      if (agentSiteResponse !== undefined) {
        return yield* agentSiteResponse
      }

      const siteCommerceResponse = dependencies.routeSiteCommerceRequest(
        request,
        env,
        ctx,
      )

      if (siteCommerceResponse !== undefined) {
        return yield* siteCommerceResponse
      }

      const siteReferralResponse = dependencies.routeSiteReferralRequest(
        request,
        env,
        ctx,
      )

      if (siteReferralResponse !== undefined) {
        return yield* siteReferralResponse
      }

      const siteReferralInspectionResponse =
        dependencies.routeSiteReferralInspectionRequest(request, env, ctx)

      if (siteReferralInspectionResponse !== undefined) {
        return yield* siteReferralInspectionResponse
      }

      const siteReferralPayoutLedgerResponse =
        dependencies.routeSiteReferralPayoutLedgerRequest(request, env, ctx)

      if (siteReferralPayoutLedgerResponse !== undefined) {
        return yield* siteReferralPayoutLedgerResponse
      }

      const inferenceReferralResponse =
        dependencies.routeInferenceReferralRequest(request, env, ctx)

      if (inferenceReferralResponse !== undefined) {
        return yield* inferenceReferralResponse
      }

      const pylonApiResponse = dependencies.routePylonApiRequest(
        request,
        env,
        ctx,
      )

      if (pylonApiResponse !== undefined) {
        return yield* pylonApiResponse
      }

      const tassadarTraceContributionResponse =
        dependencies.routeTassadarTraceContributionRequest(request, env, ctx)

      if (tassadarTraceContributionResponse !== undefined) {
        return yield* tassadarTraceContributionResponse
      }

      const trainingRunWindowResponse =
        dependencies.routeTrainingRunWindowRequest(request, env, ctx)

      if (trainingRunWindowResponse !== undefined) {
        return yield* trainingRunWindowResponse
      }

      const hygieneLaneSettlementResponse =
        dependencies.routeHygieneLaneSettlementRequest(request, env, ctx)

      if (hygieneLaneSettlementResponse !== undefined) {
        return yield* hygieneLaneSettlementResponse
      }

      const firmupLaneSettlementResponse =
        dependencies.routeFirmupLaneSettlementRequest(request, env, ctx)

      if (firmupLaneSettlementResponse !== undefined) {
        return yield* firmupLaneSettlementResponse
      }

      const trainingVerificationResponse =
        dependencies.routeTrainingVerificationRequest(request, env, ctx)

      if (trainingVerificationResponse !== undefined) {
        return yield* trainingVerificationResponse
      }

      const nexusPylonVisibilityResponse =
        dependencies.routeNexusPylonVisibilityRequest(request, env, ctx)

      if (nexusPylonVisibilityResponse !== undefined) {
        return yield* nexusPylonVisibilityResponse
      }

      const publicInferenceReceiptResponse =
        dependencies.routePublicInferenceReceiptRequest(request, env, ctx)

      if (publicInferenceReceiptResponse !== undefined) {
        return yield* publicInferenceReceiptResponse
      }

      const publicCloudPrimitiveReceiptResponse =
        dependencies.routePublicCloudPrimitiveReceiptRequest(request, env, ctx)

      if (publicCloudPrimitiveReceiptResponse !== undefined) {
        return yield* publicCloudPrimitiveReceiptResponse
      }

      const publicCardCreditSpendReceiptResponse =
        dependencies.routePublicCardCreditSpendReceiptRequest(request, env, ctx)

      if (publicCardCreditSpendReceiptResponse !== undefined) {
        return yield* publicCardCreditSpendReceiptResponse
      }

      const publicStripeCheckoutReceiptResponse =
        dependencies.routePublicStripeCheckoutReceiptRequest(request, env, ctx)

      if (publicStripeCheckoutReceiptResponse !== undefined) {
        return yield* publicStripeCheckoutReceiptResponse
      }

      const publicSiteReferralPayoutReceiptResponse =
        dependencies.routePublicSiteReferralPayoutReceiptRequest(
          request,
          env,
          ctx,
        )

      if (publicSiteReferralPayoutReceiptResponse !== undefined) {
        return yield* publicSiteReferralPayoutReceiptResponse
      }

      const publicPartnerPayoutReceiptResponse =
        dependencies.routePublicPartnerPayoutReceiptRequest(request, env, ctx)

      if (publicPartnerPayoutReceiptResponse !== undefined) {
        return yield* publicPartnerPayoutReceiptResponse
      }

      const publicNip90MarketReceiptResponse =
        dependencies.routePublicNip90MarketReceiptRequest(request, env, ctx)

      if (publicNip90MarketReceiptResponse !== undefined) {
        return yield* publicNip90MarketReceiptResponse
      }

      const ecommerceCampaignReceiptResponse =
        dependencies.routeEcommerceCampaignReceiptRequest(request, env, ctx)

      if (ecommerceCampaignReceiptResponse !== undefined) {
        return yield* ecommerceCampaignReceiptResponse
      }

      const ecommerceCampaignReceiptOperatorResponse =
        dependencies.routeEcommerceCampaignReceiptOperatorRequest(request, env, ctx)

      if (ecommerceCampaignReceiptOperatorResponse !== undefined) {
        return yield* ecommerceCampaignReceiptOperatorResponse
      }

      const ecommerceCampaignSelfServeResponse =
        dependencies.routeEcommerceCampaignSelfServeRequest(request, env, ctx)

      if (ecommerceCampaignSelfServeResponse !== undefined) {
        return yield* ecommerceCampaignSelfServeResponse
      }

      const marketingAgencyReceiptResponse =
        dependencies.routeMarketingAgencyReceiptRequest(request, env, ctx)

      if (marketingAgencyReceiptResponse !== undefined) {
        return yield* marketingAgencyReceiptResponse
      }

      const marketingAgencySelfServeResponse =
        dependencies.routeMarketingAgencySelfServeRequest(request, env, ctx)

      if (marketingAgencySelfServeResponse !== undefined) {
        return yield* marketingAgencySelfServeResponse
      }

      const operatorAdjutantResponse =
        dependencies.routeOperatorAdjutantRequest(request, env, ctx)

      if (operatorAdjutantResponse !== undefined) {
        return yield* operatorAdjutantResponse
      }

      const operatorArtanisConsoleResponse =
        dependencies.routeOperatorArtanisConsoleRequest(request, env, ctx)

      if (operatorArtanisConsoleResponse !== undefined) {
        return yield* operatorArtanisConsoleResponse
      }

      const operatorOrderTriageResponse =
        dependencies.routeOperatorOrderTriageRequest(request, env, ctx)

      if (operatorOrderTriageResponse !== undefined) {
        return yield* operatorOrderTriageResponse
      }

      const operatorEmailInspectionResponse =
        dependencies.routeOperatorEmailInspectionRequest(request, env, ctx)

      if (operatorEmailInspectionResponse !== undefined) {
        return yield* operatorEmailInspectionResponse
      }

      const operatorPylonMarketplaceResponse =
        dependencies.routeOperatorPylonMarketplaceRequest(request, env, ctx)

      if (operatorPylonMarketplaceResponse !== undefined) {
        return yield* operatorPylonMarketplaceResponse
      }

      const operatorProviderAccountResponse =
        dependencies.routeOperatorProviderAccountRequest(request, env, ctx)

      if (operatorProviderAccountResponse !== undefined) {
        return yield* operatorProviderAccountResponse
      }

      const operatorSitesResponse = dependencies.routeOperatorSitesRequest(
        request,
        env,
        ctx,
      )

      if (operatorSitesResponse !== undefined) {
        return yield* operatorSitesResponse
      }

      const mulletResponse = dependencies.routeMulletRequest(request, env, ctx)

      if (mulletResponse !== undefined) {
        return yield* mulletResponse
      }

      // Forum thread document requests get per-thread Open Graph / Twitter
      // Card metadata injected into the server-rendered shell so shared
      // `/forum/t/{id}` links render rich previews for non-JS social crawlers.
      // Only GET/HEAD document navigations are intercepted; every other forum
      // request (including the `/og/forum/...svg` image and all `/api/forum`
      // calls) flows through routeForumRequest unchanged.
      const forumThreadPageMatch =
        (request.method === 'GET' || request.method === 'HEAD') &&
        acceptsDocument(request)
          ? /^\/forum\/t\/([^/]+)$/.exec(url.pathname)
          : null

      if (forumThreadPageMatch !== null) {
        const topicSegment = forumThreadPageMatch[1]
        const topicId =
          topicSegment === undefined
            ? undefined
            : safeDecodeTopicSegment(topicSegment)

        if (topicId !== undefined) {
          return yield* routeEffectOrResponse(
            dependencies.handleForumThreadPage(request, env, ctx, topicId),
          )
        }
      }

      const forumResponse = dependencies.routeForumRequest(request, env, ctx)

      if (forumResponse !== undefined) {
        return yield* forumResponse
      }

      const threadPageMatch = /^\/t\/([^/]+)$/.exec(url.pathname)

      if (threadPageMatch !== null) {
        const threadId = dependencies.optionalUuid(threadPageMatch[1])

        if (threadId === undefined) {
          return notFound()
        }

        return yield* routeEffectOrResponse(
          dependencies.handleThreadPage(request, env, ctx, threadId),
        )
      }

      if (url.pathname.startsWith('/api/')) {
        return yield* dependencies.routeSyncRequest(request, env, ctx)
      }

      if (url.pathname.startsWith('/assets/')) {
        return yield* routeEffectOrResponse(
          dependencies.handleAssetRequest(request, env),
        )
      }

      if (shouldRedirectUnknownDocumentToHome(request, url.pathname)) {
        return redirectResponse('/')
      }

      return yield* routeEffectOrResponse(
        dependencies.handleAppShellPage(request, env, ctx),
      )
    })
