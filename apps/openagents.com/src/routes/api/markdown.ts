import type { HttpServerRequest } from "@effect/platform"
import { HttpServerResponse } from "@effect/platform"
import { renderMarkdown } from "@openagentsinc/psionic"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"

/**
 * POST /api/markdown - Render markdown to HTML with syntax highlighting
 */
export function renderMarkdownRoute(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = yield* ctx.request.text

    const body = JSON.parse(bodyText)
    const { content } = body

    if (!content) {
      return yield* HttpServerResponse.json(
        { error: "Content is required" },
        { status: 400 }
      )
    }

    // Render markdown with syntax highlighting
    const rendered = yield* Effect.promise(() => renderMarkdown(content))

    return yield* HttpServerResponse.json({ html: rendered })
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Failed to render markdown:", error)
      return HttpServerResponse.json(
        { error: "Failed to render markdown" },
        { status: 500 }
      )
    })
  )
}
