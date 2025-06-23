import { HttpServerResponse } from "@effect/platform"
import * as Ai from "@openagentsinc/ai"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect, Stream } from "effect"

/**
 * GET /api/ollama/status - Check Ollama status
 */
export function ollamaStatus(_ctx: RouteContext) {
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
) {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text
    const body = JSON.parse(bodyText)
    const { messages, model, options } = body

    // Create and run the streaming effect
    const readableStream = yield* Effect.gen(function*() {
      const client = yield* Ai.Ollama.OllamaClient
      const encoder = new TextEncoder()

      // Get the async generator from the client
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

      // Create a stream from the async generator
      const sseStream = Stream.fromAsyncIterable(generator, (error) => 
        new Error(`Ollama stream error: ${error}`)
      ).pipe(
        Stream.map((chunk) => {
          const responseChunk = {
            model,
            created_at: new Date().toISOString(),
            message: {
              role: "assistant" as const,
              content: chunk.content
            },
            done: chunk.done || false
          }
          return encoder.encode(`data: ${JSON.stringify(responseChunk)}\n\n`)
        }),
        Stream.concat(Stream.make(encoder.encode(`data: [DONE]\n\n`))),
        Stream.catchAll((error) => {
          console.error("Ollama streaming error:", error)
          return Stream.make(encoder.encode(`data: ${JSON.stringify({ error: error.message })}\n\n`))
        })
      )

      // Convert to ReadableStream
      return yield* Stream.toReadableStreamEffect(sseStream)
    }).pipe(Effect.provide(Ai.Ollama.OllamaClientLive()))

    return HttpServerResponse.raw(
      new Response(readableStream, {
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          "Connection": "keep-alive"
        }
      })
    )
  }).pipe(
    Effect.catchAll((error: any) => {
      console.error("Chat API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 })
    })
  )
}