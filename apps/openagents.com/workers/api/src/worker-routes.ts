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
  optionalUuid: (value: string | undefined) => string | undefined
  routeAutopilotWorkRequest: OptionalEffectRoute
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
  routeMulletRequest: OptionalEffectRoute
  routeOmniRequest: OptionalEffectRoute
  routeOnboardingRequest: OptionalEffectRoute
  routeNexusPylonVisibilityRequest: OptionalEffectRoute
  routePublicNip90MarketReceiptRequest: OptionalEffectRoute
  routePylonApiRequest: OptionalEffectRoute
  routeSiteCommerceRequest: OptionalEffectRoute
  routeSiteReferralInspectionRequest: OptionalEffectRoute
  routeSiteReferralPayoutLedgerRequest: OptionalEffectRoute
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
  /^\/demo(?:\/.*)?$/,
  /^\/dashboard$/,
  /^\/docs(?:\/[^/]+)?$/,
  /^\/files(?:\/[^/]+)?$/,
  /^\/forum(?:\/.*)?$/,
  /^\/images$/,
  /^\/moksha$/,
  /^\/moksha2$/,
  /^\/mullet$/,
  /^\/pylon$/,
  /^\/live$/,
  /^\/onboarding$/,
  /^\/order$/,
  /^\/orders\/[^/]+$/,
  /^\/promises$/,
  /^\/settings(?:\/[^/]+)?$/,
  /^\/share\/[^/]+$/,
  /^\/sites\/demo-checkout(?:\/[^/]+)?$/,
  /^\/stats$/,
  /^\/stats-old$/,
  /^\/teams\/[^/]+(?:\/chat|\/files(?:\/[^/]+)?|\/projects\/[^/]+\/chat)$/,
  /^\/t\/[^/]+$/,
  /^\/training\/runs(?:\/[^/]+)?$/,
  /^\/usage$/,
]

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

      const pylonApiResponse = dependencies.routePylonApiRequest(
        request,
        env,
        ctx,
      )

      if (pylonApiResponse !== undefined) {
        return yield* pylonApiResponse
      }

      const trainingRunWindowResponse =
        dependencies.routeTrainingRunWindowRequest(request, env, ctx)

      if (trainingRunWindowResponse !== undefined) {
        return yield* trainingRunWindowResponse
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

      const publicNip90MarketReceiptResponse =
        dependencies.routePublicNip90MarketReceiptRequest(request, env, ctx)

      if (publicNip90MarketReceiptResponse !== undefined) {
        return yield* publicNip90MarketReceiptResponse
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
