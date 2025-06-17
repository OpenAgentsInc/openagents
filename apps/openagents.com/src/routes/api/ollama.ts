import { Elysia } from "elysia"
import { checkOllama, Inference } from "@openagentsinc/sdk"

export const ollamaApi = new Elysia({ prefix: "/api/ollama" })
  .get("/status", async () => {
    try {
      const status = await checkOllama()
      return Response.json(status)
    } catch (error) {
      return Response.json({ online: false, models: [], modelCount: 0 }, { status: 503 })
    }
  })
  .post("/chat", async ({ body }) => {
    try {
      const { model, messages, options } = body as any
      
      // Create a TransformStream for streaming response
      const stream = new TransformStream()
      const writer = stream.writable.getWriter()
      const encoder = new TextEncoder()
      
      // Start streaming in background
      (async () => {
        try {
          for await (const chunk of Inference.chat({
            model,
            messages,
            stream: true,
            options
          })) {
            await writer.write(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
          }
          await writer.write(encoder.encode(`data: [DONE]\n\n`))
        } catch (error) {
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
    } catch (error) {
      return Response.json({ error: error.message }, { status: 500 })
    }
  })