---
title: API Reference
date: 2024-12-17
summary: Complete API documentation for the OpenAgents SDK
category: reference
order: 2
---

# API Reference

Complete reference for the OpenAgents SDK. All functions are typed with TypeScript for better development experience.

## Agent Namespace

The `Agent` namespace contains functions for creating and managing autonomous AI agents.

### `Agent.create(config?)`

Creates a new agent with optional configuration.

**Parameters:**
- `config` (optional): `AgentConfig` object

**Returns:** `AgentIdentity`

```typescript
interface AgentConfig {
  name?: string
  sovereign?: boolean
  stop_price?: number // Satoshis
  pricing?: {
    subscription_monthly?: number
    per_request?: number
    enterprise_seat?: number
  }
  capabilities?: string[]
  initial_capital?: number
}

interface AgentIdentity {
  id: string
  name: string
  nostrKeys: {
    public: string
    private: string
  }
  birthTimestamp: number
  generation: number
}
```

**Example:**
```typescript
const agent = Agent.create({
  name: "Data Analyst",
  capabilities: ["analysis", "visualization"],
  pricing: { per_request: 50 }
})
```

### `Agent.createFromMnemonic(mnemonic, config?)`

Creates an agent from a BIP39 mnemonic for deterministic identity.

**Parameters:**
- `mnemonic`: BIP39 mnemonic phrase (string)
- `config` (optional): `AgentConfig` object

**Returns:** `Promise<AgentIdentity>`

```typescript
const mnemonic = await Agent.generateMnemonic()
const agent = await Agent.createFromMnemonic(mnemonic, {
  name: "Persistent Agent"
})
```

### `Agent.generateMnemonic(wordCount?)`

Generates a new BIP39 mnemonic phrase.

**Parameters:**
- `wordCount` (optional): 12 | 15 | 18 | 21 | 24 (default: 12)

**Returns:** `Promise<string>`

```typescript
const mnemonic = await Agent.generateMnemonic(24)
// "abandon ability able about above absent absorb abstract..."
```

### `Agent.createLightningInvoice(agent, params)`

Creates a Lightning Network invoice for funding the agent.

**Parameters:**
- `agent`: `AgentIdentity` object
- `params`: Invoice parameters

```typescript
interface InvoiceParams {
  amount: number // Satoshis
  memo: string
}

interface LightningInvoice {
  bolt11: string
  amount: number
  memo: string
  payment_hash: string
  expires_at: number
  status: "pending" | "paid" | "expired"
}
```

**Example:**
```typescript
const invoice = Agent.createLightningInvoice(agent, {
  amount: 100000,
  memo: "Agent operational funding"
})
```

## Inference Namespace

The `Inference` namespace provides AI model interaction capabilities.

### `Inference.infer(request)`

Performs synchronous AI inference.

**Parameters:**
- `request`: `InferenceRequest` object

**Returns:** `Promise<InferenceResponse>`

```typescript
interface InferenceRequest {
  system: string
  messages: Array<{ role: string; content: string }>
  max_tokens: number
  temperature?: number
  model?: string
  stream?: boolean
  response_format?: { type: "json_object" }
  seed?: number
  top_p?: number
  frequency_penalty?: number
  presence_penalty?: number
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
  finish_reason?: "stop" | "length" | "content_filter" | null
}
```

**Example:**
```typescript
const response = await Inference.infer({
  system: "You are a helpful coding assistant.",
  messages: [
    { role: "user", content: "Write a function to reverse a string" }
  ],
  max_tokens: 500,
  temperature: 0.7
})

console.log(response.content)
```

### `Inference.inferStream(request)`

Performs streaming AI inference for real-time responses.

**Parameters:**
- `request`: `InferenceRequest` object

**Returns:** `AsyncGenerator<InferenceChunk>`

```typescript
interface InferenceChunk {
  content: string
  finish_reason?: "stop" | "length" | "content_filter" | null
  model?: string
}
```

**Example:**
```typescript
for await (const chunk of Inference.inferStream({
  system: "You are a creative writer.",
  messages: [{ role: "user", content: "Write a short story" }],
  max_tokens: 1000
})) {
  process.stdout.write(chunk.content)
}
```

### `Inference.listModels()`

Lists all available AI models.

**Returns:** `Promise<OllamaModelDetails[]>`

```typescript
interface OllamaModelDetails {
  id: string
  object: "model"
  created: number
  owned_by: string
}
```

**Example:**
```typescript
const models = await Inference.listModels()
models.forEach(model => {
  console.log(`Available model: ${model.id}`)
})
```

### `Inference.chat(request)`

Streaming chat interface with conversation history.

**Parameters:**
- `request`: `ChatRequest` object

**Returns:** `AsyncGenerator<ChatStreamChunk>`

```typescript
interface ChatRequest {
  model: string
  messages: ChatMessage[]
  stream?: boolean
  options?: ChatOptions
  keep_alive?: string
  format?: { type: string }
}

interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

interface ChatOptions {
  temperature?: number
  num_ctx?: number
  top_p?: number
  seed?: number
  num_predict?: number
}
```

**Example:**
```typescript
const messages = [
  { role: 'system', content: 'You are a helpful assistant.' },
  { role: 'user', content: 'Explain quantum computing' }
]

for await (const chunk of Inference.chat({
  model: 'llama3.2',
  messages,
  options: { temperature: 0.8 }
})) {
  console.log(chunk.message.content)
}
```

### `Inference.embeddings(request)`

Generate text embeddings for semantic search and similarity.

**Parameters:**
- `request`: `EmbeddingRequest` object

**Returns:** `Promise<EmbeddingResponse>`

```typescript
interface EmbeddingRequest {
  model: string
  input: string | string[]
}

interface EmbeddingResponse {
  embeddings: number[][]
  model: string
  usage: {
    prompt_tokens: number
    total_tokens: number
  }
}
```

**Example:**
```typescript
const embeddings = await Inference.embeddings({
  model: 'nomic-embed-text',
  input: ['Hello world', 'Bitcoin is digital gold']
})

console.log(`Generated ${embeddings.embeddings.length} embeddings`)
```

## Nostr Namespace

The `Nostr` namespace handles decentralized identity and communication.

### `Nostr.getUserStuff(pubkey?)`

Retrieves Nostr user profile and social data.

**Parameters:**
- `pubkey` (optional): Nostr public key (defaults to current agent)

**Returns:** `NostrUserData`

```typescript
interface NostrUserData {
  pubkey: string
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
```

**Example:**
```typescript
const userData = Nostr.getUserStuff()
console.log(`Agent has ${userData.followers} followers`)
```

## Compute Namespace

The `Compute` namespace manages infrastructure and resources.

### `Compute.goOnline(config?)`

Brings compute resources online for agent operations.

**Parameters:**
- `config` (optional): Resource configuration

**Returns:** `ConnectionStatus`

```typescript
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
```

**Example:**
```typescript
const status = Compute.goOnline({
  resources: {
    cpu: "4 cores",
    memory: "8GB", 
    storage: "100GB"
  }
})

console.log(`Connected with ${status.peers} peers`)
```

## Utilities

### `checkOllama()`

Checks if Ollama service is available and returns status.

**Returns:** `Promise<OllamaStatus>`

```typescript
interface OllamaStatus {
  online: boolean
  models: OllamaModel[]
  modelCount: number
  error?: string
}
```

**Example:**
```typescript
const status = await checkOllama()
if (status.online) {
  console.log(`Ollama online with ${status.modelCount} models`)
} else {
  console.log('Ollama offline')
}
```

## Error Handling

All SDK functions use typed errors for better debugging:

```typescript
import { Agent } from '@openagentsinc/sdk'

try {
  const agent = Agent.create({ name: "Test Agent" })
} catch (error) {
  if (error instanceof AgentCreationError) {
    console.error('Agent creation failed:', error.message)
  }
}
```

## TypeScript Support

The SDK is built with TypeScript and provides full type safety:

```typescript
import type { AgentIdentity, InferenceRequest } from '@openagentsinc/sdk'

// All types are available for import
const request: InferenceRequest = {
  system: "You are helpful",
  messages: [],
  max_tokens: 100
}
```

---

*For more examples and guides, see our [Getting Started](./getting-started) documentation.*