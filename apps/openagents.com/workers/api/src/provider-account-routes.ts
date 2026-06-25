import { Effect } from 'effect'

import { type RouteEffect, routeEffectOrResponse } from './http/route-effects'
import type { Env as OpenAgentsEnv } from './index'

type HttpResponse = globalThis.Response

type ProviderAccountRouteDependencies<Bindings = OpenAgentsEnv> = Readonly<{
  handleGitHubWriteDisconnectApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    connectionRef: string,
  ) => RouteEffect
  handleProviderAccountDisconnectApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    providerAccountRef: string,
  ) => RouteEffect
  handleProviderAccountGrantIssueApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    providerAccountRef: string,
  ) => RouteEffect
  handleProviderAccountGrantResolveApi: (
    request: Request,
    env: Bindings,
  ) => RouteEffect
  handleGoogleGeminiGrantResolveApi: (
    request: Request,
    env: Bindings,
  ) => RouteEffect
  handleGoogleGeminiBuiltinGrantApi: (
    request: Request,
    env: Bindings,
  ) => RouteEffect
  handleGoogleGeminiGenerateContentApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    model: string,
  ) => RouteEffect
  handleProviderAccountHealthApi: (
    request: Request,
    env: Bindings,
    providerAccountRef: string,
  ) => RouteEffect
  handleProviderApiKeyConnectApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    providerRouteSegment: string,
  ) => RouteEffect
  handleProviderAccountPoolApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => RouteEffect
  handleProviderAccountUsageApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => RouteEffect
  handleProviderAccountsListApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => RouteEffect
  handleProviderDeviceLoginConnectedApi: (
    request: Request,
    env: Bindings,
    attemptId: string,
  ) => RouteEffect
  handleProviderDeviceLoginFailedApi: (
    request: Request,
    env: Bindings,
    attemptId: string,
  ) => RouteEffect
  handleProviderDeviceLoginStartApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => RouteEffect
  handleProviderDeviceLoginStatusApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    attemptId: string,
  ) => RouteEffect
  handlePylonProviderDeviceLoginStartApi: (
    request: Request,
    env: Bindings,
  ) => RouteEffect
  handlePylonProviderDeviceLoginStatusApi: (
    request: Request,
    env: Bindings,
    attemptId: string,
  ) => RouteEffect
  handlePylonOpenAgentsAuthStartApi: (
    request: Request,
    env: Bindings,
  ) => RouteEffect
  handlePylonOpenAgentsAuthStatusApi: (
    request: Request,
    env: Bindings,
    attemptId: string,
  ) => RouteEffect
  handlePylonOpenAgentsAuthVerifyApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => RouteEffect
}>

export const makeProviderAccountRoutes = <Bindings = OpenAgentsEnv>(
  dependencies: ProviderAccountRouteDependencies<Bindings>,
) => ({
  routeProviderAccountRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/provider-accounts') {
      return routeEffectOrResponse(
        dependencies.handleProviderAccountsListApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/provider-accounts/pool') {
      return routeEffectOrResponse(
        dependencies.handleProviderAccountPoolApi(request, env, ctx),
      )
    }

    if (url.pathname === '/api/admin/provider-accounts/usage') {
      return routeEffectOrResponse(
        dependencies.handleProviderAccountUsageApi(request, env, ctx),
      )
    }

    if (
      url.pathname === '/api/provider-accounts/chatgpt-codex/device-login/start'
    ) {
      return routeEffectOrResponse(
        dependencies.handleProviderDeviceLoginStartApi(request, env, ctx),
      )
    }

    if (
      url.pathname ===
      '/api/pylon/provider-accounts/chatgpt-codex/device-login/start'
    ) {
      return routeEffectOrResponse(
        dependencies.handlePylonProviderDeviceLoginStartApi(request, env),
      )
    }

    if (url.pathname === '/api/pylon/auth/openagents/device/start') {
      return routeEffectOrResponse(
        dependencies.handlePylonOpenAgentsAuthStartApi(request, env),
      )
    }

    if (url.pathname === '/api/pylon/auth/openagents/device/verify') {
      return routeEffectOrResponse(
        dependencies.handlePylonOpenAgentsAuthVerifyApi(request, env, ctx),
      )
    }

    if (
      url.pathname === '/api/provider-accounts/chatgpt-codex/grants/resolve'
    ) {
      return routeEffectOrResponse(
        dependencies.handleProviderAccountGrantResolveApi(request, env),
      )
    }

    if (
      url.pathname === '/api/provider-accounts/google-gemini/grants/resolve'
    ) {
      return routeEffectOrResponse(
        dependencies.handleGoogleGeminiGrantResolveApi(request, env),
      )
    }

    if (
      url.pathname === '/api/provider-accounts/google-gemini/grants/builtin'
    ) {
      return routeEffectOrResponse(
        dependencies.handleGoogleGeminiBuiltinGrantApi(request, env),
      )
    }

    const providerApiKeyConnectMatch =
      /^\/api\/provider-accounts\/(anthropic|google-gemini)\/connect$/.exec(
        url.pathname,
      )

    if (providerApiKeyConnectMatch !== null) {
      const providerRouteSegment = providerApiKeyConnectMatch[1]

      if (providerRouteSegment !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderApiKeyConnectApi(
            request,
            env,
            ctx,
            providerRouteSegment,
          ),
        )
      }
    }

    const googleGeminiGenerateMatch =
      /^\/api\/provider-accounts\/google-gemini\/models\/([^/]+):streamGenerateContent$/.exec(
        url.pathname,
      )

    if (googleGeminiGenerateMatch !== null) {
      const model = googleGeminiGenerateMatch[1]

      if (model !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleGoogleGeminiGenerateContentApi(
            request,
            env,
            ctx,
            model,
          ),
        )
      }
    }

    const providerDeviceLoginConnectedMatch =
      /^\/api\/provider-accounts\/chatgpt-codex\/device-login\/([^/]+)\/connected$/.exec(
        url.pathname,
      )

    if (providerDeviceLoginConnectedMatch !== null) {
      const attemptId = providerDeviceLoginConnectedMatch[1]

      if (attemptId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderDeviceLoginConnectedApi(
            request,
            env,
            attemptId,
          ),
        )
      }
    }

    const providerDeviceLoginFailedMatch =
      /^\/api\/provider-accounts\/chatgpt-codex\/device-login\/([^/]+)\/failed$/.exec(
        url.pathname,
      )

    if (providerDeviceLoginFailedMatch !== null) {
      const attemptId = providerDeviceLoginFailedMatch[1]

      if (attemptId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderDeviceLoginFailedApi(
            request,
            env,
            attemptId,
          ),
        )
      }
    }

    const providerDeviceLoginStatusMatch =
      /^\/api\/provider-accounts\/chatgpt-codex\/device-login\/([^/]+)$/.exec(
        url.pathname,
      )

    if (providerDeviceLoginStatusMatch !== null) {
      const attemptId = providerDeviceLoginStatusMatch[1]

      if (attemptId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderDeviceLoginStatusApi(
            request,
            env,
            ctx,
            attemptId,
          ),
        )
      }
    }

    const pylonProviderDeviceLoginStatusMatch =
      /^\/api\/pylon\/provider-accounts\/chatgpt-codex\/device-login\/([^/]+)$/.exec(
        url.pathname,
      )

    if (pylonProviderDeviceLoginStatusMatch !== null) {
      const attemptId = pylonProviderDeviceLoginStatusMatch[1]

      if (attemptId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handlePylonProviderDeviceLoginStatusApi(
            request,
            env,
            attemptId,
          ),
        )
      }
    }

    const pylonOpenAgentsAuthStatusMatch =
      /^\/api\/pylon\/auth\/openagents\/device\/([^/]+)$/.exec(url.pathname)

    if (pylonOpenAgentsAuthStatusMatch !== null) {
      const attemptId = pylonOpenAgentsAuthStatusMatch[1]

      if (attemptId !== undefined) {
        return routeEffectOrResponse(
          dependencies.handlePylonOpenAgentsAuthStatusApi(
            request,
            env,
            attemptId,
          ),
        )
      }
    }

    const githubWriteDisconnectMatch =
      /^\/api\/github-write\/connections\/([^/]+)\/disconnect$/.exec(
        url.pathname,
      )

    if (githubWriteDisconnectMatch !== null) {
      const connectionRef = githubWriteDisconnectMatch[1]

      if (connectionRef !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleGitHubWriteDisconnectApi(
            request,
            env,
            ctx,
            connectionRef,
          ),
        )
      }
    }

    const providerHealthMatch =
      /^\/api\/provider-accounts\/([^/]+)\/health$/.exec(url.pathname)

    if (providerHealthMatch !== null) {
      const providerAccountRef = providerHealthMatch[1]

      if (providerAccountRef !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderAccountHealthApi(
            request,
            env,
            providerAccountRef,
          ),
        )
      }
    }

    const providerGrantIssueMatch =
      /^\/api\/provider-accounts\/([^/]+)\/grants$/.exec(url.pathname)

    if (providerGrantIssueMatch !== null) {
      const providerAccountRef = providerGrantIssueMatch[1]

      if (providerAccountRef !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderAccountGrantIssueApi(
            request,
            env,
            ctx,
            providerAccountRef,
          ),
        )
      }
    }

    const providerDisconnectMatch =
      /^\/api\/provider-accounts\/([^/]+)\/disconnect$/.exec(url.pathname)

    if (providerDisconnectMatch !== null) {
      const providerAccountRef = providerDisconnectMatch[1]

      if (providerAccountRef !== undefined) {
        return routeEffectOrResponse(
          dependencies.handleProviderAccountDisconnectApi(
            request,
            env,
            ctx,
            providerAccountRef,
          ),
        )
      }
    }

    return undefined
  },
})
