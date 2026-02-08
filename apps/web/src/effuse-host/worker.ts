import { tryServeAsset } from "./assets"
import { handleAutopilotRequest } from "./autopilot"
import { handleAuthRequest } from "./auth"
import { handleCallbackRequest } from "./callback"
import { handleContractsRequest } from "./contracts"
import { handleSsrRequest } from "./ssr"
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

    const url = new URL(request.url)

    // Autopilot execution plane (Convex-first MVP).
    if (url.pathname.startsWith("/api/autopilot/")) {
      const response = await handleAutopilotRequest(request, env, ctx)
      if (response) return response
    }

    if (url.pathname.startsWith("/api/contracts/")) {
      const response = await handleContractsRequest(request)
      if (response) return response
    }

    // WorkOS OAuth callback (legacy path configured as WORKOS_REDIRECT_URI).
    if (url.pathname === "/callback") {
      return handleCallbackRequest(request)
    }

    if (url.pathname.startsWith("/api/auth/")) {
      const response = await handleAuthRequest(request, env)
      if (response) return response
    }

    // Static assets.
    const asset = await tryServeAsset(request, env)
    if (asset) return asset

    // SSR (GET/HEAD only).
    if (request.method === "GET" || request.method === "HEAD") {
      return handleSsrRequest(request, env)
    }

    return new Response("Not found", { status: 404 })
  },
} satisfies ExportedHandler<WorkerEnv>
