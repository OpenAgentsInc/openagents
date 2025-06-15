import { Effect, Console } from "effect"

// Hello World example
export const helloWorld = Console.log("Hello from OpenAgents SDK!")

export const runHelloWorld = (): void => Effect.runSync(helloWorld)

// Ollama connection checker
const OLLAMA_DEFAULT_PORT = 11434
const OLLAMA_BASE_URL = `http://localhost:${OLLAMA_DEFAULT_PORT}`

// Types
interface OllamaModel {
  name: string
  model: string
  modified_at: string
  size: number
  digest: string
  details?: {
    parent_model?: string
    format?: string
    family?: string
    families?: string[]
    parameter_size?: string
    quantization_level?: string
  }
}

interface OllamaStatus {
  online: boolean
  models: OllamaModel[]
  modelCount: number
  error?: string
}

// Custom error class for Ollama connection failures
class OllamaConnectionError extends Error {
  constructor(public cause: unknown) {
    super("Failed to connect to Ollama")
    this.name = "OllamaConnectionError"
  }
}

// Check if Ollama is running by hitting the API tags endpoint
export const checkOllamaStatus = Effect.tryPromise({
  try: async (): Promise<OllamaStatus> => {
    const response = await fetch(`${OLLAMA_BASE_URL}/api/tags`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json'
      },
      // Short timeout for quick status checks
      signal: AbortSignal.timeout(3000)
    })
    
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }
    
    const data = await response.json() as { models?: OllamaModel[] }
    return {
      online: true,
      models: data.models || [],
      modelCount: (data.models || []).length
    }
  },
  catch: (error) => new OllamaConnectionError(error)
})

// Fallback to simpler root endpoint check if tags endpoint fails
const checkOllamaRootEndpoint = Effect.tryPromise({
  try: async (): Promise<OllamaStatus> => {
    const response = await fetch(`${OLLAMA_BASE_URL}/`, {
      method: 'GET',
      signal: AbortSignal.timeout(3000)
    })
    
    return {
      online: response.ok || response.status < 500,
      models: [],
      modelCount: 0
    }
  },
  catch: (error) => new OllamaConnectionError(error)
})

// Main status checker with fallback
export const getOllamaStatus: Effect.Effect<OllamaStatus, never, never> = Effect.orElse(
  checkOllamaStatus,
  () => checkOllamaRootEndpoint
).pipe(
  Effect.catchAll(() => 
    Effect.succeed({
      online: false,
      models: [],
      modelCount: 0,
      error: "Cannot connect to Ollama"
    })
  )
)

// Run the status check and return a promise
export const checkOllama = (): Promise<OllamaStatus> => Effect.runPromise(getOllamaStatus)