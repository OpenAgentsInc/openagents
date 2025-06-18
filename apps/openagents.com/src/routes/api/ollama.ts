import { Elysia } from "elysia"

// Simple Ollama status check
async function checkOllamaStatus() {
  try {
    const response = await fetch("http://localhost:11434/api/tags")
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }
    const data = await response.json()
    return {
      online: true,
      models: data.models || [],
      modelCount: (data.models || []).length
    }
  } catch (error) {
    return {
      online: false,
      models: [],
      modelCount: 0,
      error: "Cannot connect to Ollama"
    }
  }
}

// Simple chat streaming function
async function* chatStream(request: {
  model: string
  messages: Array<{ role: string; content: string }>
  options?: any
}) {
  const response = await fetch("http://localhost:11434/api/chat", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model: request.model,
      messages: request.messages,
      stream: true,
      options: request.options || {}
    })
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Ollama API error: ${response.status} - ${errorText}`)
  }

  const reader = response.body?.getReader()
  if (!reader) throw new Error("No response body")

  const decoder = new TextDecoder()
  let buffer = ""

  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""

      for (const line of lines) {
        if (line.trim()) {
          try {
            const chunk = JSON.parse(line)
            // Yield in the format expected by the frontend
            yield {
              model: request.model,
              created_at: new Date().toISOString(),
              message: {
                role: "assistant" as const,
                content: chunk.message?.content || ""
              },
              done: chunk.done || false
            }
            if (chunk.done) return
          } catch {
            // Skip invalid JSON
          }
        }
      }
    }
  } finally {
    reader.releaseLock()
  }
}

export const ollamaApi = new Elysia({ prefix: "/api/ollama" })
  .get("/status", async () => {
    try {
      const status = await checkOllamaStatus()
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

      // Start streaming in background
      ;(async () => {
        try {
          for await (const chunk of chatStream({ model, messages, options })) {
            await writer.write(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
          }
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
