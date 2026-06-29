import { Effect } from 'effect'

import { type RouteEffect, routeEffectOrResponse } from './http/route-effects'
import type { Env } from './index'

type OmniEnvironment = Env
type OmniSessionRouteHandler = (
  request: Request,
  env: OmniEnvironment,
  ctx: ExecutionContext,
) => RouteEffect
type OmniSimpleRouteHandler = (
  request: Request,
  environment: OmniEnvironment,
) => RouteEffect
type OmniDetailRouteHandler = (
  request: Request,
  env: OmniEnvironment,
  ctx: ExecutionContext,
  id: string,
) => RouteEffect

type OmniRouteDependencies = Readonly<{
  handleAutopilotFleetApi: OmniSessionRouteHandler
  handleAutopilotTokenLeaderboardsApi: OmniSessionRouteHandler
  handleBillingCheckoutApi: OmniSessionRouteHandler
  handleBillingInferenceCreditApi: OmniSessionRouteHandler
  handleBillingAutoTopUpPolicyApi: OmniSessionRouteHandler
  handleBillingAutoTopUpRunApi: OmniSessionRouteHandler
  handleBillingCouponRedeemApi: OmniSessionRouteHandler
  handleBillingSummaryApi: OmniSessionRouteHandler
  handleBillingStripeCheckoutReturnApi: OmniSimpleRouteHandler
  handleBillingStripeSetupIntentApi: OmniSessionRouteHandler
  handleBillingStripeSetupIntentSaveApi: OmniSessionRouteHandler
  handleBillingStripeWebhookApi: OmniSimpleRouteHandler
  handleEmailResendWebhookApi: OmniSimpleRouteHandler
  handleOmniAgentRunDetailApi: OmniDetailRouteHandler
  handleOmniAgentRunEventsApi: OmniDetailRouteHandler
  handleOmniAgentRunsApi: OmniSessionRouteHandler
  handleOmniDeploymentDetailApi: OmniDetailRouteHandler
  handleOmniDeploymentEventsApi: OmniDetailRouteHandler
  handleOmniDeploymentsApi: OmniSessionRouteHandler
  handleOmniOperatorAgentRunDetailApi: (
    request: Request,
    env: Env,
    runId: string,
  ) => RouteEffect
  handleOmniOperatorAgentRunsApi: (request: Request, env: Env) => RouteEffect
  handleOmniOperatorBillingCreditsApi: (
    request: Request,
    env: Env,
  ) => RouteEffect
  handleOmniOperatorInferenceCreditApi: (
    request: Request,
    env: Env,
  ) => RouteEffect
  handleOmniOperatorDeploymentsApi: (request: Request, env: Env) => RouteEffect
  handleOmniOperatorFleetApi: (request: Request, env: Env) => RouteEffect
  handleOmniOperatorTeamChatMessagesApi: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => RouteEffect
}>

export const makeOmniRoutes = (dependencies: OmniRouteDependencies) => ({
  routeOmniRequest: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/autopilot/fleet') {
      return routeEffectOrResponse(
        dependencies.handleAutopilotFleetApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/autopilot/token-leaderboards') {
      return routeEffectOrResponse(
        dependencies.handleAutopilotTokenLeaderboardsApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/summary') {
      return routeEffectOrResponse(
        dependencies.handleBillingSummaryApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/coupons/redeem') {
      return routeEffectOrResponse(
        dependencies.handleBillingCouponRedeemApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/checkout') {
      return routeEffectOrResponse(
        dependencies.handleBillingCheckoutApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/inference-credit') {
      return routeEffectOrResponse(
        dependencies.handleBillingInferenceCreditApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/auto-top-up-policy') {
      return routeEffectOrResponse(
        dependencies.handleBillingAutoTopUpPolicyApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/auto-top-up/run') {
      return routeEffectOrResponse(
        dependencies.handleBillingAutoTopUpRunApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/stripe/setup-intents') {
      return routeEffectOrResponse(
        dependencies.handleBillingStripeSetupIntentApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/stripe/setup-intents/save') {
      return routeEffectOrResponse(
        dependencies.handleBillingStripeSetupIntentSaveApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/billing/stripe/webhook') {
      return routeEffectOrResponse(
        dependencies.handleBillingStripeWebhookApi(request, env),
      )
    }

    if (url.pathname === '/api/webhooks/resend') {
      return routeEffectOrResponse(
        dependencies.handleEmailResendWebhookApi(request, env),
      )
    }

    if (url.pathname === '/api/billing/stripe/checkout-return') {
      return routeEffectOrResponse(
        dependencies.handleBillingStripeCheckoutReturnApi(request, env),
      )
    }

    if (url.pathname === '/api/omni/operator/fleet') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorFleetApi(request, env),
      )
    }

    if (url.pathname === '/api/omni/operator/team-chat/messages') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorTeamChatMessagesApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/omni/operator/billing/credits') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorBillingCreditsApi(request, env),
      )
    }

    if (url.pathname === '/api/omni/operator/billing/inference-credit') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorInferenceCreditApi(request, env),
      )
    }

    if (url.pathname === '/api/omni/operator/agent-runs') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorAgentRunsApi(request, env),
      )
    }

    if (
      url.pathname === '/api/operator/autopilot/preflight' ||
      url.pathname === '/api/omni/operator/autopilot/preflight' ||
      url.pathname === '/api/omni/operator/autopilot/checklist'
    ) {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorAgentRunsApi(request, env),
      )
    }

    const omniOperatorAgentRunDetailMatch =
      /^\/api\/omni\/operator\/agent-runs\/([^/]+)(?:\/(?:callbacks\/retry|continue|checklist))?$/.exec(
        url.pathname,
      )

    if (omniOperatorAgentRunDetailMatch !== null) {
      const runId = omniOperatorAgentRunDetailMatch[1]

      if (runId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleOmniOperatorAgentRunDetailApi(request, env, runId),
        )
      }
    }

    if (url.pathname === '/api/omni/operator/deployments') {
      return routeEffectOrResponse(
        dependencies.handleOmniOperatorDeploymentsApi(request, env),
      )
    }

    if (
      url.pathname === '/api/autopilot/missions' ||
      url.pathname === '/api/omni/agent-runs'
    ) {
      return routeEffectOrResponse(
        dependencies.handleOmniAgentRunsApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/omni/deployments') {
      return routeEffectOrResponse(
        dependencies.handleOmniDeploymentsApi(request, env, ctx),
      )
    }

    const omniAgentRunEventsMatch =
      /^\/api\/omni\/agent-runs\/([^/]+)\/events(?:\/ingest)?$/.exec(
        url.pathname,
      )

    if (omniAgentRunEventsMatch !== null) {
      const runId = omniAgentRunEventsMatch[1]

      if (runId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleOmniAgentRunEventsApi(request, env, ctx, runId),
        )
      }
    }

    const omniAgentRunDetailMatch = /^\/api\/omni\/agent-runs\/([^/]+)$/.exec(
      url.pathname,
    )

    if (omniAgentRunDetailMatch !== null) {
      const runId = omniAgentRunDetailMatch[1]

      if (runId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleOmniAgentRunDetailApi(request, env, ctx, runId),
        )
      }
    }

    const omniDeploymentEventsMatch =
      /^\/api\/omni\/deployments\/([^/]+)\/events(?:\/ingest)?$/.exec(
        url.pathname,
      )

    if (omniDeploymentEventsMatch !== null) {
      const deployId = omniDeploymentEventsMatch[1]

      if (deployId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleOmniDeploymentEventsApi(
            request,
            env,
            ctx,
            deployId,
          ),
        )
      }
    }

    const omniDeploymentDetailMatch =
      /^\/api\/omni\/deployments\/([^/]+)$/.exec(url.pathname)

    if (omniDeploymentDetailMatch !== null) {
      const deployId = omniDeploymentDetailMatch[1]

      if (deployId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleOmniDeploymentDetailApi(
            request,
            env,
            ctx,
            deployId,
          ),
        )
      }
    }

    return undefined
  },
})
