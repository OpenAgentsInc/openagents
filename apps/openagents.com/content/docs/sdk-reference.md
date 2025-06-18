---
title: SDK Reference
date: 2024-12-17
summary: Complete API documentation for the OpenAgents SDK
category: reference
order: 2
---

# SDK Reference

Complete API documentation for the @openagentsinc/sdk package. The SDK provides AI agent creation, inference capabilities, and integration with local AI models through Ollama.

> ⚠️ **Early Development Notice**: The SDK is under active development. APIs may change between versions.

## Installation

```bash
pnpm add @openagentsinc/sdk
```

## Core Imports

```typescript
import { 
  Agent,           // Agent creation and management
  Inference,       // AI inference capabilities
  Compute,         // Resource management (placeholder)
  Nostr,           // Nostr protocol integration (placeholder)
  checkOllama      // Ollama connectivity check
} from '@openagentsinc/sdk'

// TypeScript types
import type {
  AgentIdentity,
  AgentLifecycleState,
  InferenceRequest,
  InferenceResponse,
  ChatMessage,
  OllamaStatus
} from '@openagentsinc/sdk'
```

## Agent Namespace

The `Agent` namespace provides functions for creating and managing AI agents.

### `Agent.create(config?)`

Creates a new agent with optional configuration.

**Parameters:**
- `config` (optional): `AgentConfig` object

**Returns:** `AgentIdentity`

```typescript
interface AgentConfig {
  name?: string              // Agent display name
  sovereign?: boolean        // Reserved for future use
  stop_price?: number        // Reserved for future use (satoshis)
  pricing?: {                // Reserved for future use
    subscription_monthly?: number
    per_request?: number
    enterprise_seat?: number
  }
  capabilities?: string[]    // Agent capabilities/features
  initial_capital?: number   // Reserved for future use
}

interface AgentIdentity {
  id: string                 // Unique agent identifier
  name: string               // Agent display name
  nostrKeys: {
    public: string           // Nostr public key (npub format)
    private: string          // Nostr private key
  }
  birthTimestamp: number     // Creation timestamp
  generation: number         // Agent generation (always 0)
}
```

**Example:**
```typescript
const agent = Agent.create({
  name: "Research Assistant",
  capabilities: ["research", "analysis", "summarization"]
})

console.log(`Created agent: ${agent.name} (${agent.id})`)
```

### `Agent.createFromMnemonic(mnemonic, config?)`

Creates an agent from a BIP39 mnemonic phrase for deterministic identity.

**Parameters:**
- `mnemonic`: BIP39 mnemonic phrase (string)
- `config` (optional): `AgentConfig` object

**Returns:** `Promise<AgentIdentity>`

**Example:**
```typescript
// Using an existing mnemonic
const mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
const agent = await Agent.createFromMnemonic(mnemonic, {
  name: "Deterministic Agent"
})

// The same mnemonic will always generate the same agent ID
```

### `Agent.generateMnemonic(wordCount?)`

Generates a new BIP39 mnemonic phrase.

**Parameters:**
- `wordCount` (optional): 12 | 15 | 18 | 21 | 24 (default: 12)

**Returns:** `Promise<string>`

**Example:**
```typescript
// Generate a 24-word mnemonic for extra security
const mnemonic = await Agent.generateMnemonic(24)
console.log(`Generated mnemonic: ${mnemonic}`)

// Save this mnemonic securely - it's the agent's identity seed
```

### `Agent.createLightningInvoice(agent, params)`

Creates a Lightning Network invoice (currently returns stub data).

> **Note**: Lightning payments are not yet implemented. This returns placeholder data.

**Parameters:**
- `agent`: `AgentIdentity` object
- `params`: Invoice parameters

**Returns:** `LightningInvoice`

```typescript
interface LightningInvoice {
  bolt11: string          // BOLT11 invoice string (stub)
  amount: number          // Amount in satoshis
  memo: string            // Invoice description
  payment_hash: string    // Payment hash
  expires_at: number      // Expiration timestamp
  status: "pending" | "paid" | "expired"
}
```

### Agent Lifecycle States

```typescript
enum AgentLifecycleState {
  BOOTSTRAPPING = "bootstrapping",  // Initial state
  ACTIVE = "active",                // Operational
  HIBERNATING = "hibernating",      // Low activity
  DYING = "dying",                  // Shutting down
  DEAD = "dead"                     // Terminated
}
```

## Inference Namespace

The `Inference` namespace provides AI model interactions via Ollama.

### `Inference.infer(request)`

Performs synchronous AI inference.

**Parameters:**
- `request`: `InferenceRequest` object

**Returns:** `Promise<InferenceResponse>`

```typescript
interface InferenceRequest {
  system: string                    // System prompt
  messages: Array<{                 // Conversation history
    role: string
    content: string
  }>
  max_tokens: number                // Maximum response tokens
  temperature?: number              // Randomness (0-2, default: 0.8)
  model?: string                    // Model name (default: "llama3.2")
  stream?: boolean                  // Enable streaming (use inferStream instead)
  response_format?: {               // Output format
    type: "json_object"
  }
  seed?: number                     // Reproducible generation
  top_p?: number                    // Nucleus sampling
  frequency_penalty?: number        // Reduce repetition
  presence_penalty?: number         // Encourage new topics
}

interface InferenceResponse {
  content: string                   // Generated text
  usage: {
    prompt_tokens: number           // Input token count
    completion_tokens: number       // Output token count
    total_tokens: number            // Total tokens used
  }
  model: string                     // Model used
  latency: number                   // Response time (ms)
  finish_reason?: "stop" | "length" | "content_filter" | null
}
```

**Example:**
```typescript
const response = await Inference.infer({
  system: "You are a helpful coding assistant. Be concise.",
  messages: [
    { role: "user", content: "Write a TypeScript function to validate email addresses" }
  ],
  model: "llama3.2",
  max_tokens: 500,
  temperature: 0.7
})

console.log(response.content)
console.log(`Generated ${response.usage.completion_tokens} tokens in ${response.latency}ms`)
```

### `Inference.inferStream(request)`

Performs streaming AI inference for real-time responses.

**Parameters:**
- `request`: `InferenceRequest` object

**Returns:** `AsyncGenerator<InferenceChunk>`

```typescript
interface InferenceChunk {
  content: string                   // Partial content
  finish_reason?: "stop" | "length" | "content_filter" | null
  model?: string                    // Model name
}
```

**Example:**
```typescript
console.log("Assistant: ", end="")
for await (const chunk of Inference.inferStream({
  system: "You are a creative writer.",
  messages: [
    { role: "user", content: "Write a haiku about programming" }
  ],
  max_tokens: 100
})) {
  process.stdout.write(chunk.content)
  
  if (chunk.finish_reason === "stop") {
    console.log("\n\nCompleted!")
  }
}
```

### `Inference.chat(request)`

Streaming chat interface with conversation history support.

**Parameters:**
- `request`: `ChatRequest` object

**Returns:** `AsyncGenerator<ChatStreamChunk>`

```typescript
interface ChatRequest {
  model: string                     // Model name
  messages: ChatMessage[]           // Full conversation
  stream?: boolean                  // Stream response (default: true)
  options?: ChatOptions             // Model parameters
  keep_alive?: string               // Model memory duration
  format?: { type: string }         // Output format
}

interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

interface ChatOptions {
  temperature?: number              // Randomness
  num_ctx?: number                  // Context window size
  top_p?: number                    // Nucleus sampling
  seed?: number                     // Reproducible output
  num_predict?: number              // Max tokens to generate
}

interface ChatStreamChunk {
  model: string
  created_at: string
  message: {
    role: 'assistant'
    content: string
  }
  done: boolean                     // Indicates completion
  done_reason?: string
  total_duration?: number           // Total time (nanoseconds)
  load_duration?: number            // Model load time
  prompt_eval_count?: number        // Prompt tokens evaluated
  prompt_eval_duration?: number     // Prompt eval time
  eval_count?: number               // Tokens generated
  eval_duration?: number            // Generation time
}
```

**Example:**
```typescript
const conversation: ChatMessage[] = [
  { role: 'system', content: 'You are a TypeScript expert.' },
  { role: 'user', content: 'What are generics?' },
  { role: 'assistant', content: 'Generics are a way to create reusable components...' },
  { role: 'user', content: 'Can you show me an example?' }
]

for await (const chunk of Inference.chat({
  model: 'llama3.2',
  messages: conversation,
  options: {
    temperature: 0.8,
    num_predict: 300
  }
})) {
  process.stdout.write(chunk.message.content)
  
  if (chunk.done) {
    console.log(`\n\nStats: ${chunk.eval_count} tokens in ${chunk.eval_duration}ns`)
  }
}
```

### `Inference.listModels()`

Lists all available AI models in Ollama.

**Returns:** `Promise<OllamaModelDetails[]>`

```typescript
interface OllamaModelDetails {
  id: string                        // Model identifier
  object: "model"
  created: number                   // Unix timestamp
  owned_by: string                  // Always "ollama"
}
```

**Example:**
```typescript
const models = await Inference.listModels()

console.log("Available models:")
models.forEach(model => {
  const date = new Date(model.created * 1000)
  console.log(`- ${model.id} (added: ${date.toLocaleDateString()})`)
})
```

### `Inference.embeddings(request)`

Generate text embeddings for semantic search and similarity.

**Parameters:**
- `request`: `EmbeddingRequest` object

**Returns:** `Promise<EmbeddingResponse>`

```typescript
interface EmbeddingRequest {
  model: string                     // Embedding model name
  input: string | string[]          // Text(s) to embed
}

interface EmbeddingResponse {
  embeddings: number[][]            // Vector embeddings
  model: string                     // Model used
  usage: {
    prompt_tokens: number           // Tokens processed
    total_tokens: number            // Total token count
  }
}
```

**Example:**
```typescript
// Generate embeddings for semantic search
const response = await Inference.embeddings({
  model: 'nomic-embed-text',
  input: [
    'OpenAgents SDK documentation',
    'Building AI agents with TypeScript',
    'Local AI inference with Ollama'
  ]
})

console.log(`Generated ${response.embeddings.length} embeddings`)
console.log(`Embedding dimensions: ${response.embeddings[0].length}`)

// Calculate cosine similarity between first two embeddings
function cosineSimilarity(a: number[], b: number[]): number {
  const dotProduct = a.reduce((sum, val, i) => sum + val * b[i], 0)
  const magnitudeA = Math.sqrt(a.reduce((sum, val) => sum + val * val, 0))
  const magnitudeB = Math.sqrt(b.reduce((sum, val) => sum + val * val, 0))
  return dotProduct / (magnitudeA * magnitudeB)
}

const similarity = cosineSimilarity(
  response.embeddings[0], 
  response.embeddings[1]
)
console.log(`Similarity: ${(similarity * 100).toFixed(2)}%`)
```

## Nostr Namespace

The `Nostr` namespace provides decentralized identity and communication features.

> **Note**: Currently returns placeholder data. Full Nostr integration is available via the @openagentsinc/nostr package.

### `Nostr.getUserStuff(pubkey?)`

Retrieves Nostr user profile and social data.

**Parameters:**
- `pubkey` (optional): Nostr public key

**Returns:** `NostrUserData`

```typescript
interface NostrUserData {
  pubkey: string                    // Nostr public key
  profile?: {
    name?: string                   // Display name
    about?: string                  // Bio/description
    picture?: string                // Avatar URL
    nip05?: string                  // NIP-05 identifier
  }
  relays: string[]                  // Connected relays
  followers: number                 // Follower count
  following: number                 // Following count
}
```

## Compute Namespace

The `Compute` namespace manages infrastructure and resources.

> **Note**: Currently returns placeholder data. Container deployment features are planned.

### `Compute.goOnline(config?)`

Brings compute resources online.

**Parameters:**
- `config` (optional): Resource configuration

**Returns:** `ConnectionStatus`

```typescript
interface ConnectionStatus {
  connected: boolean
  peers?: number                    // Connected peers
  resources?: {
    cpu: string                     // CPU allocation
    memory: string                  // Memory allocation
    storage: string                 // Storage allocation
  }
  uptime?: number                   // Uptime timestamp
}
```

## Utility Functions

### `checkOllama()`

Checks if Ollama service is available and returns its status.

**Returns:** `Promise<OllamaStatus>`

```typescript
interface OllamaStatus {
  online: boolean                   // Service availability
  models: OllamaModel[]             // Available models
  modelCount: number                // Total model count
  error?: string                    // Error message if offline
}

interface OllamaModel {
  name: string                      // Model name
  model: string                     // Model identifier
  modified_at: string               // Last modified
  size: number                      // Size in bytes
  digest: string                    // Model hash
  details?: {
    parameter_size?: string         // Model size (e.g., "7B")
    quantization_level?: string     // Quantization (e.g., "Q4_0")
  }
}
```

**Example:**
```typescript
// Check Ollama before using inference
const status = await checkOllama()

if (!status.online) {
  console.error('Ollama is not running. Please start it with:')
  console.error('ollama serve')
  process.exit(1)
}

console.log(`Ollama is online with ${status.modelCount} models:`)
status.models.forEach(model => {
  const sizeMB = (model.size / 1024 / 1024).toFixed(0)
  console.log(`- ${model.name} (${sizeMB} MB)`)
})
```

## Error Handling

The SDK uses standard JavaScript errors. Common error scenarios:

```typescript
try {
  const response = await Inference.infer({
    system: "You are helpful",
    messages: [{ role: "user", content: "Hello" }],
    max_tokens: 100
  })
} catch (error) {
  if (error.message.includes("Ollama is not available")) {
    console.error("Please ensure Ollama is running")
  } else if (error.message.includes("model")) {
    console.error("Model not found. Pull it with: ollama pull <model>")
  } else {
    console.error("Inference failed:", error.message)
  }
}
```

## TypeScript Support

The SDK is written in TypeScript and provides full type definitions:

```typescript
import type {
  // Agent types
  AgentIdentity,
  AgentConfig,
  AgentLifecycleState,
  
  // Inference types
  InferenceRequest,
  InferenceResponse,
  InferenceChunk,
  ChatRequest,
  ChatMessage,
  ChatStreamChunk,
  
  // Embedding types
  EmbeddingRequest,
  EmbeddingResponse,
  
  // Utility types
  OllamaStatus,
  OllamaModel,
  
  // Placeholder types
  LightningInvoice,
  NostrUserData,
  ConnectionStatus
} from '@openagentsinc/sdk'
```

## Best Practices

### 1. Always Check Ollama Status

```typescript
// Start of your application
const status = await checkOllama()
if (!status.online) {
  throw new Error("Ollama must be running")
}
```

### 2. Handle Streaming Errors

```typescript
try {
  for await (const chunk of Inference.inferStream(request)) {
    // Process chunk
  }
} catch (error) {
  console.error("Streaming failed:", error)
  // Implement retry logic if needed
}
```

### 3. Use Appropriate Models

```typescript
// For code: use code-specific models
const codeResponse = await Inference.infer({
  model: "codellama",
  // ...
})

// For embeddings: use embedding models
const embeddings = await Inference.embeddings({
  model: "nomic-embed-text",
  // ...
})
```

### 4. Manage Conversation Context

```typescript
// Keep conversation history manageable
const MAX_HISTORY = 10
let messages: ChatMessage[] = [
  { role: 'system', content: 'You are helpful.' }
]

function addMessage(role: 'user' | 'assistant', content: string) {
  messages.push({ role, content })
  
  // Keep only recent messages plus system prompt
  if (messages.length > MAX_HISTORY + 1) {
    messages = [messages[0], ...messages.slice(-MAX_HISTORY)]
  }
}
```

## Examples

### Complete Example: Research Assistant

```typescript
import { Agent, Inference, checkOllama } from '@openagentsinc/sdk'

async function createResearchAssistant() {
  // Ensure Ollama is running
  const status = await checkOllama()
  if (!status.online) {
    throw new Error("Ollama is required")
  }
  
  // Create specialized agent
  const agent = Agent.create({
    name: "Research Assistant",
    capabilities: ["research", "analysis", "summarization"]
  })
  
  console.log(`Created ${agent.name} (${agent.id})`)
  
  // Research function
  async function research(topic: string): Promise<string> {
    const response = await Inference.infer({
      system: `You are a research assistant. Provide comprehensive, 
               well-structured information about the given topic. 
               Include key facts, recent developments, and cite sources 
               when possible.`,
      messages: [
        { role: "user", content: `Research this topic: ${topic}` }
      ],
      model: "llama3.2",
      max_tokens: 1000,
      temperature: 0.7
    })
    
    return response.content
  }
  
  // Summarization function
  async function summarize(text: string, style: 'bullet' | 'paragraph' = 'bullet'): Promise<string> {
    const response = await Inference.infer({
      system: `You are an expert at summarization. Create a ${style} summary.`,
      messages: [
        { role: "user", content: `Summarize this text:\n\n${text}` }
      ],
      model: "llama3.2",
      max_tokens: 300,
      temperature: 0.3
    })
    
    return response.content
  }
  
  return { agent, research, summarize }
}

// Usage
const assistant = await createResearchAssistant()
const research = await assistant.research("Effect.js for TypeScript")
const summary = await assistant.summarize(research, 'bullet')

console.log("Research Results:")
console.log(research)
console.log("\nSummary:")
console.log(summary)
```

---

*For more examples and guides, check out the [Getting Started](./getting-started) guide or browse the [source code](https://github.com/OpenAgentsInc/openagents).*