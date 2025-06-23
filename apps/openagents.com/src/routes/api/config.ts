import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"

/**
 * GET /api/config - Get configuration status
 */
export function getConfig(_ctx: RouteContext) {
  const config = {
    hasOpenRouterKey: !!process.env.OPENROUTER_API_KEY,
    hasCloudflareKey: !!process.env.CLOUDFLARE_API_KEY
  }

  return HttpServerResponse.json(config)
}
