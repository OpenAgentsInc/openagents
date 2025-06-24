import { FetchHttpClient, HttpServerResponse } from "@effect/platform"
import { BunHttpPlatform } from "@effect/platform-bun"
import * as Ai from "@openagentsinc/ai"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect, Layer, Redacted, Stream } from "effect"

/**
 * GET /api/cloudflare/status - Check Cloudflare availability
 */
export function cloudflareStatus(
  _ctx: RouteContext
) {
  // Check environment variables
  const apiKey = process.env.CLOUDFLARE_API_KEY
  const accountId = process.env.CLOUDFLARE_ACCOUNT_ID

  // If we have both, return available
  if (apiKey && accountId) {
    return HttpServerResponse.json({
      available: true,
      provider: "cloudflare"
    })
  }

  return HttpServerResponse.json({
    available: false,
    provider: "cloudflare"
  })
}

/**
 * POST /api/cloudflare/chat - Stream chat completion
 */
export function cloudflareChat(
  ctx: RouteContext
) {
  return Effect.gen(function*() {
    const bodyText = yield* ctx.request.text
    const body = JSON.parse(bodyText)
    
    // Handle both single message and messages array formats
    let messages = body.messages
    if (!messages && body.message) {
      // Convert single message to messages array
      messages = [
        { role: "user", content: body.message }
      ]
    }
    
    const model = body.model || "@cf/meta/llama-3.1-8b-instruct"

    const apiKey = process.env.CLOUDFLARE_API_KEY
    const accountId = process.env.CLOUDFLARE_ACCOUNT_ID

    if (!apiKey || !accountId) {
      return yield* HttpServerResponse.json({ error: "Cloudflare not configured" }, { status: 500 })
    }

    // Create the layers for the CloudflareClient
    const layers = Layer.mergeAll(
      BunHttpPlatform.layer,
      FetchHttpClient.layer,
      Ai.Cloudflare.layerCloudflareClient({
        apiKey: Redacted.make(apiKey),
        accountId,
        useOpenAIEndpoints: true
      })
    )

    // Create and run the streaming effect
    const readableStream = yield* Effect.gen(function*() {
      const client = yield* Ai.Cloudflare.CloudflareClient
      const encoder = new TextEncoder()

      // Get the AI response stream
      const aiStream = client.stream({
        model,
        messages,
        temperature: 0.7,
        max_tokens: 4096,
        stream: true
      })

      // Transform AI responses to SSE format
      const sseStream = aiStream.pipe(
        Stream.mapConcat((response) => {
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
              const data = `data: ${JSON.stringify(chunk)}\n\n`
              chunks.push(encoder.encode(data))
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
              chunks.push(encoder.encode(data))
            }
          }

          return chunks
        }),
        Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n"))),
        Stream.catchAll((error) => {
          console.error("Streaming error:", error)
          return Stream.make(encoder.encode(`data: {"error": "${error}"}\n\n`))
        })
      )

      // Convert to ReadableStream with ALL layers provided
      return yield* Stream.toReadableStreamEffect(sseStream).pipe(
        Effect.provide(layers)
      )
    })

    // Return the response with SSE headers
    return HttpServerResponse.raw(readableStream, {
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        "Connection": "keep-alive",
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "POST, GET, OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type"
      }
    })
  }).pipe(
    Effect.catchAll((error: any) => {
      console.error("Cloudflare API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 })
    })
  )
}
