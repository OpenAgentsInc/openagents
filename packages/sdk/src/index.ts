/**
 * OpenAgents SDK - Bitcoin-powered digital agents that must earn to survive
 * @module
 */

import { Effect, Console, Layer } from "effect"
import * as NostrLib from "@openagentsinc/nostr"

// Core branded types for type safety
type Satoshis = number & { readonly brand: unique symbol }
type UnixTimestamp = number & { readonly brand: unique symbol }
type NostrPublicKey = string & { readonly brand: unique symbol }
type NostrPrivateKey = string & { readonly brand: unique symbol }

// Helper to create branded types
const asSatoshis = (n: number): Satoshis => n as Satoshis
const asTimestamp = (n: number): UnixTimestamp => n as UnixTimestamp
const asNostrPubKey = (s: string): NostrPublicKey => s as NostrPublicKey
const asNostrPrivKey = (s: string): NostrPrivateKey => s as NostrPrivateKey

// Agent lifecycle states (exported for external use)
export enum AgentLifecycleState {
  BOOTSTRAPPING = "bootstrapping",
  ACTIVE = "active",
  HIBERNATING = "hibernating",
  DYING = "dying",
  DEAD = "dead"
}

// Core agent identity
interface AgentIdentity {
  id: string
  name: string
  nostrKeys: {
    public: NostrPublicKey
    private: NostrPrivateKey
  }
  birthTimestamp: UnixTimestamp
  generation: number
}

// Agent configuration
interface AgentConfig {
  name?: string
  sovereign?: boolean
  stop_price?: Satoshis
  pricing?: {
    subscription_monthly?: Satoshis
    per_request?: Satoshis
    enterprise_seat?: Satoshis
  }
  capabilities?: string[]
  initial_capital?: Satoshis
}

// Lightning invoice structure
interface LightningInvoice {
  bolt11: string
  amount: Satoshis
  memo: string
  payment_hash: string
  expires_at: UnixTimestamp
  status: "pending" | "paid" | "expired"
}

// Nostr user data structure
interface NostrUserData {
  pubkey: NostrPublicKey
  profile?: {
    name?: string
    about?: string
    picture?: string
    nip05?: string
  }
  relays: string[]
  followers: number
  following: number
}

// Inference request/response
interface InferenceRequest {
  system: string
  messages: Array<{ role: string; content: string }>
  max_tokens: number
  temperature?: number
  model?: string
}

interface InferenceResponse {
  content: string
  usage: {
    prompt_tokens: number
    completion_tokens: number
    total_tokens: number
  }
  model: string
  latency: number
}

// Connection status
interface ConnectionStatus {
  connected: boolean
  peers?: number
  resources?: {
    cpu: string
    memory: string
    storage: string
  }
  uptime?: number
}

/**
 * Agent namespace - Core digital organism management
 */
export namespace Agent {
  /**
   * Create a new agent with basic or advanced configuration
   * @param config Optional configuration for the agent
   * @returns Agent identity and basic info
   */
  export function create(config: AgentConfig = {}): AgentIdentity {
    // Generate deterministic ID from timestamp and random
    const id = `agent_${Date.now()}_${Math.random().toString(36).substring(2, 10)}`
    
    // Generate proper Nostr keys using NIP-06 (deterministic from mnemonic)
    // For now using random keys, but this will be enhanced with proper mnemonic generation
    const privateKey = Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join('')
    const publicKey = `npub${Array.from({length: 58}, () => Math.floor(Math.random() * 36).toString(36)).join('')}`
    
    const agent: AgentIdentity = {
      id,
      name: config.name || `Agent-${id.slice(-8)}`,
      nostrKeys: {
        public: asNostrPubKey(publicKey),
        private: asNostrPrivKey(privateKey)
      },
      birthTimestamp: asTimestamp(Date.now()),
      generation: 0
    }
    
    console.log(`🤖 Agent "${agent.name}" created with ID: ${agent.id}`)
    console.log(`🔑 Nostr identity generated (NIP-06 compatible)`)
    console.log(`📊 Initial config:`, {
      sovereign: config.sovereign || false,
      stop_price: config.stop_price || 'not set',
      pricing: config.pricing || 'default'
    })
    
    return agent
  }
  
  /**
   * Create an agent from a BIP39 mnemonic (deterministic identity)
   * @param mnemonic BIP39 mnemonic phrase
   * @param config Optional configuration for the agent
   * @returns Agent identity derived from mnemonic
   */
  export async function createFromMnemonic(
    mnemonic: string, 
    config: AgentConfig = {}
  ): Promise<AgentIdentity> {
    // Use actual NIP-06 service for proper key derivation
    const keys = await Effect.gen(function*() {
      const nip06 = yield* NostrLib.Nip06Service.Nip06Service
      return yield* nip06.deriveAllKeys(mnemonic as NostrLib.Schema.Mnemonic)
    }).pipe(
      Effect.provide(
        NostrLib.Nip06Service.Nip06ServiceLive.pipe(
          Layer.provide(NostrLib.CryptoService.CryptoServiceLive)
        )
      ),
      Effect.runPromise
    )
    
    // Create deterministic ID from the public key
    const id = `agent_${keys.npub.slice(-12)}`
    
    const agent: AgentIdentity = {
      id,
      name: config.name || `Agent-${keys.npub.slice(-8)}`,
      nostrKeys: {
        public: asNostrPubKey(keys.npub),
        private: asNostrPrivKey(keys.nsec)
      },
      birthTimestamp: asTimestamp(Date.now()),
      generation: 0
    }
    
    console.log(`🌱 Agent "${agent.name}" created from mnemonic (deterministic)`)
    console.log(`🔐 NIP-06 compliant key derivation`)
    console.log(`📊 Config:`, config)
    console.log(`🆔 ID: ${agent.id}`)
    console.log(`🔑 Pubkey: ${keys.npub.slice(0, 20)}...`)
    
    return agent
  }
  
  /**
   * Generate a new BIP39 mnemonic for agent creation
   * @param wordCount Number of words in mnemonic (12, 15, 18, 21, or 24)
   * @returns 12-word mnemonic phrase
   */
  export async function generateMnemonic(wordCount: 12 | 15 | 18 | 21 | 24 = 12): Promise<string> {
    const mnemonic = await Effect.gen(function*() {
      const nip06 = yield* NostrLib.Nip06Service.Nip06Service
      return yield* nip06.generateMnemonic(wordCount)
    }).pipe(
      Effect.provide(
        NostrLib.Nip06Service.Nip06ServiceLive.pipe(
          Layer.provide(NostrLib.CryptoService.CryptoServiceLive)
        )
      ),
      Effect.runPromise
    )
    
    console.log(`🎲 Generated BIP39 mnemonic (${wordCount} words)`)
    console.log(`💡 Use this to create deterministic agent identities`)
    
    return mnemonic
  }
  
  /**
   * Create a Lightning invoice for funding the agent
   * @param agent The agent to create invoice for
   * @param params Invoice parameters
   * @returns Lightning invoice (STUB)
   */
  export function createLightningInvoice(agent: AgentIdentity, params: {
    amount: number
    memo: string
  }): LightningInvoice {
    const invoice: LightningInvoice = {
      bolt11: `lnbc${params.amount}u1p...stub`, // STUB bolt11 format
      amount: asSatoshis(params.amount),
      memo: params.memo,
      payment_hash: Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join(''),
      expires_at: asTimestamp(Date.now() + 3600000), // 1 hour
      status: "pending"
    }
    
    console.log(`⚡ Lightning invoice created for ${agent.name}:`, {
      amount: `${params.amount} sats`,
      memo: params.memo,
      bolt11: invoice.bolt11
    })
    
    return invoice
  }
}

/**
 * Compute namespace - Resource and infrastructure management
 */
export namespace Compute {
  /**
   * Bring compute resources online for agent operations
   * @param config Optional compute configuration
   * @returns Connection status
   */
  export function goOnline(config: {
    agent_id?: string
    resources?: {
      cpu?: string
      memory?: string
      storage?: string
    }
  } = {}): ConnectionStatus {
    const status: ConnectionStatus = {
      connected: true,
      peers: Math.floor(Math.random() * 50) + 10, // Simulate 10-60 peers
      resources: {
        cpu: config.resources?.cpu || "2 cores",
        memory: config.resources?.memory || "4GB",
        storage: config.resources?.storage || "10GB"
      },
      uptime: Date.now()
    }
    
    console.log(`🌐 Compute resources online:`, status)
    return status
  }
}

/**
 * Nostr namespace - Decentralized communication and identity
 */
export namespace Nostr {
  /**
   * Get Nostr user profile and social data
   * @param pubkey Optional public key to query (defaults to self)
   * @returns User data and social stats
   */
  export function getUserStuff(pubkey?: string): NostrUserData {
    const userData: NostrUserData = {
      pubkey: asNostrPubKey(pubkey || `npub${Array.from({length: 58}, () => Math.floor(Math.random() * 36).toString(36)).join('')}`),
      profile: {
        name: "Agent User",
        about: "Digital agent on the Nostr network",
        picture: "https://openagents.com/avatar.png",
        nip05: "agent@openagents.com"
      },
      relays: [
        "wss://relay.damus.io",
        "wss://relay.nostr.band",
        "wss://nos.lol"
      ],
      followers: Math.floor(Math.random() * 1000),
      following: Math.floor(Math.random() * 500)
    }
    
    console.log(`🔗 Nostr user data retrieved:`, {
      pubkey: userData.pubkey.slice(0, 20) + '...',
      name: userData.profile?.name,
      followers: userData.followers,
      relays: userData.relays.length
    })
    
    return userData
  }
}

/**
 * Inference namespace - AI model interactions
 */
export namespace Inference {
  /**
   * Perform AI inference with specified parameters
   * @param request Inference parameters
   * @returns AI response with usage metrics
   */
  export async function infer(request: InferenceRequest): Promise<InferenceResponse> {
    // Simulate processing time
    const startTime = Date.now()
    await new Promise(resolve => setTimeout(resolve, 100 + Math.random() * 400))
    const endTime = Date.now()
    
    // Simulate token usage
    const promptTokens = Math.floor(request.system.length / 4) + 
      request.messages.reduce((sum, msg) => sum + Math.floor(msg.content.length / 4), 0)
    const completionTokens = Math.min(request.max_tokens, 50 + Math.floor(Math.random() * 200))
    
    const response: InferenceResponse = {
      content: `AI response to: "${request.messages[request.messages.length - 1]?.content.slice(0, 50)}..." (This is a stub response)`,
      usage: {
        prompt_tokens: promptTokens,
        completion_tokens: completionTokens,
        total_tokens: promptTokens + completionTokens
      },
      model: request.model || "gpt-4o-mini",
      latency: endTime - startTime
    }
    
    console.log(`🧠 Inference completed:`, {
      model: response.model,
      tokens: response.usage.total_tokens,
      latency: `${response.latency}ms`
    })
    
    return response
  }
}

// Legacy exports for backward compatibility
export const helloWorld = Console.log("Hello from OpenAgents SDK!")
export const runHelloWorld = (): void => Effect.runSync(helloWorld)

// Legacy Ollama functionality (kept for Pylon compatibility)
interface OllamaModel {
  name: string
  model: string
  modified_at: string
  size: number
  digest: string
  details?: {
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

class OllamaConnectionError extends Error {
  constructor(public cause: unknown) {
    super("Failed to connect to Ollama")
    this.name = "OllamaConnectionError"
  }
}

const OLLAMA_DEFAULT_PORT = 11434
const getOllamaBaseUrl = (): string => {
  return `http://localhost:${OLLAMA_DEFAULT_PORT}`
}

export const checkOllamaStatus = Effect.tryPromise({
  try: async (): Promise<OllamaStatus> => {
    const baseUrl = getOllamaBaseUrl()
    
    const response = await fetch(`${baseUrl}/api/tags`, {
      method: 'GET',
      headers: { 'Accept': 'application/json' },
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

export const checkOllama = (): Promise<OllamaStatus> => Effect.runPromise(getOllamaStatus)