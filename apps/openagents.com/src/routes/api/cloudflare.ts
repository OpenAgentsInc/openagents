import { Effect } from "effect"

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
      // Parse the request body from Effect HttpServerRequest
      const bodyText = await Effect.runPromise(
        Effect.gen(function*() {
          const request = context.request
          return yield* request.text
        }) as Effect.Effect<string, never, never>
      )

      const body = JSON.parse(bodyText)
      const { messages, model } = body

      // Get credentials
      const apiKey = process.env.CLOUDFLARE_API_KEY
      const accountId = process.env.CLOUDFLARE_ACCOUNT_ID

      if (!apiKey || !accountId) {
        return Response.json({ error: "Cloudflare not configured" }, { status: 500 })
      }

      // Use OpenAI-compatible endpoint for proper streaming
      const url = `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1/chat/completions`

      const response = await fetch(url, {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${apiKey}`,
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          model,
          messages,
          stream: true,
          max_tokens: 4096
        })
      })

      if (!response.ok) {
        const error = await response.text()
        console.error("Cloudflare API error:", error)
        return Response.json({ error: "Cloudflare API error" }, { status: response.status })
      }

      // Create a transform stream to convert Cloudflare's format to OpenAI's format
      const transformStream = new TransformStream({
        async transform(chunk, controller) {
          const text = new TextDecoder().decode(chunk)
          console.log("Raw chunk received:", JSON.stringify(text))

          // Handle Cloudflare's streaming format which can be:
          // 1. SSE format: "data: {...}\n\n"
          // 2. Raw JSON lines: "{...}\n"
          const lines = text.split("\n").filter((line) => line.trim())

          for (const line of lines) {
            let jsonData = ""

            if (line.startsWith("data: ")) {
              jsonData = line.slice(6)
              if (jsonData === "[DONE]") {
                controller.enqueue(new TextEncoder().encode("data: [DONE]\n\n"))
                continue
              }
            } else if (line.trim().startsWith("{")) {
              jsonData = line.trim()
            } else {
              continue
            }

            try {
              const parsed = JSON.parse(jsonData)
              console.log("Parsed chunk:", parsed)

              // Handle OpenAI-compatible format from Cloudflare
              if (parsed.choices?.[0]?.delta?.content) {
                // Already in OpenAI format, pass through directly
                controller.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(parsed)}\n\n`))
              } else if (parsed.response) {
                // Old Cloudflare format, convert to OpenAI format
                const openAIChunk = {
                  id: "chatcmpl-" + Math.random().toString(36).substring(2),
                  object: "chat.completion.chunk",
                  created: Math.floor(Date.now() / 1000),
                  model,
                  choices: [{
                    index: 0,
                    delta: {
                      content: parsed.response
                    },
                    finish_reason: null
                  }]
                }
                controller.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(openAIChunk)}\n\n`))
              }
            } catch (e) {
              console.error("Error parsing Cloudflare response:", e, "Raw data:", jsonData)
            }
          }
        },
        flush(controller) {
          // Send final done signal
          controller.enqueue(new TextEncoder().encode("data: [DONE]\n\n"))
        }
      })

      // Pipe the response through the transform stream
      return new Response(response.body!.pipeThrough(transformStream), {
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          "Connection": "keep-alive",
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "POST, GET, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type"
        }
      })
    } catch (error: any) {
      console.error("Cloudflare API error:", error)
      return Response.json({ error: error.message }, { status: 500 })
    }
  })
}
