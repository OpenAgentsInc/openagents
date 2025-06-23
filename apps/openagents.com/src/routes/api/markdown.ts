import { renderMarkdown } from "@openagentsinc/psionic"
import { Effect } from "effect"
import { HttpServerRequest, HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"

/**
 * POST /api/markdown - Render markdown to HTML with syntax highlighting
 */
export function renderMarkdownRoute(ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function* () {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = yield* ctx.request.text.pipe(Effect.orDie)

    const body = JSON.parse(bodyText)
    const { content } = body

    if (!content) {
      return yield* HttpServerResponse.json(
        { error: "Content is required" },
        { status: 400 }
      ).pipe(Effect.orDie)
    }

    // Render markdown with syntax highlighting
    const rendered = yield* Effect.promise(() => renderMarkdown(content))

    return yield* HttpServerResponse.json({ html: rendered }).pipe(Effect.orDie)
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to render markdown:", error)
      return HttpServerResponse.json(
        { error: "Failed to render markdown" },
        { status: 500 }
      ).pipe(Effect.orDie)
    })
  )
}
