import { Effect } from "effect"
import type { PsionicApp, PsionicContext } from "@openagentsinc/psionic"

export function configApi(app: PsionicApp) {
  // Get configuration status
  app.get("/api/config", async (ctx: PsionicContext) => {
    try {
      const config = {
        hasOpenRouterKey: !!process.env.OPENROUTER_API_KEY,
        hasCloudflareKey: !!process.env.CLOUDFLARE_API_KEY
      }
      
      return new Response(JSON.stringify(config), {
        headers: { "Content-Type": "application/json" }
      })
    } catch (error) {
      console.error("Config check error:", error)
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      })
    }
  })
}