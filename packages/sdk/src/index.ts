import { Effect, Console } from "effect"

// Hello World example
export const helloWorld = Console.log("Hello from OpenAgents SDK!")

export const runHelloWorld = (): void => Effect.runSync(helloWorld)

// Ollama connection checker
const OLLAMA_DEFAULT_PORT = 11434
// Use a dynamic URL based on the current location
const getOllamaBaseUrl = (): string => {
  // If we're on localhost, use localhost
  if (typeof window !== 'undefined' && window.location.hostname === 'localhost') {
    return `http://localhost:${OLLAMA_DEFAULT_PORT}`
  }
  // For production, we can't directly access localhost from HTTPS
  // Return a special value to indicate CORS restriction
  return 'CORS_RESTRICTED'
}

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
    const baseUrl = getOllamaBaseUrl()
    
    // If CORS restricted, return a special status
    if (baseUrl === 'CORS_RESTRICTED') {
      return {
        online: false,
        models: [],
        modelCount: 0,
        error: 'Cannot check Ollama status from HTTPS (CORS restriction). Please use localhost for development.'
      }
    }
    
    const response = await fetch(`${baseUrl}/api/tags`, {
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
    const response = await fetch(`${getOllamaBaseUrl()}/`, {
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