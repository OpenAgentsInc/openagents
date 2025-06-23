import { renderMarkdown } from "@openagentsinc/psionic"
import { Effect } from "effect"

/**
 * POST /api/markdown - Render markdown to HTML with syntax highlighting
 */
export async function renderMarkdownRoute(ctx: any) {
  try {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = await Effect.runPromise(
      Effect.gen(function*() {
        const request = ctx.request
        return yield* request.text
      }) as Effect.Effect<string, never, never>
    )

    const body = JSON.parse(bodyText)
    const { content } = body

    if (!content) {
      return new Response(JSON.stringify({ error: "Content is required" }), {
        status: 400,
        headers: { "Content-Type": "application/json" }
      })
    }

    // Render markdown with syntax highlighting
    const rendered = await renderMarkdown(content)

    return new Response(JSON.stringify({ html: rendered }), {
      headers: { "Content-Type": "application/json" }
    })
  } catch (error) {
    console.error("Failed to render markdown:", error)
    return new Response(JSON.stringify({ error: "Failed to render markdown" }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    })
  }
}
