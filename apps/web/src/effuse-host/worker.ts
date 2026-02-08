import { tryServeAsset } from "./assets"
import { handleAutopilotRequest } from "./autopilot"
import { handleAuthRequest } from "./auth"
import { handleCallbackRequest } from "./callback"
import { handleContractsRequest } from "./contracts"
import { handleDseCompileRequest } from "./dseCompile"
import { handleDseAdminRequest } from "./dseAdmin"
import { handleSsrRequest } from "./ssr"
import { handleStorybookApiRequest } from "./storybook"
import {
  formatRequestIdLogToken,
  getOrCreateRequestId,
  withRequestIdHeader,
  withResponseRequestIdHeader,
} from "./requestId"
import type { WorkerEnv } from "./env"

/**
 * Cloudflare Durable Object compatibility shims.
 *
 * Production currently has existing DO instances created under these class names.
 * Even though the MVP path is Convex-first, Cloudflare requires that we continue
 * exporting the classes (or explicitly delete them via a migration) to deploy.
 *
 * Note: These classes are not on the MVP hot path; they exist to keep deploys
 * unblocked while we decommission/migrate old execution-plane state.
 */
export class UserSpaceDO {
  constructor(
    readonly _state: DurableObjectState,
    readonly _env: WorkerEnv,
  ) {}

  async fetch(_request: Request): Promise<Response> {
    return new Response("UserSpaceDO is deprecated (Convex-first MVP).", { status: 410 })
  }
}

export class Chat {
  constructor(
    readonly _state: DurableObjectState,
    readonly _env: WorkerEnv,
  ) {}

  async fetch(_request: Request): Promise<Response> {
    return new Response("Chat DO is deprecated (Convex-first MVP).", { status: 410 })
  }
}

export default {
  async fetch(request: Request, env: WorkerEnv, ctx: ExecutionContext): Promise<Response> {
    ctx.passThroughOnException()

    const requestId = getOrCreateRequestId(request)
    const requestWithId = withRequestIdHeader(request, requestId)
    const url = new URL(requestWithId.url)

    // Autopilot execution plane (Convex-first MVP).
    if (url.pathname.startsWith("/api/autopilot/")) {
      const response = await handleAutopilotRequest(requestWithId, env, ctx)
      if (response) return withResponseRequestIdHeader(response, requestId)
    }

    if (url.pathname.startsWith("/api/contracts/")) {
      const response = await handleContractsRequest(requestWithId)
      if (response) return withResponseRequestIdHeader(response, requestId)
    }

    if (url.pathname.startsWith("/api/dse/")) {
      const response = await handleDseCompileRequest(requestWithId, env, ctx)
      if (response) return withResponseRequestIdHeader(response, requestId)
      const admin = await handleDseAdminRequest(requestWithId, env, ctx)
      if (admin) return withResponseRequestIdHeader(admin, requestId)
    }

    // WorkOS OAuth callback (legacy path configured as WORKOS_REDIRECT_URI).
    if (url.pathname === "/callback") {
      const response = await handleCallbackRequest(requestWithId)
      return withResponseRequestIdHeader(response, requestId)
    }

    if (url.pathname.startsWith("/api/auth/")) {
      const response = await handleAuthRequest(requestWithId, env)
      if (response) return withResponseRequestIdHeader(response, requestId)
    }

    // Storybook metadata API (used by visual regression tests).
    const storybookApi = await handleStorybookApiRequest(requestWithId)
    if (storybookApi) return withResponseRequestIdHeader(storybookApi, requestId)

    // Static assets.
    const asset = await tryServeAsset(requestWithId, env)
    if (asset) return withResponseRequestIdHeader(asset, requestId)

    // SSR (GET/HEAD only).
    if (request.method === "GET" || request.method === "HEAD") {
      try {
        const response = await handleSsrRequest(requestWithId, env)
        return withResponseRequestIdHeader(response, requestId)
      } catch (err) {
        // Always surface an ID users can report + operators can tail logs for.
        console.error(`[worker:ssr] ${formatRequestIdLogToken(requestId)}`, err)
        return withResponseRequestIdHeader(new Response("SSR failed", { status: 500 }), requestId)
      }
    }

    return withResponseRequestIdHeader(new Response("Not found", { status: 404 }), requestId)
  },
} satisfies ExportedHandler<WorkerEnv>
