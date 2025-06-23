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

      // For now, use direct API call with streaming response
      const url = `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/run/${model}`

      const response = await fetch(url, {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${apiKey}`,
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          messages,
          stream: true
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
          const lines = text.split("\n").filter((line) => line.trim())

          for (const line of lines) {
            if (line.startsWith("data: ")) {
              const data = line.slice(6)
              if (data === "[DONE]") {
                controller.enqueue(new TextEncoder().encode("data: [DONE]\n\n"))
              } else {
                try {
                  const parsed = JSON.parse(data)
                  if (parsed.response) {
                    // Convert Cloudflare format to OpenAI format
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
                  console.error("Error parsing Cloudflare response:", e)
                }
              }
            }
          }
        }
      })

      // Pipe the response through the transform stream
      return new Response(response.body!.pipeThrough(transformStream), {
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
