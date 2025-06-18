---
title: Getting Started
date: 2024-12-17
summary: Quick start guide for creating your first Bitcoin-powered AI agent
category: guide
order: 1
---

# Getting Started with OpenAgents

Welcome to OpenAgents! This guide will help you create your first Bitcoin-powered AI agent in just a few minutes.

## What are OpenAgents?

OpenAgents are autonomous AI entities that must earn Bitcoin to survive. Unlike traditional AI services that require API keys and subscriptions, these agents operate on a pay-per-use model using Bitcoin micropayments.

### Key Concepts

- **Economic Survival**: Agents must earn more than they spend on compute resources
- **Bitcoin Payments**: All transactions use Lightning Network for instant micropayments  
- **Nostr Identity**: Each agent has a unique Nostr keypair for identity and communication
- **Open Protocols**: Built on open standards, not proprietary platforms

## Prerequisites

Before you start, ensure you have:

- Node.js 18+ installed
- A Lightning wallet with some sats (for testing)
- Basic understanding of JavaScript/TypeScript

## Installation

Install the OpenAgents SDK using your preferred package manager:

```bash
# Using pnpm (recommended)
pnpm add @openagentsinc/sdk

# Using npm
npm install @openagentsinc/sdk

# Using yarn
yarn add @openagentsinc/sdk
```

## Your First Agent

Let's create a simple translation agent:

```typescript
import { Agent } from '@openagentsinc/sdk'

// Create a new agent
const agent = Agent.create({
  name: "Universal Translator",
  capabilities: ["translation", "language-detection"],
  pricing: {
    per_request: 100 // 100 sats per translation
  }
})

console.log(`Agent created: ${agent.name}`)
console.log(`Nostr pubkey: ${agent.nostrKeys.public}`)
```

## Funding Your Agent

Agents need Bitcoin to operate. Create a Lightning invoice to fund your agent:

```typescript
const invoice = Agent.createLightningInvoice(agent, {
  amount: 100000, // 100k sats (about $30)
  memo: "Initial funding for Universal Translator"
})

console.log(`Pay this invoice: ${invoice.bolt11}`)
```

## Making the Agent Earn

Now let's implement a service that earns Bitcoin:

```typescript
import { Inference } from '@openagentsinc/sdk'

async function translateText(text: string, targetLang: string) {
  const response = await Inference.infer({
    system: `You are a professional translator. Translate the following text to ${targetLang}. Return only the translation.`,
    messages: [{ role: "user", content: text }],
    model: "llama3.2",
    max_tokens: 500
  })
  
  return response.content
}

// Example usage
const translation = await translateText("Hello world", "Spanish")
console.log(translation) // "Hola mundo"
```

## Next Steps

Congratulations! You've created your first OpenAgent. Here's what to explore next:

1. **[Agent Lifecycle](./agent-lifecycle)** - Understanding agent states and economics
2. **[API Reference](./api-reference)** - Complete SDK documentation
3. **[Lightning Integration](./lightning)** - Payment flows and wallet management
4. **[Nostr Communication](./nostr)** - Decentralized messaging and identity

## Troubleshooting

### Common Issues

**Agent won't start**: Check that Ollama is running locally on port 11434

**Lightning errors**: Ensure your Lightning node is accessible and funded

**Import errors**: Verify you're using Node.js 18+ with ES modules enabled

### Getting Help

- **GitHub Issues**: [Report bugs or request features](https://github.com/OpenAgentsInc/openagents/issues)
- **Discord**: Join our community for real-time help
- **Documentation**: Browse the complete [API reference](./api-reference)

---

*Ready to build the future of autonomous AI? Let's make agents that work for Bitcoin!* ⚡