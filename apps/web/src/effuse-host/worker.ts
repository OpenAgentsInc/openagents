import { tryServeAsset } from "./assets"
import { handleAutopilotRequest } from "./autopilot"
import { handleAuthRequest } from "./auth"
import { handleCallbackRequest } from "./callback"
import { handleContractsRequest } from "./contracts"
import { handleSsrRequest } from "./ssr"
import type { WorkerEnv } from "./env"

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
