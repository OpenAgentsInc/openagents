import type { HttpServerRequest } from "@effect/platform"
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
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    // Get header from Effect HttpServerRequest
    const headers = ctx.request.headers
    const apiKeyFromHeader = Option.getOrNull(Headers.get(headers, "x-api-key"))

    // Use header API key first, fall back to environment variable
    const apiKey = apiKeyFromHeader || process.env.OPENROUTER_API_KEY

    if (!apiKey) {
      return yield* HttpServerResponse.json({ error: "API key required" }, { status: 401 }).pipe(Effect.orDie)
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
      return yield* HttpServerResponse.json({ valid: true, status: "API key is valid" }).pipe(Effect.orDie)
    } else {
      return yield* HttpServerResponse.json({ valid: false, status: "Invalid API key" }, { status: 401 }).pipe(
        Effect.orDie
      )
    }
  }).pipe(
    Effect.catchAll((error) => {
      console.error("OpenRouter status check error:", error)
      return HttpServerResponse.json({ error: "Failed to validate API key" }, { status: 500 }).pipe(Effect.orDie)
    })
  )
}

/**
 * POST /api/openrouter/chat - Stream chat completion
 */
export function openrouterChat(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text.pipe(Effect.orDie)
    const body = JSON.parse(bodyText)
    const { messages, model } = body

    // Get header from Effect HttpServerRequest
    const headers = ctx.request.headers
    const apiKeyFromHeader = Option.getOrNull(Headers.get(headers, "x-api-key"))

    // Use header API key first, fall back to environment variable
    const apiKey = apiKeyFromHeader || process.env.OPENROUTER_API_KEY

    if (!apiKey) {
      return yield* HttpServerResponse.json({ error: "API key required" }, { status: 401 }).pipe(Effect.orDie)
    }

    // Create a TransformStream for streaming response
    const stream = new TransformStream()
    const writer = stream.writable.getWriter()
    const encoder = new TextEncoder()

    // Run the streaming in background
    Effect.runPromise(
      // @ts-expect-error - Type issue with HttpClient requirement
      Effect.gen(function*() {
        // Get the client from context
        const client = yield* Ai.OpenRouter.OpenRouterClient

        // Get the stream
        const responseStream = client.stream({
          model,
          messages: messages.map((msg: any) => ({
            role: msg.role,
            content: msg.content
          }))
        })

        // Process the stream
        yield* responseStream.pipe(
          Stream.tap((response: any) =>
            Effect.sync(() => {
              // Convert AiResponse to OpenAI-compatible format
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
                  writer.write(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`)).catch(() => {})
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
                  writer.write(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`)).catch(() => {})
                }
              }
            })
          ),
          Stream.runDrain
        )

        // Send completion signal
        writer.write(encoder.encode(`data: [DONE]\n\n`))
        writer.close()
      }).pipe(
        Effect.provide(
          Layer.mergeAll(
            BunHttpPlatform.layer,
            FetchHttpClient.layer,
            Layer.succeed(Ai.OpenRouter.OpenRouterConfig, {}),
            Ai.OpenRouter.layerOpenRouterClient({
              apiKey: Redacted.make(apiKey),
              referer: "https://openagents.com",
              title: "OpenAgents"
            })
          )
        ),
        Effect.catchAll((error) =>
          Effect.sync(() => {
            console.error("OpenRouter streaming error:", error)
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
      console.error("OpenRouter API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 }).pipe(Effect.orDie)
    })
  )
}
