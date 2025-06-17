import { checkOllama, Inference } from "@openagentsinc/sdk"
import { Elysia } from "elysia"

export const ollamaApi = new Elysia({ prefix: "/api/ollama" })
  .get("/status", async () => {
    try {
      const status = await checkOllama()
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
      const encoder = new TextEncoder() // Start streaming in background
      ;(async () => {
        try {
          for await (
            const chunk of Inference.chat({
              model,
              messages,
              stream: true,
              options
            })
          ) {
            await writer.write(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
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
