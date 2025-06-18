/**
 * Ollama provider for @openagentsinc/ai
 *
 * Simplified Ollama integration using Effect patterns
 * @since 1.0.0
 */
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"

/**
 * Ollama configuration
 */
export interface OllamaConfig {
  readonly baseUrl?: string
  readonly maxRetries?: number
}

/**
 * Chat message interface
 */
export interface ChatMessage {
  readonly role: "system" | "user" | "assistant"
  readonly content: string
}

/**
 * Chat request interface
 */
export interface ChatRequest {
  readonly model: string
  readonly messages: ReadonlyArray<ChatMessage>
  readonly stream?: boolean
  readonly options?: {
    readonly temperature?: number
    readonly num_ctx?: number
    readonly top_p?: number
    readonly seed?: number
  }
}

/**
 * Chat response chunk
 */
export interface ChatChunk {
  readonly content: string
  readonly done?: boolean
}

/**
 * Ollama client service
 */
export class OllamaClient extends Context.Tag("@openagentsinc/ai/OllamaClient")<
  OllamaClient,
  {
    readonly baseUrl: string
    readonly maxRetries: number
    readonly chat: (request: ChatRequest) => AsyncGenerator<ChatChunk>
  }
>() {}

/**
 * Create Ollama client implementation
 */
export const OllamaClientLive = (config: OllamaConfig = {}): Layer.Layer<OllamaClient> =>
  Layer.succeed(OllamaClient, {
    baseUrl: config.baseUrl ?? "http://localhost:11434",
    maxRetries: config.maxRetries ?? 3,

    chat: (request: ChatRequest) => {
      return chatGenerator(request, config.baseUrl ?? "http://localhost:11434")
    }
  })

/**
 * Internal chat generator function
 */
async function* chatGenerator(request: ChatRequest, baseUrl: string): AsyncGenerator<ChatChunk> {
  const response = await fetch(`${baseUrl}/api/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model: request.model,
      messages: request.messages,
      stream: request.stream ?? true,
      options: request.options ?? {}
    })
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Ollama API error: ${response.status} - ${errorText}`)
  }

  if (!request.stream) {
    const data = await response.json()
    yield { content: data.message?.content ?? "", done: true }
    return
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
            if (chunk.message?.content) {
              yield { content: chunk.message.content, done: chunk.done }
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

// Simplified version - model and modelLayer commented out for now as they need refactoring
// to work with the direct async generator approach

/**
 * Helper to check Ollama availability
 */
export const checkStatus = (baseUrl?: string) =>
  Effect.tryPromise({
    try: async () => {
      const url = baseUrl ?? "http://localhost:11434"
      const response = await fetch(`${url}/api/tags`)
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`)
      }
      const data = await response.json()
      return {
        online: true,
        models: data.models || [],
        modelCount: (data.models || []).length
      }
    },
    catch: () => ({
      online: false,
      models: [],
      modelCount: 0,
      error: "Cannot connect to Ollama"
    })
  })
