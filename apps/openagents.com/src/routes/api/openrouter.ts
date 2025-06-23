import { FetchHttpClient, Headers, HttpServerResponse } from "@effect/platform"
import { BunHttpPlatform } from "@effect/platform-bun"
import * as Ai from "@openagentsinc/ai"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect, Layer, Option, Redacted, Stream } from "effect"

/**
 * GET /api/openrouter/status - Validate API key
 */
export function openrouterStatus(
  ctx: RouteContext
) {
  return Effect.gen(function*() {
    // Get header from Effect HttpServerRequest
    const headers = ctx.request.headers
    const apiKeyFromHeader = Option.getOrNull(Headers.get(headers, "x-api-key"))

    // Use header API key first, fall back to environment variable
    const apiKey = apiKeyFromHeader || process.env.OPENROUTER_API_KEY

    if (!apiKey) {
      return yield* HttpServerResponse.json({ error: "API key required" }, { status: 401 })
    }

    // Test the API key by making a simple request to OpenRouter
    const response = yield* Effect.tryPromise(() =>
      fetch("https://openrouter.ai/api/v1/models", {
        headers: {
          "Authorization": `Bearer ${apiKey}`,
          "HTTP-Referer": "https://openagents.com",
          "X-Title": "OpenAgents"
        }
      })
    )

    if (response.ok) {
      return yield* HttpServerResponse.json({ valid: true, status: "API key is valid" })
    } else {
      return yield* HttpServerResponse.json({ valid: false, status: "Invalid API key" }, { status: 401 })
    }
  }).pipe(
    Effect.catchAll((error) => {
      console.error("OpenRouter status check error:", error)
      return HttpServerResponse.json({ error: "Failed to validate API key" }, { status: 500 })
    })
  )
}

/**
 * POST /api/openrouter/chat - Stream chat completion
 */
export function openrouterChat(
  ctx: RouteContext
) {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text
    const body = JSON.parse(bodyText)
    const { messages, model } = body

    // Get header from Effect HttpServerRequest
    const headers = ctx.request.headers
    const apiKeyFromHeader = Option.getOrNull(Headers.get(headers, "x-api-key"))

    // Use header API key first, fall back to environment variable
    const apiKey = apiKeyFromHeader || process.env.OPENROUTER_API_KEY

    if (!apiKey) {
      return yield* HttpServerResponse.json({ error: "API key required" }, { status: 401 })
    }

    // Create the layers for the OpenRouterClient
    const layers = Layer.mergeAll(
      BunHttpPlatform.layer,
      FetchHttpClient.layer,
      Layer.succeed(Ai.OpenRouter.OpenRouterConfig, {}),
      Ai.OpenRouter.layerOpenRouterClient({
        apiKey: Redacted.make(apiKey),
        referer: "https://openagents.com",
        title: "OpenAgents"
      })
    )

    // Create and run the streaming effect
    const readableStream = yield* Effect.gen(function*() {
      const client = yield* Ai.OpenRouter.OpenRouterClient
      const encoder = new TextEncoder()

      // Get the AI response stream
      const aiStream = client.stream({
        model,
        messages: messages.map((msg: any) => ({
          role: msg.role,
          content: msg.content
        }))
      })

      // Transform AI responses to SSE format
      const sseStream = aiStream.pipe(
        Stream.mapConcat((response: any) => {
          const chunks: Array<Uint8Array> = []
          
          for (const part of response.parts) {
            if (part._tag === "TextPart") {
              const chunk = {
                id: "chatcmpl-" + Math.random().toString(36).substring(2),
                object: "chat.completion.chunk",
                created: Math.floor(Date.now() / 1000),
                model,
                choices: [{
                  index: 0,
                  delta: {
                    content: part.text
                  },
                  finish_reason: null
                }]
              }
              chunks.push(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
            } else if (part._tag === "FinishPart") {
              const chunk = {
                id: "chatcmpl-" + Math.random().toString(36).substring(2),
                object: "chat.completion.chunk",
                created: Math.floor(Date.now() / 1000),
                model,
                choices: [{
                  index: 0,
                  delta: {},
                  finish_reason: part.reason
                }]
              }
              chunks.push(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
            }
          }
          
          return chunks
        }),
        Stream.concat(Stream.make(encoder.encode(`data: [DONE]\n\n`))),
        Stream.catchAll((error) => {
          console.error("OpenRouter streaming error:", error)
          return Stream.make(encoder.encode(`data: ${JSON.stringify({ error: error.message })}\n\n`))
        })
      )

      // Convert to ReadableStream
      return yield* Stream.toReadableStreamEffect(sseStream)
    }).pipe(Effect.provide(layers))

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
      console.error("OpenRouter API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 })
    })
  )
}