import * as Ai from "@openagentsinc/ai"
import { Effect, Stream } from "effect"
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
      const encoder = new TextEncoder()

      // Start streaming in background using Effect patterns
      ;(async () => {
        try {
          // Create the chat effect using the Ollama client
          const chatProgram = Effect.gen(function*() {
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
          })
          
          // Run the program with the Ollama layer
          await Effect.runPromise(
            chatProgram.pipe(
              Effect.provide(Ai.Ollama.OllamaClientLive()),
              Effect.tapError((error) => 
                Effect.sync(() => {
                  console.error("Effect chat error:", error)
                  return error
                })
              )
            )
          )

          // Send completion signal
          await writer.write(encoder.encode(`data: [DONE]\n\n`))
        } catch (error: any) {
          console.error("Chat streaming error:", error)
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
      console.error("Chat API error:", error)
      return Response.json({ error: error.message }, { status: 500 })
    }
  })
