import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"

/**
 * GET /api/test - Test route to verify Effect handling
 */
export function testRoute(_ctx: RouteContext) {
  console.log("🧪 TEST ROUTE: Handler called")
  
  // Test 1: Direct HttpServerResponse.json
  const response = HttpServerResponse.json({ 
    message: "Test route working",
    timestamp: new Date().toISOString()
  })
  
  console.log("🧪 TEST ROUTE: Response type:", typeof response)
  console.log("🧪 TEST ROUTE: Is Effect?", Effect.isEffect(response))
  
  return response
}