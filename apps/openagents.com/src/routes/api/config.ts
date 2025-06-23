import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"

/**
 * GET /api/config - Get configuration status
 */
export function getConfig(_ctx: RouteContext) {
  console.log("🔍 CONFIG ROUTE: Handler called")

  const config = {
    hasOpenRouterKey: !!process.env.OPENROUTER_API_KEY,
    hasCloudflareKey: !!process.env.CLOUDFLARE_API_KEY
  }

  const response = HttpServerResponse.json(config)
  console.log("🔍 CONFIG ROUTE: Response:", response)
  console.log("🔍 CONFIG ROUTE: Response type:", typeof response)
  console.log("🔍 CONFIG ROUTE: Response keys:", Object.keys(response))
  console.log("🔍 CONFIG ROUTE: Response _tag:", (response as any)._tag)
  console.log("🔍 CONFIG ROUTE: Response _id:", (response as any)._id)

  return response
}
