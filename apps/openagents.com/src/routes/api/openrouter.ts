import { BunHttpPlatform } from "@effect/platform-bun"
import * as Ai from "@openagentsinc/ai"
import { Effect, Layer, Redacted, Stream } from "effect"
import { Elysia } from "elysia"

export const openrouterApi = new Elysia({ prefix: "/api/openrouter" })
  .post("/chat", async ({ body, headers }: { body: any; headers: Record<string, string | undefined> }) => {
    try {
      const { messages, model } = body
      const apiKey = headers["x-api-key"]

      if (!apiKey) {
        return Response.json({ error: "API key required" }, { status: 401 })
      }

      // Create a TransformStream for streaming response
      const stream = new TransformStream()
      const writer = stream.writable.getWriter()
      const encoder = new TextEncoder() // Start streaming in background using Effect patterns
      ;(async () => {
        try {
          // Create and run the chat program
          const program = Effect.gen(function*() {
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
          })

          // Run the program with all layers provided
          await Effect.runPromise(
            // @ts-expect-error - Type issue with HttpClient requirement from layerOpenRouterClient
            program.pipe(
              Effect.provide(
                Layer.mergeAll(
                  BunHttpPlatform.layer,
                  Layer.succeed(Ai.OpenRouter.OpenRouterConfig, {}),
                  Ai.OpenRouter.layerOpenRouterClient({
                    apiKey: Redacted.make(apiKey),
                    referer: "https://openagents.com",
                    title: "OpenAgents"
                  })
                )
              )
            )
          )

          // Send completion signal
          await writer.write(encoder.encode(`data: [DONE]\n\n`))
        } catch (error: any) {
          console.error("OpenRouter streaming error:", error)
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
      console.error("OpenRouter API error:", error)
      return Response.json({ error: error.message }, { status: 500 })
    }
  })
