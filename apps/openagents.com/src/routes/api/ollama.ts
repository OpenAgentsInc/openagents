import type { HttpServerRequest } from "@effect/platform"
import { HttpServerResponse } from "@effect/platform"
import * as Ai from "@openagentsinc/ai"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"

/**
 * GET /api/ollama/status - Check Ollama status
 */
export function ollamaStatus(_ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, never> {
  return Effect.gen(function*() {
    // Use the Ollama provider's checkStatus function
    const status = yield* Ai.Ollama.checkStatus()
    return yield* HttpServerResponse.json(status)
  }).pipe(
    Effect.catchAll(() => HttpServerResponse.json({ online: false, models: [], modelCount: 0 }, { status: 503 }))
  )
}

/**
 * POST /api/ollama/chat - Stream chat completion
 */
export function ollamaChat(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text
    const body = JSON.parse(bodyText)
    const { messages, model, options } = body

    // Create a TransformStream for streaming response
    const stream = new TransformStream()
    const writer = stream.writable.getWriter()
    const encoder = new TextEncoder()

    // Run the streaming in background
    Effect.runPromise(
      Effect.gen(function*() {
        const client = yield* Ai.Ollama.OllamaClient

        // Get the async generator directly (not wrapped in Effect)
        const generator = client.chat({
          model,
          messages,
          stream: true,
          options: {
            temperature: options?.temperature || 0.7,
            num_ctx: options?.num_ctx || 4096,
            ...options
          }
        })

        // Process the generator manually within Effect
        yield* Effect.tryPromise({
          try: async () => {
            for await (const chunk of generator) {
              const responseChunk = {
                model,
                created_at: new Date().toISOString(),
                message: {
                  role: "assistant" as const,
                  content: chunk.content
                },
                done: chunk.done || false
              }
              await writer.write(encoder.encode(`data: ${JSON.stringify(responseChunk)}\n\n`))
            }
          },
          catch: (error) => new Error(`Stream processing error: ${error}`)
        })

        // Send completion signal
        writer.write(encoder.encode(`data: [DONE]\n\n`))
        writer.close()
      }).pipe(
        Effect.provide(Ai.Ollama.OllamaClientLive()),
        Effect.catchAll((error) =>
          Effect.sync(() => {
            console.error("Chat streaming error:", error)
            writer.write(encoder.encode(`data: ${JSON.stringify({ error: error.message })}\n\n`))
            writer.close()
          })
        )
      )
    )

    return yield* Effect.succeed(
      HttpServerResponse.raw(
        new Response(stream.readable, {
          headers: {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            "Connection": "keep-alive"
          }
        })
      )
    )
  }).pipe(
    Effect.catchAll((error: any) => {
      console.error("Chat API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 })
    })
  )
}
