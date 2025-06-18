---
title: Getting Started
date: 2024-12-17
summary: Quick start guide for using the OpenAgents SDK
category: guide
order: 1
---

# Getting Started with OpenAgents

Welcome to OpenAgents! This guide will help you get started with the OpenAgents SDK for building AI-powered applications.

> ⚠️ **Early Development Notice**: OpenAgents is heavily in development and not production-ready. APIs may change significantly.

## What is OpenAgents?

OpenAgents is a platform for building AI agents using open protocols. The vision is to create Bitcoin-powered autonomous agents, though the current implementation focuses on providing:

- **Local AI Integration**: Interface with Ollama for privacy-preserving AI
- **Effect-based Architecture**: Built on Effect.js for type-safe, functional programming
- **Nostr Protocol**: Decentralized identity and communication (via our Nostr package)
- **Modular Design**: Clean separation between SDK, UI, and application layers

## Prerequisites

Before you start, ensure you have:

- Node.js 18+ installed
- [Ollama](https://ollama.com) installed and running locally
- Basic understanding of JavaScript/TypeScript
- pnpm package manager (recommended)

## Installation

Install the OpenAgents SDK:

```bash
# Using pnpm (recommended)
pnpm add @openagentsinc/sdk

# Using npm
npm install @openagentsinc/sdk

# Using yarn
yarn add @openagentsinc/sdk
```

### Setting up Ollama

The SDK uses Ollama for local AI inference. Install and start Ollama:

```bash
# Install Ollama (macOS)
brew install ollama

# Start Ollama service
ollama serve

# Pull a model
ollama pull llama3.2
```

## Your First Agent

Let's create a simple AI agent:

```typescript
import { Agent } from '@openagentsinc/sdk'

// Create a new agent
const agent = Agent.create({
  name: "My First Agent",
  capabilities: ["chat", "analysis"]
})

console.log(`Agent created: ${agent.name}`)
console.log(`Agent ID: ${agent.id}`)
console.log(`Nostr pubkey: ${agent.nostrKeys.public}`)
```

> **Note**: The current implementation generates placeholder Nostr keys. Full Nostr integration with NIP-06 key derivation is planned but not yet implemented.

## Creating Agents with Mnemonics

For deterministic agent identities, you can use BIP39 mnemonics:

```typescript
// Generate a new mnemonic
const mnemonic = await Agent.generateMnemonic(12)
console.log(`Mnemonic: ${mnemonic}`)

// Create agent from mnemonic
const agent = await Agent.createFromMnemonic(mnemonic, {
  name: "Persistent Agent"
})
```

## Using AI Inference

The SDK provides comprehensive AI capabilities through Ollama:

### Basic Inference

```typescript
import { Inference } from '@openagentsinc/sdk'

// Check if Ollama is available
const status = await checkOllama()
if (!status.online) {
  console.error('Ollama is not running. Please start it with: ollama serve')
  process.exit(1)
}

// Perform inference
const response = await Inference.infer({
  system: "You are a helpful assistant.",
  messages: [{ role: "user", content: "Explain quantum computing in simple terms" }],
  model: "llama3.2",
  max_tokens: 200,
  temperature: 0.7
})

console.log(response.content)
console.log(`Tokens used: ${response.usage.total_tokens}`)
console.log(`Latency: ${response.latency}ms`)
```

### Streaming Responses

For real-time AI responses:

```typescript
// Stream response tokens as they arrive
for await (const chunk of Inference.inferStream({
  system: "You are a creative storyteller.",
  messages: [{ role: "user", content: "Tell me a short story about robots" }],
  max_tokens: 500
})) {
  process.stdout.write(chunk.content)
  
  if (chunk.finish_reason) {
    console.log(`\n\nFinished: ${chunk.finish_reason}`)
  }
}
```

### Chat Interface

For conversational AI with history:

```typescript
const messages = [
  { role: 'system' as const, content: 'You are a knowledgeable assistant.' },
  { role: 'user' as const, content: 'What is TypeScript?' },
  { role: 'assistant' as const, content: 'TypeScript is a typed superset of JavaScript...' },
  { role: 'user' as const, content: 'What are its main benefits?' }
]

for await (const chunk of Inference.chat({
  model: 'llama3.2',
  messages,
  options: {
    temperature: 0.8,
    num_predict: 300
  }
})) {
  if (chunk.message.content) {
    process.stdout.write(chunk.message.content)
  }
  
  if (chunk.done) {
    console.log('\n\nChat completed')
  }
}
```

## Working with Models

### List Available Models

```typescript
// See what models are available
const models = await Inference.listModels()
models.forEach(model => {
  console.log(`- ${model.id} (created: ${new Date(model.created * 1000).toLocaleDateString()})`)
})
```

### Generate Embeddings

For semantic search and similarity:

```typescript
const embeddings = await Inference.embeddings({
  model: 'nomic-embed-text',
  input: ['OpenAgents SDK', 'AI development', 'Bitcoin payments']
})

console.log(`Generated ${embeddings.embeddings.length} embeddings`)
console.log(`Dimensions: ${embeddings.embeddings[0].length}`)
```

## Next Steps

Congratulations! You've learned the basics of the OpenAgents SDK. Here's what to explore next:

1. **[SDK Reference](./sdk-reference)** - Complete API documentation
2. **[Architecture](./architecture)** - Understanding the codebase structure
3. **[Psionic Framework](./psionic)** - Building web interfaces
4. **[Development Guide](./development)** - Contributing to OpenAgents

## Example: Building a Code Assistant

Here's a complete example of building a code review assistant:

```typescript
import { Agent, Inference } from '@openagentsinc/sdk'
import { Effect } from 'effect'

// Create a specialized agent
const codeAssistant = Agent.create({
  name: "Code Review Assistant",
  capabilities: ["code-review", "refactoring", "testing"]
})

// Code review function
async function reviewCode(code: string, language: string) {
  const response = await Inference.infer({
    system: `You are an expert ${language} developer. Review the provided code for:
    - Potential bugs
    - Performance issues
    - Security concerns
    - Code style improvements
    Provide specific, actionable feedback.`,
    messages: [
      { role: "user", content: `Please review this ${language} code:\n\n${code}` }
    ],
    model: "llama3.2",
    max_tokens: 1000,
    temperature: 0.3 // Lower temperature for more focused analysis
  })
  
  return response.content
}

// Example usage
const pythonCode = `
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)
`

const review = await reviewCode(pythonCode, "Python")
console.log("Code Review Results:")
console.log(review)
```

## Troubleshooting

### Common Issues

**Ollama Connection Failed**
```bash
# Ensure Ollama is running
ollama serve

# Check connection
curl http://localhost:11434/api/tags
```

**Import Errors**
```json
// Ensure your tsconfig.json includes:
{
  "compilerOptions": {
    "module": "ESNext",
    "moduleResolution": "node",
    "target": "ES2022"
  }
}
```

**Model Not Found**
```bash
# Pull the required model
ollama pull llama3.2
```

### Getting Help

- **Documentation**: [Full SDK Reference](./sdk-reference)
- **GitHub Issues**: [Report bugs](https://github.com/OpenAgentsInc/openagents/issues)
- **Source Code**: [Browse examples](https://github.com/OpenAgentsInc/openagents)

---

*Building the future of AI agents with open protocols and local inference* 🤖