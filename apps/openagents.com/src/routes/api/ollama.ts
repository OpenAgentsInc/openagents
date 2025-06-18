import * as Ai from "@openagentsinc/ai"
import { Effect } from "effect"
import { Elysia } from "elysia"

export const ollamaApi = new Elysia({ prefix: "/api/ollama" })
  .get("/status", async () => {
    try {
      // Use the Ollama provider's checkStatus function
      const status = await Effect.runPromise(Ai.Ollama.checkStatus())
      return Response.json(status)
    } catch {
      return Response.json({ online: false, models: [], modelCount: 0 }, { status: 503 })
    }
  })
  .post("/chat", async ({ body }: { body: any }) => {
    try {
      const { messages, model, options } = body

      // Create a TransformStream for streaming response
      const stream = new TransformStream()
      const writer = stream.writable.getWriter()
      const encoder = new TextEncoder() // Start streaming in background using new AI library
      ;(async () => {
        try {
          // Create Ollama client and run chat
          const chatEffect = Effect.gen(function*() {
            const client = yield* Ai.Ollama.OllamaClient
            const generator = yield* client.chat({
              model,
              messages,
              stream: true,
              options: {
                temperature: options?.temperature || 0.7,
                num_ctx: options?.num_ctx || 4096,
                ...options
              }
            })
            return generator
          })

          // Run the effect with the Ollama layer
          const generator = await Effect.runPromise(
            chatEffect.pipe(
              Effect.provide(Ai.Ollama.OllamaClientLive())
            )
          )

          // Stream the results
          for await (const chunk of generator) {
            // Convert to expected format for frontend compatibility
            const formattedChunk = {
              model,
              created_at: new Date().toISOString(),
              message: {
                role: "assistant" as const,
                content: chunk.content
              },
              done: chunk.done || false
            }
            await writer.write(encoder.encode(`data: ${JSON.stringify(formattedChunk)}\n\n`))
          }
          await writer.write(encoder.encode(`data: [DONE]\n\n`))
        } catch (error: any) {
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
      return Response.json({ error: error.message }, { status: 500 })
    }
  })
