import { HttpServerRequest, HttpServerResponse, FetchHttpClient } from "@effect/platform"
import { BunHttpPlatform } from "@effect/platform-bun"
import * as Ai from "@openagentsinc/ai"
import { Effect, Layer, Redacted, Stream } from "effect"
import type { RouteContext } from "@openagentsinc/psionic"

/**
 * GET /api/cloudflare/status - Check Cloudflare availability
 */
export function cloudflareStatus(_ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, never> {
  // Check environment variables
  const apiKey = process.env.CLOUDFLARE_API_KEY
  const accountId = process.env.CLOUDFLARE_ACCOUNT_ID

  // If we have both, return available
  if (apiKey && accountId) {
    return HttpServerResponse.json({
      available: true,
      provider: "cloudflare"
    }).pipe(Effect.orDie)
  }

  return HttpServerResponse.json({
    available: false,
    provider: "cloudflare"
  }).pipe(Effect.orDie)
}

/**
 * POST /api/cloudflare/chat - Stream chat completion
 */
export function cloudflareChat(ctx: RouteContext): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function* () {
      const bodyText = yield* ctx.request.text.pipe(Effect.orDie)

      const body = JSON.parse(bodyText)
      const { messages, model } = body

      const apiKey = process.env.CLOUDFLARE_API_KEY
      const accountId = process.env.CLOUDFLARE_ACCOUNT_ID

      if (!apiKey || !accountId) {
        return yield* HttpServerResponse.json({ error: "Cloudflare not configured" }, { status: 500 }).pipe(Effect.orDie)
      }

      // Create a TransformStream for SSE format
      const { readable, writable } = new TransformStream()
      const writer = writable.getWriter()
      const encoder = new TextEncoder()

      // Run the streaming in background
      Effect.runPromise(
        // @ts-expect-error - Type issue with HttpClient requirement
        Effect.gen(function*() {
          // Create the CloudflareClient service
          const client = yield* Ai.Cloudflare.CloudflareClient

          // Stream the response directly using the client
          const stream = client.stream({
            model,
            messages,
            temperature: 0.7,
            max_tokens: 4096,
            stream: true
          })

          // Process the stream
          yield* stream.pipe(
            Stream.tap((response) => Effect.sync(() => {
              // Convert AiResponse to OpenAI-compatible SSE format
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
                  const data = `data: ${JSON.stringify(chunk)}\n\n`
                  writer.write(encoder.encode(data))
                } else if (part._tag === "FinishPart") {
                  // Send finish chunk
                  const chunk = {
                    id: "chatcmpl-" + Math.random().toString(36).substring(2),
                    object: "chat.completion.chunk",
                    created: Math.floor(Date.now() / 1000),
                    model,
                    choices: [{
                      index: 0,
                      delta: {},
                      finish_reason: part.reason
                    }],
                    usage: {
                      prompt_tokens: part.usage.inputTokens,
                      completion_tokens: part.usage.outputTokens,
                      total_tokens: part.usage.totalTokens
                    }
                  }
                  const data = `data: ${JSON.stringify(chunk)}\n\n`
                  writer.write(encoder.encode(data))
                }
              }
            })),
            Stream.runDrain
          )

          // Send done signal
          writer.write(encoder.encode("data: [DONE]\n\n"))
          writer.close()
        }).pipe(
          Effect.provide(
            Layer.mergeAll(
              BunHttpPlatform.layer,
              FetchHttpClient.layer,
              Ai.Cloudflare.layerCloudflareClient({
                apiKey: Redacted.make(apiKey),
                accountId,
                useOpenAIEndpoints: true
              })
            )
          ),
          Effect.catchAll((error) => Effect.sync(() => {
            console.error("Streaming error:", error)
            writer.write(encoder.encode(`data: {"error": "${error}"}\n\n`))
            writer.close()
          }))
        )
      )

      // Return the readable stream as SSE response using raw
      return yield* Effect.succeed(
        HttpServerResponse.raw(new Response(readable, {
          headers: {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Methods": "POST, GET, OPTIONS",
            "Access-Control-Allow-Headers": "Content-Type"
          }
        }))
      )
  }).pipe(
    Effect.catchAll((error: any) => {
      console.error("Cloudflare API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 }).pipe(Effect.orDie)
    })
  )
}
