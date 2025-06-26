import { HttpServerResponse } from "@effect/platform"
import * as Ai from "@openagentsinc/ai"
import { Effect } from "effect"

export const ollamaRoutes = [
  {
    method: "GET" as const,
    path: "/api/ollama/status",
    handler: Effect.gen(function*() {
      try {
        const status = yield* Ai.Ollama.checkStatus()
        return HttpServerResponse.json(status)
      } catch {
        return HttpServerResponse.json(
          { online: false, models: [], modelCount: 0 },
          { status: 503 }
        )
      }
    })
  },
  {
    method: "POST" as const,
    path: "/api/ollama/chat",
    handler: HttpServerResponse.json(
      { error: "Streaming not yet implemented in Effect version" },
      { status: 501 }
    )
  }
]
