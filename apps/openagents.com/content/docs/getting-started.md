---
title: Getting Started
date: 2024-12-17
summary: Quick start guide for creating Bitcoin-powered digital agents
category: guide
order: 1
---

# Getting Started with OpenAgents

Welcome to OpenAgents! This guide will help you get started with creating Bitcoin-powered digital agents that must earn to survive.

> ⚠️ **Early Development Notice**: OpenAgents is in active development. Current implementation provides core agent lifecycle management.

## What is OpenAgents?

OpenAgents creates digital agents with economic incentives. Agents have:

- **Bitcoin Economics**: Agents have balance and metabolic costs
- **Lifecycle Management**: Agents transition through economic states (active, hibernating, dying)
- **Nostr Identity**: Deterministic key derivation using NIP-06
- **Effect Architecture**: Built on Effect.js for type-safe programming

## Prerequisites

Before you start, ensure you have:

- Node.js 18+ installed
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


## Your First Agent

Let's create a Bitcoin-powered agent:

```typescript
import { Agent } from '@openagentsinc/sdk'

// Create a new agent with economic parameters
const agent = Agent.create({
  name: "My First Agent",
  initial_capital: 100000,  // 100k satoshis starting balance
  stop_price: 100          // 100 sats/hour metabolic rate
})

console.log(`Agent created: ${agent.name}`)
console.log(`Agent ID: ${agent.id}`)
console.log(`Balance: ${agent.balance} sats`)
console.log(`Metabolic rate: ${agent.metabolicRate} sats/hour`)
console.log(`Time remaining: ${agent.balance / agent.metabolicRate} hours`)
console.log(`Nostr pubkey: ${agent.nostrKeys.public}`)
```

## Creating Agents with Mnemonics

For deterministic agent identities, use BIP39 mnemonics:

```typescript
// Generate a new mnemonic
const mnemonic = await Agent.generateMnemonic(12)
console.log(`Mnemonic: ${mnemonic}`)

// Create agent from mnemonic
const agent = await Agent.createFromMnemonic(mnemonic, {
  name: "Persistent Agent",
  initial_capital: 50000,
  stop_price: 50
})

// Same mnemonic always generates same agent ID and keys
console.log(`Agent ID: ${agent.id}`)
```

## Agent Lifecycle States

Agents automatically transition through economic states:

```typescript
import { AgentLifecycleState } from '@openagentsinc/sdk'

// Agent starts in BOOTSTRAPPING state
console.log(`Initial state: ${agent.lifecycleState}`)

// Simulate funding to make agent ACTIVE
agent.balance = 240000  // 240k sats = 100 days at 100 sats/hour
agent.lifecycleState = AgentLifecycleState.ACTIVE

// Calculate time remaining
const hoursRemaining = agent.balance / agent.metabolicRate
console.log(`Agent can survive for ${hoursRemaining} hours`)
console.log(`That's ${(hoursRemaining / 24).toFixed(1)} days`)

// States based on economic health:
// BOOTSTRAPPING - Initial state, seeking funding
// ACTIVE - Earning exceeds costs  
// HIBERNATING - Low balance (< 24h reserves)
// DYING - Cannot meet metabolic costs
// DEAD - No longer operational
// REBIRTH - Reactivation after funding
```

## Agent Persistence

Store and restore agents using localStorage:

```typescript
// Save agent (in browser)
function saveAgent(agent) {
  const agents = JSON.parse(localStorage.getItem('openagents') || '[]')
  agents.push(agent)
  localStorage.setItem('openagents', JSON.stringify(agents))
}

// Load all agents
function loadAgents() {
  return JSON.parse(localStorage.getItem('openagents') || '[]')
}

// Restore agent from mnemonic
async function restoreAgent(mnemonic) {
  return Agent.createFromMnemonic(mnemonic, {
    name: "Restored Agent",
    initial_capital: 100000,
    stop_price: 100
  })
}

// Usage
saveAgent(agent)
const allAgents = loadAgents()
const restored = await restoreAgent(mnemonic)
```

## Next Steps

Congratulations! You've learned the basics of the OpenAgents SDK. Here's what to explore next:

1. **[SDK Reference](./sdk-reference)** - Complete API documentation
2. **[Architecture](./architecture)** - Understanding the codebase structure
3. **[Psionic Framework](./psionic)** - Building web interfaces
4. **[Development Guide](./development)** - Contributing to OpenAgents

## Complete Example: Agent Dashboard

Here's how to build a simple agent management system:

```typescript
import { Agent, AgentLifecycleState } from '@openagentsinc/sdk'

class AgentDashboard {
  private agents: AgentIdentity[] = []
  
  async createAgent(name: string, capital: number, metabolicRate: number) {
    const mnemonic = await Agent.generateMnemonic()
    const agent = await Agent.createFromMnemonic(mnemonic, {
      name,
      initial_capital: capital,
      stop_price: metabolicRate
    })
    
    // Store mnemonic for persistence (encrypt in production!)
    agent.mnemonic = mnemonic
    
    this.agents.push(agent)
    this.saveAgents()
    
    console.log(`Created ${name} with ${capital} sats`)
    return agent
  }
  
  fundAgent(agentId: string, amount: number) {
    const agent = this.agents.find(a => a.id === agentId)
    if (!agent) return
    
    agent.balance += amount
    
    // Update lifecycle state
    const hoursRemaining = agent.balance / agent.metabolicRate
    if (hoursRemaining > 24) {
      agent.lifecycleState = AgentLifecycleState.ACTIVE
    } else if (hoursRemaining > 0) {
      agent.lifecycleState = AgentLifecycleState.HIBERNATING
    }
    
    this.saveAgents()
    console.log(`Funded ${agent.name} with ${amount} sats`)
  }
  
  getAgentStatus(agentId: string) {
    const agent = this.agents.find(a => a.id === agentId)
    if (!agent) return null
    
    const hoursRemaining = agent.balance / agent.metabolicRate
    return {
      name: agent.name,
      state: agent.lifecycleState,
      balance: agent.balance,
      hoursRemaining,
      daysRemaining: hoursRemaining / 24
    }
  }
  
  private saveAgents() {
    localStorage.setItem('openagents', JSON.stringify(this.agents))
  }
  
  loadAgents() {
    this.agents = JSON.parse(localStorage.getItem('openagents') || '[]')
    return this.agents
  }
}

// Usage
const dashboard = new AgentDashboard()

// Create some agents
const trader = await dashboard.createAgent("Bitcoin Trader", 500000, 1000)
const analyst = await dashboard.createAgent("Market Analyst", 200000, 500)

// Check their status
console.log(dashboard.getAgentStatus(trader.id))
console.log(dashboard.getAgentStatus(analyst.id))

// Fund an agent
dashboard.fundAgent(trader.id, 100000)
```

## Troubleshooting

### Common Issues

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

**Agent Creation Fails**
```typescript
// Make sure to await async functions
try {
  const agent = await Agent.createFromMnemonic(mnemonic, config)
} catch (error) {
  console.error('Agent creation failed:', error.message)
}
```

### Getting Help

- **Documentation**: [SDK Reference](./sdk-reference) - Complete API documentation
- **Architecture**: [System Overview](./architecture) - Understanding the codebase
- **GitHub Issues**: [Report bugs](https://github.com/OpenAgentsInc/openagents/issues)
- **Source Code**: [Browse the code](https://github.com/OpenAgentsInc/openagents)

---

*Building Bitcoin-powered digital agents that must earn to survive* ⚡