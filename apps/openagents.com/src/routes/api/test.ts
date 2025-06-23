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
  console.log("🧪 TEST ROUTE: Response:", response)
  console.log("🧪 TEST ROUTE: Response keys:", Object.keys(response))
  console.log("🧪 TEST ROUTE: Response _tag:", (response as any)._tag)
  console.log("🧪 TEST ROUTE: Is Effect?", Effect.isEffect(response))
  
  // Let's also try creating a simple Effect directly
  const simpleEffect = Effect.succeed("test")
  console.log("🧪 TEST ROUTE: Simple Effect:", simpleEffect)
  console.log("🧪 TEST ROUTE: Simple Effect keys:", Object.keys(simpleEffect))
  console.log("🧪 TEST ROUTE: Simple Effect _tag:", (simpleEffect as any)._tag)
  console.log("🧪 TEST ROUTE: Is Simple Effect?", Effect.isEffect(simpleEffect))
  
  return response
}