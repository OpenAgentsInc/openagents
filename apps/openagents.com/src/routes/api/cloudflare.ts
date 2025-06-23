import { BunHttpPlatform } from "@effect/platform-bun"
import * as Ai from "@openagentsinc/ai"
import { Config, Effect, Layer, Stream } from "effect"

export const cloudflareApi = (app: any) => {
  const prefix = "/api/cloudflare"

  app.get(`${prefix}/status`, async () => {
    try {
      // Check environment variables
      const apiKey = process.env.CLOUDFLARE_API_KEY
      const accountId = process.env.CLOUDFLARE_ACCOUNT_ID
      
      // If we have both, return available
      if (apiKey && accountId) {
        return Response.json({
          available: true,
          provider: "cloudflare"
        })
      }
      
      return Response.json({
        available: false,
        provider: "cloudflare"
      })
    } catch (error) {
      console.error("Cloudflare status error:", error)
      return Response.json({ available: false })
    }
  })

  app.post(`${prefix}/chat`, async (context: any) => {
    try {
      // Parse the request body
      const body = await context.request.json()
      const { messages, model } = body

      // Create a TransformStream for streaming response
      const stream = new TransformStream()
      const writer = stream.writable.getWriter()
      const encoder = new TextEncoder() // Start streaming in background using Effect patterns
      ;(async () => {
        try {
          // Create and run the chat program
          const program = Effect.gen(function*() {
            // Get the client from context
            const client = yield* Ai.Cloudflare.CloudflareClient

            // Get the stream
            const responseStream = client.stream({
              model,
              messages: messages.map((msg: any) => ({
                role: msg.role,
                content: msg.content
              })),
              stream: true
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
          })

          // Get config directly from environment
          const config = {
            apiKey: process.env.CLOUDFLARE_API_KEY,
            accountId: process.env.CLOUDFLARE_ACCOUNT_ID
          }
          
          if (!config.apiKey || !config.accountId) {
            throw new Error("Cloudflare credentials not configured")
          }

          // Run the main program with all layers provided
          await Effect.runPromise(
            // @ts-expect-error - Type issue with HttpClient requirement from layerCloudflareClient
            program.pipe(
              Effect.provide(
                Layer.mergeAll(
                  BunHttpPlatform.layer,
                  Layer.succeed(Ai.Cloudflare.CloudflareConfig, {}),
                  Ai.Cloudflare.layerCloudflareClient({
                    apiKey: config.apiKey,
                    accountId: config.accountId,
                    useOpenAIEndpoints: true
                  })
                )
              )
            )
          )

          // Send completion signal
          await writer.write(encoder.encode(`data: [DONE]\n\n`))
        } catch (error: any) {
          console.error("Cloudflare streaming error:", error)
          await writer.write(encoder.encode(`data: ${JSON.stringify({ error: error.message })}\n\n`))
        } finally {
          await writer.close()
        }
      })()

      return new Response(stream.readable, {
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          "Connection": "keep-alive"
        }
      })
    } catch (error: any) {
      console.error("Cloudflare API error:", error)
      return Response.json({ error: error.message }, { status: 500 })
    }
  })
}
