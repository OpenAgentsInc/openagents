import * as HttpClientBun from "@effect/platform-bun/BunHttpClient"
import * as Ai from "@openagentsinc/ai"
import { Config, Effect, Layer, Stream } from "effect"
import { Elysia } from "elysia"

// Cloudflare configuration from environment
const CloudflareApiKey = Config.redacted("CLOUDFLARE_API_KEY")
const CloudflareAccountId = Config.string("CLOUDFLARE_ACCOUNT_ID")

export const cloudflareApi = new Elysia({ prefix: "/api/cloudflare" })
  .get("/status", async () => {
    try {
      // Check if Cloudflare is configured
      const configResult = await Effect.runPromise(
        Effect.gen(function*() {
          const apiKey = yield* CloudflareApiKey
          const accountId = yield* CloudflareAccountId
          return { apiKey, accountId }
        }).pipe(
          Effect.catchAll(() => Effect.succeed(null))
        )
      )

      return Response.json({
        available: configResult !== null,
        provider: "cloudflare"
      })
    } catch {
      return Response.json({ available: false })
    }
  })
  .post("/chat", async ({ body }: { body: any }) => {
    try {
      const { messages, model } = body

      // Create a TransformStream for streaming response
      const stream = new TransformStream()
      const writer = stream.writable.getWriter()
      const encoder = new TextEncoder() // Start streaming in background using Effect patterns
      ;(async () => {
        try {
          // Create and run the chat program
          const program = Effect.gen(function*() {
            // Load configuration from environment
            const apiKey = yield* CloudflareApiKey
            const accountId = yield* CloudflareAccountId

            // Create the client directly using make
            const client = yield* Ai.Cloudflare.makeCloudflareClient({
              apiKey,
              accountId,
              useOpenAIEndpoints: true // Use OpenAI-compatible endpoints for better streaming
            })

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

          // Provide the required layers
          const layers = Layer.merge(
            HttpClientBun.layer,
            Layer.succeed(Ai.Cloudflare.CloudflareConfig, {})
          )

          await Effect.runPromise(
            program.pipe(
              Effect.provide(layers)
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
