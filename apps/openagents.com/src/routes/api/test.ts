import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"

/**
 * GET /api/test - Test route to verify Effect handling
 */
export function testRoute(_ctx: RouteContext) {
  return HttpServerResponse.json({
    message: "Test route working",
    timestamp: new Date().toISOString()
  })
}
